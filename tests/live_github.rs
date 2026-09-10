fn live_repo_is_allowed(repo: &str) -> bool {
    repo.strip_prefix("sethjuarez/")
        .map(|name| name.starts_with("fake-repo-") || name.starts_with("autorepo-test-"))
        .unwrap_or(false)
}

#[test]
#[ignore = "live GitHub tests are opt-in and must never run in normal cargo test"]
fn live_github_guard_requires_explicit_safe_target() {
    assert_eq!(std::env::var("AUTOREPO_LIVE_GITHUB").as_deref(), Ok("1"));

    let repo = std::env::var("AUTOREPO_LIVE_REPO").expect(
        "AUTOREPO_LIVE_REPO must be set to sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*",
    );
    assert!(
        live_repo_is_allowed(&repo),
        "live GitHub tests may target only sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*"
    );
}

#[test]
fn live_repo_guard_accepts_only_safe_names() {
    assert!(live_repo_is_allowed("sethjuarez/fake-repo-demo"));
    assert!(live_repo_is_allowed("sethjuarez/autorepo-test-123"));
    assert!(!live_repo_is_allowed("sethjuarez/autorepo"));
    assert!(!live_repo_is_allowed("other/autorepo-test-123"));
    assert!(!live_repo_is_allowed("sethjuarez/not-a-fake"));
}
