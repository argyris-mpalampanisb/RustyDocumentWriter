# GitHub commands cheat sheet

Run these from `D:\LO2024Theme\vellum`.

## 1. First-time setup (once)

```powershell
git init
git add .
git commit -m "Initial commit: RustyDocumentWriter word processor"
git branch -M main
```

Make sure the repository **https://github.com/argyris-mpalampanisb/RustyDocumentWriter** is **empty** (no README or license added on GitHub), so it doesn't conflict with your local files. Then connect it and push:

```powershell
git remote add origin https://github.com/argyris-mpalampanisb/RustyDocumentWriter.git
git push -u origin main
```

> Keep the repo **public** if you can: GitHub's ARM64 Linux runners (`ubuntu-22.04-arm`) are free only for public repos.

## 2. Build all six executables (no release)

- On GitHub: **Actions** → **Release** → **Run workflow** → **Run workflow**
- Or from the terminal: `gh workflow run release.yml`

When it finishes, the zips/tarballs are under **Artifacts** at the bottom of the run page.
To download them with the CLI: `gh run download`

## 3. Publish a release (builds + attaches all archives)

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The archives appear under **Releases** on the repo page.

## 4. Everyday changes

```powershell
git status                      # what changed
git add .
git commit -m "Describe the change"
git push
```

New release afterwards: bump `version` in `Cargo.toml`, commit, then tag `v0.1.1` (and so on) as in step 3.

## 5. Fixing mistakes

| Situation | Command |
|---|---|
| Undo the last commit, keep the changes | `git reset --soft HEAD~1` |
| Discard local changes to a file | `git restore <file>` |
| Delete a tag (local + remote) to re-run a release | `git tag -d v0.1.0` then `git push origin :refs/tags/v0.1.0` |
| See recent commits | `git log --oneline -10` |

## Notes

- `Cargo.lock` must be committed: the workflow builds with `--locked`. `git add .` includes it.
- `target/`, `dist/` and `render-out/` are excluded by `.gitignore`, so build output won't be uploaded.
- If a CI job fails, open it in **Actions** to see the log. The jobs run independently, so one failing platform doesn't stop the others.
