param([Parameter(Mandatory)][string]$ExpectedOperatorSid)
$ErrorActionPreference='Stop'
trap {
    @{state='PROBE_FAILED';line=$_.InvocationInfo.ScriptLineNumber;script=[IO.Path]::GetFileName($_.InvocationInfo.ScriptName);type=$_.Exception.GetType().FullName;error_id=$_.FullyQualifiedErrorId}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $env:LOCALAPPDATA 'EZRA\AgentPilotSetup_20261121\probe_diagnostic.json') -Encoding UTF8
    break
}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
if ($identity.User.Value -ne $ExpectedOperatorSid) { throw 'Wrong operator.' }
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Elevation required.' }
$state=Join-Path $env:LOCALAPPDATA 'EZRA\agent-pilot'
$receiptPath=Join-Path $state 'setup_receipt.json'
$r=Get-Content -LiteralPath $receiptPath -Raw|ConvertFrom-Json
$u=Get-LocalUser -Name agent-readonly
if ($u.SID.Value -ne $r.sid -or $r.credential_manager -ne 'STORED_IN_AGENT_PROFILE') { throw 'Pilot is not ready for this probe.' }
try {
    Enable-LocalUser -Name agent-readonly
    $probe=& (Join-Path $PSScriptRoot 'Start-AgentPilot.ps1') -ProbeOnly
    $probe|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $state 'identity_probe.json') -Encoding UTF8
    $r.probe='VERIFIED_IDENTITY_NONADMIN'
} finally {
    Disable-LocalUser -Name agent-readonly
    $r.disabled=$true
    $r|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $receiptPath -Encoding UTF8
}
