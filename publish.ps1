#Requires -Version 7.0

<#
.SYNOPSIS
    Publish Smart Todo to the configured deploy directory.

.DESCRIPTION
    Auto-bumps patch version, builds with Tauri, backs up previous binary,
    deploys new binary to the path defined in publish.config.ps1.

    Supports Windows (.exe), macOS (.app bundle), and Linux (no extension).

    FIRST TIME SETUP:
    Copy publish.config.example.ps1 to publish.config.ps1 and set $DeployDir.

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
# Platform detection
# ---------------------------------------------------------------------------
if ($IsWindows) {
    $ExeName   = "smart-todo.exe"
    $BuildExe  = Join-Path "src-tauri" "target" "release" $ExeName
    $Sep       = "\"
} elseif ($IsMacOS) {
    $ExeName   = "smart-todo.app"
    $BuildExe  = Join-Path "src-tauri" "target" "release" "bundle" "macos" $ExeName
    $Sep       = "/"
} else {
    # Linux
    $ExeName   = "smart-todo"
    $BuildExe  = Join-Path "src-tauri" "target" "release" $ExeName
    $Sep       = "/"
}

# ---------------------------------------------------------------------------
# Config — loaded from publish.config.ps1 (gitignored, copy from example)
# ---------------------------------------------------------------------------
$ConfigFile = Join-Path $PSScriptRoot "publish.config.ps1"
if (-not (Test-Path $ConfigFile)) {
    Write-Error "Missing publish.config.ps1 — copy publish.config.example.ps1 to publish.config.ps1 and set your deploy path."
    exit 1
}
. $ConfigFile

$BackupDir = Join-Path $DeployDir "backups"
$LogDir    = Join-Path $DeployDir "logs"

# ---------------------------------------------------------------------------
# Directories
# ---------------------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $DeployDir | Out-Null
New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
New-Item -ItemType Directory -Force -Path $LogDir    | Out-Null

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
$LogTimestamp = Get-Date -Format "yyyy-MM-dd_HH-mm-ss"
$LogFile      = Join-Path $LogDir "publish-$LogTimestamp.log"

function Write-Log {
    param([string]$Message)
    $ts   = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "[$ts] $Message"
    Write-Host $line
    Add-Content -Path $LogFile -Value $line -Encoding UTF8
}

Write-Log "=== Smart Todo Publish Script ==="
Write-Log "Platform: $( if ($IsWindows) { 'Windows' } elseif ($IsMacOS) { 'macOS' } else { 'Linux' } )"

# ---------------------------------------------------------------------------
# Git status check
# ---------------------------------------------------------------------------
Write-Log "Sprawdzanie git status..."
$gitStatus = git status --porcelain
if ($LASTEXITCODE -ne 0) {
    Write-Log "BLAD: nie mozna sprawdzic git status (kod $LASTEXITCODE)"
    exit 1
}
if ($gitStatus) {
    Write-Log "UWAGA: Niezacommitowane zmiany:"
    $gitStatus | ForEach-Object { Write-Log "  $_" }
    $answer = Read-Host "Kontynuowac mimo niezacommitowanych zmian? [t/N]"
    if ($answer -ne "t" -and $answer -ne "T") {
        Write-Log "Przerwano przez uzytkownika."
        exit 0
    }
} else {
    Write-Log "Git status OK (czyste drzewo robocze)"
}

# ---------------------------------------------------------------------------
# Version bump
# ---------------------------------------------------------------------------
$tauriConfPath = Join-Path "src-tauri" "tauri.conf.json"
$packagePath   = "package.json"
$cargoPath     = Join-Path "src-tauri" "Cargo.toml"

$tauriConf  = Get-Content $tauriConfPath -Raw | ConvertFrom-Json
$oldVersion = $tauriConf.version

$parts      = $oldVersion -split "\."
$newVersion = "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"

Write-Log "Wersja: $oldVersion -> $newVersion"

# UTF-8 without BOM — Encoding.UTF8 adds BOM which breaks JSON parsers
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

# tauri.conf.json — replace "version": "X.Y.Z"
$content = Get-Content $tauriConfPath -Raw
$content = $content -replace '"version":\s*"[\d\.]+"', "`"version`": `"$newVersion`""
[System.IO.File]::WriteAllText(
    (Resolve-Path $tauriConfPath).Path,
    $content,
    $utf8NoBom
)
Write-Log "Zaktualizowano tauri.conf.json"

