//! Pure streaming/allocation controls; not admitted compiler source evidence.
use super::*;
use std::io::Cursor;
struct Short<'a> {
    input: &'a [u8],
    reads: usize,
    delivered: usize,
}
impl Read for Short<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        let n = self.input.len().min(output.len()).min(3);
        output[..n].copy_from_slice(&self.input[..n]);
        self.input = &self.input[n..];
        self.delivered += n;
        Ok(n)
    }
}
#[test]
fn bounded_original_exact_capacity_empty_short_and_maximum() {
    for bytes in [b"".as_slice(), b"abc".as_slice(), b"abcdefg".as_slice()] {
        let mut input = Short {
            input: bytes,
            reads: 0,
            delivered: 0,
        };
        let observed = read_original(&mut input, bytes.len(), 7).unwrap();
        assert_eq!(observed, bytes);
        assert_eq!(observed.capacity(), bytes.len() + 1);
        assert_eq!(input.delivered, bytes.len());
        assert!(input.reads >= 1);
    }
    let data = vec![7; MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1];
    let observed = read_original(&mut Cursor::new(&data), data.len(), data.len()).unwrap();
    assert_eq!(observed.capacity(), data.len() + 1);
}
#[test]
fn bounded_original_refuses_invalid_limit_and_length_before_reads() {
    struct NoRead;
    impl Read for NoRead {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("inadmissible read");
        }
    }
    assert!(read_original(&mut NoRead, 0, 0).is_err());
    assert!(read_original(&mut NoRead, 1, MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + 1).is_err());
    assert!(read_original(&mut NoRead, 8, 7).is_err());
    assert!(read_original(&mut NoRead, usize::MAX, 7).is_err());
}
#[test]
fn bounded_original_refuses_truncated_and_extended_without_replacement() {
    assert!(read_original(&mut Cursor::new(b"ab"), 3, 3).is_err());
    let mut longer = Short {
        input: b"abcdefghijk",
        reads: 0,
        delivered: 0,
    };
    assert!(read_original(&mut longer, 3, 3).is_err());
    assert_eq!(longer.delivered, 4, "one rejection byte only");
}
#[test]
fn bounded_compare_short_reads_reaches_real_eof() {
    let mut input = Short {
        input: b"abcdefg",
        reads: 0,
        delivered: 0,
    };
    compare_stream(&mut input, b"abcdefg").unwrap();
    assert_eq!(input.delivered, 7);
    assert!(input.reads >= 4);
    compare_stream(&mut Cursor::new(b""), b"").unwrap();
}
#[test]
fn bounded_compare_refuses_changed_truncated_extended_and_chunk_boundary() {
    for actual in [b"abX".as_slice(), b"ab".as_slice(), b"abcd".as_slice()] {
        assert!(compare_stream(&mut Cursor::new(actual), b"abc").is_err());
    }
    let data = vec![9; BOUNDED_SOURCE_IO_CHUNK_BYTES_V1 + 1];
    compare_stream(&mut Cursor::new(&data), &data).unwrap();
    let mut changed = data.clone();
    changed[BOUNDED_SOURCE_IO_CHUNK_BYTES_V1] = 8;
    assert!(compare_stream(&mut Cursor::new(changed), &data).is_err());
}
#[test]
fn bounded_compare_never_reads_more_than_expected_plus_one() {
    let mut input = Short {
        input: b"abcdefghijklmnop",
        reads: 0,
        delivered: 0,
    };
    assert!(compare_stream(&mut input, b"abcde").is_err());
    assert_eq!(input.delivered, 6);
}
#[test]
fn bounded_streaming_read_failure_is_not_eof_success() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("pure injected failure"))
        }
    }
    assert!(read_original(&mut Broken, 0, 1).is_err());
    assert!(compare_stream(&mut Broken, b"").is_err());
}

#[test]
fn bounded_owner_charge_uses_actual_capacity_and_one_header() {
    assert_eq!(
        payload_storage(3, 8, 17, 7).unwrap(),
        size_of::<RetainedSource>() + 8 + 17
    );
    assert_eq!(
        payload_storage(0, 1, 0, 1).unwrap(),
        size_of::<RetainedSource>() + 1
    );
    assert!(payload_storage(8, 7, 0, 8).is_err());
    assert!(payload_storage(0, 9, 0, 7).is_err());
    assert!(payload_storage(0, 1, MAX_SOURCE_EDIT_PATH_BYTES_V1 + 1, 7).is_err());
    assert!(payload_storage(0, usize::MAX, 0, 7).is_err());
}
