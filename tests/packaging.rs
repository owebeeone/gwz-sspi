use gwz_sspi::{
    ErrorKind,
    packaging::{build_fingerprint, installed_worker},
};
#[test]
fn compiled_metadata_is_required_exact_and_installed_selection_is_absolute() {
    assert!(matches!(build_fingerprint(None), Err(e) if e.kind()==ErrorKind::WorkerUnavailable));
    for bad in ["", "42", &"g".repeat(64), &"0".repeat(65)] {
        assert!(
            matches!(build_fingerprint(Some(bad)),Err(e) if e.kind()==ErrorKind::WorkerMismatch)
        );
    }
    assert_eq!(
        build_fingerprint(Some(&"42".repeat(32))).unwrap(),
        [0x42; 32]
    );
    assert!(
        matches!(installed_worker(std::path::Path::new("relative/module.pyd"),Some(&"42".repeat(32))),Err(e) if e.kind()==ErrorKind::InvalidRequest)
    );
    let path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("missing-host-module.pyd");
    assert!(
        matches!(installed_worker(&path,Some(&"42".repeat(32))),Err(e) if e.kind()==ErrorKind::WorkerUnavailable)
    );
}
