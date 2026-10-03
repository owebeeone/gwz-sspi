// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct ErrorRef {
    pub(in crate::protocol) kind: ErrorKind,
    pub(in crate::protocol) phase: ErrorPhase,
    pub(in crate::protocol) native_status: Option<u64>,
}
pub(in crate::protocol) struct ErrorOwned {
    pub(in crate::protocol) kind: ErrorKind,
    pub(in crate::protocol) phase: ErrorPhase,
    pub(in crate::protocol) native_status: Option<u64>,
}
impl<'a> ErrorRef {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(3, depth)?;
        r.key(1)?;
        let kind = ErrorKind::read(r)?;
        r.key(2)?;
        let phase = ErrorPhase::read(r)?;
        r.key(3)?;
        let native_status = if r.null()? { None } else { Some(r.uint()?) };
        Ok(Self {
            kind,
            phase,
            native_status,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 3)?;
        sink.head(0, 1)?;
        sink.head(0, self.kind as u64)?;
        sink.head(0, 2)?;
        sink.head(0, self.phase as u64)?;
        sink.head(0, 3)?;
        if let Some(value) = &self.native_status {
            sink.head(0, *value)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> ErrorOwned {
        ErrorOwned {
            kind: self.kind,
            phase: self.phase,
            native_status: self.native_status.as_ref().map(|value| *value),
        }
    }
}
impl ErrorOwned {
    pub(in crate::protocol) fn borrow(&self) -> ErrorRef {
        ErrorRef {
            kind: self.kind,
            phase: self.phase,
            native_status: self.native_status.as_ref().map(|value| *value),
        }
    }
}
