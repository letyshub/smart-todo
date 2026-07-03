# Publish Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `publish.ps1` — a PowerShell script that bumps the patch version, builds the Tauri app, backs up the previous `.exe`, and deploys the new portable `.exe` to `D:\Programs\smart-todo`.

**Architecture:** Single self-contained PowerShell script at the project root. Version is read from `tauri.conf.json` (source of truth), bumped patch-wise, then propagated to `package.json` and `src-tauri/Cargo.toml` via regex replacement to preserve original file formatting. Git commit is created before the build; git tag is created only after a successful build.

**Tech Stack:** PowerShell 7+, git CLI, npx / Tauri CLI

## Global Constraints

- PowerShell 7+ required (`#Requires -Version 7.0`)
- `$ErrorActionPreference = "Stop"` — abort on any cmdlet error
- Native commands (`git`, `npx`) checked via `$LASTEXITCODE` — they don't throw
- Deploy target: `D:\Programs\smart-todo\smart-todo.exe`
- Backup target: `D:\Programs\smart-todo\backups\smart-todo-{oldVersion}.exe`
- Log target: `D:\Programs\smart-todo\logs\publish-{timestamp}.log`
- Version source of truth: `src-tauri/tauri.conf.json` → `.version` field
- All three version files updated via regex — do **not** use `ConvertTo-Json` (it reformats the file and creates noisy diffs)
- Script must be run from the project root directory

---

### Task 1: Script skeleton, config, logging, directory setup

**Files:**
- Create: `publish.ps1`

**Interfaces:**
- Produces: `Write-Log` function with signature `Write-Log([string]$Message)` — used by all later tasks

- [ ] **Step 1: Create `publish.ps1` with header and config**

  ```powershell
  #Requires -Version 7.0

  <#
  .SYNOPSIS
      Publish Smart Todo to D:\Programs\smart-todo

  .DESCRIPTION
      Auto-bumps patch version, builds with Tauri, backs up previous .exe,
      deploys new .exe to D:\Programs\smart-todo.

      RE-RUN AFTER BUILD FAILURE:
      If the build fails, the version was already bumped and committed.
      Running this script again would double-bump the version.
      Fix the build error first, then run the script again — it will bump
      to the next patch. If you want to keep the same version number, do:
          git revert HEAD --no-edit
      before re-running the script.
  #>

  [CmdletBinding()]
  param()

  $ErrorActionPreference = "Stop"

  # ---------------------------------------------------------------------------
  # Config
  # ---------------------------------------------------------------------------
  $DeployDir = "D:\Programs\smart-todo"
  $BackupDir = "$DeployDir\backups"
  $LogDir    = "$DeployDir\logs"
  $ExeName   = "smart-todo.exe"
  $BuildExe  = "src-tauri\target\release\$ExeName"
  ```

- [ ] **Step 2: Create deploy directories**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Directories
  # ---------------------------------------------------------------------------
  New-Item -ItemType Directory -Force -Path $DeployDir | Out-Null
  New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
  New-Item -ItemType Directory -Force -Path $LogDir    | Out-Null
  ```

- [ ] **Step 3: Add logging helper and init log file**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Logging
  # ---------------------------------------------------------------------------
  $LogTimestamp = Get-Date -Format "yyyy-MM-dd_HH-mm-ss"
  $LogFile      = "$LogDir\publish-$LogTimestamp.log"

  function Write-Log {
      param([string]$Message)
      $ts   = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
      $line = "[$ts] $Message"
      Write-Host $line
      Add-Content -Path $LogFile -Value $line -Encoding UTF8
  }

  Write-Log "=== Smart Todo Publish Script ==="
  ```

- [ ] **Step 4: Verify directories and logging work**

  From the project root:
  ```powershell
  .\publish.ps1
  ```

  Expected output:
  ```
  [2026-07-03 14:32:05] === Smart Todo Publish Script ===
  ```

  Verify:
  ```powershell
  Test-Path "D:\Programs\smart-todo\backups"   # True
  Test-Path "D:\Programs\smart-todo\logs"      # True
  ls "D:\Programs\smart-todo\logs"             # one publish-*.log file
  ```

