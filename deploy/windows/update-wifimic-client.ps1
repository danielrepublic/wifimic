[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Tag,
    [switch]$TestMode,
    [switch]$AcceptHostMutation,
    [string]$TestStateRoot,
    [ValidateSet('DirtyCheckout', 'BadRevision', 'AmbiguousRevision', 'Fetch', 'Worktree', 'Build', 'Endpoint', 'DisableTask', 'StopTask', 'BeforeAtomicSwap', 'AtomicSwap', 'AfterAtomicSwap', 'SetTask', 'TaskRegistration', 'StartTask', 'Health')]
    [string]$FailurePoint
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:CanonicalInstallRoot = 'C:\Program Files\wifimic-client'
$script:CanonicalExecutableName = 'wifimic_client.exe'
$script:CanonicalExecutablePath = 'C:\Program Files\wifimic-client\wifimic_client.exe'
$script:CanonicalTaskFolder = '\wifimic\'
$script:CanonicalTaskName = 'wifimic-client'
$script:CanonicalTaskPath = '\wifimic\wifimic-client'
$script:CanonicalEndpoint = 'CABLE Input (VB-Audio Virtual Cable)'
$script:TaskNamespace = 'http://schemas.microsoft.com/windows/2004/02/mit/task'
$script:TaskTimeoutAttempts = 50

function New-WifimicUpdaterException {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Code,
        [Parameter(Mandatory = $true)][string]$Message,
        [System.Exception]$InnerException
    )

    $exception = if ($null -eq $InnerException) {
        [System.InvalidOperationException]::new("[$Code] $Message")
    }
    else {
        [System.InvalidOperationException]::new("[$Code] $Message", $InnerException)
    }
    $exception.Data['WifimicCode'] = $Code
    return $exception
}

function Throw-WifimicUpdaterError {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Code,
        [Parameter(Mandatory = $true)][string]$Message
    )

    throw (New-WifimicUpdaterException -Code $Code -Message $Message)
}

function Invoke-WifimicOperation {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][string]$Name,
        [object[]]$Arguments = @()
    )

    $operation = $Operations.$Name
    if ($null -eq $operation) {
        Throw-WifimicUpdaterError -Code 'MissingOperation' -Message "Native operation '$Name' is not configured."
    }
    return & $operation @Arguments
}

function Get-WifimicIdentity {
    [CmdletBinding()]
    param()

    [pscustomobject]@{
        InstallRoot = $script:CanonicalInstallRoot
        ExecutableName = $script:CanonicalExecutableName
        ExecutablePath = $script:CanonicalExecutablePath
        TaskFolder = $script:CanonicalTaskFolder
        TaskName = $script:CanonicalTaskName
        TaskPath = $script:CanonicalTaskPath
        Endpoint = $script:CanonicalEndpoint
        TaskUri = $script:CanonicalTaskPath
        Description = 'Interactive wifimic client.'
    }
}

function Assert-WifimicRevisionText {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Revision)

    if ([string]::IsNullOrEmpty($Revision)) {
        Throw-WifimicUpdaterError -Code 'InvalidRevision' -Message 'Exactly one non-empty -Tag revision is required.'
    }
    if ($Revision -ne $Revision.Trim()) {
        Throw-WifimicUpdaterError -Code 'InvalidRevisionWhitespace' -Message 'The -Tag revision may not have leading or trailing whitespace.'
    }
    if ($Revision.StartsWith('-', [System.StringComparison]::Ordinal)) {
        Throw-WifimicUpdaterError -Code 'InvalidRevisionOption' -Message 'The -Tag revision may not begin with an option marker.'
    }
    if ($Revision.IndexOf([char]0) -ge 0 -or $Revision -match '\s') {
        Throw-WifimicUpdaterError -Code 'InvalidRevisionWhitespace' -Message 'The -Tag revision may not contain whitespace or NUL characters.'
    }
    if ($Revision -match '\.\.|@\{|\\|:|\^') {
        Throw-WifimicUpdaterError -Code 'InvalidRevisionSyntax' -Message 'The -Tag revision must be a tag name or hexadecimal commit, not a composite Git expression.'
    }
    if ($Revision -notmatch '^[A-Za-z0-9][A-Za-z0-9._/@+~-]*$') {
        Throw-WifimicUpdaterError -Code 'InvalidRevisionSyntax' -Message 'The -Tag revision contains characters that are not valid for an explicit tag or commit.'
    }
    return $Revision
}

function ConvertTo-WifimicSha256 {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function Get-WifimicXmlNode {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][xml]$Xml,
        [Parameter(Mandatory = $true)][string]$XPath
    )

    $manager = New-Object System.Xml.XmlNamespaceManager($Xml.NameTable)
    $manager.AddNamespace('task', $script:TaskNamespace)
    $node = $Xml.SelectSingleNode($XPath, $manager)
    if ($null -eq $node) {
        Throw-WifimicUpdaterError -Code 'InvalidTaskXml' -Message "Task XML is missing '$XPath'."
    }
    return $node
}

function New-WifimicTaskXml {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][pscustomobject]$Identity)

    $command = [System.Security.SecurityElement]::Escape($Identity.ExecutablePath)
    $workingDirectory = [System.Security.SecurityElement]::Escape($Identity.InstallRoot)
    $uri = [System.Security.SecurityElement]::Escape($Identity.TaskUri)
    $description = [System.Security.SecurityElement]::Escape($Identity.Description)
    return @"
<?xml version="1.0" encoding="UTF-16"?>
<Task xmlns="$script:TaskNamespace" version="1.4">
  <RegistrationInfo>
    <Description>$description</Description>
    <URI>$uri</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>$command</Command>
      <WorkingDirectory>$workingDirectory</WorkingDirectory>
      <Arguments />
    </Exec>
  </Actions>
</Task>
"@
}

