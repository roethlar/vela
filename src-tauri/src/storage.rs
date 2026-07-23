//! Shared durable JSON-file mechanics for Vela-owned state.
//!
//! Settings, connections, and playlists deliberately use separate files, but
//! they need the same failure properties: an unreadable/corrupt file is never
//! treated as an empty one, every update is serialized across threads and
//! processes, and a reader never observes a partial write.

use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A file inside the app's config directory, creating the directory if needed.
pub fn config_dir_file(name: &str) -> io::Result<PathBuf> {
    let proj = ProjectDirs::from("com", "vela", "vela")
        .ok_or_else(|| io::Error::other("could not determine a config directory"))?;
    let dir = proj.config_dir();
    ensure_private_directory(dir)?;
    Ok(dir.join(name))
}

fn ensure_private_directory(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        fs::create_dir_all(dir)?;
        windows_private::harden(dir, true)?;
    }
    #[cfg(not(any(unix, windows)))]
    fs::create_dir_all(dir)?;
    Ok(())
}

pub(crate) fn harden_existing_regular(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        if fs::metadata(path)?.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "durable file is not owner-only",
            ));
        }
    }
    #[cfg(windows)]
    windows_private::harden(path, false)?;
    Ok(())
}

fn require_regular_or_absent(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            harden_existing_regular(path)?;
            Ok(true)
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "durable path is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Read a JSON object. Only a genuinely absent path becomes `Default`; a
/// dangling symlink, permission error, or parse error fails closed.
pub fn load_json<T>(path: &Path) -> io::Result<T>
where
    T: DeserializeOwned + Default,
{
    if !require_regular_or_absent(path)? {
        return Ok(T::default());
    }
    let mut json = String::new();
    fs::File::open(path)?.read_to_string(&mut json)?;
    serde_json::from_str(&json).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn read_regular_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    if !require_regular_or_absent(path)? {
        return Ok(None);
    }
    fs::read(path).map(Some)
}

fn serialized_json<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn private_temp(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(windows)]
        windows_private::harden(parent, true)?;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    let tmp = path.with_extension(format!(
        "{extension}.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let write_result = (|| {
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)?
        };
        #[cfg(not(unix))]
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(windows)]
        windows_private::harden(&tmp, false)?;
        Ok::<(), io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(tmp)
}

/// Move one regular file without replacing any existing destination.
///
/// Recovery must preserve the canonical source on a destination collision, so
/// this uses each supported platform's atomic no-replace rename primitive.
pub(crate) fn rename_noreplace(source: &Path, destination: &Path) -> io::Result<()> {
    require_regular_or_absent(source)?.then_some(()).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "rename source is absent")
    })?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "rename destination already exists",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid source path"))?;
        let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid destination path")
        })?;
        // SAFETY: both C strings are NUL-terminated and live for this call.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid source path"))?;
        let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid destination path")
        })?;
        // SAFETY: both C strings are NUL-terminated and live for this call.
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let source = source
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let destination = destination
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: both UTF-16 buffers are NUL-terminated and live for this call.
        unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(destination.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(io::Error::other)?;
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no atomic no-replace rename is available",
    ));

    sync_parent(destination)
}

/// Atomically replace one JSON file. On Unix the temporary file is owner-only
/// from its first byte; it is never written and chmodded afterward.
pub fn save_json<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    require_regular_or_absent(path)?;
    let bytes = serialized_json(value)?;
    let tmp = private_temp(path, &bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => {
            sync_parent(path)?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

/// Atomically install a new JSON file without replacing any existing path.
pub(crate) fn install_json_new<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    if require_regular_or_absent(path)? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "durable file already exists",
        ));
    }
    let bytes = serialized_json(value)?;
    let tmp = private_temp(path, &bytes)?;
    match rename_noreplace(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

pub(crate) fn write_private_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if require_regular_or_absent(path)? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "backup already exists",
        ));
    }
    let tmp = private_temp(path, bytes)?;
    if let Err(error) = rename_noreplace(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    let verify = fs::read(path)?;
    if verify != bytes {
        return Err(io::Error::other("backup verification failed"));
    }
    sync_parent(path)
}

pub(crate) fn remove_private_regular(path: &Path) -> io::Result<()> {
    if !require_regular_or_absent(path)? {
        return Ok(());
    }
    fs::remove_file(path)?;
    sync_parent(path)
}

pub(crate) fn open_private_lock(path: &Path) -> io::Result<fs::File> {
    require_regular_or_absent(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(windows)]
        windows_private::harden(parent, true)?;
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?
    };
    #[cfg(not(unix))]
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    harden_existing_regular(path)?;
    Ok(file)
}

