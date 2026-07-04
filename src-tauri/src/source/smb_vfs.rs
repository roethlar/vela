//! Native SMB filesystem provider for the local-family pipeline
//! (Linux-family only). Paths in this provider's namespace are
//! share-relative with a leading slash (`/movies/Heat.mkv`); the share and
//! credentials come from the owning mount record, so no server or
//! credential material ever appears in a path, rating key, or cache key.
//!
//! Directory listings are the network primitive. Each listing caches its
//! children's kind/size, so the walkers' per-entry `is_dir`/`is_file`/
//! `file_len` probes are answered from memory instead of one network stat
//! per entry. A directory that has been listed is authoritative for its
//! children: a name absent from a listed directory is reported absent
//! without touching the network (artwork probes would otherwise cost seven
//! round-trips per item).

#![cfg(all(unix, not(target_os = "macos")))]

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use crate::config::SmbMount;
use crate::smb_client::{connect_mount, SmbConnection};
use crate::source::vfs::Vfs;

#[derive(Clone, Copy)]
struct EntryMeta {
    is_dir: bool,
    size: u64,
}

#[derive(Default)]
struct CacheState {
    /// Kind/size for every child seen in a listing, keyed by provider path.
    entries: HashMap<PathBuf, EntryMeta>,
    /// Directories whose listings are cached and therefore authoritative
    /// for child existence.
    listed: HashSet<PathBuf>,
}

pub struct SmbVfs {
    mount: SmbMount,
    conn: Mutex<Option<SmbConnection>>,
    cache: Mutex<CacheState>,
}

impl SmbVfs {
    pub fn new(mount: SmbMount) -> Self {
        Self {
            mount,
            conn: Mutex::new(None),
            cache: Mutex::new(CacheState::default()),
        }
    }

    /// Provider path (`/movies`) → share-relative path (`movies`).
    fn relative(p: &Path) -> Option<String> {
        let norm = normalize(p)?;
        let s = norm.to_string_lossy();
        Some(s.trim_start_matches('/').to_string())
    }

    /// Run `f` with a live connection, connecting lazily. A failed call
    /// drops the connection so the next call reconnects fresh.
    fn with_conn<T>(&self, f: impl Fn(&SmbConnection) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self
            .conn
            .lock()
            .map_err(|_| "SMB provider lock poisoned".to_string())?;
        if slot.is_none() {
            *slot = Some(connect_mount(&self.mount)?);
        }
        let conn = slot.as_ref().expect("just ensured");
        match f(conn) {
            Ok(v) => Ok(v),
            Err(e) => {
                // Connection-level failures (server rebooted, share went
                // away) must not wedge this provider forever.
                *slot = None;
                Err(e)
            }
        }
    }

    /// Meta for `p`, from cache; if its parent has been listed the cache is
    /// authoritative (absent = doesn't exist). Otherwise list the parent
    /// once and re-check. The share root is always a directory.
    fn meta(&self, p: &Path) -> Option<EntryMeta> {
        let norm = normalize(p)?;
        if norm == Path::new("/") {
            return Some(EntryMeta {
                is_dir: true,
                size: 0,
            });
        }
        {
            let cache = self.cache.lock().ok()?;
            if let Some(m) = cache.entries.get(&norm) {
                return Some(*m);
            }
            if let Some(parent) = norm.parent() {
                if cache.listed.contains(parent) {
                    return None; // parent listed, name absent → doesn't exist
                }
            }
        }
        let parent = norm.parent()?.to_path_buf();
        // Populate the parent's listing, then answer from cache.
        let _ = self.list_and_cache(&parent);
        self.cache.lock().ok()?.entries.get(&norm).copied()
    }

    fn list_and_cache(&self, dir: &Path) -> Result<Vec<PathBuf>, String> {
        let norm = normalize(dir).ok_or("path escapes the share")?;
        let rel = Self::relative(&norm).ok_or("path escapes the share")?;
        let listed = self.with_conn(|conn| conn.list_dir(&rel))?;
        let mut children: Vec<PathBuf> = Vec::with_capacity(listed.len());
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| "SMB provider lock poisoned".to_string())?;
        for entry in listed {
            let child = norm.join(&entry.name);
            cache.entries.insert(
                child.clone(),
                EntryMeta {
                    is_dir: entry.is_dir,
                    size: entry.size,
                },
            );
            children.push(child);
        }
        cache.listed.insert(norm);
        children.sort();
        Ok(children)
    }
}

/// Logical normalization only: no network, no symlink resolution (the
/// server resolves those; the client namespace is just names). `..` refuses
/// to resolve — walkers never produce it, so seeing one means a crafted key
/// and the caller must treat the path as outside every root.
fn normalize(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::from("/");
    for c in p.components() {
        match c {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::Normal(name) => out.push(name),
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    // A relative input (no leading slash) is not in this provider's
    // namespace; require explicit rooting so path spaces can't mix.
    if !p.has_root() {
        return None;
    }
    Some(out)
}

impl Vfs for SmbVfs {
    fn read_dir_sorted(&self, dir: &Path) -> Vec<PathBuf> {
        // Unreadable/missing directories list as empty, matching StdFs.
        self.list_and_cache(dir).unwrap_or_default()
    }

    fn is_dir(&self, p: &Path) -> bool {
        self.meta(p).map(|m| m.is_dir).unwrap_or(false)
    }

    fn is_file(&self, p: &Path) -> bool {
        self.meta(p).map(|m| !m.is_dir).unwrap_or(false)
    }

    fn canonicalize(&self, p: &Path) -> Option<PathBuf> {
        // Purely logical: containment comes from the share scope itself.
        normalize(p)
    }

    fn file_len(&self, p: &Path) -> u64 {
        self.meta(p).map(|m| m.size).unwrap_or(0)
    }

    fn read_to_string(&self, _p: &Path) -> Option<String> {
        // Sidecar (.nfo) reading over SMB arrives with positioned reads in
        // the stream-proxy slice; until then SMB items enrich from
        // filenames and the online cache only.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_roots_and_cleans() {
        assert_eq!(normalize(Path::new("/movies")), Some(PathBuf::from("/movies")));
        assert_eq!(
            normalize(Path::new("/movies//4k/./x.mkv")),
            Some(PathBuf::from("/movies/4k/x.mkv"))
        );
        assert_eq!(normalize(Path::new("/")), Some(PathBuf::from("/")));
    }

    #[test]
    fn normalize_rejects_escapes_and_unrooted() {
        assert_eq!(normalize(Path::new("/movies/../etc")), None);
        assert_eq!(normalize(Path::new("../x")), None);
        assert_eq!(normalize(Path::new("movies")), None, "unrooted is outside");
    }
}
