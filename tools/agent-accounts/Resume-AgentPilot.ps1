[CmdletBinding()]
param([Parameter(Mandatory)][string]$ExpectedOperatorSid)
$ErrorActionPreference='Stop'
trap {
    @{state='RESUME_FAILED';line=$_.InvocationInfo.ScriptLineNumber;script=[IO.Path]::GetFileName($_.InvocationInfo.ScriptName);type=$_.Exception.GetType().FullName;error_id=$_.FullyQualifiedErrorId} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $env:LOCALAPPDATA 'EZRA\AgentPilotSetup_20261121\resume_diagnostic.json') -Encoding UTF8
    break
}
. (Join-Path $PSScriptRoot 'Pilot.Common.ps1')
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
if ($identity.User.Value -ne $ExpectedOperatorSid) { throw 'Wrong operator.' }
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Elevation required.' }
$name='agent-readonly';$state=Join-Path $env:LOCALAPPDATA 'EZRA\agent-pilot'
$receiptPath=Join-Path $state 'setup_receipt.json'
$receipt=Get-Content -LiteralPath $receiptPath -Raw|ConvertFrom-Json
$account=Get-LocalUser -Name $name
if ($receipt.sid -ne $account.SID.Value -or $receipt.operator_sid -ne $ExpectedOperatorSid) { throw 'Pilot ownership mismatch.' }
if (Test-Path -LiteralPath (Join-Path $state 'synology_agent_readonly.tsv')) { throw 'Preserve existing NAS import; do not rotate it.' }
$cred=Import-Clixml -LiteralPath (Join-Path $state 'agent-readonly.cred.xml')
$install='C:\ProgramData\EZRA\AgentPilot';$work='C:\Agents\agent-readonly'
foreach ($file in @('Pilot.Common.ps1','Pilot.StoreNasCredential.ps1','Start-AgentPilot.ps1','Pilot.Identity.ps1')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $install $file) -Force }
$nas=New-PilotPassword
try {
    Enable-LocalUser -Name $name
    $info=New-PilotProcessInfo -Credential $cred -Work $work -Arguments ('-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "'+(Join-Path $install 'Pilot.StoreNasCredential.ps1')+'"')
    $p=[Diagnostics.Process]::new();$p.StartInfo=$info
    try {
        [void]$p.Start()
        $p.StandardInput.WriteLine((@{host=$receipt.nas_host;user=$name;password=$nas}|ConvertTo-Json -Compress));$p.StandardInput.Close()
        if (-not $p.WaitForExit(30000)) { $p.Kill();throw 'Private helper timed out.' }
        $diagnostic=$p.StandardError.ReadToEnd()
        $redacted=$diagnostic.Replace($nas,'[REDACTED]')
        $redacted=$redacted -replace 'sk-[A-Za-z0-9_-]{20,}','[REDACTED]'
        $redacted|Set-Content -LiteralPath (Join-Path $state 'credential_helper_redacted.txt')
        @{exit_code=$p.ExitCode;stderr_length=$diagnostic.Length}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $state 'credential_helper_exit.json')
        # Helper emits static stage/type/numeric error only. Never print free-form errors.
        $safe=@($diagnostic -split "`r?`n"|Where-Object { $_ -match '^(Stage=[A-Z_]+;ExceptionType=[A-Za-z0-9_.]+|CredentialWriteError=\d+)$' })
        $safe|Set-Content -LiteralPath (Join-Path $state 'credential_helper_diagnostic.txt')
        if ($p.ExitCode -ne 0) { throw 'Credential helper failed; inspect nonsecret stage diagnostic.' }
    } finally { $p.Dispose() }
    $receipt.credential_manager='STORED_IN_AGENT_PROFILE'
    $import=Join-Path $state 'synology_agent_readonly.tsv'
    [IO.File]::WriteAllText($import,(($name,$nas,'EZRA read-only identity pilot') -join "`t")+"`r`n",(New-Object Text.UTF8Encoding($false)))
    $receipt|Add-Member -NotePropertyName import_file -NotePropertyValue $import -Force
    $receipt.windows='READY_PENDING_NAS'
} finally {
    Disable-LocalUser -Name $name
    $receipt.disabled=$true
    $receipt|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $receiptPath -Encoding UTF8
    $nas=$null
}
