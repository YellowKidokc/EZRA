# Internal helper. Password arrives through stdin; never command arguments.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Pilot.Common.ps1')
try {
    $record=[Console]::In.ReadLine()|ConvertFrom-Json
    Assert-PilotName $record.user
    Assert-PilotHost $record.host
    if ([Security.Principal.WindowsIdentity]::GetCurrent().Name.Split('\')[-1] -ne $record.user) { throw 'Unexpected account identity.' }
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class EzraPilotNative {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]
 public struct Credential {
  public UInt32 Flags; public UInt32 Type; public string TargetName; public string Comment;
  public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
  public UInt32 CredentialBlobSize; public IntPtr CredentialBlob; public UInt32 Persist;
  public UInt32 AttributeCount; public IntPtr Attributes; public string TargetAlias; public string UserName;
 }
 [DllImport("Advapi32.dll",EntryPoint="CredWriteW",CharSet=CharSet.Unicode,SetLastError=true)]
 public static extern bool Write(ref Credential credential,UInt32 flags);
}
'@
    $c=New-Object EzraPilotNative+Credential
    $c.Type=2;$c.TargetName=$record.host;$c.UserName=$record.user;$c.Persist=2
    $c.CredentialBlob=[Runtime.InteropServices.Marshal]::StringToCoTaskMemUni($record.password)
    $c.CredentialBlobSize=[Text.Encoding]::Unicode.GetByteCount($record.password)
    try {
        if (-not [EzraPilotNative]::Write([ref]$c,0)) { throw 'Credential Manager rejected the credential.' }
    } finally { [Runtime.InteropServices.Marshal]::ZeroFreeCoTaskMemUnicode($c.CredentialBlob) }
    exit 0
} catch { exit 1 }
