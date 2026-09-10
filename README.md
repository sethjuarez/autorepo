# autorepo

`autorepo` prepares GitHub repositories for demos, workshops, and agent workflows.

Pick an explicit pack and point it at a GitHub repository. The tool validates the pack, produces a plan, and applies safe repository setup in a boring serial order.

V1 is intentionally small. It starts with packs because a demo should be repeatable. It will not try to be Terraform for GitHub.

## What works now

The write executor applies supported safe writes serially. Dry runs and validation show the exact shape before anything changes.

- `autorepo doctor OWNER/REPO` checks target syntax and local auth hints.
- `autorepo validate <PACK_DIR>` loads and validates a strict `pack.yml`.
- `autorepo prepare OWNER/REPO --pack <PACK_DIR|builtin> --dry-run` renders a deterministic plan.
- `autorepo prepare OWNER/REPO --pack <PACK_DIR|builtin> --yes` validates the plan and applies supported safe writes.
- `autorepo warm OWNER/REPO --pack <PACK_DIR|builtin>` renders Copilot app session links and warmup guidance.
- `autorepo warm OWNER/REPO --pack <PACK_DIR|builtin> --open-app` opens generated Copilot app session links.
- `autorepo warm OWNER/REPO --pack <PACK_DIR|builtin> --only <ID>` limits warmup output and launch actions to specific pack warmup items.
- `autorepo warm OWNER/REPO --pack <PACK_DIR|builtin> --start-agent-tasks` includes optional cloud-agent task guidance.

The included `generic-starter` pack is useful for smoke testing the CLI and for shaping mostly empty demo repositories.

## Install

After the crate is published:

```powershell
cargo install autorepo
```

From a checkout:

```powershell
cargo install --path .
```

## Quick start

Validate the built-in pack:

```powershell
autorepo validate builtin
```

Render a dry-run plan:

```powershell
autorepo prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run
```

Render warmup links and notes:

```powershell
autorepo warm sethjuarez/autorepo-test-dry-run --pack builtin
```

Open a Copilot app warmup session:

```powershell
autorepo warm sethjuarez/autorepo-test-dry-run --pack builtin --only facilitator_session --open-app
```

## Pack model

A pack is a folder with a `pack.yml` manifest and templates.

```text
packs/generic-starter/
  pack.yml
  templates/
    README.md
    copilot-instructions.md
    ci.yml
    issues/
      improve-readme.md
      add-smoke-test.md
    prs/
      roadmap.md
    files/
      docs-roadmap.md
```

The manifest is strict. Unknown fields fail validation, paths must stay inside the pack, and resource IDs must be stable. Labels used by issues and pull requests have to be declared. References have to resolve. `safety.max_writes` caps the write plan.

## Safety model

`autorepo` is built around a small safety contract.

- Explicit packs only.
- Deterministic planning.
- Serial execution.
- No deletes.
- No force pushes.
- No overwriting different user content.
- No drift reconciliation in V1.
- No local mutable state database.

Idempotency is designed around GitHub as the source of truth and embedded markers like this:

```html
<!-- autorepo:pack=generic-starter;id=issue.improve_readme -->
```

Issues and pull requests should be matched by marker, not by title alone. If an unmarked resource conflicts with a planned resource, V1 should report a conflict rather than guess.

## Warmup is separate

Copilot cloud-agent sessions are nondeterministic. They do not belong in the `prepare` plan.

`autorepo warm` renders warmup notes, checklists, Copilot app links, Copilot app session links, and automation draft links from the pack. App session links use the public `ghapp://session/new` route with the target repo, mode, and kickoff prompt encoded in the URL. Automation draft links use `ghapp://automations/new` and still require user confirmation in the app. Passing `--open-app` opens generated links through the GitHub-hosted launcher. Use `--only <ID>` when you want to open a single warmup target instead of the whole pack warmup set.

Cloud-agent tasks are still separate. They are async and nondeterministic, so they stay behind explicit opt-in flags and do not belong in the deterministic `prepare` plan.

## Development

Run the same deterministic gates used by CI:

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run -- validate builtin
cargo run -- prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run
```

Install the optional pre-commit hooks:

```powershell
pipx install pre-commit
pre-commit install
pre-commit run --all-files
```

Live GitHub tests are opt-in only. They are never part of normal tests or pre-commit. Use `scripts\live-github.ps1 -Create` on Windows or `scripts/live-github.sh --create` on macOS and Linux. See `docs/live-github-testing.md`.

## Release and publishing

Release-please manages versions, changelog entries, release PRs, and GitHub Releases.

When release-please creates a release, the same workflow publishes to crates.io with the `CARGO_REGISTRY_TOKEN` GitHub secret and the `crates-io` environment. The standalone `publish.yml` workflow is a manual fallback. See `docs/publishing.md`.

## License

MIT
