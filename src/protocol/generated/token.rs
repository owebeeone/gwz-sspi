// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct TokenRef<'a> {
    pub(in crate::protocol) round: u64,
    pub(in crate::protocol) status: TokenStatus,
    pub(in crate::protocol) attributes: u64,
    pub(in crate::protocol) observation: MechanismObservationRef,
    pub(in crate::protocol) payload: &'a [u8],
}
pub(in crate::protocol) struct TokenOwned {
    pub(in crate::protocol) round: u64,
    pub(in crate::protocol) status: TokenStatus,
    pub(in crate::protocol) attributes: u64,
    pub(in crate::protocol) observation: MechanismObservationOwned,
    pub(in crate::protocol) payload: SecretBytes,
}
impl<'a> TokenRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(5, depth)?;
        r.key(1)?;
        let round = r.uint()?;
        r.key(2)?;
        let status = TokenStatus::read(r)?;
        r.key(3)?;
        let attributes = r.uint()?;
        r.key(4)?;
        let observation = MechanismObservationRef::read(r, depth + 1)?;
        r.key(5)?;
        let payload = r.bytes()?;
        Ok(Self {
            round,
            status,
            attributes,
            observation,
            payload,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 5)?;
        sink.head(0, 1)?;
        sink.head(0, self.round)?;
        sink.head(0, 2)?;
        sink.head(0, self.status as u64)?;
        sink.head(0, 3)?;
        sink.head(0, self.attributes)?;
        sink.head(0, 4)?;
        (self.observation).emit(sink)?;
        sink.head(0, 5)?;
        sink.string(2, self.payload)?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> TokenOwned {
        TokenOwned {
            round: self.round,
            status: self.status,
            attributes: self.attributes,
            observation: self.observation.own(),
            payload: SecretBytes::new(self.payload),
        }
    }
}
impl TokenOwned {
    pub(in crate::protocol) fn borrow(&self) -> TokenRef<'_> {
        TokenRef {
            round: self.round,
            status: self.status,
            attributes: self.attributes,
            observation: self.observation.borrow(),
            payload: self.payload.as_bytes(),
        }
    }
}
