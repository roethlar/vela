//! Native SMB client over libsmbclient (raw `pavao-sys` bindings).
//!
//! Linux-family only: macOS and Windows keep their OS mount flows
//! (`smb.rs`). Each [`SmbConnection`] owns its own `SMBCCTX`, so mounts with
//! different servers/credentials coexist and a long-lived stream never
//! shares a context with directory listings. libsmbclient contexts are not
//! thread-safe, so every call on a context runs under that connection's
//! mutex; callers must invoke these blocking functions from
//! `spawn_blocking` (or a dedicated thread), never on async workers.
//!
//! Credentials are delivered through libsmbclient's auth callback from a
//! process-global registry keyed by context pointer. They are never logged,
//! and never placed in the `smb://` URL (which libsmbclient would otherwise
//! echo into errors).

#![cfg(all(unix, not(target_os = "macos")))]

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::{Mutex, OnceLock};

use pavao_sys::{
    libsmb_file_info, smbc_close_fn, smbc_closedir_fn, smbc_free_context,
    smbc_getFunctionClose, smbc_getFunctionClosedir, smbc_getFunctionLseek,
    smbc_getFunctionOpen, smbc_getFunctionOpendir, smbc_getFunctionRead,
    smbc_getFunctionReaddirPlus, smbc_getFunctionStat, smbc_init_context,
    smbc_new_context, smbc_setFunctionAuthDataWithContext,
    smbc_setOptionNoAutoAnonymousLogin, smbc_setTimeout, SMBCCTX, SMBCFILE,
};
use std::os::raw::c_void;

/// FILE_ATTRIBUTE_DIRECTORY bit in `libsmb_file_info.attrs` (DOS attributes).
const DOS_ATTR_DIRECTORY: u16 = 0x10;

/// Whether a DOS attribute word marks a directory.
fn is_dir_attrs(attrs: u16) -> bool {
    attrs & DOS_ATTR_DIRECTORY != 0
}

/// One directory entry or stat result. (Positioned reads arrive with their
/// first user, the stream proxy.)
#[derive(Debug, Clone)]
pub struct SmbEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

struct Creds {
    username: String,
    password: String,
    workgroup: String,
}

/// Credentials for live contexts, keyed by context pointer. Entries live
/// exactly as long as the owning `SmbConnection` (inserted after
/// `smbc_init_context`, removed in `Drop` before the context is freed).
fn creds_registry() -> &'static Mutex<HashMap<usize, Creds>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, Creds>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// libsmbclient context creation/free is serialized process-wide: per-context
/// calls are safe under each connection's lock, but samba's context setup
/// touches shared global state.
fn ctx_lifecycle_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Copy `value` into a fixed-size C buffer, truncating but always
/// NUL-terminating. Interior NULs are dropped so the credential can't be
/// silently cut short at an attacker-chosen point.
fn write_cstr_buf(dst: *mut c_char, dst_len: usize, value: &str) {
    if dst.is_null() || dst_len == 0 {
        return;
    }
    let clean: Vec<u8> = value.bytes().filter(|&b| b != 0).collect();
    let n = clean.len().min(dst_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(clean.as_ptr(), dst as *mut u8, n);
        *dst.add(n) = 0;
    }
}

extern "C" fn auth_callback(
    ctx: *mut SMBCCTX,
    _server: *const c_char,
    _share: *const c_char,
    wg: *mut c_char,
    wg_len: c_int,
    un: *mut c_char,
    un_len: c_int,
    pw: *mut c_char,
    pw_len: c_int,
) {
    let registry = match creds_registry().lock() {
        Ok(r) => r,
        Err(_) => return, // poisoned: leave buffers untouched → auth fails closed
    };
    if let Some(creds) = registry.get(&(ctx as usize)) {
        write_cstr_buf(wg, wg_len.max(0) as usize, &creds.workgroup);
        write_cstr_buf(un, un_len.max(0) as usize, &creds.username);
        write_cstr_buf(pw, pw_len.max(0) as usize, &creds.password);
    }
}

