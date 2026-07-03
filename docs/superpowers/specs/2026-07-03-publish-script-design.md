# Publish Script Design — Smart Todo

**Date:** 2026-07-03  
**Status:** Approved

## Overview

A single PowerShell script `publish.ps1` at the project root that builds the Smart Todo Tauri application, auto-bumps the patch version, creates a backup of the previous release, and deploys the portable `.exe` to `D:\Programs\smart-todo`.

## Deployment Target

```
D:\Programs\smart-todo\
  smart-todo.exe          ← active version
  backups\
    smart-todo-0.1.0.exe  ← previous versions
    smart-todo-0.1.1.exe
    ...
  logs\
    publish-2026-07-03_14-32-05.log
    ...
```

## Version Bumping

- **Strategy:** Auto-bump patch segment (`MAJOR.MINOR.PATCH`)
- **Source of truth:** `tauri.conf.json` → `version` field
- **Files updated on each publish:**
  1. `tauri.conf.json` — JSON, updated with `ConvertFrom-Json` / `ConvertTo-Json`
  2. `package.json` — JSON, same approach
  3. `src-tauri/Cargo.toml` — TOML, updated with regex (`version = "X.Y.Z"`)

## Execution Flow

Steps execute in this exact order. Script stops on any failure (`$ErrorActionPreference = "Stop"`).

```
1. Read current version from tauri.conf.json
2. Bump patch:  0.1.0 → 0.1.1
3. Update tauri.conf.json, package.json, Cargo.toml
4. git add those 3 files
5. git commit "chore: bump version to 0.1.1"
6. npx tauri build                    ← may fail here
7. git tag v0.1.1                     ← tag only after successful build
8. Backup D:\Programs\smart-todo\smart-todo.exe
        → D:\Programs\smart-todo\backups\smart-todo-{old_version}.exe
   (skip if no previous .exe exists)
9. Copy src-tauri/target/release/smart-todo.exe
        → D:\Programs\smart-todo\smart-todo.exe
10. Write final log entry and print summary
```

## Error Handling

| Scenario | Behaviour |
|---|---|
| Uncommitted changes in git | Warn user and ask to confirm before continuing |
| `D:\Programs\smart-todo` doesn't exist | Create it (including `backups\` and `logs\` subdirs) |
| No previous `.exe` to backup | Skip backup silently — not an error (first deploy) |
| `npx tauri build` fails | Abort. Files bumped, commit done, but **no tag** yet. User must fix and re-run. |
| Old `.exe` locked (app running) | Catch the copy error, print clear message, exit 1 |

> **Note on re-run after build failure:** version is already bumped and committed. Running the script again would double-bump. User should manually revert the version commit or fix the build error and re-run with the current version. This is documented in the script's header comment.

## Logging

- **Location:** `D:\Programs\smart-todo\logs\publish-YYYY-MM-DD_HH-mm-ss.log`
- **Format:** Each line prefixed with `[YYYY-MM-DD HH:mm:ss]`
- **Scope:** All steps, their outcomes, and any exceptions
- **Stdout:** Same lines printed to terminal in real time (dual output)
- **Retention:** Logs are never auto-deleted. Each publish creates one ~1-2 KB file.

### Log example

```
[2026-07-03 14:32:05] === Smart Todo Publish Script ===
[2026-07-03 14:32:05] Wersja: 0.1.0 → 0.1.1
[2026-07-03 14:32:05] Sprawdzanie git status... OK (czyste drzewo)
[2026-07-03 14:32:06] Zaktualizowano tauri.conf.json
[2026-07-03 14:32:06] Zaktualizowano package.json
[2026-07-03 14:32:06] Zaktualizowano Cargo.toml
[2026-07-03 14:32:07] git commit: chore: bump version to 0.1.1
[2026-07-03 14:32:08] npx tauri build — start
[2026-07-03 14:35:42] npx tauri build — zakończony pomyślnie
[2026-07-03 14:35:42] git tag v0.1.1
[2026-07-03 14:35:42] Backup: smart-todo.exe → backups/smart-todo-0.1.0.exe
[2026-07-03 14:35:43] Skopiowano: smart-todo.exe → D:\Programs\smart-todo\
[2026-07-03 14:35:43] === Gotowe. Wersja 0.1.1 wdrożona. ===
```

## Usage

```powershell
# From project root:
.\publish.ps1
```

No parameters required. Version is bumped automatically.

## Files Created / Modified

| File | Change |
|---|---|
| `publish.ps1` | New — publish script |
| `tauri.conf.json` | Version bumped on each publish |
| `package.json` | Version bumped on each publish |
| `src-tauri/Cargo.toml` | Version bumped on each publish |
