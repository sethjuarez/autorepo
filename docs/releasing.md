# Releasing autorepo

`autorepo` has two release lanes:

- the published `autorepo` CLI crate on crates.io
- the desktop app bundles produced from `apps\desktop`

Release Please owns version PRs and tags from the root manifest. The configuration updates the root package version, the root Cargo workspace version, the CLI crate version in `apps\cli\Cargo.toml`, the desktop package version, and Tauri's `Cargo.toml` and `tauri.conf.json` versions.

## Release Please

`release-please.yml` runs on pushes to `main` and can also be dispatched manually. When Release Please creates a release, it dispatches `release.yml` for the created tag.

This follows the CutReady-style split:

- Release Please creates release PRs and tags.
- `release.yml` builds desktop platform artifacts from a tag.
- `publish.yml` remains the explicit crates.io lane for the CLI crate.

## CLI crate publishing

The existing crates.io publishing path is preserved and made workspace-aware. The `autorepo` package exposes both the CLI binary and the shared library API used by the desktop app, so crates.io still has one public crate:

```powershell
cargo publish -p autorepo --dry-run
cargo publish -p autorepo
```

The GitHub workflow keeps publishing behind `workflow_dispatch`, the `crates-io` environment, and `CARGO_REGISTRY_TOKEN`. Run the dry-run path before publishing the real crate. Supporting crates under `crates\` should stay private until there is a clear reason to publish a separate package.

## Desktop assets

`release.yml` builds Tauri desktop bundles on Linux, Windows, and macOS from the release tag. Apple signing and notarization secrets are consumed only by the release workflow.

The desktop release lane is separate from crates.io publishing so a desktop packaging failure does not accidentally half-run CLI publishing.

Expected desktop release artifacts:

| Matrix key | Runner | Target | Expected bundles |
|---|---|---|---|
| `linux-x64` | `ubuntu-24.04` | default Linux target | Linux bundles from Tauri `targets = "all"` such as `.deb`, `.rpm`, and `.AppImage`. |
| `windows-x64` | `windows-latest` | default Windows target | Windows installer bundles such as `.msi` and NSIS `.exe`. |
| `macos-silicon` | `macos-latest` | `aarch64-apple-darwin` | Apple Silicon macOS bundles such as `.dmg`; updater archives are not enabled yet. |
| `macos-intel` | `macos-latest` | `x86_64-apple-darwin` | Intel macOS bundles such as `.dmg`; updater archives are not enabled yet. |

The Tauri action attaches bundle assets to the `v*` GitHub release created by the `resolve-tag` job. After a release run, verify the asset list includes all four matrix legs:

```powershell
gh release view v0.10.0 --repo sethjuarez/autorepo
gh release view v0.10.0 --repo sethjuarez/autorepo --json assets --jq '.assets[].name'
```

## Versioning policy

Release Please keeps the CLI and desktop app on the same semantic version for now. A release PR must update all of these together:

- root `package.json`
- root Cargo workspace package version in `Cargo.toml`
- `apps\cli\Cargo.toml`
- `apps\desktop\package.json`
- `apps\desktop\src-tauri\Cargo.toml`
- `apps\desktop\src-tauri\tauri.conf.json`

Do not split desktop versioning from CLI versioning until there is a separate desktop release cadence and a user-facing reason to explain the divergence.

`Cargo.lock` is not listed in `release-please-config.json`, so a version PR that changes Cargo package versions should refresh it before merge:

```powershell
cargo check --workspace
git diff -- Cargo.lock
```

If `Cargo.lock` changes, commit the refreshed lockfile to the release PR before merging it; this is a manual release gate, not currently enforced by Release Please.

## Local release smoke test

Before dispatching `release.yml` or merging a release PR that affects desktop packaging, run the local confidence gates from a clean worktree. The Auditaur drill requires `auditaur` on `PATH` and writes transient session files under `.auditaur`.

```powershell
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm run test
npm run build
cargo check -p autorepo-desktop
npm --workspace apps/desktop run tauri -- build
npm run drill:desktop
cargo publish -p autorepo --dry-run
```

`npm --workspace apps/desktop run tauri -- build` is the production packaging gate. It catches release-bundle failures such as missing icons, resource mapping problems, or production CSP mistakes. Launch the produced local bundle once and confirm the packaged `generic-starter` pack loads.

`npm run drill:desktop` is the desktop dev-runtime smoke gate. It starts the Tauri app under Auditaur, loads the local `packs` collection, verifies the plan preview, runs the no-write dry-run, exercises pack search, and fails if frontend errors, exceptions, or failed IPC calls are observed.
