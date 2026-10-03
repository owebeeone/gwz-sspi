// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct BeginRef<'a> {
    pub(in crate::protocol) package: Package,
    pub(in crate::protocol) target: &'a str,
    pub(in crate::protocol) identity: IdentityRef<'a>,
    pub(in crate::protocol) channel_binding: &'a [u8],
    pub(in crate::protocol) token_limit: u64,
    pub(in crate::protocol) digest: Option<DigestInputRef<'a>>,
}
pub(in crate::protocol) struct BeginOwned {
    pub(in crate::protocol) package: Package,
    pub(in crate::protocol) target: SecretText,
    pub(in crate::protocol) identity: IdentityOwned,
    pub(in crate::protocol) channel_binding: SecretBytes,
    pub(in crate::protocol) token_limit: u64,
    pub(in crate::protocol) digest: Option<DigestInputOwned>,
}
impl<'a> BeginRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(6, depth)?;
        r.key(1)?;
        let package = Package::read(r)?;
        r.key(2)?;
        let target = r.text()?;
        r.key(3)?;
        let identity = IdentityRef::read(r, depth + 1)?;
        r.key(4)?;
        let channel_binding = r.bytes()?;
        r.key(5)?;
        let token_limit = r.uint()?;
        r.key(6)?;
        let digest = if r.null()? {
            None
        } else {
            Some(DigestInputRef::read(r, depth + 1)?)
        };
        Ok(Self {
            package,
            target,
            identity,
            channel_binding,
            token_limit,
            digest,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 6)?;
        sink.head(0, 1)?;
        sink.head(0, self.package as u64)?;
        sink.head(0, 2)?;
        sink.string(3, (self.target).as_bytes())?;
        sink.head(0, 3)?;
        (self.identity).emit(sink)?;
        sink.head(0, 4)?;
        sink.string(2, self.channel_binding)?;
        sink.head(0, 5)?;
        sink.head(0, self.token_limit)?;
        sink.head(0, 6)?;
        if let Some(value) = &self.digest {
            (value).emit(sink)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> BeginOwned {
        BeginOwned {
            package: self.package,
            target: SecretText::admitted(self.target),
            identity: self.identity.own(),
            channel_binding: SecretBytes::new(self.channel_binding),
            token_limit: self.token_limit,
            digest: self.digest.as_ref().map(|value| value.own()),
        }
    }
}
impl BeginOwned {
    pub(in crate::protocol) fn borrow(&self) -> BeginRef<'_> {
        BeginRef {
            package: self.package,
            target: self.target.as_str(),
            identity: self.identity.borrow(),
            channel_binding: self.channel_binding.as_bytes(),
            token_limit: self.token_limit,
            digest: self.digest.as_ref().map(|value| value.borrow()),
        }
    }
}
