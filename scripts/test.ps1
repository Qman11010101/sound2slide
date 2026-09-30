#Requires -Version 5.1
param([switch]$Release)
$ErrorActionPreference = "Stop"

$cmakeCandidates = @(
    "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe",
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
)
foreach ($cmake in $cmakeCandidates) {
    if (Test-Path -LiteralPath $cmake) {
        $cmakeDirectory = Split-Path $cmake -Parent
        if ($env:PATH -notlike "*$cmakeDirectory*") {
            $env:PATH = "$cmakeDirectory;$env:PATH"
        }
        break
    }
}
$env:RUSTFLAGS = "-Ctarget-feature=+crt-static"
$cargoArguments = @("test", "--workspace", "--locked")
if ($Release) {
    $cargoArguments += "--release"
}
cargo @cargoArguments
if ($LASTEXITCODE -ne 0) {
    throw "Cargo tests failed."
}
