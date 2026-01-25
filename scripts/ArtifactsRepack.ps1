param (
    [Parameter(Mandatory=$false)]
    [string]$RootPath = "..\artifacts" # Defaults to current folder if no path is given
)

# 1. Resolve to a full path for reliability
$RootPath = Resolve-Path $RootPath
$stagingDir = "Extracted_Bundle_$(Get-Date -Format 'yyyyMMdd_HHmm')"

Write-Host "Searching for ZIPs in: $RootPath" -ForegroundColor Cyan
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

# 2. Find and Extract
Get-ChildItem -Path $RootPath -Filter *.zip -Recurse | ForEach-Object {
    $targetPath = Join-Path (Get-Location) "$stagingDir\$($_.BaseName)"
    Write-Host "Extracting: $($_.Name)"
    Expand-Archive -Path $_.FullName -DestinationPath $targetPath -Force
}

# 3. Create Master Zip
Write-Host "Creating rsdev.zip..." -ForegroundColor Green
Compress-Archive -Path "$stagingDir\*" -DestinationPath "rsdev.zip" -Force

Remove-Item -Recurse -Force $stagingDir
Write-Host "Done!" -ForegroundColor Green
