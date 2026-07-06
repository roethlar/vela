//! SSH/SFTP remote folders mounted through sshfs. Vela uses OpenSSH's normal
//! config, keys, and agent instead of storing SSH passwords.

use crate::config::SshMount;

/// Locate the sshfs binary Vela would use: PATH (via `which`) plus the
/// well-known install locations. Shared by `mount` and the add-SSH UI's
/// status check (`sshfs_status` command).
pub fn locate() -> Option<String> {
    find_program(
        "sshfs",
        &[
            "/usr/bin/sshfs",
            "/usr/local/bin/sshfs",
            "/opt/homebrew/bin/sshfs",
        ],
    )
}

/// Platform-aware "sshfs missing" message. On macOS the generic "install
/// sshfs" advice is a dead end (Homebrew core's formula needs Linux-only
/// libfuse), so spell out the macFUSE route instead.
#[cfg(unix)]
fn not_found_message() -> String {
    if cfg!(target_os = "macos") {
        "sshfs was not found. On macOS: install macFUSE (brew install --cask macfuse), \
         approve its system extension, then install a macFUSE-compatible sshfs build \
         (brew install gromgit/fuse/sshfs-mac) and try again."
            .to_string()
    } else {
        "sshfs was not found. Install sshfs with your package manager, then try again."
            .to_string()
    }
}

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

