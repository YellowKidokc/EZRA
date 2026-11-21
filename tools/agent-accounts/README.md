# Agent accounts: one identity per agent family

**Status: untested scripts.** Try them with one agent first (trim `agents.csv` to one row).

Each agent family gets:
- a hidden local Windows account on the HP, with a private folder `C:\Agents\<name>`,
- its own Synology account (same name), whose password is stored in **that agent's** Windows Credential Manager, so it connects to the NAS as itself automatically,
- a session number every time it's started, logged to `%LOCALAPPDATA%\EZRA\agents\sessions.log`.

The Synology log then shows which agent made each change, and btrfs snapshots let you put things back.

## 1. Create the accounts (once, on the HP, in an Administrator PowerShell window)

Replace `NAS_HOST` with your NAS's address.

```powershell
cd <EZRA>\tools\agent-accounts
Set-ExecutionPolicy -Scope Process Bypass
.\New-AgentAccounts.ps1 -NasHost NAS_HOST
```

This writes `%LOCALAPPDATA%\EZRA\agents\synology_user_import.txt`.

## 2. Import the users on the Synology

DSM → Control Panel → User & Group → User → Create → **Import Users**, then choose that file.
Check the column order against the help text on DSM's import dialog. The script writes:
`username <TAB> password <TAB> description <TAB> email <TAB> quota <TAB> phone`.

**Then delete the import file.** It contains plaintext passwords.

After the import, in DSM:
- Put the accounts in a group (e.g. `agents`) and set **shared folder permissions**. Read-only by default, write only where an agent really needs it. The import file doesn't set permissions.
- Log Center → turn on **SMB file transfer logs** (include create, delete, rename, write).
- Snapshot Replication → hourly snapshots on the shares that hold your originals.

## 3. Start an agent ("sign in")

```powershell
.\Start-Agent.ps1 -Agent agent-claude
.\Start-Agent.ps1 -Agent agent-codex -Command 'codex'
```

This opens a PowerShell window running **as that agent**, titled with its session number, starting in `C:\Agents\<name>`. Anything it does on the NAS is logged under its own name.

## What this does and doesn't lock

- It does: an agent started this way can't read your personal files (your user profile is private to you) and can only write where you grant it.
- It doesn't: an agent app you start normally, as yourself, still runs as you. The rule is to start agents only through `Start-Agent.ps1`.
- Cloud agents (claude.ai/code sessions) never run on the HP, so they're out of scope here.

## Files

| File | Holds |
|---|---|
| `agents.csv` | The agent families. Edit before running. |
| `New-AgentAccounts.ps1` | One-time setup (admin) |
| `Start-Agent.ps1` | Launcher / sign-in |
| `%LOCALAPPDATA%\EZRA\agents\*.cred.xml` | Agents' Windows logins, encrypted so only your Windows account can read them. Never commit. |
