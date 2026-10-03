//! Public early-dispatch construction is platform independent and effect free.
use gwz_sspi::{ErrorKind, WorkerBootstrap};
use std::ffi::OsString;
fn parse(args: &[&str]) -> Result<Option<WorkerBootstrap>, gwz_sspi::Error> {
    WorkerBootstrap::from_args(args.iter().map(OsString::from))
}
#[test]
fn public_bootstrap_recognizes_only_the_exact_early_internal_shape() {
    assert!(parse(&[]).unwrap().is_none());
    assert!(parse(&["clone", "repository"]).unwrap().is_none());
    for args in [
        vec!["--gwz-sspi-worker"],
        vec!["--gwz-sspi-worker", "1"],
        vec!["--gwz-sspi-worker", "0", "2"],
        vec!["--gwz-sspi-worker", "1", "1"],
        vec!["--gwz-sspi-worker", "+1", "2"],
        vec!["--gwz-sspi-worker", "1", "2", "extra"],
        vec!["--gwz-sspi-worker", "synthetic-sensitive-value", "2"],
        vec!["--gwz-sspi-worker", "1844674407370955161600000", "2"],
    ] {
        match parse(&args) {
            Err(error) => assert_eq!(error.kind(), ErrorKind::InvalidRequest),
            Ok(_) => panic!("malformed bootstrap accepted"),
        }
    }
    let bootstrap = parse(&["--gwz-sspi-worker", "1", "2"]).unwrap().unwrap();
    fn send_sync<T: Send + Sync>(_: &T) {}
    send_sync(&bootstrap);
}
#[cfg(not(windows))]
mod unsupported {
    #[test]
    fn owned_entry_refuses_without_native_fallback() {
        let bootstrap = super::parse(&["--gwz-sspi-worker", "1", "2"])
            .unwrap()
            .unwrap();
        assert_eq!(
            gwz_sspi::worker_entry(bootstrap, [0x42; 32])
                .unwrap_err()
                .kind(),
            super::ErrorKind::UnsupportedPlatform
        );
    }
}