function ConvertTo-WifimicTaskDefinition {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [Parameter(Mandatory = $true)][string]$XmlText,
        [Parameter(Mandatory = $true)][bool]$Enabled,
        [string]$State = 'Ready'
    )

    [xml]$xml = $XmlText
    $command = (Get-WifimicXmlNode -Xml $xml -XPath '//task:Actions/task:Exec/task:Command').InnerText
    $workingDirectory = (Get-WifimicXmlNode -Xml $xml -XPath '//task:Actions/task:Exec/task:WorkingDirectory').InnerText
    # Task Scheduler drops a genuinely empty <Arguments/> element on export, so
    # a task registered with no CLI arguments (the normal wifimic-client case)
    # has no Arguments node at all. Treat its absence as an empty string
    # instead of the mandatory-node error the other Exec fields use.
    $manager = New-Object System.Xml.XmlNamespaceManager($xml.NameTable)
    $manager.AddNamespace('task', $script:TaskNamespace)
    $argumentsNode = $xml.SelectSingleNode('//task:Actions/task:Exec/task:Arguments', $manager)
    $arguments = if ($null -eq $argumentsNode) { '' } else { $argumentsNode.InnerText }
    $uri = (Get-WifimicXmlNode -Xml $xml -XPath '//task:RegistrationInfo/task:URI').InnerText
    [pscustomobject]@{
        TaskPath = $Identity.TaskPath
        XmlText = $XmlText
        Enabled = $Enabled
        State = $State
        Signature = [pscustomobject]@{
            Uri = $uri
            ExecutablePath = $command
            WorkingDirectory = $workingDirectory
            Arguments = $arguments
        }
    }
}

function Test-WifimicTaskContract {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Definition,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity
    )

    [xml]$xml = $Definition.XmlText
    $trigger = Get-WifimicXmlNode -Xml $xml -XPath '//task:Triggers/task:LogonTrigger'
    $principal = Get-WifimicXmlNode -Xml $xml -XPath '//task:Principals/task:Principal'
    $logonType = (Get-WifimicXmlNode -Xml $xml -XPath '//task:Principals/task:Principal/task:LogonType').InnerText
    $manager = New-Object System.Xml.XmlNamespaceManager($xml.NameTable)
    $manager.AddNamespace('task', $script:TaskNamespace)
    $password = $xml.SelectSingleNode('//task:Principals/task:Principal/task:Password', $manager)
    if ($trigger.LocalName -ne 'LogonTrigger' -or $principal.Attributes['id'].Value -ne 'Author' -or $logonType -ne 'InteractiveToken') {
        Throw-WifimicUpdaterError -Code 'TaskContractMismatch' -Message 'The task must be the canonical interactive LogonTrigger task.'
    }
    if ($null -ne $password) {
        Throw-WifimicUpdaterError -Code 'CredentialPersistence' -Message 'The task may not persist a password or other credential.'
    }
    if (-not [string]::Equals($Definition.TaskPath, $Identity.TaskPath, [System.StringComparison]::Ordinal) -or
        -not [string]::Equals($Definition.Signature.Uri, $Identity.TaskUri, [System.StringComparison]::Ordinal) -or
        -not [string]::Equals($Definition.Signature.ExecutablePath, $Identity.ExecutablePath, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($Definition.Signature.WorkingDirectory, $Identity.InstallRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($Definition.Signature.Arguments, '', [System.StringComparison]::Ordinal)) {
        Throw-WifimicUpdaterError -Code 'TaskContractMismatch' -Message 'The task path, URI, executable, working directory, or arguments are not canonical.'
    }
    if ($Definition.Signature.ExecutablePath -match '(?i)(powershell|cmd)\.exe') {
        Throw-WifimicUpdaterError -Code 'ShellWrapperRejected' -Message 'The task must launch wifimic_client.exe directly.'
    }
    if (@('Ready', 'Running', 'Disabled') -notcontains [string]$Definition.State) {
        Throw-WifimicUpdaterError -Code 'TaskStateUnsupported' -Message "The task reported unsupported state '$($Definition.State)'."
    }
    return $true
}

function Test-WifimicCapturedFileEqual {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Actual,
        [Parameter(Mandatory = $true)][pscustomobject]$Expected
    )

    return [string]::Equals($Actual.Hash, $Expected.Hash, [System.StringComparison]::OrdinalIgnoreCase) -and
        $Actual.Bytes.Length -eq $Expected.Bytes.Length
}

function Assert-WifimicPreflight {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [Parameter(Mandatory = $true)][pscustomobject]$Task,
        [Parameter(Mandatory = $true)][pscustomobject]$Executable
    )

    Test-WifimicTaskContract -Definition $Task -Identity $Identity | Out-Null
    if (-not [bool]$Task.Enabled) {
        Throw-WifimicUpdaterError -Code 'TaskDisabled' -Message "The canonical task '$($Identity.TaskPath)' must be enabled before an update."
    }
    if ([string]$Task.State -notin @('Ready', 'Running')) {
        Throw-WifimicUpdaterError -Code 'TaskNotUsable' -Message "The canonical task '$($Identity.TaskPath)' is in state '$($Task.State)'."
    }
    if ($null -eq $Executable) {
        Throw-WifimicUpdaterError -Code 'ExecutableMissing' -Message "The installed executable '$($Identity.ExecutablePath)' was not found."
    }
    $endpointNames = @(Invoke-WifimicOperation -Operations $Operations -Name 'GetRenderEndpointNames')
    if ($endpointNames -notcontains $Identity.Endpoint) {
        $available = if ($endpointNames.Count -eq 0) { '<none>' } else { $endpointNames -join ', ' }
        Throw-WifimicUpdaterError -Code 'EndpointNotFound' -Message "Exact render endpoint '$($Identity.Endpoint)' was not enumerated. Available render endpoints: $available"
    }
}

function Invoke-WifimicRequestedFailure {
    [CmdletBinding()]
    param(
        [string]$Requested,
        [Parameter(Mandatory = $true)][string]$Point
    )

    if ($Requested -eq $Point) {
        Throw-WifimicUpdaterError -Code 'SimulatedFailure' -Message "Deterministic test failure at '$Point'."
    }
}

