$ErrorActionPreference='Stop'
$files=@('Pilot.Common.ps1','Pilot.StoreNasCredential.ps1','New-AgentPilot.ps1','Start-AgentPilot.ps1')
foreach ($file in $files) {
    $tokens=$null;$errors=$null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $file),[ref]$tokens,[ref]$errors)
    if ($errors.Count) { throw "PowerShell parser failure: $file" }
}
. (Join-Path $PSScriptRoot 'Pilot.Common.ps1')
Assert-PilotName 'agent-readonly'
Assert-PilotHost '192.168.2.50'
foreach ($bad in @('agent-admin','agent-readonly\..\David',"x';whoami")) {
    $rejected=$false;try { Assert-PilotName $bad } catch { $rejected=$true }
    if (-not $rejected) { throw 'Unsafe account selector was accepted.' }
}
foreach ($bad in @('\\host\share','http://nas:5000','nas /pass:value')) {
    $rejected=$false;try { Assert-PilotHost $bad } catch { $rejected=$true }
    if (-not $rejected) { throw 'Unsafe NAS host was accepted.' }
}
$seen=New-Object 'Collections.Generic.HashSet[string]'
for ($i=0;$i -lt 100;$i++) {
    $value=New-PilotPassword
    if ($value.Length -lt 32 -or $value -match '[\t\r\n]' -or -not $seen.Add($value)) { throw 'Password generation sanity check failed.' }
}
$value=$null
# Compile only the native layout/signature. Do not call CredWrite or change accounts.
$helper=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Pilot.StoreNasCredential.ps1') -Raw
$native=[regex]::Match($helper,"(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@").Groups[1].Value
if (-not $native) { throw 'Native helper source was not found.' }
Add-Type -TypeDefinition $native
$credential=[pscredential]::new('HP\agent-readonly',(ConvertTo-SecureString 'Synthetic-test-only' -AsPlainText -Force))
$info=New-PilotProcessInfo -Credential $credential -Work 'C:\Agents\agent-readonly' -Arguments '-NoProfile -File "C:\ProgramData\EZRA\AgentPilot\Pilot.StoreNasCredential.ps1"'
if ($info.Arguments -match 'Synthetic-test-only' -or -not $info.RedirectStandardInput) { throw 'Credential transport boundary failed.' }
& (Join-Path $PSScriptRoot 'New-AgentPilot.ps1') -WhatIf
& (Join-Path $PSScriptRoot 'Start-AgentPilot.ps1') -WhatIf
Write-Output 'PASS: parser, selector rejection, password sanity, native API compilation, private-pipe setup and non-mutating previews. No account or credential was created.'
