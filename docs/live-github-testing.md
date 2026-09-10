# Live GitHub fake-repo testing

Normal `cargo test`, pre-commit hooks, and pull-request quality gates must never touch GitHub. Live GitHub testing is opt-in only.

## Allowed targets

Live tests may target only repositories owned by `sethjuarez` whose names match one of these prefixes:

- `fake-repo-*`
- `autorepo-test-*`

The live test enforces this naming rule before it creates or touches a repository.

## Running the guard locally

To create a generated test repository, run the live test wrapper:

```powershell
scripts\live-github.ps1 -Create
```

The test creates a public repository named `sethjuarez/autorepo-test-*`, runs the built-in `generic-starter` pack, reruns it with `--allow-non-empty`, verifies no duplicate labels, milestones, issues, or pull requests were created, and tries to delete the generated repository only after every assertion passes.

To target an existing fake repository instead:

```powershell
scripts\live-github.ps1 -Repo sethjuarez/fake-repo-example
```

On macOS or Linux, use the matching shell wrapper:

```bash
scripts/live-github.sh --create
scripts/live-github.sh --repo sethjuarez/fake-repo-example
```

The wrappers set the `AUTOREPO_LIVE_*` environment variables for the test process. Those variables are still part of the safety guard, but they are not the interface contributors need to remember.

Failures leave repositories intact for inspection. If cleanup fails because the local GitHub token does not have `delete_repo`, delete the generated repo manually or run `gh auth refresh -h github.com -s delete_repo` before the next generated cleanup. Live tests never clean up user-managed `fake-repo-*` repositories automatically.

## GitHub Actions

The `Live fake-repo tests` workflow is manual-only. It requires a repository input and runs the ignored live test against that repository.
