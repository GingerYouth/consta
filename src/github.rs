use crate::cli::Args;
use crate::model::{Commit, RepoStats};
use std::path::PathBuf;

const GITHUB_API: &str = "https://api.github.com";
const PER_PAGE: usize = 100;

/// Parsed GitHub repository reference.
#[derive(Debug, Clone)]
pub struct GitHubRepo {
    pub owner: String,
    pub name: String,
}

impl GitHubRepo {
    /// Try to parse a GitHub URL into owner/repo.
    ///
    /// Accepts:
    ///  - `https://github.com/owner/repo`
    ///  - `https://github.com/owner/repo.git`
    ///  - `https://github.com/owner/repo/tree/...` (extra segments ignored)
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim().trim_end_matches('/');

        let rest = trimmed
            .strip_prefix("https://github.com/")
            .or_else(|| trimmed.strip_prefix("http://github.com/"))?;

        let mut segments = rest.splitn(3, '/');
        let owner = segments.next().filter(|s| !s.is_empty())?;
        let name = segments.next().filter(|s| !s.is_empty())?;
        let name = name.strip_suffix(".git").unwrap_or(name);

        Some(Self { owner: owner.to_string(), name: name.to_string() })
    }

    /// Returns a display string like `owner/repo`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Returns a synthetic `PathBuf` used to fill `RepoStats.path`
    /// so that the table display shows `owner/repo`.
    pub fn as_display_path(&self) -> PathBuf {
        PathBuf::from(self.full_name())
    }
}

/// Returns `true` if the input looks like a GitHub URL.
pub fn is_github_url(input: &str) -> bool {
    let t = input.trim().to_lowercase();
    t.starts_with("https://github.com/") || t.starts_with("http://github.com/")
}

