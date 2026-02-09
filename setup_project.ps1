# Define paths based on your specific workspace structure
$libRoot = "lib_common/src"
$serverRoot = "servers/src"

# 1. Define the internal library hierarchy
$dirs = @(
    "$libRoot/core",
    "$libRoot/ingestors"
)

# Create Directories
foreach ($dir in $dirs) {
    if (!(Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        Write-Host "Created library directory: $dir" -ForegroundColor Green
    }
}

# 2. Define Module Boilerplate for lib_common
$libFiles = @{
    # Core Engine Modules
    "$libRoot/core/mod.rs"          = "pub mod registry;`npub mod memory_guard;`npub mod dispatcher;"
    "$libRoot/core/registry.rs"      = "// Logic for Ref-Counting and Linger (CancellationToken)"
    "$libRoot/core/memory_guard.rs"  = "// Logic for GlobalMemoryGuard (AtomicU64)"
    "$libRoot/core/dispatcher.rs"    = "// Logic for Fan-out and Priority Eviction"
    
    # Ingestor/Plugin Modules
    "$libRoot/ingestors/mod.rs"         = "pub mod yahoo_wss;`npub mod cnn_polling;"
    "$libRoot/ingestors/yahoo_wss.rs"   = "// Yahoo Finance WSS Ingestor"
    "$libRoot/ingestors/cnn_polling.rs"  = "// CNN Polling Plugin (Self-Scheduling)"
}

# 3. Update lib_common/src/lib.rs
$libRsPath = "$libRoot/lib.rs"
$libRsContent = "pub mod core;`npub mod ingestors;"

# Function to write files without overwriting existing work
function Write-Boilerplate($fileMap) {
    foreach ($path in $fileMap.Keys) {
        if (!(Test-Path $path)) {
            Set-Content -Path $path -Value $fileMap[$path]
            Write-Host "Created file: $path" -ForegroundColor Cyan
        } else {
            Write-Host "Skipping: $path (already exists)" -ForegroundColor Gray
        }
    }
}

Write-Boilerplate $libFiles

# Update lib.rs if necessary
if (Test-Path $libRsPath) {
    $currentContent = Get-Content $libRsPath
    if ($currentContent -notmatch "mod core") {
        Add-Content -Path $libRsPath -Value "`npub mod core;`npub mod ingestors;"
        Write-Host "Updated lib.rs with new modules" -ForegroundColor Yellow
    }
} else {
    Set-Content -Path $libRsPath -Value $libRsContent
    Write-Host "Created lib.rs" -ForegroundColor Yellow
}

Write-Host "`nWorkspace Structure Updated for ReStream!" -ForegroundColor Magenta