pub(crate) fn is_private_regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file()) && {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o077 == 0)
        }
        #[cfg(windows)]
        {
            windows_private::verify(path).is_ok()
        }
        #[cfg(not(any(unix, windows)))]
        {
            true
        }
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Load, mutate, and atomically save one JSON object while holding both its
/// in-process mutex and mandatory cross-process advisory lock.
pub fn update_json<T, R, F>(
    label: &str,
    process_lock: &Mutex<()>,
    path: &Path,
    lock_path: &Path,
    mutate: F,
) -> Result<R, String>
where
    T: DeserializeOwned + Serialize + Default,
    F: FnOnce(&mut T) -> Result<R, String>,
{
    update_json_before_save(label, process_lock, path, lock_path, mutate, |_| Ok(()))
}

/// The strict settings and connections stores preserve their exact valid
/// pre-write bytes while the owning locks are still held.
pub(crate) fn update_json_before_save<T, R, F, B>(
    label: &str,
    process_lock: &Mutex<()>,
    path: &Path,
    lock_path: &Path,
    mutate: F,
    before_save: B,
) -> Result<R, String>
where
    T: DeserializeOwned + Serialize + Default,
    F: FnOnce(&mut T) -> Result<R, String>,
    B: FnOnce(Option<&[u8]>) -> Result<(), String>,
{
    let _guard = process_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let lock_file = open_private_lock(lock_path)
        .map_err(|error| format!("could not open {label} lock: {error}"))?;
    lock_file
        .lock()
        .map_err(|error| format!("could not acquire {label} lock: {error}"))?;

    let original =
        read_regular_bytes(path).map_err(|error| format!("could not read {label}: {error}"))?;
    let mut value = match original.as_deref() {
        Some(bytes) => serde_json::from_slice(bytes)
            .map_err(|error| format!("could not read {label}: {error}"))?,
        None => T::default(),
    };
    let output = mutate(&mut value)?;
    before_save(original.as_deref())?;
    save_json(path, &value).map_err(|error| format!("could not save {label}: {error}"))?;
    Ok(output)
}

