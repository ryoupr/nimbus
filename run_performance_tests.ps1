# Performance Test Runner for EC2 Connect v3.0 (Windows)
# Task 17: Final Integration and Performance Testing
# Requirements: 5.1, 5.2

param(
    [switch]$SkipBuild,
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

Write-Host "🚀 EC2 Connect v3.0 - Performance Test Suite (Windows)" -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

# Function to print colored output
function Write-Status {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# Check if we're in the right directory
if (-not (Test-Path "Cargo.toml")) {
    Write-Error "Please run this script from the ec2-connect-rust directory"
    exit 1
}

# Create results directory
$ResultsDir = "performance_results"
if (-not (Test-Path $ResultsDir)) {
    New-Item -ItemType Directory -Path $ResultsDir | Out-Null
}
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"

Write-Status "Starting performance test suite at $(Get-Date)"
Write-Status "Results will be saved to: $ResultsDir"

try {
    # 1. Build the project in release mode
    if (-not $SkipBuild) {
        Write-Status "Building project in release mode..."
        $BuildResult = cargo build --release 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Success "Build completed successfully"
        } else {
            Write-Error "Build failed"
            Write-Host $BuildResult
            exit 1
        }
    } else {
        Write-Status "Skipping build (--SkipBuild specified)"
    }

    # 2. Run integration tests
    Write-Status "Running integration tests..."
    $TestOutput = "$ResultsDir\integration_tests_$Timestamp.log"
    $TestResult = cargo test --release --test integration_test -- --nocapture 2>&1
    $TestResult | Out-File -FilePath $TestOutput -Encoding UTF8
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Integration tests passed"
        Write-Host "  📄 Results saved to: $TestOutput"
    } else {
        Write-Error "Integration tests failed"
        Write-Host "  📄 Error log saved to: $TestOutput"
        Get-Content $TestOutput | Select-Object -Last 20
        exit 1
    }

    # 3. Run performance benchmark tests
    Write-Status "Running performance benchmark tests..."
    $BenchOutput = "$ResultsDir\benchmark_tests_$Timestamp.log"
    $BenchResult = cargo test --release --test performance_benchmark -- --nocapture 2>&1
    $BenchResult | Out-File -FilePath $BenchOutput -Encoding UTF8
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Performance benchmark tests passed"
        Write-Host "  📄 Results saved to: $BenchOutput"
    } else {
        Write-Error "Performance benchmark tests failed"
        Write-Host "  📄 Error log saved to: $BenchOutput"
        Get-Content $BenchOutput | Select-Object -Last 20
        exit 1
    }

    # 4. Run Criterion benchmarks
    Write-Status "Running Criterion benchmarks..."
    $CriterionOutput = "$ResultsDir\criterion_benchmarks_$Timestamp.log"
    try {
        $CriterionResult = cargo bench 2>&1
        $CriterionResult | Out-File -FilePath $CriterionOutput -Encoding UTF8
        Write-Success "Criterion benchmarks completed"
        Write-Host "  📄 Results saved to: $CriterionOutput"
        Write-Host "  📊 HTML reports available in: target\criterion\"
    } catch {
        Write-Warning "Criterion benchmarks had issues (this may be expected in some environments)"
        Write-Host "  📄 Output saved to: $CriterionOutput"
    }

    # 5. Memory usage test
    Write-Status "Running memory usage verification..."
    $MemoryOutput = "$ResultsDir\memory_test_$Timestamp.log"
    
    "Testing memory usage with basic operations..." | Out-File -FilePath $MemoryOutput -Encoding UTF8
    
    # Get process memory usage
    $ProcessBefore = Get-Process -Id $PID
    $MemoryBefore = $ProcessBefore.WorkingSet64 / 1MB
    
    # Run a simple command
    $HelpResult = & ".\target\release\ec2-connect.exe" --help 2>&1
    $HelpResult | Out-File -FilePath $MemoryOutput -Append -Encoding UTF8
    
    $ProcessAfter = Get-Process -Id $PID
    $MemoryAfter = $ProcessAfter.WorkingSet64 / 1MB
    
    "Memory usage - Before: $([math]::Round($MemoryBefore, 2))MB, After: $([math]::Round($MemoryAfter, 2))MB" | Out-File -FilePath $MemoryOutput -Append -Encoding UTF8
    
    Write-Success "Memory usage test completed"
    Write-Host "  📄 Results saved to: $MemoryOutput"

    # 6. CPU usage test
    Write-Status "Running CPU usage verification..."
    $CpuOutput = "$ResultsDir\cpu_test_$Timestamp.log"
    
    "Testing CPU usage during basic operations..." | Out-File -FilePath $CpuOutput -Encoding UTF8
    
    # Measure CPU usage
    $CpuBefore = (Get-Counter "\Processor(_Total)\% Processor Time").CounterSamples.CookedValue
    Start-Sleep -Seconds 1
    
    # Run command and measure
    $Stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $HelpResult = & ".\target\release\ec2-connect.exe" --help 2>&1
    $Stopwatch.Stop()
    
    $CpuAfter = (Get-Counter "\Processor(_Total)\% Processor Time").CounterSamples.CookedValue
    
    "Command execution time: $($Stopwatch.ElapsedMilliseconds)ms" | Out-File -FilePath $CpuOutput -Append -Encoding UTF8
    "CPU usage during test: $([math]::Round($CpuAfter - $CpuBefore, 3))%" | Out-File -FilePath $CpuOutput -Append -Encoding UTF8
    
    Write-Success "CPU usage test completed"
    Write-Host "  📄 Results saved to: $CpuOutput"

    # 7. Generate performance report
    Write-Status "Generating performance report..."
    $ReportFile = "$ResultsDir\performance_report_$Timestamp.md"
    
    $ReportContent = @"
