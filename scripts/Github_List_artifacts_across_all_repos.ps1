# List artifacts across all repos for user 'ckir'
$user = 'ckir'
$total = 0
$all = @()

# get repo list (may return many)
$reposJson = gh repo list $user --limit 1000 --json name
$repos = ($reposJson | ConvertFrom-Json) | ForEach-Object { $_.name }

foreach ($r in $repos) {
  Write-Host "Checking $r..."
  # get artifacts; returns [] if none
  $artJson = gh api repos/$user/$r/actions/artifacts --jq '.artifacts[]? | {repo: "'"$r"'", id, name, size_in_bytes}' 2>$null
  if ($artJson) {
    # artJson may be multiple JSON objects separated by newlines; convert them
    $artLines = $artJson -split "`n" | Where-Object { $_ -ne '' }
    foreach ($line in $artLines) {
      $obj = $line | ConvertFrom-Json
      $all += $obj
      $total += [int64]$obj.size_in_bytes
    }
  }
}

Write-Host "`nTop artifacts by size:"
$all | Sort-Object -Property size_in_bytes -Descending | Select-Object -First 30 | Format-Table -AutoSize
Write-Host "`nTotal artifact bytes across repos: $total"
