use super::{
    DistributedPublicationContractErrorV1 as E, Reader, Writer, codec_primitives as codec,
};

// The pre-refinement slice API remains an independent native differential oracle.
fn reference_take<'a>(bytes: &'a [u8], offset: &mut usize, count: usize) -> Result<&'a [u8], E> {
    let end = offset.checked_add(count).ok_or(E::WrongLength)?;
    let value = bytes.get(*offset..end).ok_or(E::WrongLength)?;
    *offset = end;
    Ok(value)
}

#[test]
fn take_matches_original_ranges_and_preserves_failure_cursor() {
    let storage = core::array::from_fn::<_, 17, _>(|i| (i * 13 + 5) as u8);
    for length in 0..=storage.len() {
        let bytes = &storage[..length];
        for offset in (0..=length + 1).chain([usize::MAX - 1, usize::MAX]) {
            for count in (0..=length + 1).chain([usize::MAX - 1, usize::MAX]) {
                let mut expected_cursor = offset;
                let expected = reference_take(bytes, &mut expected_cursor, count);
                let mut reader = Reader { bytes, offset };
                let actual = reader.take(count);
                assert_eq!(
                    actual, expected,
                    "length={length} offset={offset} count={count}"
                );
                assert_eq!(reader.offset, expected_cursor);
                if let Ok(value) = actual {
                    assert_eq!(value.len(), count);
                    assert_eq!(value.as_ptr(), bytes[offset..].as_ptr());
                } else {
                    assert_eq!(reader.offset, offset);
                }
            }
        }
    }
    let mut reader = Reader::new(&storage);
    assert_eq!(reader.take(5), Ok(&storage[..5]));
    assert_eq!(reader.take(usize::MAX), Err(E::WrongLength));
    assert_eq!(reader.offset, 5);
    assert_eq!(reader.take(13), Err(E::WrongLength));
    assert_eq!(reader.offset, 5);
    assert_eq!(reader.take(12), Ok(&storage[5..]));
    assert_eq!(reader.take(0), Ok(&storage[17..]));
    assert_eq!(reader.offset, 17);
}

fn check_fixed<const N: usize>() {
    let storage = core::array::from_fn::<_, 40, _>(|i| (i * 29 + 7) as u8);
    for length in 0..=storage.len() {
        let bytes = &storage[..length];
        for offset in (0..=length + 1).chain([usize::MAX]) {
            let mut expected_cursor = offset;
            let expected: Result<[u8; N], E> = reference_take(bytes, &mut expected_cursor, N)
                .map(|value| value.try_into().unwrap());
            let mut reader = Reader { bytes, offset };
            assert_eq!(reader.fixed::<N>(), expected);
            assert_eq!(reader.offset, expected_cursor);
        }
    }
}

#[test]
fn fixed_arrays_match_original_conversion_at_all_boundaries() {
    check_fixed::<0>();
    check_fixed::<2>();
    check_fixed::<4>();
    check_fixed::<8>();
    check_fixed::<32>();
}

#[test]
fn writes_preserve_every_byte_outside_the_exact_frame() {
    let original = core::array::from_fn::<_, 17, _>(|i| (i * 19 + 3) as u8);
    let value = core::array::from_fn::<_, 17, _>(|i| (i * 41 + 11) as u8);
    for length in 0..=original.len() {
        for offset in 0..=length {
            for count in 0..=length - offset {
                let mut storage = original;
                let mut writer = Writer {
                    bytes: &mut storage[..length],
                    offset,
                };
                writer.put(&value[..count]);
                assert_eq!(writer.offset, offset + count);
                let mut expected = original;
                expected[offset..offset + count].copy_from_slice(&value[..count]);
                assert_eq!(storage, expected);
            }
        }
    }
    let mut storage = [0xa5; 17];
    let mut writer = Writer::new(&mut storage);
    writer.put(&value[..5]);
    writer.put(&[]);
    writer.put(&value[5..]);
    assert_eq!(writer.offset, value.len());
    assert_eq!(storage, value);
}

