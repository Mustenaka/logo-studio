$ErrorActionPreference = "Stop"

Write-Host "=== Logo Studio GPU Build Setup ===" -ForegroundColor Cyan
Write-Host "Searching for cl.exe (MSVC C++ compiler)..." -ForegroundColor Yellow

$clDir = $null

# --- 1. Try vswhere.exe ---
$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualCpp.Tools.HostX64.TargetX64 -property installationPath 2>$null
    if ($vsPath) {
        $msvcRoot = Join-Path $vsPath "VC\Tools\MSVC"
        if (Test-Path $msvcRoot) {
            $allCl = Get-ChildItem -Path $msvcRoot -Recurse -Filter "cl.exe" -ErrorAction SilentlyContinue
            $filtered = $allCl | Where-Object { $_.FullName -match "Hostx64\\x64" }
            $sorted = $filtered | Sort-Object LastWriteTime -Descending
            $first = $sorted | Select-Object -First 1
            if ($first) {
                $clDir = $first.DirectoryName
            }
        }
    }
}

# --- 2. Fallback: search common paths ---
if (-not $clDir) {
    $roots = @(
        "C:\Program Files\Microsoft Visual Studio",
        "C:\Program Files (x86)\Microsoft Visual Studio"
    )
    foreach ($root in $roots) {
        if (Test-Path $root) {
            $allCl = Get-ChildItem -Path $root -Recurse -Filter "cl.exe" -ErrorAction SilentlyContinue
            $filtered = $allCl | Where-Object { $_.FullName -match "Hostx64\\x64" }
            $sorted = $filtered | Sort-Object LastWriteTime -Descending
            $first = $sorted | Select-Object -First 1
            if ($first) {
                $clDir = $first.DirectoryName
                break
            }
        }
    }
}

if (-not $clDir) {
    Write-Host ""
    Write-Host "ERROR: cl.exe not found." -ForegroundColor Red
    Write-Host "Install Visual Studio 2019/2022 with C++ Desktop Development workload." -ForegroundColor Yellow
    Write-Host "Download: https://visualstudio.microsoft.com/downloads/" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

Write-Host "Found: $clDir" -ForegroundColor Green

# --- 3. Write NVCC_CCBIN to .cargo/config.toml ---
$configDir = Join-Path $PSScriptRoot ".cargo"
$configFile = Join-Path $configDir "config.toml"

if (-not (Test-Path $configDir)) {
    New-Item -ItemType Directory -Path $configDir | Out-Null
}

$escaped = $clDir.Replace("\", "\\")
$newLine = 'NVCC_CCBIN = "' + $escaped + '"'

if (Test-Path $configFile) {
    $raw = [System.IO.File]::ReadAllText($configFile, [System.Text.Encoding]::UTF8)
    if ($raw -match "NVCC_CCBIN") {
        $pattern = 'NVCC_CCBIN\s*=\s*"[^"]*"'
        $raw = [regex]::Replace($raw, $pattern, $newLine)
        [System.IO.File]::WriteAllText($configFile, $raw, [System.Text.Encoding]::UTF8)
        Write-Host "Updated NVCC_CCBIN in $configFile" -ForegroundColor Cyan
    } else {
        $raw = $raw + "`n[env]`n" + $newLine + "`n"
        [System.IO.File]::WriteAllText($configFile, $raw, [System.Text.Encoding]::UTF8)
        Write-Host "Appended NVCC_CCBIN to $configFile" -ForegroundColor Green
    }
} else {
    $content = "[env]`n" + $newLine + "`n"
    [System.IO.File]::WriteAllText($configFile, $content, [System.Text.Encoding]::UTF8)
    Write-Host "Created $configFile" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Green
Write-Host "Now run: npm run tauri:dev:gpu" -ForegroundColor White
Write-Host ""
