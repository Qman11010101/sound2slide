#Requires -Version 5.1
$ErrorActionPreference = "Stop"

function Invoke-GitHub {
    param([string[]]$CommandArguments)
    $result = & gh @CommandArguments
    if ($LASTEXITCODE -ne 0) {
        throw "GitHub CLI failed: $($CommandArguments[0])"
    }
    return $result
}

foreach ($name in @("GH_REPO", "RELEASE_SHA", "RELEASE_RUN_ID")) {
    if (-not [Environment]::GetEnvironmentVariable($name)) {
        throw "Required environment variable is missing: $name"
    }
}
$archivePath = Join-Path $PSScriptRoot "../dist/sound2slide-windows-x64.zip"
if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf)) {
    throw "Release archive is missing."
}

$run = (Invoke-GitHub -CommandArguments @("api", "repos/$env:GH_REPO/actions/runs/$env:RELEASE_RUN_ID")) | ConvertFrom-Json
$japanTimeZone = [TimeZoneInfo]::FindSystemTimeZoneById("Tokyo Standard Time")
# Use the original run time so retrying after midnight keeps the release date.
$date = [TimeZoneInfo]::ConvertTime([DateTimeOffset]::Parse($run.created_at), $japanTimeZone).ToString("yyyy-MM-dd")
$main = (Invoke-GitHub -CommandArguments @("api", "repos/$env:GH_REPO/git/ref/heads/main")) | ConvertFrom-Json
if ($main.object.sha -ne $env:RELEASE_SHA) {
    Write-Host "Skipping an outdated main build."
    return
}
$tagPattern = '^' + [regex]::Escape($date) + '(?:-([2-9]|[1-9][0-9]+))?$'
$pages = (Invoke-GitHub -CommandArguments @("api", "--paginate", "--slurp", "repos/$env:GH_REPO/releases?per_page=100")) | ConvertFrom-Json
$releases = @($pages | ForEach-Object { $_ } | ForEach-Object { $_ })
$existing = $releases | Where-Object { $_.tag_name -match $tagPattern } | Select-Object -First 1
$refs = (Invoke-GitHub -CommandArguments @("api", "repos/$env:GH_REPO/git/matching-refs/tags/$date")) | ConvertFrom-Json
$dayTags = @($refs | ForEach-Object {
    $name = $_.ref -replace '^refs/tags/', ''
    if ($name -match $tagPattern) {
        $number = if ($Matches[1]) { [int]$Matches[1] } else { 1 }
        [PSCustomObject]@{ Name = $name; Number = $number; Sha = $_.object.sha }
    }
} | Sort-Object Number -Descending)
$existingTag = $dayTags | Where-Object { $_.Name -eq $existing.tag_name } | Select-Object -First 1
if ($existing -and -not $existing.draft -and $existing.target_commitish -eq $env:RELEASE_SHA -and $existingTag.Sha -eq $env:RELEASE_SHA) {
    Write-Host "Release already published for this commit: $($existing.html_url)"
    return
}
# Reuse a tag allocated by an interrupted attempt for this exact commit.
$retryTag = $dayTags | Where-Object { $_.Sha -eq $env:RELEASE_SHA } | Select-Object -First 1
if ($retryTag) {
    $tag = $retryTag.Name
} else {
    $number = if ($dayTags.Count -gt 0) { $dayTags[0].Number + 1 } else { 1 }
    $tag = if ($number -eq 1) { $date } else { "$date-$number" }
    Invoke-GitHub -CommandArguments @("api", "--method", "POST", "repos/$env:GH_REPO/git/refs",
        "-f", "ref=refs/tags/$tag", "-f", "sha=$env:RELEASE_SHA") | Out-Null
}
# Keep a partially replaced release unpublished until its tag, asset, and notes agree.
if ($existing) {
    Invoke-GitHub -CommandArguments @("release", "edit", $existing.tag_name, "--repo", $env:GH_REPO,
        "--tag", $tag, "--target", $env:RELEASE_SHA, "--title", "sound2slide $tag", "--draft=true")
} else {
    Invoke-GitHub -CommandArguments @("release", "create", $tag, "--repo", $env:GH_REPO,
        "--target", $env:RELEASE_SHA, "--title", "sound2slide $tag", "--verify-tag", "--draft")
}
Invoke-GitHub -CommandArguments @("release", "upload", $tag, $archivePath, "--repo", $env:GH_REPO, "--clobber")
$notes = (Invoke-GitHub -CommandArguments @("api", "--method", "POST", "repos/$env:GH_REPO/releases/generate-notes",
    "-f", "tag_name=$tag", "-f", "target_commitish=$env:RELEASE_SHA")) | ConvertFrom-Json
$notesPath = [System.IO.Path]::GetTempFileName()
try {
    [System.IO.File]::WriteAllText($notesPath, $notes.body, (New-Object System.Text.UTF8Encoding($false)))
    Invoke-GitHub -CommandArguments @("release", "edit", $tag, "--repo", $env:GH_REPO,
        "--target", $env:RELEASE_SHA, "--title", "sound2slide $tag", "--notes-file", $notesPath,
        "--draft=false", "--latest")
} finally {
    Remove-Item -LiteralPath $notesPath -Force
}
Write-Host "Published sound2slide $tag"
