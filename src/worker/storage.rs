//! Aligned initialized fixed UTF-16 and CBT storage, retained across all SSPI use.
use crate::secret::audit;
use zeroize::{Zeroize, Zeroizing};
pub(super) struct Wide {
    words: Zeroizing<Box<[u16]>>,
    pub(super) probe: audit::Probe,
}
impl Wide {
    pub(super) fn new(text: &str) -> Self {
        let len = text.encode_utf16().count();
        let mut words = Zeroizing::new(vec![0u16; len + 1].into_boxed_slice());
        for (to, from) in words.iter_mut().zip(text.encode_utf16()) {
            *to = from;
        }
        Self {
            words,
            probe: audit::Probe::default(),
        }
    }
    pub(super) fn pointer(&self) -> *const u16 {
        self.words.as_ptr()
    }
    pub(super) fn count(&self) -> u32 {
        (self.words.len() - 1) as u32
    }
}
impl Drop for Wide {
    fn drop(&mut self) {
        self.words.zeroize();
        self.probe.words16(&self.words);
    }
}
pub(super) struct Binding {
    words: Zeroizing<Box<[u32]>>,
    len: usize,
    pub(super) probe: audit::Probe,
}
impl Binding {
    pub(super) fn new(bytes: &[u8]) -> Self {
        let len = 32 + bytes.len();
        let mut result = Self {
            words: Zeroizing::new(vec![0u32; len.div_ceil(4)].into_boxed_slice()),
            len,
            probe: audit::Probe::default(),
        };
        // SEC_CHANNEL_BINDINGS: six zero initiator/acceptor fields, then
        // application data length and byte offset. Wire profile checks bytes.
        result.words[6] = bytes.len() as u32;
        result.words[7] = 32;
        for (index, byte) in bytes.iter().enumerate() {
            result.words[8 + index / 4] |= u32::from(*byte) << ((index % 4) * 8);
        }
        result
    }
    pub(super) fn pointer(&mut self) -> *mut std::ffi::c_void {
        self.words.as_mut_ptr().cast()
    }
    pub(super) fn len(&self) -> u32 {
        self.len as u32
    }
}
impl Drop for Binding {
    fn drop(&mut self) {
        self.words.zeroize();
        self.probe.words32(&self.words);
    }
}
