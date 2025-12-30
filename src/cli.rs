use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "consta")]
pub struct Args {
    #[arg(short, long)]
    pub author: String,

    #[arg(long)]
    pub since: Option<String>,

    #[arg(long)]
    pub until: Option<String>,

    #[arg(long)]
    pub breakdown: bool,

    #[arg(required = true)]
    pub repos: Vec<std::path::PathBuf>,
}
