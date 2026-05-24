//! App-managed SMB/CIFS mounting. Vela gets each share to a POSIX path, then
//! browses that path through the local source (so all of P2c/P2d applies for
//! free). macOS uses `mount_smbfs`, Windows uses `net use`; Linux deliberately
//! stays in user space by resolving KIO-FUSE/GVfs mounts and never invoking
//! `mount.cifs` or `pkexec` by default.
//!
//! The mount target is `SmbMount::mountpoint`. On macOS/Windows it is stable and
//! app-selected; on Linux it can be the current desktop FUSE path for the share.

use crate::config::SmbMount;

/// The path the OS mounts at and the local source browses — computed once,
/// then persisted in `SmbMount::mountpoint`.
#[cfg(target_os = "macos")]
pub fn default_mountpoint(m: &SmbMount) -> Result<String, String> {
    let base = crate::config::config_dir_file("mounts").map_err(|e| e.to_string())?;
    // Include the (stable, persisted) mount id so shares whose names sanitize to
    // the same string (e.g. "Media-4K" vs "Media_4K") don't collide.
    let mp = base.join(format!(
        "{}_{}_{}",
        sanitize(&m.server),
        sanitize(&m.share),
        m.id
    ));
    Ok(mp.to_string_lossy().to_string())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn default_mountpoint(m: &SmbMount) -> Result<String, String> {
    Ok(resolve_user_mountpoint(m).unwrap_or_else(|| gvfs_candidates(m).remove(0)))
}

#[cfg(windows)]
pub fn default_mountpoint(m: &SmbMount) -> Result<String, String> {
    Ok(format!(r"\\{}\{}", m.server, m.share))
}

/// Prepare the share for browsing and update `m.mountpoint` to the path that
/// should be persisted. On Linux that path may be a desktop-provided FUSE path,
/// so it is resolved as part of the mount operation instead of being invented
/// under Vela's config directory.
#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn prepare_mount(m: &mut SmbMount) -> Result<(), String> {
    m.mountpoint = default_mountpoint(m)?;
    mount(m)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn prepare_mount(m: &mut SmbMount) -> Result<(), String> {
    m.mountpoint = default_mountpoint(m)?;
    match mount(m) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(path) = resolve_user_mountpoint(m) {
                m.mountpoint = path;
                mount(m)
            } else {
                Err(e)
            }
        }
    }
}