function Wait-WifimicTaskNotRunning {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity
    )

    for ($attempt = 0; $attempt -lt $script:TaskTimeoutAttempts; $attempt++) {
        $task = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($Identity)
        if ($null -eq $task -or [string]$task.State -ne 'Running') {
            return $true
        }
        Invoke-WifimicOperation -Operations $Operations -Name 'Sleep' -Arguments @(100) | Out-Null
    }
    Throw-WifimicUpdaterError -Code 'TaskStopTimeout' -Message "The task '$($Identity.TaskPath)' remained Running beyond the bounded stop wait."
}

function Wait-WifimicClientHealth {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [Parameter(Mandatory = $true)][bool]$RequireRunning
    )

    for ($attempt = 0; $attempt -lt $script:TaskTimeoutAttempts; $attempt++) {
        $health = Invoke-WifimicOperation -Operations $Operations -Name 'CheckHealth' -Arguments @($Identity, $RequireRunning)
        if ([bool]$health.Healthy) {
            return $health
        }
        Invoke-WifimicOperation -Operations $Operations -Name 'Sleep' -Arguments @(100) | Out-Null
    }
    Throw-WifimicUpdaterError -Code 'HealthTimeout' -Message "The updated task did not reach an enabled Ready/Running state with the exact VB-CABLE endpoint within the bounded health wait."
}

function Restore-WifimicClientTransaction {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [Parameter(Mandatory = $true)][pscustomobject]$Transaction
    )

    $errors = [System.Collections.ArrayList]::new()
    try {
        $current = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($Identity)
        if ($null -ne $current) {
            Invoke-WifimicOperation -Operations $Operations -Name 'DisableTask' -Arguments @($Identity) | Out-Null
            if ([string]$current.State -eq 'Running') {
                Invoke-WifimicOperation -Operations $Operations -Name 'StopTask' -Arguments @($Identity) | Out-Null
                Wait-WifimicTaskNotRunning -Operations $Operations -Identity $Identity | Out-Null
            }
        }
    }
    catch { [void]$errors.Add("stop current task: $($_.Exception.Message)") }

    try {
        Invoke-WifimicOperation -Operations $Operations -Name 'RestoreAtomicFile' -Arguments @($Identity.ExecutablePath, $Transaction.PriorExecutable.Bytes) | Out-Null
    }
    catch { [void]$errors.Add("restore executable: $($_.Exception.Message)") }

    try {
        Invoke-WifimicOperation -Operations $Operations -Name 'RestoreTask' -Arguments @($Identity, $Transaction.PriorTask.XmlText, $true) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'EnableTask' -Arguments @($Identity) | Out-Null
        if ($Transaction.PriorWasRunning) {
            Invoke-WifimicOperation -Operations $Operations -Name 'StartTask' -Arguments @($Identity) | Out-Null
        }
    }
    catch { [void]$errors.Add("restore task: $($_.Exception.Message)") }

    try {
        $restoredFile = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($Identity.ExecutablePath)
        if ($null -eq $restoredFile -or -not (Test-WifimicCapturedFileEqual -Actual $restoredFile -Expected $Transaction.PriorExecutable)) {
            Throw-WifimicUpdaterError -Code 'RollbackVerification' -Message 'The prior executable hash was not restored.'
        }
        $restoredTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($Identity)
        if ($null -eq $restoredTask -or -not [bool]$restoredTask.Enabled -or
            -not [string]::Equals($restoredTask.XmlText, $Transaction.PriorTask.XmlText, [System.StringComparison]::Ordinal)) {
            Throw-WifimicUpdaterError -Code 'RollbackVerification' -Message 'The prior task XML was not restored with the task enabled.'
        }
        Test-WifimicTaskContract -Definition $restoredTask -Identity $Identity | Out-Null
        if ([string]$restoredTask.State -notin @('Ready', 'Running')) {
            Throw-WifimicUpdaterError -Code 'RollbackVerification' -Message "The restored task remained in state '$($restoredTask.State)'."
        }
        if ($Transaction.PriorWasRunning -and [string]$restoredTask.State -ne 'Running') {
            Throw-WifimicUpdaterError -Code 'RollbackVerification' -Message 'The previously running task was not restarted.'
        }
    }
    catch { [void]$errors.Add("verify rollback: $($_.Exception.Message)") }

    if ($errors.Count -gt 0) {
        Throw-WifimicUpdaterError -Code 'RollbackFailed' -Message ($errors -join '; ')
    }
}

