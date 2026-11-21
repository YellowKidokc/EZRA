Set-StrictMode -Version Latest
function Assert-PilotName([string]$Name) {
    if ($Name -ne 'agent-readonly') { throw 'This pilot only provisions agent-readonly.' }
}
function Assert-PilotHost([string]$Name) {
    if ($Name -notmatch '^[A-Za-z0-9][A-Za-z0-9.-]{0,252}$') { throw 'Use a NAS hostname or IPv4 address, not a URL/share.' }
}
function New-PilotPassword {
    $bytes=New-Object byte[] 32
    $rng=[Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
    'Aa9!'+[Convert]::ToBase64String($bytes)
}
function Set-PilotAcl([string]$Path,[string]$OwnerSid,[string]$AgentSid='',[switch]$Readers) {
    $acl=New-Object Security.AccessControl.DirectorySecurity
    $acl.SetAccessRuleProtection($true,$false)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new($OwnerSid))
    $grants=@(@($OwnerSid,'FullControl'),@('S-1-5-18','FullControl'),@('S-1-5-32-544','FullControl'))
    if ($AgentSid) { $grants+=,@($AgentSid,'Modify') }
    if ($Readers) { $grants+=,@('S-1-5-32-545','ReadAndExecute') }
    foreach ($g in $grants) {
        $rule=[Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($g[0]),$g[1],'ContainerInherit,ObjectInherit','None','Allow')
        $acl.AddAccessRule($rule)
    }
    Set-Acl -LiteralPath $Path -AclObject $acl
}
function New-PilotProcessInfo([pscredential]$Credential,[string]$Work,[string]$Arguments) {
    $info=New-Object Diagnostics.ProcessStartInfo
    $info.FileName=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $info.Arguments=$Arguments;$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.LoadUserProfile=$true;$info.WorkingDirectory=$Work
    $info.Domain=$env:COMPUTERNAME;$info.UserName=$Credential.UserName.Split('\')[-1]
    $info.Password=$Credential.Password
    $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $info
}
