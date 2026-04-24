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

    # Add the output directory to User PATH if not already present
    $FullPath = (Resolve-Path $OutputDir).Path
    $CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    
    if ($CurrentPath -notmatch [regex]::Escape($FullPath)) {
        Write-Host "Adding $FullPath to User PATH..."
        [Environment]::SetEnvironmentVariable("Path", "$CurrentPath;$FullPath", "User")
        Write-Host "Note: You may need to restart your terminal for the PATH changes to take effect."
    }
} else {
    Write-Host "Build failed: Could not find compiled executable."
    exit 1
}
