use super::{
    DistributedPublicationContractErrorV1 as E, Reader, Writer, codec_fields::bytes_equal,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn reference_take<'a>(bytes: &'a [u8], offset: &mut usize, count: usize) -> Result<&'a [u8], E> {
    let end = offset.checked_add(count).ok_or(E::WrongLength)?;
    let value = bytes.get(*offset..end).ok_or(E::WrongLength)?;
    *offset = end;
    Ok(value)
}

fn reference_read_header(bytes: &[u8], offset: &mut usize, domain: &[u8]) -> Result<(), E> {
    if reference_take(bytes, offset, domain.len())? != domain {
        return Err(E::WrongDomain);
    }
    let schema = u16::from_le_bytes(reference_take(bytes, offset, 2)?.try_into().unwrap());
    if schema != super::DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 {
        return Err(E::WrongSchema);
    }
    if reference_take(bytes, offset, 2)? != [0, 0] {
        return Err(E::NonzeroReserved);
    }
    Ok(())
}

#[test]
fn byte_equality_matches_standard_rust_for_every_position_and_byte() {
    let original = core::array::from_fn::<_, 129, _>(|i| (i * 71 + 29) as u8);
    for length in 0..=original.len() {
        for other_length in 0..=original.len() {
            let left = &original[..length];
            let right = &original[..other_length];
            assert_eq!(bytes_equal(left, right), left == right);
        }
        for index in 0..length {
            let mut changed = original;
            changed[index] ^= 0x81;
            assert!(!bytes_equal(&original[..length], &changed[..length]));
            assert!(!bytes_equal(&changed[..length], &original[..length]));
            assert_eq!(original[index] ^ changed[index], 0x81);
        }
    }
    for left in 0..=u8::MAX {
        for right in 0..=u8::MAX {
            assert_eq!(bytes_equal(&[left], &[right]), left == right);
            assert_eq!(bytes_equal(&[0x73, left], &[0x73, right]), left == right);
        }
    }
}

#[test]
fn header_reads_match_original_for_arbitrary_domains_and_each_mismatch() {
    let domain = core::array::from_fn::<_, 129, _>(|i| (i * 97 + 13) as u8);
    let mut storage = [0xa5; 137];
    for domain_length in 0..=domain.len() {
        let domain = &domain[..domain_length];
        storage.fill(0xa5);
        storage[2..2 + domain_length].copy_from_slice(domain);
        storage[2 + domain_length..6 + domain_length].copy_from_slice(&[1, 0, 0, 0]);
        let original = storage;
        for length in 0..=storage.len() {
            for offset in [0, 1, 2, length, length + 1, usize::MAX - 1, usize::MAX] {
                let mut expected_offset = offset;
                let expected =
                    reference_read_header(&storage[..length], &mut expected_offset, domain);
                let mut reader = Reader {
                    bytes: &storage[..length],
                    offset,
                };
                assert_eq!(reader.header(domain), expected);
                assert_eq!(reader.offset, expected_offset);
            }
        }
        for index in 0..domain_length + 4 {
            storage[2 + index] ^= 0x81;
            for length in domain_length + 2..=domain_length + 6 {
                let mut expected_offset = 2;
                let expected =
                    reference_read_header(&storage[..length], &mut expected_offset, domain);
                let mut reader = Reader {
                    bytes: &storage[..length],
                    offset: 2,
                };
                assert_eq!(reader.header(domain), expected);
                assert_eq!(reader.offset, expected_offset);
            }
            storage[2 + index] ^= 0x81;
        }
        assert_eq!(storage, original);
    }
}

fn reference_put(bytes: &mut [u8], offset: &mut usize, value: &[u8]) {
    bytes[*offset..*offset + value.len()].copy_from_slice(value);
    *offset += value.len();
}

fn reference_write_header(bytes: &mut [u8], offset: &mut usize, domain: &[u8]) {
    reference_put(bytes, offset, domain);
    reference_put(
        bytes,
        offset,
        &super::DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1.to_le_bytes(),
    );
    reference_put(bytes, offset, &[0; 2]);
}

