//! Windows half of playback's existing IPC stream. Overlapped pipe I/O lets
//! the embedded controls and reporter use the same deadlines as UnixStream.
//! Trackers that set no timeout retain their blocking byte-stream behavior.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::{
    fs::OpenOptionsExt,
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, ERROR_IO_PENDING, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::{ReadFile, WriteFile, FILE_FLAG_OVERLAPPED},
    System::{
        Pipes::GetNamedPipeServerProcessId,
        Threading::{CreateEventW, WaitForSingleObject, INFINITE},
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
    },
};

struct Timeouts {
    read: AtomicU32,
    write: AtomicU32,
}

pub(crate) struct IpcStream {
    file: File,
    timeouts: Arc<Timeouts>,
}

fn io_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error(error.code().0 & 0xffff)
}

fn milliseconds(timeout: Option<Duration>) -> io::Result<u32> {
    match timeout {
        Some(value) if value.is_zero() => Err(io::ErrorKind::InvalidInput.into()),
        Some(value) => Ok(value
            .as_nanos()
            .div_ceil(1_000_000)
            .min((INFINITE - 1) as u128) as u32),
        None => Ok(INFINITE),
    }
}

impl IpcStream {
    pub(crate) fn connect(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_OVERLAPPED.0)
            .open(path)?;
        Ok(Self {
            file,
            timeouts: Arc::new(Timeouts {
                read: AtomicU32::new(INFINITE),
                write: AtomicU32::new(INFINITE),
            }),
        })
    }

    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            timeouts: self.timeouts.clone(),
        })
    }

    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.timeouts
            .read
            .store(milliseconds(timeout)?, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.timeouts
            .write
            .store(milliseconds(timeout)?, Ordering::Relaxed);
        Ok(())
    }

    /// An embedded pipe must belong to this Vela process. Pipe names have no
    /// Unix directory permissions; never send a command to a pre-created peer.
    pub(crate) fn verify_embedded_peer(&self) -> io::Result<()> {
        let mut pid = 0;
        // SAFETY: a live pipe and writable process-id storage.
        unsafe { GetNamedPipeServerProcessId(self.handle(), &mut pid) }.map_err(io_error)?;
        if pid != std::process::id() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.file.as_raw_handle())
    }

    /// Each operation owns its event and OVERLAPPED until Windows has
    /// finished with the buffer, including cancellation. A clone may read
    /// while the original writes without sharing an OVERLAPPED or event.
    fn transfer(
        &self,
        timeout: u32,
        begin: impl FnOnce(*mut OVERLAPPED) -> windows::core::Result<()>,
    ) -> io::Result<usize> {
        // SAFETY: unnamed manual-reset event, owned until this operation ends.
        let event = unsafe { CreateEventW(None, true, false, None) }.map_err(io_error)?;
        let event = unsafe { OwnedHandle::from_raw_handle(event.0) };
        let mut operation = OVERLAPPED {
            hEvent: HANDLE(event.as_raw_handle()),
            ..Default::default()
        };
        if let Err(error) = begin(&mut operation) {
            if error.code() != ERROR_IO_PENDING.to_hresult() {
                return Err(io_error(error));
            }
        }
        // SAFETY: event and operation stay live through completion/cancellation.
        let waited = unsafe { WaitForSingleObject(operation.hEvent, timeout) };
        let mut count = 0;
        if waited != WAIT_OBJECT_0 {
            let error = if waited == WAIT_TIMEOUT {
                io::ErrorKind::TimedOut.into()
            } else {
                io::Error::last_os_error()
            };
            // Cancellation is specific to this operation. Reap it before its
            // stack storage or borrowed buffer goes away; ERROR_NOT_FOUND
            // simply means it completed concurrently with cancellation.
            unsafe {
                let _ = CancelIoEx(self.handle(), Some(&operation));
                let _ = GetOverlappedResult(self.handle(), &operation, &mut count, true);
            }
            // A completion that raced the deadline must not lose bytes from
            // a partial JSON line, or make write_all repeat a completed write.
            if count > 0 {
                return Ok(count as usize);
            }
            return Err(error);
        }
        unsafe { GetOverlappedResult(self.handle(), &operation, &mut count, false) }
            .map_err(io_error)?;
        Ok(count as usize)
    }
}

impl Read for &IpcStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let result = self.transfer(self.timeouts.read.load(Ordering::Relaxed), |operation| {
            // SAFETY: borrowed buffer lives until transfer has reaped the I/O.
            unsafe { ReadFile(self.handle(), Some(buffer), None, Some(operation)) }
        });
        match result {
            Err(error) if error.raw_os_error() == Some(ERROR_BROKEN_PIPE.0 as i32) => Ok(0),
            result => result,
        }
    }
}

impl Write for &IpcStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.transfer(self.timeouts.write.load(Ordering::Relaxed), |operation| {
            // SAFETY: borrowed buffer lives until transfer has reaped the I/O.
            unsafe { WriteFile(self.handle(), Some(buffer), None, Some(operation)) }
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, mut buffer: &[u8]) -> io::Result<()> {
        let timeout = self.timeouts.write.load(Ordering::Relaxed);
        let deadline =
            (timeout != INFINITE).then(|| Instant::now() + Duration::from_millis(timeout as u64));
        while !buffer.is_empty() {
            let left = match deadline {
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    milliseconds(Some(left))?
                }
                None => INFINITE,
            };
            let count = self.transfer(left, |operation| {
                // SAFETY: buffer stays borrowed until this operation completes.
                unsafe { WriteFile(self.handle(), Some(buffer), None, Some(operation)) }
            })?;
            if count == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            buffer = &buffer[count..];
        }
        Ok(())
    }
}

impl Read for IpcStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buffer)
    }
}

impl Write for IpcStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&*self).write(buffer)
    }
    fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        (&*self).write_all(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
