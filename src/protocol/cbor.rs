use super::generated::{MAX_BODY, MAX_DEPTH};
use crate::{Error, ErrorKind};
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}

pub(super) struct Reader<'a> {
    source: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn new(source: &'a [u8]) -> Self {
        Self { source, pos: 0 }
    }
    fn byte(&mut self) -> Result<u8, Error> {
        let b = *self.source.get(self.pos).ok_or_else(bad)?;
        self.pos += 1;
        Ok(b)
    }
    fn head(&mut self, major: u8) -> Result<u64, Error> {
        let b = self.byte()?;
        if b >> 5 != major {
            return Err(bad());
        }
        let additional = b & 31;
        if additional < 24 {
            return Ok(u64::from(additional));
        }
        let (n, min) = match additional {
            24 => (1, 24),
            25 => (2, 256),
            26 => (4, 65536),
            27 => (8, 4294967296),
            _ => {
                return Err(bad());
            }
        };
        let mut value = 0u64;
        for _ in 0..n {
            value = (value << 8) | u64::from(self.byte()?);
        }
        if value < min {
            return Err(bad());
        }
        Ok(value)
    }
    pub(super) fn uint(&mut self) -> Result<u64, Error> {
        self.head(0)
    }
    pub(super) fn map(&mut self, count: usize, depth: usize) -> Result<(), Error> {
        if depth > MAX_DEPTH || self.head(5)? != count as u64 {
            return Err(bad());
        }
        Ok(())
    }
    pub(super) fn key(&mut self, key: u64) -> Result<(), Error> {
        if self.uint()? != key {
            return Err(bad());
        }
        Ok(())
    }
    pub(super) fn null(&mut self) -> Result<bool, Error> {
        if self.source.get(self.pos) == Some(&0xf6) {
            self.pos += 1;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0xf4 => Ok(false),
            0xf5 => Ok(true),
            _ => Err(bad()),
        }
    }
    fn string(&mut self, major: u8) -> Result<&'a [u8], Error> {
        let len = usize::try_from(self.head(major)?).map_err(|_| bad())?;
        let end = self.pos.checked_add(len).ok_or_else(bad)?;
        let value = self.source.get(self.pos..end).ok_or_else(bad)?;
        self.pos = end;
        Ok(value)
    }
    pub(super) fn bytes(&mut self) -> Result<&'a [u8], Error> {
        self.string(2)
    }
    pub(super) fn text(&mut self) -> Result<&'a str, Error> {
        let value = self.string(3)?;
        if value.contains(&0) {
            return Err(bad());
        }
        std::str::from_utf8(value).map_err(|_| bad())
    }
    pub(super) fn end(&self) -> Result<(), Error> {
        if self.pos != self.source.len() {
            return Err(bad());
        }
        Ok(())
    }
}

pub(super) trait Sink {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error>;
    fn head(&mut self, major: u8, value: u64) -> Result<(), Error> {
        let mut bytes = [0u8; 9];
        let (additional, width) = if value < 24 {
            (value as u8, 0)
        } else if value <= 255 {
            (24, 1)
        } else if value <= 65535 {
            (25, 2)
        } else if value <= u64::from(u32::MAX) {
            (26, 4)
        } else {
            (27, 8)
        };
        bytes[0] = (major << 5) | additional;
        for n in 0..width {
            bytes[width - n] = (value >> (n * 8)) as u8;
        }
        self.raw(&bytes[..width + 1])
    }
    fn string(&mut self, major: u8, bytes: &[u8]) -> Result<(), Error> {
        self.head(major, bytes.len() as u64)?;
        self.raw(bytes)
    }
}
pub(super) struct Size(pub(super) usize);
impl Sink for Size {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.0 = self.0.checked_add(bytes.len()).ok_or_else(bad)?;
        if self.0 > MAX_BODY {
            return Err(bad());
        }
        Ok(())
    }
}
pub(super) struct Writer<'a> {
    bytes: &'a mut [u8],
    pos: usize,
}
impl<'a> Writer<'a> {
    pub(super) fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub(super) fn end(&self) -> Result<(), Error> {
        if self.pos != self.bytes.len() {
            return Err(bad());
        }
        Ok(())
    }
}
impl Sink for Writer<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self.pos.checked_add(bytes.len()).ok_or_else(bad)?;
        self.bytes
            .get_mut(self.pos..end)
            .ok_or_else(bad)?
            .copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
}
