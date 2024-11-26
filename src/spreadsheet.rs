use crate::cli::JournalInputs;
use crate::model::JournalEntry;
use anyhow::{bail, Context};
use chrono::{DateTime, Datelike, Local, Timelike};
use log::{info, trace, warn};
use std::collections::HashMap;
use std::fs;
use umya_spreadsheet::helper::date::{convert_date, excel_to_date_time_object};
use umya_spreadsheet::{Spreadsheet, Worksheet};

const DEFAULT_SHEET_NAME: &'static str = "Journal";

//Columns start at 1 (not 0)
const COLUMN_DATE: u32 = 1;
const COLUMN_SUMMARY: u32 = 2;
const COLUMN_DESCRIPTION: u32 = 3;
const COLUMN_DURATION: u32 = 4;
const COLUMN_COMMIT: u32 = 5;
const COLUMN_REFERENCE: u32 = 6;

pub(crate) fn merge_entries_with_spreadsheet(
    journal_inputs: &JournalInputs,
    entries: &mut Vec<JournalEntry>,
) -> anyhow::Result<u32> {
    info!("Merging {} entries with spreadsheet", entries.len());

    //Create workbook if needed
    let path = std::path::Path::new(&journal_inputs.file);
    let mut workbook: Spreadsheet = if !fs::exists(&journal_inputs.file)
        .with_context(|| format!("Failed to check existence of {}", &journal_inputs.file))?
    {
        umya_spreadsheet::new_file()
    } else {
        umya_spreadsheet::reader::xlsx::read(path)?
    };

    //Create worksheet if needed
    let worksheet = match workbook.get_sheet_mut(&(0usize)) {
        Some(sheet) => sheet,
        None => workbook.new_sheet(DEFAULT_SHEET_NAME).unwrap(),
    };

    //Force ordering by date DESC for faster processing later (vec.pop)
    entries.sort_by(|entry1, entry2| entry2.date.cmp(&entry1.date));

    let mut previous_date: Option<DateTime<Local>> = None; //spreadsheet ordering check

    //rows start at 1, skip header
    for mut row_num in 2..=worksheet.get_highest_row() {
        let date = worksheet
            .get_cell_value((&COLUMN_DATE, &row_num))
            .get_value_number();
        let commit = worksheet
            .get_cell_value((&COLUMN_COMMIT, &row_num))
            .get_value()
            .to_string();

        //trace!("{:?}",worksheet.get_cell((&COLUMN_COMMIT, &row_num)));

        //Found a valid line, insert optional missing entries
        trace!("line {} => date: {:?}, commit: {:?}", row_num, date, commit);
        if date.is_some() && !commit.is_empty() {
            let date = DateTime::from_naive_utc_and_offset(
                excel_to_date_time_object(&date.unwrap(), None),
                *Local::now().fixed_offset().offset(),
            );

            if previous_date.is_none() {
                previous_date = Some(date);
            } else if date.lt(&previous_date.unwrap()) {
                bail!(format!("spreadsheet entries are not ordered by date ASC (new date: {} < previous: {}), please fix it",
                date,previous_date.unwrap()))
            }

            //Entries are ordered by date DESC, so we look from the end...
            while !&entries.is_empty() && date.ge(&entries.last().unwrap().date)
            // some entries are missing in the spreadsheet
            {
                let entry = entries.pop().unwrap();
                if entry.id != commit {
                    insert_entry(worksheet, &row_num, &entry);
                    row_num += 1;
                } else {
                    info!(
                        "discarding remote commit {} as already in spreadsheet",
                        commit
                    )
                }
            }
        } else {
            warn!("invalid line {row_num}, skipping")
        }
    }

    //add any more recent missing entries
    if !entries.is_empty() {
        //Add header row if empty
        if worksheet.get_highest_row() == 0 {
            HashMap::from([
                (COLUMN_DATE, "Date"),
                (COLUMN_SUMMARY, "Résumé"),
                (COLUMN_DESCRIPTION, "Description"),
                (COLUMN_DURATION, "Durée (min)"),
                (COLUMN_COMMIT, "Lien vers le commit"),
                (COLUMN_REFERENCE, "Référence"),
            ])
            .iter()
            .for_each(|(column, value)| {
                worksheet
                    .get_cell_mut((*column, 1))
                    .set_value_string(*value);
            });
        }
        entries.iter().rev().for_each(|entry| {
            insert_entry(
                worksheet,
                &(worksheet.get_highest_row() + 1/*insert one after the other*/),
                entry,
            )
        });
    }

    if fs::exists(&journal_inputs.file)? && journal_inputs.backup {
        let backup = path.with_file_name(format!(
            "{}-{}.{}",
            path.file_stem().unwrap().to_string_lossy(),
            Local::now().format("%Y-%m-%d_%H-%M-%S").to_string(),
            path.extension().unwrap().to_string_lossy()
        ));
        fs::copy(&path, backup.as_path()).with_context(|| format!("Cannot backup {:?}", path))?;
    }

    let last_row = worksheet.get_highest_row(); // after write, not available anymore
    umya_spreadsheet::writer::xlsx::write(&workbook, path).with_context(|| {
        format!(
            "error updating spreadsheet content into {}, is it already opened ?",
            &journal_inputs.file
        )
    })?;

    Ok(last_row)
}

