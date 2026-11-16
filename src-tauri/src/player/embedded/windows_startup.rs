//! Windows settings-file handles; the decision and recovery marker are shared.

use super::bounded_text;

pub(super) fn regular_file_at(path: &std::path::Path) -> Option<std::fs::File> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            GetFileType, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_TYPE_DISK,
        },
    };
    // OPEN_REPARSE_POINT opens a link itself, never its target. The known
    // config path is a disk path; a FIFO cannot be installed there on Windows.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0 | FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)
        .ok()?;
    // SAFETY: a live file handle. Devices and named pipes are never read.
    if unsafe { GetFileType(HANDLE(held.as_raw_handle())) } != FILE_TYPE_DISK
        || !held.metadata().ok()?.file_type().is_file()
    {
        return None;
    }
    Some(held)
}

pub(super) fn text_of(held: &std::fs::File) -> Option<String> {
    bounded_text(held)
}
