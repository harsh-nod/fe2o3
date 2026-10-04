use super::*;
use rustix::pipe::{PipeFlags, pipe_with};

const BYTES: usize = CHILD_NAMESPACE_REPORT_BYTES;

fn append(reader: &mut ProfileReportRead, bytes: &[u8]) -> Result<Option<()>, ReportError> {
    let mut calls = 0;
    let result = reader.observe(|out| {
        calls += 1;
        assert!(bytes.len() <= out.len());
        out[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    });
    assert_eq!(calls, 1);
    result
}

fn assert_terminal(reader: &mut ProfileReportRead) {
    assert_eq!(
        reader.observe(|_| panic!("terminal report state must not perform I/O")),
        Err(ReportError::State)
    );
}

#[test]
fn every_fragment_boundary_preserves_bytes_and_requires_separate_eof() {
    let bytes: [u8; BYTES] = std::array::from_fn(|index| index as u8);
    for split in 1..BYTES {
        let mut reader = ProfileReportRead::new();
        assert!(reader.report().is_err());
        assert_eq!(append(&mut reader, &bytes[..split]), Ok(None));
        assert!(reader.report().is_err());
        assert_eq!(append(&mut reader, &bytes[split..]), Ok(None));
        assert!(reader.report().is_err());
        assert_eq!(append(&mut reader, &[]), Ok(Some(())));
        assert_eq!(reader.report().unwrap(), bytes);
        assert_terminal(&mut reader);
        assert_eq!(reader.report().unwrap(), bytes);
    }
}

#[test]
fn one_byte_fragmentation_needs_exactly_one_more_attempt_than_the_frame_size() {
    let mut reader = ProfileReportRead::new();
    for index in 0..BYTES {
        assert_eq!(append(&mut reader, &[index as u8]), Ok(None));
        assert_eq!(reader.used, index + 1);
    }
    assert_eq!(append(&mut reader, &[]), Ok(Some(())));
    assert_eq!(reader.report().unwrap().len(), BYTES);
}

#[test]
fn every_truncated_frame_and_the_old_acknowledgement_reject() {
    for prefix in 0..BYTES {
        let mut reader = ProfileReportRead::new();
        if prefix != 0 {
            assert_eq!(append(&mut reader, &vec![0xa5; prefix]), Ok(None));
        }
        assert_eq!(append(&mut reader, &[]), Err(ReportError::Truncated));
        assert!(reader.report().is_err());
        assert_terminal(&mut reader);
    }
}

#[test]
fn trailing_sentinel_rejects_in_one_read_or_after_a_complete_frame() {
    for split in 0..=BYTES {
        let bytes = [0x71; BYTES + 1];
        let mut reader = ProfileReportRead::new();
        if split != 0 {
            assert_eq!(append(&mut reader, &bytes[..split]), Ok(None));
        }
        assert_eq!(
            append(&mut reader, &bytes[split..]),
            Err(ReportError::Trailing)
        );
        assert!(reader.report().is_err());
        assert_terminal(&mut reader);
    }
}

#[test]
fn transient_errors_keep_the_accepted_prefix_and_never_drain_another_read() {
    let mut reader = ProfileReportRead::new();
    assert_eq!(append(&mut reader, &[0x31; 7]), Ok(None));
    for errno in [Errno::AGAIN, Errno::INTR] {
        let mut calls = 0;
        assert_eq!(
            reader.observe(|_| {
                calls += 1;
                Err(errno)
            }),
            Ok(None)
        );
        assert_eq!(calls, 1);
        assert_eq!(reader.used, 7);
    }
    assert_eq!(append(&mut reader, &[0x32; BYTES - 7]), Ok(None));
    assert_eq!(append(&mut reader, &[]), Ok(Some(())));
    assert_eq!(&reader.report().unwrap()[..7], &[0x31; 7]);
    assert_eq!(&reader.report().unwrap()[7..], &[0x32; BYTES - 7]);
}

#[test]
fn permanent_errors_and_impossible_read_lengths_poison_the_reader() {
    let mut reader = ProfileReportRead::new();
    assert_eq!(
        reader.observe(|_| Err(Errno::BADF)),
        Err(ReportError::Io(Errno::BADF))
    );
    assert_terminal(&mut reader);
    let mut reader = ProfileReportRead::new();
    assert_eq!(
        reader.observe(|out| Ok(out.len() + 1)),
        Err(ReportError::State)
    );
    assert_terminal(&mut reader);
}

#[test]
fn live_writer_alias_prevents_admission_until_every_writer_closes() {
    let (reader_fd, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    let alias = rustix::io::dup(&writer).unwrap();
    let bytes = [0x41; BYTES];
    assert_eq!(rustix::io::write(&writer, &bytes).unwrap(), BYTES);
    drop(writer);
    let mut reader = ProfileReportRead::new();
    assert_eq!(
        reader.observe(|out| rustix::io::read(&reader_fd, out)),
        Ok(None)
    );
    for _ in 0..3 {
        assert_eq!(
            reader.observe(|out| rustix::io::read(&reader_fd, out)),
            Ok(None)
        );
        assert!(reader.report().is_err());
    }
    drop(alias);
    assert_eq!(
        reader.observe(|out| rustix::io::read(&reader_fd, out)),
        Ok(Some(()))
    );
    assert_eq!(reader.report().unwrap(), bytes);
}