function Invoke-WifimicClientUpdate {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Revision,
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [string]$Mode = 'Native',
        [string]$FailurePoint
    )

    $identity = Get-WifimicIdentity
    Assert-WifimicRevisionText -Revision $Revision | Out-Null
    $sourceRoot = [System.IO.Path]::GetFullPath($SourceRoot).TrimEnd('\', '/')

    $sourceStatus = Invoke-WifimicOperation -Operations $Operations -Name 'GetSourceStatus' -Arguments @($sourceRoot)
    if (-not [string]::IsNullOrEmpty([string]$sourceStatus)) {
        Throw-WifimicUpdaterError -Code 'DirtyCheckout' -Message "The source checkout is dirty; refusing to fetch, build, or mutate the installed client. Changes: $sourceStatus"
    }
    Invoke-WifimicOperation -Operations $Operations -Name 'FetchTags' -Arguments @($sourceRoot) | Out-Null
    $resolvedOutput = Invoke-WifimicOperation -Operations $Operations -Name 'ResolveRevision' -Arguments @($sourceRoot, $Revision)
    $resolvedLines = @(([string]$resolvedOutput) -split '\r?\n' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if ($resolvedLines.Count -ne 1 -or $resolvedLines[0] -notmatch '^[0-9a-fA-F]{40}$') {
        Throw-WifimicUpdaterError -Code 'AmbiguousRevision' -Message "The explicit revision '$Revision' did not resolve to exactly one commit."
    }
    $resolvedCommit = $resolvedLines[0].ToLowerInvariant()
    $priorTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
    $priorExecutable = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.ExecutablePath)
    if ($null -eq $priorTask) {
        Throw-WifimicUpdaterError -Code 'TaskMissing' -Message "The canonical task '$($identity.TaskPath)' does not exist."
    }
    Assert-WifimicPreflight -Operations $Operations -Identity $identity -Task $priorTask -Executable $priorExecutable

    $stageRoot = Join-Path $identity.InstallRoot ('.wifimic-client-stage-' + [Guid]::NewGuid().ToString('N'))
    $transactionRoot = Join-Path $identity.InstallRoot ('.wifimic-client-transaction-' + [Guid]::NewGuid().ToString('N'))
    $candidatePath = Join-Path $stageRoot ('target\release\' + $identity.ExecutableName)
    $failure = $null
    $worktreeAdded = $false
    $transactionStarted = $false
    $taskMutationStarted = $false
    $binaryMutationStarted = $false
    $transaction = [pscustomobject]@{
        PriorExecutable = $priorExecutable
        PriorTask = $priorTask
        PriorTaskEnabled = [bool]$priorTask.Enabled
        PriorWasRunning = [string]$priorTask.State -eq 'Running'
        StageRoot = $stageRoot
        TransactionRoot = $transactionRoot
        CandidatePath = $candidatePath
        ResolvedCommit = $resolvedCommit
    }

    try {
        Invoke-WifimicOperation -Operations $Operations -Name 'EnsureDirectory' -Arguments @($stageRoot) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'EnsureDirectory' -Arguments @($transactionRoot) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'AddDetachedWorktree' -Arguments @($sourceRoot, $stageRoot, $resolvedCommit) | Out-Null
        $worktreeAdded = $true
        Invoke-WifimicOperation -Operations $Operations -Name 'BuildCandidate' -Arguments @($stageRoot) | Out-Null
        $candidate = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($candidatePath)
        if ($null -eq $candidate) {
            Throw-WifimicUpdaterError -Code 'BuildOutputMissing' -Message "Cargo did not produce '$candidatePath'."
        }
        if ($candidate.Bytes.Length -eq 0) {
            Throw-WifimicUpdaterError -Code 'BuildOutputEmpty' -Message 'The built client executable was empty.'
        }
        $transaction | Add-Member -NotePropertyName Candidate -NotePropertyValue $candidate

        $postBuildTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
        $postBuildExecutable = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.ExecutablePath)
        Assert-WifimicPreflight -Operations $Operations -Identity $identity -Task $postBuildTask -Executable $postBuildExecutable
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'Endpoint'

        Invoke-WifimicOperation -Operations $Operations -Name 'WriteTransactionBytes' -Arguments @($transactionRoot, $priorExecutable.Bytes) | Out-Null
        $transactionStarted = $true

        $taskMutationStarted = $true
        Invoke-WifimicOperation -Operations $Operations -Name 'DisableTask' -Arguments @($identity) | Out-Null
        $disabledTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
        if ($null -eq $disabledTask -or [bool]$disabledTask.Enabled) {
            Throw-WifimicUpdaterError -Code 'TaskDisableVerification' -Message "The canonical task '$($identity.TaskPath)' was not disabled before the binary swap."
        }
        if ([string]$disabledTask.State -eq 'Running') {
            Invoke-WifimicOperation -Operations $Operations -Name 'StopTask' -Arguments @($identity) | Out-Null
            Wait-WifimicTaskNotRunning -Operations $Operations -Identity $identity | Out-Null
        }

        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'BeforeAtomicSwap'
        $binaryMutationStarted = $true
        Invoke-WifimicOperation -Operations $Operations -Name 'AtomicInstallFile' -Arguments @($candidatePath, $identity.ExecutablePath) | Out-Null
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'AtomicSwap'
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'AfterAtomicSwap'

        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'SetTask'
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'TaskRegistration'
        Invoke-WifimicOperation -Operations $Operations -Name 'RestoreTask' -Arguments @($identity, $priorTask.XmlText, $true) | Out-Null
        $taskAfterRestore = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
        Test-WifimicTaskContract -Definition $taskAfterRestore -Identity $identity | Out-Null
        if (-not [bool]$taskAfterRestore.Enabled) {
            Throw-WifimicUpdaterError -Code 'TaskRegistrationDisabled' -Message 'The task registration was not enabled after the candidate swap.'
        }
        Invoke-WifimicOperation -Operations $Operations -Name 'EnableTask' -Arguments @($identity) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'StartTask' -Arguments @($identity) | Out-Null
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'StartTask'
        $health = Wait-WifimicClientHealth -Operations $Operations -Identity $identity -RequireRunning:$transaction.PriorWasRunning
        Invoke-WifimicRequestedFailure -Requested $FailurePoint -Point 'Health'

        $installed = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.ExecutablePath)
        [pscustomobject]@{
            Status = 'Updated'
            Mode = $Mode
            Tag = $Revision
            ResolvedCommit = $resolvedCommit
            ExecutablePath = $identity.ExecutablePath
            PriorHash = $priorExecutable.Hash
            CandidateHash = $candidate.Hash
            InstalledHash = $installed.Hash
            TaskPath = $identity.TaskPath
            TaskEnabled = [bool]$health.Enabled
            TaskState = [string]$health.State
            Endpoint = $identity.Endpoint
            PriorTaskEnabled = [bool]$transaction.PriorTaskEnabled
            PriorTaskWasRunning = [bool]$transaction.PriorWasRunning
            StagingRoot = $stageRoot
        }
    }
    catch {
        $failure = $_
        if ($transactionStarted -and ($taskMutationStarted -or $binaryMutationStarted)) {
            try {
                Restore-WifimicClientTransaction -Operations $Operations -Identity $identity -Transaction $transaction
            }
            catch {
                throw (New-WifimicUpdaterException -Code 'RollbackFailed' -Message "Update failed with '$($failure.Exception.Message)'; automatic rollback failed with '$($_.Exception.Message)'." -InnerException $failure.Exception)
            }
        }
        throw $failure
    }
    finally {
        try {
            if ($worktreeAdded) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDetachedWorktree' -Arguments @($sourceRoot, $stageRoot) | Out-Null
            }
            Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDirectoryTree' -Arguments @($stageRoot) | Out-Null
            Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDirectoryTree' -Arguments @($transactionRoot) | Out-Null
        }
        catch {
            if ($null -eq $failure) {
                Throw-WifimicUpdaterError -Code 'CleanupFailed' -Message $_.Exception.Message
            }
            throw (New-WifimicUpdaterException -Code 'CleanupFailed' -Message "Update cleanup failed after '$($failure.Exception.Message)': $($_.Exception.Message)" -InnerException $failure.Exception)
        }
    }
}

