use std::time::Instant;
use clap::Parser;
use consta::{cli::Args, git, table};

fn main() {
    let start = Instant::now();
    let args = Args::parse();
    let stats = git::collect(&args);
    table::print_summary(&stats, args.breakdown);
    table::print_grid(&stats, args.grid);
    if args.debug {
        eprintln!("Finished in {:.2?}", start.elapsed());
    }
}
