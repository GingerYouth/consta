use clap::Parser;
use consta::{cli::Args, git, github, table};
use rayon::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let start = Instant::now();
    let args = Args::parse();

    let mut local_paths = Vec::new();
    let mut github_repos = Vec::new();

    for input in &args.repos {
        if github::is_github_url(input) {
            if let Some(repo) = github::GitHubRepo::parse(input) {
                github_repos.push(repo);
            } else {
                eprintln!("\x1b[31mCould not parse GitHub URL: {input}\x1b[0m");
                std::process::exit(1);
            }
        } else {
            local_paths.push(PathBuf::from(input));
        }
    }

    // Breakdown is not supported for remote repos (no per-commit stats).
    if args.breakdown && !github_repos.is_empty() {
        eprintln!(
            "\x1b[31m--breakdown is not supported for remote GitHub repositories.\n\
             Remove the --breakdown flag, or use local clones instead.\x1b[0m"
        );
        std::process::exit(1);
    }

    // Collect local stats
    let t = Instant::now();
    let mut stats = git::collect(&local_paths, &args);
    if args.debug && !local_paths.is_empty() {
        eprintln!("[local] collected {} repos in {:.2?}", stats.len(), t.elapsed());
    }

    // Collect GitHub stats
    let t = Instant::now();
    let github_results: Vec<_> =
        github_repos.par_iter().map(|repo| (repo, github::collect_repo(repo, &args))).collect();

    for (repo, result) in github_results {
        match result {
            Ok(s) => stats.push(s),
            Err(e) => {
                eprintln!("\x1b[33m[warn] {}: {e}\x1b[0m", repo.full_name());
            }
        }
    }
    if args.debug && !github_repos.is_empty() {
        eprintln!("[github] collected {} repos in {:.2?}", github_repos.len(), t.elapsed());
    }

    table::print_summary(&stats, args.breakdown);
    table::print_grid(&stats, args.grid);

    if args.debug {
        eprintln!("Finished in {:.2?}", start.elapsed());
    }
}