function ConvertTo-WifimicProcessArgument {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)

    if ($Value.Length -eq 0) { return '""' }
    if ($Value -notmatch '[\s"]') { return $Value }
    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

function Invoke-WifimicNativeProcess {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [int]$TimeoutMilliseconds = 120000
    )

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $FilePath
    $info.Arguments = (($Arguments | ForEach-Object { ConvertTo-WifimicProcessArgument -Value $_ }) -join ' ')
    $info.WorkingDirectory = $WorkingDirectory
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    try {
        if (-not $process.Start()) {
            Throw-WifimicUpdaterError -Code 'ProcessStartFailed' -Message "Could not start '$FilePath'."
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            try { $process.Kill() } catch { }
            Throw-WifimicUpdaterError -Code 'ProcessTimeout' -Message "Process '$FilePath' exceeded the bounded $TimeoutMilliseconds ms timeout."
        }
        $stdout = $stdoutTask.Result
        $stderr = $stderrTask.Result
        [pscustomobject]@{ ExitCode = $process.ExitCode; StdOut = $stdout; StdErr = $stderr }
    }
    finally {
        $process.Dispose()
    }
}

function New-WifimicNativeOperations {
    [CmdletBinding()]
    param()

    $getTask = {
        param($identity)
        $task = Get-ScheduledTask -TaskPath $identity.TaskFolder -TaskName $identity.TaskName -ErrorAction SilentlyContinue
        if ($null -eq $task) { return $null }
        $xml = Export-ScheduledTask -TaskPath $identity.TaskFolder -TaskName $identity.TaskName -ErrorAction Stop
        ConvertTo-WifimicTaskDefinition -Identity $identity -XmlText $xml -Enabled ([bool]$task.Settings.Enabled) -State ([string]$task.State)
    }.GetNewClosure()
    $run = {
        param($file, $arguments, $workingDirectory, $timeout)
        $result = Invoke-WifimicNativeProcess -FilePath $file -Arguments $arguments -WorkingDirectory $workingDirectory -TimeoutMilliseconds $timeout
        if ($result.ExitCode -ne 0) {
            Throw-WifimicUpdaterError -Code 'NativeProcessFailed' -Message "$file failed with exit code $($result.ExitCode): $($result.StdErr.Trim())"
        }
        return $result
    }.GetNewClosure()
    $getEndpoints = { @(Get-PnpDevice -Class AudioEndpoint -Status OK -ErrorAction Stop | ForEach-Object { [string]$_.FriendlyName }) }.GetNewClosure()

    [pscustomobject]@{
        GetSourceStatus = {
            param($sourceRoot)
            $result = & $run 'git.exe' @('-C', $sourceRoot, 'status', '--porcelain', '--untracked-files=all') $sourceRoot 30000
            return $result.StdOut.Trim()
        }.GetNewClosure()
        FetchTags = {
            param($sourceRoot)
            & $run 'git.exe' @('-C', $sourceRoot, 'fetch', '--tags', '--prune', 'origin') $sourceRoot 120000 | Out-Null
        }.GetNewClosure()
        ResolveRevision = {
            param($sourceRoot, $revision)
            $expression = $revision + '^{commit}'
            $result = & $run 'git.exe' @('-C', $sourceRoot, 'rev-parse', '--verify', '--end-of-options', $expression) $sourceRoot 30000
            return $result.StdOut.Trim()
        }.GetNewClosure()
        EnsureDirectory = { param($path) New-Item -ItemType Directory -Path $path -Force | Out-Null }.GetNewClosure()
        AddDetachedWorktree = {
            param($sourceRoot, $stageRoot, $commit)
            & $run 'git.exe' @('-C', $sourceRoot, 'worktree', 'add', '--detach', $stageRoot, $commit) $sourceRoot 120000 | Out-Null
        }.GetNewClosure()
        BuildCandidate = {
            param($stageRoot)
            & $run 'cargo.exe' @('build', '--release', '--locked', '-p', 'wifimic_client') $stageRoot 600000 | Out-Null
        }.GetNewClosure()
        RemoveDetachedWorktree = {
            param($sourceRoot, $stageRoot)
            & $run 'git.exe' @('-C', $sourceRoot, 'worktree', 'remove', '--force', $stageRoot) $sourceRoot 30000 | Out-Null
        }.GetNewClosure()
        GetTask = $getTask
        RestoreTask = {
            param($identity, $xmlText, $enabled)
            $temporary = Join-Path $env:TEMP ('wifimic-client-task-' + [Guid]::NewGuid().ToString('N') + '.xml')
            try {
                [System.IO.File]::WriteAllText($temporary, $xmlText, [System.Text.Encoding]::Unicode)
                & $run 'schtasks.exe' @('/Create', '/TN', $identity.TaskPath, '/XML', $temporary, '/F') $identity.InstallRoot 30000 | Out-Null
                $switch = if ($enabled) { '/ENABLE' } else { '/DISABLE' }
                & $run 'schtasks.exe' @('/Change', '/TN', $identity.TaskPath, $switch) $identity.InstallRoot 30000 | Out-Null
            }
            finally { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue }
        }.GetNewClosure()
        DisableTask = { param($identity) & $run 'schtasks.exe' @('/Change', '/TN', $identity.TaskPath, '/DISABLE') $identity.InstallRoot 30000 | Out-Null }.GetNewClosure()
        EnableTask = { param($identity) & $run 'schtasks.exe' @('/Change', '/TN', $identity.TaskPath, '/ENABLE') $identity.InstallRoot 30000 | Out-Null }.GetNewClosure()
        StopTask = { param($identity) & $run 'schtasks.exe' @('/End', '/TN', $identity.TaskPath) $identity.InstallRoot 30000 | Out-Null }.GetNewClosure()
        StartTask = { param($identity) & $run 'schtasks.exe' @('/Run', '/TN', $identity.TaskPath) $identity.InstallRoot 30000 | Out-Null }.GetNewClosure()
        GetRenderEndpointNames = $getEndpoints
        CaptureFile = {
            param($path)
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
            $bytes = [System.IO.File]::ReadAllBytes($path)
            [pscustomobject]@{ Path = $path; Bytes = $bytes; Hash = ConvertTo-WifimicSha256 -Bytes $bytes }
        }.GetNewClosure()
        WriteTransactionBytes = {
            param($transactionRoot, $bytes)
            [System.IO.File]::WriteAllBytes((Join-Path $transactionRoot 'prior-client.exe'), [byte[]]$bytes)
        }.GetNewClosure()
        AtomicInstallFile = {
            param($source, $destination)
            if (-not [string]::Equals([System.IO.Path]::GetPathRoot($source), [System.IO.Path]::GetPathRoot($destination), [System.StringComparison]::OrdinalIgnoreCase)) {
                Throw-WifimicUpdaterError -Code 'CrossVolumeSwap' -Message 'The candidate and installed executable are not on the same volume.'
            }
            $temporary = $destination + '.wifimic-swap-' + [Guid]::NewGuid().ToString('N')
            try {
                [System.IO.File]::Copy($source, $temporary, $true)
                if ([System.IO.File]::Exists($destination)) {
                    [System.IO.File]::Replace($temporary, $destination, $null, $true)
                }
                else {
                    [System.IO.File]::Move($temporary, $destination)
                }
            }
            finally { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue }
        }.GetNewClosure()
        RestoreAtomicFile = {
            param($destination, $bytes)
            $temporary = $destination + '.wifimic-restore-' + [Guid]::NewGuid().ToString('N')
            try {
                [System.IO.File]::WriteAllBytes($temporary, [byte[]]$bytes)
                if ([System.IO.File]::Exists($destination)) {
                    [System.IO.File]::Replace($temporary, $destination, $null, $true)
                }
                else {
                    [System.IO.File]::Move($temporary, $destination)
                }
            }
            finally { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue }
        }.GetNewClosure()
        CheckHealth = {
            param($identity, $requireRunning)
            $task = & $getTask $identity
            $endpoints = & $getEndpoints
            $healthy = $null -ne $task -and [bool]$task.Enabled -and @('Ready', 'Running') -contains [string]$task.State -and $endpoints -contains $identity.Endpoint
            if ($requireRunning) { $healthy = $healthy -and [string]$task.State -eq 'Running' }
            [pscustomobject]@{ Healthy = $healthy; Enabled = if ($null -ne $task) { [bool]$task.Enabled } else { $false }; State = if ($null -ne $task) { [string]$task.State } else { 'Missing' } }
        }.GetNewClosure()
        Sleep = { param($milliseconds) Start-Sleep -Milliseconds $milliseconds }.GetNewClosure()
        RemoveDirectoryTree = { param($path) if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction Stop } }.GetNewClosure()
    }
}

