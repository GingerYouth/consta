use crate::cli::Args;
use crate::model::{Commit, RepoStats};
use std::process::Command;

#[must_use]
pub fn collect(args: &Args) -> Vec<RepoStats> {
    args.repos.iter().map(|path| collect_repo(path, args)).collect()
}

fn collect_repo(path: &std::path::PathBuf, args: &Args) -> RepoStats {
    check_validity(path);

    let mut numstat_request = Command::new("git");
    numstat_request.arg("log").arg("--numstat").arg("--pretty=format:commit %H|%cI|%s");

    if !args.author.trim().is_empty() {
        numstat_request.arg("--author").arg(&args.author);
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
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut entries = Vec::new();

    // current commit accumulator
    let mut cur_hash = String::new();
    let mut cur_date = String::new();
    let mut cur_message = String::new();
    let mut cur_added = 0usize;
    let mut cur_deleted = 0usize;

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
                    added: cur_added as u64,
                    deleted: cur_deleted as u64,
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
            let a_num = a.parse::<usize>().unwrap_or(0);
            let d_num = d.parse::<usize>().unwrap_or(0);
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
            added: cur_added as u64,
            deleted: cur_deleted as u64,
        });
    }

    RepoStats { path: path.clone(), commits_amount: commits, added, deleted, commits: entries }
}

fn check_validity(path: &std::path::PathBuf) {
    assert!(
        !(!path.exists() || !path.is_dir()),
        "Repository path does not exist or is not a directory: {:?}",
        path.display()
    );

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

    assert!(is_git_repo, "{} is not a valid Git repository", path.display());
}
