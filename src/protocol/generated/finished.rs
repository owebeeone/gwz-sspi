// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct FinishedRef {}
pub(in crate::protocol) struct FinishedOwned {}
impl<'a> FinishedRef {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(0, depth)?;
        Ok(Self {})
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 0)?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> FinishedOwned {
        FinishedOwned {}
    }
}
impl FinishedOwned {
    pub(in crate::protocol) fn borrow(&self) -> FinishedRef {
        FinishedRef {}
    }
}
