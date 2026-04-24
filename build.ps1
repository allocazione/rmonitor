#!/usr/bin/env pwsh

# Build rmonitor and copy the executable into release/windows
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

Write-Host "╔══════════════════════════════════════════════════════╗" -ForegroundColor Blue
Write-Host "║             rmonitor Build System                    ║" -ForegroundColor Blue
Write-Host "╚══════════════════════════════════════════════════════╝" -ForegroundColor Blue
Write-Host ""

# Dependency Checks
Write-Host "Checking dependencies..." -ForegroundColor Cyan
$missingDeps = @()

if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    $missingDeps += "Rust/Cargo"
}

if ($missingDeps.Count -gt 0) {
    Write-Host "Error: Missing dependencies: $($missingDeps -join ', ')" -ForegroundColor Red
    Write-Host "Please refer to the Prerequisites section in README.md for installation instructions." -ForegroundColor Yellow
    exit 1
}

$startTime = Get-Date
Write-Host "Building rmonitor (Release)..." -ForegroundColor Cyan
cargo build --release --quiet

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit $LASTEXITCODE
}

$OutputDir = "release\windows"
if (!(Test-Path -Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$ExePath = "target\release\rmonitor.exe"
if (Test-Path -Path $ExePath) {
    Copy-Item -Path $ExePath -Destination "$OutputDir\rmonitor.exe" -Force
    $endTime = Get-Date
    $duration = $endTime - $startTime
    Write-Host ""
    Write-Host "Build complete! " -NoNewline -ForegroundColor Green
    Write-Host "(Duration: $($duration.Seconds)s)" -ForegroundColor White
    Write-Host "Executable located at: " -NoNewline -ForegroundColor Green
    Write-Host "$OutputDir\rmonitor.exe" -ForegroundColor Gray
    Write-Host ""

    $releasePath = Join-Path $ScriptDir $OutputDir
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathElements = $userPath -split ";" | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
    
    if ($pathElements -notcontains $releasePath) {
        Write-Host "To run 'rmonitor' from anywhere, you can add the release directory to your PATH." -ForegroundColor Yellow
        $addPath = Read-Host "Do you want to add $releasePath to your PATH? (Y/N)"
        if ($addPath -eq "Y" -or $addPath -eq "y") {
            [Environment]::SetEnvironmentVariable("Path", "$userPath;$releasePath", "User")
            $env:Path = "$env:Path;$releasePath"
            Write-Host "Directory added to PATH for current and future sessions." -ForegroundColor Green
            Write-Host "You can now type 'rmonitor' from any new terminal window." -ForegroundColor Yellow
        }
        Write-Host ""
    }

    $run = Read-Host "Do you want to run the program now? (Y/N)"
    if ($run -eq "Y" -or $run -eq "y") {
        & "$OutputDir\rmonitor.exe"
    }
} else {
    Write-Host "Could not find compiled executable at $ExePath" -ForegroundColor Red
    exit 1
}