function New-WifimicFakeOperations {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$StateRoot,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [string]$RequestedFailure
    )

    $installRoot = Join-Path $StateRoot 'install'
    $sourceRoot = Join-Path $StateRoot 'source'
    $canonicalRoot = $script:CanonicalInstallRoot
    $priorBytes = [System.Text.Encoding]::UTF8.GetBytes('prior-wifimic-client')
    $priorTaskXml = New-WifimicTaskXml -Identity $Identity
    $state = [pscustomobject]@{
        Task = ConvertTo-WifimicTaskDefinition -Identity $Identity -XmlText $priorTaskXml -Enabled $true -State 'Running'
        InstalledBytes = $priorBytes
        PriorBytes = $priorBytes
        PriorTaskXml = $priorTaskXml
        CandidateBytes = [System.Text.Encoding]::UTF8.GetBytes('candidate-wifimic-client')
        Endpoints = @($script:CanonicalEndpoint)
        Events = [System.Collections.ArrayList]::new()
        FailureConsumed = $false
        SourceDirty = $false
        WorktreeAdded = $false
        Fetched = $false
        HealthForcedFailure = $false
    }
    $mapPath = {
        param($path)
        if ([string]::Equals($path, $canonicalRoot, [System.StringComparison]::OrdinalIgnoreCase)) { return $installRoot }
        $prefix = $canonicalRoot.TrimEnd('\') + '\'
        if ($path.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) { return Join-Path $installRoot $path.Substring($prefix.Length) }
        if ([string]::Equals($path, $sourceRoot, [System.StringComparison]::OrdinalIgnoreCase)) { return $sourceRoot }
        return $path
    }.GetNewClosure()
    $record = { param($event) [void]$state.Events.Add($event) }.GetNewClosure()
    $failOnce = {
        param($point)
        if ($RequestedFailure -eq $point -and -not $state.FailureConsumed) {
            $state.FailureConsumed = $true
            Throw-WifimicUpdaterError -Code 'SimulatedFailure' -Message "Deterministic fake operation failure at '$point'."
        }
    }.GetNewClosure()
    $getTask = {
        param($identity)
        & $record 'GetTask'
        if ($null -eq $state.Task) { return $null }
        return ConvertTo-WifimicTaskDefinition -Identity $identity -XmlText $state.Task.XmlText -Enabled ([bool]$state.Task.Enabled) -State ([string]$state.Task.State)
    }.GetNewClosure()
    $capture = {
        param($path)
        $mapped = & $mapPath $path
        if ([string]::Equals($path, $Identity.ExecutablePath, [System.StringComparison]::OrdinalIgnoreCase)) {
            if ($null -eq $state.InstalledBytes) { return $null }
            return [pscustomobject]@{ Path = $path; Bytes = [byte[]]$state.InstalledBytes; Hash = ConvertTo-WifimicSha256 -Bytes ([byte[]]$state.InstalledBytes) }
        }
        if (-not (Test-Path -LiteralPath $mapped -PathType Leaf)) { return $null }
        $bytes = [System.IO.File]::ReadAllBytes($mapped)
        return [pscustomobject]@{ Path = $path; Bytes = $bytes; Hash = ConvertTo-WifimicSha256 -Bytes $bytes }
    }.GetNewClosure()

    [pscustomobject]@{
        GetSourceStatus = {
            param($root)
            & $record 'GetSourceStatus'
            & $failOnce 'DirtyCheckout'
            if ($state.SourceDirty) { return ' M source' }
            return ''
        }.GetNewClosure()
        FetchTags = {
            param($root)
            & $record 'FetchTags'
            & $failOnce 'Fetch'
            $state.Fetched = $true
        }.GetNewClosure()
        ResolveRevision = {
            param($root, $revision)
            & $record 'ResolveRevision'
            if (-not $state.Fetched) { Throw-WifimicUpdaterError -Code 'RevisionNotFetched' -Message 'The fake remote-only revision was resolved before FetchTags.' }
            & $failOnce 'BadRevision'
            if ($RequestedFailure -eq 'AmbiguousRevision') { return "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" }
            return 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        }.GetNewClosure()
        EnsureDirectory = { param($path) & $record 'EnsureDirectory'; New-Item -ItemType Directory -Path (& $mapPath $path) -Force | Out-Null }.GetNewClosure()
        AddDetachedWorktree = {
            param($root, $stage, $commit)
            & $record 'AddDetachedWorktree'
            & $failOnce 'Worktree'
            $state.WorktreeAdded = $true
            New-Item -ItemType Directory -Path (& $mapPath (Join-Path $stage 'target\release')) -Force | Out-Null
        }.GetNewClosure()
        BuildCandidate = {
            param($stage)
            & $record 'BuildCandidate'
            & $failOnce 'Build'
            $candidate = & $mapPath (Join-Path (Join-Path $stage 'target\release') $Identity.ExecutableName)
            New-Item -ItemType Directory -Path (Split-Path -Parent $candidate) -Force | Out-Null
            [System.IO.File]::WriteAllBytes($candidate, [byte[]]$state.CandidateBytes)
        }.GetNewClosure()
        RemoveDetachedWorktree = { param($root, $stage) & $record 'RemoveDetachedWorktree'; $state.WorktreeAdded = $false }.GetNewClosure()
        GetTask = $getTask
        RestoreTask = {
            param($identity, $xml, $enabled)
            & $record 'RestoreTask'
            & $failOnce 'RestoreTask'
            $state.Task = ConvertTo-WifimicTaskDefinition -Identity $identity -XmlText $xml -Enabled ([bool]$enabled) -State 'Ready'
        }.GetNewClosure()
        DisableTask = {
            param($identity)
            & $record 'DisableTask'
            & $failOnce 'DisableTask'
            $state.Task.Enabled = $false
        }.GetNewClosure()
        EnableTask = { param($identity) & $record 'EnableTask'; $state.Task.Enabled = $true }.GetNewClosure()
        StopTask = {
            param($identity)
            & $record 'StopTask'
            & $failOnce 'StopTask'
            $state.Task.State = 'Ready'
        }.GetNewClosure()
        StartTask = {
            param($identity)
            & $record 'StartTask'
            & $failOnce 'StartTask'
            $state.Task.State = 'Running'
        }.GetNewClosure()
        GetRenderEndpointNames = {
            & $record 'GetRenderEndpointNames'
            if ($RequestedFailure -eq 'Endpoint') { return @('CABLE Output (VB-Audio Virtual Cable)') }
            return @($state.Endpoints)
        }.GetNewClosure()
        CaptureFile = $capture
        WriteTransactionBytes = {
            param($root, $bytes)
            & $record 'WriteTransactionBytes'
            $mapped = & $mapPath (Join-Path $root 'prior-client.exe')
            New-Item -ItemType Directory -Path (Split-Path -Parent $mapped) -Force | Out-Null
            [System.IO.File]::WriteAllBytes($mapped, [byte[]]$bytes)
        }.GetNewClosure()
        AtomicInstallFile = {
            param($source, $destination)
            & $record 'AtomicInstallFile'
            & $failOnce 'AtomicSwap'
            $mappedSource = & $mapPath $source
            $mappedDestination = & $mapPath $destination
            $state.InstalledBytes = [System.IO.File]::ReadAllBytes($mappedSource)
            Remove-Item -LiteralPath $mappedSource -Force
        }.GetNewClosure()
        RestoreAtomicFile = {
            param($destination, $bytes)
            & $record 'RestoreAtomicFile'
            $state.InstalledBytes = [byte[]]$bytes
        }.GetNewClosure()
        CheckHealth = {
            param($identity, $requireRunning)
            & $record 'CheckHealth'
            if ($RequestedFailure -eq 'Health') { return [pscustomobject]@{ Healthy = $false; Enabled = [bool]$state.Task.Enabled; State = [string]$state.Task.State } }
            $healthy = [bool]$state.Task.Enabled -and @('Ready', 'Running') -contains [string]$state.Task.State -and $state.Endpoints -contains $identity.Endpoint
            if ($requireRunning) { $healthy = $healthy -and [string]$state.Task.State -eq 'Running' }
            return [pscustomobject]@{ Healthy = $healthy; Enabled = [bool]$state.Task.Enabled; State = [string]$state.Task.State }
        }.GetNewClosure()
        Sleep = { param($milliseconds) & $record 'Sleep' }.GetNewClosure()
        RemoveDirectoryTree = {
            param($path)
            & $record 'RemoveDirectoryTree'
            $mapped = & $mapPath $path
            if (Test-Path -LiteralPath $mapped) { Remove-Item -LiteralPath $mapped -Recurse -Force }
        }.GetNewClosure()
        GetState = { return $state }.GetNewClosure()
        GetSourceRoot = { return $sourceRoot }.GetNewClosure()
        GetInstallRoot = { return $installRoot }.GetNewClosure()
    }
}

