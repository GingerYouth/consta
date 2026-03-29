use super::commit::Commit;
use std::path::PathBuf;

#[derive(Debug)]
pub struct RepoStats {
    pub path: PathBuf,
    pub commits_amount: usize,
    pub added: u64,
    pub deleted: u64,
    pub commits: Vec<Commit>,
}
