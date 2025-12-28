use clap::Parser;
use consta::{cli::Args, git, table};

fn main() {
    let args = Args::parse();
    let stats = git::collect(&args);
    table::print_summary(&stats, args.breakdown);
}