#[test]
fn invalid_writes_match_original_panics_without_partial_mutation() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn reference_put(bytes: &mut [u8], offset: &mut usize, value: &[u8]) {
        bytes[*offset..*offset + value.len()].copy_from_slice(value);
        *offset += value.len();
    }

    let original = core::array::from_fn::<_, 17, _>(|i| (i * 19 + 3) as u8);
    let value = [0x5a; 17];
    for length in [0, 1, 8, 17] {
        for offset in [length, length + 1, usize::MAX - 1, usize::MAX] {
            for count in [0, 1, 2, 17] {
                if offset.checked_add(count).is_some_and(|end| end <= length) {
                    continue;
                }
                let mut expected_storage = original;
                let mut expected_cursor = offset;
                let expected = catch_unwind(AssertUnwindSafe(|| {
                    reference_put(
                        &mut expected_storage[..length],
                        &mut expected_cursor,
                        &value[..count],
                    );
                }));
                let mut storage = original;
                let (actual, cursor) = {
                    let mut writer = Writer {
                        bytes: &mut storage[..length],
                        offset,
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| writer.put(&value[..count])));
                    (result, writer.offset)
                };
                assert!(expected.is_err() && actual.is_err());
                assert_eq!(cursor, expected_cursor);
                assert_eq!(cursor, offset);
                assert_eq!(storage, expected_storage);
                assert_eq!(storage, original);
            }
        }
    }
}

#[test]
fn finish_accepts_only_the_exact_end_without_advancing() {
    let storage = [0x5a; 17];
    for length in 0..=storage.len() {
        for offset in (0..=length + 1).chain([usize::MAX]) {
            let reader = Reader {
                bytes: &storage[..length],
                offset,
            };
            assert_eq!(
                reader.finish(),
                if offset == length {
                    Ok(())
                } else {
                    Err(E::WrongLength)
                }
            );
            assert_eq!(reader.offset, offset);
        }
    }
}

#[test]
fn u16_endian_primitives_match_standard_rust_exhaustively() {
    for value in 0..=u16::MAX {
        let bytes = value.to_le_bytes();
        assert_eq!(codec::u16_le(value), bytes);
        assert_eq!(codec::u16_from_le(bytes), u16::from_le_bytes(bytes));
        assert_eq!(codec::u16_from_le(codec::u16_le(value)), value);
    }
}

fn check_u64(value: u64) {
    let bytes = value.to_le_bytes();
    assert_eq!(codec::u64_le(value), bytes);
    assert_eq!(codec::u64_from_le(bytes), u64::from_le_bytes(bytes));
    assert_eq!(codec::u64_from_le(codec::u64_le(value)), value);
    let mut storage = [0xa5; 10];
    let mut writer = Writer {
        bytes: &mut storage,
        offset: 1,
    };
    writer.u64(value);
    assert_eq!(writer.offset, 9);
    assert_eq!(&storage[1..9], &bytes);
    assert_eq!((storage[0], storage[9]), (0xa5, 0xa5));
    let mut reader = Reader {
        bytes: &storage,
        offset: 1,
    };
    assert_eq!(reader.u64(), Ok(value));
    assert_eq!(reader.offset, 9);
    for length in 0..8 {
        let mut reader = Reader::new(&bytes[..length]);
        assert_eq!(reader.u64(), Err(E::WrongLength));
        assert_eq!(reader.offset, 0);
    }
}

#[test]
fn u64_endian_primitives_match_standard_rust_and_real_wrappers() {
    check_u64(0);
    check_u64(u64::MAX);
    check_u64(0x0123_4567_89ab_cdef);
    for bit in 0..64 {
        check_u64(1u64 << bit);
        check_u64(!(1u64 << bit));
        check_u64((1u64 << bit) - 1);
    }
    let mut value = 0x4f3a_6279_c581_bd06u64;
    for _ in 0..4096 {
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        check_u64(value);
    }
}

