//! Pure phase and first-terminal arbitration; no I/O, allocation or callbacks.
use crate::{Error, ErrorKind, TokenStatus};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Fault {
    pub(super) kind: ErrorKind,
    pub(super) native: Option<u32>,
}
impl Fault {
    pub(super) const fn new(kind: ErrorKind) -> Self {
        Self { kind, native: None }
    }
    pub(super) fn from_error(error: Error) -> Self {
        Self {
            kind: error.kind(),
            native: error.native_status(),
        }
    }
    pub(super) fn error(self) -> Error {
        if self.kind == ErrorKind::ProviderRejected {
            return Error::provider(self.native);
        }
        Error::new(self.kind)
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Proof {
    pub(super) process_exited: bool,
    pub(super) job_empty: bool,
    pub(super) launch_finished: bool,
    pub(super) reader_finished: bool,
    pub(super) writer_finished: bool,
}
impl Proof {
    pub(super) fn complete(self) -> bool {
        self.process_exited
            && self.job_empty
            && self.launch_finished
            && self.reader_finished
            && self.writer_finished
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    Begin,
    Challenge(u8),
    Finish,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Expected {
    Bootstrap,
    Begin,
    Challenge(u8),
    Finish,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    Launching,
    Hello,
    Ready,
    Waiting {
        round: u8,
        write_done: bool,
    },
    TokenReady {
        round: u8,
        status: TokenStatus,
        write_done: bool,
    },
    Between(u8),
    Complete,
    Closing {
        ack: bool,
        write_done: bool,
    },
    Terminal,
    Reaped,
}
pub(super) struct Kernel {
    pub(super) deadline: Instant,
    pub(super) stage: Stage,
    pub(super) terminal: Option<Fault>,
    pub(super) proof: Proof,
    pub(super) normal_finished: bool,
    pub(super) quarantined: bool,
}
impl Kernel {
    pub(super) fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            stage: Stage::Launching,
            terminal: None,
            proof: Proof::default(),
            normal_finished: false,
            quarantined: false,
        }
    }
    pub(super) fn fail(&mut self, fault: Fault) -> bool {
        if self.terminal.is_some() || self.stage == Stage::Reaped {
            return false;
        }
        self.terminal = Some(fault);
        self.stage = Stage::Terminal;
        true
    }
    pub(super) fn control(&mut self, now: Instant, cancelled: bool) {
        if cancelled {
            self.fail(Fault::new(ErrorKind::Cancelled));
        } else if now >= self.deadline {
            self.fail(Fault::new(ErrorKind::Timeout));
        }
    }
    pub(super) fn launch_returned(&mut self, contained: bool) -> Result<bool, Fault> {
        if self.terminal.is_some() {
            return Ok(false);
        }
        if self.stage != Stage::Launching || !contained {
            return Err(Fault::new(ErrorKind::ContainmentFailed));
        }
        self.stage = Stage::Hello;
        Ok(true)
    }
    pub(super) fn hello(&mut self) -> Result<(), Fault> {
        if self.stage != Stage::Hello {
            return Err(Fault::new(ErrorKind::Protocol));
        }
        self.stage = Stage::Ready;
        Ok(())
    }
    pub(super) fn expected(&self) -> Option<Expected> {
        match self.stage {
            Stage::Hello => Some(Expected::Bootstrap),
            Stage::Waiting { round: 1, .. } => Some(Expected::Begin),
            Stage::Waiting { round, .. } => Some(Expected::Challenge(round)),
            Stage::Closing { ack: false, .. } => Some(Expected::Finish),
            _ => None,
        }
    }
    pub(super) fn command(&mut self, command: Command) -> Result<(), Fault> {
        if let Some(fault) = self.terminal {
            return Err(fault);
        }
        self.stage = match (self.stage, command) {
            (Stage::Ready, Command::Begin) => Stage::Waiting {
                round: 1,
                write_done: false,
            },
            (Stage::Between(previous), Command::Challenge(round))
                if previous < 8 && round == previous + 1 =>
            {
                Stage::Waiting {
                    round,
                    write_done: false,
                }
            }
            (Stage::Ready | Stage::Between(_) | Stage::Complete, Command::Finish) => {
                Stage::Closing {
                    ack: false,
                    write_done: false,
                }
            }
            _ => {
                return Err(Fault::new(ErrorKind::Protocol));
            }
        };
        Ok(())
    }
    pub(super) fn write_done(&mut self) -> Result<(), Fault> {
        if self.terminal.is_some() {
            return Ok(());
        }
        match &mut self.stage {
            Stage::Waiting { write_done, .. }
            | Stage::TokenReady { write_done, .. }
            | Stage::Closing { write_done, .. } => {
                if *write_done {
                    return Err(Fault::new(ErrorKind::Protocol));
                }
                *write_done = true;
                Ok(())
            }
            _ => Err(Fault::new(ErrorKind::Protocol)),
        }
    }
    pub(super) fn token(&mut self, round: u8, status: TokenStatus) -> Result<(), Fault> {
        match self.stage {
            Stage::Waiting {
                round: expected,
                write_done,
            } if round == expected
                && (1..=8).contains(&round)
                && !(round == 8 && status == TokenStatus::Continue) =>
            {
                self.stage = Stage::TokenReady {
                    round,
                    status,
                    write_done,
                };
                Ok(())
            }
            _ => Err(Fault::new(ErrorKind::Protocol)),
        }
    }
    pub(super) fn publishable(&self) -> bool {
        self.terminal.is_none()
            && matches!(
                self.stage,
                Stage::TokenReady {
                    write_done: true,
                    ..
                }
            )
    }
    pub(super) fn published(&mut self) -> Result<(), Fault> {
        match self.stage {
            Stage::TokenReady {
                round,
                status,
                write_done: true,
            } if self.terminal.is_none() => {
                self.stage = if status == TokenStatus::Complete {
                    Stage::Complete
                } else {
                    Stage::Between(round)
                };
                Ok(())
            }
            _ => Err(Fault::new(ErrorKind::Protocol)),
        }
    }
    pub(super) fn finished(&mut self) -> Result<(), Fault> {
        match &mut self.stage {
            Stage::Closing { ack, .. } if !*ack => {
                *ack = true;
                self.normal_finished = true;
                Ok(())
            }
            _ => Err(Fault::new(ErrorKind::Protocol)),
        }
    }
    pub(super) fn normal_ack(&self) -> bool {
        self.terminal.is_none()
            && matches!(
                self.stage,
                Stage::Closing {
                    ack: true,
                    write_done: true
                }
            )
    }
    pub(super) fn reapable(&self) -> bool {
        self.proof.complete() && (self.terminal.is_some() || self.normal_ack())
    }
}
