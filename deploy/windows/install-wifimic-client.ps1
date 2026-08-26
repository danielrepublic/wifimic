[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ClientExecutable,
    [string]$RenderEndpoint = 'CABLE Input (VB-Audio Virtual Cable)',
    [switch]$TestMode,
    [switch]$DryRun,
    [switch]$AcceptHostMutation,
    [string]$TestStateRoot,
    [string[]]$FakeRenderEndpoints = @('CABLE Input (VB-Audio Virtual Cable)'),
    [ValidateSet('BeforeTask', 'AfterExecutableCopy', 'AfterTask', 'BeforeFirewall', 'AfterFirewall', 'BeforeVerification')]
    [string]$FailurePoint
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:CanonicalInstallRoot = 'C:\Program Files\wifimic-client'
$script:CanonicalExecutableName = 'wifimic_client.exe'
$script:CanonicalUpdaterExecutableName = 'wifimic_client_updater.exe'
$script:CanonicalTaskFolder = '\wifimic\'
$script:CanonicalTaskName = 'wifimic-client'
$script:CanonicalTaskPath = '\wifimic\wifimic-client'
$script:CanonicalFirewallDisplayName = 'wifimic-client'
$script:CanonicalPeer = '192.168.0.210/32'
$script:CanonicalPort = '6902'
$script:CanonicalEndpoint = 'CABLE Input (VB-Audio Virtual Cable)'
$script:CanonicalMarkerFileName = 'test.md'

function New-WifimicInstallerException {
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

function Throw-WifimicInstallerError {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Code,
        [Parameter(Mandatory = $true)][string]$Message
    )

    throw (New-WifimicInstallerException -Code $Code -Message $Message)
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
        Throw-WifimicInstallerError -Code 'MissingOperation' -Message "Native operation '$Name' is not configured."
    }
    return & $operation @Arguments
}

function Get-WifimicIdentity {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Endpoint)

    if ([string]::IsNullOrWhiteSpace($Endpoint)) {
        Throw-WifimicInstallerError -Code 'InvalidEndpoint' -Message 'RenderEndpoint is required.'
    }
    if (-not [string]::Equals($Endpoint, $script:CanonicalEndpoint, [System.StringComparison]::Ordinal)) {
        Throw-WifimicInstallerError -Code 'UnsupportedEndpoint' -Message "The client is pinned to the exact render endpoint '$($script:CanonicalEndpoint)'."
    }

    [pscustomobject]@{
        InstallRoot = $script:CanonicalInstallRoot
        ExecutableName = $script:CanonicalExecutableName
        ExecutablePath = Join-Path $script:CanonicalInstallRoot $script:CanonicalExecutableName
        UpdaterExecutableName = $script:CanonicalUpdaterExecutableName
        UpdaterExecutablePath = Join-Path $script:CanonicalInstallRoot $script:CanonicalUpdaterExecutableName
        TaskFolder = $script:CanonicalTaskFolder
        TaskName = $script:CanonicalTaskName
        TaskPath = $script:CanonicalTaskPath
        FirewallDisplayName = $script:CanonicalFirewallDisplayName
        PeerAddress = $script:CanonicalPeer
        Port = $script:CanonicalPort
        Endpoint = $Endpoint
        TaskUri = $script:CanonicalTaskPath
        Description = 'Interactive wifimic client.'
        MarkerFileName = $script:CanonicalMarkerFileName
        MarkerFilePath = Join-Path $script:CanonicalInstallRoot $script:CanonicalMarkerFileName
    }
}

function Resolve-WifimicClientExecutable {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        Throw-WifimicInstallerError -Code 'InvalidExecutable' -Message 'ClientExecutable is required.'
    }
    try {
        $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).ProviderPath
    }
    catch {
        throw (New-WifimicInstallerException -Code 'ExecutableNotFound' -Message "Client executable was not found: '$Path'." -InnerException $_.Exception)
    }
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        Throw-WifimicInstallerError -Code 'ExecutableNotFound' -Message "Client executable was not a file: '$Path'."
    }
    if (-not [string]::Equals([System.IO.Path]::GetFileName($resolved), $script:CanonicalExecutableName, [System.StringComparison]::OrdinalIgnoreCase)) {
        Throw-WifimicInstallerError -Code 'InvalidExecutableName' -Message "ClientExecutable must be named '$($script:CanonicalExecutableName)'."
    }
    return $resolved
}

function Get-WifimicTaskXmlNode {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][xml]$Xml,
        [Parameter(Mandatory = $true)][string]$XPath
    )

    $namespace = New-Object System.Xml.XmlNamespaceManager($Xml.NameTable)
    $namespace.AddNamespace('task', 'http://schemas.microsoft.com/windows/2004/02/mit/task')
    $node = $Xml.SelectSingleNode($XPath, $namespace)
    if ($null -eq $node) {
        Throw-WifimicInstallerError -Code 'InvalidTaskXml' -Message "Task XML is missing '$XPath'."
    }
    return $node
}

