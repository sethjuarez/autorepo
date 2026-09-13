use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn live_repo_is_allowed(repo: &str) -> bool {
    repo.strip_prefix("sethjuarez/")
        .map(|name| name.starts_with("fake-repo-") || name.starts_with("autorepo-test-"))
        .unwrap_or(false)
}

#[test]
#[ignore = "live GitHub tests are opt-in and must never run in normal cargo test"]
fn live_github_prepare_generic_starter_is_idempotent() {
    assert_eq!(std::env::var("AUTOREPO_LIVE_GITHUB").as_deref(), Ok("1"));

    let generated = std::env::var("AUTOREPO_LIVE_CREATE").as_deref() == Ok("1");
    let repo = if generated {
        format!("sethjuarez/autorepo-test-{}", unique_suffix())
    } else {
        std::env::var("AUTOREPO_LIVE_REPO").expect(
            "AUTOREPO_LIVE_REPO must be set to sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*",
        )
    };
    assert!(
        live_repo_is_allowed(&repo),
        "live GitHub tests may target only sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*"
    );

    if generated {
        run("gh", &["repo", "create", &repo, "--public"]);
    }

    run_bin(&["prepare", &repo, "--pack", "builtin", "--dry-run"]);
    run_bin(&["prepare", &repo, "--pack", "builtin", "--yes"]);
    run_bin(&[
        "prepare",
        &repo,
        "--pack",
        "builtin",
        "--yes",
        "--allow-non-empty",
    ]);

    assert_eq!(
        api_count(&repo, "labels?per_page=100", "\"name\":\"demo\""),
        1
    );
    assert_eq!(
        api_count(
            &repo,
            "labels?per_page=100",
            "\"name\":\"good first issue\""
        ),
        1
    );
    assert_eq!(
        api_count(
            &repo,
            "labels?per_page=100",
            "\"name\":\"good_first_issue\""
        ),
        0
    );
    assert_eq!(
        api_count(
            &repo,
            "milestones?state=all&per_page=100",
            "\"title\":\"Demo ready\""
        ),
        1
    );
    assert_eq!(
        api_count(
            &repo,
            "issues?state=all&per_page=100",
            "issue.improve_readme"
        ),
        1
    );
    assert_eq!(
        api_count(
            &repo,
            "issues?state=all&per_page=100",
            "issue.add_smoke_test"
        ),
        1
    );
    assert_eq!(
        api_count(
            &repo,
            "pulls?state=all&per_page=100",
            "pull_request.roadmap"
        ),
        1
    );
    assert!(api_raw(&repo, "contents/README.md").contains("file.readme"));
    assert!(
        api_raw(
            &repo,
            "contents/.autorepo/branches/roadmap.md?ref=demo/generic-starter/roadmap"
        )
        .contains("branch.roadmap")
    );

    if generated && let Err(error) = try_run("gh", &["repo", "delete", &repo, "--yes"]) {
        eprintln!(
            "live test succeeded, but cleanup failed for {repo}. Delete it manually or run `gh auth refresh -h github.com -s delete_repo` before the next generated cleanup.\n{error}"
        );
    }
}

#[test]
fn live_repo_guard_accepts_only_safe_names() {
    assert!(live_repo_is_allowed("sethjuarez/fake-repo-demo"));
    assert!(live_repo_is_allowed("sethjuarez/autorepo-test-123"));
    assert!(!live_repo_is_allowed("sethjuarez/autorepo"));
    assert!(!live_repo_is_allowed("other/autorepo-test-123"));
    assert!(!live_repo_is_allowed("sethjuarez/not-a-fake"));
}

fn unique_suffix() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{millis}-{}", std::process::id())
}

fn run_bin(args: &[&str]) {
    let bin = env!("CARGO_BIN_EXE_autorepo");
    run(bin, args);
}

fn api(repo: &str, path: &str) -> String {
    let endpoint = format!("repos/{repo}/{path}");
    output("gh", &["api", &endpoint])
}

fn api_raw(repo: &str, path: &str) -> String {
    let endpoint = format!("repos/{repo}/{path}");
    output(
        "gh",
        &["api", "-H", "Accept: application/vnd.github.raw", &endpoint],
    )
}

fn api_count(repo: &str, path: &str, needle: &str) -> usize {
    api(repo, path).matches(needle).count()
}

fn run(program: &str, args: &[&str]) {
    if let Err(error) = try_run(program, args) {
        panic!("{error}");
    }
}

fn output(program: &str, args: &[&str]) -> String {
    let output = Command::new(program).args(args).output().unwrap();
    if !output.status.success() {
        panic!(
            "{} {} failed\nstdout:\n{}\nstderr:\n{}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).unwrap()
}

fn try_run(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program).args(args).output().unwrap();
    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "{} {} failed\nstdout:\n{}\nstderr:\n{}",
        program,
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}