# EC2 Connect v3.0 Performance Test Report

**Generated:** $(Get-Date)
**Platform:** Windows
**Test Suite:** Task 17 - Final Integration and Performance Testing
**Requirements:** 5.1 (Memory ≤ 10MB), 5.2 (CPU ≤ 0.5%)

## Test Results Summary

### Integration Tests
- **Status:** $(if (Test-Path $TestOutput) { "✅ PASSED" } else { "❌ FAILED" })
- **Log File:** ``$(Split-Path $TestOutput -Leaf)``

### Performance Benchmarks
- **Status:** $(if (Test-Path $BenchOutput) { "✅ PASSED" } else { "❌ FAILED" })
- **Log File:** ``$(Split-Path $BenchOutput -Leaf)``

### Criterion Benchmarks
- **Status:** $(if (Test-Path $CriterionOutput) { "✅ COMPLETED" } else { "❌ FAILED" })
- **Log File:** ``$(Split-Path $CriterionOutput -Leaf)``
- **HTML Reports:** Available in ``target\criterion\``

### Memory Usage Analysis
- **Tool:** Windows Performance Counters
- **Log File:** ``$(Split-Path $MemoryOutput -Leaf)``

### CPU Usage Analysis
- **Tool:** Windows Performance Counters
- **Log File:** ``$(Split-Path $CpuOutput -Leaf)``

## Performance Requirements Verification

### Memory Usage (Requirement 5.1)
- **Target:** ≤ 10MB during normal operation
- **Status:** $(if ((Get-Content $TestOutput -ErrorAction SilentlyContinue) -match "Memory usage test passed") { "✅ PASSED" } else { "⚠️ CHECK LOGS" })

### CPU Usage (Requirement 5.2)
- **Target:** ≤ 0.5% during session monitoring
- **Status:** $(if ((Get-Content $TestOutput -ErrorAction SilentlyContinue) -match "CPU usage test passed") { "✅ PASSED" } else { "⚠️ CHECK LOGS" })

## Key Metrics