function Get-WifimicTaskXmlNodeOptional {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][xml]$Xml,
        [Parameter(Mandatory = $true)][string]$XPath
    )

    $namespace = New-Object System.Xml.XmlNamespaceManager($Xml.NameTable)
    $namespace.AddNamespace('task', 'http://schemas.microsoft.com/windows/2004/02/mit/task')
    return $Xml.SelectSingleNode($XPath, $namespace)
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
<Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task" version="1.4">
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
        [bool]$Enabled = $true
    )

    [xml]$xml = $XmlText
    $namespace = New-Object System.Xml.XmlNamespaceManager($xml.NameTable)
    $namespace.AddNamespace('task', 'http://schemas.microsoft.com/windows/2004/02/mit/task')
    $getNode = {
        param([string]$XPath)
        $node = $xml.SelectSingleNode($XPath, $namespace)
        if ($null -eq $node) { throw "Task XML is missing '$XPath'." }
        return $node
    }.GetNewClosure()
    $command = (& $getNode '//task:Actions/task:Exec/task:Command').InnerText
    $workingDirectory = (& $getNode '//task:Actions/task:Exec/task:WorkingDirectory').InnerText
    $argumentsNode = $xml.SelectSingleNode('//task:Actions/task:Exec/task:Arguments', $namespace)
    $arguments = if ($null -eq $argumentsNode) { '' } else { $argumentsNode.InnerText }
    $uri = (& $getNode '//task:RegistrationInfo/task:URI').InnerText
    $principal = & $getNode '//task:Principals/task:Principal'
    $principalId = $principal.Attributes['id'].Value
    $logonType = (& $getNode '//task:Principals/task:Principal/task:LogonType').InnerText

    [pscustomobject]@{
        TaskPath = $Identity.TaskPath
        XmlText = $XmlText
        Enabled = $Enabled
        Signature = [pscustomobject]@{
            Uri = $uri
            ExecutablePath = $command
            WorkingDirectory = $workingDirectory
            Arguments = $arguments
            Principal = $principalId
            LogonType = $logonType
        }
    }
}

function Test-WifimicTaskDefinition {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Definition,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity
    )

    $xml = [xml]$Definition.XmlText
    $trigger = Get-WifimicTaskXmlNode -Xml $xml -XPath '//task:Triggers/task:LogonTrigger'
    if ($trigger.LocalName -ne 'LogonTrigger') {
        Throw-WifimicInstallerError -Code 'TaskContractMismatch' -Message 'The task must use a LogonTrigger.'
    }
    $principal = Get-WifimicTaskXmlNode -Xml $xml -XPath '//task:Principals/task:Principal'
    $logonType = (Get-WifimicTaskXmlNode -Xml $xml -XPath '//task:Principals/task:Principal/task:LogonType').InnerText
    if ($principal.Attributes['id'].Value -ne 'Author' -or $logonType -ne 'InteractiveToken') {
        Throw-WifimicInstallerError -Code 'TaskContractMismatch' -Message 'The task must use the Author principal with InteractiveToken.'
    }
    $namespace = New-Object System.Xml.XmlNamespaceManager($xml.NameTable)
    $namespace.AddNamespace('task', 'http://schemas.microsoft.com/windows/2004/02/mit/task')
    $passwordNode = $xml.SelectSingleNode('//task:Principals/task:Principal/task:Password', $namespace)
    if ($null -ne $passwordNode -or $logonType -in @('ServiceAccount', 'S4U', 'Batch')) {
        Throw-WifimicInstallerError -Code 'CredentialPersistence' -Message 'The task may not contain a password or a non-interactive logon type.'
    }
    if (-not [string]::Equals($Definition.TaskPath, $Identity.TaskPath, [System.StringComparison]::Ordinal) -or
        -not [string]::Equals($Definition.Signature.Uri, $Identity.TaskUri, [System.StringComparison]::Ordinal) -or
        -not [string]::Equals($Definition.Signature.ExecutablePath, $Identity.ExecutablePath, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($Definition.Signature.WorkingDirectory, $Identity.InstallRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not [string]::Equals($Definition.Signature.Arguments, '', [System.StringComparison]::Ordinal)) {
        Throw-WifimicInstallerError -Code 'TaskContractMismatch' -Message 'The task executable, path, working directory, or arguments are not canonical.'
    }
    if ($Definition.Signature.ExecutablePath -match '(?i)(powershell|cmd)\.exe') {
        Throw-WifimicInstallerError -Code 'ShellWrapperRejected' -Message 'The task must launch the client executable directly.'
    }
    return $true
}

function New-WifimicFirewallSignature {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][pscustomobject]$Identity)

    [pscustomobject]@{
        Name = $Identity.FirewallDisplayName
        DisplayName = $Identity.FirewallDisplayName
        Protocol = 'UDP'
        LocalPort = $Identity.Port
        RemoteAddress = $Identity.PeerAddress
        Profile = 'Any'
        Direction = 'Inbound'
        Action = 'Allow'
        Enabled = 'True'
    }
}

function ConvertTo-WifimicFirewallAddressList {
    [CmdletBinding()]
    param([object]$Value)

    @(
        foreach ($item in @($Value)) {
            if ($null -eq $item) {
                ''
                continue
            }
            foreach ($candidate in ([string]$item).Split(',')) {
                $address = $candidate.Trim()
                $slash = $address.IndexOf('/')
                $hostPart = if ($slash -ge 0) { $address.Substring(0, $slash).Trim() } else { $address }
                $prefix = if ($slash -ge 0) { $address.Substring($slash + 1).Trim() } else { $null }
                $parsed = $null
                if ([System.Net.IPAddress]::TryParse($hostPart, [ref]$parsed) -and
                    $parsed.AddressFamily -eq [System.Net.Sockets.AddressFamily]::InterNetwork -and
                    ($slash -lt 0 -or $prefix -eq '32')) {
                    '{0}/32' -f $parsed.ToString()
                }
                else {
                    $address
                }
            }
        }
    )
}

