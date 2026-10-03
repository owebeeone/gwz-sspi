// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.
use super::*;
pub(in crate::protocol) struct MechanismObservationRef {
    pub(in crate::protocol) kind: ObservationKind,
    pub(in crate::protocol) mechanism: Option<Mechanism>,
    pub(in crate::protocol) authoritative: bool,
}
pub(in crate::protocol) struct MechanismObservationOwned {
    pub(in crate::protocol) kind: ObservationKind,
    pub(in crate::protocol) mechanism: Option<Mechanism>,
    pub(in crate::protocol) authoritative: bool,
}
impl<'a> MechanismObservationRef {
    pub(in crate::protocol) fn read(r: &mut Reader<'a>, depth: usize) -> Result<Self, CodecError> {
        r.map(3, depth)?;
        r.key(1)?;
        let kind = ObservationKind::read(r)?;
        r.key(2)?;
        let mechanism = if r.null()? {
            None
        } else {
            Some(Mechanism::read(r)?)
        };
        r.key(3)?;
        let authoritative = r.boolean()?;
        Ok(Self {
            kind,
            mechanism,
            authoritative,
        })
    }
    pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {
        sink.head(5, 3)?;
        sink.head(0, 1)?;
        sink.head(0, self.kind as u64)?;
        sink.head(0, 2)?;
        if let Some(value) = &self.mechanism {
            sink.head(0, *value as u64)?;
        } else {
            sink.raw(&[0xf6])?;
        }
        sink.head(0, 3)?;
        sink.raw(&[if self.authoritative { 0xf5 } else { 0xf4 }])?;
        Ok(())
    }
    pub(in crate::protocol) fn own(&self) -> MechanismObservationOwned {
        MechanismObservationOwned {
            kind: self.kind,
            mechanism: self.mechanism.as_ref().map(|value| *value),
            authoritative: self.authoritative,
        }
    }
}
impl MechanismObservationOwned {
    pub(in crate::protocol) fn borrow(&self) -> MechanismObservationRef {
        MechanismObservationRef {
            kind: self.kind,
            mechanism: self.mechanism.as_ref().map(|value| *value),
            authoritative: self.authoritative,
        }
    }
}
