#Requires -Version 5.1
param(
    [string]$OutputPath = (Join-Path $PSScriptRoot "../dist/third-party-library.txt")
)
$ErrorActionPreference = "Stop"
$projectRoot = Split-Path $PSScriptRoot -Parent
$encoding = New-Object System.Text.UTF8Encoding($false)

Push-Location $projectRoot
try {
    if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
        cargo install cargo-about --version 0.9.2 --locked --features cli
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to install cargo-about."
        }
    }

    $OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
    New-Item -ItemType Directory -Path (Split-Path $OutputPath -Parent) -Force | Out-Null
    $temporaryOutput = [System.IO.Path]::GetTempFileName()
    try {
        cargo about generate --locked --fail --workspace `
            --config (Join-Path $projectRoot "about.toml") `
            --manifest-path (Join-Path $projectRoot "Cargo.toml") `
            --output-file $temporaryOutput `
            (Join-Path $PSScriptRoot "third-party-licenses.hbs")
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to generate third-party licenses."
        }

        $text = [System.IO.File]::ReadAllText($temporaryOutput)
        $vendorRoot = Join-Path $projectRoot "crates/marmkmt-sys/vendor/marmkmt"
        # Cargo's dependency graph does not include the vendored C++ SDK.
        foreach ($relativePath in @("LICENSE.md", "third_party/MargretePluginSDK/LICENSE.md")) {
            $licensePath = Join-Path $vendorRoot $relativePath
            $text += "`n========================================================================`n"
            $text += "Vendored component: marmkmt/$relativePath`n`n"
            $text += [System.IO.File]::ReadAllText($licensePath) + "`n"
        }
        $metadataJson = cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to locate bundled font notices."
        }
        $metadata = $metadataJson | ConvertFrom-Json
        # The font crate's SPDX expression does not describe all embedded font notices.
        foreach ($fontPackage in @($metadata.packages | Where-Object { $_.name -eq "epaint_default_fonts" })) {
            $fontDirectory = Join-Path (Split-Path $fontPackage.manifest_path -Parent) "fonts"
            $notices = @(Get-ChildItem -LiteralPath $fontDirectory -Filter *.txt -File | Sort-Object Name)
            if ($notices.Count -eq 0) {
                throw "No bundled font notices found for epaint_default_fonts."
            }
            foreach ($notice in $notices) {
                $text += "`n========================================================================`n"
                $text += "Bundled font notice: epaint_default_fonts $($fontPackage.version)/$($notice.Name)`n`n"
                $text += [System.IO.File]::ReadAllText($notice.FullName) + "`n"
            }
        }
        [System.IO.File]::WriteAllText($OutputPath, $text, $encoding)
    } finally {
        Remove-Item -LiteralPath $temporaryOutput -Force
    }
    Write-Host "Third-party licenses: $OutputPath"
} finally {
    Pop-Location
}
