<#
.SYNOPSIS
  "Sign in" an agent: opens a shell running AS that agent's Windows account,
  with a session number, and logs the start.

.NOTES
  UNTESTED. Run as yourself (no elevation needed). Uses the encrypted login
  saved by New-AgentAccounts.ps1, which only your Windows account can decrypt.

.EXAMPLE
  .\Start-Agent.ps1 -Agent agent-claude
  .\Start-Agent.ps1 -Agent agent-codex -Command 'codex'
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Agent,
    [string] $Command  = '',          # optional program to run inside the agent's shell
    [string] $CredDir  = (Join-Path $env:LOCALAPPDATA 'EZRA\agents'),
    [string] $WorkRoot = 'C:\Agents'
)
$ErrorActionPreference = 'Stop'

$credFile = Join-Path $CredDir "$Agent.cred.xml"
if (-not (Test-Path $credFile)) { throw "No saved login for $Agent ($credFile). Run New-AgentAccounts.ps1 first." }
$cred = Import-Clixml $credFile

$session = '{0}-{1:yyyyMMdd-HHmmss}-{2}' -f $Agent, (Get-Date), ([guid]::NewGuid().ToString('N').Substring(0, 6))
$work    = Join-Path $WorkRoot $Agent

# Launcher log (yours, append-only by convention): who was started, when, doing what
$log = Join-Path $CredDir 'sessions.log'
"{0:o}`t{1}`t{2}`t{3}" -f (Get-Date), $session, $Agent, $Command | Add-Content -Path $log

$inner = "`$env:EZRA_AGENT='$Agent'; `$env:EZRA_SESSION='$session'; " +
         "`$host.UI.RawUI.WindowTitle='$Agent  [$session]'; " +
         "Write-Host 'Signed in as $Agent  session $session' -ForegroundColor Green; " +
         "Set-Location '$work'; $Command"

Start-Process -FilePath powershell.exe `
    -ArgumentList '-NoExit', '-NoProfile', '-Command', $inner `
    -Credential $cred -LoadUserProfile -WorkingDirectory $work

Write-Host "Started $Agent  session $session"
