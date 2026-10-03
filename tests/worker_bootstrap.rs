//! Real-process bootstrap tier, separate from deterministic library tests.

use std::process::Command;

#[test]
fn malformed_worker_refuses_silently_without_echoing_supplied_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_gwz-sspi-worker"))
        .arg("synthetic-sensitive-argument-not-for-logging")
        .output()
        .expect("scaffold worker must launch");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "worker must emit no protocol token"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn runtime_metadata_cannot_activate_an_unpackaged_or_invalid_bootstrap() {
    let output = Command::new(env!("CARGO_BIN_EXE_gwz-sspi-worker"))
        .env("GWZ_SSPI_BUILD_FINGERPRINT", "42".repeat(32))
        .args(["--gwz-sspi-worker", "0", "1"])
        .output()
        .expect("worker launches");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
