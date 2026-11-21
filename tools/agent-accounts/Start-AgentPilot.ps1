[CmdletBinding(SupportsShouldProcess)]
param([switch]$ProbeOnly)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Pilot.Common.ps1')
$name='agent-readonly';$work='C:\Agents\agent-readonly'
$state=Join-Path $env:LOCALAPPDATA 'EZRA\agent-pilot'
if ($WhatIfPreference) { [void]$PSCmdlet.ShouldProcess($name,'Open shell as the pilot account');return }
$cred=Import-Clixml -LiteralPath (Join-Path $state 'agent-readonly.cred.xml')
if ($cred.UserName -ne "$env:COMPUTERNAME\$name") { throw 'Unexpected saved credential identity.' }
if (-not (Get-LocalUser -Name $name).Enabled) { throw 'Pilot remains disabled until NAS permission setup is verified.' }
$session='agent-readonly-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')+'-'+[guid]::NewGuid().ToString('N').Substring(0,8)
$inner="`$env:EZRA_AGENT='$name';`$env:EZRA_SESSION='$session';Set-Location -LiteralPath '$work';"
if ($ProbeOnly) {
    $inner+="`$me=[Security.Principal.WindowsIdentity]::GetCurrent();@{name=`$me.Name;sid=`$me.User.Value;session=`$env:EZRA_SESSION;admin=([Security.Principal.WindowsPrincipal]`$me).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)}|ConvertTo-Json -Compress"
    $args='-NoProfile -NonInteractive -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($inner))
    $p=[Diagnostics.Process]::new();$p.StartInfo=New-PilotProcessInfo -Credential $cred -Work $work -Arguments $args
    try {
        [void]$p.Start();$output=$p.StandardOutput.ReadToEnd()
        if (-not $p.WaitForExit(30000)) { $p.Kill();throw 'Probe timed out.' }
        if ($p.ExitCode -ne 0) { throw 'Identity probe failed.' }
        $result=$output|ConvertFrom-Json
        if ($result.name -ne "$env:COMPUTERNAME\$name" -or $result.admin) { throw 'Unexpected identity or administrator privileges.' }
        $result
    } finally { $p.Dispose() }
} else {
    $inner+="`$host.UI.RawUI.WindowTitle='$session';Write-Host 'Agent session $session';"
    $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($inner))
    # An interactive window is the requested purpose of this launcher.
    $p=Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe') -ArgumentList '-NoProfile','-NoExit','-EncodedCommand',$encoded -Credential $cred -LoadUserProfile -WorkingDirectory $work -PassThru
}
@{event='STARTED';session=$session;agent=$name;utc=[DateTime]::UtcNow.ToString('o');probe=[bool]$ProbeOnly}|ConvertTo-Json -Compress|Add-Content -LiteralPath (Join-Path $state 'sessions.jsonl')
