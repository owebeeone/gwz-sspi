use super::super::ports::{ReadPort, WritePort};
struct Bytes {
    source: Vec<u8>,
    offset: usize,
    limit: usize,
    failure: Option<usize>,
    bad_count: bool,
}
impl ReadPort for Bytes {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
        if self.failure == Some(self.offset) {
            return Err(Error::new(ErrorKind::Protocol));
        }
        if self.bad_count {
            return Ok(destination.len() + 1);
        }
        let count = destination
            .len()
            .min(self.limit)
            .min(self.source.len() - self.offset);
        destination[..count].copy_from_slice(&self.source[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}
struct Sink {
    written: usize,
    limit: usize,
    failure: Option<usize>,
    bad_count: bool,
}
impl WritePort for Sink {
    fn write(&mut self, source: &[u8]) -> Result<usize, Error> {
        if self.failure == Some(self.written) {
            return Err(Error::new(ErrorKind::Protocol));
        }
        if self.bad_count {
            return Ok(source.len() + 1);
        }
        let count = source.len().min(self.limit);
        self.written += count;
        Ok(count)
    }
}
#[test]
fn actual_port_frame_reader_every_partial_eof_failure_and_accounting_refuses() {
    let body = b"synthetic-only";
    let mut full = (body.len() as u32).to_le_bytes().to_vec();
    full.extend_from_slice(body);
    for cut in 0..full.len() {
        let mut port = Bytes {
            source: full[..cut].to_vec(),
            offset: 0,
            limit: 1,
            failure: None,
            bad_count: false,
        };
        assert!(
            super::super::io::read_frame(&mut port).is_err(),
            "eof={cut}"
        );
        let mut port = Bytes {
            source: full.clone(),
            offset: 0,
            limit: 1,
            failure: Some(cut),
            bad_count: false,
        };
        assert!(
            super::super::io::read_frame(&mut port).is_err(),
            "failure={cut}"
        );
    }
    for limit in 1..full.len() + 1 {
        let mut port = Bytes {
            source: full.clone(),
            offset: 0,
            limit,
            failure: None,
            bad_count: false,
        };
        assert_eq!(
            super::super::io::read_frame(&mut port).unwrap().as_slice(),
            body
        );
    }
    let mut port = Bytes {
        source: full,
        offset: 0,
        limit: 1,
        failure: None,
        bad_count: true,
    };
    assert!(super::super::io::read_frame(&mut port).is_err());
    for length in [0, 100001, u32::MAX] {
        let mut port = Bytes {
            source: length.to_le_bytes().to_vec(),
            offset: 0,
            limit: 1,
            failure: None,
            bad_count: false,
        };
        assert!(super::super::io::read_frame(&mut port).is_err());
        assert_eq!(port.offset, 4);
    }
}
#[test]
fn actual_port_writer_partial_failures_and_cancellation_retain_fixed_source() {
    let context = context(1);
    let record = register(&context);
    let source = SecretBytes::new(b"synthetic-only");
    for failure in 0..source.as_bytes().len() {
        let mut port = Sink {
            written: 0,
            limit: 1,
            failure: Some(failure),
            bad_count: false,
        };
        assert!(
            super::super::io::exact_write(&mut port, source.as_bytes(), &record).is_err(),
            "failure={failure}"
        );
        assert_eq!(source.as_bytes(), b"synthetic-only");
    }
    for limit in 1..source.as_bytes().len() + 1 {
        let mut port = Sink {
            written: 0,
            limit,
            failure: None,
            bad_count: false,
        };
        super::super::io::exact_write(&mut port, source.as_bytes(), &record).unwrap();
        assert_eq!(port.written, source.as_bytes().len());
    }
    for (limit, bad_count) in [(0, false), (1, true)] {
        let mut port = Sink {
            written: 0,
            limit,
            failure: None,
            bad_count,
        };
        assert!(super::super::io::exact_write(&mut port, source.as_bytes(), &record).is_err());
    }
    record.stop.store(true, Ordering::Release);
    let mut port = Sink {
        written: 0,
        limit: 1,
        failure: None,
        bad_count: false,
    };
    assert_eq!(
        super::super::io::exact_write(&mut port, source.as_bytes(), &record)
            .err()
            .unwrap()
            .kind(),
        ErrorKind::Cancelled
    );
    assert_eq!(port.written, 0);
}
