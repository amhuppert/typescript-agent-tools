//! Opening a file without waiting for it: the one owner of this build's
//! `O_NONBLOCK` value. Opening a pipe or device can wait until something
//! appears at its other end; with `O_NONBLOCK` the open returns at once, and
//! the caller then checks the kind of the file it opened, which nothing put
//! at the path after an earlier check can change. The library, reading
//! program inputs, and the command-line binary, reading the reference
//! service's runtime files, both compile this module.

use std::io;
use std::path::Path;

/// `O_NONBLOCK` where this build knows its value; reading a regular file
/// ignores it.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
pub(crate) const NONBLOCK: Option<i32> = Some(0x0004);
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv64"
    )
))]
pub(crate) const NONBLOCK: Option<i32> = Some(0o4000);
/// Elsewhere the open can wait, and only a caller's check before opening
/// declines a pipe.
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "x86",
            target_arch = "x86_64",
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )
    )
)))]
pub(crate) const NONBLOCK: Option<i32> = None;

/// `path` opened for reading, without waiting where `NONBLOCK` is known.
#[cfg(unix)]
pub(crate) fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    if let Some(flag) = NONBLOCK {
        options.custom_flags(flag);
    }
    options.open(path)
}

#[cfg(not(unix))]
pub(crate) fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    std::fs::File::open(path)
}
