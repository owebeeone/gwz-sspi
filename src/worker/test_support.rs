use crate::secret::audit::{Events, Probe};
use crate::{ErrorKind, Identity, Mechanism, MechanismObservation, SecretText, TokenLimit};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct Trace {
    calls: Vec<&'static str>,
    rounds: usize,
}
type Log = Arc<Mutex<Trace>>;
struct Provider {
    log: Log,
    max: u32,
    status: u32,
    observation: MechanismObservation,
    fail: Option<&'static str>,
    size: usize,
    events: Events,
    complete_round: Option<usize>,
}
impl Provider {
    fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(Trace::default())),
            max: 65536,
            status: 0x90312,
            observation: MechanismObservation::Selected {
                mechanism: Mechanism::Ntlm,
                authoritative: true,
            },
            fail: None,
            size: 9,
            events: Arc::new(Mutex::new(Vec::new())),
            complete_round: None,
        }
    }
    fn stamp(&self, name: &'static str) -> Result<(), Error> {
        self.log.lock().unwrap().calls.push(name);
        if self.fail == Some(name) {
            Err(Error::provider(Some(0x80090308)))
        } else {
            Ok(())
        }
    }
}
struct FakeSession {
    log: Log,
    status: u32,
    observation: MechanismObservation,
    fail: Option<&'static str>,
    size: usize,
    events: Events,
    cleaned: bool,
    complete_round: Option<usize>,
}
struct FakeOutput {
    bytes: Option<Storage>,
    fail: bool,
    log: Log,
}
impl native::Output for FakeOutput {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn bytes(&self) -> Result<&[u8], Error> {
        Ok(self.bytes.as_ref().unwrap().as_slice())
    }
    fn dispose(&mut self) -> Result<(), Error> {
        self.log.lock().unwrap().calls.push("free_output");
        drop(self.bytes.take());
        if self.fail {
            Err(Error::provider(Some(0x80090304)))
        } else {
            Ok(())
        }
    }
}
impl Drop for FakeOutput {
    fn drop(&mut self) {
        if self.bytes.is_some() {
            let _ = native::Output::dispose(self);
        }
    }
}
impl native::Provider for Provider {
    fn maximum(&mut self, _: Package) -> Result<u32, Error> {
        self.stamp("maximum")?;
        Ok(self.max)
    }
    fn acquire(&mut self, request: &AuthRequest) -> Result<Box<dyn native::Session>, Error> {
        self.stamp("acquire")?;
        match &request.identity {
            Identity::Explicit {
                user,
                domain,
                password,
            } => {
                assert_eq!(user.as_str(), "u😀");
                assert_eq!(domain.as_str(), "域");
                assert_eq!(password.as_str(), "p😀");
            }
            Identity::CurrentLogon => {}
        }
        Ok(Box::new(FakeSession {
            log: self.log.clone(),
            status: self.status,
            observation: self.observation,
            fail: self.fail,
            size: self.size,
            events: self.events.clone(),
            cleaned: false,
            complete_round: self.complete_round,
        }))
    }
}
impl FakeSession {
    fn stamp(&self, name: &'static str) -> Result<(), Error> {
        self.log.lock().unwrap().calls.push(name);
        if self.fail == Some(name) {
            Err(Error::provider(Some(0x80090308)))
        } else {
            Ok(())
        }
    }
}
impl native::Session for FakeSession {
    fn initialize(&mut self, input: &[u8]) -> Result<native::Step, Error> {
        assert!(!self.cleaned);
        let mut trace = self.log.lock().unwrap();
        trace.rounds += 1;
        if trace.rounds == 1 {
            assert!(input.is_empty());
        } else {
            assert_eq!(input, b"challenge");
        }
        let status = if self.complete_round == Some(trace.rounds) {
            0
        } else {
            self.status
        };
        drop(trace);
        self.stamp("initialize")?;
        let mut bytes = Storage::zeroed(self.size);
        bytes.as_mut().fill(0x53);
        bytes.probe = Probe(Some(self.events.clone()));
        Ok(native::Step {
            status,
            attributes: u32::MAX,
            output: Box::new(FakeOutput {
                bytes: Some(bytes),
                fail: self.fail == Some("free_output"),
                log: self.log.clone(),
            }),
        })
    }
    fn complete(&mut self, output: &mut dyn native::Output) -> Result<(), Error> {
        assert!(output.as_any_mut().is::<FakeOutput>());
        self.stamp("complete")
    }
    fn observation(&mut self) -> Result<MechanismObservation, Error> {
        self.stamp("observation")?;
        Ok(self.observation)
    }
    fn cleanup(&mut self) -> Result<(), Error> {
        if !self.cleaned {
            self.cleaned = true;
            self.stamp("cleanup")?;
        }
        Ok(())
    }
}
impl Drop for FakeSession {
    fn drop(&mut self) {
        let _ = native::Session::cleanup(self);
        self.log.lock().unwrap().calls.push("session_drop");
    }
}
fn request(package: Package, explicit: bool, cap: u32) -> AuthRequest {
    let mut binding = Storage::zeroed(53);
    binding.as_mut()[..21].copy_from_slice(b"tls-server-end-point:");
    binding.as_mut()[21..].fill(0x42);
    AuthRequest {
        package,
        target: SecretText::new("HTTP/example.test").unwrap(),
        identity: if explicit {
            Identity::Explicit {
                user: SecretText::new("u😀").unwrap(),
                domain: SecretText::new("域").unwrap(),
                password: SecretText::new("p😀").unwrap(),
            }
        } else {
            Identity::CurrentLogon
        },
        channel_binding: SecretBytes(binding),
        token_limit: TokenLimit::new(cap).unwrap(),
        digest: None,
    }
}
fn sequence(frames: &[Storage]) -> Input {
    let total = frames.iter().map(|f| 4 + f.as_slice().len()).sum();
    let mut bytes = Storage::zeroed(total);
    let mut at = 0;
    for frame in frames {
        let len = frame.as_slice().len();
        bytes.as_mut()[at..at + 4].copy_from_slice(&(len as u32).to_le_bytes());
        bytes.as_mut()[at + 4..at + 4 + len].copy_from_slice(frame.as_slice());
        at += 4 + len;
    }
    Input {
        bytes,
        at: 0,
        chunk: 1,
    }
}
struct Output {
    bytes: Storage,
    at: usize,
    chunk: usize,
    fail_at: Option<usize>,
}
impl Output {
    fn new() -> Self {
        Self {
            bytes: Storage::zeroed(200000),
            at: 0,
            chunk: 7,
            fail_at: None,
        }
    }
}
impl WritePort for Output {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        if self.fail_at == Some(self.at) {
            return Err(Error::new(ErrorKind::Protocol));
        }
        let n = bytes.len().min(self.chunk).min(
            self.fail_at
                .map_or(usize::MAX, |fail| fail.saturating_sub(self.at)),
        );
        if n == 0 {
            return Ok(0);
        }
        self.bytes.as_mut()[self.at..self.at + n].copy_from_slice(&bytes[..n]);
        self.at += n;
        Ok(n)
    }
}
fn replies(
    output: &Output,
    package: Package,
    cap: u32,
) -> Vec<crate::protocol::supervision::Reply> {
    use crate::supervisor::kernel::Expected;
    let mut at = 0;
    let mut result = Vec::new();
    let mut expected = Expected::Bootstrap;
    while at < output.at {
        let len =
            u32::from_le_bytes(output.bytes.as_slice()[at..at + 4].try_into().unwrap()) as usize;
        let frame = Storage::copy(&output.bytes.as_slice()[at + 4..at + 4 + len]);
        if crate::protocol::worker_codec::inspection::is_finish_reply(&frame) {
            expected = Expected::Finish;
        }
        let reply = crate::protocol::supervision::reply(
            &frame,
            expected,
            &primary(),
            &[0x42; 32],
            package,
            TokenLimit::new(cap).unwrap(),
        )
        .unwrap();
        expected = match &reply {
            crate::protocol::supervision::Reply::Hello => Expected::Begin,
            crate::protocol::supervision::Reply::Token { round, .. } => {
                Expected::Challenge(round + 1)
            }
            _ => Expected::Finish,
        };
        // Finish is legal between rounds; inspect fixed message kind in private
        // test projection to choose phase without shadowing any wire fields.
        result.push(reply);
        at += 4 + len;
    }
    result
}