/// Build the `smb://` URL for a share-relative path (`""` or `"a/b"`, as
/// produced by the command layer's `normalize_smb_relative_path`). No
/// credentials ever go in the URL.
pub fn smb_url(server: &str, share: &str, relative: &str) -> String {
    let server = server.trim().trim_matches('/');
    let share = share.trim().trim_matches('/');
    let mut url = format!("smb://{server}/{share}");
    for part in relative.split('/').filter(|p| !p.is_empty()) {
        url.push('/');
        url.push_str(part);
    }
    url
}

/// Map an OS error from libsmbclient to a user-facing message that names the
/// condition without echoing URLs or credentials.
fn friendly_error(action: &str, err: std::io::Error) -> String {
    use std::io::ErrorKind;
    let detail = match err.kind() {
        ErrorKind::PermissionDenied => {
            "access denied — check the username, password, and share permissions".to_string()
        }
        ErrorKind::NotFound => "no such share or folder on the server".to_string(),
        ErrorKind::TimedOut | ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => {
            "the server did not respond — check the address and that it is online".to_string()
        }
        ErrorKind::ConnectionRefused => {
            "the server refused the connection — is SMB enabled?".to_string()
        }
        _ => err.to_string(),
    };
    format!("{action}: {detail}")
}

fn last_error(action: &str) -> String {
    friendly_error(action, std::io::Error::last_os_error())
}

/// Raw context pointer. Only ever touched while holding the owning
/// connection's mutex (or exclusively during connect/drop).
struct Ctx(*mut SMBCCTX);
// SAFETY: the pointer is only dereferenced under `SmbConnection::ctx`'s
// Mutex, giving each context single-threaded access; libsmbclient supports
// distinct contexts on distinct threads.
unsafe impl Send for Ctx {}

pub struct SmbConnection {
    ctx: Mutex<Ctx>,
    server: String,
    share: String,
    /// 10s per-operation network timeout, mirrored from connect for fstat use.
    _timeout_ms: i32,
}

const OP_TIMEOUT_MS: i32 = 10_000;

impl SmbConnection {
    /// Create a context for `server`/`share`. Empty `username` means
    /// guest/anonymous. Reachability and credentials are verified lazily on
    /// the first operation (open/stat/list); add-time callers use
    /// [`verify_mount`] for an explicit up-front check. The stream/browse hot
    /// paths deliberately skip that extra share-root round-trip — on a seek
    /// mpv reconnects and rebuilds a session, and re-enumerating the root each
    /// time was a felt freeze on a real NAS.
    pub fn connect(
        server: &str,
        share: &str,
        username: &str,
        password: &str,
        domain: &str,
    ) -> Result<Self, String> {
        let ctx = {
            let _guard = ctx_lifecycle_lock()
                .lock()
                .map_err(|_| "SMB context lock poisoned".to_string())?;
            unsafe {
                let raw = smbc_new_context();
                if raw.is_null() {
                    return Err(last_error("could not create SMB context"));
                }
                smbc_setFunctionAuthDataWithContext(raw, Some(auth_callback));
                smbc_setTimeout(raw, OP_TIMEOUT_MS);
                if !username.trim().is_empty() {
                    // With real credentials, make a bad password fail loudly
                    // instead of silently downgrading to an anonymous session.
                    smbc_setOptionNoAutoAnonymousLogin(raw, 1);
                }
                let initialized = smbc_init_context(raw);
                if initialized.is_null() {
                    let err = last_error("could not initialize SMB context");
                    smbc_free_context(raw, 1);
                    return Err(err);
                }
                creds_registry()
                    .lock()
                    .map_err(|_| "SMB credential registry poisoned".to_string())?
                    .insert(
                        initialized as usize,
                        Creds {
                            username: username.trim().to_string(),
                            password: password.to_string(),
                            workgroup: domain.trim().to_string(),
                        },
                    );
                initialized
            }
        };
        let conn = SmbConnection {
            ctx: Mutex::new(Ctx(ctx)),
            server: server.trim().to_string(),
            share: share.trim().to_string(),
            _timeout_ms: OP_TIMEOUT_MS,
        };
        Ok(conn)
    }

