//Author: JMY
//Date  : 2024
//Place : ETML
mod cli;
mod github;
mod model;
mod spreadsheet;

use anyhow::Result;

use crate::github::retrieve_remote_commits;
use crate::model::JournalEntry;
use crate::spreadsheet::merge_entries_with_spreadsheet;
use clap::Parser;
use cli::JournalInputs;
use log::{error, info, trace, warn, LevelFilter};

#[tokio::main] //octocrab needs async context
async fn main() {
    setup_logger();
    let mut journal_inputs = JournalInputs::parse();
    if journal_inputs.pat.is_none() {
        let pat_from_env = std::env::var("GITHUB_PAT");
        if pat_from_env.is_ok() {
            trace!("Using PAT from env");
            journal_inputs.pat = Some(pat_from_env.unwrap());
        }
    }

    match update_journal(journal_inputs).await {
        Ok(_) => {
            info!("Journal successfully updated")
        }
        Err(e) => {
            error!("Failed to update journal : {:#}", e)
        }
    }
}

async fn update_journal(journal_inputs: JournalInputs) -> Result<()> {
    info!("Start working on {}", journal_inputs);

    let remote_commits = retrieve_remote_commits(&journal_inputs).await?;

    merge_entries_with_spreadsheet(
        &journal_inputs,
        &mut JournalEntry::from_repo_commits(remote_commits),
    )?;

    Ok(())
}

fn setup_logger() {
    match simple_logger::SimpleLogger::new()
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
    {
        //Mostly when cargo test is ran...
        Err(e) => {
            warn!("Cannot create logging: {}", e);
        }
        _ => {}
    };
}
