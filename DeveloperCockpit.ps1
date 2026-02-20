# // DeveloperCockpit.ps1
# // Purpose: Comprehensive Build Management for rsdev Workspace
# // Logic: Clean/Refresh Context -> Selective Cargo Builds (Feature-aware)

$ErrorActionPreference = "Stop"

# // Function: Displays a consistent visual header
function Show-Header {
    Clear-Host
    Write-Host "===============================================" -ForegroundColor Cyan
    Write-Host "         RSDEV BUILD COCKPIT                   " -ForegroundColor Cyan
    Write-Host "===============================================" -ForegroundColor Cyan
}

# // Function: Verifies that required CLI tools are available in the PATH
function Check-Prerequisites {
    Write-Host "[*] Checking prerequisites..." -ForegroundColor Gray
    $requiredTools = @("cargo", "dir-to-text")
    
    foreach ($tool in $requiredTools) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            Write-Host "Error: '$tool' is not installed or not in PATH." -ForegroundColor Red
            exit 1
        }
    }
    Write-Host "Prerequisites met.`n" -ForegroundColor Green
}

# // Function: Cleans workspace by removing context files and Rust target artifacts
function Clean-Workspace {
    Show-Header
    Write-Host "[!] Cleaning Workspace..." -ForegroundColor Yellow
    
    # // Statement: Remove context file
    Remove-Item "rsdev.txt" -ErrorAction SilentlyContinue
    
    # // Statement: Run cargo clean to wipe build artifacts
    cargo clean
    
    Write-Host "`nWorkspace is clean." -ForegroundColor Green
    Read-Host "Press Enter to return to menu"
}

# // Function: Regenerates the project context map using the external dir-to-text tool
function Refresh-Context {
    Write-Host "[1/2] Cleaning old context..." -ForegroundColor Gray
    Remove-Item "rsdev.txt" -ErrorAction SilentlyContinue

    Write-Host "[2/2] Running dir-to-text..." -ForegroundColor Gray
    # // Statement: Execute external tool with your specific exclusion rules
    & dir-to-text --use-gitignore -e "target" -e !pyTests -e "Cargo.lock" -e vendor -e .git .
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: dir-to-text execution failed." -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "Context refreshed successfully.`n" -ForegroundColor Green
}

# // Function: Orchestrates cargo builds with conditional feature flags
function Execute-Build {
    param (
        [string]$PackageName,
        [string]$BinaryName,
        [switch]$All
    )

    Refresh-Context

    if ($All) {
        Write-Host "Building entire workspace (lib_common with full features)..." -ForegroundColor Yellow
        cargo build --workspace --features full
    }
    elseif ($PackageName -eq "lib_common") {
        Write-Host "Building lib_common with --features full..." -ForegroundColor Yellow
        cargo build -p lib_common --features full
    }
    elseif ($BinaryName) {
        Write-Host "Building binary: $BinaryName (Package: $PackageName)..." -ForegroundColor Yellow
        # // Statement: All other binaries build without feature flags
        cargo build -p $PackageName --bin $BinaryName
    }
    else {
        Write-Host "Building package: $PackageName..." -ForegroundColor Yellow
        cargo build -p $PackageName
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nBUILD FAILED: Stopping." -ForegroundColor Red
        Read-Host "Press Enter to return to menu"
        return
    }

    Write-Host "`nBuild Successful." -ForegroundColor Green
    Read-Host "Press Enter to return to menu"
}

# // Main Execution Start
Check-Prerequisites

do {
    Show-Header
    Write-Host "Select Build Target:"
    Write-Host "0) BUILD WHOLE WORKSPACE (lib_common: full)" -ForegroundColor Magenta
    Write-Host "1) lib_common (full)     2) misc (All)       3) rs_cli (All)     4) servers (All)"
    Write-Host "-----------------------------------------------"
    Write-Host "   --- SERVERS ---"
    Write-Host "s1) server_speak    s2) server_log      s3) server_sql      s4) server_dummy"
    Write-Host "-----------------------------------------------"
    Write-Host "   --- MISC BINARIES ---"
    Write-Host "m1) audio_test      m2) monitor_net     m3) monitor_postgres m4) monitor_redis"
    Write-Host "-----------------------------------------------"
    Write-Host "   --- RS_CLI BINARIES ---"
    Write-Host "c1) dir-to-json     c2) dir-to-text     c3) dir-to-yaml     c4) j5-format"
    Write-Host "c5) j5-to-json      c6) j5-to-yaml      c7) js-paths        c8) local-deps-tree"
    Write-Host "c9) rs_encrypt      c10) zip"
    Write-Host "-----------------------------------------------"
    Write-Host "C) Clean Workspace   R) Refresh Context Only   Q) Quit"
    Write-Host "-----------------------------------------------"
    
    $choice = Read-Host "Choice"

    switch ($choice) {
        "0" { Execute-Build -All }
        "1" { Execute-Build -PackageName "lib_common" }
        "2" { Execute-Build -PackageName "misc" }
        "3" { Execute-Build -PackageName "rs_cli" }
        "4" { Execute-Build -PackageName "servers" }
        
        # // Servers Package
        "s1" { Execute-Build -PackageName "servers" -BinaryName "server_speak" }
        "s2" { Execute-Build -PackageName "servers" -BinaryName "server_log" }
        "s3" { Execute-Build -PackageName "servers" -BinaryName "server_sql" }
        "s4" { Execute-Build -PackageName "servers" -BinaryName "server_dummy" }

        # // Misc Package
        "m1" { Execute-Build -PackageName "misc" -BinaryName "audio_test" }
        "m2" { Execute-Build -PackageName "misc" -BinaryName "monitor_net" }
        "m3" { Execute-Build -PackageName "misc" -BinaryName "monitor_postgres" }
        "m4" { Execute-Build -PackageName "misc" -BinaryName "monitor_redis" }

        # // RS_CLI Package
        "c1" { Execute-Build -PackageName "rs_cli" -BinaryName "dir-to-json" }
        "c2" { Execute-Build -PackageName "rs_cli" -BinaryName "dir-to-text" }
        "c3" { Execute-Build -PackageName "rs_cli" -BinaryName "dir-to-yaml" }
        "c4" { Execute-Build -PackageName "rs_cli" -BinaryName "j5-format" }
        "c5" { Execute-Build -PackageName "rs_cli" -BinaryName "j5-to-json" }
        "c6" { Execute-Build -PackageName "rs_cli" -BinaryName "j5-to-yaml" }
        "c7" { Execute-Build -PackageName "rs_cli" -BinaryName "js-paths" }
        "c8" { Execute-Build -PackageName "rs_cli" -BinaryName "local-deps-tree" }
        "c9" { Execute-Build -PackageName "rs_cli" -BinaryName "rs_encrypt" }
        "c10" { Execute-Build -PackageName "rs_cli" -BinaryName "zip" }

        "C" { Clean-Workspace }
        "R" { Show-Header; Refresh-Context; Read-Host "Done. Press Enter." }
        "Q" { break }
        default { Write-Host "Invalid selection." -ForegroundColor Red; Start-Sleep -Seconds 1 }
    }
} while ($choice -ne "Q")