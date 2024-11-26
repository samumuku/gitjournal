use clap::Parser;
use std::fmt;
use std::fmt::Formatter;

type Owner = String;
type Repo = String;
type Branch = String;
type File = String;
type Pat = String;

#[derive(Parser, Debug)]
#[command(version, about="Generate/Merge an external file with commits containing special time spent formats..", long_about = None
)]
pub struct JournalInputs {
    /// Repository owner
    #[arg()]
    pub(crate) owner: Owner,

    /// Repository name
    #[arg()]
    pub(crate) repo: Repo,

    /// Path to target file (for export and merge)
    #[arg(default_value = "jdt.xlsx")]
    pub file: File,

    /// Branch name
    #[arg(short, long, default_value = "main")]
    pub(crate) branch: Branch,

    /// GitHub PAT to access protected repos
    #[arg(short, long)]
    pub(crate) pat: Option<Pat>,

    /// Backup target file before updating it
    #[arg(long, default_value = "true")]
    pub(crate) backup: bool,
}

impl fmt::Display for JournalInputs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} [branch: {}] [using PAT: {}]",
            self.owner,
            self.repo,
            self.branch,
            self.pat.is_some()
        )
    }
}
