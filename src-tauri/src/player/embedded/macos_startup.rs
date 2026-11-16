//! The pre-window settings read on macOS. Never reads a link or special file.

use super::bounded_text;

pub(super) fn regular_file_at(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let before = std::fs::symlink_metadata(path).ok()?;
    if !before.file_type().is_file() {
        return None;
    }
    // macOS has no O_PATH or /proc/self/fd reopening. NOFOLLOW rejects a
    // substituted link; NONBLOCK/NOCTTY prevent a substituted FIFO/terminal
    // from hanging startup or becoming the controlling terminal. Verify the
    // opened identity before reading, and bound the read as on Linux.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .ok()?;
    let after = held.metadata().ok()?;
    (after.file_type().is_file() && before.dev() == after.dev() && before.ino() == after.ino())
        .then_some(held)
}

pub(super) fn text_of(held: &std::fs::File) -> Option<String> {
    bounded_text(held)
}
