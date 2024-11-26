use crate::cli::JournalInputs;
use anyhow::{bail, Context};
use itertools::Itertools;
use log::info;
use octocrab::models;
use octocrab::models::repos::RepoCommit;

pub(crate) async fn retrieve_remote_commits(
    journal_inputs: &JournalInputs,
) -> anyhow::Result<Vec<RepoCommit>> {
    let mut builder = octocrab::OctocrabBuilder::default();
    if journal_inputs.pat.is_some() {
        builder = builder.personal_token(journal_inputs.pat.clone().unwrap());
    }
    let octocrab = builder
        .build()
        .with_context(|| "Failed to build octocrab")?;
    let repository = octocrab.repos(&journal_inputs.owner, &journal_inputs.repo);

    let branches = repository.list_branches().send().await.with_context(|| {
        format!(
            "Failed to list branches of repo {}, does it exist or is it misspelled ?",
            &journal_inputs
        )
    })?;
    if branches
        .items
        .iter()
        .filter(|branch| branch.name == journal_inputs.branch)
        .count()
        == 0
    {
        bail!(
            "Unknown branch `{}` (availables:{})",
            journal_inputs.branch,
            branches.items.iter().map(|b| &b.name).join(",")
        );
    }

    let first_commits = repository
        .list_commits()
        .branch(&journal_inputs.branch)
        .send()
        .await
        .with_context(|| format!("Failed to list commits of {}", &journal_inputs))?;

    let commits = octocrab
        .all_pages::<models::repos::RepoCommit>(first_commits)
        .await?;

    info!("Found {} remote commits", commits.len());

    Ok(commits)
}
