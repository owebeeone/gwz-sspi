//! Exact partial framing over fake byte input/output; no I/O or process access.
use super::generated::MAX_BODY;
use crate::secret::{Storage, audit};
use crate::{Error, ErrorKind};
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}

pub(super) struct ReadFrame {
    header: [u8; 4],
    header_read: usize,
    body: Option<Storage>,
    body_read: usize,
    terminal: bool,
    pub(super) probe: audit::Probe,
}
impl ReadFrame {
    pub(super) fn new() -> Self {
        Self {
            header: [0; 4],
            header_read: 0,
            body: None,
            body_read: 0,
            terminal: false,
            probe: audit::Probe::default(),
        }
    }
    fn fail(&mut self) -> Error {
        self.terminal = true;
        self.body.take();
        bad()
    }
    /// Consume only the bytes belonging to this one frame. Caller holds leftovers.
    pub(super) fn push(&mut self, input: &[u8]) -> Result<usize, Error> {
        if self.terminal {
            return Err(bad());
        }
        let mut consumed = 0;
        if self.header_read < 4 {
            let count = (4 - self.header_read).min(input.len());
            self.header[self.header_read..self.header_read + count]
                .copy_from_slice(&input[..count]);
            self.header_read += count;
            consumed += count;
            if self.header_read == 4 {
                let len = u32::from_le_bytes(self.header) as usize;
                if !(1..=MAX_BODY).contains(&len) {
                    return Err(self.fail());
                }
                let mut storage = Storage::zeroed(len);
                std::mem::swap(&mut storage.probe, &mut self.probe);
                self.body = Some(storage);
            }
        }
        if let Some(body) = &mut self.body {
            let count = (body.as_slice().len() - self.body_read).min(input.len() - consumed);
            body.as_mut()[self.body_read..self.body_read + count]
                .copy_from_slice(&input[consumed..consumed + count]);
            self.body_read += count;
            consumed += count;
        }
        Ok(consumed)
    }
    pub(super) fn complete(&self) -> bool {
        self.body
            .as_ref()
            .is_some_and(|body| self.body_read == body.as_slice().len())
    }
    pub(super) fn take(&mut self) -> Result<Storage, Error> {
        if self.terminal || !self.complete() {
            return Err(self.fail());
        }
        self.terminal = true;
        self.body.take().ok_or_else(bad)
    }
    /// EOF is a terminal transport failure. This framing layer cannot prove
    /// Finished or process disposal, even after receiving a complete body.
    pub(super) fn eof(&mut self) -> Error {
        self.fail()
    }
    pub(super) fn abort(&mut self) {
        let _ = self.fail();
    }
}

pub(super) struct WriteFrame {
    header: [u8; 4],
    body: Option<Storage>,
    header_written: usize,
    body_written: usize,
    terminal: bool,
}
impl WriteFrame {
    pub(super) fn new(body: Storage) -> Result<Self, Error> {
        if !(1..=MAX_BODY).contains(&body.as_slice().len()) {
            return Err(bad());
        }
        Ok(Self {
            header: (body.as_slice().len() as u32).to_le_bytes(),
            body: Some(body),
            header_written: 0,
            body_written: 0,
            terminal: false,
        })
    }
    pub(super) fn remaining(&self) -> Result<&[u8], Error> {
        if self.terminal {
            return Err(bad());
        }
        if self.header_written < 4 {
            return Ok(&self.header[self.header_written..]);
        }
        Ok(&self.body.as_ref().ok_or_else(bad)?.as_slice()[self.body_written..])
    }
    pub(super) fn advance(&mut self, count: usize) -> Result<(), Error> {
        if count == 0 || count > self.remaining()?.len() {
            self.abort();
            return Err(bad());
        }
        if self.header_written < 4 {
            self.header_written += count;
        } else {
            self.body_written += count;
        }
        Ok(())
    }
    pub(super) fn complete(&self) -> bool {
        !self.terminal
            && self.header_written == 4
            && self
                .body
                .as_ref()
                .is_some_and(|body| self.body_written == body.as_slice().len())
    }
    /// Release only when fake writer completion is accounted for.
    pub(super) fn finish(mut self) -> Result<(), Error> {
        if !self.complete() {
            self.abort();
            return Err(bad());
        }
        self.body.take();
        self.terminal = true;
        Ok(())
    }
    pub(super) fn abort(&mut self) {
        self.body.take();
        self.terminal = true;
    }
}
