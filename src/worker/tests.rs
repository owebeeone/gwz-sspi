use super::*;
use crate::secret::Storage;
use crate::supervisor::ports::{Primary, ReadPort, WritePort};
use crate::{AuthRequest, Error, Package, SecretBytes};
struct Input {
    bytes: Storage,
    at: usize,
    chunk: usize,
}
impl ReadPort for Input {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        let n = output
            .len()
            .min(self.chunk)
            .min(self.bytes.as_slice().len() - self.at);
        output[..n].copy_from_slice(&self.bytes.as_slice()[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}
struct Sink {
    bytes: usize,
}
impl WritePort for Sink {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let n = bytes.len().min(1);
        self.bytes += n;
        Ok(n)
    }
}
struct Never;
impl native::Provider for Never {
    fn maximum(&mut self, _: Package) -> Result<u32, Error> {
        panic!("pre-Begin provider work")
    }
    fn acquire(&mut self, _: &AuthRequest) -> Result<Box<dyn native::Session>, Error> {
        panic!("pre-Begin credentials")
    }
}
fn primary() -> Primary {
    Primary {
        sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 5]),
        luid: SecretBytes::new(&[0; 8]),
        session: 0,
    }
}
#[test]
fn finish_before_begin_runs_production_worker_without_native_credentials() {
    let frame = crate::protocol::supervision::finish().unwrap();
    let mut bytes = Storage::zeroed(4 + frame.as_slice().len());
    bytes.as_mut()[..4].copy_from_slice(&(frame.as_slice().len() as u32).to_le_bytes());
    bytes.as_mut()[4..].copy_from_slice(frame.as_slice());
    let mut input = Input {
        bytes,
        at: 0,
        chunk: 1,
    };
    let mut output = Sink { bytes: 0 };
    assert!(session::run(&mut input, &mut output, &mut Never, &primary(), &[0x42; 32]).is_ok());
    assert!(output.bytes > 0);
}
#[test]
fn eof_before_begin_disposes_without_native_work() {
    let mut input = Input {
        bytes: Storage::zeroed(0),
        at: 0,
        chunk: 1,
    };
    let mut output = Sink { bytes: 0 };
    assert!(session::run(&mut input, &mut output, &mut Never, &primary(), &[0x42; 32]).is_ok());
}

include!("test_support.rs");
include!("bridge_tests.rs");
#[test]
fn aligned_utf16_and_binding_owners_wipe_full_initialized_allocations() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let probe = Probe(Some(events.clone()));
    let mut wide = storage::Wide::new("u😀域");
    wide.probe = probe.clone();
    assert_eq!(wide.count(), 4);
    let mut binding =
        storage::Binding::new(b"tls-server-end-point:01234567890123456789012345678901");
    binding.probe = probe;
    assert_eq!(binding.len(), 85);
    drop(wide);
    drop(binding);
    assert_eq!(*events.lock().unwrap(), [(10, true), (88, true)]);
}

include!("adversarial_tests.rs");
