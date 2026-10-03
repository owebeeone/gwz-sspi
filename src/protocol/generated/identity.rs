// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct IdentityRef<'a> {
    pub(in crate::protocol) mode: IdentityMode,
    pub(in crate::protocol) user: Option<&'a str>,
    pub(in crate::protocol) domain: Option<&'a str>,
    pub(in crate::protocol) password: Option<&'a str>,
}
pub(in crate::protocol) struct IdentityOwned {
    pub(in crate::protocol) mode: IdentityMode,
    pub(in crate::protocol) user: Option<SecretText>,
    pub(in crate::protocol) domain: Option<SecretText>,
    pub(in crate::protocol) password: Option<SecretText>,
}
impl<'a> IdentityRef<'a> {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(4, depth)?;
        r.key(1)?;
        let mode = IdentityMode::read(r)?;
        r.key(2)?;
        let user = if r.null()? { None } else { Some(r.text()?) };
        r.key(3)?;
        let domain = if r.null()? { None } else { Some(r.text()?) };
        r.key(4)?;
        let password = if r.null()? { None } else { Some(r.text()?) };
        Ok(Self {
            mode,
            user,
            domain,
            password,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 4)?;
        sink.head(0, 1)?;
        sink.head(0, self.mode as u64)?;
        sink.head(0, 2)?;
        if let Some(value) = &self.user {
            sink.string(3, (value).as_bytes())?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 3)?;
        if let Some(value) = &self.domain {
            sink.string(3, (value).as_bytes())?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 4)?;
        if let Some(value) = &self.password {
            sink.string(3, (value).as_bytes())?;
        } else {
            sink.raw(&[0xf6])?;
        }
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> IdentityOwned {
        IdentityOwned {
            mode: self.mode,
            user: self.user.as_ref().map(|value| SecretText::admitted(value)),
            domain: self
                .domain
                .as_ref()
                .map(|value| SecretText::admitted(value)),
            password: self
                .password
                .as_ref()
                .map(|value| SecretText::admitted(value)),
        }
    }
}
impl IdentityOwned {
    pub(in crate::protocol) fn borrow(&self) -> IdentityRef<'_> {
        IdentityRef {
            mode: self.mode,
            user: self.user.as_ref().map(|value| value.as_str()),
            domain: self.domain.as_ref().map(|value| value.as_str()),
            password: self.password.as_ref().map(|value| value.as_str()),
        }
    }
}
