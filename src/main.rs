use clap::Parser;
use consta::{cli::Args, git, table};
use std::time::Instant;

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
