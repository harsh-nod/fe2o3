use super::*;

#[test]
fn native_reads_never_retry_interruptions_or_short_transfers() {
    for kind in [io::ErrorKind::Interrupted, io::ErrorKind::WouldBlock] {
        let mut calls = 0;
        let error = read_exact_with(4, |_, _| {
            calls += 1;
            Err(io::Error::from(kind))
        })
        .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(calls, 1);
    }
    let mut calls = 0;
    assert_eq!(
        read_exact_with(4, |_, _| {
            calls += 1;
            Ok(3)
        })
        .unwrap_err()
        .kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(calls, 1);
}

#[test]
fn native_reader_has_exact_chunk_and_eof_attempts() {
    let length = 2 * 64 * 1024 + 7;
    let mut calls = Vec::new();
    let bytes = read_exact_with(length, |bytes, offset| {
        calls.push((offset, bytes.len()));
        if offset == length as u64 {
            return Ok(0);
        }
        bytes.fill(0x73);
        Ok(bytes.len())
    })
    .unwrap();
    assert_eq!(bytes, vec![0x73; length]);
    assert_eq!(
        calls,
        [(0, 65536), (65536, 65536), (131072, 7), (length as u64, 1)]
    );
    assert_eq!(
        read_exact_with(0, |_, _| Ok(1)).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    let mut calls = 0;
    assert_eq!(
        read_exact_with(1, |_, _| {
            calls += 1;
            if calls == 1 {
                Ok(1)
            } else {
                Err(io::Error::from(io::ErrorKind::Interrupted))
            }
        })
        .unwrap_err()
        .kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(calls, 2);
}

#[test]
fn limits_reject_empty_oversized_and_overflowing_inputs() {
    for (readiness, artifact) in [(0, 1), (1, 0), (usize::MAX, 1), (1, usize::MAX)] {
        assert!(NativeCurrentPublicationLimitsV1::new(readiness, artifact).is_err());
    }
    let limits = NativeCurrentPublicationLimitsV1::new(100, 200).unwrap();
    assert_eq!(limits.readiness_bytes(), 100);
    assert_eq!(limits.artifact_bytes(), 200);
    assert!(limits.quota().unwrap() > crate::MAX_ATTEMPT_BYTES);
}
