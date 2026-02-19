# AGENTS.md

## Commit Message Convention

This project uses **Conventional Commits** to drive automated releases via [release-please](https://github.com/googleapis/release-please). Every commit message merged to `main` affects version bumps and changelog generation. Follow this format exactly.

### Format

```
<type>(<optional scope>): <description>

<optional body>

<optional footer>
```

### Types

| Type | Purpose | Version bump |
|---|---|---|
| `feat` | New feature or capability | Minor (`0.1.0` → `0.2.0`) |
| `fix` | Bug fix | Patch (`0.1.0` → `0.1.1`) |
| `docs` | Documentation only | None |
| `style` | Formatting, whitespace (no logic change) | None |
| `refactor` | Code restructuring (no feature/fix) | None |
| `perf` | Performance improvement | None |
| `test` | Adding or updating tests | None |
| `chore` | Build, CI, tooling, dependencies | None |

### Breaking Changes

Append `!` after the type or add a `BREAKING CHANGE:` footer. While on `0.x.x`, breaking changes bump **minor** (not major).

```
feat!: redesign settings storage API

BREAKING CHANGE: settings are now stored in SQLite instead of JSON files.
```

### Scope

Optional. Use the affected module name:

- `ui` — React frontend (`src/`)
- `backend` — Rust backend (`src-tauri/src/`)
- `db` — Database layer
- `polling` — Background polling system
- `tray` — System tray integration
- `cli` — CLI wrapper (`scripts/`)
- `ci` — GitHub Actions, release config

### Examples

```
feat(ui): add dark mode toggle to settings panel

fix(polling): prevent duplicate requests when interval is shorter than response time

chore(ci): update Rust toolchain to 1.82

refactor(backend): extract HTTP client into shared module

feat(cli): support tracking multiple commands in parallel

fix(db): handle migration failure on corrupted database
```

### Rules

1. **Subject line**: imperative mood, lowercase, no period, max 72 chars
2. **Body**: explain *what* and *why*, not *how*. Wrap at 80 chars.
3. **One logical change per commit**. Don't mix a feature with a refactor.
4. **No version bumps in commit messages**. Release-please handles versioning automatically.
5. Commits with types `feat` and `fix` appear in the auto-generated CHANGELOG. Use other types for changes that don't need user-facing release notes.

## Project Structure

```
in-the-loop/
├── src/                    # React frontend (TypeScript)
├── src-tauri/              # Rust backend (Tauri v2)
│   ├── src/                # Rust source
│   └── tauri.conf.json     # Tauri config (version synced by release-please)
├── scripts/                # CLI wrapper
├── .github/workflows/      # CI/CD
├── release-please-config.json
└── .release-please-manifest.json
```

## Release Process

Automated via GitHub Actions on merge to `main`:

1. `release-please` analyzes conventional commits and opens/updates a Release PR
2. The Release PR bumps versions in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`
3. When the Release PR is merged, a GitHub Release is created with a tag
4. The `build-tauri` job builds macOS installers (aarch64 + x86_64) and uploads `.dmg` files as release assets

## Tech Stack

- **Frontend**: React 18, TypeScript, Vite
- **Backend**: Rust, Tauri v2, SQLite (rusqlite), Axum
- **Build**: `npm run tauri:build` → `.dmg` installer
- **CI**: GitHub Actions, release-please, tauri-action