function Test-WifimicFirewallSignature {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Actual,
        [Parameter(Mandatory = $true)][pscustomobject]$Expected
    )

    $actualAddresses = @(ConvertTo-WifimicFirewallAddressList -Value $Actual.RemoteAddress)
    $expectedAddresses = @(ConvertTo-WifimicFirewallAddressList -Value $Expected.RemoteAddress)
    $peerAddressMatches = $actualAddresses.Count -eq 1 -and
        $expectedAddresses.Count -eq 1 -and
        [string]::Equals($actualAddresses[0], $expectedAddresses[0], [System.StringComparison]::OrdinalIgnoreCase)

    return [string]::Equals($Actual.Name, $Expected.Name, [System.StringComparison]::Ordinal) -and
        [string]::Equals($Actual.DisplayName, $Expected.DisplayName, [System.StringComparison]::Ordinal) -and
        [string]::Equals($Actual.Protocol, $Expected.Protocol, [System.StringComparison]::OrdinalIgnoreCase) -and
        [string]::Equals([string]$Actual.LocalPort, [string]$Expected.LocalPort, [System.StringComparison]::Ordinal) -and
        $peerAddressMatches -and
        [string]::Equals($Actual.Profile, $Expected.Profile, [System.StringComparison]::OrdinalIgnoreCase) -and
        [string]::Equals($Actual.Direction, $Expected.Direction, [System.StringComparison]::OrdinalIgnoreCase) -and
        [string]::Equals($Actual.Action, $Expected.Action, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-WifimicFileCaptureEqual {
    [CmdletBinding()]
    param(
        [pscustomobject]$Actual,
        [pscustomobject]$Expected
    )

    if ($null -eq $Actual -or $null -eq $Expected) {
        return $null -eq $Actual -and $null -eq $Expected
    }
    if ($Actual.Bytes.Length -ne $Expected.Bytes.Length) {
        return $false
    }
    for ($index = 0; $index -lt $Actual.Bytes.Length; $index++) {
        if ($Actual.Bytes[$index] -ne $Expected.Bytes[$index]) {
            return $false
        }
    }
    return $true
}

function Assert-WifimicPreexistingState {
    [CmdletBinding()]
    param(
        [pscustomobject]$Task,
        [pscustomobject]$Firewall,
        [pscustomobject]$Executable,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [Parameter(Mandatory = $true)][pscustomobject]$ExpectedFirewall
    )

    if ($null -ne $Task) {
        try { [void](Test-WifimicTaskDefinition -Definition $Task -Identity $Identity) }
        catch { throw (New-WifimicInstallerException -Code 'ConflictingTask' -Message "Existing task '$($Identity.TaskPath)' is not an owned canonical task." -InnerException $_.Exception) }
        if ($null -eq $Executable) {
            Throw-WifimicInstallerError -Code 'ConflictingTask' -Message "Existing task '$($Identity.TaskPath)' has no executable to preserve for rollback."
        }
    }
    if ($null -ne $Firewall -and -not (Test-WifimicFirewallSignature -Actual $Firewall -Expected $ExpectedFirewall)) {
        Throw-WifimicInstallerError -Code 'ConflictingFirewall' -Message "Existing firewall rule '$($Identity.FirewallDisplayName)' is not an owned UDP/$($Identity.Port) peer-scoped rule."
    }
}

function Invoke-WifimicFailurePoint {
    [CmdletBinding()]
    param(
        [string]$Requested,
        [Parameter(Mandatory = $true)][string]$Point
    )

    if ($Requested -eq $Point) {
        Throw-WifimicInstallerError -Code 'SimulatedFailure' -Message "Simulated installer failure at '$Point'."
    }
}

function Restore-WifimicTransaction {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [Parameter(Mandatory = $true)][pscustomobject]$Identity,
        [pscustomobject]$PriorTask,
        [pscustomobject]$PriorFirewall,
        [pscustomobject]$PriorExecutable,
        [pscustomobject]$PriorUpdater,
        [pscustomobject]$PriorMarker,
        [bool]$TaskChanged,
        [bool]$FirewallChanged,
        [bool]$ExecutableChanged,
        [bool]$UpdaterChanged,
        [bool]$MarkerChanged,
        [bool]$InstallRootCreated
    )

    $errors = [System.Collections.ArrayList]::new()
    try {
        if ($TaskChanged) {
            if ($null -eq $PriorTask) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveTask' -Arguments @($Identity) | Out-Null
            }
            else {
                Invoke-WifimicOperation -Operations $Operations -Name 'RestoreTask' -Arguments @($Identity, $PriorTask.XmlText, [bool]$PriorTask.Enabled) | Out-Null
            }
        }
    }
    catch { [void]$errors.Add("task: $($_.Exception.Message)") }

    try {
        if ($FirewallChanged) {
            if ($null -eq $PriorFirewall) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveFirewall' -Arguments @($Identity) | Out-Null
            }
            else {
                Invoke-WifimicOperation -Operations $Operations -Name 'SetFirewall' -Arguments @($PriorFirewall) | Out-Null
            }
        }
    }
    catch { [void]$errors.Add("firewall: $($_.Exception.Message)") }

    try {
        if ($ExecutableChanged) {
            if ($null -eq $PriorExecutable) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveFile' -Arguments @($Identity.ExecutablePath) | Out-Null
            }
            else {
                Invoke-WifimicOperation -Operations $Operations -Name 'RestoreFile' -Arguments @($Identity.ExecutablePath, $PriorExecutable.Bytes) | Out-Null
            }
        }
        if ($UpdaterChanged) {
            if ($null -eq $PriorUpdater) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveFile' -Arguments @($Identity.UpdaterExecutablePath) | Out-Null
            }
            else {
                Invoke-WifimicOperation -Operations $Operations -Name 'RestoreFile' -Arguments @($Identity.UpdaterExecutablePath, $PriorUpdater.Bytes) | Out-Null
            }
        }
        if ($InstallRootCreated) {
            Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDirectoryIfEmpty' -Arguments @($Identity.InstallRoot) | Out-Null
        }
    }
    catch { [void]$errors.Add("files: $($_.Exception.Message)") }

    try {
        if ($MarkerChanged) {
            if ($null -eq $PriorMarker) {
                Invoke-WifimicOperation -Operations $Operations -Name 'RemoveFile' -Arguments @($Identity.MarkerFilePath) | Out-Null
            }
            else {
                Invoke-WifimicOperation -Operations $Operations -Name 'RestoreFile' -Arguments @($Identity.MarkerFilePath, $PriorMarker.Bytes) | Out-Null
            }
        }
    }
    catch { [void]$errors.Add("marker: $($_.Exception.Message)") }

    try {
        $currentTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($Identity)
        if ($null -eq $PriorTask) {
            if ($null -ne $currentTask) { Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The new task remained after rollback.' }
        }
        elseif (-not (Test-WifimicTaskDefinition -Definition $currentTask -Identity $Identity) -or
            [bool]$currentTask.Enabled -ne [bool]$PriorTask.Enabled) {
            Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The prior task was not restored exactly.'
        }

        $currentFirewall = Invoke-WifimicOperation -Operations $Operations -Name 'GetFirewall' -Arguments @($Identity)
        if ($null -eq $PriorFirewall) {
            if ($null -ne $currentFirewall) { Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The new firewall rule remained after rollback.' }
        }
        elseif (-not (Test-WifimicFirewallSignature -Actual $currentFirewall -Expected $PriorFirewall)) {
            Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The prior firewall rule was not restored exactly.'
        }

        $currentExecutable = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($Identity.ExecutablePath)
        if (-not (Test-WifimicFileCaptureEqual -Actual $currentExecutable -Expected $PriorExecutable)) {
            Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The prior executable was not restored exactly.'
        }

        $currentUpdater = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($Identity.UpdaterExecutablePath)
        if (-not (Test-WifimicFileCaptureEqual -Actual $currentUpdater -Expected $PriorUpdater)) {
            Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The prior updater executable was not restored exactly.'
        }

        $currentMarker = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($Identity.MarkerFilePath)
        if (-not (Test-WifimicFileCaptureEqual -Actual $currentMarker -Expected $PriorMarker)) {
            Throw-WifimicInstallerError -Code 'RollbackVerification' -Message 'The prior marker file was not restored exactly.'
        }
    }
    catch { [void]$errors.Add("verification: $($_.Exception.Message)") }

    if ($errors.Count -gt 0) {
        Throw-WifimicInstallerError -Code 'RollbackFailed' -Message ($errors -join '; ')
    }
}

function Invoke-WifimicInstall {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ClientExecutable,
        [Parameter(Mandatory = $true)][string]$RenderEndpoint,
        [Parameter(Mandatory = $true)][pscustomobject]$Operations,
        [switch]$DryRun,
        [string]$FailurePoint,
        [string]$Mode = 'Native'
    )

    $identity = Get-WifimicIdentity -Endpoint $RenderEndpoint
    $source = Resolve-WifimicClientExecutable -Path $ClientExecutable
    $updaterSource = Join-Path (Split-Path -Parent $source) $script:CanonicalUpdaterExecutableName
    if (-not (Test-Path -LiteralPath $updaterSource -PathType Leaf)) {
        Throw-WifimicInstallerError -Code 'MissingUpdater' -Message "Updater executable was not found: '$updaterSource'."
    }
    $firewall = New-WifimicFirewallSignature -Identity $identity
    $priorTask = $null
    $priorFirewall = $null
    $priorExecutable = $null
    $priorUpdater = $null
    $priorMarker = $null
    $taskChanged = $false
    $firewallChanged = $false
    $executableChanged = $false
    $updaterChanged = $false
    $markerChanged = $false
    $installRootCreated = $false
    $stageRoot = $null

    try {
        $priorTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
        $priorFirewall = Invoke-WifimicOperation -Operations $Operations -Name 'GetFirewall' -Arguments @($identity)
        $priorExecutable = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.ExecutablePath)
        $priorUpdater = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.UpdaterExecutablePath)
        $priorMarker = Invoke-WifimicOperation -Operations $Operations -Name 'CaptureFile' -Arguments @($identity.MarkerFilePath)
        Assert-WifimicPreexistingState -Task $priorTask -Firewall $priorFirewall -Executable $priorExecutable -Identity $identity -ExpectedFirewall $firewall

        $endpointNames = @(Invoke-WifimicOperation -Operations $Operations -Name 'GetRenderEndpointNames')
        if ($endpointNames -notcontains $identity.Endpoint) {
            $available = if ($endpointNames.Count -eq 0) { '<none>' } else { $endpointNames -join ', ' }
            Throw-WifimicInstallerError -Code 'EndpointNotFound' -Message "Exact render endpoint '$($identity.Endpoint)' was not enumerated. Available render endpoints: $available"
        }

        if ($DryRun) {
            return [pscustomobject]@{
                Status = 'Validated'
                Mode = 'DryRun'
                InstallRoot = $identity.InstallRoot
                TaskPath = $identity.TaskPath
                FirewallDisplayName = $identity.FirewallDisplayName
                RemoteAddress = $identity.PeerAddress
                Protocol = $firewall.Protocol
                Port = $identity.Port
                Endpoint = $identity.Endpoint
                LogonTrigger = 'LogonTrigger'
                LogonType = 'InteractiveToken'
            }
        }

        $taskXml = New-WifimicTaskXml -Identity $identity
        $newDefinition = ConvertTo-WifimicTaskDefinition -Identity $identity -XmlText $taskXml -Enabled $true
        Test-WifimicTaskDefinition -Definition $newDefinition -Identity $identity | Out-Null
        $rootExists = Invoke-WifimicOperation -Operations $Operations -Name 'DirectoryExists' -Arguments @($identity.InstallRoot)
        if (-not $rootExists) {
            $installRootCreated = $true
        }
        $stageRoot = Join-Path $identity.InstallRoot ('.wifimic-stage-' + [Guid]::NewGuid().ToString('N'))
        Invoke-WifimicOperation -Operations $Operations -Name 'EnsureDirectory' -Arguments @($identity.InstallRoot) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'EnsureDirectory' -Arguments @($stageRoot) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @($source, (Join-Path $stageRoot $identity.ExecutableName)) | Out-Null
        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'BeforeTask'
        Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @((Join-Path $stageRoot $identity.ExecutableName), $identity.ExecutablePath) | Out-Null
        $executableChanged = $true
        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'AfterExecutableCopy'
        Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @($updaterSource, (Join-Path $stageRoot $identity.UpdaterExecutableName)) | Out-Null
        Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @((Join-Path $stageRoot $identity.UpdaterExecutableName), $identity.UpdaterExecutablePath) | Out-Null
        $updaterChanged = $true

        $markerSource = Join-Path (Split-Path -Parent $source) $identity.MarkerFileName
        if (Test-Path -LiteralPath $markerSource -PathType Leaf) {
            Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @($markerSource, (Join-Path $stageRoot $identity.MarkerFileName)) | Out-Null
            Invoke-WifimicOperation -Operations $Operations -Name 'CopyFile' -Arguments @((Join-Path $stageRoot $identity.MarkerFileName), $identity.MarkerFilePath) | Out-Null
            $markerChanged = $true
        }

        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'BeforeTask'
        $taskChanged = $true
        Invoke-WifimicOperation -Operations $Operations -Name 'SetTask' -Arguments @($identity, $taskXml, $true) | Out-Null
        $registeredTask = Invoke-WifimicOperation -Operations $Operations -Name 'GetTask' -Arguments @($identity)
        Test-WifimicTaskDefinition -Definition $registeredTask -Identity $identity | Out-Null
        if (-not [bool]$registeredTask.Enabled) {
            Throw-WifimicInstallerError -Code 'TaskContractMismatch' -Message 'The installed task was not enabled.'
        }
        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'AfterTask'

        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'BeforeFirewall'
        $firewallChanged = $true
        Invoke-WifimicOperation -Operations $Operations -Name 'SetFirewall' -Arguments @($firewall) | Out-Null
        $registeredFirewall = Invoke-WifimicOperation -Operations $Operations -Name 'GetFirewall' -Arguments @($identity)
        if (-not (Test-WifimicFirewallSignature -Actual $registeredFirewall -Expected $firewall)) {
            Throw-WifimicInstallerError -Code 'FirewallContractMismatch' -Message 'The installed firewall rule did not match the exact peer-scoped UDP contract.'
        }
        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'AfterFirewall'
        Invoke-WifimicFailurePoint -Requested $FailurePoint -Point 'BeforeVerification'

        [pscustomobject]@{
            Status = 'Installed'
            Mode = $Mode
            InstallRoot = $identity.InstallRoot
            ExecutablePath = $identity.ExecutablePath
            MarkerFilePath = if ($markerChanged) { $identity.MarkerFilePath } else { $null }
            TaskFolder = $identity.TaskFolder
            TaskName = $identity.TaskName
            TaskPath = $identity.TaskPath
            FirewallDisplayName = $firewall.DisplayName
            RemoteAddress = $firewall.RemoteAddress
            Protocol = $firewall.Protocol
            Port = $firewall.LocalPort
            Endpoint = $identity.Endpoint
            LogonTrigger = 'LogonTrigger'
            LogonType = 'InteractiveToken'
        }
    }
    catch {
        $failureRecord = $_
        $failure = $_.Exception
        try {
            Restore-WifimicTransaction -Operations $Operations -Identity $identity -PriorTask $priorTask -PriorFirewall $priorFirewall -PriorExecutable $priorExecutable -PriorUpdater $priorUpdater -PriorMarker $priorMarker -TaskChanged $taskChanged -FirewallChanged $firewallChanged -ExecutableChanged $executableChanged -UpdaterChanged $updaterChanged -MarkerChanged $markerChanged -InstallRootCreated $installRootCreated
        }
        catch {
            throw (New-WifimicInstallerException -Code 'RollbackFailed' -Message "Install failed with '$($failure.Message)' at '$($failureRecord.ScriptStackTrace)' and rollback failed with '$($_.Exception.Message)' at '$($_.ScriptStackTrace)'." -InnerException $failure)
        }
        throw $failure
    }
    finally {
        if ($null -ne $stageRoot) {
            Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDirectory' -Arguments @($stageRoot) | Out-Null
        }
        if ($installRootCreated) {
            Invoke-WifimicOperation -Operations $Operations -Name 'RemoveDirectoryIfEmpty' -Arguments @($identity.InstallRoot) | Out-Null
        }
    }
}

