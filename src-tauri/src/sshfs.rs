//! SSH/SFTP remote folders mounted through sshfs. Vela uses OpenSSH's normal
//! config, keys, and agent instead of storing SSH passwords.

use crate::config::SshMount;

pub fn default_mountpoint(m: &SshMount) -> Result<String, String> {
    let base = crate::config::config_dir_file("mounts").map_err(|e| e.to_string())?;
    let mp = base.join(format!(
        "ssh_{}_{}_{}",
        sanitize(&m.host),
        sanitize(&m.remote_path),
        m.id
    ));
    Ok(mp.to_string_lossy().to_string())
}

pub fn prepare_mount(m: &mut SshMount) -> Result<(), String> {
    m.mountpoint = default_mountpoint(m)?;
    mount(m)
}

#[cfg(unix)]
pub fn mount(m: &SshMount) -> Result<(), String> {
    validate(m)?;
    if is_mounted(&m.mountpoint) {
        return Ok(());
    }
    std::fs::create_dir_all(&m.mountpoint)
        .map_err(|e| format!("could not create mountpoint: {e}"))?;

    let sshfs = find_program(
        "sshfs",
        &[
            "/usr/bin/sshfs",
            "/usr/local/bin/sshfs",
            "/opt/homebrew/bin/sshfs",
        ],
    )
    .ok_or("sshfs was not found. Install sshfs, then try again.")?;
    let mut args = vec![
        remote_spec(m),
        m.mountpoint.clone(),
        "-p".to_string(),
        m.port.to_string(),
        "-o".to_string(),
        "reconnect".to_string(),
        "-o".to_string(),
        "ServerAliveInterval=15".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "follow_symlinks".to_string(),
    ];
    if !m.identity_file.trim().is_empty() {
        let identity = expand_user_path(m.identity_file.trim());
        if identity.to_string_lossy().contains(',') {
            let _ = std::fs::remove_dir(&m.mountpoint);
            return Err("identity file path cannot contain a comma".into());
        }
        if !identity.is_file() {
            let _ = std::fs::remove_dir(&m.mountpoint);
            return Err("identity file was not found".into());
        }
        args.push("-o".to_string());
        args.push(format!("IdentityFile={}", identity.to_string_lossy()));
    }

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = run_with_timeout(&sshfs, &arg_refs, std::time::Duration::from_secs(20));
    match result {
        Ok(()) if is_mounted(&m.mountpoint) => Ok(()),
        Ok(()) => {
            let _ = std::fs::remove_dir(&m.mountpoint);
            Err("sshfs exited successfully but the mountpoint is not mounted".into())
        }
        Err(e) => {
            let _ = std::fs::remove_dir(&m.mountpoint);
            Err(format!(
                "{e}. Vela uses SSH keys, ssh-agent, and ~/.ssh/config; it does not store SSH passwords. If this is the first connection to that host, trust the host key with ssh first."
            ))
        }
    }
}

#[cfg(not(unix))]
pub fn mount(_m: &SshMount) -> Result<(), String> {
    Err("SSH/SFTP folders currently require sshfs on Linux or macOS".into())
}

#[cfg(unix)]
pub fn unmount(mountpoint: &str) -> Result<(), String> {
    if !is_mounted(mountpoint) {
        let _ = std::fs::remove_dir(mountpoint);
        return Ok(());
    }
    let commands: Vec<(String, Vec<String>)> = if cfg!(target_os = "macos") {
        vec![("umount".to_string(), vec![mountpoint.to_string()])]
    } else {
        vec![
            (
                "fusermount3".to_string(),
                vec!["-u".to_string(), mountpoint.to_string()],
            ),
            (
                "fusermount".to_string(),
                vec!["-u".to_string(), mountpoint.to_string()],
            ),
            ("umount".to_string(), vec![mountpoint.to_string()]),
        ]
    };
    let mut last_err = None;
    for (name, args) in commands {
        let Some(program) = find_program(
            &name,
            &[&format!("/usr/bin/{name}"), &format!("/bin/{name}")],
        ) else {
            continue;
        };
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match run_with_timeout(&program, &arg_refs, std::time::Duration::from_secs(10)) {
            Ok(()) => {
                let _ = std::fs::remove_dir(mountpoint);
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "no user-space unmount command was found".into()))
}

#[cfg(not(unix))]
pub fn unmount(_mountpoint: &str) -> Result<(), String> {
    Ok(())
}

pub fn unmount_for_removal(mountpoint: &str) {
    if let Err(e) = unmount(mountpoint) {
        eprintln!("vela: sshfs unmount failed (the mount may linger): {e}");
    }
}

pub fn is_active_mount(m: &SshMount) -> bool {
    is_mounted(&m.mountpoint)
}

fn validate(m: &SshMount) -> Result<(), String> {
    if m.host.trim().is_empty() {
        return Err("host is required".into());
    }
    if m.remote_path.trim().is_empty() {
        return Err("remote path is required".into());
    }
    if m.port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    reject_newline("host", &m.host)?;
    reject_newline("username", &m.username)?;
    reject_newline("remote path", &m.remote_path)?;
    reject_newline("identity file", &m.identity_file)?;
    Ok(())
}

fn remote_spec(m: &SshMount) -> String {
    let user = m.username.trim();
    let authority = if user.is_empty() {
        ssh_host(&m.host)
    } else {
        format!("{user}@{}", ssh_host(&m.host))
    };
    format!("{authority}:{}", m.remote_path.trim())
}

fn ssh_host(host: &str) -> String {
    let host = host.trim();
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

#[cfg(unix)]
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

#[cfg(not(unix))]
fn is_mounted(_mountpoint: &str) -> bool {
    false
}

fn expand_user_path(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

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

fn find_program(name: &str, candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).is_file())
        .map(|p| (*p).to_string())
        .or_else(|| command_stdout("which", &[name]).ok())
}

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

fn reject_newline(field: &str, value: &str) -> Result<(), String> {
    if value.contains(['\n', '\r']) {
        Err(format!("{field} cannot contain a newline"))
    } else {
        Ok(())
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount() -> SshMount {
        SshMount {
            host: "media.example.test".into(),
            username: "michael".into(),
            remote_path: "/srv/media".into(),
            port: 2222,
            ..SshMount::default()
        }
    }

    #[test]
    fn remote_spec_includes_user_host_port_path_shape() {
        assert_eq!(
            remote_spec(&mount()),
            "michael@media.example.test:/srv/media"
        );
    }

    #[test]
    fn ipv6_host_is_bracketed() {
        let mut m = mount();
        m.host = "2001:db8::10".into();
        assert_eq!(remote_spec(&m), "michael@[2001:db8::10]:/srv/media");
    }
}
