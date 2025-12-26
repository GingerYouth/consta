#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub date: String,
    pub message: String,
    pub added: u64,
    pub deleted: u64,
}
