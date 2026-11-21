<#
.SYNOPSIS
  Creates one hidden local Windows account per agent, stores each agent's NAS
  login in that agent's own Credential Manager, and writes a Synology user
  import file.

.NOTES
  UNTESTED. Run in an elevated (Administrator) Windows PowerShell 5.1+ window.
  Passwords are generated on this machine. Nothing is sent anywhere.
  The Synology import file contains plaintext NAS passwords: import it, then
  delete it (the script prints a reminder).

.EXAMPLE
  .\New-AgentAccounts.ps1 -NasHost 192.168.2.50
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $NasHost,
    [string] $AgentsCsv = (Join-Path $PSScriptRoot 'agents.csv'),
    [string] $OutDir    = (Join-Path $env:LOCALAPPDATA 'EZRA\agents'),
    [string] $WorkRoot  = 'C:\Agents'
)
$ErrorActionPreference = 'Stop'

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw 'Run this from an elevated (Run as Administrator) PowerShell window.' }

function New-StrongPassword([int]$Length = 24) {
    $sets = @('ABCDEFGHJKLMNPQRSTUVWXYZ', 'abcdefghijkmnopqrstuvwxyz', '23456789', '!#%+-=?@_')
    $all  = -join $sets
    $rng  = [Security.Cryptography.RandomNumberGenerator]::Create()
    $pick = {
        param($chars)
        $b = New-Object byte[] 4; $rng.GetBytes($b)
        $chars[[BitConverter]::ToUInt32($b, 0) % $chars.Length]
    }
    $chars = @(foreach ($s in $sets) { & $pick $s })              # one from each class
    while ($chars.Count -lt $Length) { $chars += & $pick $all }
    # shuffle
    -join ($chars | Sort-Object { $b = New-Object byte[] 4; $rng.GetBytes($b); [BitConverter]::ToUInt32($b, 0) })
}

New-Item -ItemType Directory -Force -Path $OutDir, $WorkRoot | Out-Null
$importFile = Join-Path $OutDir 'synology_user_import.txt'
$importLines = New-Object System.Collections.Generic.List[string]
$usersSid = 'S-1-5-32-545'   # BUILTIN\Users, works on any Windows language
$hideKey  = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList'

foreach ($a in Import-Csv $AgentsCsv) {
    $name = $a.name.Trim()
    Write-Host "== $name" -ForegroundColor Cyan

    if (Get-LocalUser -Name $name -ErrorAction SilentlyContinue) {
        Write-Warning "$name already exists on Windows; skipping (delete it first to recreate)."
        continue
    }

    $winPass = New-StrongPassword
    $nasPass = New-StrongPassword
    $secWin  = ConvertTo-SecureString $winPass -AsPlainText -Force

    # 1. Local Windows account
    New-LocalUser -Name $name -Password $secWin -Description "EZRA agent: $($a.description)" `
                  -PasswordNeverExpires -UserMayNotChangePassword -AccountNeverExpires | Out-Null
    Add-LocalGroupMember -SID $usersSid -Member $name

    # 2. Hide it from the sign-in screen
    New-Item -Path $hideKey -Force | Out-Null
    New-ItemProperty -Path $hideKey -Name $name -Value 0 -PropertyType DWord -Force | Out-Null

    # 3. Private working folder only this agent (and admins) can write
    $work = Join-Path $WorkRoot $name
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    icacls $work /inheritance:r /grant:r "${name}:(OI)(CI)M" "Administrators:(OI)(CI)F" "SYSTEM:(OI)(CI)F" | Out-Null

    # 4. Save the agent's Windows login for the launcher, encrypted to YOUR Windows account (DPAPI)
    $cred = New-Object System.Management.Automation.PSCredential("$env:COMPUTERNAME\$name", $secWin)
    $cred | Export-Clixml -Path (Join-Path $OutDir "$name.cred.xml")

    # 5. Store the agent's NAS login in the AGENT's own Credential Manager.
    #    (The password is briefly visible on this process's command line.)
    $p = Start-Process -FilePath cmdkey.exe `
            -ArgumentList "/add:$NasHost", "/user:$name", "/pass:$nasPass" `
            -Credential $cred -LoadUserProfile -WorkingDirectory $work -WindowStyle Hidden -Wait -PassThru
    if ($p.ExitCode -ne 0) { Write-Warning "cmdkey for $name exited with $($p.ExitCode)" }

    # 6. Synology import line: username<TAB>password<TAB>description<TAB>email<TAB>quota<TAB>phone
    $importLines.Add(($name, $nasPass, $a.description, '', '', '') -join "`t")

    $winPass = $null; $nasPass = $null
}

if ($importLines.Count) {
    [IO.File]::WriteAllLines($importFile, $importLines, (New-Object Text.UTF8Encoding($false)))
    Write-Host ""
    Write-Host "Synology import file: $importFile" -ForegroundColor Yellow
    Write-Host "  DSM > Control Panel > User & Group > User > Create > Import Users" -ForegroundColor Yellow
    Write-Host "  It holds plaintext NAS passwords. DELETE IT after importing:" -ForegroundColor Yellow
    Write-Host "  Remove-Item '$importFile'" -ForegroundColor Yellow
}
Write-Host "`nDone. Start an agent with: .\Start-Agent.ps1 -Agent agent-claude" -ForegroundColor Green
