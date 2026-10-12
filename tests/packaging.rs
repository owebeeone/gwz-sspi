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

// WH3 installed paths (GwzTransportWindowsParityPlan.md, step 4.10): the worker is the file named beside the loaded
// image, wherever the install is, and nothing else is a worker.
mod installed_paths {
    use super::*;
    use std::path::{Path, PathBuf};

    const WORKER: &str = if cfg!(windows) {
        "gwz-sspi-worker.exe"
    } else {
        "gwz-sspi-worker"
    };

    /// A new directory under the system temporary directory, removed when dropped.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "gwz-sspi-packaging-{}-{}-{label}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fingerprint() -> String {
        "42".repeat(32)
    }

    /// An install directory under `scratch`, with the image and (if `worker`) the worker beside it.
    fn install(scratch: &Scratch, directory: &[&str], worker: bool) -> (PathBuf, PathBuf) {
        let mut dir = scratch.0.clone();
        for part in directory {
            dir.push(part);
        }
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("_gwz_core.pyd");
        std::fs::write(&image, b"image").unwrap();
        let worker_path = dir.join(WORKER);
        if worker {
            std::fs::write(&worker_path, b"worker").unwrap();
        }
        (image, worker_path)
    }

    #[test]
    fn the_worker_is_found_at_its_packaged_path_whatever_the_directory_is_called() {
        for directory in [
            vec!["plain", "gwz"],
            vec!["with spaces", "site packages", "gwz"],
            vec![
                "\u{fc}ml\u{e4}ut \u{65e5}\u{672c}\u{8a9e}",
                "\u{43f}\u{430}\u{43f}\u{43a}\u{430}",
                "gwz",
            ],
            vec!["trailing-dot.", "a b", "gwz"],
        ] {
            let scratch = Scratch::new("found");
            let (image, expected) = install(&scratch, &directory, true);
            let descriptor = installed_worker(&image, Some(&fingerprint()))
                .unwrap_or_else(|error| panic!("{directory:?}: {:?}", error.kind()));
            assert_eq!(descriptor.path(), expected.as_path(), "{directory:?}");
        }
    }

    #[test]
    fn a_worker_elsewhere_is_never_found_there_is_no_path_discovery() {
        let scratch = Scratch::new("elsewhere");
        // The only worker is in another directory, as one on `PATH` would be.
        let (_, _) = install(&scratch, &["on-path"], true);
        let (image, _) = install(&scratch, &["site-packages", "gwz"], false);
        assert!(
            matches!(installed_worker(&image, Some(&fingerprint())), Err(e) if e.kind() == ErrorKind::WorkerUnavailable)
        );
        // Nor does the process's working directory stand in for the image's.
        let here = std::env::current_dir().unwrap();
        assert!(!here.join(WORKER).exists());
    }

    #[test]
    fn a_directory_named_as_the_worker_is_not_a_worker() {
        let scratch = Scratch::new("directory");
        let (image, worker) = install(&scratch, &["with spaces", "gwz"], false);
        std::fs::create_dir(&worker).unwrap();
        assert!(
            matches!(installed_worker(&image, Some(&fingerprint())), Err(e) if e.kind() == ErrorKind::WorkerUnavailable)
        );
    }

    #[test]
    fn the_build_fingerprint_is_judged_before_the_filesystem_is_touched() {
        let scratch = Scratch::new("fingerprint");
        let (image, _) = install(&scratch, &["gwz"], true);
        // A worker is there: a missing or malformed fingerprint is still the refusal, and the two are told apart.
        assert!(
            matches!(installed_worker(&image, None), Err(e) if e.kind() == ErrorKind::WorkerUnavailable)
        );
        for bad in ["", "42", &"4g".repeat(32), &"42".repeat(33)] {
            assert!(
                matches!(installed_worker(&image, Some(bad)), Err(e) if e.kind() == ErrorKind::WorkerMismatch),
                "{bad:?}"
            );
        }
        // Either case of hex digit is the same build.
        let upper = installed_worker(&image, Some(&"AB".repeat(32))).unwrap();
        let lower = installed_worker(&image, Some(&"ab".repeat(32))).unwrap();
        assert_eq!(upper.path(), lower.path());
    }

    #[test]
    fn an_image_path_that_is_not_absolute_names_no_worker() {
        for relative in ["gwz/_gwz_core.pyd", "./_gwz_core.pyd", "_gwz_core.pyd", ""] {
            assert!(
                matches!(installed_worker(Path::new(relative), Some(&fingerprint())), Err(e) if e.kind() == ErrorKind::InvalidRequest),
                "{relative:?}"
            );
        }
    }

    // A symlink named as the worker is refused. Making one needs no privilege on a Unix platform only, so the row is
    // the platform module below; Windows install trees hold no symlinks (a wheel's RECORD lists regular files).
    #[cfg(unix)]
    mod symlink {
        use super::*;

        #[test]
        fn a_symlink_named_as_the_worker_is_not_a_worker() {
            let scratch = Scratch::new("symlink");
            let (image, worker) = install(&scratch, &["with spaces", "gwz"], false);
            let real = worker.with_file_name("real-worker");
            std::fs::write(&real, b"worker").unwrap();
            std::os::unix::fs::symlink(&real, &worker).unwrap();
            assert!(
                matches!(installed_worker(&image, Some(&fingerprint())), Err(e) if e.kind() == ErrorKind::WorkerUnavailable)
            );
        }
    }
}
