# crates.io publishing

`autorepo` is prepared for token-based crates.io publishing from GitHub Actions using the `CARGO_REGISTRY_TOKEN` repository secret.

## Workflow

Publishing is handled by `.github/workflows/publish.yml`.

- Pull requests run `cargo publish --dry-run`.
- Manual workflow runs default to `cargo publish --dry-run`.
- Manual workflow runs can publish when `publish` is set to `true`.
- GitHub Release publication also publishes to crates.io.
- The publish job uses the `crates-io` GitHub Environment so maintainers can add reviewers or deployment protection rules.
- Real publishing reads `${{ secrets.CARGO_REGISTRY_TOKEN }}` and passes it to `cargo publish` through the `CARGO_REGISTRY_TOKEN` environment variable.
- The workflow never prints or requests the secret value.

Release-please remains responsible for release PR and release creation in `.github/workflows/release-please.yml`.
Release-please requires the repository setting that allows GitHub Actions to create and approve pull requests, plus workflow permissions `contents: write` and `pull-requests: write`.

## Token-based publishing values

Configure a crates.io API token as this GitHub Actions repository secret:

| Field | Value |
|---|---|
| GitHub secret name | `CARGO_REGISTRY_TOKEN` |
| GitHub repository owner/account | `sethjuarez` |
| GitHub repository name | `autorepo` |
| Workflow filename | `publish.yml` |
| Environment name | `crates-io` |
| Crate name | `autorepo` |

## Trusted publishing alternative

If the project switches from token-based publishing to crates.io trusted publishing/OIDC later, configure the `autorepo` crate on crates.io with these trusted publishing values:

| Field | Value |
|---|---|
| GitHub repository owner/account | `sethjuarez` |
| GitHub repository name | `autorepo` |
| Workflow filename | `publish.yml` |
| Environment name | `crates-io` |
| Crate name | `autorepo` |

The publishing workflow would also need `permissions: id-token: write`, `rust-lang/crates-io-auth-action@v1`, and `CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}` for the `cargo publish` step.
