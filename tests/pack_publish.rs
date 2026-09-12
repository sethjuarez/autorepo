use std::{fs, path::Path, process::Command};

use tempfile::TempDir;

#[test]
fn pack_update_refreshes_files_and_preserves_curated_sections() {
    let source = TempDir::new().unwrap();
    write(source.path(), "README.md", "new readme");
    write(source.path(), "docs/guide.md", "new guide");
    write(source.path(), ".env", "SECRET=1");

    let pack = TempDir::new().unwrap();
    write(
        pack.path(),
        "pack.yml",
        r#"schema: 1
id: demo-pack
name: Demo pack
safety:
  max_writes: 4
files:
  - id: old
    path: OLD.md
    template: templates/files/OLD.md
issues:
  - id: review
    title: Review demo
    template: templates/issues/review.md
warmup:
  - id: start
    title: Start demo
    kind: note
    body: Keep this curated note.
"#,
    );
    write(pack.path(), "templates/files/OLD.md", "old");
    write(pack.path(), "templates/issues/review.md", "review");

    run_bin(&[
        "pack",
        "update",
        source.path().to_str().unwrap(),
        "--out",
        pack.path().to_str().unwrap(),
        "--include",
        "README.md",
        "--include",
        "docs/**",
        "--include",
        ".env",
    ]);

    assert!(pack.path().join("templates/files/README.md").is_file());
    assert!(pack.path().join("templates/files/docs/guide.md").is_file());
    assert!(!pack.path().join("templates/files/OLD.md").exists());
    assert!(!pack.path().join("templates/files/.env").exists());

    let manifest = fs::read_to_string(pack.path().join("pack.yml")).unwrap();
    assert!(manifest.contains("id: demo-pack"));
    assert!(manifest.contains("template: templates/files/README.md"));
    assert!(manifest.contains("template: templates/files/docs/guide.md"));
    assert!(!manifest.contains("OLD.md"));
    assert!(manifest.contains("title: Review demo"));
    assert!(manifest.contains("Keep this curated note."));
    assert!(manifest.contains("max_writes: 4"));
    run_bin(&["validate", pack.path().to_str().unwrap()]);
}

#[test]
fn pack_publish_commits_and_pushes_branch_to_target_remote() {
    let source = TempDir::new().unwrap();
    write(source.path(), "README.md", "published readme");

    let remote = TempDir::new().unwrap();
    git(None, &["init", "--bare", remote.path().to_str().unwrap()]);

    let seed = TempDir::new().unwrap();
    git(Some(seed.path()), &["init"]);
    git(
        Some(seed.path()),
        &["config", "user.email", "test@example.com"],
    );
    git(Some(seed.path()), &["config", "user.name", "Test User"]);
    write(seed.path(), "README.md", "catalog");
    git(Some(seed.path()), &["add", "README.md"]);
    git(Some(seed.path()), &["commit", "-m", "seed"]);
    git(
        Some(seed.path()),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git(Some(seed.path()), &["push", "-u", "origin", "master"]);

    let target = TempDir::new().unwrap();
    git(
        None,
        &[
            "clone",
            remote.path().to_str().unwrap(),
            target.path().to_str().unwrap(),
        ],
    );
    git(
        Some(target.path()),
        &["config", "user.email", "test@example.com"],
    );
    git(Some(target.path()), &["config", "user.name", "Test User"]);

    run_bin(&[
        "pack",
        "publish",
        source.path().to_str().unwrap(),
        "--target-repo",
        "example/catalog",
        "--target-checkout",
        target.path().to_str().unwrap(),
        "--target-path",
        "packs/demo",
        "--branch",
        "pack/demo",
        "--id",
        "demo",
        "--name",
        "Demo",
        "--include",
        "README.md",
        "--yes",
    ]);

    git(
        None,
        &[
            "--git-dir",
            remote.path().to_str().unwrap(),
            "show-ref",
            "--verify",
            "refs/heads/pack/demo",
        ],
    );
    run_bin(&[
        "validate",
        target.path().join("packs/demo").to_str().unwrap(),
    ]);
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run_bin(args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_autorepo"))
        .args(args)
        .output()
        .unwrap();
    if !output.status.success() {
        panic!(
            "autorepo {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn git(cwd: Option<&Path>, args: &[&str]) {
    let mut command = Command::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command.args(args).output().unwrap();
    if !output.status.success() {
        panic!(
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
