#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$cmakeCandidates = @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
)

foreach ($cmake in $cmakeCandidates) {
    if (Test-Path $cmake) {
        $cmakeDir = Split-Path $cmake -Parent
        if ($env:PATH -notlike "*$cmakeDir*") {
            $env:PATH = "$cmakeDir;$env:PATH"
        }
        break
    }
}

$env:RUSTFLAGS = "-Ctarget-feature=+crt-static"

$package = if ($args.Count -gt 0) { $args[0] } else { "sound2slide" }
Write-Host "Building package: $package"

cargo build --release --locked -p $package
if ($LASTEXITCODE -ne 0) {
    throw "Cargo build failed."
}

$dllName = $package -replace '-', '_'
$metadata = cargo metadata --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw "Cargo metadata failed."
}
$targetDir = ($metadata | ConvertFrom-Json).target_directory
$dllCandidates = @(
    (Join-Path $targetDir "x86_64-pc-windows-msvc\release\$dllName.dll"),
    (Join-Path $targetDir "release\$dllName.dll")
)
$dllPath = $dllCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $dllPath) {
    Write-Warning "Expected DLL was not found. Checked:`n$($dllCandidates -join "`n")"
    exit 1
}

$distDir = Join-Path $PSScriptRoot "..\dist"
New-Item -ItemType Directory -Path $distDir -Force | Out-Null

& (Join-Path $PSScriptRoot "generate-licenses.ps1") -OutputPath (Join-Path $distDir "third-party-library.txt")

$distDll = Join-Path $distDir (Split-Path $dllPath -Leaf)
Copy-Item -Path $dllPath -Destination $distDll -Force

$iniSource = Join-Path $PSScriptRoot "..\plugin\$dllName.ini"
if (Test-Path $iniSource) {
    $distIni = Join-Path $distDir "$dllName.ini"
    Copy-Item -Path $iniSource -Destination $distIni -Force
    Write-Host "Plugin INI: $distIni"
} else {
    Write-Warning "INI file was not found at $iniSource"
}

Write-Host "Plugin DLL: $distDll"