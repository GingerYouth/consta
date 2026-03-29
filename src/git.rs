use crate::cli::Args;
use crate::model::{Commit, RepoStats};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

#[must_use]
pub fn collect(paths: &[PathBuf], args: &Args) -> Vec<RepoStats> {
    let resolved: Vec<_> = args.recursive.map_or_else(
        || paths.iter().filter(|path| is_valid_repo(path, true)).cloned().collect(),
        |depth| paths.iter().flat_map(|path| discover_repos(path.as_path(), depth)).collect(),
    );

    resolved.par_iter().map(|path| collect_repo(path, args)).collect()
}

fn discover_repos(root: &Path, depth: i32) -> Vec<PathBuf> {
    if root.join(".git").exists() {
        return vec![root.to_path_buf()];
    }

    if depth == 0 || !root.is_dir() {
        return vec![];
    }

    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            result.extend(discover_repos(path.as_path(), depth - 1));
        }
    }

    result
}

fn collect_repo(path: &PathBuf, args: &Args) -> RepoStats {
    let mut numstat_request = Command::new("git");
    numstat_request.arg("log").arg("--numstat").arg("--pretty=format:commit %H|%cI|%s");

    for author in &args.author {
        if !author.trim().is_empty() {
            numstat_request.arg("--author").arg(author);
        }
    }

    if let Some(since) = &args.since
        && !since.trim().is_empty()
    {
        numstat_request.arg("--since").arg(since);
    }
    if let Some(until) = &args.until
        && !until.trim().is_empty()
    {
        numstat_request.arg("--until").arg(until);
    }

    let numstat_output = match numstat_request.current_dir(path).output() {
        Ok(o) if o.status.success() => o,
        _ => {
            return RepoStats {
                path: path.clone(),
                commits_amount: 0,
                added: 0,
                deleted: 0,
                commits: vec![],
            };
        }
    };

    let text = String::from_utf8_lossy(&numstat_output.stdout);
    let mut commits = 0usize;
    let mut added = 0u64;
    let mut deleted = 0u64;
    let mut entries = Vec::new();

    // current commit accumulator
    let mut cur_hash = String::new();
    let mut cur_date = String::new();
    let mut cur_message = String::new();
    let mut cur_added = 0u64;
    let mut cur_deleted = 0u64;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("commit ") {
            if !cur_hash.is_empty() {
                entries.push(Commit {
                    hash: cur_hash.clone(),
                    date: cur_date.clone(),
                    message: cur_message.clone(),
                    added: cur_added,
                    deleted: cur_deleted,
                });
                cur_added = 0;
                cur_deleted = 0;
            }
            if let Some(rest) = line.strip_prefix("commit ") {
                let mut parts = rest.splitn(3, '|');
                cur_hash = parts.next().unwrap_or_default().to_string();
                cur_date = parts.next().unwrap_or_default().to_string();
                cur_message = parts.next().unwrap_or_default().to_string();
            }
            commits += 1;
            continue;
        }
        // numstat lines: "<add>\t<del>\t<path>"
        // handle binary files where add/del can be "-"
        let mut parts = line.split_whitespace();
        if let (Some(a), Some(d)) = (parts.next(), parts.next()) {
            let a_num = a.parse::<u64>().unwrap_or(0);
            let d_num = d.parse::<u64>().unwrap_or(0);
            cur_added += a_num;
            cur_deleted += d_num;
            added += a_num;
            deleted += d_num;
        }
    }

    if !cur_hash.is_empty() {
        entries.push(Commit {
            hash: cur_hash,
            date: cur_date,
            message: cur_message,
            added: cur_added,
            deleted: cur_deleted,
        });
    }

    RepoStats { path: path.clone(), commits_amount: commits, added, deleted, commits: entries }
}

fn is_valid_repo(path: &Path, strict: bool) -> bool {
    if !path.exists() || !path.is_dir() {
        assert!(
            !strict,
            "\x1b[31mRepository path does not exist or is not a directory: {}\x1b[0m",
            path.display()
        );
        return false;
    }

    let is_git_repo = if path.join(".git").exists() {
        true
    } else {
        match Command::new("git")
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .current_dir(path)
            .output()
        {
            Ok(out) => {
                out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true"
            }
            Err(_) => false,
        }
    };

    assert!(
        !strict || is_git_repo,
        "\x1b[31m{} is not a valid Git repository\x1b[0m",
        path.display()
    );
    is_git_repo
}
