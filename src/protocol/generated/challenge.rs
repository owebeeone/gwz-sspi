// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct ChallengeRef<'a> {
    pub(in crate::protocol) round: u64,
    pub(in crate::protocol) payload: &'a [u8],
}
pub(in crate::protocol) struct ChallengeOwned {
    pub(in crate::protocol) round: u64,
    pub(in crate::protocol) payload: SecretBytes,
}
impl<'a> ChallengeRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(2, depth)?;
        r.key(1)?;
        let round = r.uint()?;
        r.key(2)?;
        let payload = r.bytes()?;
        Ok(Self { round, payload })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 2)?;
        sink.head(0, 1)?;
        sink.head(0, self.round)?;
        sink.head(0, 2)?;
        sink.string(2, self.payload)?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> ChallengeOwned {
        ChallengeOwned {
            round: self.round,
            payload: SecretBytes::new(self.payload),
        }
    }
}
impl ChallengeOwned {
    pub(in crate::protocol) fn borrow(&self) -> ChallengeRef<'_> {
        ChallengeRef {
            round: self.round,
            payload: self.payload.as_bytes(),
        }
    }
}
