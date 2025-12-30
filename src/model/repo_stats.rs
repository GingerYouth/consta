use super::commit::Commit;
use std::path::PathBuf;

#[derive(Debug)]
pub struct RepoStats {
    pub path: PathBuf,
    pub commits_amount: usize,
    pub added: usize,
    pub deleted: usize,
    pub commits: Vec<Commit>,
}
