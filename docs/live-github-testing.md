# Live GitHub fake-repo testing

Normal `cargo test`, pre-commit hooks, and pull-request quality gates must never touch GitHub. Live GitHub testing is opt-in only and is reserved for proving real repository behavior after write execution exists.

## Allowed targets

Live tests may target only repositories owned by `sethjuarez` whose names match one of these prefixes:

- `fake-repo-*`
- `autorepo-test-*`

The initial guard test enforces this naming rule through `AUTOREPO_LIVE_REPO`.

## Running the guard locally

```powershell
$env:AUTOREPO_LIVE_GITHUB = "1"
$env:AUTOREPO_LIVE_REPO = "sethjuarez/autorepo-test-example"
cargo test --test live_github -- --ignored
```

The current live test is a guard/stub only. It validates opt-in environment variables and the target repository name, then exits without making GitHub API calls.

## Future live test contract

When real GitHub write behavior lands, live tests should verify:

1. `prepare --dry-run` renders the expected deterministic plan.
2. `prepare --yes` creates only marked resources.
3. Re-running `prepare --yes` creates no duplicates.
4. Cleanup deletes only generated `sethjuarez/autorepo-test-*` repositories created by the test run.

Failures should leave repositories intact for inspection. Live tests must never clean up user-managed `fake-repo-*` repositories automatically.

## GitHub Actions

The `Live fake-repo tests` workflow is manual-only. It requires a repository input and runs only the ignored guard test today. It does not perform live GitHub writes in V1.
