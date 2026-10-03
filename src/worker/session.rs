//! Serial phase and ownership bridge, independent of native implementation.
use super::{
    framing,
    native::{Provider, Session},
};
use crate::protocol::worker_codec::{self as codec, Command, Phase};
use crate::supervisor::ports::{Primary, ReadPort, WritePort};
use crate::{AuthRequest, Error, Package, SecretBytes, TokenStatus, TokenStep};
struct Active {
    native: Box<dyn Session>,
    request: AuthRequest,
    maximum: u32,
    probe: crate::secret::audit::Probe,
}
impl Active {
    fn token(&mut self, input: &[u8], round: u8) -> Result<(crate::secret::Storage, bool), Error> {
        let mut raw = self.native.initialize(input)?;
        let status = match raw.status {
            0 => TokenStatus::Complete,
            0x90312 => TokenStatus::Continue,
            0x90313 => {
                self.native.complete(&mut *raw.output)?;
                TokenStatus::Complete
            }
            0x90314 => {
                self.native.complete(&mut *raw.output)?;
                TokenStatus::Continue
            }
            value => return Err(Error::provider(Some(value))),
        };
        let observation = self.native.observation()?;
        let allowed = match observation {
            crate::MechanismObservation::Unresolved => {
                self.request.package == Package::Negotiate && status == TokenStatus::Continue
            }
            crate::MechanismObservation::Selected {
                mechanism,
                authoritative,
            } => {
                let matches = match self.request.package {
                    Package::Negotiate => matches!(
                        mechanism,
                        crate::Mechanism::Kerberos | crate::Mechanism::Ntlm
                    ),
                    Package::Ntlm => mechanism == crate::Mechanism::Ntlm,
                    Package::Digest => mechanism == crate::Mechanism::Digest,
                };
                matches && (status != TokenStatus::Complete || authoritative)
            }
        };
        if !allowed {
            return Err(Error::provider(None));
        }
        let bytes = raw.output.bytes()?;
        if bytes.len() > self.request.token_limit.raw_bytes().min(self.maximum) as usize
            || (round == 8 && status == TokenStatus::Continue)
        {
            return Err(Error::provider(None));
        }
        let mut step = TokenStep {
            status,
            attributes: raw.attributes,
            observation,
            payload: SecretBytes::new(bytes),
        };
        step.payload.0.probe = self.probe.clone();
        // Free must succeed before any token can be written to the parent.
        raw.output.dispose()?;
        let mut frame = codec::token(round, &step, &self.request, self.maximum)?;
        frame.probe = self.probe.clone();
        Ok((frame, status == TokenStatus::Complete))
    }
    fn cleanup(&mut self) -> Result<(), Error> {
        self.native.cleanup()
    }
}
pub(super) fn run(
    input: &mut dyn ReadPort,
    output: &mut dyn WritePort,
    provider: &mut dyn Provider,
    primary: &Primary,
    build: &[u8; 32],
) -> Result<(), Error> {
    run_audited(
        input,
        output,
        provider,
        primary,
        build,
        &crate::secret::audit::Probe::default(),
    )
}
fn write(
    output: &mut dyn WritePort,
    mut frame: crate::secret::Storage,
    probe: &crate::secret::audit::Probe,
) -> Result<(), Error> {
    frame.probe = probe.clone();
    framing::write(output, &frame)
}
pub(super) fn run_audited(
    input: &mut dyn ReadPort,
    output: &mut dyn WritePort,
    provider: &mut dyn Provider,
    primary: &Primary,
    build: &[u8; 32],
    probe: &crate::secret::audit::Probe,
) -> Result<(), Error> {
    write(output, codec::hello(primary, build)?, probe)?;
    let mut phase = Phase::Begin;
    let mut active: Option<Active> = None;
    let result = (|| {
        loop {
            let Some(frame) = framing::read(input, probe)? else {
                return Ok(());
            };
            let command = codec::command(
                &frame,
                phase,
                active.as_ref().map(|a| (&a.request, a.maximum)),
                |p| provider.maximum(p),
            )?;
            drop(frame);
            match command {
                Command::Finish => {
                    phase = Phase::Finish;
                    if let Some(a) = active.as_mut() {
                        a.cleanup()?;
                    }
                    drop(active.take());
                    write(output, codec::finished()?, probe)?;
                    return Ok(());
                }
                Command::Begin(mut request, maximum) => {
                    audit_request(&mut request, probe);
                    if request.package == Package::Digest {
                        return Err(Error::provider(None));
                    }
                    let native = provider.acquire(&request)?;
                    active = Some(Active {
                        native,
                        request,
                        maximum,
                        probe: probe.clone(),
                    });
                }
                Command::Challenge(mut payload) => {
                    payload.0.probe = probe.clone();
                    let round = match phase {
                        Phase::Challenge(r) => r,
                        _ => unreachable!(),
                    };
                    let a = active.as_mut().unwrap();
                    let (token, complete) = a.token(payload.as_bytes(), round)?;
                    drop(payload);
                    framing::write(output, &token)?;
                    drop(token);
                    phase = if complete {
                        Phase::Finish
                    } else {
                        Phase::Challenge(round + 1)
                    };
                    continue;
                }
            }
            let a = active.as_mut().unwrap();
            let (token, complete) = a.token(&[], 1)?;
            framing::write(output, &token)?;
            drop(token);
            phase = if complete {
                Phase::Finish
            } else {
                Phase::Challenge(2)
            };
        }
    })();
    // EOF/error paths also perform normal serial cleanup; a failed cleanup
    // cannot become Finished or successful worker exit.
    let cleanup = active.as_mut().map(|a| a.cleanup()).transpose();
    drop(active);
    let result = result.and(cleanup.map(|_| ()));
    if let Err(error) = &result
        && let Ok(frame) = codec::error(phase, error)
    {
        let _ = write(output, frame, probe);
    }
    result
}

fn audit_request(request: &mut AuthRequest, probe: &crate::secret::audit::Probe) {
    request.target.0.probe = probe.clone();
    request.channel_binding.0.probe = probe.clone();
    if let crate::Identity::Explicit {
        user,
        domain,
        password,
    } = &mut request.identity
    {
        user.0.probe = probe.clone();
        domain.0.probe = probe.clone();
        password.0.probe = probe.clone();
    }
    if let Some(digest) = request.digest.as_mut() {
        digest.initial_challenge.0.probe = probe.clone();
        digest.method.0.probe = probe.clone();
        digest.uri.0.probe = probe.clone();
    }
}
