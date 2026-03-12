use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "consta", about = "Git contribution statistics tool")]
pub struct Args {
    #[arg(
        short,
        long,
        required = true,
        num_args = 1..,
        help = "Filter commits by author name or email (multiple allowed)"
    )]
    pub author: Vec<String>,

    #[arg(long, help = "Show commits after this date (e.g. 2026-01-01)")]
    pub since: Option<String>,

    #[arg(long, help = "Show commits before this date")]
    pub until: Option<String>,

    #[arg(long, help = "Show commit breakdown")]
    pub breakdown: bool,

    #[arg(required = true, help = "Paths to git repositories, directories, or GitHub URLs")]
    pub repos: Vec<String>,

    // To get grid for specific year. If none - current year is shown.
    #[arg(long, help = "Show activity grid for a specific year")]
    pub grid: Option<i32>,

    // Recursively search for git repos, depth can be specified
    #[arg(long, num_args = 0..=1, default_missing_value = "3", help = "Discover repos in subdirectories [default depth: 3]")]
    pub recursive: Option<i32>,

    #[arg(long, help = "Print debug information")]
    pub debug: bool,

    #[arg(long, help = "GitHub personal access token")]
    pub token: Option<String>,
}
