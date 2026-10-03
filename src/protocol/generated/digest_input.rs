// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct DigestInputRef<'a> {
    pub(in crate::protocol) initial_challenge: &'a [u8],
    pub(in crate::protocol) method: &'a str,
    pub(in crate::protocol) uri: &'a str,
}
pub(in crate::protocol) struct DigestInputOwned {
    pub(in crate::protocol) initial_challenge: SecretBytes,
    pub(in crate::protocol) method: SecretText,
    pub(in crate::protocol) uri: SecretText,
}
impl<'a> DigestInputRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(3, depth)?;
        r.key(1)?;
        let initial_challenge = r.bytes()?;
        r.key(2)?;
        let method = r.text()?;
        r.key(3)?;
        let uri = r.text()?;
        Ok(Self {
            initial_challenge,
            method,
            uri,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 3)?;
        sink.head(0, 1)?;
        sink.string(2, self.initial_challenge)?;
        sink.head(0, 2)?;
        sink.string(3, (self.method).as_bytes())?;
        sink.head(0, 3)?;
        sink.string(3, (self.uri).as_bytes())?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> DigestInputOwned {
        DigestInputOwned {
            initial_challenge: SecretBytes::new(self.initial_challenge),
            method: SecretText::admitted(self.method),
            uri: SecretText::admitted(self.uri),
        }
    }
}
impl DigestInputOwned {
    pub(in crate::protocol) fn borrow(&self) -> DigestInputRef<'_> {
        DigestInputRef {
            initial_challenge: self.initial_challenge.as_bytes(),
            method: self.method.as_str(),
            uri: self.uri.as_str(),
        }
    }
}
