# ================================
# CONFIGURATION
# ================================
$Owner = "ckir"
$Repo  = "rsdev"

# IMPORTANT: Set your GitHub PAT in an environment variable first:
#   $env:GITHUB_TOKEN = "ghp_XXXXXXXXXXXXXXXXXXXX"
$Token = $env:GITHUB_TOKEN
if (-not $Token) {
    try {
        $Token = gh auth token
    } catch {
        Write-Error "No GITHUB_TOKEN and unable to get token from gh CLI."
        exit 1
    }
}

if (-not $Token) {
    Write-Error "GITHUB_TOKEN environment variable not set."
    exit 1
}

$Headers = @{
    Authorization = "Bearer $Token"
    Accept        = "application/vnd.github+json"
}

# ================================
# 1. LIST ARTIFACTS
# ================================
Write-Host "Fetching artifacts from $Owner/$Repo ..." -ForegroundColor Cyan

$ArtifactsUrl = "https://api.github.com/repos/$Owner/$Repo/actions/artifacts?per_page=100"
$Artifacts = Invoke-RestMethod -Uri $ArtifactsUrl -Headers $Headers -Method GET

if ($Artifacts.total_count -eq 0) {
    Write-Host "No artifacts found." -ForegroundColor Yellow
    exit 0
}

Write-Host "`nArtifacts found:" -ForegroundColor Green
$Artifacts.artifacts | Select-Object id, name, size_in_bytes, expired, created_at | Format-Table

# ================================
# 2. CONFIRMATION
# ================================
$Confirm = Read-Host "`nDelete ALL artifacts? (y/N)"

if ($Confirm -ne "y") {
    Write-Host "Aborted. No artifacts deleted." -ForegroundColor Yellow
    exit 0
}

# ================================
# 3. DELETE ARTIFACTS
# ================================
foreach ($A in $Artifacts.artifacts) {
    $DeleteUrl = "https://api.github.com/repos/$Owner/$Repo/actions/artifacts/$($A.id)"
    Write-Host "Deleting artifact $($A.name) (ID: $($A.id)) ..." -ForegroundColor Red
    Invoke-RestMethod -Uri $DeleteUrl -Headers $Headers -Method DELETE
}

Write-Host "`nAll artifacts deleted successfully." -ForegroundColor Green
