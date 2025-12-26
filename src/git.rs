use crate::cli::Args;
use crate::model::RepoStats;
use std::process::Command;

#[must_use]
pub fn collect(args: &Args) -> Vec<RepoStats> {
    args.repos.iter().map(|path| collect_repo(path, args)).collect()
}

fn collect_repo(path: &std::path::PathBuf, args: &Args) -> RepoStats {
    check_validity(path);

    let mut numstat_request = Command::new("git");
    numstat_request.arg("log").arg("--numstat").arg("--pretty=format:commit %H");

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
                commits: 0,
                added: 0,
                deleted: 0,
                entries: vec![],
            };
        }
    };

    let text = String::from_utf8_lossy(&numstat_output.stdout);
    let mut commits = 0usize;
    let mut added = 0usize;
    let mut deleted = 0usize;
    let entries = vec![];

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("commit ") {
            commits += 1;
            continue;
        }
        // numstat lines: "<add>\t<del>\t<path>"
        // handle binary files where add/del can be "-"
        let mut parts = line.split_whitespace();
        if let (Some(a), Some(d)) = (parts.next(), parts.next()) {
            added += a.parse::<usize>().unwrap_or(0);
            deleted += d.parse::<usize>().unwrap_or(0);
        }
    }

    RepoStats { path: path.clone(), commits, added, deleted, entries }
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
