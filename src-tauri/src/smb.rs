//! OS-level SMB mounting for macOS (`mount_smbfs`) and Windows (`net use`),
//! where the OS mounts rootlessly and the local source browses the mounted
//! path. Linux does NOT mount: it speaks SMB natively via
//! `smb_client`/`smb_vfs` and streams playback through `stream_proxy` (see
//! `.agents/plans/smb-native-client.md`), so the Linux surface here is
//! only the no-op teardown hooks the cross-platform command paths call.
//!
//! The mount target is `SmbMount::mountpoint` on macOS/Windows; on Linux
//! it stays empty as the marker of a native (mountless) share record.

#[cfg(not(all(unix, not(target_os = "macos"))))]
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

/// Percent-encode credential components for the mount_smbfs URL authority.
#[cfg(target_os = "macos")]
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
