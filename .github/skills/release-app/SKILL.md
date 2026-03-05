---
name: release-app
description: Release a new version of the In The Loop app. Use this skill when asked to release, create a release, ship a version, merge PRs for release, or prepare a release branch.
---

# Release App

This skill automates the release process for the In The Loop Tauri desktop app. It handles merging pending PRs into a release branch, creating a release PR, and merging it to `main` where CI takes over (version bump, GitHub Release, macOS builds).

## Prerequisites

- `gh` CLI must be authenticated (`gh auth status`)
- GitHub MCP server tools must be available
- Repository remote is `mszkudelski/in-the-loop`

## Parameters

Before starting, determine from the user's request (ask if not specified):

| Parameter | Default | Description |
|-----------|---------|-------------|
| Version | Next minor (feat) or patch (fix only) | Target release version, e.g. `0.10.0` |
| PRs to include | All open PRs | Specific PR numbers, or "all" |

## Steps

### 1. Determine the version

Read `package.json` to get the current version. Determine the next version based on the PRs to be included:
- If any PR title starts with `feat` → bump **minor** (e.g. `0.9.0` → `0.10.0`)
- If only `fix` PRs → bump **patch** (e.g. `0.9.0` → `0.9.1`)
- If the user specified a version, use that instead

### 2. List open PRs

Use the GitHub MCP `list_pull_requests` tool to list all open PRs in `mszkudelski/in-the-loop`.

Present the list to the user and confirm which PRs to include. Example:

```
Found 5 open PRs:
- #30: feat(ui): add dark mode toggle
- #31: fix(polling): handle timeout errors
- #32: docs: update README
...

Include all in release v0.10.0?
```

### 3. Create or update the release branch

```bash
git fetch origin main
git checkout -b release/v{VERSION} origin/main
git push -u origin release/v{VERSION}
```

If the branch already exists, ask the user whether to reuse it or recreate it.

### 4. Retarget and merge each PR

For each PR to include:

1. Change the PR's base branch to `release/v{VERSION}`:
   ```bash
   gh pr edit {PR_NUMBER} --base release/v{VERSION}
   ```

2. Squash-merge the PR:
   ```bash
   gh pr merge {PR_NUMBER} --squash
   ```

3. If merge fails (e.g. conflicts), report the error and ask the user how to proceed.

### 5. Create the release PR

Generate the release PR body from the merged PRs, categorized by type:

```markdown
## Release v{VERSION}

### Features
- **feat(scope):** description (#PR)

### Bug Fixes
- **fix(scope):** description (#PR)

### Other
- **refactor/perf/etc:** description (#PR)

---

All {N} pending PRs squash-merged into this release branch.
```

Create the PR:
```bash
gh pr create \
  --base main \
  --head release/v{VERSION} \
  --title "release: v{VERSION}" \
  --body "{RELEASE_BODY}"
```

### 6. Merge the release PR to main

Merge the release PR with a merge commit (not squash, to preserve individual commit history):
```bash
gh pr merge {RELEASE_PR_NUMBER} --merge
```

### 7. Close leftover PRs

If any of the original PRs still show as "open" (because they were squash-merged into the release branch, not directly into main), close them with a comment:
```bash
gh pr close {PR_NUMBER} --comment "Included in release v{VERSION} (PR #{RELEASE_PR_NUMBER})"
```

### 8. Verify

After merging to main, the CI release workflow (`release.yml`) will automatically:
1. Detect conventional commits and determine version bump
2. Update version in `package.json`, `tauri.conf.json`, `Cargo.toml`
3. Create a GitHub Release with generated notes
4. Build macOS `.dmg` installers (aarch64 + x86_64)

Verify by checking the GitHub Actions run:
```bash
gh run list --workflow=release.yml --limit=1
```

Report the release status to the user.

## Important Notes

- **Always use `--merge` (not `--squash`) for the release PR** so individual squash commits are preserved in `main` history. This ensures the release notes generator can see all conventional commit messages.
- **Always use `--squash` for feature/fix PRs into the release branch** to keep the release branch clean.
- **Do NOT manually bump versions** — the CI workflow handles this automatically from conventional commits.
- If a PR has merge conflicts with the release branch, ask the user to resolve them before proceeding.

## Output

After completion, report:

| Field | Value |
|-------|-------|
| **Version** | v{VERSION} |
| **PRs merged** | #{N1}, #{N2}, ... |
| **Release PR** | #{RELEASE_PR_NUMBER} |
| **CI status** | Running / Completed |
| **Release URL** | https://github.com/mszkudelski/in-the-loop/releases/tag/v{VERSION} |
