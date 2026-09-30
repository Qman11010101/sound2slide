#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$distDirectory = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../dist"))
$fileNames = @("sound2slide.dll", "sound2slide.ini", "third-party-library.txt")
$files = @($fileNames | ForEach-Object {
    $path = Join-Path $distDirectory $_
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Release file not found: $path"
    }
    if ((Get-Item -LiteralPath $path).Length -eq 0) {
        throw "Release file is empty: $path"
    }
    $path
})
$archivePath = Join-Path $distDirectory "sound2slide-windows-x64.zip"
Compress-Archive -LiteralPath $files -DestinationPath $archivePath -CompressionLevel Optimal -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($archivePath)
try {
    $entries = @($archive.Entries | ForEach-Object { $_.FullName } | Sort-Object)
    if (@(Compare-Object ($fileNames | Sort-Object) $entries).Count -ne 0) {
        throw "The release archive must contain exactly the three release files."
    }
} finally {
    $archive.Dispose()
}
Write-Host "Release archive: $archivePath"