# package.json — replace "version": "X.Y.Z"
$content = Get-Content $packagePath -Raw
$content = $content -replace '"version":\s*"[\d\.]+"', "`"version`": `"$newVersion`""
[System.IO.File]::WriteAllText(
    (Resolve-Path $packagePath).Path,
    $content,
    $utf8NoBom
)
Write-Log "Zaktualizowano package.json"

# Cargo.toml — replace first ^version = "X.Y.Z" line (package version, not dep versions)
$content = Get-Content $cargoPath -Raw
$content = $content -replace '(?m)^version = "[\d\.]+"', "version = `"$newVersion`""
[System.IO.File]::WriteAllText(
    (Resolve-Path $cargoPath).Path,
    $content,
    $utf8NoBom
)
Write-Log "Zaktualizowano Cargo.toml"

# ---------------------------------------------------------------------------
# Git commit (version bump)
# ---------------------------------------------------------------------------
git add $tauriConfPath $packagePath $cargoPath
if ($LASTEXITCODE -ne 0) {
    Write-Log "BLAD: git add zakonczony kodem $LASTEXITCODE"
    exit 1
}

git commit -m "chore: bump version to $newVersion"
if ($LASTEXITCODE -ne 0) {
    Write-Log "BLAD: git commit zakonczony kodem $LASTEXITCODE"
    exit 1
}
Write-Log "git commit: chore: bump version to $newVersion"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
Write-Log "npx tauri build — start (moze potrwac kilka minut)"
npx tauri build
if ($LASTEXITCODE -ne 0) {
    Write-Log "BLAD: npx tauri build zakonczony kodem $LASTEXITCODE"
    Write-Log "Wersja jest juz zacommitowana jako $newVersion."
    Write-Log "Po naprawieniu bledu uruchom skrypt ponownie (wersja zostanie zbumpowana do kolejnego patcha)."
    Write-Log "Aby cofnac bump wersji: git revert HEAD --no-edit"
    exit 1
}
Write-Log "npx tauri build — zakonczony pomyslnie"

# ---------------------------------------------------------------------------
# Git tag (after successful build only)
# ---------------------------------------------------------------------------
git tag "v$newVersion"
if ($LASTEXITCODE -ne 0) {
    Write-Log "BLAD: git tag zakonczony kodem $LASTEXITCODE (tag v$newVersion juz istnieje?)"
    exit 1
}
Write-Log "git tag v$newVersion"

# ---------------------------------------------------------------------------
# Backup previous binary
# ---------------------------------------------------------------------------
$deployTarget = Join-Path $DeployDir $ExeName
if (Test-Path $deployTarget) {
    $backupName = if ($IsMacOS) {
        "smart-todo-$oldVersion.app"
    } elseif ($IsWindows) {
        "smart-todo-$oldVersion.exe"
    } else {
        "smart-todo-$oldVersion"
    }
    $backupPath = Join-Path $BackupDir $backupName
    if ($IsMacOS) {
        # .app is a directory bundle — use recursive copy
        Copy-Item -Path $deployTarget -Destination $backupPath -Recurse -Force
    } else {
        Copy-Item -Path $deployTarget -Destination $backupPath -Force
    }
    Write-Log "Backup: $ExeName -> backups${Sep}$backupName"
} else {
    Write-Log "Brak poprzedniego pliku — pomijam backup (pierwsze wdrozenie)"
}

# ---------------------------------------------------------------------------
# Deploy
# ---------------------------------------------------------------------------
if (-not (Test-Path $BuildExe)) {
    Write-Log "BLAD: Nie znaleziono zbudowanego pliku: $BuildExe"
    exit 1
}

try {
    if ($IsMacOS) {
        # .app is a directory bundle — remove old and copy recursively
        if (Test-Path $deployTarget) {
            Remove-Item -Path $deployTarget -Recurse -Force
        }
        Copy-Item -Path $BuildExe -Destination $deployTarget -Recurse -Force
    } else {
        Copy-Item -Path $BuildExe -Destination $deployTarget -Force
    }
    Write-Log "Skopiowano: $ExeName -> $DeployDir${Sep}"
} catch {
    Write-Log "BLAD: Nie mozna skopiowac $ExeName — czy aplikacja jest uruchomiona?"
    Write-Log "Szczegoly: $_"
    exit 1
}

Write-Log "=== Gotowe. Wersja $newVersion wdrozona. ==="
