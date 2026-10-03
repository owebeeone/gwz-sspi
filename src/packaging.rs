//! Trusted installed-host metadata handoff. No runtime hashing or path search.
use crate::{Error, ErrorKind, WorkerExecutable};
use std::path::Path;
/// Decode exactly the compile-time artifact-set identifier. Absence means the
/// host is unprovisioned; malformed metadata is a mismatch. Runtime environment
/// values and sidecar receipts are never inputs to this function.
pub fn build_fingerprint(text: Option<&str>) -> Result<[u8; 32], Error> {
    let text = text.ok_or_else(|| Error::new(ErrorKind::WorkerUnavailable))?;
    if text.len() != 64 {
        return Err(Error::new(ErrorKind::WorkerMismatch));
    }
    let mut bytes = [0; 32];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        fn digit(b: u8) -> Option<u8> {
            match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            }
        }
        *byte = digit(pair[0])
            .zip(digit(pair[1]))
            .map(|(a, b)| (a << 4) | b)
            .ok_or_else(|| Error::new(ErrorKind::WorkerMismatch))?;
    }
    Ok(bytes)
}
/// Select only the worker adjacent to the actual loaded extension image. The
/// host obtains `image` through its OS loader, never mutable Python attributes.
/// Reject relative paths, absent workers and symlinks. Existence is not build
/// verification: Supervisor verifies the compiled worker Hello before Begin.
/// This function captures a descriptor only; it creates no supervisor/process.
pub fn installed_worker(image: &Path, compiled: Option<&str>) -> Result<WorkerExecutable, Error> {
    if !image.is_absolute() {
        return Err(Error::new(ErrorKind::InvalidRequest));
    }
    let fingerprint = build_fingerprint(compiled)?;
    let name = if cfg!(windows) {
        "gwz-sspi-worker.exe"
    } else {
        "gwz-sspi-worker"
    };
    let path = image
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidRequest))?
        .join(name);
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|_| Error::new(ErrorKind::WorkerUnavailable))?;
    if !metadata.is_file() {
        return Err(Error::new(ErrorKind::WorkerUnavailable));
    }
    WorkerExecutable::new(path, fingerprint)
}
