# Retrieve all environment variables and sort them
$envVars = Get-ChildItem Env: | Sort-Object Name

# --- Output in .env format ---
Write-Host "`n=== .ENV FORMAT ===" -ForegroundColor Cyan
foreach ($var in $envVars) {
    Write-Output "$($var.Name)=$($var.Value)"
}

# --- Output in JSON format ---
Write-Host "`n=== JSON FORMAT ===" -ForegroundColor Cyan
$jsonHash = @{}
foreach ($var in $envVars) {
    $jsonHash[$var.Name] = $var.Value
}

# Convert the hashtable to a JSON string and output
$jsonHash | ConvertTo-Json