    fn url(&self, relative: &str) -> String {
        smb_url(&self.server, &self.share, relative)
    }

    /// List a share-relative directory. Blocking; run under spawn_blocking.
    pub fn list_dir(&self, relative: &str) -> Result<Vec<SmbEntry>, String> {
        let url = CString::new(self.url(relative))
            .map_err(|_| "SMB path contains a NUL byte".to_string())?;
        let guard = self
            .ctx
            .lock()
            .map_err(|_| "SMB connection lock poisoned".to_string())?;
        let ctx = guard.0;
        unsafe {
            let opendir = smbc_getFunctionOpendir(ctx)
                .ok_or("libsmbclient has no opendir function")?;
            let readdirplus = smbc_getFunctionReaddirPlus(ctx)
                .ok_or("libsmbclient has no readdirplus function")?;
            let closedir: smbc_closedir_fn = smbc_getFunctionClosedir(ctx);
            let closedir = closedir.ok_or("libsmbclient has no closedir function")?;

            let dir = opendir(ctx, url.as_ptr());
            if dir.is_null() {
                return Err(last_error("could not open SMB folder"));
            }
            let mut out = Vec::new();
            loop {
                // readdirplus returns NULL both at end-of-dir and on error;
                // clear errno first so we can tell them apart.
                *libc::__errno_location() = 0;
                let info: *mut libsmb_file_info = readdirplus(ctx, dir);
                if info.is_null() {
                    let errno = *libc::__errno_location();
                    closedir(ctx, dir);
                    if errno != 0 {
                        return Err(friendly_error(
                            "could not read SMB folder",
                            std::io::Error::from_raw_os_error(errno),
                        ));
                    }
                    break;
                }
                let name_ptr = (*info).name;
                if name_ptr.is_null() {
                    continue;
                }
                let name = CStr::from_ptr(name_ptr).to_string_lossy().to_string();
                if name == "." || name == ".." {
                    continue;
                }
                out.push(SmbEntry {
                    name,
                    is_dir: is_dir_attrs((*info).attrs),
                    size: (*info).size as u64,
                });
            }
            Ok(out)
        }
    }

    /// Stat one share-relative path. Blocking; run under spawn_blocking.
    pub fn stat(&self, relative: &str) -> Result<SmbEntry, String> {
        let url = CString::new(self.url(relative))
            .map_err(|_| "SMB path contains a NUL byte".to_string())?;
        let name = relative
            .rsplit('/')
            .find(|p| !p.is_empty())
            .unwrap_or(&self.share)
            .to_string();
        let guard = self
            .ctx
            .lock()
            .map_err(|_| "SMB connection lock poisoned".to_string())?;
        let ctx = guard.0;
        unsafe {
            let stat_fn =
                smbc_getFunctionStat(ctx).ok_or("libsmbclient has no stat function")?;
            let mut st: libc::stat = std::mem::zeroed();
            if stat_fn(ctx, url.as_ptr(), &mut st) != 0 {
                return Err(last_error("could not stat SMB path"));
            }
            Ok(SmbEntry {
                name,
                is_dir: (st.st_mode & libc::S_IFMT) == libc::S_IFDIR,
                size: st.st_size.max(0) as u64,
            })
        }
    }