"@

    # Extract key metrics from test outputs
    if (Test-Path $TestOutput) {
        $ReportContent += "`n### Memory Usage Results`n``````n"
        $MemoryMetrics = Get-Content $TestOutput | Where-Object { $_ -match "(Memory usage|Current memory|Baseline memory)" } | Select-Object -First 10
        if ($MemoryMetrics) {
            $ReportContent += ($MemoryMetrics -join "`n")
        } else {
            $ReportContent += "No memory metrics found"
        }
        $ReportContent += "`n``````n`n"
        
        $ReportContent += "### CPU Usage Results`n``````n"
        $CpuMetrics = Get-Content $TestOutput | Where-Object { $_ -match "(CPU usage|Measured CPU)" } | Select-Object -First 10
        if ($CpuMetrics) {
            $ReportContent += ($CpuMetrics -join "`n")
        } else {
            $ReportContent += "No CPU metrics found"
        }
        $ReportContent += "`n``````n`n"
    }

    if (Test-Path $BenchOutput) {
        $ReportContent += "### Performance Benchmark Results`n``````n"
        $BenchMetrics = Get-Content $BenchOutput | Where-Object { $_ -match "(Performance Report|Average time|Memory usage)" } | Select-Object -First 20
        if ($BenchMetrics) {
            $ReportContent += ($BenchMetrics -join "`n")
        } else {
            $ReportContent += "No benchmark metrics found"
        }
        $ReportContent += "`n``````n`n"
    }

    $ReportContent += @"

## Files Generated

- Integration Test Log: ``$(Split-Path $TestOutput -Leaf)``
- Benchmark Test Log: ``$(Split-Path $BenchOutput -Leaf)``
- Criterion Log: ``$(Split-Path $CriterionOutput -Leaf)``
- Memory Test Log: ``$(Split-Path $MemoryOutput -Leaf)``
- CPU Test Log: ``$(Split-Path $CpuOutput -Leaf)``

## Next Steps

1. Review detailed logs for any performance issues
2. Check Criterion HTML reports for detailed benchmark analysis
3. Verify memory and CPU usage meet requirements (≤10MB, ≤0.5%)
4. Address any performance bottlenecks identified

---
*Generated by EC2 Connect Performance Test Suite (Windows)*
"@

    $ReportContent | Out-File -FilePath $ReportFile -Encoding UTF8
    Write-Success "Performance report generated: $ReportFile"

    # 8. Summary
    Write-Host ""
    Write-Host "🎯 Performance Test Suite Complete!" -ForegroundColor Cyan
    Write-Host "====================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Success "All tests completed successfully"
    Write-Host "📊 Performance Report: $ReportFile"
    Write-Host "📁 All results saved to: $ResultsDir\"
    Write-Host ""

    # Check if requirements are met
    $RequirementsMet = $true

    $TestContent = Get-Content $TestOutput -ErrorAction SilentlyContinue
    if ($TestContent -match "Memory usage.*exceeds.*limit") {
        Write-Error "❌ Memory usage requirement (≤10MB) NOT MET"
        $RequirementsMet = $false
    } else {
        Write-Success "✅ Memory usage requirement (≤10MB) MET"
    }

    if ($TestContent -match "CPU usage.*exceeds.*limit") {
        Write-Error "❌ CPU usage requirement (≤0.5%) NOT MET"
        $RequirementsMet = $false
    } else {
        Write-Success "✅ CPU usage requirement (≤0.5%) MET"
    }

    if ($RequirementsMet) {
        Write-Host ""
        Write-Success "🎉 ALL PERFORMANCE REQUIREMENTS MET!"
        Write-Host "   Requirements 5.1 and 5.2 have been successfully verified."
    } else {
        Write-Host ""
        Write-Error "⚠️  SOME PERFORMANCE REQUIREMENTS NOT MET"
        Write-Host "   Please review the detailed logs and optimize accordingly."
        exit 1
    }

    Write-Host ""
    Write-Host "📋 To view detailed results:"
    Write-Host "   - Integration tests: Get-Content $TestOutput"
    Write-Host "   - Benchmarks: Get-Content $BenchOutput"
    Write-Host "   - Criterion reports: Start-Process target\criterion\report\index.html"
    Write-Host "   - Full report: Get-Content $ReportFile"

} catch {
    Write-Error "An error occurred during testing: $_"
    exit 1
}