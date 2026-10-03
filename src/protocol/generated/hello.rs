// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct HelloRef<'a> {
    pub(in crate::protocol) protocol_version: u64,
    pub(in crate::protocol) schema_fingerprint: &'a [u8],
    pub(in crate::protocol) build_fingerprint: &'a [u8],
    pub(in crate::protocol) primary_identity: PrimaryIdentityRef<'a>,
}
pub(in crate::protocol) struct HelloOwned {
    pub(in crate::protocol) protocol_version: u64,
    pub(in crate::protocol) schema_fingerprint: SecretBytes,
    pub(in crate::protocol) build_fingerprint: SecretBytes,
    pub(in crate::protocol) primary_identity: PrimaryIdentityOwned,
}
impl<'a> HelloRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(4, depth)?;
        r.key(1)?;
        let protocol_version = r.uint()?;
        r.key(2)?;
        let schema_fingerprint = r.bytes()?;
        r.key(3)?;
        let build_fingerprint = r.bytes()?;
        r.key(4)?;
        let primary_identity = PrimaryIdentityRef::read(r, depth + 1)?;
        Ok(Self {
            protocol_version,
            schema_fingerprint,
            build_fingerprint,
            primary_identity,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 4)?;
        sink.head(0, 1)?;
        sink.head(0, self.protocol_version)?;
        sink.head(0, 2)?;
        sink.string(2, self.schema_fingerprint)?;
        sink.head(0, 3)?;
        sink.string(2, self.build_fingerprint)?;
        sink.head(0, 4)?;
        (self.primary_identity).emit(sink)?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> HelloOwned {
        HelloOwned {
            protocol_version: self.protocol_version,
            schema_fingerprint: SecretBytes::new(self.schema_fingerprint),
            build_fingerprint: SecretBytes::new(self.build_fingerprint),
            primary_identity: self.primary_identity.own(),
        }
    }
}
impl HelloOwned {
    pub(in crate::protocol) fn borrow(&self) -> HelloRef<'_> {
        HelloRef {
            protocol_version: self.protocol_version,
            schema_fingerprint: self.schema_fingerprint.as_bytes(),
            build_fingerprint: self.build_fingerprint.as_bytes(),
            primary_identity: self.primary_identity.borrow(),
        }
    }
}