function New-WifimicNativeOperations {
    [CmdletBinding()]
    param()

    $convertTaskDefinition = ${function:ConvertTo-WifimicTaskDefinition}.GetNewClosure()
    $setTask = {
        param($identity, $xmlText, $enabled)
        $temporary = Join-Path ([System.IO.Path]::GetTempPath()) ('wifimic-client-task-' + [Guid]::NewGuid().ToString('N') + '.xml')
        try {
            [System.IO.File]::WriteAllText($temporary, $xmlText, [System.Text.Encoding]::Unicode)
            & schtasks.exe /Create /TN $identity.TaskPath /XML $temporary /F | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "schtasks /Create failed with exit code $LASTEXITCODE." }
            $stateSwitch = if ($enabled) { '/ENABLE' } else { '/DISABLE' }
            & schtasks.exe /Change /TN $identity.TaskPath $stateSwitch | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "schtasks /Change $stateSwitch failed with exit code $LASTEXITCODE."
            }
        }
        finally { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue }
    }.GetNewClosure()

    [pscustomobject]@{
        GetTask = {
            param($identity)
            $task = Get-ScheduledTask -TaskPath $identity.TaskFolder -TaskName $identity.TaskName -ErrorAction SilentlyContinue
            if ($null -eq $task) { return $null }
            $xml = Export-ScheduledTask -TaskPath $identity.TaskFolder -TaskName $identity.TaskName -ErrorAction Stop
            return & $convertTaskDefinition -Identity $identity -XmlText $xml -Enabled ([bool]$task.Settings.Enabled)
        }.GetNewClosure()
        SetTask = $setTask
        RestoreTask = {
            param($identity, $xmlText, $enabled)
            & $setTask $identity $xmlText $true
            if (-not $enabled) {
                & schtasks.exe /Change /TN $identity.TaskPath /DISABLE | Out-Null
                if ($LASTEXITCODE -ne 0) { throw "schtasks /Change /DISABLE failed with exit code $LASTEXITCODE." }
            }
        }.GetNewClosure()
        RemoveTask = {
            param($identity)
            $task = Get-ScheduledTask -TaskPath $identity.TaskFolder -TaskName $identity.TaskName -ErrorAction SilentlyContinue
            if ($null -eq $task) { return }
            & schtasks.exe /Delete /TN $identity.TaskPath /F | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "schtasks /Delete failed with exit code $LASTEXITCODE." }
        }
        GetFirewall = {
            param($identity)
            $rules = @(Get-NetFirewallRule -DisplayName $identity.FirewallDisplayName -ErrorAction SilentlyContinue)
            if ($rules.Count -eq 0) { return $null }
            if ($rules.Count -ne 1) { throw "Firewall DisplayName '$($identity.FirewallDisplayName)' has duplicate rules." }
            $rule = $rules[0]
            $port = Get-NetFirewallPortFilter -AssociatedNetFirewallRule $rule -ErrorAction Stop
            $address = Get-NetFirewallAddressFilter -AssociatedNetFirewallRule $rule -ErrorAction Stop
            return [pscustomobject]@{
                Name = [string]$rule.Name
                DisplayName = [string]$rule.DisplayName
                Protocol = [string]$port.Protocol
                LocalPort = [string]$port.LocalPort
                RemoteAddress = @($address.RemoteAddress) -join ','
                Profile = [string]$rule.Profile
                Direction = [string]$rule.Direction
                Action = [string]$rule.Action
                Enabled = [string]$rule.Enabled
            }
        }
        SetFirewall = {
            param($signature)
            @(Get-NetFirewallRule -DisplayName $signature.DisplayName -ErrorAction SilentlyContinue) | Remove-NetFirewallRule -ErrorAction Stop
            New-NetFirewallRule -Name $signature.Name -DisplayName $signature.DisplayName -Protocol $signature.Protocol -LocalPort $signature.LocalPort -RemoteAddress $signature.RemoteAddress -Profile $signature.Profile -Direction $signature.Direction -Action $signature.Action -Enabled True -ErrorAction Stop | Out-Null
        }
        RemoveFirewall = {
            param($identity)
            @(Get-NetFirewallRule -DisplayName $identity.FirewallDisplayName -ErrorAction SilentlyContinue) | Remove-NetFirewallRule -ErrorAction SilentlyContinue
        }
        GetRenderEndpointNames = {
            @(Get-PnpDevice -Class AudioEndpoint -Status OK -ErrorAction Stop | ForEach-Object { [string]$_.FriendlyName })
        }
        DirectoryExists = { param($path) Test-Path -LiteralPath $path -PathType Container }
        EnsureDirectory = { param($path) New-Item -ItemType Directory -Path $path -Force | Out-Null }
        CopyFile = { param($source, $destination) Copy-Item -LiteralPath $source -Destination $destination -Force }
        CaptureFile = {
            param($path)
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
            [pscustomobject]@{ Path = $path; Bytes = [System.IO.File]::ReadAllBytes($path) }
        }
        RestoreFile = {
            param($path, $bytes)
            $parent = Split-Path -Parent $path
            New-Item -ItemType Directory -Path $parent -Force | Out-Null
            [System.IO.File]::WriteAllBytes($path, $bytes)
        }
        RemoveFile = { param($path) Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue }
        RemoveDirectory = {
            param($path)
            if (Test-Path -LiteralPath $path -PathType Container) {
                Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction Stop
            }
        }
        RemoveDirectoryIfEmpty = {
            param($path)
            if (Test-Path -LiteralPath $path -PathType Container) {
                if (@(Get-ChildItem -LiteralPath $path -Force).Count -eq 0) { Remove-Item -LiteralPath $path -Force }
            }
        }
    }
}

