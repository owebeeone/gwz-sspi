//! One dedicated reader and writer; buffers/handles remain owned through calls.
use super::{
    context::{Context, Record},
    kernel::Fault,
    ports::{ReadPort, WritePort},
};
use crate::protocol::supervision::{self, Reply};
use crate::secret::Storage;
use crate::{Error, ErrorKind};
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc::Receiver};
use std::time::Instant;
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}
fn exact_read(port: &mut dyn ReadPort, buffer: &mut [u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buffer.len() {
        let count = port.read(&mut buffer[offset..])?;
        if count == 0 || count > buffer.len() - offset {
            return Err(bad());
        }
        offset += count;
    }
    Ok(())
}
pub(super) fn read_frame(port: &mut dyn ReadPort) -> Result<Storage, Error> {
    let mut header = [0; 4];
    exact_read(port, &mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if !(1..=100000).contains(&len) {
        return Err(bad());
    }
    let mut frame = Storage::zeroed(len);
    exact_read(port, frame.as_mut())?;
    Ok(frame)
}
pub(super) fn exact_write(
    port: &mut dyn WritePort,
    buffer: &[u8],
    record: &Record,
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buffer.len() {
        if record.stop.load(Ordering::Acquire) {
            return Err(Error::new(ErrorKind::Cancelled));
        }
        let count = port.write(&buffer[offset..])?;
        if count == 0 || count > buffer.len() - offset {
            return Err(bad());
        }
        offset += count;
    }
    Ok(())
}
pub(super) fn reader(context: Arc<Context>, record: Arc<Record>, mut port: Box<dyn ReadPort>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        read_loop(&context, &record, &mut *port)
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => context.fault(&record, Fault::from_error(error)),
        Err(_) => context.fault(&record, Fault::new(ErrorKind::Protocol)),
    }
}
fn read_loop(context: &Context, record: &Record, port: &mut dyn ReadPort) -> Result<(), Error> {
    loop {
        if record.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let frame = read_frame(port)?;
        let expected = context
            .update(record, Instant::now(), |kernel| kernel.expected())
            .flatten();
        let Some(expected) = expected else {
            if record.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            return Err(bad());
        };
        let reply = supervision::reply(
            &frame,
            expected,
            context.platform.primary(),
            &context.executable.build,
            record.package,
            record.cap,
        )?;
        drop(frame);
        match reply {
            Reply::Hello => {
                let result = context.update(record, Instant::now(), |kernel| kernel.hello());
                if let Some(Err(fault)) = result {
                    context.fault(record, fault);
                    return Ok(());
                }
            }
            Reply::Token { round, step } => {
                // Pending bytes are placed outside the phase lock. The subsequent
                // transition still arbitrates cancellation/deadline first.
                let status = step.status;
                let old = record
                    .payload
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .token
                    .replace(step);
                drop(old);
                let result =
                    context.update(record, Instant::now(), |kernel| kernel.token(round, status));
                if let Some(Err(fault)) = result {
                    context.fault(record, fault);
                    return Ok(());
                }
            }
            Reply::Finished => {
                let result = context.update(record, Instant::now(), |kernel| kernel.finished());
                if let Some(Err(fault)) = result {
                    context.fault(record, fault);
                }
                context.hub.notify();
                return Ok(());
            }
            Reply::Failure(error) => {
                context.fault(record, Fault::from_error(error));
                return Ok(());
            }
        }
        context.hub.notify();
    }
}
pub(super) fn writer(
    context: Arc<Context>,
    record: Arc<Record>,
    mut port: Box<dyn WritePort>,
    commands: Receiver<Storage>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        while let Ok(frame) = commands.recv() {
            if record.stop.load(Ordering::Acquire) {
                break;
            }
            exact_write(
                &mut *port,
                &(frame.as_slice().len() as u32).to_le_bytes(),
                &record,
            )?;
            exact_write(&mut *port, frame.as_slice(), &record)?;
            drop(frame);
            if let Some(Err(fault)) =
                context.update(&record, Instant::now(), |kernel| kernel.write_done())
            {
                context.fault(&record, fault);
                break;
            }
            context.hub.notify();
        }
        Ok::<_, Error>(())
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => context.fault(&record, Fault::from_error(error)),
        Err(_) => context.fault(&record, Fault::new(ErrorKind::Protocol)),
    }
}
