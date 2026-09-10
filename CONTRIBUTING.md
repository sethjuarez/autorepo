# Contributing

Thanks for helping improve `autorepo`.

## Development

Run the focused Rust checks before opening a pull request:

```powershell
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -- validate builtin
cargo run -- prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run
```

Keep changes small and reviewable. Prefer deterministic planner behavior and narrow GitHub API seams over broad abstractions.

## Pre-commit

This repository includes local pre-commit hooks for deterministic checks only. They run:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo run --quiet -- validate builtin`
- `cargo run --quiet -- prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run`

Install and run them with:

```powershell
pipx install pre-commit
pre-commit install
pre-commit run --all-files
```

Do not add live GitHub tests to pre-commit. Live tests are opt-in only. Use `scripts\live-github.ps1 -Create` on Windows or `scripts/live-github.sh --create` on macOS and Linux. See `docs/live-github-testing.md` for the full guard and cleanup notes.

## Publishing

Release-please manages release PRs and GitHub Releases. It depends on the repository setting that allows GitHub Actions to create and approve pull requests, plus `contents: write` and `pull-requests: write` workflow permissions. Release PRs only change crate metadata and changelog files, so CI ignores those PRs. The push to `main` after a release PR merge still runs CI.

Crates.io publishing normally runs inside `.github/workflows/release-please.yml` after release-please creates a GitHub Release. The manual `.github/workflows/publish.yml` workflow is a fallback for dry-runs or explicit manual publishing. Both real publishing paths use the `crates-io` GitHub Environment and the `CARGO_REGISTRY_TOKEN` repository secret. See `docs/publishing.md` for token-based publishing details and the OIDC trusted-publishing alternative.

## Commit messages

Use Conventional Commits so release-please can produce changelogs and releases:

```text
feat: add pack validation
fix: reject unsafe template paths
docs: document warmup behavior
chore: update release workflow
```

Use `feat` for user-visible capability, `fix` for bug fixes, `docs` for documentation-only changes, and `chore` for maintenance.

## Safety expectations

- Do not add destructive actions without an explicit design discussion.
- Do not introduce local mutable state for V1 idempotency.
- Do not make planner behavior depend on network calls or wall-clock time.
- Keep Copilot cloud-agent warmup separate from deterministic repository preparation.
