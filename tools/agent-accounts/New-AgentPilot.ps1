[CmdletBinding(SupportsShouldProcess)]
param([string]$NasHost='192.168.2.50',[string]$ExpectedOperatorSid='')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Pilot.Common.ps1')
Assert-PilotHost $NasHost
$name='agent-readonly'
$identity=[Security.Principal.WindowsIdentity]::GetCurrent();$owner=$identity.User.Value
if ($ExpectedOperatorSid -and $owner -ne $ExpectedOperatorSid) { throw 'Elevate as David, not a different administrator; credentials must belong to the launcher operator.' }
$state=Join-Path $env:LOCALAPPDATA 'EZRA\agent-pilot'
$work='C:\Agents\agent-readonly'
$install='C:\ProgramData\EZRA\AgentPilot'
if ($WhatIfPreference) { [void]$PSCmdlet.ShouldProcess($name,'Create one standard pilot account and private credentials; stage DSM import; disable account pending NAS setup');return }
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrator elevation required.' }
if (Get-LocalUser -Name $name -ErrorAction SilentlyContinue) { throw 'Pilot account already exists. Preserve it; inspect its receipt instead of resetting it.' }
foreach ($path in @($state,$work,$install)) {
    if (Test-Path -LiteralPath $path) { throw "Pilot path already exists; inspect before changing: $path" }
}
if (-not $PSCmdlet.ShouldProcess($name,'Provision pilot')) { return }
foreach ($path in @($state,$work,$install)) { New-Item -ItemType Directory -Path $path -Force|Out-Null;Set-PilotAcl -Path $path -OwnerSid $owner -Readers:($path -eq $install) }
foreach ($file in @('Pilot.Common.ps1','Pilot.StoreNasCredential.ps1','Start-AgentPilot.ps1','Pilot.Identity.ps1')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $install $file) }
$receipt=@{run=[guid]::NewGuid().ToString('N');utc=[DateTime]::UtcNow.ToString('o');agent=$name;nas_host=$NasHost;operator_sid=$owner;windows='NOT_CREATED';nas='NOT_CREATED';nas_permissions='NOT_CONFIGURED';snapshots='NOT_CONFIGURED';credential_manager='NOT_CONFIGURED';probe='NOT_RUN'}
$created=$false
try {
    $win=New-PilotPassword;$nas=New-PilotPassword
    $secure=ConvertTo-SecureString $win -AsPlainText -Force
    $account=New-LocalUser -Name $name -Password $secure -Description 'EZRA read-only identity pilot' -AccountNeverExpires -PasswordNeverExpires -UserMayNotChangePassword
    $created=$true;$receipt.windows='CREATED';$receipt.sid=$account.SID.Value
    Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $account
    Set-PilotAcl -Path $work -OwnerSid $owner -AgentSid $account.SID.Value
    $cred=[pscredential]::new("$env:COMPUTERNAME\$name",$secure)
    $cred|Export-Clixml -LiteralPath (Join-Path $state 'agent-readonly.cred.xml')
    $info=New-PilotProcessInfo -Credential $cred -Work $work -Arguments ('-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "'+(Join-Path $install 'Pilot.StoreNasCredential.ps1')+'"')
    $p=[Diagnostics.Process]::new();$p.StartInfo=$info
    try {
        [void]$p.Start()
        $p.StandardInput.WriteLine((@{host=$NasHost;user=$name;password=$nas}|ConvertTo-Json -Compress));$p.StandardInput.Close()
        if (-not $p.WaitForExit(30000)) { $p.Kill();throw 'Private credential helper timed out.' }
        if ($p.ExitCode -ne 0) { throw 'Private credential helper failed; do not import this pilot.' }
    } finally { $p.Dispose() }
    $receipt.credential_manager='STORED_IN_AGENT_PROFILE'
    # DSM 7 documented order: username, password, description, email, group, volume quota.
    # Omit optional columns: group permissions are configured before enabling the pilot.
    $import=Join-Path $state 'synology_agent_readonly.tsv'
    [IO.File]::WriteAllText($import,(($name,$nas,'EZRA read-only identity pilot') -join "`t")+"`r`n",(New-Object Text.UTF8Encoding($false)))
    $receipt.import_file=$import
    $receipt.windows='READY_PENDING_NAS'
} catch {
    $receipt.failure_type=$_.Exception.GetType().FullName
    $receipt.windows='SETUP_FAILED_HOLD'
    throw
} finally {
    if ($created) { Disable-LocalUser -Name $name;$receipt.disabled=$true }
    $receipt|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $state 'setup_receipt.json') -Encoding UTF8
    $win=$null;$nas=$null
}
Write-Host 'Pilot created and disabled pending NAS permissions. No other accounts were created.'
Write-Host "Private setup receipt: $(Join-Path $state 'setup_receipt.json')"
