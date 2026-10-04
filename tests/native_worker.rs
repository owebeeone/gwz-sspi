//! Opt-in public Windows process/provider fixtures. Default fast runs ignore them.
#[cfg(windows)]
mod windows {
    include!("native/windows_worker.rs");
    mod completion {
        include!("native/completion.rs");
    }
}
