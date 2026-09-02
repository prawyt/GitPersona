---
name: release-sync
description: "Sync develop branch to main by creating a pull request, verifying CI, merging via merge commit (NEVER squash/rebase), and tagging a release (e.g. v0.1.0)."
---

# Release Sync Skill (`release-sync`)

Use this skill when you need to sync the `develop` branch to `main`, create a release PR, ensure a standard merge commit is used, and create/push a semantic version tag once `main` is updated.

> **CRITICAL RULE**: For syncing `develop` to `main`, **ALWAYS use a merge commit** (`--merge`). Never squash or rebase when merging `develop` into `main` so branch history remains intact and synchronized.

## Workflow

### 1. Pre-flight Checks
Ensure local branches and refs are up-to-date with remote:
```bash
git fetch origin develop main
```

Verify commits pending merge from `develop` into `main`:
```bash
git log origin/main..origin/develop --oneline
```

### 2. Create PR to Sync Develop -> Main
Use GitHub CLI to create the pull request targeting `main`:
```bash
gh pr create --base main --head develop --title "Release <version>: Sync develop to main" --body "## Release <version> Summary

Sync develop into main.

### Included Commits
$(git log origin/main..origin/develop --pretty=format:"* %s (%h)")"
```

### 3. Merge PR (ALWAYS Merge Commit)
Once CI passes and approvals are obtained, merge the PR **strictly with a merge commit**:
```bash
# Explicitly use --merge flag (Do NOT use --squash or --rebase)
gh pr merge <PR_NUMBER> --merge
```

### 4. Fetch Updated Main
Update references:
```bash
git fetch origin main
```

### 5. Create and Push Tag
Create an annotated release tag pointing to the merge commit on `main` and push it to remote:
```bash
git tag -a <version> origin/main -m "Release <version>"
git push origin <version>
```

Optionally generate GitHub Release notes:
```bash
gh release create <version> --title "<version>" --generate-notes
```
