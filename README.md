# autorepo

`autorepo` prepares GitHub repositories for demos, workshops, and agent workflows from explicit packs.

V1 is intentionally small: it validates a pack, renders a deterministic plan, and keeps GitHub writes behind a narrow serial executor seam. It is not a repository classifier, Terraform replacement, cleanup tool, or plugin framework.

## Commands

```powershell
autorepo doctor OWNER/REPO
autorepo validate <PACK_DIR>
autorepo prepare OWNER/REPO --pack <PACK_DIR|builtin> --dry-run
autorepo prepare OWNER/REPO --pack <PACK_DIR|builtin> --yes
autorepo warm OWNER/REPO --pack <PACK_DIR|builtin> --start-agent-tasks
```

Use `builtin` or `generic-starter` to load the included starter pack.

## Safety model

- Uses explicit packs only; there is no automatic scenario selection.
- Plans are deterministic and validated before execution.
- Execution is serial.
- V1 never deletes, force-pushes, overwrites different user content, or reconciles drift.
- Issues and pull requests are intended to be matched by embedded `autorepo` markers.
- Copilot cloud-agent warmup is represented as an explicit `warm` phase because those sessions are async and nondeterministic.

## Pack shape

A pack is a directory containing `pack.yml` and templates:

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

Manifest validation rejects unknown fields, unsafe paths, unresolved references, undeclared labels, unstable IDs, and write counts above `safety.max_writes`.

## Development

```powershell
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -- validate builtin
cargo run -- prepare sethjuarez/autorepo-test-dry-run --pack builtin --dry-run
```

Install the optional pre-commit hooks to run the same deterministic local gates before each commit:

```powershell
pipx install pre-commit
pre-commit install
pre-commit run --all-files
```

Live GitHub tests are never part of normal tests or pre-commit. See `docs/live-github-testing.md` for the opt-in fake-repo guard and future live test contract.

Crates.io publishing is prepared through the `CARGO_REGISTRY_TOKEN` GitHub secret, with trusted publishing/OIDC documented as an alternative. See `docs/publishing.md` for the exact configuration values.

## License

This project is licensed under the MIT License.
