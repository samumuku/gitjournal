//Author: JMY
//Date  : 2024
//Place : ETML
mod cli;
mod model;

use std::error::Error;
use std::fs;
use anyhow::{bail, Context, Result};

use clap::Parser;
use cli::JournalInputs;
use log::{debug, error, info, warn, Level, LevelFilter};
use std::path::Path;
use itertools::Itertools;
use octocrab::models;
use octocrab::models::repos::RepoCommit;
use octocrab::repos::RepoHandler;

use calamine::{Reader, open_workbook_auto, Xlsx, DataType};

#[tokio::main]
async fn main() {
    setup_logger();

    match update_journal(JournalInputs::parse())
        .await {
        Ok(_) => {info!("Journal successfully updated")}
        Err(e) => {error!("Failed to update journal : {:#}",e)}
    }
}

async fn update_journal(journal_inputs: JournalInputs) -> Result<()> {
    debug!("Start working on {}",journal_inputs);

    //let remote_commits = retrieve_remote_commits(&JournalInputs);

    let local_entries = retrieve_local_entries(&journal_inputs);

    //wait for excel + remote
    //let remote_commits = remote_commits.await?;

    //merge entries

    //write
    
    Ok(())

}

fn write_entries(entries:Vec<()>) -> Result<()>{
    for (i, entry) in entries.enumerate(){

    }
}

fn retrieve_local_entries(journal_inputs: &JournalInputs) -> Result<Option<Vec<RepoCommit>>> {
    //read local commits on excel file
    if fs::exists(&journal_inputs.file).with_context(||format!("Failed to check existence of {}",&journal_inputs.file))
    {
        let mut workbook = open_workbook_auto(&journal_inputs.file)?;

        let (first_sheet_name, _) = &workbook.worksheets()[0];

        // Read whole worksheet data and provide some statisticsf
        if let Some(Ok(range)) = workbook.worksheet_range(first_sheet_name) {
            for row in range.rows().iter().skip(1) {
                println!("row={:?}, row[0]={:?}", row, row[0]);
            }
            /*
            range.row
            let total_cells = range.get_size().0 * range.get_size().1;
            let non_empty_cells: usize = range.used_cells().count();
            println!("Found {} cells in 'Sheet1', including {} non empty cells",
                     total_cells, non_empty_cells);
            // alternatively, we can manually filter rows
            assert_eq!(non_empty_cells, range.rows()
                .flat_map(|r| r.iter().filter(|&c| c != &DataType::Empty)).count());*/
        }

        return (Ok(Some(vec![])));
    }
    info!("{} does not exist yet, nothing to import",journal_inputs.file);
    Ok(None)
}

async fn retrieve_remote_commits(journal_inputs: &JournalInputs) -> Result<Vec<RepoCommit>>
{
    let mut builder = octocrab::OctocrabBuilder::default();
    if journal_inputs.pat.is_some() {
        builder = builder.personal_token(journal_inputs.pat.clone().unwrap());
    }
    let octocrab = builder.build().with_context(|| { "Failed to build octocrab" })?;
    let repository = octocrab.repos(&journal_inputs.owner, &journal_inputs.repo);

    let branches = repository
        .list_branches().send().await
        .with_context(||format!("Failed to list branches of repo {}, does it exist or is it misspelled ?",&journal_inputs))?;
    if branches.items.iter().filter(|branch| branch.name == journal_inputs.branch).count() == 0
    {
        bail!("Unknown branch `{}` (availables:{})",journal_inputs.branch,
            branches.items.iter().map(|b|&b.name).join(","));
    }

    let first_commits = repository.list_commits().branch(&journal_inputs.branch)
        .send().await.with_context(||format!("Failed to list commits of {}",&journal_inputs))?;

    let commits = octocrab.all_pages::<models::repos::RepoCommit>(first_commits).await?;
    debug!("Found {} remote commits",commits.len());

    Ok(commits)
}


fn setup_logger() {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .with_local_timestamps()
        .with_level(LevelFilter::Info) //others libs
        .with_module_level(
            "gitjournal",
            if cfg!(debug_assertions) {
                LevelFilter::Trace
            } else {
                LevelFilter::Info
            },
        )
        .env()
        .init()
        .unwrap();
}