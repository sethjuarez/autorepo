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
- `autorepo pack-from <SOURCE> --out <PACK_DIR> --id <ID> --name <NAME>` scaffolds a strict starter pack from a local checkout or GitHub repository.
- `autorepo labs session ...` experimentally inspects, captures, and rehydrates local Copilot app session snapshots.

The included `generic-starter` pack is useful for smoke testing the CLI and for shaping mostly empty demo repositories.

## Copilot skill

This repository includes a project skill at `.github/skills/autorepo/SKILL.md` with agent-facing command guidance, pack source examples, safety rules, and quality gates for future `autorepo` work.

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

Scaffold a starter pack from a local repository checkout:

```powershell
autorepo pack-from C:\path\to\source-repo `
  --out .\packs\contract-expert `
  --id contract-expert `
  --name "Contract expert" `
  --include ".github/extensions/**" `
  --include "src/contract-policy-expert-agent/**" `
  --include "azure.yaml" `
  --include "data/**" `
  --include "docs/**" `
  --include "README.md" `
  --include "AGENTS.md" `
  --include ".gitignore" `
  --include ".github/copilot-instructions.md" `
  --with-issues `
  --with-warmup
```

Scaffold from a GitHub repository or repository folder:

```powershell
autorepo pack-from caldova/contract-policy-expert --ref main --out .\packs\contract-expert --id contract-expert --name "Contract expert" --include "README.md"
autorepo pack-from "github:caldova/contract-policy-expert//fixtures/source?ref=main" --out .\packs\contract-expert --id contract-expert --name "Contract expert"
autorepo pack-from "https://github.com/caldova/contract-policy-expert/tree/main/fixtures/source" --out .\packs\contract-expert --id contract-expert --name "Contract expert"
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

`autorepo pack-from` creates this same strict pack shape from a local checkout, `OWNER/REPO`, `github:OWNER/REPO//path?ref=<branch-or-tag>`, or a GitHub URL. Use `--ref <branch-or-tag>` with remote sources when the source string does not already include `?ref=` or a `/tree/<ref>/...` URL segment. Remote sources are shallow-cloned into a temporary folder and removed after the command. It copies selected UTF-8 text files to `templates/files/<repo-relative-path>`, writes `files` manifest entries whose `path` values preserve the target repository paths, and validates the generated pack before reporting success. If no `--include` patterns are supplied, it considers every file except default exclusions. Repeated `--include` and `--exclude` patterns use repository-relative globs with `/` separators; `--dry-run` prints the generated manifest and copy/skip summary without writing the output directory.

Pack extraction has always-on hard exclusions for local, private, generated, and secret-bearing paths. It skips `.git`, other VCS metadata, real `.env` files while allowing explicit templates like `.env.example`, private keys/certificates, credentials files, editor state, OS files, logs, local databases, dependency folders such as `node_modules`, caches, build outputs such as `target`, `dist`, `build`, `.next`, `.turbo`, virtual environments, symlinks, unreadable files, and binary or non-UTF-8 files. User includes can narrow the selection and user excludes can remove more files, but V1 does not let includes override hard safety exclusions.

Optional extraction stubs stay modest: `--with-issues` adds a `Review extracted pack` issue, and `--with-warmup` adds an app link plus a review app-session prompt. Labels, milestones, pull requests, workflow dispatches, repository issue migration, automation schedules, binary assets, and session snapshots remain manual in V1.

Refresh an existing pack from a repo after improving a demo checkout:

```powershell
autorepo pack update . `
  --out .\packs\contract-expert `
  --include ".github/extensions/**" `
  --include "src/contract-policy-expert-agent/**" `
  --include "azure.yaml" `
  --include "data/**" `
  --include "docs/**" `
  --include "README.md" `
  --include "AGENTS.md" `
  --include ".gitignore" `
  --include ".github/copilot-instructions.md"
```

`pack update` preserves curated pack metadata by default: `id`, `name`, `description`, labels, milestones, issues, pull requests, workflow dispatches, warmup entries, and non-file templates stay in place. It refreshes `templates/files/**`, replaces the manifest `files` entries, recalculates `safety.max_writes`, applies the same hard exclusions as `pack-from`, and validates the result. Use `--replace` only when you intentionally want to regenerate the whole pack directory; metadata flags such as `--id`, `--name`, and `--description` require `--replace` when updating an existing pack.

Publish a refreshed pack to a catalog repository branch:

```powershell
autorepo pack publish . `
  --target-repo sethjuarez/ghcp-starters `
  --target-path packs/contract-expert `
  --branch pack/contract-expert-refresh `
  --include ".github/extensions/**" `
  --include "src/contract-policy-expert-agent/**" `
  --include "azure.yaml" `
  --include "data/**" `
  --include "docs/**" `
  --include "README.md" `
  --include "AGENTS.md" `
  --include ".gitignore" `
  --include ".github/copilot-instructions.md" `
  --dry-run
```

After reviewing the generated diff, rerun with `--yes` to commit and push the branch. Add `--pr` to open a pull request, or reuse an existing open PR for the same branch:

```powershell
autorepo pack publish . `
  --target-repo sethjuarez/ghcp-starters `
  --target-path packs/contract-expert `
  --branch pack/contract-expert-refresh `
  --pr `
  --yes
```

`pack publish` uses the local `git` and `gh` CLIs instead of handling credentials itself. By default it clones the target repository into a temporary checkout, creates or updates the requested branch, runs `pack update`, validates the pack, commits changed files under `--target-path`, pushes the branch, and optionally opens a PR. Use `--target-checkout <PATH>` to use the `origin` from an existing local checkout without mutating that checkout. Use `--base <BRANCH>` when you want the first publish branch based on a branch other than the target repository default.

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

## Experimental session snapshots

`autorepo labs session` is an experimental warmup tool for demos that need a Copilot app session to look lived-in, including completed turns. It treats local Copilot app state as an implementation detail and patches only a narrow allowlist of session/index records.

Inspect a local session without changing state:

```powershell
autorepo labs session inspect <SESSION_ID>
```

Capture a fixture with explicit transcript opt-in:

```powershell
autorepo labs session capture <SESSION_ID> --out .\snapshots\facilitator --include-transcripts
```

Captured fixtures include user and assistant transcript text so they can recreate completed turns. Review them before sharing, and do not commit fixtures that contain secrets, private repo content, personal paths, or non-demo conversation history.

Rehydrate into an isolated Copilot home that has been seeded with Copilot app databases:

```powershell
# With the Copilot app closed, seed an isolated home first.
New-Item -ItemType Directory -Force .\.tmp\copilot-home
Copy-Item $HOME\.copilot\data.db, $HOME\.copilot\session-store.db .\.tmp\copilot-home\

autorepo labs session rehydrate sethjuarez/autorepo-test-demo `
  --snapshot .\snapshots\facilitator `
  --copilot-home .\.tmp\copilot-home `
  --workspace .\.tmp\workspace `
  --yes
```

Rehydrate writes a new session id, synthesized session folder, and app/history index rows for the target repository. The target Copilot home must already contain `data.db` and `session-store.db`, and the target repository must already be present as a configured project in that home. By default rehydrate refuses to write the live default `~/.copilot` home; pass `--copilot-home` for a seeded isolated home, or `--allow-live-copilot-home` when you intentionally want to patch the live app state.

Close the Copilot app before writing to a live Copilot home. Running app instances may keep SQLite state cached or locked.

## Development

Run the same deterministic gates used by CI:

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run -- validate builtin
cargo run -- prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run
cargo run -- pack-from . --out .\.tmp\self-pack --id self-pack --name "Self pack" --include "README.md" --dry-run
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