fn insert_entry(worksheet: &mut Worksheet, row_num: &u32, entry: &JournalEntry) {
    trace!("inserting {:?}", entry);
    worksheet.insert_new_row(row_num, &1);

    let date = entry.date;
    let date_cell = worksheet.get_cell_mut((&COLUMN_DATE, row_num));
    date_cell.set_value_number(convert_date(
        date.year(),
        date.month() as i32,
        date.day() as i32,
        date.hour() as i32,
        date.minute() as i32,
        date.second() as i32,
    ));
    date_cell
        .get_style_mut()
        .get_number_format_mut()
        .set_format_code("dd.mm.yyyy hh:mm:ss");

    worksheet
        .get_cell_mut((&COLUMN_SUMMARY, row_num))
        .set_value_string(&entry.summary);
    worksheet
        .get_cell_mut((&COLUMN_DESCRIPTION, row_num))
        .set_value_string(&entry.description);
    worksheet
        .get_cell_mut((&COLUMN_DURATION, row_num))
        .set_value_number(entry.duration as f64);

    let commit_cell = worksheet.get_cell_mut((&COLUMN_COMMIT, row_num));
    commit_cell.set_value_string(&entry.id);
    if entry.commit_url.is_some() {
        commit_cell
            .get_hyperlink_mut()
            .set_url(entry.commit_url.clone().unwrap());
        let style = commit_cell.get_style_mut();

        //mimic hyperlink format
        style.get_font_mut().get_color_mut().set_argb("FF0463d6");
        style.get_font_mut().set_underline("single");
    }

    if entry.reference.is_some() {
        worksheet
            .get_cell_mut((&COLUMN_REFERENCE, row_num))
            .set_value_string(&entry.reference.clone().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use chrono::{NaiveDate, TimeZone};
    use std::path::Path;

    #[test]
    fn merge_existing() {
        //Given
        setup_logger();
        let source = Path::new("test/data1.xlsx");
        let test = Path::new("test/tmp/data-test-unit.xlsx");

        //remove previous data
        if test.exists() {
            fs::remove_file(test).unwrap();
        }

        fs::copy(source, test).expect("Unable to clone test file");
        let journal_inputs = &JournalInputs {
            file: String::from(test.to_string_lossy()),
            repo: "test".into(),
            owner: String::from("test"),
            branch: String::from("branch"),
            pat: None,
            backup: false,
        };

        let entries = &mut vec![
            create_journal_entry(4, "before", None),
            create_journal_entry(6, "inbetween", Some(30)),
            create_journal_entry(5, "1fedite", None), //Same, should be discarded
            create_journal_entry(13, "after1", None),
            create_journal_entry(16, "after2", None),
            create_journal_entry(14, "after3", None), //verify that items are reordered
        ];

        //When
        let total_rows =
            merge_entries_with_spreadsheet(journal_inputs, entries).expect("Error generating file");

        //Then
        assert_eq!(14, total_rows, "Rows count does not match");
    }

    #[test]
    fn from_empty_spreadsheet() {
        //Given
        setup_logger();
        let test = Path::new("test/tmp/first.xlsx");

        //remove previous data
        if test.exists() {
            fs::remove_file(test).unwrap();
        }

        let journal_inputs = &JournalInputs {
            file: String::from(test.to_string_lossy()),
            repo: "test".into(),
            owner: String::from("test"),
            branch: String::from("branch"),
            pat: None,
            backup: false,
        };

        let entries = &mut vec![
            create_journal_entry(13, "13", None),
            create_journal_entry(16, "16", Some(5)),
            create_journal_entry(14, "14", None), //verify that items are reordered
        ];

        //When
        let total_rows =
            merge_entries_with_spreadsheet(journal_inputs, entries).expect("Error generating file");

        //Then
        assert_eq!(
            3 + 1, /*for headers*/
            total_rows,
            "Rows count does not match"
        );
    }

    fn create_journal_entry(day: u32, id: &str, minute: Option<u32>) -> JournalEntry {
        let minute = minute.unwrap_or(0);
        JournalEntry {
            id: id.into(),
            date: Local
                .from_local_datetime(
                    &NaiveDate::from_ymd_opt(2024, 11, day)
                        .unwrap()
                        .and_hms_opt(1, minute, 0)
                        .unwrap(),
                )
                .unwrap(),
            summary: String::from(format!("Summary-{}", id)),
            description: String::from(format!("description-{}", id)),
            duration: day as i32,
            reference: Some(String::from(format!("#4-{}", id))),
            commit_url: Some(String::from(format!("https://gh.com/12345-{}", id))),
        }
    }
}