function Get-WifimicTestStateRoot {
    [CmdletBinding()]
    param([string]$Requested)

    $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\', '/')
    $candidate = if ([string]::IsNullOrWhiteSpace($Requested)) {
        Join-Path $tempRoot ('wifimic-client-update-test-' + [Guid]::NewGuid().ToString('N'))
    }
    else { [System.IO.Path]::GetFullPath($Requested).TrimEnd('\', '/') }
    $prefix = $tempRoot + [System.IO.Path]::DirectorySeparatorChar
    if (-not $candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) -or [string]::Equals($candidate, $tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Throw-WifimicUpdaterError -Code 'InvalidTestRoot' -Message 'TestStateRoot must be a private child of the Windows temporary directory.'
    }
    if (Test-Path -LiteralPath $candidate) {
        Throw-WifimicUpdaterError -Code 'TestRootExists' -Message "TestStateRoot already exists: '$candidate'."
    }
    New-Item -ItemType Directory -Path $candidate -Force | Out-Null
    return $candidate
}

function Assert-WifimicHostMutationAllowed {
    [CmdletBinding()]
    param([switch]$ExplicitAcceptance)

    if (-not $ExplicitAcceptance) {
        Throw-WifimicUpdaterError -Code 'HostMutationNotExplicit' -Message 'Real task/file mutation requires -AcceptHostMutation; use -TestMode for deterministic isolated verification.'
    }
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        Throw-WifimicUpdaterError -Code 'WindowsOnly' -Message 'The native updater must run on Windows.'
    }
    if (-not [Environment]::UserInteractive) {
        Throw-WifimicUpdaterError -Code 'InteractiveSessionRequired' -Message 'The interactive client task must be updated from an interactive session.'
    }
    $principal = [Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Throw-WifimicUpdaterError -Code 'AdministratorRequired' -Message 'Administrator rights are required to stop and restart the Program Files Scheduled Task.'
    }
}

$testState = $null
$operations = $null
try {
    if ($TestMode -and $AcceptHostMutation) {
        Throw-WifimicUpdaterError -Code 'InvalidMode' -Message 'TestMode and AcceptHostMutation are mutually exclusive.'
    }
    if (-not $TestMode) {
        if ($FailurePoint) {
            Throw-WifimicUpdaterError -Code 'InvalidMode' -Message 'FailurePoint is available only in TestMode.'
        }
        Assert-WifimicHostMutationAllowed -ExplicitAcceptance:$AcceptHostMutation
    }

    $identity = Get-WifimicIdentity
    if ($TestMode) {
        $testState = Get-WifimicTestStateRoot -Requested $TestStateRoot
        $operations = New-WifimicFakeOperations -StateRoot $testState -Identity $identity -RequestedFailure $FailurePoint
        $sourceRoot = Invoke-WifimicOperation -Operations $operations -Name 'GetSourceRoot'
        $result = Invoke-WifimicClientUpdate -Revision $Tag -SourceRoot $sourceRoot -Operations $operations -Mode 'Test' -FailurePoint $FailurePoint
        $state = Invoke-WifimicOperation -Operations $operations -Name 'GetState'
        $result | Add-Member -NotePropertyName FakeEvents -NotePropertyValue @($state.Events)
        $result | Add-Member -NotePropertyName FakeInstalledHash -NotePropertyValue (ConvertTo-WifimicSha256 -Bytes ([byte[]]$state.InstalledBytes))
        $result | Add-Member -NotePropertyName FakeStateRoot -NotePropertyValue $testState
    }
    else {
        $sourceRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
        $result = Invoke-WifimicClientUpdate -Revision $Tag -SourceRoot $sourceRoot -Operations (New-WifimicNativeOperations) -Mode 'Native'
    }
    $result | ConvertTo-Json -Compress
    exit 0
}
catch {
    if ($TestMode -and $null -ne $operations) {
        try {
            $state = Invoke-WifimicOperation -Operations $operations -Name 'GetState'
            $priorHash = ConvertTo-WifimicSha256 -Bytes ([byte[]]$state.PriorBytes)
            $finalHash = ConvertTo-WifimicSha256 -Bytes ([byte[]]$state.InstalledBytes)
            $receipt = [pscustomobject]@{
                Deterministic = $true
                PriorHash = $priorHash
                FinalHash = $finalHash
                PriorExecutableRestored = $priorHash -eq $finalHash
                TaskPath = $script:CanonicalTaskPath
                TaskEnabled = [bool]$state.Task.Enabled
                TaskState = [string]$state.Task.State
                TaskXmlPreserved = [string]::Equals($state.Task.XmlText, [string]$state.PriorTaskXml, [System.StringComparison]::Ordinal)
                StagingAndTransactionClean = @(Get-ChildItem -LiteralPath (Join-Path $testState 'install') -Force -ErrorAction SilentlyContinue).Count -eq 0
                Events = @($state.Events)
            }
            [Console]::Error.WriteLine("TestModeStateReceipt: $($receipt | ConvertTo-Json -Compress)")
        }
        catch { }
    }
    [Console]::Error.WriteLine("wifimic-client updater failed: $($_.Exception.Message)")
    exit 1
}
finally {
    if ($null -ne $testState -and (Test-Path -LiteralPath $testState)) {
        Remove-Item -LiteralPath $testState -Recurse -Force -ErrorAction SilentlyContinue
    }
}
