use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use itertools::Itertools;
use log::warn;
use octocrab::models::repos::{CommitAuthor, RepoCommit};
use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

type Summary = String;
type Description = String;
type Duration = i32; //allow negative duration in case of adjustments...
type Id = String;

#[derive(Debug)]
pub struct JournalEntry {
    pub(crate) id: Id, //gh hash or random UUID for import
    pub(crate) date: DateTime<Local>,
    pub(crate) summary: Summary,
    pub(crate) description: Description,
    pub(crate) duration: Duration,        //in minutes
    pub(crate) reference: Option<String>, //#4 for https://issues.jira.com/#4 for instance
    pub(crate) commit_url: Option<String>,
}

static REGEX_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\r?\n+").unwrap());

static REGEX_TIME_STATUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\[?(((?<hours>\d+)h]?)|((?<minutes>\d+)m))]?)+(-\[?(?<status>DONE|WIP)]?)?(-\[?(?<reference>#.+)]?)?(-\[?(?<date>(\d{4}-\d{1,2}-\d{1,2})|(\d{2}\.\d{1,2}\.\d{4}))]?)?")
        .expect("Bad regex")
});

const DEFAULT_STATUS: Status = Status::WIP;

impl JournalEntry {
    pub fn from_repo_commits(commits: Vec<RepoCommit>) -> Vec<JournalEntry> {
        commits
            .iter()
            .filter_map(|repo_commit| JournalEntry::from(repo_commit))
            .collect()
    }
}

enum Status {
    WIP,
    DONE,
}

struct RawJournalEntry {
    time_in_minutes: Duration,
    status: Status,
    reference: Option<String>,
    date: Option<DateTime<Local>>,
}

impl JournalEntry {
    fn from(repo_commit: &RepoCommit) -> Option<Self> {
        let message = &repo_commit.commit.message;
        let lines = REGEX_NEWLINE.split(&message).collect::<Vec<&str>>();

        let summary = &lines[0];

        let mut raw_journal_entry: Option<RawJournalEntry> = None;

        //Extract AND filter out special line with infos
        let description = lines[1..]
            .iter()
            .filter(|line| {
                let captures = REGEX_TIME_STATUS.captures(line);
                if captures.is_some() {
                    let captures = captures.unwrap();
                    let hours: Option<i32> = match captures.name("hours") {
                        Some(_match) => {
                            Some(i32::from_str(_match.as_str()).expect("invalid hours"))
                        }
                        None => None,
                    };
                    let minutes: Option<i32> = match captures.name("minutes") {
                        Some(_match) => {
                            Some(i32::from_str(_match.as_str()).expect("invalid minutes"))
                        }
                        None => None,
                    };

                    let time_in_minutes =
                        hours.unwrap_or_default() * 60 + minutes.unwrap_or_default();

                    let status: Status = match captures.name("status") {
                        Some(_match) => {
                            if _match.as_str().to_lowercase() == "wip" {
                                Status::WIP
                            } else {
                                Status::DONE
                            }
                        }
                        None => DEFAULT_STATUS,
                    };
                    let reference: Option<String> = match captures.name("reference") {
                        Some(_match) => Some(String::from(_match.as_str())),
                        None => None,
                    };
                    let date: Option<DateTime<Local>> = match captures.name("date") {
                        Some(_match) => {
                            let date_string = _match.as_str();
                            let (year, month, day) = if date_string.contains(".") {
                                let parts = date_string.split(".").collect::<Vec<&str>>();
                                (parts[2], parts[1], parts[0])
                            } else {
                                let parts = date_string.split("-").collect_vec();
                                (parts[0], parts[1], parts[2])
                            };

                            Some(DateTime::from_naive_utc_and_offset(
                                NaiveDateTime::from(
                                    NaiveDate::from_ymd_opt(
                                        year.parse::<i32>().unwrap(),
                                        u32::from_str(month).unwrap(),
                                        day.parse::<u32>().unwrap(),
                                    )
                                    .unwrap(),
                                ),
                                *Local::now().offset(),
                            ))
                        }
                        None => None, //Date from repo_commit will set afterwards
                    };

                    //Should be None
                    if raw_journal_entry.is_some() {
                        warn!(
                            "Multiple journal data in commit {}, taking only the first !",
                            repo_commit.sha
                        )
                    }

                    raw_journal_entry = Some(RawJournalEntry {
                        status,
                        reference,
                        date,
                        time_in_minutes,
                    });

                    return false; //filter out time_status_ref lines
                }
                true //default keep all other lines
            })
            .join("\n");

        //It’s a "normal" commit without data to handle
        if raw_journal_entry.is_none() {
            return None;
        }

        let raw_journal_entry = raw_journal_entry.unwrap();

        let description = match raw_journal_entry.status {
            Status::WIP => format!("{} [en cours]", description),
            Status::DONE => format!("{} [terminé]", description),
        };

        let date = repo_commit
            .commit
            .author
            .iter()
            .by_ref()
            .next()
            .unwrap_or(&CommitAuthor {
                email: String::new(),
                name: String::new(),
                date: None,
            })
            .date
            .unwrap();

        Some(Self {
            id: String::from(&repo_commit.sha),
            date: raw_journal_entry.date.unwrap_or_else(|| {
                DateTime::from_naive_utc_and_offset(date.naive_utc(), *date.fixed_offset().offset())
            }),
            summary: String::from(*summary),
            description,
            duration: raw_journal_entry.time_in_minutes,
            reference: raw_journal_entry.reference,
            commit_url: Some(repo_commit.html_url.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use regex::Captures;
    use serde_json::json;
    use url::Url;

    #[test]
    fn convert() {
        setup_logger();
        //Given
        let sha = "abcd";
        let url = Url::parse("https://github.com/").unwrap();

        // Example JSON string representing a RepoCommit
        let json_data = json!(
        {
            "sha": sha,
            "node_id": "MDY6Q29tbWl0MTIzNDU2",
            "commit": {
                "author": {
                    "name": "John Doe",
                    "email": "johndoe@example.com",
                    "date": "2024-11-22T10:00:00Z"
                },
                "committer": {
                    "name": "Jane Doe",
                    "email": "janedoe@example.com",
                    "date": "2024-11-22T10:30:00Z"
                },
                "message": "Initial commit\n5h10m-DONE-#4\nnice to have\nthis feature",
                "tree": {
                    "sha": sha,
                    "url": url
                },
                "url": url,
                "comment_count": 0,
                "verification": null
            },
            "url": url,
            "html_url": url,
            "comments_url": url,
            "author": null,
            "committer": null,
            "parents": []
        });

        // Deserialize JSON into RepoCommit
        let repo_commit: RepoCommit = serde_json::from_value(json_data).unwrap();

        //When
        let converted = JournalEntry::from(&repo_commit).unwrap();

        //Then
        assert_eq!(converted.id, sha);
    }

    #[test]
    fn special_infos_line_regex() {
        setup_logger();
        let captures = REGEX_TIME_STATUS
            .captures("5h10m-WIP-#1")
            .expect("cannot get captures");

        assert_eq!(capture_string(&captures, "hours"), "5");
        assert_eq!(capture_string(&captures, "minutes"), "10");
        assert_eq!(captures.name("seconds"), None);
        assert_eq!(capture_string(&captures, "status"), "WIP");
        assert_eq!(capture_string(&captures, "reference"), "#1");

        let captures = REGEX_TIME_STATUS
            .captures("55m-DONE")
            .expect("cannot get captures");

        assert_eq!(captures.name("hours"), None);
        assert_eq!(capture_string(&captures, "minutes"), "55");
        assert_eq!(captures.name("seconds"), None);
        assert_eq!(capture_string(&captures, "status"), "DONE");
        assert_eq!(captures.name("reference"), None);

        let captures = REGEX_TIME_STATUS
            .captures("55m-25.09.2014")
            .expect("cannot get captures");

        assert_eq!(capture_string(&captures, "minutes"), "55");
        assert_eq!(capture_string(&captures, "date"), "25.09.2014");

        let captures = REGEX_TIME_STATUS.captures("nice to have");
        assert!(captures.is_none(), "nice to have shouldnt be captured");

        let buggy = "[4h]";
        let captures = REGEX_TIME_STATUS.captures(buggy).expect(format!("cannot get captures for {}",buggy).as_str());
        assert_eq!(capture_string(&captures, "hours"), "4");
    }

    fn capture_string(captures: &Captures, field: &str) -> String {
        String::from(
            captures
                .name(field)
                .iter()
                .map(|m| m.as_str())
                .next()
                .unwrap(),
        )
    }
}
