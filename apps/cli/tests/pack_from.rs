use std::{fs, process::Command};

use tempfile::TempDir;

#[test]
fn pack_from_cli_creates_valid_pack_from_local_checkout() {
    let source = TempDir::new().unwrap();
    write(source.path(), "README.md", "hello");
    write(source.path(), "nested/README.md", "nested");
    write(source.path(), ".env", "SECRET=1");
    write(source.path(), ".env.example", "KEY=");
    write(source.path(), "node_modules/pkg/index.js", "ignored");

    let out = TempDir::new().unwrap().path().join("pack");
    run_bin(&[
        "pack-from",
        source.path().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--id",
        "cli-pack",
        "--name",
        "CLI pack",
        "--include",
        "README.md",
        "--include",
        ".env.example",
        "--include",
        ".env",
        "--include",
        "node_modules/**",
        "--with-issues",
        "--with-warmup",
    ]);

    assert!(out.join("templates/files/README.md").is_file());
    assert!(out.join("templates/files/.env.example").is_file());
    assert!(!out.join("templates/files/.env").exists());
    assert!(!out.join("templates/files/nested/README.md").exists());
    assert!(
        !out.join("templates/files/node_modules/pkg/index.js")
            .exists()
    );

    let manifest = fs::read_to_string(out.join("pack.yml")).unwrap();
    assert!(manifest.contains("id: cli-pack"));
    assert!(manifest.contains("path: README.md"));
    assert!(manifest.contains("path: .env.example"));
    assert!(manifest.contains("review_extracted_pack"));
    assert!(manifest.contains("review_pack"));
    run_bin(&["validate", out.to_str().unwrap()]);
}

#[test]
fn pack_from_remote_ref_option_is_exposed_in_help() {
    let output = output_bin(&["pack-from", "--help"]);
    assert!(output.contains("--ref <GIT_REF>"));
}

#[test]
fn pack_from_ref_rejects_local_sources() {
    let source = TempDir::new().unwrap();
    write(source.path(), "README.md", "hello");
    let out = source.path().join("pack");

    let output = Command::new(env!("CARGO_BIN_EXE_autorepo"))
        .args([
            "pack-from",
            source.path().to_str().unwrap(),
            "--ref",
            "main",
            "--out",
            out.to_str().unwrap(),
            "--id",
            "local-ref-pack",
            "--name",
            "Local ref pack",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--ref is only supported for remote sources"));
}

#[test]
fn pack_from_compat_alias_supports_dry_run_without_writing() {
    let source = TempDir::new().unwrap();
    write(source.path(), "README.md", "hello");
    let out = source.path().join("pack");

    let output = output_bin(&[
        "pack",
        "from-repo",
        source.path().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--id",
        "compat-pack",
        "--name",
        "Compat pack",
        "--include",
        "README.md",
        "--dry-run",
    ]);

    assert!(output.contains("Dry run: would write pack"));
    assert!(output.contains("Files selected: 1"));
    assert!(!out.exists());
}

fn write(root: &std::path::Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run_bin(args: &[&str]) {
    output_bin(args);
}

fn output_bin(args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap()
}
