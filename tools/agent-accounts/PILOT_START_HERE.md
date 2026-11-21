# Agent identity pilot — HP

Use New-AgentPilot.ps1 and Start-AgentPilot.ps1 for the corrected single-account pilot. The older five-account scripts remain historical untested inputs; do not use them as the current installer.

The coordinator found HP\David is running without an elevated token, seclogon is installed but stopped/manual, the NAS share is 192.168.2.50, and D: grants broad access to Authenticated Users. A separate standard account is identity separation, not confinement to C:\Agents. Existing broad permissions must be assessed separately; this pilot does not rewrite shared drive ACLs.

## Corrected boundaries

- Only agent-readonly is provisioned. No existing account or preexisting pilot directory is reset.
- Windows credentials are DPAPI-encrypted to the operator who runs setup. The expected-operator SID guard rejects elevation under a different account.
- NAS credentials travel by redirected stdin to a profile-loaded process as the agent and are saved using CredWrite. They are not included in process arguments.
- Credential/export directories have explicit operator, Administrators and SYSTEM ACLs. The agent workspace adds only that agent's Modify permission. Installed helper code is read-only to standard Users.
- A failed or successful account creation leaves the pilot disabled pending NAS configuration. A nonsecret receipt records incomplete stages.
- DSM 7 uses username, password, description, email, group, then volume quotas. The pilot exports only the first three supported columns and leaves group/permission setup explicit. There is no phone column. Reference: https://kb.synology.com/en-us/DSM/help/DSM/AdminCenter/file_user_import?version=7
- The private TSV has a plaintext NAS password. Do not print it, commit it, or copy it to shared storage. Import it using DSM, then remove it locally after a verified import.
- This pilot does not configure NAS accounts, group permissions, logs, snapshots or proof of restore. Those require the signed-in DSM administrator and supported storage.

## Sequence

1. Run Test-AgentPilot.ps1 for non-mutating checks.
2. Elevate the prepared pilot launcher as David. It creates only the disabled agent-readonly account.
3. In DSM, create a read-only agents group and verify effective permissions on the intended research share before granting the new user access. Do not add the user to administrators. Import the private TSV, assign the reviewed group and check effective permissions including inherited users-group grants.
4. Configure supported SMB operation logs and snapshot/retention settings only after checking existing policies and Btrfs/package support. Do not replace an existing schedule blindly.
5. Enable the local pilot from an elevated David shell, run Start-AgentPilot.ps1 -ProbeOnly, and test allowed reads and denied writes in agreed test locations. Also test that the agent cannot read David's credential folder. Do not use manuscripts as write probes.
6. Only after those checks pass, use the interactive launcher and extend to another family. A session label is not tamper-proof auditing. Use NAS-side identity/logs for operations actually performed under that account; signing a shell in does not convert existing AI processes running as David.

No live account or DSM verification is implied by static checks.