/// The sshfs `-o` options Vela always passes.
///
/// `max_conns=4` opens parallel SFTP channels. With sshfs's single default
/// channel, a seek's read head-of-line-blocks behind the outstanding
/// sequential-readahead backlog on that one channel; on a real latency-bearing
/// link (a NAS) that stalls playback for seconds on every seek (Bug 2). Parallel
/// channels let a seek's read proceed on a free channel instead of queuing. 4 is
/// a conservative default — enough to break the head-of-line stall without
/// tripping a server's per-session limits. The remaining options: auto-reconnect
/// on a dropped connection, liveness probes, key-only auth (no password prompts),
/// and symlink following.
pub(crate) const SSHFS_OPTIONS: &[&str] = &[
    "reconnect",
    "ServerAliveInterval=15",
    "ServerAliveCountMax=3",
    "BatchMode=yes",
    "follow_symlinks",
    "max_conns=4",
];

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

    let sshfs = locate().ok_or_else(not_found_message)?;
    let mut args = vec![
        remote_spec(m),
        m.mountpoint.clone(),
        "-p".to_string(),
        m.port.to_string(),
    ];
    for opt in SSHFS_OPTIONS {
        args.push("-o".to_string());
        args.push((*opt).to_string());
    }
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

    // Bare "install sshfs" advice is a dead end on macOS (Homebrew core's
    // formula needs Linux-only libfuse), so the message must name the actual
    // macFUSE route there and package-manager advice elsewhere.
    #[cfg(unix)]
    #[test]
    fn not_found_message_is_platform_aware() {
        let msg = not_found_message();
        if cfg!(target_os = "macos") {
            assert!(msg.contains("macfuse"), "macOS message must name macFUSE: {msg}");
            assert!(msg.contains("sshfs-mac"), "macOS message must name the tap build: {msg}");
        } else {
            assert!(msg.contains("package manager"), "non-mac message: {msg}");
        }
    }

    // Bug 2: the SSH seek stall comes from a single SFTP channel head-of-line-
    // blocking a seek's read behind the readahead backlog. Vela must request
    // parallel connections. Removing `max_conns` from SSHFS_OPTIONS fails this.
    #[test]
    fn sshfs_options_request_parallel_sftp_channels() {
        let n: u32 = SSHFS_OPTIONS
            .iter()
            .find_map(|o| o.strip_prefix("max_conns="))
            .expect("SSHFS_OPTIONS must include max_conns for parallel channels")
            .parse()
            .expect("max_conns must be numeric");
        assert!(n >= 2, "max_conns must be >= 2 to break head-of-line blocking, got {n}");
    }

    // Hermetic functional guard (owner decision 2026-07-05): stand up a loopback
    // sshd, mount it with sshfs using Vela's exact SSHFS_OPTIONS (max_conns and
    // all), and read a file back. Proves the option set is accepted by the
    // installed sshfs and that max_conns coexists with `reconnect` and mounts +
    // reads correctly — guarding against an incompatible/broken option string.
    // (It does NOT reproduce the latency-driven stall — a localhost sshd has ~0
    // latency; the owner's NAS playtest is the authoritative stall-fix check.)
    // Gated on sshd/sshfs/ssh-keygen being present; skips with a message if not.
    #[cfg(target_os = "linux")]
    #[test]
    fn max_conns_option_set_mounts_and_reads_over_loopback_sshd() {
        use std::net::{TcpListener, TcpStream};
        use std::path::PathBuf;
        use std::process::{Child, Command};
        use std::time::{Duration, Instant};

        let (Some(sshd), Some(sshfs), Some(keygen)) = (
            find_program("sshd", &["/usr/bin/sshd", "/usr/sbin/sshd"]),
            locate(),
            find_program("ssh-keygen", &["/usr/bin/ssh-keygen"]),
        ) else {
            eprintln!("skipping loopback sshfs test: sshd/sshfs/ssh-keygen not all present");
            return;
        };

        // Cleanup on drop, even if an assertion panics: unmount the FUSE mount,
        // kill sshd, remove the temp tree.
        struct Cleanup {
            base: PathBuf,
            mnt: PathBuf,
            sshd: Option<Child>,
        }
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for prog in ["fusermount3", "fusermount"] {
                    let _ = Command::new(prog).arg("-u").arg(&self.mnt).status();
                }
                let _ = Command::new("umount").arg(&self.mnt).status();
                if let Some(mut c) = self.sshd.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                let _ = std::fs::remove_dir_all(&self.base);
            }
        }

        let base = std::env::temp_dir().join(format!("vela-sshfs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let srv = base.join("srv");
        let mnt = base.join("mnt");
        let keys = base.join("keys");
        for d in [&srv, &mnt, &keys] {
            std::fs::create_dir_all(d).unwrap();
        }
        let content = "hello-from-vela-sshfs\n";
        std::fs::write(srv.join("probe.txt"), content).unwrap();

        for name in ["host", "client"] {
            let ok = Command::new(&keygen)
                .args(["-q", "-t", "ed25519", "-N", ""])
                .arg("-f")
                .arg(keys.join(name))
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            assert!(ok, "ssh-keygen {name} failed");
        }
        std::fs::copy(keys.join("client.pub"), keys.join("authorized_keys")).unwrap();

        // A free loopback port: bind :0, read it back, release it for sshd.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let cfg = format!(
            "Port {port}\nListenAddress 127.0.0.1\nHostKey {host}\nAuthorizedKeysFile {ak}\n\
             PidFile {pid}\nUsePAM no\nStrictModes no\nPasswordAuthentication no\n\
             PubkeyAuthentication yes\nSubsystem sftp internal-sftp\n",
            host = keys.join("host").display(),
            ak = keys.join("authorized_keys").display(),
            pid = keys.join("sshd.pid").display(),
        );
        std::fs::write(keys.join("sshd_config"), cfg).unwrap();

        let child = Command::new(&sshd)
            .arg("-D")
            .arg("-f")
            .arg(keys.join("sshd_config"))
            .arg("-E")
            .arg(keys.join("sshd.log"))
            .spawn()
            .expect("spawn sshd");
        let mut guard = Cleanup { base: base.clone(), mnt: mnt.clone(), sshd: Some(child) };

        // Wait for sshd to accept connections.
        let deadline = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if Instant::now() > deadline {
                let log = std::fs::read_to_string(keys.join("sshd.log")).unwrap_or_default();
                panic!("sshd did not start listening on {port}: {log}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .expect("USER/LOGNAME set");
        let mut args: Vec<String> = vec![
            format!("{user}@127.0.0.1:{}", srv.display()),
            mnt.to_string_lossy().into_owned(),
            "-p".into(),
            port.to_string(),
        ];
        // Vela's real option set — this is what the test guards.
        for opt in SSHFS_OPTIONS {
            args.push("-o".into());
            args.push((*opt).into());
        }
        args.push("-o".into());
        args.push(format!("IdentityFile={}", keys.join("client").display()));
        // Test scaffolding only: trust the throwaway host key without touching
        // the user's known_hosts (production relies on a pre-trusted host).
        args.push("-o".into());
        args.push("UserKnownHostsFile=/dev/null".into());
        args.push("-o".into());
        args.push("StrictHostKeyChecking=no".into());

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        // sshfs backgrounds itself once mounted, so status() returns promptly.
        let status = run_with_timeout(&sshfs, &arg_refs, Duration::from_secs(20));
        assert!(status.is_ok(), "sshfs mount with {SSHFS_OPTIONS:?} failed: {status:?}");

        assert!(is_mounted(&mnt.to_string_lossy()), "mountpoint is not mounted");
        let got = std::fs::read_to_string(mnt.join("probe.txt")).expect("read probe.txt over sshfs");
        assert_eq!(got, content, "file content mismatch over the max_conns mount");

        // Explicit unmount so the read handle is released before cleanup; the
        // guard still cleans up on any panic above.
        for prog in ["fusermount3", "fusermount"] {
            let _ = Command::new(prog).arg("-u").arg(&mnt).status();
        }
        if let Some(mut c) = guard.sshd.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
