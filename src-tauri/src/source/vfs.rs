//! Filesystem provider for the local-family sources. `LocalSource` walks
//! directories, checks containment, and reads sidecars only through this
//! trait, so the same listing/cache/metadata pipeline can serve plain
//! folders (std::fs) and, later, native SMB shares. Implementations must be
//! cheap to call from the blocking pool; they may block.

use std::path::{Path, PathBuf};

pub trait Vfs: Send + Sync {
    /// One level of a directory, sorted by name. Missing or unreadable
    /// directories yield an empty list (matching the historical local-source
    /// behavior of skipping unreadable levels rather than erroring).
    fn read_dir_sorted(&self, dir: &Path) -> Vec<PathBuf>;

    fn is_dir(&self, p: &Path) -> bool;

    fn is_file(&self, p: &Path) -> bool;

    /// Canonical form of a path for containment checks (symlink-escape
    /// protection on real filesystems). `None` when it cannot resolve; the
    /// caller must treat that as "outside".
    fn canonicalize(&self, p: &Path) -> Option<PathBuf>;

    /// File size in bytes; 0 when unknown.
    fn file_len(&self, p: &Path) -> u64;

    /// Read a small sidecar text file (e.g. `.nfo`). `None` if absent or
    /// unreadable.
    fn read_to_string(&self, p: &Path) -> Option<String>;
}

/// The real filesystem: exactly the std::fs calls the local source has
/// always made.
pub struct StdFs;

impl Vfs for StdFs {
    fn read_dir_sorted(&self, dir: &Path) -> Vec<PathBuf> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        entries
    }

    fn is_dir(&self, p: &Path) -> bool {
        p.is_dir()
    }

    fn is_file(&self, p: &Path) -> bool {
        p.is_file()
    }

    fn canonicalize(&self, p: &Path) -> Option<PathBuf> {
        std::fs::canonicalize(p).ok()
    }

    fn file_len(&self, p: &Path) -> u64 {
        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    }

    fn read_to_string(&self, p: &Path) -> Option<String> {
        std::fs::read_to_string(p).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_fs_reads_sorted_and_tolerates_missing_dirs() {
        // Scratch space on the crate's own filesystem, NOT temp_dir(): tmpfs
        // on some kernels returns readdir entries already name-sorted, which
        // silently neuters this test as a guard for the explicit sort. The
        // checkout's filesystem (ext4/btrfs/xfs) uses creation/hash order,
        // so files created in reverse-sorted order below arrive unsorted.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("vela-vfs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let names: Vec<String> = (0..24).rev().map(|i| format!("f{i:02}.txt")).collect();
        for n in &names {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let listed = StdFs.read_dir_sorted(&dir);
        let mut expect: Vec<_> = names.iter().map(|n| dir.join(n)).collect();
        expect.sort();
        assert_eq!(listed, expect, "sorted by name regardless of creation order");
        assert!(StdFs.is_file(&dir.join("f00.txt")));
        assert!(StdFs.is_dir(&dir));
        assert_eq!(StdFs.file_len(&dir.join("f00.txt")), 1);
        assert_eq!(StdFs.read_to_string(&dir.join("f00.txt")).as_deref(), Some("x"));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            StdFs.read_dir_sorted(&dir).is_empty(),
            "missing dir lists as empty, not an error"
        );
        assert!(StdFs.canonicalize(&dir).is_none());
    }
}