/// Collect stats for a single GitHub repository via the REST API.
///
/// - `/stats/contributors` → add/delete totals (single request)
/// - `/commits?author=` → commit dates for grid + commit count (paginated)
pub fn collect_repo(repo: &GitHubRepo, args: &Args) -> Result<RepoStats, String> {
    let token = resolve_token(args)?;
    let agent = build_agent();

    let t = std::time::Instant::now();
    let (total_added, total_deleted) =
        fetch_contributor_stats(&agent, &token, repo, &args.author, args.debug)?;
    if args.debug {
        eprintln!(
            "  [github] {} contributor stats: {:.2?}",
            repo.full_name(),
            t.elapsed()
        );
    }

    let t = std::time::Instant::now();
    let commits = fetch_commit_list(&agent, &token, repo, args)?;
    if args.debug {
        eprintln!(
            "  [github] {} commit list ({} commits): {:.2?}",
            repo.full_name(),
            commits.len(),
            t.elapsed()
        );
    }

    Ok(RepoStats {
        path: repo.as_display_path(),
        commits_amount: commits.len(),
        added: total_added,
        deleted: total_deleted,
        commits,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn resolve_token(args: &Args) -> Result<String, String> {
    if let Some(ref t) = args.token {
        if !t.trim().is_empty() {
            return Ok(t.clone());
        }
    }
    std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .map_err(|_| {
            "GitHub token required. Pass --token <TOKEN> or set GITHUB_TOKEN / GH_TOKEN env var."
                .to_string()
        })
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::new()
}

fn authed_get(agent: &ureq::Agent, url: &str, token: &str) -> Result<ureq::Response, String> {
    agent
        .get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "consta-cli")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| format!("GitHub API request failed: {e}"))
}

/// Fetch `/repos/{owner}/{repo}/stats/contributors` and extract totals for the
/// matching author. Returns `(added, deleted)`.
///
/// This endpoint may return 202 (computing) on first call; we retry a few times.
fn fetch_contributor_stats(
    agent: &ureq::Agent,
    token: &str,
    repo: &GitHubRepo,
    authors: &[String],
    debug: bool,
) -> Result<(usize, usize), String> {
    let url = format!("{GITHUB_API}/repos/{}/{}/stats/contributors", repo.owner, repo.name);
    let authors_lower: Vec<String> = authors.iter().map(|a| a.trim().to_lowercase()).collect();

    for attempt in 0..4 {
        let resp = authed_get(agent, &url, token)?;
        let status = resp.status();

        if status == 202 {
            let wait = std::time::Duration::from_secs(2u64.pow(attempt));
            if debug {
                eprintln!(
                    "  [github] {} stats computing... retrying in {}s",
                    repo.full_name(),
                    wait.as_secs()
                );
            }
            std::thread::sleep(wait);
            continue;
        }

        if status == 204 || status == 404 {
            return Err(format!(
                "Repository {} not found or empty (HTTP {status})",
                repo.full_name()
            ));
        }

        let body: serde_json::Value =
            resp.into_json().map_err(|e| format!("Failed to parse contributor stats: {e}"))?;

        let contributors = body.as_array().ok_or("Unexpected contributor stats format")?;

        for contributor in contributors {
            let login = contributor["author"]["login"]
                .as_str()
                .unwrap_or("")
                .to_lowercase();

            let matches = authors_lower
                .iter()
                .any(|a| login.contains(a.as_str()) || a.contains(login.as_str()));

            if matches {
                let empty = vec![];
                let weeks = contributor["weeks"].as_array().unwrap_or(&empty);
                let mut added = 0usize;
                let mut deleted = 0usize;
                for week in weeks {
                    added += week["a"].as_u64().unwrap_or(0) as usize;
                    deleted += week["d"].as_u64().unwrap_or(0) as usize;
                }
                return Ok((added, deleted));
            }
        }

        return Err(format!(
            "No matching author found in contributors of {}",
            repo.full_name()
        ));
    }

    Err(format!("GitHub stats not ready after retries for {}", repo.full_name()))
}

/// Fetch paginated commit list from `/repos/{owner}/{repo}/commits`.
/// Returns lightweight `Commit` entries (hash, date, message) — for grid and commit count.
fn fetch_commit_list(
    agent: &ureq::Agent,
    token: &str,
    repo: &GitHubRepo,
    args: &Args,
) -> Result<Vec<Commit>, String> {
    let base_url = format!("{GITHUB_API}/repos/{}/{}/commits", repo.owner, repo.name);

    let mut all_commits = Vec::new();
    let mut seen_shas = std::collections::HashSet::new();

    for author in &args.author {
        let mut page = 1u32;
        loop {
            let mut url = format!("{base_url}?per_page={PER_PAGE}&page={page}");

            if !author.trim().is_empty() {
                url.push_str(&format!("&author={}", urlencoding(author.trim())));
            }
            if let Some(ref since) = args.since {
                if !since.trim().is_empty() {
                    let iso = to_iso_timestamp(since.trim());
                    url.push_str(&format!("&since={iso}"));
                }
            }
            if let Some(ref until) = args.until {
                if !until.trim().is_empty() {
                    let iso = to_iso_timestamp(until.trim());
                    url.push_str(&format!("&until={iso}"));
                }
            }

            let resp = authed_get(agent, &url, token)?;
            let status = resp.status();

            if status == 409 {
                break;
            }
            if status != 200 {
                return Err(format!(
                    "GitHub commits API returned HTTP {status} for {}",
                    repo.full_name()
                ));
            }

            let body: serde_json::Value =
                resp.into_json().map_err(|e| format!("Failed to parse commits: {e}"))?;

            let items = body.as_array().ok_or("Unexpected commits response format")?;
            if items.is_empty() {
                break;
            }

            for item in items {
                let sha = item["sha"].as_str().unwrap_or("").to_string();
                if seen_shas.contains(&sha) {
                    continue;
                }
                let date = item["commit"]["committer"]["date"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let message = item["commit"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();

                seen_shas.insert(sha.clone());
                all_commits.push(Commit { hash: sha, date, message, added: 0, deleted: 0 });
            }

            if items.len() < PER_PAGE {
                break;
            }
            page += 1;
        }
    }

    Ok(all_commits)
}

/// Minimal percent-encoding for query parameter values.
fn urlencoding(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('@', "%40")
        .replace('&', "%26")
        .replace('+', "%2B")
}

/// If a date string looks like `YYYY-MM-DD`, append `T00:00:00Z` to make it
/// ISO 8601 as required by the GitHub API.
fn to_iso_timestamp(s: &str) -> String {
    if s.contains('T') {
        s.to_string()
    } else {
        format!("{s}T00:00:00Z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_url() {
        let r = GitHubRepo::parse("https://github.com/rust-lang/rust").unwrap();
        assert_eq!(r.owner, "rust-lang");
        assert_eq!(r.name, "rust");
    }

    #[test]
    fn parse_url_with_git_suffix() {
        let r = GitHubRepo::parse("https://github.com/owner/repo.git").unwrap();
        assert_eq!(r.name, "repo");
    }

    #[test]
    fn parse_url_with_trailing_slash() {
        let r = GitHubRepo::parse("https://github.com/owner/repo/").unwrap();
        assert_eq!(r.name, "repo");
    }

    #[test]
    fn parse_url_with_extra_path() {
        let r = GitHubRepo::parse("https://github.com/owner/repo/tree/main/src").unwrap();
        assert_eq!(r.owner, "owner");
        assert_eq!(r.name, "repo");
    }

    #[test]
    fn parse_non_github_returns_none() {
        assert!(GitHubRepo::parse("/some/local/path").is_none());
        assert!(GitHubRepo::parse("https://gitlab.com/owner/repo").is_none());
    }

    #[test]
    fn is_github_url_works() {
        assert!(is_github_url("https://github.com/owner/repo"));
        assert!(is_github_url("  HTTPS://GITHUB.COM/a/b  "));
        assert!(!is_github_url("/local/path"));
        assert!(!is_github_url("https://gitlab.com/a/b"));
    }
}