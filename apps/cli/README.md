# autorepo CLI

This is the published `autorepo` command-line package.

Install from a monorepo checkout:

```powershell
cargo install --path .\apps\cli
```

The package carries its built-in `generic-starter` pack under `apps\cli\packs` so crates.io installs can run:

```powershell
autorepo validate builtin
```

See the repository root `README.md` for product docs, desktop UI notes, and pack authoring guidance.
