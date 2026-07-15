//! Shared durable JSON-file mechanics for Vela-owned state.
//!
//! Config and playlists deliberately use separate files, but they need the
//! same failure properties: an unreadable/corrupt file is never treated as an
//! empty one, every update is serialized across threads and processes, and a
//! reader never observes a partial write.

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
    fs::create_dir_all(dir)?;
    Ok(dir.join(name))
}

/// Read a JSON object. Only a genuinely absent path becomes `Default`; a
/// dangling symlink, permission error, or parse error fails closed.
pub fn load_json<T>(path: &Path) -> io::Result<T>
where
    T: DeserializeOwned + Default,
{
    let mut json = String::new();
    match fs::File::open(path) {
        Ok(mut file) => {
            file.read_to_string(&mut json)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::symlink_metadata(path) {
            Err(metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound => {
                return Ok(T::default());
            }
            _ => return Err(error),
        },
        Err(error) => return Err(error),
    }
    serde_json::from_str(&json).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Atomically replace one JSON file. On Unix the temporary file is owner-only
/// from its first byte; it is never written and chmodded afterward.
pub fn save_json<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    let tmp = path.with_extension(format!("{extension}.tmp.{}", std::process::id()));

    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    {
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
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(tmp, path)
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
    let _guard = process_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{label} lock unavailable: {error}"))?;
    }
    #[cfg(unix)]
    let lock_file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(lock_path)
    };
    #[cfg(not(unix))]
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path);
    let lock_file = lock_file.map_err(|error| format!("could not open {label} lock: {error}"))?;
    lock_file
        .lock()
        .map_err(|error| format!("could not acquire {label} lock: {error}"))?;

    let mut value = load_json(path).map_err(|error| format!("could not read {label}: {error}"))?;
    let output = mutate(&mut value)?;
    save_json(path, &value).map_err(|error| format!("could not save {label}: {error}"))?;
    Ok(output)
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
}
