$ErrorActionPreference='Stop'
$me=[Security.Principal.WindowsIdentity]::GetCurrent()
@{name=$me.Name;sid=$me.User.Value;admin=([Security.Principal.WindowsPrincipal]$me).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)}|ConvertTo-Json -Compress
