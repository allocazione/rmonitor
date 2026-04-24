# Build rmonitor and copy the executable to release\windows
$OutputDir = "release\windows"
$ExePath = "target\release\rmonitor.exe"

Write-Host -NoNewline "Building rmonitor... "
$StartTime = Get-Date

# Build the project in release mode
cargo build --release --quiet

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed."
    exit $LASTEXITCODE
}

# Ensure the output directory exists
if (!(Test-Path -Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

# Copy the executable to the output directory
if (Test-Path -Path $ExePath) {
    Copy-Item -Path $ExePath -Destination "$OutputDir\rmonitor.exe" -Force
    $Duration = [Math]::Round(((Get-Date) - $StartTime).TotalSeconds)
    Write-Host "Done! (${Duration}s)"
    Write-Host "Binary: $OutputDir\rmonitor.exe"
} else {
    Write-Host "Build failed: Could not find compiled executable."
    exit 1
}
