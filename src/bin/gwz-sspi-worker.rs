//! Minimal early dispatch; fingerprint is trusted compile-time packaging data.
fn fingerprint(text: Option<&str>) -> Option<[u8; 32]> {
    let text = text?;
    if text.len() != 64 {
        return None;
    }
    let mut result = [0; 32];
    for (output, pair) in result.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        fn digit(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            }
        }
        *output = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(result)
}
fn main() {
    // No runtime environment lookup or hashing and no default fingerprint.
    // Missing/malformed metadata refuses before adopting inherited handles.
    let result = (|| {
        let build = fingerprint(option_env!("GWZ_SSPI_BUILD_FINGERPRINT")).ok_or(())?;
        let bootstrap = gwz_sspi::WorkerBootstrap::from_args(std::env::args_os().skip(1))
            .map_err(|_| ())?
            .ok_or(())?;
        gwz_sspi::worker_entry(bootstrap, build).map_err(|_| ())
    })();
    std::process::exit(if result.is_ok() { 0 } else { 2 });
}
#[cfg(test)]
mod tests {
    use super::fingerprint;
    #[test]
    fn packaging_metadata_is_exact_without_default_or_runtime_lookup() {
        assert!(fingerprint(None).is_none());
        assert!(fingerprint(Some("")).is_none());
        assert!(fingerprint(Some(&"g".repeat(64))).is_none());
        assert!(fingerprint(Some(&"0".repeat(63))).is_none());
        assert!(fingerprint(Some(&"0".repeat(65))).is_none());
        assert_eq!(fingerprint(Some(&"42".repeat(32))), Some([0x42; 32]));
    }
}
