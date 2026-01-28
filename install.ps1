$ErrorActionPreference = "Stop"

$Repo = "ryoupr/nimbus"
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:LOCALAPPDATA\nimbus" }

# Get latest version
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
$Version = $Release.tag_name

Write-Host "Installing nimbus $Version..."

# Download
$Filename = "nimbus-windows-x86_64.zip"
$Url = "https://github.com/$Repo/releases/download/$Version/$Filename"

$TmpDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }

Invoke-WebRequest -Uri $Url -OutFile "$TmpDir\$Filename"
Invoke-WebRequest -Uri "$Url.sha256" -OutFile "$TmpDir\$Filename.sha256"

# Verify checksum
$Expected = (Get-Content "$TmpDir\$Filename.sha256").Split(" ")[0]
$Actual = (Get-FileHash "$TmpDir\$Filename" -Algorithm SHA256).Hash.ToLower()
if ($Expected -ne $Actual) {
    throw "Checksum mismatch"
}

# Extract and install
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
Expand-Archive -Path "$TmpDir\$Filename" -DestinationPath $InstallDir -Force

# Add to PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "Added $InstallDir to PATH (restart terminal to apply)"
}

Remove-Item -Recurse -Force $TmpDir

Write-Host "`n✓ nimbus installed to $InstallDir\nimbus.exe"
Write-Host "Run 'nimbus --help' to get started"