function New-WifimicFakeOperations {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$StateRoot,
        [Parameter(Mandatory = $true)][string[]]$EndpointNames
    )

    $stateInstallRoot = Join-Path $StateRoot 'install'
    $canonicalRoot = $script:CanonicalInstallRoot
    $convertTaskDefinition = ${function:ConvertTo-WifimicTaskDefinition}.GetNewClosure()
    $state = [pscustomobject]@{
        Task = $null
        Firewall = $null
        Events = [System.Collections.ArrayList]::new()
    }
    $mapPath = {
        param($path)
        if ([string]::Equals($path, $canonicalRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $stateInstallRoot
        }
        $prefix = $canonicalRoot.TrimEnd('\') + '\'
        if ($path.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return Join-Path $stateInstallRoot $path.Substring($prefix.Length)
        }
        return $path
    }.GetNewClosure()
    $record = {
        param($name)
        [void]$state.Events.Add($name)
    }.GetNewClosure()

    return [pscustomobject]@{
        GetTask = { param($identity) & $record 'GetTask'; return $state.Task }.GetNewClosure()
        SetTask = {
            param($identity, $xmlText, $enabled)
            & $record 'SetTask'
            $state.Task = & $convertTaskDefinition -Identity $identity -XmlText $xmlText -Enabled ([bool]$enabled)
        }.GetNewClosure()
        RestoreTask = {
            param($identity, $xmlText, $enabled)
            & $record 'RestoreTask'
            $state.Task = & $convertTaskDefinition -Identity $identity -XmlText $xmlText -Enabled ([bool]$enabled)
        }.GetNewClosure()
        RemoveTask = { param($identity) & $record 'RemoveTask'; $state.Task = $null }.GetNewClosure()
        GetFirewall = { param($identity) & $record 'GetFirewall'; return $state.Firewall }.GetNewClosure()
        SetFirewall = {
            param($signature)
            & $record 'SetFirewall'
            $state.Firewall = [pscustomobject]@{
                Name = $signature.Name
                DisplayName = $signature.DisplayName
                Protocol = $signature.Protocol
                LocalPort = $signature.LocalPort
                RemoteAddress = $signature.RemoteAddress
                Profile = $signature.Profile
                Direction = $signature.Direction
                Action = $signature.Action
                Enabled = $signature.Enabled
            }
        }.GetNewClosure()
        RemoveFirewall = { param($identity) & $record 'RemoveFirewall'; $state.Firewall = $null }.GetNewClosure()
        GetRenderEndpointNames = { & $record 'GetRenderEndpointNames'; return $EndpointNames }.GetNewClosure()
        DirectoryExists = { param($path) Test-Path -LiteralPath (& $mapPath $path) -PathType Container }.GetNewClosure()
        EnsureDirectory = { param($path) & $record 'EnsureDirectory'; New-Item -ItemType Directory -Path (& $mapPath $path) -Force | Out-Null }.GetNewClosure()
        CopyFile = {
            param($source, $destination)
            & $record 'CopyFile'
            $mappedSource = & $mapPath $source
            $mappedDestination = & $mapPath $destination
            $parent = Split-Path -Parent $mappedDestination
            New-Item -ItemType Directory -Path $parent -Force | Out-Null
            Copy-Item -LiteralPath $mappedSource -Destination $mappedDestination -Force
        }.GetNewClosure()
        CaptureFile = {
            param($path)
            $mapped = & $mapPath $path
            if (-not (Test-Path -LiteralPath $mapped -PathType Leaf)) { return $null }
            [pscustomobject]@{ Path = $path; Bytes = [System.IO.File]::ReadAllBytes($mapped) }
        }.GetNewClosure()
        RestoreFile = {
            param($path, $bytes)
            & $record 'RestoreFile'
            $mapped = & $mapPath $path
            $parent = Split-Path -Parent $mapped
            New-Item -ItemType Directory -Path $parent -Force | Out-Null
            [System.IO.File]::WriteAllBytes($mapped, $bytes)
        }.GetNewClosure()
        RemoveFile = {
            param($path)
            & $record 'RemoveFile'
            Remove-Item -LiteralPath (& $mapPath $path) -Force -ErrorAction SilentlyContinue
        }.GetNewClosure()
        RemoveDirectory = {
            param($path)
            Remove-Item -LiteralPath (& $mapPath $path) -Recurse -Force -ErrorAction SilentlyContinue
        }.GetNewClosure()
        RemoveDirectoryIfEmpty = {
            param($path)
            $mapped = & $mapPath $path
            if ((Test-Path -LiteralPath $mapped -PathType Container) -and (@(Get-ChildItem -LiteralPath $mapped -Force).Count -eq 0)) {
                Remove-Item -LiteralPath $mapped -Force
            }
        }.GetNewClosure()
        GetState = { return $state }.GetNewClosure()
    }
}

function Assert-WifimicHostMutationAllowed {
    [CmdletBinding()]
    param([switch]$ExplicitAcceptance)

    if (-not $ExplicitAcceptance) {
        Throw-WifimicInstallerError -Code 'HostMutationNotExplicit' -Message 'Real task/firewall/file mutation requires -AcceptHostMutation; use -TestMode or -DryRun for isolated verification.'
    }
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        Throw-WifimicInstallerError -Code 'WindowsOnly' -Message 'The native installer must run on Windows.'
    }
    if (-not [Environment]::UserInteractive) {
        Throw-WifimicInstallerError -Code 'InteractiveSessionRequired' -Message 'The client task must be installed from an interactive user session.'
    }
    $administrator = [Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $administrator.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Throw-WifimicInstallerError -Code 'AdministratorRequired' -Message 'Administrator rights are required for Scheduled Task and firewall registration.'
    }
}