- [ ] **Step 5: Commit**

  ```powershell
  git add publish.ps1
  git commit -m "feat: add publish.ps1 scaffold with logging"
  ```

---

### Task 2: Version read and bump

**Files:**
- Modify: `publish.ps1`

**Interfaces:**
- Consumes: `Write-Log([string]$Message)` from Task 1
- Produces:
  - `$oldVersion` — string, e.g. `"0.1.0"`
  - `$newVersion` — string, e.g. `"0.1.1"`
  - All three version files updated on disk

- [ ] **Step 1: Read current version and compute new version**

  Append to `publish.ps1` (after the logging section):

  ```powershell
  # ---------------------------------------------------------------------------
  # Version bump
  # ---------------------------------------------------------------------------
  $tauriConfPath = "src-tauri\tauri.conf.json"
  $packagePath   = "package.json"
  $cargoPath     = "src-tauri\Cargo.toml"

  $tauriConf  = Get-Content $tauriConfPath -Raw | ConvertFrom-Json
  $oldVersion = $tauriConf.version

  $parts      = $oldVersion -split "\."
  $newVersion = "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"

  Write-Log "Wersja: $oldVersion → $newVersion"
  ```

- [ ] **Step 2: Verify version parsing**

  Temporarily add at the end of the version section and run the script:
  ```powershell
  Write-Host "DEBUG oldVersion=$oldVersion newVersion=$newVersion"
  exit 0
  ```

  Expected:
  ```
  [2026-07-03 14:32:05] Wersja: 0.1.0 → 0.1.1
  DEBUG oldVersion=0.1.0 newVersion=0.1.1
  ```

  Remove the two debug lines after verifying.

- [ ] **Step 3: Update all three version files via regex**

  Append to `publish.ps1`:

  ```powershell
  # tauri.conf.json — replace "version": "X.Y.Z"
  $content = Get-Content $tauriConfPath -Raw
  $content = $content -replace '"version":\s*"[\d\.]+"', "`"version`": `"$newVersion`""
  [System.IO.File]::WriteAllText(
      (Resolve-Path $tauriConfPath).Path,
      $content,
      [System.Text.Encoding]::UTF8
  )
  Write-Log "Zaktualizowano tauri.conf.json"

  # package.json — replace "version": "X.Y.Z"
  $content = Get-Content $packagePath -Raw
  $content = $content -replace '"version":\s*"[\d\.]+"', "`"version`": `"$newVersion`""
  [System.IO.File]::WriteAllText(
      (Resolve-Path $packagePath).Path,
      $content,
      [System.Text.Encoding]::UTF8
  )
  Write-Log "Zaktualizowano package.json"

  # Cargo.toml — replace first ^version = "X.Y.Z" line (package version, not dep versions)
  $content = Get-Content $cargoPath -Raw
  $content = $content -replace '(?m)^version = "[\d\.]+"', "version = `"$newVersion`""
  [System.IO.File]::WriteAllText(
      (Resolve-Path $cargoPath).Path,
      $content,
      [System.Text.Encoding]::UTF8
  )
  Write-Log "Zaktualizowano Cargo.toml"
  ```

  > **Why `[System.IO.File]::WriteAllText`?** `Set-Content` on Windows adds `\r\n` line endings and may add a trailing newline, creating noisy git diffs. The .NET method writes the string exactly as-is.

