use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn unc(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.starts_with(r"\\")
        || text.starts_with("//")
        || text.to_ascii_uppercase().starts_with(r"UNC\")
}

pub fn absolute(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let resolved = fs::canonicalize(&absolute)?;
    #[cfg(windows)]
    if resolved.to_string_lossy().starts_with(r"\\?\") {
        let raw = resolved.to_string_lossy();
        if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{unc}")));
        }
        if raw.as_bytes().get(5) == Some(&b':') {
            return Ok(PathBuf::from(&raw[4..]));
        }
    }
    Ok(resolved)
}

#[cfg(not(windows))]
pub fn scan_path(path: &Path) -> Result<PathBuf> {
    absolute(path)
}

#[cfg(windows)]
pub fn scan_path(path: &Path) -> Result<PathBuf> {
    use windows_sys::Win32::{
        Foundation::ERROR_MORE_DATA,
        NetworkManagement::WNet::{
            UNIVERSAL_NAME_INFO_LEVEL, UNIVERSAL_NAME_INFOW, WNetGetUniversalNameW,
        },
        Storage::FileSystem::GetDriveTypeW,
        System::WindowsProgramming::DRIVE_REMOTE,
    };
    let full = std::path::absolute(path)?;
    if !unc(&full)
        && let Some(root) = full.ancestors().last()
        && unsafe { GetDriveTypeW(wide(root.as_os_str()).as_ptr()) } == DRIVE_REMOTE
    {
        let name = wide(full.as_os_str());
        let mut size = 0;
        let result = unsafe {
            WNetGetUniversalNameW(
                name.as_ptr(),
                UNIVERSAL_NAME_INFO_LEVEL,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if result != ERROR_MORE_DATA {
            bail!("Cannot resolve mapped drive to UNC (Windows error {result})");
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        let result = unsafe {
            WNetGetUniversalNameW(
                name.as_ptr(),
                UNIVERSAL_NAME_INFO_LEVEL,
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if result != 0 {
            bail!("Cannot resolve mapped drive to UNC (Windows error {result})");
        }
        use std::os::windows::ffi::OsStringExt;
        let info = unsafe { &*buffer.as_ptr().cast::<UNIVERSAL_NAME_INFOW>() };
        let length = (0..)
            .take_while(|&offset| unsafe { *info.lpUniversalName.add(offset) != 0 })
            .count();
        let unc_path = std::ffi::OsString::from_wide(unsafe {
            std::slice::from_raw_parts(info.lpUniversalName, length)
        });
        return absolute(Path::new(&unc_path));
    }
    absolute(&full)
}

pub fn local_path(path: &Path) -> Result<PathBuf> {
    if unc(path) {
        bail!("SQLite database must be on a local disk, not a UNC share");
    }
    let full = std::path::absolute(path)?;
    let mut ancestor = full.as_path();
    let mut suffix = Vec::new();
    let resolved = loop {
        match absolute(ancestor) {
            Ok(path) => break path,
            Err(error) => {
                match fs::symlink_metadata(ancestor) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return Err(error).context("resolve database location"),
                }
                let component = ancestor
                    .file_name()
                    .context("cannot resolve database drive or parent")?;
                suffix.push(component);
                ancestor = ancestor
                    .parent()
                    .context("cannot resolve database parent")?;
            }
        }
    };
    if unc(&resolved) || !is_local(&full, &resolved)? {
        bail!(
            "SQLite database must be on a local disk; network or unknown drive: {}",
            resolved.display()
        );
    }
    Ok(suffix
        .into_iter()
        .rev()
        .fold(resolved, |path, part| path.join(part)))
}

#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain([0]).collect()
}

#[cfg(windows)]
fn is_local(original: &Path, resolved: &Path) -> Result<bool> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW};
    use windows_sys::Win32::System::WindowsProgramming::{
        DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOVABLE,
    };
    for path in [original, resolved] {
        let Some(root) = path.ancestors().last() else {
            return Ok(false);
        };
        let drive = unsafe { GetDriveTypeW(wide(root.as_os_str()).as_ptr()) };
        if !matches!(drive, DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_RAMDISK) {
            return Ok(false);
        }
    }
    let mut volume = vec![0u16; 32768];
    let found = unsafe {
        GetVolumePathNameW(
            wide(resolved.as_os_str()).as_ptr(),
            volume.as_mut_ptr(),
            volume.len() as u32,
        )
    };
    Ok(found != 0
        && matches!(
            unsafe { GetDriveTypeW(volume.as_ptr()) },
            DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_RAMDISK
        ))
}

#[cfg(target_os = "linux")]
fn is_local(_original: &Path, resolved: &Path) -> Result<bool> {
    use std::os::unix::ffi::OsStrExt;
    let mounts = fs::read_to_string("/proc/self/mountinfo")?;
    let mut best = None;
    for line in mounts.lines() {
        let Some((fields, fs_fields)) = line.split_once(" - ") else {
            continue;
        };
        let Some(encoded) = fields.split_whitespace().nth(4) else {
            continue;
        };
        let decoded = encoded
            .replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\012", "\n")
            .replace("\\134", "\\");
        let mount = Path::new(std::ffi::OsStr::from_bytes(decoded.as_bytes()));
        if resolved.starts_with(mount) && best.is_none_or(|(length, _)| decoded.len() >= length) {
            best = Some((
                decoded.len(),
                fs_fields.split_whitespace().next().unwrap_or(""),
            ));
        }
    }
    Ok(best.is_some_and(|(_, fs)| {
        matches!(
            fs,
            "overlay"
                | "ext2"
                | "ext3"
                | "ext4"
                | "xfs"
                | "btrfs"
                | "tmpfs"
                | "zfs"
                | "f2fs"
                | "vfat"
                | "exfat"
                | "ntfs3"
                | "ramfs"
                | "squashfs"
        )
    }))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn is_local(_: &Path, _: &Path) -> Result<bool> {
    bail!("Local disk verification is supported on Windows and Linux")
}

#[cfg(windows)]
pub fn open(path: &Path, select: bool) -> Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    if select {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path.as_os_str());
        std::process::Command::new("explorer.exe")
            .arg(arg)
            .spawn()?;
    } else {
        let verb = wide(std::ffi::OsStr::new("open"));
        let path = wide(path.as_os_str());
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            bail!(
                "Windows could not open this path (ShellExecute code {})",
                result as isize
            );
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn open(path: &Path, select: bool) -> Result<()> {
    let target = if select {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let status = std::process::Command::new("xdg-open")
        .arg(target)
        .status()?;
    if !status.success() {
        bail!("xdg-open returned {status}");
    }
    Ok(())
}

#[cfg(windows)]
pub fn clipboard(link: &str) -> Result<()> {
    arboard::Clipboard::new()?.set_text(link)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn clipboard(link: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        Command::new("wl-copy").stdin(Stdio::piped()).spawn()
    } else {
        Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()
    }
    .context("desktop clipboard unavailable")?;
    child
        .stdin
        .take()
        .context("clipboard stdin unavailable")?
        .write_all(link.as_bytes())?;
    if !child.wait()?.success() {
        bail!("desktop clipboard unavailable");
    }
    Ok(())
}

#[cfg(windows)]
pub fn registration(register: bool, context_menu: bool) -> Result<()> {
    use std::io::ErrorKind;
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let root = r"Software\Classes\tp";
    let context = r"Software\Classes\*\shell\EZRA.CopyTpLink";
    let owns = |key: &RegKey| {
        key.get_value::<String, _>("EZRA.Owner")
            .is_ok_and(|v| v == "EZRA.tp.v1")
    };
    let executable = std::env::current_exe()?;
    for (name, requested, action, label) in [
        (root, register, "open", "URL:EZRA permanent link"),
        (context, register && context_menu, "link", "Copy tp link"),
    ] {
        match user.open_subkey(name) {
            Ok(key) if !owns(&key) => {
                if requested {
                    bail!("Registry entry is owned by another application: {name}");
                }
                continue;
            }
            Ok(_) if !requested => {
                user.delete_subkey_all(name)?;
                continue;
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound && !requested => continue,
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let (key, _) = user.create_subkey(name)?;
        key.set_value("EZRA.Owner", &"EZRA.tp.v1")?;
        key.set_value("", &label)?;
        let command_path = if action == "open" {
            key.set_value("URL Protocol", &"")?;
            r"shell\open\command"
        } else {
            "command"
        };
        let (command, _) = key.create_subkey(command_path)?;
        command.set_value("", &handler(&executable, action))?;
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub fn handler(executable: &Path, action: &str) -> String {
    format!("\"{}\" {action} -- \"%1\"", executable.display())
}

#[cfg(not(windows))]
pub fn registration(_: bool, _: bool) -> Result<()> {
    bail!("Protocol registration requires Windows");
}
