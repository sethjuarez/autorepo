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

## Safety rules

- Always run `validate` before `prepare --yes`.
- Prefer `prepare --dry-run` and inspect the plan before applying.
- Never use `autorepo` as a cleanup/reset/destructive tool; V1 creates but does not delete.
- Do not overwrite user content or bypass non-empty repository checks.
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
```

For session snapshot changes, also test against a copied Copilot home, not live `~/.copilot`.