fn reference_header(bytes: &[u8], offset: &mut usize, domain: &[u8]) -> Result<(), E> {
    if reference_take(bytes, offset, domain.len())? != domain {
        return Err(E::WrongDomain);
    }
    let schema: [u8; 2] = reference_take(bytes, offset, 2)?.try_into().unwrap();
    if u16::from_le_bytes(schema) != super::DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 {
        return Err(E::WrongSchema);
    }
    if reference_take(bytes, offset, 2)? != [0; 2] {
        return Err(E::NonzeroReserved);
    }
    Ok(())
}

#[test]
fn real_header_preserves_error_precedence_and_partial_cursor_progress() {
    let domain = [0x21, 0x34, 0x87];
    let valid = [0xa5, 0x21, 0x34, 0x87, 1, 0, 0, 0, 0x5a];
    for domain_length in 0..=domain.len() {
        for length in 0..=valid.len() {
            for bad_index in 0..=valid.len() {
                let mut storage = valid;
                if bad_index < storage.len() {
                    storage[bad_index] ^= 0xff;
                }
                for offset in (0..=length + 1).chain([usize::MAX]) {
                    let bytes = &storage[..length];
                    let mut expected_cursor = offset;
                    let expected =
                        reference_header(bytes, &mut expected_cursor, &domain[..domain_length]);
                    let mut reader = Reader { bytes, offset };
                    assert_eq!(reader.header(&domain[..domain_length]), expected);
                    assert_eq!(reader.offset, expected_cursor);
                }
            }
        }
    }
    let cases = [
        (1, Err(E::WrongLength), 1),
        (3, Err(E::WrongLength), 1),
        (4, Err(E::WrongLength), 4),
        (5, Err(E::WrongLength), 4),
        (6, Err(E::WrongLength), 6),
        (7, Err(E::WrongLength), 6),
        (8, Ok(()), 8),
    ];
    for (length, result, cursor) in cases {
        let mut reader = Reader {
            bytes: &valid[..length],
            offset: 1,
        };
        assert_eq!(reader.header(&domain), result);
        assert_eq!(reader.offset, cursor);
    }
    let mut invalid = valid;
    invalid[2] ^= 1;
    invalid[4] = 2;
    invalid[6] = 1;
    let mut reader = Reader {
        bytes: &invalid,
        offset: 1,
    };
    assert_eq!(reader.header(&domain), Err(E::WrongDomain));
    assert_eq!(reader.offset, 4);
    invalid[2] = valid[2];
    let mut reader = Reader {
        bytes: &invalid,
        offset: 1,
    };
    assert_eq!(reader.header(&domain), Err(E::WrongSchema));
    assert_eq!(reader.offset, 6);
    invalid[4] = valid[4];
    let mut reader = Reader {
        bytes: &invalid,
        offset: 1,
    };
    assert_eq!(reader.header(&domain), Err(E::NonzeroReserved));
    assert_eq!(reader.offset, 8);
}

#[test]
fn real_digest_and_header_writes_preserve_nonuniform_payloads_and_frames() {
    let payload = core::array::from_fn::<_, 32, _>(|i| (i * 37 + 9) as u8);
    let digest = super::IdentityDigestV1::from_untrusted_bytes(payload);
    let mut storage = [0xa5; 41];
    let mut writer = Writer {
        bytes: &mut storage,
        offset: 1,
    };
    writer.header(b"abc");
    super::codec_operation::write_digest(writer.bytes, &mut writer.offset, digest);
    assert_eq!(writer.offset, 40);
    assert_eq!(&storage[1..8], &[b'a', b'b', b'c', 1, 0, 0, 0]);
    assert_eq!(&storage[8..40], &payload);
    assert_eq!((storage[0], storage[40]), (0xa5, 0xa5));
    let mut reader = Reader {
        bytes: &storage,
        offset: 1,
    };
    assert_eq!(reader.header(b"abc"), Ok(()));
    assert_eq!(
        super::codec_operation::read_digest(reader.bytes, &mut reader.offset),
        Ok(digest)
    );
    assert_eq!(reader.offset, 40);
    for length in 0..32 {
        let mut reader = Reader::new(&payload[..length]);
        assert_eq!(
            super::codec_operation::read_digest(reader.bytes, &mut reader.offset),
            Err(E::WrongLength)
        );
        assert_eq!(reader.offset, 0);
    }
}
