# autorepo

Use this skill when preparing a GitHub repository for a demo, workshop, or agent workflow with the `autorepo` CLI.

`autorepo` is intentionally small and explicit:

- Use explicit packs only; do not infer or auto-select scenarios.
- Keep deterministic GitHub repository hydration in `prepare`.
- Keep Copilot app/session warmup in `warm` or `labs`.
- Treat `labs session` as experimental local Copilot app state work, not normal repository state.

## Commands

Inspect auth and target syntax:

```powershell
autorepo doctor OWNER/REPO
```

Validate a pack source:

```powershell
autorepo validate builtin
autorepo validate .\packs\generic-starter
autorepo validate "github:OWNER/REPO//path/to/pack?ref=main"
autorepo validate "https://github.com/OWNER/REPO/tree/main/path/to/pack"
```

Plan deterministic repository hydration:

```powershell
autorepo prepare OWNER/REPO --pack <PACK_SOURCE> --dry-run
```

Apply deterministic safe writes only after review:

```powershell
autorepo prepare OWNER/REPO --pack <PACK_SOURCE> --yes
```

Render Copilot app warmup guidance:

```powershell
autorepo warm OWNER/REPO --pack <PACK_SOURCE>
autorepo warm OWNER/REPO --pack <PACK_SOURCE> --only facilitator_session --open-app
```

Scaffold a starter pack from a local checkout:

```powershell
autorepo pack-from <SOURCE> --out <PACK_DIR> --id <PACK_ID> --name <PACK_NAME> --include "README.md"
```

For contract-expert-style extraction, explicitly include the source paths that belong in the starter:

```powershell
autorepo pack-from C:\path\to\contract-policy-expert `
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

Remote sources can be `OWNER/REPO`, `github:OWNER/REPO//path?ref=<branch-or-tag>`, or a GitHub URL:

```powershell
autorepo pack-from caldova/contract-policy-expert --ref main --out .\packs\contract-expert --id contract-expert --name "Contract expert" --include "README.md"
autorepo pack-from "github:caldova/contract-policy-expert//fixtures/source?ref=main" --out .\packs\contract-expert --id contract-expert --name "Contract expert"
```

Refresh an existing pack after improving a demo checkout:

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

Publish that refresh as a reviewable branch in a pack catalog:

```powershell
autorepo pack publish . `
  --target-repo sethjuarez/ghcp-starters `
  --target-path packs/contract-expert `
  --branch pack/contract-expert-refresh `
  --dry-run
```

After reviewing the diff, rerun with `--yes` to commit and push the branch, and add `--pr` to open or reuse a GitHub pull request. `pack publish` uses local `git` and `gh` credentials; it does not manage tokens. Use `--target-checkout <PATH>` to borrow that checkout's `origin` without mutating its branch or files. Use `--replace` only when curated pack metadata/templates should be regenerated instead of preserved.

Experimental session snapshot flow:

```powershell
autorepo labs session inspect <SESSION_ID>
autorepo labs session capture <SESSION_ID> --out .\snapshots\facilitator --include-transcripts
autorepo labs session rehydrate OWNER/REPO --snapshot .\snapshots\facilitator --copilot-home .\.tmp\copilot-home --workspace <CHECKOUT> --yes
```

## Pack sources

Supported pack sources are:

- `builtin` or `generic-starter`
- local folder containing `pack.yml` or `pack.yaml`
- `github:OWNER/REPO?ref=main`
- `github:OWNER/REPO//path/to/pack?ref=main`
- `https://github.com/OWNER/REPO/tree/main/path/to/pack`

Remote pack sources are shallow-cloned into a temporary folder. GitHub tree URLs resolve actual branch/tag names, including slash-containing refs.

## Pack scaffolding

`autorepo pack-from` is pack authoring from a local checkout or GitHub repository source. Use `--ref <branch-or-tag>` with remote sources when the source string does not already include `?ref=` or a `/tree/<ref>/...` URL segment. It copies selected UTF-8 text files to `templates/files/<repo-relative-path>`, preserves target paths in generated `files` entries, writes a strict `pack.yml`, and validates the generated pack before reporting success.

Use repeated `--include` globs to keep the starter intentional. Use repeated `--exclude` globs to remove project-specific files. The command has always-on hard exclusions for `.git`, other VCS metadata, real `.env` files while allowing explicit templates like `.env.example`, private keys/certificates, credentials files, editor state, OS files, logs, local databases, dependency folders, caches, build outputs, virtual environments, symlinks, unreadable files, and binary/non-UTF-8 files. Includes do not override these hard safety exclusions.

`--with-issues` adds a single review issue stub. `--with-warmup` adds a repo app link and review app-session prompt. Keep rich labels, milestones, pull requests, workflow dispatches, issue migration, automation schedules, binary assets, and session snapshots manual unless the pack author intentionally adds them after reviewing the generated pack.

`autorepo pack update` is the refresh path for the demo loop. It keeps curated manifest sections, comments outside generated fields, and non-file templates, refreshes only `templates/files/**`, surgically replaces manifest `files`, recalculates `safety.max_writes`, and validates the pack. Metadata flags like `--id`, `--name`, and `--description` require `--replace` when updating an existing pack. `autorepo pack publish` wraps `pack update` with target-repository clone/branch/commit/push/optional-PR primitives so pack catalogs can receive community-style contributions by branch and pull request.

## Safety rules

- Always run `validate` before `prepare --yes`.
- Prefer `prepare --dry-run` and inspect the plan before applying.
- Never use `autorepo` as a cleanup/reset/destructive tool; V1 creates but does not delete.
- Do not overwrite user content or bypass non-empty repository checks.
- Review `pack-from` skip output before sharing a generated pack; skipped files are deliberate safety signals, not noise.
- Keep cloud-agent/session warmup explicit because it is asynchronous and nondeterministic.
- For `labs session capture`, fixtures contain transcript text. Review them before sharing and do not commit secrets, private repo content, personal paths, or unrelated conversation history.
- For `labs session rehydrate`, use a seeded isolated `--copilot-home` when possible. Writing live `~/.copilot` requires `--allow-live-copilot-home` and the Copilot app should be closed.

## Quality gates

Before committing changes to `autorepo`, run:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --quiet -- validate builtin
cargo run --quiet -- pack-from . --out .\.tmp\self-pack --id self-pack --name "Self pack" --include "README.md" --dry-run
```

For session snapshot changes, also test against a copied Copilot home, not live `~/.copilot`.