    /// Open a share-relative file for positioned reads (streaming). The
    /// length comes from a URL stat (pavao-sys binds no fstat). Blocking;
    /// run off async workers.
    pub fn open_read(&self, relative: &str) -> Result<SmbReadHandle<'_>, String> {
        let len = self.stat(relative)?.size;
        let file = self.open_raw(relative)?;
        Ok(SmbReadHandle {
            conn: self,
            file,
            len,
        })
    }

    /// Like [`open_read`] but trusts a caller-supplied `len` instead of a
    /// fresh network `stat`. The stream proxy uses this on a seek, where the
    /// entity length was already learned (and cached) on the first request,
    /// so a seek makes no redundant round-trip. Blocking; run off async
    /// workers.
    pub fn open_read_with_len(
        &self,
        relative: &str,
        len: u64,
    ) -> Result<SmbReadHandle<'_>, String> {
        let file = self.open_raw(relative)?;
        Ok(SmbReadHandle {
            conn: self,
            file,
            len,
        })
    }

    /// Open the file and return the raw handle. The context lock is held only
    /// for the `open` call and released on return; positioned reads re-take it
    /// per op. Blocking; run off async workers.
    fn open_raw(&self, relative: &str) -> Result<*mut SMBCFILE, String> {
        let url = CString::new(self.url(relative))
            .map_err(|_| "SMB path contains a NUL byte".to_string())?;
        let guard = self
            .ctx
            .lock()
            .map_err(|_| "SMB connection lock poisoned".to_string())?;
        let ctx = guard.0;
        unsafe {
            let open_fn =
                smbc_getFunctionOpen(ctx).ok_or("libsmbclient has no open function")?;
            let file = open_fn(ctx, url.as_ptr(), libc::O_RDONLY, 0);
            if file.is_null() {
                return Err(last_error("could not open SMB file"));
            }
            Ok(file)
        }
    }

    /// Read a whole small file (sidecars), refusing anything over `cap`
    /// bytes so a mislabeled path can't balloon memory.
    pub fn read_small(&self, relative: &str, cap: u64) -> Result<Vec<u8>, String> {
        let handle = self.open_read(relative)?;
        if handle.is_empty() {
            return Ok(Vec::new());
        }
        if handle.len() > cap {
            return Err("file too large for a sidecar read".into());
        }
        let mut out = vec![0u8; handle.len() as usize];
        let mut filled = 0usize;
        while filled < out.len() {
            let n = handle.read_at(filled as u64, &mut out[filled..])?;
            if n == 0 {
                out.truncate(filled); // shrank mid-read; serve what exists
                break;
            }
            filled += n;
        }
        Ok(out)
    }
}

/// Best-effort close, used while holding the ctx lock.
unsafe fn close_quiet(ctx: *mut SMBCCTX, file: *mut SMBCFILE) {
    let close_fn: smbc_close_fn = smbc_getFunctionClose(ctx);
    if let Some(f) = close_fn {
        f(ctx, file);
    }
}

/// An open SMB file supporting positioned reads. Borrows its connection, so
/// the context outlives every handle by construction; each read serializes
/// on the connection mutex.
pub struct SmbReadHandle<'a> {
    conn: &'a SmbConnection,
    file: *mut SMBCFILE,
    len: u64,
}

// SAFETY: `file` is only used under the owning connection's mutex.
unsafe impl Send for SmbReadHandle<'_> {}

impl SmbReadHandle<'_> {
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Read up to `buf.len()` bytes at absolute `offset`. Returns bytes
    /// read (0 at EOF). Blocking; run off async workers.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, String> {
        let guard = self
            .conn
            .ctx
            .lock()
            .map_err(|_| "SMB connection lock poisoned".to_string())?;
        let ctx = guard.0;
        unsafe {
            let lseek_fn =
                smbc_getFunctionLseek(ctx).ok_or("libsmbclient has no lseek function")?;
            let read_fn =
                smbc_getFunctionRead(ctx).ok_or("libsmbclient has no read function")?;
            let off = libc::off_t::try_from(offset).map_err(|_| "offset too large")?;
            if lseek_fn(ctx, self.file, off, libc::SEEK_SET) < 0 {
                return Err(last_error("could not seek SMB file"));
            }
            let n = read_fn(ctx, self.file, buf.as_mut_ptr() as *mut c_void, buf.len());
            if n < 0 {
                return Err(last_error("could not read SMB file"));
            }
            Ok(n as usize)
        }
    }
}

impl Drop for SmbReadHandle<'_> {
    fn drop(&mut self) {
        if let Ok(guard) = self.conn.ctx.lock() {
            unsafe { close_quiet(guard.0, self.file) };
        }
    }
}

impl Drop for SmbConnection {
    fn drop(&mut self) {
        let ctx = match self.ctx.lock() {
            Ok(guard) => guard.0,
            Err(poisoned) => poisoned.into_inner().0,
        };
        if let Ok(mut registry) = creds_registry().lock() {
            registry.remove(&(ctx as usize));
        }
        let _guard = ctx_lifecycle_lock().lock();
        unsafe {
            smbc_free_context(ctx, 1);
        }
    }
}

