use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const FLOOR: usize = 43;
const WORK: usize = 16_000_000;
const STORAGE: usize = 16 << 20;

fn c(operation: u32) -> Coordinate {
    Coordinate {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(2),
            block: 3,
        },
        operation,
    }
}
fn append(out: &mut Vec<u8>, value: Coordinate) {
    for word in [value.block.function.0, value.block.block, value.operation] {
        out.extend_from_slice(&word.to_le_bytes());
    }
}
fn wire(rows: &[Row], retained: &[Retained]) -> Vec<u8> {
    let extent = 384 + 24 * (rows.len() + retained.len());
    let mut out = vec![0; 384];
    out[..16].copy_from_slice(b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    // Deliberately not a semantically valid P6: decoding creates inert rows only.
    for (offset, count) in [
        (352, rows.len()),
        (360, retained.len()),
        (368, 1),
        (376, extent),
    ] {
        out[offset..offset + 8].copy_from_slice(&(count as u64).to_le_bytes());
    }
    for row in rows {
        append(&mut out, row.anchor);
        append(&mut out, row.removed);
    }
    for row in retained {
        append(&mut out, row.input);
        append(&mut out, row.output);
    }
    out
}
fn fixture() -> Vec<u8> {
    wire(
        &[
            Row {
                anchor: c(0),
                removed: c(1),
            },
            Row {
                anchor: c(1),
                removed: c(2),
            },
        ],
        &[
            Retained {
                input: c(0),
                output: c(0),
            },
            Retained {
                input: c(3),
                output: c(1),
            },
        ],
    )
}
fn reencode(rows: &DecodedCanonicalPolicy7RowsV1<'_>) -> Vec<u8> {
    let claims = rows.claims();
    let mut bytes = claims.execution_record[..384].to_vec();
    for row in claims.deletion_rows {
        append(&mut bytes, row.anchor);
        append(&mut bytes, row.removed);
    }
    for row in claims.retained_operations {
        append(&mut bytes, row.input);
        append(&mut bytes, row.output);
    }
    bytes
}
fn run(
    bytes: &[u8],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<usize, Error>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = decode_canonical_policy7_rows_v1(bytes, &mut budget).map(|rows| {
        assert_eq!(rows.wire.as_ptr(), bytes.as_ptr());
        assert_eq!(rows.wire.len(), bytes.len());
        assert_eq!(reencode(&rows), bytes);
        assert!(!rows.grants_authority());
        let retained = rows.storage().retained_storage();
        assert_eq!(
            retained,
            std::mem::size_of_val(&rows)
                + rows.deletions.capacity() * size_of::<Row>()
                + rows.retained.capacity() * size_of::<Retained>()
        );
        assert_eq!(budget.storage(), FLOOR);
        budget.reserve_storage(retained).unwrap();
        drop(rows);
        budget.release_storage(retained).unwrap();
        retained
    });
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn exact_owned_rows_full_bytes_noop_and_determinism() {
    for bytes in [fixture(), wire(&[], &[])] {
        let first = run(&bytes, WORK, STORAGE);
        let storage = first.0.unwrap();
        let second = run(&bytes, WORK, STORAGE);
        assert_eq!(second.0.unwrap(), storage);
        assert_eq!((first.1, first.2), (second.1, second.2));
        assert_eq!(first.1, 8 + bytes.len() + (bytes.len() - 384) / 24);
        assert_eq!(first.2, FLOOR + storage);
    }
}

#[test]
fn every_byte_is_either_rejected_or_retained_exactly_without_authority() {
    let original = fixture();
    for offset in 0..original.len() {
        let mut bytes = original.clone();
        bytes[offset] ^= 1;
        let decoded = run(&bytes, WORK, STORAGE);
        if offset < 16 || (352..384).contains(&offset) {
            assert!(matches!(
                decoded.0,
                Err(Error::Record | Error::Resource(Resource::Arithmetic))
            ));
        } else if offset < 352 {
            // Nested P6 and I/J identities are claims, not decoder authority.
            decoded.0.unwrap();
        }
    }
    for end in 0..original.len() {
        assert!(run(&original[..end], WORK, STORAGE).0.is_err());
    }
    let mut trailing = original;
    trailing.push(0);
    assert!(matches!(
        run(&trailing, WORK, STORAGE).0,
        Err(Error::Record)
    ));
}

#[test]
fn duplicate_reordered_and_invalid_rows_are_refused_after_allocations_drop() {
    let a = Row {
        anchor: c(0),
        removed: c(1),
    };
    let b = Row {
        anchor: c(1),
        removed: c(2),
    };
    let x = Retained {
        input: c(0),
        output: c(0),
    };
    let y = Retained {
        input: c(3),
        output: c(1),
    };
    for bytes in [
        wire(&[a, a], &[x, y]),
        wire(&[b, a], &[x, y]),
        wire(
            &[Row {
                anchor: c(1),
                removed: c(1),
            }],
            &[x],
        ),
        wire(
            &[Row {
                anchor: c(2),
                removed: c(1),
            }],
            &[x],
        ),
        wire(&[a, b], &[x, x]),
        wire(&[a, b], &[y, x]),
        wire(
            &[a, b],
            &[
                x,
                Retained {
                    input: c(3),
                    output: c(0),
                },
            ],
        ),
        wire(
            &[a, b],
            &[
                x,
                Retained {
                    input: c(0),
                    output: c(1),
                },
            ],
        ),
    ] {
        let result = run(&bytes, WORK, STORAGE);
        assert!(matches!(result.0, Err(Error::Rows)));
        assert!(result.2 > FLOOR);
    }
}

#[test]
fn tiny_oversized_overflow_and_false_extents_do_not_allocate() {
    for bytes in [
        vec![],
        vec![0; 383],
        vec![0; MAX_POLICY7_EXECUTION_RECORD_BYTES_V1 + 1],
    ] {
        let result = run(&bytes, WORK, STORAGE);
        assert!(matches!(result.0, Err(Error::Record)));
        assert_eq!((result.1, result.2), (1, FLOOR));
    }
    for (left, right) in [(u64::MAX, 1), (u64::MAX, 0), (0, u64::MAX), (0, 1)] {
        let mut bytes = wire(&[], &[]);
        bytes[352..360].copy_from_slice(&left.to_le_bytes());
        bytes[360..368].copy_from_slice(&right.to_le_bytes());
        let result = run(&bytes, WORK, STORAGE);
        assert!(matches!(
            result.0,
            Err(Error::Record | Error::Resource(Resource::Arithmetic))
        ));
        assert_eq!((result.1, result.2), (388, FLOOR));
    }
    let mut bytes = wire(&[], &[]);
    bytes[376..384].copy_from_slice(&u64::MAX.to_le_bytes());
    let result = run(&bytes, WORK, STORAGE);
    assert!(matches!(result.0, Err(Error::Record)));
    assert_eq!((result.1, result.2), (388, FLOOR));
    assert!(matches!(
        run(&[], 0, STORAGE).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(&bytes, 387, STORAGE).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
}

#[test]
fn exact_work_and_actual_capacity_limits_restore_the_incoming_floor() {
    let bytes = fixture();
    let result = run(&bytes, WORK, STORAGE);
    result.0.unwrap();
    let exact = run(&bytes, result.1, result.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (result.1, result.2));
    assert!(matches!(
        run(&bytes, result.1 - 1, result.2).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(&bytes, result.1, result.2 - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    // The first Vec exists when the second allocation's prepayment fails.
    let first_capacity = 2 * size_of::<Row>();
    let limit = FLOOR + size_of::<DecodedCanonicalPolicy7RowsV1<'_>>() + first_capacity;
    assert!(matches!(
        run(&bytes, WORK, limit).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn bounded_largest_record_and_linear_work() {
    for count in [1, 1024, (MAX_POLICY7_EXECUTION_RECORD_BYTES_V1 - 384) / 24] {
        let retained: Vec<_> = (0..count)
            .map(|n| Retained {
                input: c(u32::try_from(n).unwrap()),
                output: c(u32::try_from(n).unwrap()),
            })
            .collect();
        let bytes = wire(&[], &retained);
        assert!(bytes.len() <= MAX_POLICY7_EXECUTION_RECORD_BYTES_V1);
        let result = run(&bytes, WORK, STORAGE);
        result.0.unwrap();
        assert_eq!(result.1, 8 + bytes.len() + count);
    }
}
