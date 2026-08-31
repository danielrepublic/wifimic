[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$installer = Join-Path $PSScriptRoot 'install-wifimic-client.ps1'
$powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('wifimic-client-installer-tests-' + [Guid]::NewGuid().ToString('N'))

function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if (-not [object]::Equals($Actual, $Expected)) {
        throw "$Message Expected '$Expected', got '$Actual'."
    }
}

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Invoke-FakeInstaller {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$StateRoot,
        [switch]$DryRun,
        [string]$MachinePath,
        [string]$LegacyUpdaterContent,
        [string]$FailurePoint
    )

    $arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $installer, '-ClientExecutable', $Source, '-TestStateRoot', $StateRoot, '-FakeMachinePath', $MachinePath)
    if ($DryRun) { $arguments += '-DryRun' } else { $arguments += '-TestMode' }
    if ($PSBoundParameters.ContainsKey('LegacyUpdaterContent')) { $arguments += @('-FakeLegacyUpdaterContent', $LegacyUpdaterContent) }
    if ($PSBoundParameters.ContainsKey('FailurePoint')) { $arguments += @('-FailurePoint', $FailurePoint) }

    $json = (& $powershell @arguments) -join [Environment]::NewLine
    [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        Result = $json | ConvertFrom-Json
    }
}

try {
    New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
    $source = Join-Path $testRoot 'wifimic_client.exe'
    [System.IO.File]::WriteAllBytes($source, [byte[]](1, 2, 3))
    $actualMachinePathBefore = [Environment]::GetEnvironmentVariable('Path', [EnvironmentVariableTarget]::Machine)

    $initialPath = 'C:\Tools;C:\Program Files\wifimic-client-tools;C:\Windows\System32'
    $inserted = Invoke-FakeInstaller -Source $source -StateRoot (Join-Path $testRoot 'path-insertion') -MachinePath $initialPath
    Assert-Equal $inserted.ExitCode 0 'TestMode PATH insertion should succeed.'
    Assert-Equal $inserted.Result.FakeMachinePath "$initialPath;C:\Program Files\wifimic-client" 'TestMode should append the client directory to machine PATH.'
    Assert-True (@($inserted.Result.FakeEvents) -contains 'SetMachinePath') 'TestMode should record the machine PATH write.'
    Assert-True (@($inserted.Result.FakeEvents) -contains 'BroadcastEnvironmentChange') 'TestMode should record the environment broadcast.'

    $alreadyPresentPath = 'C:\Tools;C:\PROGRAM FILES\WIFIMIC-CLIENT;C:\Windows\System32'
    $alreadyPresent = Invoke-FakeInstaller -Source $source -StateRoot (Join-Path $testRoot 'path-idempotence') -MachinePath $alreadyPresentPath
    Assert-Equal $alreadyPresent.ExitCode 0 'TestMode idempotent PATH install should succeed.'
    Assert-Equal $alreadyPresent.Result.FakeMachinePath $alreadyPresentPath 'An existing client PATH segment must remain unchanged.'
    Assert-True (-not (@($alreadyPresent.Result.FakeEvents) -contains 'SetMachinePath')) 'An existing client PATH segment must not trigger a write.'

    $dryRun = Invoke-FakeInstaller -Source $source -StateRoot (Join-Path $testRoot 'dry-run') -DryRun -MachinePath $initialPath
    Assert-Equal $dryRun.ExitCode 0 'DryRun should validate the machine PATH operation.'
    Assert-True ([bool]$dryRun.Result.MachinePathWouldChange) 'DryRun should report the pending PATH insertion.'
    Assert-Equal $dryRun.Result.FakeMachinePath $initialPath 'DryRun must not mutate fake machine PATH.'
    Assert-True (-not (@($dryRun.Result.FakeEvents) -contains 'SetMachinePath')) 'DryRun must not write machine PATH.'

    $rollbackPath = 'C:\Tools;C:\Windows\System32'
    $rollback = Invoke-FakeInstaller -Source $source -StateRoot (Join-Path $testRoot 'rollback') -MachinePath $rollbackPath -LegacyUpdaterContent 'legacy-updater-v1' -FailurePoint 'BeforeVerification'
    Assert-Equal $rollback.ExitCode 1 'Injected failure should fail the TestMode install.'
    Assert-Equal $rollback.Result.ErrorCode 'SimulatedFailure' 'Injected failure should preserve its error code.'
    Assert-Equal $rollback.Result.FakeMachinePath $rollbackPath 'Rollback must restore machine PATH exactly.'
    Assert-True ([bool]$rollback.Result.FakeLegacyUpdaterExists) 'Rollback must restore the legacy updater.'
    Assert-Equal $rollback.Result.FakeLegacyUpdaterBase64 $rollback.Result.FakeInitialLegacyUpdaterBase64 'Rollback must restore the legacy updater bytes exactly.'
    Assert-True ((@($rollback.Result.FakeEvents | Where-Object { $_ -eq 'SetMachinePath' }).Count) -eq 2) 'Rollback must record both PATH write and restoration.'
    Assert-True (@($rollback.Result.FakeEvents) -contains 'RestoreFile') 'Rollback must restore the legacy updater through the file operation surface.'

    $cleanup = Invoke-FakeInstaller -Source $source -StateRoot (Join-Path $testRoot 'legacy-cleanup') -MachinePath $initialPath -LegacyUpdaterContent 'legacy-updater-v1'
    Assert-Equal $cleanup.ExitCode 0 'Repair install should succeed.'
    Assert-True (-not ([bool]$cleanup.Result.FakeLegacyUpdaterExists)) 'Successful repair must remove the legacy updater.'
    Assert-True (@($cleanup.Result.FakeEvents) -contains 'RemoveFile') 'Successful repair must remove the legacy updater through the file operation surface.'

    Assert-Equal ([Environment]::GetEnvironmentVariable('Path', [EnvironmentVariableTarget]::Machine)) $actualMachinePathBefore 'TestMode and DryRun must not mutate the real machine PATH.'
    'install-wifimic-client TestMode regressions passed'
}
finally {
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
