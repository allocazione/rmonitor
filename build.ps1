#!/usr/bin/env pwsh

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

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
    Write-Host "Build complete! (Duration: $($duration.Seconds)s) Executable located at: $OutputDir\rmonitor.exe" -ForegroundColor Green

    $run = Read-Host "Do you want to run the program now? (Y/N)"
    if ($run -eq "Y" -or $run -eq "y") {
        & "$OutputDir\rmonitor.exe"
    }

    $currentDir = Get-Location
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathElements = $userPath -split ";" | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }
    
    if ($pathElements -notcontains $currentDir.Path) {
        $addPath = Read-Host "Do you want to add this directory to your PATH? (Y/N)"
        if ($addPath -eq "Y" -or $addPath -eq "y") {
            [Environment]::SetEnvironmentVariable("Path", "$userPath;$currentDir", "User")
            $env:Path = "$env:Path;$currentDir"
            Write-Host "Directory added to PATH for current and future sessions." -ForegroundColor Green
        }
    }
} else {
    Write-Host "Could not find compiled executable at $ExePath" -ForegroundColor Red
    exit 1
}
