// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct EnvelopeRef<'a> {
    pub(in crate::protocol) version: u64,
    pub(in crate::protocol) kind: MessageKind,
    pub(in crate::protocol) hello: Option<HelloRef<'a>>,
    pub(in crate::protocol) begin: Option<BeginRef<'a>>,
    pub(in crate::protocol) challenge: Option<ChallengeRef<'a>>,
    pub(in crate::protocol) token: Option<TokenRef<'a>>,
    pub(in crate::protocol) finish: Option<FinishRef>,
    pub(in crate::protocol) finished: Option<FinishedRef>,
    pub(in crate::protocol) error: Option<ErrorRef>,
}
pub(in crate::protocol) struct EnvelopeOwned {
    pub(in crate::protocol) version: u64,
    pub(in crate::protocol) kind: MessageKind,
    pub(in crate::protocol) hello: Option<HelloOwned>,
    pub(in crate::protocol) begin: Option<BeginOwned>,
    pub(in crate::protocol) challenge: Option<ChallengeOwned>,
    pub(in crate::protocol) token: Option<TokenOwned>,
    pub(in crate::protocol) finish: Option<FinishOwned>,
    pub(in crate::protocol) finished: Option<FinishedOwned>,
    pub(in crate::protocol) error: Option<ErrorOwned>,
}
impl<'a> EnvelopeRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(9, depth)?;
        r.key(1)?;
        let version = r.uint()?;
        r.key(2)?;
        let kind = MessageKind::read(r)?;
        r.key(10)?;
        let hello = if r.null()? {
            None
        } else {
            Some(HelloRef::read(r, depth + 1)?)
        };
        r.key(11)?;
        let begin = if r.null()? {
            None
        } else {
            Some(BeginRef::read(r, depth + 1)?)
        };
        r.key(12)?;
        let challenge = if r.null()? {
            None
        } else {
            Some(ChallengeRef::read(r, depth + 1)?)
        };
        r.key(13)?;
        let token = if r.null()? {
            None
        } else {
            Some(TokenRef::read(r, depth + 1)?)
        };
        r.key(14)?;
        let finish = if r.null()? {
            None
        } else {
            Some(FinishRef::read(r, depth + 1)?)
        };
        r.key(15)?;
        let finished = if r.null()? {
            None
        } else {
            Some(FinishedRef::read(r, depth + 1)?)
        };
        r.key(16)?;
        let error = if r.null()? {
            None
        } else {
            Some(ErrorRef::read(r, depth + 1)?)
        };
        Ok(Self {
            version,
            kind,
            hello,
            begin,
            challenge,
            token,
            finish,
            finished,
            error,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 9)?;
        sink.head(0, 1)?;
        sink.head(0, self.version)?;
        sink.head(0, 2)?;
        sink.head(0, self.kind as u64)?;
        sink.head(0, 10)?;
        if let Some(value) = &self.hello {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 11)?;
        if let Some(value) = &self.begin {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 12)?;
        if let Some(value) = &self.challenge {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 13)?;
        if let Some(value) = &self.token {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 14)?;
        if let Some(value) = &self.finish {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 15)?;
        if let Some(value) = &self.finished {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 16)?;
        if let Some(value) = &self.error {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> EnvelopeOwned {
        EnvelopeOwned {
            version: self.version,
            kind: self.kind,
            hello: self.hello.as_ref().map(|value| value.own()),
            begin: self.begin.as_ref().map(|value| value.own()),
            challenge: self.challenge.as_ref().map(|value| value.own()),
            token: self.token.as_ref().map(|value| value.own()),
            finish: self.finish.as_ref().map(|value| value.own()),
            finished: self.finished.as_ref().map(|value| value.own()),
            error: self.error.as_ref().map(|value| value.own()),
        }
    }
}
impl EnvelopeOwned {
    pub(in crate::protocol) fn borrow(&self) -> EnvelopeRef<'_> {
        EnvelopeRef {
            version: self.version,
            kind: self.kind,
            hello: self.hello.as_ref().map(|value| value.borrow()),
            begin: self.begin.as_ref().map(|value| value.borrow()),
            challenge: self.challenge.as_ref().map(|value| value.borrow()),
            token: self.token.as_ref().map(|value| value.borrow()),
            finish: self.finish.as_ref().map(|value| value.borrow()),
            finished: self.finished.as_ref().map(|value| value.borrow()),
            error: self.error.as_ref().map(|value| value.borrow()),
        }
    }
}