#[cfg(windows)]
mod windows_private {
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        GetTokenInformation, SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: OpenProcessToken returned this owned handle and this is
            // its only closing path.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    fn io_error(context: &'static str, error: impl std::fmt::Display) -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{context}: {error}"),
        )
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        // SAFETY: the current-process pseudo-handle is always valid; `token`
        // points to writable storage for the returned owned handle.
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(|error| io_error("could not open the current Windows user token", error))?;
        let token = OwnedHandle(token);

        let mut byte_len = 0;
        // The sizing call is expected to report insufficient buffer. A
        // nonzero returned size is the only result used from it.
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut byte_len) };
        if byte_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "could not size the current Windows user identity",
            ));
        }
        let word = std::mem::size_of::<usize>();
        let mut info = vec![0usize; (byte_len as usize).div_ceil(word)];
        // SAFETY: `info` is aligned and has at least `byte_len` writable bytes.
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(info.as_mut_ptr().cast::<c_void>()),
                byte_len,
                &mut byte_len,
            )
        }
        .map_err(|error| io_error("could not read the current Windows user identity", error))?;
        // SAFETY: a successful TokenUser query initializes a TOKEN_USER at
        // the beginning of the aligned output buffer.
        let user = unsafe { &*info.as_ptr().cast::<TOKEN_USER>() };
        let mut string_sid = PWSTR::null();
        // SAFETY: the SID belongs to `info` for this call and `string_sid`
        // points to writable storage for the LocalAlloc result.
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut string_sid) }.map_err(|error| {
            io_error("could not encode the current Windows user identity", error)
        })?;
        // SAFETY: ConvertSidToStringSidW returned a NUL-terminated string.
        let result = unsafe { string_sid.to_string() }
            .map_err(|error| io_error("Windows returned an invalid user identity", error));
        // SAFETY: ConvertSidToStringSidW allocates this buffer with LocalAlloc.
        unsafe {
            LocalFree(Some(HLOCAL(string_sid.as_ptr().cast::<c_void>())));
        }
        result
    }

    fn read_dacl_sddl(path: &Path) -> io::Result<String> {
        let wide_path = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the path is NUL-terminated and `descriptor` points to
        // writable storage for the LocalAlloc result.
        unsafe {
            GetNamedSecurityInfoW(
                PCWSTR::from_raw(wide_path.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                &mut descriptor,
            )
        }
        .ok()
        .map_err(|error| io_error("could not inspect Vela's private Windows ACL", error))?;

        let mut text = PWSTR::null();
        // SAFETY: `descriptor` is a valid security descriptor returned above
        // and `text` points to writable storage for the LocalAlloc result.
        let converted = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                None,
            )
        };
        let result = converted
            .map_err(|error| io_error("could not verify Vela's private Windows ACL", error))
            .and_then(|()| {
                // SAFETY: the conversion returned a NUL-terminated string.
                unsafe { text.to_string() }
                    .map_err(|error| io_error("Windows returned an invalid ACL", error))
            });
        // SAFETY: both APIs allocate their outputs with LocalAlloc.
        unsafe {
            if !text.is_null() {
                LocalFree(Some(HLOCAL(text.as_ptr().cast::<c_void>())));
            }
            LocalFree(Some(HLOCAL(descriptor.0)));
        }
        result
    }

    pub(super) fn harden(path: &Path, directory: bool) -> io::Result<()> {
        let sid = current_user_sid()?;
        let inheritance = if directory { "OICI" } else { "" };
        let sddl = format!(
            "D:P(A;{inheritance};FA;;;{sid})(A;{inheritance};FA;;;SY)(A;{inheritance};FA;;;BA)"
        );
        let wide_sddl = sddl
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `wide_sddl` is NUL-terminated and `descriptor` points to
        // writable storage for the LocalAlloc result.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR::from_raw(wide_sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|error| io_error("could not build Vela's private Windows ACL", error))?;

        let wide_path = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: both pointers remain valid for the call. The security
        // descriptor is freed immediately afterward.
        let applied = unsafe {
            SetFileSecurityW(
                PCWSTR::from_raw(wide_path.as_ptr()),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            )
            .ok()
        };
        // SAFETY: the conversion routine allocates the descriptor with
        // LocalAlloc.
        unsafe {
            LocalFree(Some(HLOCAL(descriptor.0)));
        }
        applied
            .map_err(|error| io_error("could not protect Vela's private Windows file", error))?;

        verify(path)
    }

    pub(super) fn verify(path: &Path) -> io::Result<()> {
        let sid = current_user_sid()?;
        let actual = read_dacl_sddl(path)?;
        let current_user_ace = format!(";;;{sid})");
        let has_system = actual.contains(";;;SY)") || actual.contains(";;;S-1-5-18)");
        let has_admins = actual.contains(";;;BA)") || actual.contains(";;;S-1-5-32-544)");
        if !actual.starts_with("D:P")
            || !actual.contains(&current_user_ace)
            || !has_system
            || !has_admins
            || actual.contains(";;;WD)")
            || actual.contains(";;;S-1-1-0)")
            || actual.contains(";;;BU)")
            || actual.contains(";;;S-1-5-32-545)")
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Vela's Windows storage ACL is not private",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Fixture {
        value: u32,
    }

    fn temp_paths(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("vela-storage-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        (root.join("data.json"), root.join("data.lock"), root)
    }

    #[test]
    fn only_a_genuinely_missing_file_defaults() {
        let (data, _, root) = temp_paths("missing");
        assert_eq!(load_json::<Fixture>(&data).unwrap(), Fixture::default());

        fs::write(&data, b"{not json").unwrap();
        let before = fs::read(&data).unwrap();
        assert_eq!(
            load_json::<Fixture>(&data).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&data).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_replace_rename_moves_exact_bytes_and_preserves_both_paths_on_collision() {
        let (_, _, root) = temp_paths("rename");
        let source = root.join("source.json");
        let destination = root.join("destination.json");
        let original = b"{\"synthetic\":\"exact\"}";
        fs::write(&source, original).unwrap();

        rename_noreplace(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), original);
        assert!(is_private_regular(&destination));

        let collision_source = root.join("collision-source.json");
        let collision_destination = root.join("collision-destination.json");
        fs::write(&collision_source, b"source").unwrap();
        fs::write(&collision_destination, b"destination").unwrap();
        assert_eq!(
            rename_noreplace(&collision_source, &collision_destination)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&collision_source).unwrap(), b"source");
        assert_eq!(fs::read(&collision_destination).unwrap(), b"destination");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_not_treated_as_missing() {
        use std::os::unix::fs::symlink;

        let (data, _, root) = temp_paths("dangling");
        symlink(root.join("absent-target"), &data).unwrap();
        assert!(load_json::<Fixture>(&data).is_err());
        assert!(fs::symlink_metadata(&data)
            .unwrap()
            .file_type()
            .is_symlink());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_mutation_leaves_existing_json_byte_identical() {
        let (data, lock, root) = temp_paths("mutation");
        save_json(&data, &Fixture { value: 7 }).unwrap();
        let before = fs::read(&data).unwrap();
        let result =
            update_json::<Fixture, (), _>("fixture", &TEST_LOCK, &data, &lock, |fixture| {
                fixture.value = 9;
                Err("reject".to_string())
            });
        assert_eq!(result.unwrap_err(), "reject");
        assert_eq!(fs::read(&data).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn saved_json_and_lock_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let (data, lock, root) = temp_paths("mode");
        update_json::<Fixture, (), _>("fixture", &TEST_LOCK, &data, &lock, |fixture| {
            fixture.value = 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::metadata(&data).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&lock).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn config_directory_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("vela-storage-directory-{}", uuid::Uuid::new_v4()));
        ensure_private_directory(&root).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn saved_json_lock_and_directory_receive_a_verified_private_acl() {
        let (data, lock, root) = temp_paths("windows-acl");
        ensure_private_directory(&root).unwrap();
        update_json::<Fixture, (), _>("fixture", &TEST_LOCK, &data, &lock, |fixture| {
            fixture.value = 1;
            Ok(())
        })
        .unwrap();
        assert!(is_private_regular(&data));
        assert!(is_private_regular(&lock));
        fs::remove_dir_all(root).unwrap();
    }
}
