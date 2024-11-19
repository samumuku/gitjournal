use std::fmt;
use std::fmt::Formatter;
use clap::Parser;

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

    /// Branch name
    #[arg(short, long, default_value = "main")]
    pub(crate) branch: Branch,

    /// Path to target file (for export and merge)
    #[arg(default_value = "jdt.xlsx")]
    pub file: File,

    /// GitHub PAT to access protected repos
    #[arg(short, long)]
    pub(crate) pat: Option<Pat>,
}

impl fmt::Display for JournalInputs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} [branch: {}] [using PAT: {}]",self.owner,self.repo,self.branch,self.pat.is_some())
    }
}