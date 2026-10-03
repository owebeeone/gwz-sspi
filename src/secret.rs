use crate::{Error, ErrorKind};
use zeroize::{Zeroize, Zeroizing};

#[cfg(not(test))]
pub(crate) mod audit {
    #[derive(Default, Clone)]
    pub(crate) struct Probe {}
    impl Probe {
        pub(crate) fn observe(&self, _: &[u8]) {}
        pub(crate) fn words16(&self, _: &[u16]) {}
        pub(crate) fn words32(&self, _: &[u32]) {}
    }
}

pub(crate) struct Storage {
    bytes: Zeroizing<Box<[u8]>>,
    pub(crate) probe: audit::Probe,
}
impl Storage {
    pub(crate) fn zeroed(len: usize) -> Self {
        Self {
            bytes: Zeroizing::new(vec![0; len].into_boxed_slice()),
            probe: audit::Probe::default(),
        }
    }
    pub(crate) fn copy(source: &[u8]) -> Self {
        let mut storage = Self::zeroed(source.len());
        storage.as_mut().copy_from_slice(source);
        storage
    }
    pub(crate) fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        self.bytes.zeroize();
        self.probe.observe(&self.bytes);
    }
}

/// Owned bytes wiped when dropped; no cloning or diagnostic formatting.
///
/// ```compile_fail
/// let secret = gwz_sspi::SecretBytes::new(b"synthetic");
/// let _ = secret.clone();
/// ```
/// ```compile_fail
/// let secret = gwz_sspi::SecretBytes::new(b"synthetic");
/// println!("{secret:?}");
/// ```
pub struct SecretBytes(pub(crate) Storage);
impl SecretBytes {
    /// Allocate initialized fixed storage, then copy the borrowed source.
    /// The caller still owns the source and must arrange its own wiping.
    pub fn new(source: &[u8]) -> Self {
        Self(Storage::copy(source))
    }
    /// Borrow the bytes while their owner is held.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Owned UTF-8 text wiped when dropped; no cloning or diagnostic formatting.
///
/// ```compile_fail
/// let secret = gwz_sspi::SecretText::new("synthetic").unwrap();
/// let _ = secret.clone();
/// ```
/// ```compile_fail
/// let secret = gwz_sspi::SecretText::new("synthetic").unwrap();
/// println!("{secret:?}");
/// ```
pub struct SecretText(pub(crate) Storage);
impl SecretText {
    /// Copy borrowed UTF-8 text after rejecting NUL. The caller owns and must
    /// wipe the original source; this constructor cannot wipe borrowed memory.
    pub fn new(source: &str) -> Result<Self, Error> {
        if source.as_bytes().contains(&0) {
            return Err(Error::new(ErrorKind::InvalidRequest));
        }
        Ok(Self(Storage::copy(source.as_bytes())))
    }
    pub(crate) fn admitted(source: &str) -> Self {
        Self(Storage::copy(source.as_bytes()))
    }
    /// Borrow the text while its owner is held.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.0.as_slice()).expect("owned UTF-8 invariant")
    }
}

#[cfg(test)]
pub(crate) mod audit {
    use std::sync::Arc;
    use std::sync::Mutex;
    pub(crate) type Events = Arc<Mutex<Vec<(usize, bool)>>>;
    #[derive(Default, Clone)]
    pub(crate) struct Probe(pub(crate) Option<Events>);
    impl Probe {
        pub(crate) fn words16(&self, words: &[u16]) {
            self.record(words.len() * 2, words.iter().all(|w| *w == 0));
        }
        pub(crate) fn words32(&self, words: &[u32]) {
            self.record(words.len() * 4, words.iter().all(|w| *w == 0));
        }
        fn record(&self, length: usize, zero: bool) {
            if let Some(events) = &self.0 {
                events.lock().expect("audit mutex").push((length, zero));
            }
        }
        pub(crate) fn observe(&self, bytes: &[u8]) {
            if let Some(events) = &self.0 {
                events
                    .lock()
                    .expect("audit mutex")
                    .push((bytes.len(), bytes.iter().all(|b| *b == 0)));
            }
        }
    }
}