- [ ] **Step 4: Verify files were updated correctly**

  Run the script, then check:

  ```powershell
  # tauri.conf.json should contain "version": "0.1.1"
  Select-String -Path "src-tauri\tauri.conf.json" -Pattern '"version"'
  # Expected: "version": "0.1.1"

  # package.json should contain "version": "0.1.1"
  Select-String -Path "package.json" -Pattern '"version"'
  # Expected: "version": "0.1.1"

  # Cargo.toml should contain version = "0.1.1"
  Select-String -Path "src-tauri\Cargo.toml" -Pattern '^version'
  # Expected: version = "0.1.1"
  ```

  Then revert the changes (we'll let the script do the real bump later):
  ```powershell
  git checkout -- src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml
  ```

- [ ] **Step 5: Commit**

  ```powershell
  git add publish.ps1
  git commit -m "feat: add version bump logic to publish.ps1"
  ```

---

### Task 3: Git status check and version commit

**Files:**
- Modify: `publish.ps1`

**Interfaces:**
- Consumes: `$oldVersion`, `$newVersion` from Task 2; `Write-Log` from Task 1
- Produces: A git commit with message `"chore: bump version to X.Y.Z"` containing only the 3 version file changes

- [ ] **Step 1: Add git status check before the version bump section**

  Insert this block **before** the version bump section (before `# Version bump`) in `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Git status check
  # ---------------------------------------------------------------------------
  Write-Log "Sprawdzanie git status..."
  $gitStatus = git status --porcelain
  if ($LASTEXITCODE -ne 0) {
      Write-Log "BŁĄD: nie można sprawdzić git status (kod $LASTEXITCODE)"
      exit 1
  }
  if ($gitStatus) {
      Write-Log "UWAGA: Niezacommitowane zmiany:"
      $gitStatus | ForEach-Object { Write-Log "  $_" }
      $answer = Read-Host "Kontynuować mimo niezacommitowanych zmian? [t/N]"
      if ($answer -ne "t" -and $answer -ne "T") {
          Write-Log "Przerwano przez użytkownika."
          exit 0
      }
  } else {
      Write-Log "Git status OK (czyste drzewo robocze)"
  }
  ```

- [ ] **Step 2: Add git commit after the version file updates**

  Append to `publish.ps1` (after the three file-update blocks):

  ```powershell
  # ---------------------------------------------------------------------------
  # Git commit (version bump)
  # ---------------------------------------------------------------------------
  git add $tauriConfPath $packagePath $cargoPath
  if ($LASTEXITCODE -ne 0) {
      Write-Log "BŁĄD: git add zakończony kodem $LASTEXITCODE"
      exit 1
  }

  git commit -m "chore: bump version to $newVersion"
  if ($LASTEXITCODE -ne 0) {
      Write-Log "BŁĄD: git commit zakończony kodem $LASTEXITCODE"
      exit 1
  }
  Write-Log "git commit: chore: bump version to $newVersion"
  ```

- [ ] **Step 3: Verify git operations work (dry run)**

  Run the script (at this stage it ends after the git commit — Task 4 sections don't exist yet):
  ```powershell
  .\publish.ps1
  ```

  Check the commit was created correctly:
  ```powershell
  git log --oneline -3
  # Expected top line: chore: bump version to 0.1.1

  git diff HEAD~1 HEAD -- src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml
  # Expected: only version lines changed in each file
  ```

  Undo the test commit and restore files to their original state:
  ```powershell
  git reset --hard HEAD~1
  # Restores all 3 version files to their pre-bump state
  ```

- [ ] **Step 4: Commit**

  ```powershell
  git add publish.ps1
  git commit -m "feat: add git status check and version commit to publish.ps1"
  ```

---

### Task 4: Build, git tag, backup, and deploy

**Files:**
- Modify: `publish.ps1`

**Interfaces:**
- Consumes: `$oldVersion`, `$newVersion`, `$DeployDir`, `$BackupDir`, `$ExeName`, `$BuildExe`, `Write-Log` from Tasks 1–3

- [ ] **Step 1: Add Tauri build section**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Build
  # ---------------------------------------------------------------------------
  Write-Log "npx tauri build — start (może potrwać kilka minut)"
  npx tauri build
  if ($LASTEXITCODE -ne 0) {
      Write-Log "BŁĄD: npx tauri build zakończony kodem $LASTEXITCODE"
      Write-Log "Wersja jest już zacommitowana jako $newVersion."
      Write-Log "Po naprawieniu błędu uruchom skrypt ponownie (wersja zostanie zbumpowana do kolejnego patcha)."
      Write-Log "Aby cofnąć bump wersji: git revert HEAD --no-edit"
      exit 1
  }
  Write-Log "npx tauri build — zakończony pomyślnie"
  ```

- [ ] **Step 2: Add git tag after successful build**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Git tag (after successful build only)
  # ---------------------------------------------------------------------------
  git tag "v$newVersion"
  if ($LASTEXITCODE -ne 0) {
      Write-Log "BŁĄD: git tag zakończony kodem $LASTEXITCODE (tag v$newVersion już istnieje?)"
      exit 1
  }
  Write-Log "git tag v$newVersion"
  ```

- [ ] **Step 3: Add backup section**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Backup previous .exe
  # ---------------------------------------------------------------------------
  $deployExe = "$DeployDir\$ExeName"
  if (Test-Path $deployExe) {
      $backupName = "smart-todo-$oldVersion.exe"
      $backupPath = "$BackupDir\$backupName"
      Copy-Item -Path $deployExe -Destination $backupPath -Force
      Write-Log "Backup: $ExeName → backups\$backupName"
  } else {
      Write-Log "Brak poprzedniego .exe — pomijam backup (pierwsze wdrożenie)"
  }
  ```

- [ ] **Step 4: Add deploy section with locked-file error handling**

  Append to `publish.ps1`:

  ```powershell
  # ---------------------------------------------------------------------------
  # Deploy
  # ---------------------------------------------------------------------------
  if (-not (Test-Path $BuildExe)) {
      Write-Log "BŁĄD: Nie znaleziono zbudowanego pliku: $BuildExe"
      exit 1
  }

  try {
      Copy-Item -Path $BuildExe -Destination $deployExe -Force
      Write-Log "Skopiowano: $ExeName → $DeployDir\"
  } catch {
      Write-Log "BŁĄD: Nie można skopiować $ExeName — czy aplikacja jest uruchomiona?"
      Write-Log "Szczegóły: $_"
      exit 1
  }

  Write-Log "=== Gotowe. Wersja $newVersion wdrożona. ==="
  ```

- [ ] **Step 5: Verify full script**

  Check that `publish.ps1` has sections in this order:
  1. `#Requires`, header comment, params, `$ErrorActionPreference`
  2. Config variables
  3. Directory creation
  4. Logging init + `Write-Log` function
  5. Git status check
  6. Version bump (read, compute, update 3 files)
  7. Git commit
  8. Build
  9. Git tag
  10. Backup
  11. Deploy

  Verify the final script file is correct end-to-end by reading it:
  ```powershell
  Get-Content publish.ps1
  ```

- [ ] **Step 6: Run full end-to-end publish**

  ```powershell
  .\publish.ps1
  ```

  Expected terminal output (abbreviated):
  ```
  [2026-07-03 14:32:05] === Smart Todo Publish Script ===
  [2026-07-03 14:32:05] Git status OK (czyste drzewo robocze)
  [2026-07-03 14:32:05] Wersja: 0.1.0 → 0.1.1
  [2026-07-03 14:32:06] Zaktualizowano tauri.conf.json
  [2026-07-03 14:32:06] Zaktualizowano package.json
  [2026-07-03 14:32:06] Zaktualizowano Cargo.toml
  [2026-07-03 14:32:07] git commit: chore: bump version to 0.1.1
  [2026-07-03 14:32:08] npx tauri build — start (może potrwać kilka minut)
  [2026-07-03 14:35:42] npx tauri build — zakończony pomyślnie
  [2026-07-03 14:35:42] git tag v0.1.1
  [2026-07-03 14:35:42] Brak poprzedniego .exe — pomijam backup (pierwsze wdrożenie)
  [2026-07-03 14:35:43] Skopiowano: smart-todo.exe → D:\Programs\smart-todo\
  [2026-07-03 14:35:43] === Gotowe. Wersja 0.1.1 wdrożona. ===
  ```

  Verify results:
  ```powershell
  # Deployed exe exists
  Test-Path "D:\Programs\smart-todo\smart-todo.exe"    # True

  # Git tag exists
  git tag --list "v0.1.1"                              # v0.1.1

  # Log file exists with all entries
  Get-Content (ls "D:\Programs\smart-todo\logs\*.log" | Select-Object -Last 1).FullName

  # On second run — backup should appear
  .\publish.ps1
  ls "D:\Programs\smart-todo\backups\"                 # smart-todo-0.1.1.exe
  ```

- [ ] **Step 7: Commit**

  ```powershell
  git add publish.ps1
  git commit -m "feat: add build, tag, backup, and deploy to publish.ps1"
  ```
