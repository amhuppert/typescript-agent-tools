//! Unix process ownership. Closing the file releases the lock, including after
//! a crash; the lock file must never be unlinked while another process can open it.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    fn getuid() -> std::ffi::c_uint;
}

pub(crate) fn user_id() -> u32 {
    // SAFETY: getuid has no arguments or pointer preconditions on Unix.
    unsafe { getuid() }
}

pub(super) struct Lease {
    _file: File,
}

impl Lease {
    pub(super) fn acquire(path: &Path) -> io::Result<Option<Self>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        Ok(try_lock(&file)?.then_some(Self { _file: file }))
    }
}

/// Take the exclusive lock of `file` without waiting: `false` when another
/// open file holds it. Closing `file` releases it, also when its process dies.
pub(super) fn try_lock(file: &File) -> io::Result<bool> {
    // LOCK_EX | LOCK_NB, shared by macOS, Linux, and the BSD flock API.
    // SAFETY: file owns a live descriptor; flock retains no Rust pointers.
    if unsafe { flock(file.as_raw_fd(), 2 | 4) } == 0 {
        Ok(true)
    } else {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(false)
        } else {
            Err(error)
        }
    }
}