function Get-WifimicTestStateRoot {
    [CmdletBinding()]
    param([string]$Requested)

    $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\', '/')
    $candidate = if ([string]::IsNullOrWhiteSpace($Requested)) {
        Join-Path $tempRoot ('wifimic-client-installer-' + [Guid]::NewGuid().ToString('N'))
    }
    else {
        [System.IO.Path]::GetFullPath($Requested).TrimEnd('\', '/')
    }
    $prefix = $tempRoot + [System.IO.Path]::DirectorySeparatorChar
    if (-not $candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) -or [string]::Equals($candidate, $tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Throw-WifimicInstallerError -Code 'InvalidTestRoot' -Message 'TestStateRoot must be a private child of the Windows temporary directory.'
    }
    if (Test-Path -LiteralPath $candidate) {
        Throw-WifimicInstallerError -Code 'TestRootExists' -Message "TestStateRoot already exists: '$candidate'."
    }
    New-Item -ItemType Directory -Path $candidate -Force | Out-Null
    return $candidate
}

$testState = $null
try {
    if ($TestMode -and $DryRun) {
        Throw-WifimicInstallerError -Code 'InvalidMode' -Message 'TestMode and DryRun are mutually exclusive.'
    }
    if (-not $TestMode -and -not $DryRun) {
        Assert-WifimicHostMutationAllowed -ExplicitAcceptance:$AcceptHostMutation
    }

    $operations = if ($TestMode -or $DryRun) {
        $testState = Get-WifimicTestStateRoot -Requested $TestStateRoot
        New-WifimicFakeOperations -StateRoot $testState -EndpointNames $FakeRenderEndpoints
    }
    else {
        if ($FailurePoint) { Throw-WifimicInstallerError -Code 'InvalidMode' -Message 'FailurePoint is only available in TestMode.' }
        New-WifimicNativeOperations
    }

    $mode = if ($TestMode) { 'Test' } elseif ($DryRun) { 'DryRun' } else { 'Native' }
    $result = Invoke-WifimicInstall -ClientExecutable $ClientExecutable -RenderEndpoint $RenderEndpoint -Operations $operations -DryRun:$DryRun -FailurePoint $FailurePoint -Mode $mode
    if ($TestMode -and $null -ne $operations.GetState) {
        $state = Invoke-WifimicOperation -Operations $operations -Name 'GetState'
        $result | Add-Member -NotePropertyName FakeTask -NotePropertyValue ($(if ($null -ne $state.Task) { $state.Task.TaskPath } else { $null }))
        $result | Add-Member -NotePropertyName FakeFirewall -NotePropertyValue ($(if ($null -ne $state.Firewall) { $state.Firewall.DisplayName } else { $null }))
        $result | Add-Member -NotePropertyName FakeInstallRoot -NotePropertyValue $script:CanonicalInstallRoot
        $result | Add-Member -NotePropertyName FakeEvents -NotePropertyValue @($state.Events)
    }
    $result | ConvertTo-Json -Compress
    exit 0
}
catch {
    $stack = if ($null -eq $_.ScriptStackTrace) { '' } else { " Stack: $($_.ScriptStackTrace -replace "`r?`n", ' | ')" }
    [Console]::Error.WriteLine("wifimic-client installer failed: $($_.Exception.Message)$stack")
    exit 1
}
finally {
    if ($null -ne $testState -and (Test-Path -LiteralPath $testState)) {
        Remove-Item -LiteralPath $testState -Recurse -Force -ErrorAction SilentlyContinue
    }
}
