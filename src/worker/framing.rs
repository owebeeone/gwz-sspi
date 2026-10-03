use crate::secret::Storage;
use crate::supervisor::ports::{ReadPort, WritePort};
use crate::{Error, ErrorKind};
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}
pub(super) fn read(
    port: &mut dyn ReadPort,
    probe: &crate::secret::audit::Probe,
) -> Result<Option<Storage>, Error> {
    let mut header = [0; 4];
    let mut at = 0;
    while at < 4 {
        let n = port.read(&mut header[at..])?;
        if n == 0 && at == 0 {
            return Ok(None);
        }
        if n == 0 || n > 4 - at {
            return Err(bad());
        }
        at += n;
    }
    let len = u32::from_le_bytes(header) as usize;
    if !(1..=100000).contains(&len) {
        return Err(bad());
    }
    let mut frame = Storage::zeroed(len);
    frame.probe = probe.clone();
    at = 0;
    while at < len {
        let n = port.read(&mut frame.as_mut()[at..])?;
        if n == 0 || n > len - at {
            return Err(bad());
        }
        at += n;
    }
    Ok(Some(frame))
}
pub(super) fn write(port: &mut dyn WritePort, frame: &Storage) -> Result<(), Error> {
    for bytes in [
        &(frame.as_slice().len() as u32).to_le_bytes()[..],
        frame.as_slice(),
    ] {
        let mut at = 0;
        while at < bytes.len() {
            let n = port.write(&bytes[at..])?;
            if n == 0 || n > bytes.len() - at {
                return Err(bad());
            }
            at += n;
        }
    }
    Ok(())
}
