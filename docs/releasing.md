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

`release.yml` builds Tauri desktop bundles on Linux, Windows, and macOS from the release tag. Signing and notarization secrets are consumed only by the release workflow.

The desktop release lane is separate from crates.io publishing so a desktop packaging failure does not accidentally half-run CLI publishing.
