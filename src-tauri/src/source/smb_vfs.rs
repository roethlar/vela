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

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::config::SmbMount;
use crate::smb_client::{connect_mount, SmbConnection};
use crate::source::vfs::Vfs;

/// Custom webview scheme serving SMB sidecar artwork. URLs are STABLE
/// across app restarts (mount id + base64url of the provider path), so
/// they are safe to persist in the listing cache and recents — unlike
/// loopback proxy tokens, whose port and token die with the process.
pub const ARTWORK_SCHEME: &str = "velasmb";

/// Bytes cap for a served artwork file.
const ARTWORK_CAP: u64 = 10 * 1024 * 1024;

/// Extensions the artwork endpoint will serve, with their MIME types.
/// A whitelist keeps the webview scheme an artwork channel, not a
/// general remote-file reader.
const ARTWORK_TYPES: &[(&str, &str)] = &[
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

/// Stable artwork URL for a provider path on a mount.
pub fn artwork_url(mount_id: &str, provider_path: &Path) -> Option<String> {
    let norm = normalize(provider_path)?;
    let encoded = URL_SAFE_NO_PAD.encode(norm.to_string_lossy().as_bytes());
    Some(format!("{ARTWORK_SCHEME}://{mount_id}/{encoded}"))
}

/// Parse `velasmb://<mount-id>/<b64url-path>` back into (mount id,
/// normalized provider path), refusing anything that doesn't decode to a
/// whitelisted image inside the share namespace.
pub fn parse_artwork_url(host: &str, path: &str) -> Option<(String, PathBuf)> {
    if host.is_empty() {
        return None;
    }
    let encoded = path.trim_matches('/');
    if encoded.is_empty() || encoded.contains('/') {
        return None; // exactly one opaque segment
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let norm = normalize(Path::new(&decoded))?;
    artwork_mime(&norm)?;
    Some((host.to_string(), norm))
}

/// MIME type for a whitelisted artwork path; `None` = refuse to serve.
pub fn artwork_mime(p: &Path) -> Option<&'static str> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    ARTWORK_TYPES
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, mime)| *mime)
}

/// Serve one artwork request: look the mount up in config, read the file
/// over a fresh native connection, bounded. Returns (bytes, mime) or an
/// HTTP status code. Blocking; run off async workers.
pub fn serve_artwork(host: &str, path: &str) -> Result<(Vec<u8>, &'static str), u16> {
    let (mount_id, norm) = parse_artwork_url(host, path).ok_or(404u16)?;
    let mime = artwork_mime(&norm).ok_or(404u16)?;
    let cfg = crate::config::load_config().map_err(|_| 500u16)?;
    let mount = cfg
        .smb_mounts
        .into_iter()
        .find(|m| m.id == mount_id)
        .ok_or(404u16)?;
    let rel = SmbVfs::relative(&norm).ok_or(404u16)?;
    let conn = connect_mount(&mount).map_err(|_| 502u16)?;
    let bytes = conn.read_small(&rel, ARTWORK_CAP).map_err(|_| 404u16)?;
    Ok((bytes, mime))
}

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
    pub(crate) fn relative(p: &Path) -> Option<String> {
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

    fn read_to_string(&self, p: &Path) -> Option<String> {
        // Sidecars only (.nfo): bounded so a mislabeled path can't balloon
        // memory; absent/unreadable is a normal miss.
        const SIDECAR_CAP: u64 = 1024 * 1024;
        let rel = Self::relative(p)?;
        let bytes = self
            .with_conn(|conn| conn.read_small(&rel, SIDECAR_CAP))
            .ok()?;
        Some(String::from_utf8_lossy(&bytes).to_string())
    }

    fn resolve_stream_url(&self, p: &Path) -> Option<Result<String, String>> {
        let Some(rel) = Self::relative(p) else {
            return Some(Err("path escapes the share".into()));
        };
        Some(crate::stream_proxy::register_smb(&self.mount, &rel))
    }

    fn artwork_ref(&self, p: &Path) -> Option<String> {
        // Stable scheme URL; a path that fails to normalize yields no
        // artwork at all — never a raw provider path.
        artwork_url(&self.mount.id, p)
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
    fn artwork_urls_roundtrip_and_gate_extensions() {
        let url = artwork_url("m-1", Path::new("/movies/My Film (2021)/poster.jpg")).unwrap();
        assert!(url.starts_with("velasmb://m-1/"), "{url}");
        let (host, b64) = url
            .strip_prefix("velasmb://")
            .unwrap()
            .split_once('/')
            .unwrap();
        let (mount, path) = parse_artwork_url(host, b64).expect("roundtrip parses");
        assert_eq!(mount, "m-1");
        assert_eq!(path, PathBuf::from("/movies/My Film (2021)/poster.jpg"));

        // Non-image and escaping payloads are refused outright.
        let mkv = URL_SAFE_NO_PAD.encode("/movies/film.mkv");
        assert!(parse_artwork_url("m-1", &mkv).is_none(), "extension whitelist");
        let escape = URL_SAFE_NO_PAD.encode("/../etc/passwd.png");
        assert!(parse_artwork_url("m-1", &escape).is_none(), "no escapes");
        assert!(parse_artwork_url("", "abc").is_none(), "mount id required");
        assert!(parse_artwork_url("m-1", "not!base64").is_none());
        assert_eq!(artwork_mime(Path::new("/a/p.JPG")), Some("image/jpeg"));
        assert_eq!(artwork_mime(Path::new("/a/p.nfo")), None);
    }

    #[test]
    fn normalize_rejects_escapes_and_unrooted() {
        assert_eq!(normalize(Path::new("/movies/../etc")), None);
        assert_eq!(normalize(Path::new("../x")), None);
        assert_eq!(normalize(Path::new("movies")), None, "unrooted is outside");
    }
}
