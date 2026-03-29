#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use std::collections::HashMap;
use std::fmt::Write;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod setup;
use setup::{generate_git_log_output, generate_repo_tree};

struct BenchResult {
    name: String,
    low: f64,
    median: f64,
    high: f64,
}

fn format_time(nanos: f64) -> String {
    if nanos < 1_000.0 {
        format!("{nanos:.1} ns")
    } else if nanos < 1_000_000.0 {
        format!("{:.1} µs", nanos / 1_000.0)
    } else if nanos < 1_000_000_000.0 {
        format!("{:.1} ms", nanos / 1_000_000.0)
    } else {
        format!("{:.2} s", nanos / 1_000_000_000.0)
    }
}

fn run_bench(name: &str, mut f: impl FnMut()) -> BenchResult {
    // Warmup
    let warmup_end = Instant::now() + Duration::from_millis(500);
    while Instant::now() < warmup_end {
        f();
    }

    // Calibrate: find iteration count for ~50ms batches
    let mut iters = 1_u64;
    loop {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        if start.elapsed() >= Duration::from_millis(50) {
            break;
        }
        iters *= 2;
    }

    // Collect samples over 2 seconds
    let mut samples = Vec::new();
    let measure_end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < measure_end {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        samples.push(start.elapsed().as_nanos() as f64 / iters as f64);
    }

    samples.sort_by(f64::total_cmp);
    let n = samples.len();

    BenchResult {
        name: name.to_string(),
        low: samples[n / 20],
        median: samples[n / 2],
        high: samples[n - 1 - n / 20],
    }
}

fn find_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn baseline_path(name: &str) -> PathBuf {
    PathBuf::from("target").join("bench").join(format!("{name}.txt"))
}

fn load_baseline(name: &str) -> Option<HashMap<String, f64>> {
    let content = std::fs::read_to_string(baseline_path(name)).ok()?;
    let mut map = HashMap::new();
    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some((bench_name, nanos_str)) = line.split_once('\t')
            && let Ok(nanos) = nanos_str.parse::<f64>()
        {
            map.insert(bench_name.to_string(), nanos);
        }
    }
    Some(map)
}

fn save_baseline(name: &str, results: &[BenchResult]) {
    let path = baseline_path(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut content = String::new();
    for r in results {
        let _ = writeln!(content, "{}\t{}", r.name, r.median);
    }
    let _ = std::fs::write(&path, content);
}

fn parse_git_output(output: &str) -> (usize, usize, usize) {
    let mut commits = 0_usize;
    let mut added = 0_usize;
    let mut deleted = 0_usize;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("commit ") {
            commits += 1;
            continue;
        }
        let mut parts = line.split_whitespace();
        if let (Some(a), Some(d)) = (parts.next(), parts.next()) {
            added += a.parse::<usize>().unwrap_or(0);
            deleted += d.parse::<usize>().unwrap_or(0);
        }
    }

    (commits, added, deleted)
}

fn discover_repos_impl(root: &Path, depth: i32) -> Vec<PathBuf> {
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
            result.extend(discover_repos_impl(&path, depth - 1));
        }
    }

    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help") {
        println!("Usage: cargo bench -- [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --save <NAME>      Save results as a named baseline");
        println!("  --compare <NAME>   Compare against a saved baseline");
        println!("  --help             Print this help");
        return;
    }

    let save_name = find_arg(&args, "--save");
    let compare_name = find_arg(&args, "--compare");

    let baseline = compare_name.as_ref().and_then(|name| {
        let data = load_baseline(name);
        if data.is_none() {
            eprintln!("  Warning: baseline \"{name}\" not found, skipping comparison\n");
        }
        data
    });
    let comparing = baseline.is_some();
    let baseline = baseline.unwrap_or_default();

    // --- run benchmarks ---

    let mut results = Vec::new();

    let inputs = [
        "https://github.com/owner/repo",
        "  https://github.com/owner/repo  ",
        "http://github.com/owner/repo",
        "/local/path",
        "https://gitlab.com/owner/repo",
    ];
    results.push(run_bench("is_github_url", || {
        for input in &inputs {
            black_box(consta::github::is_github_url(input));
        }
    }));

    let small_log = generate_git_log_output(100, 5);
    results.push(run_bench("parse_git_log [100c/5f]", || {
        black_box(parse_git_output(&small_log));
    }));

    let large_log = generate_git_log_output(1000, 20);
    results.push(run_bench("parse_git_log [1000c/20f]", || {
        black_box(parse_git_output(&large_log));
    }));

    let small_tree = generate_repo_tree(10, 2);
    let small_root = small_tree.path().to_path_buf();
    results.push(run_bench("discover_repos [10 repos, d=2]", || {
        black_box(discover_repos_impl(&small_root, 2));
    }));

    let large_tree = generate_repo_tree(50, 3);
    let large_root = large_tree.path().to_path_buf();
    results.push(run_bench("discover_repos [50 repos, d=3]", || {
        black_box(discover_repos_impl(&large_root, 3));
    }));

    // --- print table ---

    let w = if comparing { 79 } else { 68 };

    println!();
    println!("  consta benchmarks");
    println!("  {}", "─".repeat(w));
    if comparing {
        println!(
            "  {:<35} {:>10} {:>10} {:>10}  {:>8}",
            "benchmark", "low", "median", "high", "change"
        );
    } else {
        println!("  {:<35} {:>10} {:>10} {:>10}", "benchmark", "low", "median", "high");
    }
    println!("  {}", "─".repeat(w));

    for r in &results {
        let bench_name = &r.name;
        let low = format_time(r.low);
        let mid = format_time(r.median);
        let high = format_time(r.high);

        if comparing {
            let change = baseline.get(&r.name).map_or_else(
                || "new".to_string(),
                |&prev| {
                    let pct = (r.median - prev) / prev * 100.0;
                    format!("{pct:+.1}%")
                },
            );
            println!("  {bench_name:<35} {low:>10} {mid:>10} {high:>10}  {change:>8}");
        } else {
            println!("  {bench_name:<35} {low:>10} {mid:>10} {high:>10}");
        }
    }

    println!("  {}", "─".repeat(w));
    println!();

    if let Some(name) = &save_name {
        save_baseline(name, &results);
        eprintln!("  Saved baseline as \"{name}\"\n");
    }
}
