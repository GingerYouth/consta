use std::fmt::Write;
use git2::{Repository, Signature};
use std::path::PathBuf;
use tempfile::TempDir;

#[allow(dead_code)]
pub fn generate_test_repo(commits: usize, files_per_commit: usize) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo = Repository::init(temp_dir.path()).expect("Failed to init repo");

    let sig = Signature::now("Benchmark Author", "bench@example.com")
        .expect("Failed to create signature");

    for i in 0..commits {
        for j in 0..files_per_commit {
            let filename = format!("src/file_{i}_{j}.rs");
            let workdir = repo.workdir().unwrap();
            let file_path = workdir.join(&filename);

            std::fs::create_dir_all(file_path.parent().unwrap()).ok();
            std::fs::write(
                &file_path,
                format!(
                    "// File {} content with some lines\nfn test() {{\n    let x = {};\n}}\n",
                    filename,
                    i * j
                ),
            )
            .ok();

            let mut index = repo.index().expect("Failed to get index");
            index.add_path(std::path::Path::new(&filename)).ok();
            let tree_id = index.write_tree().expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");

            let message = format!("Commit {i} with file {j}");
            repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[]).ok();
        }
    }

    let path = temp_dir.path().to_path_buf();
    (temp_dir, path)
}

pub fn generate_git_log_output(commits: usize, files_per_commit: usize) -> String {
    let mut output = String::new();

    for i in 0..commits {
        let hash = format!("{:040x}", i * 12345);
        let timestamp = format!("2024-01-{:02}T10:30:00+00:00", (i % 28) + 1);
        let message = format!("Commit {i} with {files_per_commit} files");

        let _ = writeln!(output, "commit {hash}|{timestamp}|{message}");

        for j in 0..files_per_commit {
            let added = 10 + (i * j % 100);
            let deleted = 5 + (i * j % 50);
            let filename = format!("src/file_{i}_{j}.rs");
            let _ = writeln!(output, "{added}\t{deleted}\t{filename}");
        }
    }

    output
}

fn create_repo_at(path: &std::path::Path) {
    std::fs::create_dir_all(path).ok();
    let repo = Repository::init(path).expect("Failed to init repo");
    let sig = Signature::now("Test", "test@test.com").expect("Failed to create sig");

    let file_path = path.join("README.md");
    std::fs::write(&file_path, "# Test").ok();

    let mut index = repo.index().expect("Failed to get index");
    index.add_path(std::path::Path::new("README.md")).ok();
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).ok();
}

fn recurse(path: &std::path::Path, remaining: usize, repo_count: &mut usize) {
    if *repo_count == 0 {
        return;
    }

    if remaining == 0 {
        return;
    }

    let dirs = remaining.min(3);
    for i in 0..dirs {
        if *repo_count == 0 {
            return;
        }

        let subdir = path.join(format!("subdir_{i}"));

        if remaining == 1 || *repo_count <= 2 {
            create_repo_at(&subdir);
            *repo_count -= 1;
        } else {
            recurse(&subdir, remaining - 1, repo_count);
        }
    }
}

pub fn generate_repo_tree(repo_count: usize, depth: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path();

    let mut remaining = repo_count;
    recurse(root, depth, &mut remaining);

    temp_dir
}
