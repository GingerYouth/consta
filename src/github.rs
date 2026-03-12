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
/// Fetches the commit list (paginated, per author), then for each commit
/// fetches the single-commit detail to get additions/deletions.
/// This avoids the `/stats/contributors` endpoint which misses authors
/// without linked GitHub profiles.
pub fn collect_repo(repo: &GitHubRepo, args: &Args) -> Result<RepoStats, String> {
    let token = resolve_token(args)?;
    let agent = build_agent();

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

    let mut total_added = 0usize;
    let mut total_deleted = 0usize;

    let t = std::time::Instant::now();
    // Fetch per-commit stats via the single-commit endpoint.
    let enriched: Vec<Commit> = commits
        .into_iter()
        .map(|c| {
            let t_commit = std::time::Instant::now();
            let (a, d) = fetch_commit_stats(&agent, &token, repo, &c.hash).unwrap_or((0, 0));
            if args.debug {
                eprintln!(
                    "  [github] {} commit {}: {:.2?}",
                    repo.full_name(),
                    &c.hash[..7],
                    t_commit.elapsed()
                );
            }
            total_added += a;
            total_deleted += d;
            Commit { added: a as u64, deleted: d as u64, ..c }
        })
        .collect();
    if args.debug {
        eprintln!(
            "  [github] {} all commit stats: {:.2?}",
            repo.full_name(),
            t.elapsed()
        );
    }

    Ok(RepoStats {
        path: repo.as_display_path(),
        commits_amount: enriched.len(),
        added: total_added,
        deleted: total_deleted,
        commits: enriched,
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

/// Fetch stats (additions, deletions) for a single commit via
/// `GET /repos/{owner}/{repo}/commits/{sha}`.
fn fetch_commit_stats(
    agent: &ureq::Agent,
    token: &str,
    repo: &GitHubRepo,
    sha: &str,
) -> Result<(usize, usize), String> {
    let url = format!("{GITHUB_API}/repos/{}/{}/commits/{sha}", repo.owner, repo.name);
    let resp = authed_get(agent, &url, token)?;

    if resp.status() != 200 {
        return Ok((0, 0));
    }

    let body: serde_json::Value =
        resp.into_json().map_err(|e| format!("Failed to parse commit detail: {e}"))?;

    let added = body["stats"]["additions"].as_u64().unwrap_or(0) as usize;
    let deleted = body["stats"]["deletions"].as_u64().unwrap_or(0) as usize;

    Ok((added, deleted))
}

/// Fetch paginated commit list from `/repos/{owner}/{repo}/commits`.
/// Returns lightweight `Commit` entries (hash, date, message) with zero stats
/// — sufficient for the grid and commit count.
fn fetch_commit_list(
    agent: &ureq::Agent,
    token: &str,
    repo: &GitHubRepo,
    args: &Args,
) -> Result<Vec<Commit>, String> {
    let base_url = format!("{GITHUB_API}/repos/{}/{}/commits", repo.owner, repo.name);

    let mut all_commits = Vec::new();
    let mut seen_shas = std::collections::HashSet::new();

    // GitHub /commits API accepts only one author, so query each and deduplicate.
    let authors: Vec<&str> = args.author.iter().map(String::as_str).collect();

    for author in &authors {
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

            let resp = authed_get(agent, &url, &token)?;
            let status = resp.status();

            if status == 409 {
                // Empty repository
                break;
            }
            if status != 200 {
                return Err(format!("GitHub commits API returned HTTP {status} for {}", repo.full_name()));
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
                let date = item["commit"]["committer"]["date"].as_str().unwrap_or("").to_string();
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