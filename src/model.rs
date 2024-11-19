use chrono::{DateTime, Local};

#[derive(Debug)]
pub struct JournalEntry{
    date : DateTime<Local>;
}