/// Connect using a configured mount record. Blocking; run under
/// spawn_blocking.
pub fn connect_mount(m: &crate::config::SmbMount) -> Result<SmbConnection, String> {
    SmbConnection::connect(&m.server, &m.share, &m.username, &m.password, &m.domain)
}

/// Verify server/share/credentials at add time by connecting and listing the
/// share root once. `connect` itself no longer lists the root (it is on the
/// per-seek hot path), so this explicit check is the single place add-time
/// callers pay for reachability. Blocking; run under spawn_blocking.
pub fn verify_mount(m: &crate::config::SmbMount) -> Result<(), String> {
    connect_mount(m)?.list_dir("").map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smb_url_joins_server_share_and_relative() {
        assert_eq!(smb_url("nas", "media", ""), "smb://nas/media");
        assert_eq!(
            smb_url("10.1.10.206", "media", "movies/4k"),
            "smb://10.1.10.206/media/movies/4k"
        );
    }

    #[test]
    fn smb_url_normalizes_stray_slashes() {
        assert_eq!(smb_url(" nas ", "/media/", ""), "smb://nas/media");
        assert_eq!(smb_url("nas", "media", "a//b/"), "smb://nas/media/a/b");
        // Sub-share config like "media/movies" keeps its inner slash.
        assert_eq!(smb_url("nas", "media/movies", ""), "smb://nas/media/movies");
    }

    #[test]
    fn write_cstr_buf_truncates_and_nul_terminates() {
        let mut buf = [0x7f_i8 as c_char; 6];
        write_cstr_buf(buf.as_mut_ptr(), buf.len(), "secretpassword");
        let bytes: Vec<u8> = buf.iter().map(|&b| b as u8).collect();
        assert_eq!(&bytes[..5], b"secre");
        assert_eq!(bytes[5], 0);
    }

    #[test]
    fn write_cstr_buf_strips_interior_nuls() {
        let mut buf = [0x7f_i8 as c_char; 8];
        write_cstr_buf(buf.as_mut_ptr(), buf.len(), "a\0bc");
        let bytes: Vec<u8> = buf.iter().map(|&b| b as u8).collect();
        assert_eq!(&bytes[..3], b"abc");
        assert_eq!(bytes[3], 0);
    }

    #[test]
    fn write_cstr_buf_handles_empty_and_tiny_buffers() {
        write_cstr_buf(std::ptr::null_mut(), 4, "x"); // must not crash
        let mut buf = [0x7f_i8 as c_char; 1];
        write_cstr_buf(buf.as_mut_ptr(), buf.len(), "value");
        assert_eq!(buf[0], 0); // room only for the terminator
    }

    /// Opt-in live probe: `VELA_SMB_LIVE=server/share cargo test live_probe`
    /// connects anonymously to a real server. It asserts only that the FFI
    /// path completes without crashing or hanging and reports the outcome —
    /// on a credentialed share the expected result is a friendly
    /// access-denied error, which exercises context setup, the auth
    /// callback, opendir, and errno mapping over the wire.
    #[test]
    fn live_probe_env_gated() {
        let Ok(target) = std::env::var("VELA_SMB_LIVE") else {
            return;
        };
        let (server, share) = target.split_once('/').expect("VELA_SMB_LIVE=server/share");
        let result = SmbConnection::connect(server, share, "", "", "");
        match result {
            Ok(conn) => {
                let entries = conn.list_dir("").expect("connected but listing failed");
                eprintln!("live probe: connected, {} entries in root", entries.len());
            }
            Err(e) => eprintln!("live probe: connect returned error: {e}"),
        }
    }

    #[test]
    fn dos_directory_attr_maps_to_is_dir() {
        assert!(is_dir_attrs(0x10)); // directory
        assert!(is_dir_attrs(0x30)); // directory + archive
        assert!(!is_dir_attrs(0x20)); // archive only (plain file)
        assert!(!is_dir_attrs(0x02)); // hidden file
    }
}
