//! Real-process bootstrap tier, separate from deterministic library tests.

use std::process::Command;

#[test]
fn unfinished_worker_refuses_without_echoing_supplied_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_gwz-sspi-worker"))
        .arg("synthetic-sensitive-argument-not-for-logging")
        .output()
        .expect("scaffold worker must launch");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "worker must emit no protocol token"
    );
    assert_eq!(output.stderr, b"gwz-sspi worker is not implemented\n");
}