#[test]
fn header_writes_match_original_partial_panics_and_canaries() {
    let original = core::array::from_fn::<_, 17, _>(|i| (i * 29 + 11) as u8);
    let domain = [0xe9, 0x37, 0x81, 0x56, 0x0a];
    for domain_length in 0..=domain.len() {
        for length in 0..=original.len() {
            for offset in (0..=length + 1).chain([usize::MAX - 1, usize::MAX]) {
                let mut expected_bytes = original;
                let mut expected_offset = offset;
                let expected = catch_unwind(AssertUnwindSafe(|| {
                    reference_write_header(
                        &mut expected_bytes[..length],
                        &mut expected_offset,
                        &domain[..domain_length],
                    );
                }));
                let mut actual_bytes = original;
                let (actual, actual_offset) = {
                    let mut writer = Writer {
                        bytes: &mut actual_bytes[..length],
                        offset,
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        writer.header(&domain[..domain_length]);
                    }));
                    (result, writer.offset)
                };
                assert_eq!(actual.is_err(), expected.is_err());
                assert_eq!(actual_offset, expected_offset);
                assert_eq!(actual_bytes, expected_bytes);
                assert_eq!(&actual_bytes[length..], &original[length..]);
            }
        }
    }
}

#[test]
fn header_panics_preserve_each_successful_put_without_eager_preflight() {
    let domain = [0x31, 0x84, 0xd7];
    for (length, expected_cursor, expected_prefix) in [
        (3, 1, &[][..]),
        (4, 4, &domain[..]),
        (5, 4, &domain[..]),
        (6, 6, &[0x31, 0x84, 0xd7, 1, 0][..]),
        (7, 6, &[0x31, 0x84, 0xd7, 1, 0][..]),
    ] {
        let mut storage = [0xa5; 10];
        let mut writer = Writer {
            bytes: &mut storage[..length],
            offset: 1,
        };
        assert!(catch_unwind(AssertUnwindSafe(|| writer.header(&domain))).is_err());
        assert_eq!(writer.offset, expected_cursor);
        assert_eq!(&storage[1..expected_cursor], expected_prefix);
        assert_eq!(storage[0], 0xa5);
        assert!(storage[expected_cursor..].iter().all(|byte| *byte == 0xa5));
    }
}

#[test]
fn u64_reads_match_standard_endian_at_all_cursor_boundaries() {
    let storage = core::array::from_fn::<_, 25, _>(|i| (i * 71 + 3) as u8);
    for length in 0..=storage.len() {
        for offset in (0..=length + 1).chain([usize::MAX - 7, usize::MAX]) {
            let expected = offset
                .checked_add(8)
                .and_then(|end| storage[..length].get(offset..end))
                .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()));
            let mut reader = Reader {
                bytes: &storage[..length],
                offset,
            };
            assert_eq!(reader.u64(), expected.ok_or(E::WrongLength));
            assert_eq!(
                reader.offset,
                if expected.is_some() {
                    offset + 8
                } else {
                    offset
                }
            );
        }
    }
}

#[test]
fn u64_writes_match_original_panics_and_nonuniform_frames() {
    let original = core::array::from_fn::<_, 25, _>(|i| (i * 17 + 9) as u8);
    for value in [0, u64::MAX, 0x0123_4567_89ab_cdef, 0xa783_09ce_4512_6bfd] {
        for length in 0..=original.len() {
            for offset in (0..=length + 1).chain([usize::MAX - 7, usize::MAX]) {
                let mut expected_bytes = original;
                let mut expected_offset = offset;
                let expected = catch_unwind(AssertUnwindSafe(|| {
                    reference_put(
                        &mut expected_bytes[..length],
                        &mut expected_offset,
                        &value.to_le_bytes(),
                    );
                }));
                let mut actual_bytes = original;
                let (actual, actual_offset) = {
                    let mut writer = Writer {
                        bytes: &mut actual_bytes[..length],
                        offset,
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| writer.u64(value)));
                    (result, writer.offset)
                };
                assert_eq!(actual.is_err(), expected.is_err());
                assert_eq!(actual_offset, expected_offset);
                assert_eq!(actual_bytes, expected_bytes);
            }
        }
    }
}

#[test]
fn header_read_errors_follow_consumed_fields_not_future_errors() {
    let domain = [0x21, 0x34, 0x87];
    let mut storage = [0xa5, 0x21, 0x34, 0x87, 1, 0, 0, 0, 0x5a];
    for (index, error, expected_cursor, length) in [
        (2, E::WrongDomain, 4, 4),
        (4, E::WrongSchema, 6, 6),
        (6, E::WrongLength, 6, 7),
        (6, E::NonzeroReserved, 8, 8),
        (7, E::NonzeroReserved, 8, 8),
    ] {
        storage[index] ^= 0xff;
        let mut reader = Reader {
            bytes: &storage[..length],
            offset: 1,
        };
        assert_eq!(reader.header(&domain), Err(error));
        assert_eq!(reader.offset, expected_cursor);
        storage[index] ^= 0xff;
    }
    let mut reader = Reader {
        bytes: &[],
        offset: 1,
    };
    assert_eq!(reader.header(&[]), Err(E::WrongLength));
    assert_eq!(reader.offset, 1);
}
