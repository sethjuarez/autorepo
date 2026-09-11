# autorepo

`autorepo` prepares GitHub repositories for demos, workshops, and agent workflows.

Pick an explicit pack and point it at a GitHub repository. The tool validates the pack, produces a plan, and applies safe repository setup in a boring serial order.

V1 is intentionally small. It starts with packs because a demo should be repeatable. It will not try to be Terraform for GitHub.

## What works now

The write executor applies supported safe writes serially. Dry runs and validation show the exact shape before anything changes.

- `autorepo doctor OWNER/REPO` checks target syntax and local auth hints.
- `autorepo validate <PACK_SOURCE>` loads and validates a strict `pack.yml` or `pack.yaml`.
- `autorepo prepare OWNER/REPO --pack <PACK_SOURCE> --dry-run` renders a deterministic plan.
- `autorepo prepare OWNER/REPO --pack <PACK_SOURCE> --yes` validates the plan and applies supported safe writes.
- `autorepo warm OWNER/REPO --pack <PACK_SOURCE>` renders Copilot app session links and warmup guidance.
- `autorepo warm OWNER/REPO --pack <PACK_SOURCE> --only <ID> --open-app` opens one intentional Copilot app warmup target.
- `autorepo warm OWNER/REPO --pack <PACK_SOURCE> --only <ID>` limits warmup output and launch actions to specific pack warmup items.
- `autorepo warm OWNER/REPO --pack <PACK_SOURCE> --start-agent-tasks` includes optional cloud-agent task guidance.

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

Validate a pack from a local folder:

```powershell
autorepo validate .\packs\generic-starter
```

Validate a pack from a GitHub repository folder:

```powershell
autorepo validate "github:sethjuarez/autorepo//packs/generic-starter?ref=main"
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

A pack is a folder with a `pack.yml` or `pack.yaml` manifest and templates.

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

Pack sources can be:

| Source | Example |
| --- | --- |
| Built-in pack | `builtin` or `generic-starter` |
| Local folder | `.\packs\generic-starter` |
| GitHub repository root | `github:OWNER/REPO?ref=main` |
| GitHub repository folder | `github:OWNER/REPO//path/to/pack?ref=main` |
| GitHub tree URL | `https://github.com/OWNER/REPO/tree/main/path/to/pack` |

The `?ref=` value is optional for `github:` sources. If omitted, `autorepo` uses the repository default branch. Remote pack sources are cloned into a temporary folder for the command and then removed.

The manifest is strict. Unknown fields fail validation, paths must stay inside the pack, and resource IDs must be stable. Labels used by issues and pull requests have to be declared. References have to resolve. `safety.max_writes` caps the write plan.

Warmup items are intentionally separate from deterministic writes. Supported warmup kinds are:

| Kind | Required fields | Optional fields | Notes |
| --- | --- | --- | --- |
| `note` | `id`, `title`, `kind` | `body`, `start_agent_task` | Renders guidance only. |
| `checklist` | `id`, `title`, `kind` | `body`, `start_agent_task` | Renders guidance only. |
| `app_link` | `id`, `title`, `kind`, `target` | none | `target` is `home`, `my_work`, or `repo`. |
| `app_session` | `id`, `title`, `kind`, `prompt` or `prompt_template` | `mode` | `mode` is `plan`, `interactive`, or `autopilot`; default is `plan`. |
| `automation_draft` | `id`, `title`, `kind`, `trigger`, `prompt` or `prompt_template` | `time`, `day` | Opens a draft that still requires app confirmation. `trigger` is `manual`, `hourly`, `daily`, or `weekly`; daily/weekly require `time` as `HH:MM`, and weekly also requires `day`. |

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

`autorepo warm` renders warmup notes, checklists, Copilot app links, Copilot app session links, and automation draft links from the pack. App session links use the public `ghapp://session/new` route with the target repo, mode, and kickoff prompt encoded in the URL. Automation draft links use `ghapp://automations/new` and still require user confirmation in the app. Passing `--open-app` requires exactly one `--only <ID>` that points to an app link, app session, or automation draft, so the CLI opens one intentional warmup target instead of a noisy set of app surfaces.

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
