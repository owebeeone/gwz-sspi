// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct PrimaryIdentityRef<'a> {
    pub(in crate::protocol) sid: &'a [u8],
    pub(in crate::protocol) authentication_luid: &'a [u8],
    pub(in crate::protocol) session_id: u64,
}
pub(in crate::protocol) struct PrimaryIdentityOwned {
    pub(in crate::protocol) sid: SecretBytes,
    pub(in crate::protocol) authentication_luid: SecretBytes,
    pub(in crate::protocol) session_id: u64,
}
impl<'a> PrimaryIdentityRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(3, depth)?;
        r.key(1)?;
        let sid = r.bytes()?;
        r.key(2)?;
        let authentication_luid = r.bytes()?;
        r.key(3)?;
        let session_id = r.uint()?;
        Ok(Self {
            sid,
            authentication_luid,
            session_id,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 3)?;
        sink.head(0, 1)?;
        sink.string(2, self.sid)?;
        sink.head(0, 2)?;
        sink.string(2, self.authentication_luid)?;
        sink.head(0, 3)?;
        sink.head(0, self.session_id)?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> PrimaryIdentityOwned {
        PrimaryIdentityOwned {
            sid: SecretBytes::new(self.sid),
            authentication_luid: SecretBytes::new(self.authentication_luid),
            session_id: self.session_id,
        }
    }
}
impl PrimaryIdentityOwned {
    pub(in crate::protocol) fn borrow(&self) -> PrimaryIdentityRef<'_> {
        PrimaryIdentityRef {
            sid: self.sid.as_bytes(),
            authentication_luid: self.authentication_luid.as_bytes(),
            session_id: self.session_id,
        }
    }
}
