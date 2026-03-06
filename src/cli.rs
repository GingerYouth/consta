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

    // To get grid for specific year. If none - current year is shown.
    #[arg(long)]
    pub grid: Option<i32>,

    // Recursively search for git repos, depth can be specified
    #[arg(long, num_args = 0..=1, default_missing_value = "3")]
    pub recursive: Option<i32>,

    #[arg(long)]
    pub debug: bool
}
