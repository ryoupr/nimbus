# EC2 Connect v3.0 (Rust) - Windows PowerShell Launcher Script

param(
    [Parameter(Position=0)]
    [string]$Command = "",
    
    [Parameter(Position=1, ValueFromRemainingArguments=$true)]
    [string[]]$Arguments = @()
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectName = "ec2-connect"

# Logging functions
function Write-Log {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] $Message" -ForegroundColor Blue
}

function Write-Error-Log {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Write-Warning-Log {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Success-Log {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

# Check if Rust is installed
function Test-Rust {
    try {
        $null = Get-Command cargo -ErrorAction Stop
        $rustVersion = (rustc --version).Split(' ')[1]
        Write-Log "Using Rust version: $rustVersion"
        return $true
    }
    catch {
        Write-Error-Log "Rust/Cargo is not installed. Please install Rust from https://rustup.rs/"
        return $false
    }
}

# Check if AWS CLI is installed
function Test-AwsCli {
    try {
        $null = Get-Command aws -ErrorAction Stop
        $awsVersion = (aws --version 2>&1).Split(' ')[0]
        Write-Log "Using $awsVersion"
    }
    catch {
        Write-Warning-Log "AWS CLI is not installed. Some features may not work properly."
        Write-Warning-Log "Install AWS CLI from: https://aws.amazon.com/cli/"
    }
}

# Check if Session Manager Plugin is installed
function Test-SessionManagerPlugin {
    try {
        $null = Get-Command session-manager-plugin -ErrorAction Stop
        Write-Log "Session Manager Plugin is available"
    }
    catch {
        Write-Warning-Log "AWS Session Manager Plugin is not installed."
        Write-Warning-Log "Install from: https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-install-plugin.html"
    }
}

# Build the project
function Build-Project {
    param([bool]$Release = $false)
    
    Write-Log "Building $ProjectName..."
    
    Push-Location $ScriptDir
    try {
        if ($Release) {
            Write-Log "Building in release mode..."
            cargo build --release
            Write-Success-Log "Release build completed"
        }
        else {
            Write-Log "Building in debug mode..."
            cargo build
            Write-Success-Log "Debug build completed"
        }
    }
    finally {
        Pop-Location
    }
}

# Run the project
function Start-Project {
    param(
        [bool]$Release = $false,
        [string[]]$Args = @()
    )
    
    Write-Log "Running $ProjectName..."
    
    Push-Location $ScriptDir
    try {
        if ($Release) {
            cargo run --release -- $Args
        }
        else {
            cargo run -- $Args
        }
    }
    finally {
        Pop-Location
    }
}

# Install the project
function Install-Project {
    Write-Log "Installing $ProjectName..."
    
    Push-Location $ScriptDir
    try {
        cargo install --path .
        Write-Success-Log "$ProjectName installed successfully"
        Write-Log "You can now run 'ec2-connect' from anywhere"
    }
    finally {
        Pop-Location
    }
}

# Run tests
function Invoke-Tests {
    Write-Log "Running tests for $ProjectName..."
    
    Push-Location $ScriptDir
    try {
        # Run unit tests
        Write-Log "Running unit tests..."
        cargo test
        
        # Run property-based tests if available
        $cargoToml = Get-Content "Cargo.toml" -Raw
        if ($cargoToml -match "proptest") {
            Write-Log "Running property-based tests..."
            cargo test --features proptest
        }
        
        Write-Success-Log "All tests completed"
    }
    finally {
        Pop-Location
    }
}

# Clean build artifacts
function Clear-Project {
    Write-Log "Cleaning build artifacts..."
    
    Push-Location $ScriptDir
    try {
        cargo clean
        Write-Success-Log "Clean completed"
    }
    finally {
        Pop-Location
    }
}

# Check dependencies
function Test-Dependencies {
    Write-Log "Checking dependencies..."
    
    $rustOk = Test-Rust
    if (-not $rustOk) {
        exit 1
    }
    
    Test-AwsCli
    Test-SessionManagerPlugin
    
    Write-Success-Log "Dependency check completed"
}

# Show help
function Show-Help {
    Write-Host "EC2 Connect v3.0 (Rust) - Windows PowerShell Launcher" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Usage: .\run.ps1 [COMMAND] [OPTIONS]" -ForegroundColor White
    Write-Host ""
    Write-Host "Commands:" -ForegroundColor Yellow
    Write-Host "  build [--release]     Build the project"
    Write-Host "  run [--release] [ARGS] Run the project with optional arguments"
    Write-Host "  install              Install the binary system-wide"
    Write-Host "  test                 Run all tests"
    Write-Host "  clean                Clean build artifacts"
    Write-Host "  check                Check dependencies"
    Write-Host "  help                 Show this help message"
    Write-Host ""
    Write-Host "Examples:" -ForegroundColor Green
    Write-Host "  .\run.ps1 build --release"
    Write-Host "  .\run.ps1 run connect --instance-id i-1234567890abcdef0"
    Write-Host "  .\run.ps1 run tui"
    Write-Host "  .\run.ps1 install"
    Write-Host ""
}

# Main execution logic
try {
    switch ($Command.ToLower()) {
        "build" {
            if (-not (Test-Rust)) { exit 1 }
            $release = $Arguments -contains "--release"
            Build-Project -Release $release
        }
        "run" {
            if (-not (Test-Rust)) { exit 1 }
            $release = $Arguments -contains "--release"
            $runArgs = $Arguments | Where-Object { $_ -ne "--release" }
            Start-Project -Release $release -Args $runArgs
        }
        "install" {
            if (-not (Test-Rust)) { exit 1 }
            Install-Project
        }
        "test" {
            if (-not (Test-Rust)) { exit 1 }
            Invoke-Tests
        }
        "clean" {
            Clear-Project
        }
        "check" {
            Test-Dependencies
        }
        "help" {
            Show-Help
        }
        "" {
            Write-Log "Starting $ProjectName with default settings..."
            if (-not (Test-Rust)) { exit 1 }
            Start-Project
        }
        default {
            Write-Error-Log "Unknown command: $Command"
            Write-Host ""
            Show-Help
            exit 1
        }
    }
}
catch {
    Write-Error-Log "An error occurred: $($_.Exception.Message)"
    exit 1
}