/// Mount the share at `m.mountpoint` (which the caller has set).
#[cfg(target_os = "macos")]
pub fn mount(m: &SmbMount) -> Result<(), String> {
    use std::process::Command;
    std::fs::create_dir_all(&m.mountpoint)
        .map_err(|e| format!("could not create mountpoint: {e}"))?;
    // mount_smbfs //[[domain;]user[:password]@]server/share mountpoint
    // NOTE: with credentials, the password rides in the URL and is therefore
    // briefly visible in process arguments — an accepted local-only exposure
    // (see README). A blank username means a guest/anonymous mount.
    let url = if m.username.is_empty() {
        format!("//{}/{}", m.server, m.share)
    } else {
        let mut authority = String::new();
        if !m.domain.is_empty() {
            authority.push_str(&pct(&m.domain));
            authority.push(';');
        }
        authority.push_str(&pct(&m.username));
        if !m.password.is_empty() {
            authority.push(':');
            authority.push_str(&pct(&m.password));
        }
        format!("//{}@{}/{}", authority, m.server, m.share)
    };
    let out = Command::new("mount_smbfs")
        .arg(&url)
        .arg(&m.mountpoint)
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_dir(&m.mountpoint); // don't leak the dir on spawn failure
            format!("failed to run mount_smbfs: {e}")
        })?;
    if out.status.success() {
        Ok(())
    } else {
        let _ = std::fs::remove_dir(&m.mountpoint); // don't leak the empty mountpoint
        Err(format!(
            "mount failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(target_os = "macos")]
pub fn unmount(mountpoint: &str) -> Result<(), String> {
    let out = std::process::Command::new("umount")
        .arg(mountpoint)
        .output()
        .map_err(|e| format!("failed to run umount: {e}"))?;
    if out.status.success() {
        let _ = std::fs::remove_dir(mountpoint); // remove the now-empty mountpoint
        Ok(())
    } else {
        Err(format!(
            "unmount failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(target_os = "windows")]
pub fn mount(m: &SmbMount) -> Result<(), String> {
    use std::process::Command;
    // net use \\server\share [password] /user:[domain\]user  (m.mountpoint = UNC)
    // NOTE: with credentials, the password is passed as a process argument —
    // an accepted local-only exposure (see README). A blank username means a
    // guest/anonymous mount (no /user:).
    let mut args = vec!["use".to_string(), m.mountpoint.clone()];
    if !m.username.is_empty() {
        if !m.password.is_empty() {
            args.push(m.password.clone());
        }
        let user = if m.domain.is_empty() {
            m.username.clone()
        } else {
            format!(r"{}\{}", m.domain, m.username)
        };
        args.push(format!("/user:{user}"));
    }
    let out = Command::new("net")
        .args(&args)
        .output()
        .map_err(|e| format!("failed to run net use: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "mount failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(target_os = "windows")]
pub fn unmount(mountpoint: &str) -> Result<(), String> {
    let out = std::process::Command::new("net")
        .args(["use", mountpoint, "/delete", "/y"])
        .output()
        .map_err(|e| format!("failed to run net use: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "unmount failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn mount(m: &SmbMount) -> Result<(), String> {
    if is_readable_dir(&m.mountpoint) {
        return Ok(());
    }
    if let Some(path) = resolve_user_mountpoint(m) {
        if path == m.mountpoint {
            return Ok(());
        }
    }

    // If GVfs is present and already has credentials in the user's session or
    // keyring, this can establish the FUSE path without privilege escalation.
    // It is intentionally bounded so a password prompt or offline share cannot
    // wedge the app.
    let _ = try_gio_mount(m);
    if is_readable_dir(&m.mountpoint) {
        return Ok(());
    }
    Err(linux_mount_error(m))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn unmount(_mountpoint: &str) -> Result<(), String> {
    Ok(())
}

/// Best-effort OS teardown after the record has already been removed (the caller
/// updates config first, so we never leave a record pointing at an unmounted
/// folder, and we never resurrect a mount). A failure just means the OS mount
/// lingers until the user closes files / reboots — it's logged, not fatal.
#[cfg(target_os = "macos")]
pub fn unmount_for_removal(mountpoint: &str) {
    if is_mounted(mountpoint) {
        if let Err(e) = unmount(mountpoint) {
            eprintln!("vela: umount failed (the mount may linger): {e}");
        }
    } else {
        let _ = std::fs::remove_dir(mountpoint); // clean a stale empty mountpoint dir
    }
}

#[cfg(target_os = "windows")]
pub fn unmount_for_removal(mountpoint: &str) {
    // No /y: if files are open, net prompts and (with no stdin) aborts, so we
    // never force-close an in-use connection (data loss). It just lingers.
    match std::process::Command::new("net")
        .args(["use", mountpoint, "/delete"])
        .output()
    {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let msg = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            // NET error 2250 = the connection was already gone — expected, not a
            // failure. Match it as a standalone token, so a share name that merely
            // contains 2250 (e.g. \\nas2250\share, a single path token) doesn't
            // suppress a genuine failure.
            let already_gone = msg.split_whitespace().any(|tok| tok == "2250");
            if !already_gone {
                eprintln!(
                    "vela: `net use /delete` failed (the connection may linger): {}",
                    msg.trim()
                );
            }
        }
        Err(e) => eprintln!("vela: failed to run net use: {e}"),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn unmount_for_removal(_mountpoint: &str) {
    // Linux SMB mounts are owned by the user's desktop session. Removing the
    // source from Vela should not disconnect Dolphin/Nautilus or other apps.
}

/// A mountpoint sits on a different device than its parent dir.
#[cfg(target_os = "macos")]
fn is_mounted(mountpoint: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    let p = std::path::Path::new(mountpoint);
    match (
        std::fs::metadata(p),
        p.parent().and_then(|par| std::fs::metadata(par).ok()),
    ) {
        (Ok(m), Some(parent)) => m.dev() != parent.dev(),
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn remount_on_startup() -> bool {
    false
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn remount_on_startup() -> bool {
    true
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn resolved_mountpoint(m: &SmbMount) -> Option<String> {
    resolve_user_mountpoint(m)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn resolve_user_mountpoint(m: &SmbMount) -> Option<String> {
    user_mount_candidates(m)
        .into_iter()
        .find(|path| is_readable_dir(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn user_mount_candidates(m: &SmbMount) -> Vec<String> {
    let mut out = gvfs_candidates(m);
    out.extend(kio_fuse_candidates(m));
    dedupe(out)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn gvfs_candidates(m: &SmbMount) -> Vec<String> {
    let uid = current_uid();
    let base = format!("/run/user/{uid}/gvfs");
    let mut out = Vec::new();
    for server in case_variants(&m.server) {
        for share in case_variants(&m.share) {
            if m.username.trim().is_empty() {
                out.push(format!("{base}/smb-share:server={server},share={share}"));
            } else {
                out.push(format!(
                    "{base}/smb-share:server={server},share={share},user={}",
                    m.username.trim()
                ));
                out.push(format!("{base}/smb-share:server={server},share={share}"));
            }
        }
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn kio_fuse_candidates(m: &SmbMount) -> Vec<String> {
    let root = format!("/run/user/{}", current_uid());
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("kio-fuse") {
            continue;
        }
        let smb_root = path.join("smb");
        for server in kio_server_variants(m) {
            for share in case_variants(&m.share) {
                out.push(
                    smb_root
                        .join(&server)
                        .join(&share)
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
        out.extend(discover_kio_share_candidates(&smb_root, m));
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn discover_kio_share_candidates(smb_root: &std::path::Path, m: &SmbMount) -> Vec<String> {
    let Ok(servers) = std::fs::read_dir(smb_root) else {
        return Vec::new();
    };
    let wanted_server = m.server.trim().to_ascii_lowercase();
    let wanted_share = m.share.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    for server_entry in servers.flatten() {
        let server_path = server_entry.path();
        let Some(server_name) = server_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let server_host = server_name
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(server_name)
            .to_ascii_lowercase();
        if server_host != wanted_server {
            continue;
        }
        let Ok(shares) = std::fs::read_dir(&server_path) else {
            continue;
        };
        for share_entry in shares.flatten() {
            let share_path = share_entry.path();
            let Some(share_name) = share_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if share_name.to_ascii_lowercase() == wanted_share {
                out.push(share_path.to_string_lossy().to_string());
            }
        }
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn kio_server_variants(m: &SmbMount) -> Vec<String> {
    let server_variants = case_variants(&m.server);
    let username = m.username.trim();
    let domain = m.domain.trim();
    let mut out = Vec::new();
    for server in server_variants {
        out.push(server.clone());
        if !username.is_empty() {
            out.push(format!("{username}@{server}"));
            if !domain.is_empty() {
                out.push(format!("{domain};{username}@{server}"));
            }
        }
    }
    dedupe(out)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn case_variants(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    dedupe(vec![trimmed.to_string(), trimmed.to_ascii_lowercase()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            out.push(value);
        }
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn is_readable_dir(path: &str) -> bool {
    let path = std::path::Path::new(path);
    path.is_dir() && std::fs::read_dir(path).is_ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn try_gio_mount(m: &SmbMount) -> Result<(), String> {
    let gio = find_program("gio", &["/usr/bin/gio"]).ok_or("gio was not found")?;
    run_with_timeout(
        &gio,
        &["mount", &smb_uri(m)],
        std::time::Duration::from_secs(10),
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run_with_timeout(
    program: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("failed to read {program} output: {e}"))?;
                return Err(format!(
                    "{program} failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} timed out"));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => return Err(format!("failed to poll {program}: {e}")),
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn smb_uri(m: &SmbMount) -> String {
    let username = m.username.trim();
    let userinfo = if username.is_empty() {
        String::new()
    } else {
        format!("{}@", pct(username))
    };
    format!(
        "smb://{}{}/{}",
        userinfo,
        m.server.trim(),
        pct(m.share.trim())
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_mount_error(m: &SmbMount) -> String {
    format!(
        "Linux SMB needs a readable user-space FUSE mount for smb://{}/{}. Open the share in your file manager first, or install/enable kio-fuse or gvfs-fuse and try again. Vela will not request root for SMB by default.",
        m.server.trim(),
        m.share.trim()
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn current_uid() -> String {
    std::env::var("UID")
        .ok()
        .filter(|uid| !uid.trim().is_empty())
        .unwrap_or_else(|| command_stdout("id", &["-u"]).unwrap_or_else(|_| "1000".to_string()))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn command_stdout(program: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn find_program(name: &str, candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).is_file())
        .map(|p| (*p).to_string())
        .or_else(|| command_stdout("which", &[name]).ok())
}

/// Percent-encode credential components for the mount_smbfs URL authority.
#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
