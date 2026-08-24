[CmdletBinding()]
param(
    [string]$Tag,
    [switch]$TestMode
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repository = 'danielrepublic/wifimic'
$assetName = 'wifimic-windows-x86_64.zip'
$releaseSegment = if ([string]::IsNullOrWhiteSpace($Tag)) { 'latest/download' } else { "download/$Tag" }
$assetBase = "https://github.com/$repository/releases/$releaseSegment"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('wifimic-release-' + [Guid]::NewGuid().ToString('N'))

function Get-ExpectedSha256 {
    param([Parameter(Mandatory = $true)][string]$ManifestPath, [Parameter(Mandatory = $true)][string]$Name)

    $entry = Get-Content -LiteralPath $ManifestPath | Where-Object { $_ -match "^([0-9a-fA-F]{64})  \Q$Name\E$" }
    if ($entry.Count -ne 1) {
        throw "The checksum manifest does not contain one entry for '$Name'."
    }
    return ($entry -replace '^([0-9a-fA-F]{64}).*$', '$1').ToLowerInvariant()
}

try {
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
    $archivePath = Join-Path $temporaryRoot $assetName
    $manifestPath = Join-Path $temporaryRoot "$assetName.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri "$assetBase/$assetName" -OutFile $archivePath
    Invoke-WebRequest -UseBasicParsing -Uri "$assetBase/$assetName.sha256" -OutFile $manifestPath

    $expected = Get-ExpectedSha256 -ManifestPath $manifestPath -Name $assetName
    $actual = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Checksum verification failed for '$assetName'."
    }

    $stagePath = Join-Path $temporaryRoot 'stage'
    Expand-Archive -LiteralPath $archivePath -DestinationPath $stagePath -Force
    $client = Join-Path $stagePath 'wifimic_client.exe'
    $installer = Join-Path $stagePath 'install-wifimic-client.ps1'
    if (-not (Test-Path -LiteralPath $client -PathType Leaf) -or -not (Test-Path -LiteralPath $installer -PathType Leaf)) {
        throw 'The verified archive is missing the Windows client or installer.'
    }

    $arguments = @{ ClientExecutable = $client }
    if ($TestMode) {
        $arguments.TestMode = $true
    }
    else {
        $arguments.AcceptHostMutation = $true
    }
    & $installer @arguments
}
finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
