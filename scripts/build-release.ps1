[CmdletBinding()]
param(
    [string]$CertificateThumbprint,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$distDir = Join-Path $repoRoot "dist"
$releaseDir = Join-Path $repoRoot "target\release"
$msiPath = Join-Path $repoRoot "installer\NukaBoost.msi"
$mainExe = Join-Path $releaseDir "NukaBoost.exe"
$ctlExe = Join-Path $releaseDir "nukaboostctl.exe"

function Invoke-Checked {
    param([scriptblock]$Command, [string]$Description)
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Find-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    $kits = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (Test-Path -LiteralPath $kits) {
        $candidate = Get-ChildItem -LiteralPath $kits -Filter signtool.exe -Recurse |
            Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) { return $candidate.FullName }
    }
    throw "signtool.exe was not found. Install the Windows SDK."
}

Push-Location $repoRoot
try {
    Invoke-Checked { cargo fmt --all -- --check } "rustfmt"
    Invoke-Checked { cargo test --workspace --locked } "tests"
    Invoke-Checked { cargo clippy --workspace --all-targets --locked -- -D warnings } "clippy"
    Invoke-Checked { cargo build --workspace --release --locked } "release build"

    $signTool = $null
    if ($CertificateThumbprint) {
        $signTool = Find-SignTool
        foreach ($file in @($mainExe, $ctlExe)) {
            Invoke-Checked {
                & $signTool sign /sha1 $CertificateThumbprint /fd SHA256 `
                    /tr $TimestampUrl /td SHA256 $file
            } "Authenticode signing of $file"
            Invoke-Checked { & $signTool verify /pa /all /v $file } "signature verification of $file"
        }
    }

    Invoke-Checked {
        wix build installer\NukaBoost.wxs -arch x64 `
            -b assets=assets -b release=target\release `
            -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
            -out installer\NukaBoost.msi
    } "MSI build"
    Invoke-Checked { wix msi validate -sice ICE91 installer\NukaBoost.msi } "MSI validation"

    if ($CertificateThumbprint) {
        Invoke-Checked {
            & $signTool sign /sha1 $CertificateThumbprint /fd SHA256 `
                /tr $TimestampUrl /td SHA256 $msiPath
        } "Authenticode signing of the MSI"
        Invoke-Checked { & $signTool verify /pa /all /v $msiPath } "MSI signature verification"
    } else {
        Write-Warning "Artifacts are UNSIGNED and must not be published as a stable release."
    }

    New-Item -ItemType Directory -Path $distDir -Force | Out-Null
    Copy-Item -LiteralPath $mainExe, $ctlExe, $msiPath -Destination $distDir -Force
    $artifacts = @("NukaBoost.exe", "nukaboostctl.exe", "NukaBoost.msi") |
        ForEach-Object { Join-Path $distDir $_ }
    $checksums = foreach ($artifact in $artifacts) {
        $hash = Get-FileHash -LiteralPath $artifact -Algorithm SHA256
        "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($artifact))"
    }
    Set-Content -LiteralPath (Join-Path $distDir "SHA256SUMS.txt") -Value $checksums -Encoding ascii
    Write-Host "Release artifacts created in $distDir"
} finally {
    Pop-Location
}
