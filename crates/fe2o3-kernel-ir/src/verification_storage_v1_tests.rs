use super::*;
use crate::{
    AccessMode, AddressSpace, CanonicalKernelIrWorkBudgetV1 as Work, FixedVectorTypeV12,
    VectorLayoutV12,
};
use std::mem::size_of;

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 128,
    edges: 512,
    containment_depth: 64,
    object_bytes: 4096,
};
const FLOOR: usize = 17;

fn field(offset: u64, layout: u32) -> StorageFieldV1 {
    StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(layout),
    }
}

fn scalar(ty: ScalarType, size: u64, alignment: u32) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size,
        alignment,
        kind: StorageLayoutKindV1::Scalar(ty),
    }
}

fn record(size: u64, alignment: u32, fields: Vec<StorageFieldV1>) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size,
        alignment,
        kind: StorageLayoutKindV1::Record(fields.into_boxed_slice()),
    }
}

fn pointer(pointee: u32) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(pointee),
            value_space: AddressSpace::Global,
            encoded_space: AddressSpace::Generic,
            access: AccessMode::ReadOnly,
            stored_bits: 64,
        }),
    }
}

fn check(
    rows: &[StorageLayoutV1],
    limits: StorageLayoutLimitsV1,
) -> Result<(), StorageLayoutErrorV1> {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = check_storage_layouts_v1(rows, limits, &mut budget);
    if let Ok(checked) = &result {
        assert_eq!(checked.rows(), rows);
    }
    assert_eq!(budget.storage(), FLOOR);
    result.map(|_| ())
}

fn rejects(rows: &[StorageLayoutV1], problem: StorageLayoutProblemV1) {
    assert!(
        matches!(check(rows, LIMITS), Err(StorageLayoutErrorV1::Invalid { problem: actual, .. }) if actual == problem)
    );
}

#[test]
fn storage_verifier_preserves_padding_packed_fields_and_overlapping_union_storage() {
    let mut rows = vec![
        scalar(ScalarType::U32, 4, 4),
        scalar(ScalarType::U64, 8, 8),
        record(16, 8, vec![field(0, 0), field(8, 1)]),
    ];
    check(&rows, LIMITS).unwrap();
    rows[2] = record(12, 1, vec![field(0, 0), field(4, 1)]);
    check(&rows, LIMITS).unwrap();
    rows[2] = record(8, 8, vec![field(0, 0), field(0, 1)]);
    rejects(&rows, StorageLayoutProblemV1::Overlap);
    rows[2].kind = StorageLayoutKindV1::Union(vec![field(0, 0), field(0, 1)].into_boxed_slice());
    check(&rows, LIMITS).unwrap();
    rows[2].kind = StorageLayoutKindV1::Union(vec![field(4, 0)].into_boxed_slice());
    rejects(&rows, StorageLayoutProblemV1::Size);
}

#[test]
fn storage_verifier_checks_ids_alignment_scalar_width_and_field_arithmetic() {
    rejects(
        &[scalar(ScalarType::U64, 8, 0)],
        StorageLayoutProblemV1::Alignment,
    );
    rejects(
        &[scalar(ScalarType::U64, 8, 3)],
        StorageLayoutProblemV1::Alignment,
    );
    rejects(
        &[scalar(ScalarType::U64, 4, 4)],
        StorageLayoutProblemV1::Scalar,
    );
    rejects(
        &[scalar(ScalarType::Bool, 0, 1)],
        StorageLayoutProblemV1::Scalar,
    );
    rejects(
        &[record(8, 8, vec![field(0, 1)])],
        StorageLayoutProblemV1::InvalidId,
    );
    rejects(
        &[
            scalar(ScalarType::U64, 8, 8),
            record(8, 8, vec![field(1, 0)]),
        ],
        StorageLayoutProblemV1::Size,
    );
    rejects(
        &[
            scalar(ScalarType::U64, 8, 8),
            record(8, 8, vec![field(u64::MAX, 0)]),
        ],
        StorageLayoutProblemV1::Size,
    );
    rejects(&[pointer(1)], StorageLayoutProblemV1::InvalidId);
    let mut rows = vec![scalar(ScalarType::U8, 1, 1), pointer(0)];
    if let StorageLayoutKindV1::Pointer(pointer) = &mut rows[1].kind {
        pointer.stored_bits = 48;
    }
    rejects(&rows, StorageLayoutProblemV1::Pointer);
}

#[test]
fn storage_verifier_array_stride_product_and_zero_size_are_explicit() {
    let mut rows = vec![
        scalar(ScalarType::U64, 8, 8),
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 3,
                stride: 8,
            },
        },
    ];
    check(&rows, LIMITS).unwrap();
    rows[1].kind = StorageLayoutKindV1::Array {
        element: StorageLayoutIdV1(0),
        length: 2,
        stride: 12,
    };
    rejects(&rows, StorageLayoutProblemV1::Size);
    rows[1].kind = StorageLayoutKindV1::Array {
        element: StorageLayoutIdV1(0),
        length: u64::MAX,
        stride: 8,
    };
    rejects(&rows, StorageLayoutProblemV1::Size);
    rows = vec![
        record(0, 1, vec![]),
        StorageLayoutV1 {
            size: 0,
            alignment: 1,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: u64::MAX,
                stride: 0,
            },
        },
        record(0, 1, vec![field(0, 0), field(0, 0)]),
    ];
    check(&rows, LIMITS).unwrap();
}

#[test]
fn storage_verifier_fixed_vectors_reuse_exact_v12_layout_validation() {
    for (vector, size, alignment) in [
        (
            FixedVectorTypeV12::new(ScalarType::U32, 4, VectorLayoutV12::Contiguous),
            16,
            16,
        ),
        (
            FixedVectorTypeV12::new(ScalarType::U32, 3, VectorLayoutV12::Contiguous),
            16,
            16,
        ),
        (
            FixedVectorTypeV12::new(
                ScalarType::U16,
                4,
                VectorLayoutV12::Interleaved { factor: 2 },
            ),
            8,
            8,
        ),
    ] {
        check(
            &[StorageLayoutV1 {
                size,
                alignment,
                kind: StorageLayoutKindV1::Vector(vector),
            }],
            LIMITS,
        )
        .unwrap();
    }
    for vector in [
        FixedVectorTypeV12::new(ScalarType::Bool, 4, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(ScalarType::Index, 4, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(ScalarType::U32, 1, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            VectorLayoutV12::Interleaved { factor: 3 },
        ),
    ] {
        rejects(
            &[StorageLayoutV1 {
                size: 16,
                alignment: 16,
                kind: StorageLayoutKindV1::Vector(vector),
            }],
            StorageLayoutProblemV1::Vector,
        );
    }
    rejects(
        &[StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Vector(FixedVectorTypeV12::new(
                ScalarType::U32,
                4,
                VectorLayoutV12::Contiguous,
            )),
        }],
        StorageLayoutProblemV1::Vector,
    );
}

#[test]
fn storage_verifier_pointer_recursion_is_not_containment_recursion() {
    let rows = vec![record(8, 8, vec![field(0, 1)]), pointer(0)];
    check(&rows, LIMITS).unwrap();
    rejects(
        &[record(0, 1, vec![field(0, 0)])],
        StorageLayoutProblemV1::ContainmentCycle,
    );
    rejects(
        &[
            record(0, 1, vec![field(0, 1)]),
            record(0, 1, vec![field(0, 0)]),
        ],
        StorageLayoutProblemV1::ContainmentCycle,
    );
}

#[test]
fn storage_verifier_deep_layouts_use_bounded_iterative_frames() {
    const COUNT: usize = 1024;
    let mut rows = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        let fields = if index + 1 == COUNT {
            vec![]
        } else {
            vec![field(0, u32::try_from(index + 1).unwrap())]
        };
        rows.push(record(0, 1, fields));
    }
    let limits = StorageLayoutLimitsV1 {
        rows: COUNT,
        edges: COUNT - 1,
        containment_depth: COUNT,
        object_bytes: 0,
    };
    check(&rows, limits).unwrap();
    assert!(matches!(
        check(
            &rows,
            StorageLayoutLimitsV1 {
                containment_depth: COUNT - 1,
                ..limits
            }
        ),
        Err(StorageLayoutErrorV1::Invalid {
            problem: StorageLayoutProblemV1::Depth,
            ..
        })
    ));
}

#[test]
fn storage_verifier_declared_limits_include_completed_child_depth_and_reference_edges() {
    // Child-before-parent ordering must not turn a depth-three graph into depth one.
    let rows = vec![
        record(0, 1, vec![]),
        record(0, 1, vec![field(0, 0)]),
        record(0, 1, vec![field(0, 1)]),
    ];
    let exact = StorageLayoutLimitsV1 {
        rows: 3,
        edges: 2,
        containment_depth: 3,
        object_bytes: 0,
    };
    check(&rows, exact).unwrap();
    for (limits, problem) in [
        (
            StorageLayoutLimitsV1 { rows: 2, ..exact },
            StorageLayoutProblemV1::Rows,
        ),
        (
            StorageLayoutLimitsV1 { edges: 1, ..exact },
            StorageLayoutProblemV1::Edges,
        ),
        (
            StorageLayoutLimitsV1 {
                containment_depth: 2,
                ..exact
            },
            StorageLayoutProblemV1::Depth,
        ),
    ] {
        assert!(
            matches!(check(&rows, limits), Err(StorageLayoutErrorV1::Invalid { problem: actual, .. }) if actual == problem)
        );
    }
    let rows = vec![record(8, 8, vec![field(0, 1)]), pointer(0)];
    assert!(matches!(
        check(&rows, StorageLayoutLimitsV1 { edges: 1, ..LIMITS }),
        Err(StorageLayoutErrorV1::Invalid {
            problem: StorageLayoutProblemV1::Edges,
            ..
        })
    ));
    assert!(matches!(
        check(
            &rows,
            StorageLayoutLimitsV1 {
                object_bytes: 7,
                ..LIMITS
            }
        ),
        Err(StorageLayoutErrorV1::Invalid {
            problem: StorageLayoutProblemV1::Size,
            ..
        })
    ));
}

fn slice_rows() -> Vec<StorageLayoutV1> {
    vec![
        scalar(ScalarType::U32, 4, 4),
        pointer(0),
        scalar(ScalarType::Index, 8, 8),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Slice {
                element: StorageLayoutIdV1(0),
                value_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                data: field(0, 1),
                length: field(8, 2),
            },
        },
    ]
}

#[test]
fn storage_verifier_slice_joins_exact_element_space_access_and_nonoverlapping_fields() {
    check(&slice_rows(), LIMITS).unwrap();
    for mutation in 0..5 {
        let mut rows = slice_rows();
        let StorageLayoutKindV1::Slice {
            value_space,
            access,
            length,
            element,
            ..
        } = &mut rows[3].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *value_space = AddressSpace::Private,
            1 => *access = AccessMode::ReadWrite,
            2 => length.offset = 0,
            3 => *element = StorageLayoutIdV1(2),
            _ => length.layout = StorageLayoutIdV1(0),
        }
        rejects(&rows, StorageLayoutProblemV1::Slice);
    }
}

fn variants_rows(niche: bool) -> Vec<StorageLayoutV1> {
    vec![
        scalar(
            if niche {
                ScalarType::Bool
            } else {
                ScalarType::U8
            },
            1,
            1,
        ),
        record(1, 1, vec![field(0, 0)]),
        StorageLayoutV1 {
            size: 1,
            alignment: 1,
            kind: StorageLayoutKindV1::Variants {
                encoding: if niche {
                    StorageVariantEncodingV1::Niche {
                        tag: field(0, 0),
                        untagged_variant: 0,
                        first_niche_variant: 1,
                        last_niche_variant: 1,
                        niche_start: 2,
                    }
                } else {
                    StorageVariantEncodingV1::Direct { tag: field(0, 0) }
                },
                variants: vec![
                    StorageVariantV1 {
                        discriminant: if niche { 0 } else { u128::MAX },
                        direct_tag_bits: (!niche).then_some(255),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(1),
                    },
                    StorageVariantV1 {
                        discriminant: if niche { 1 } else { 42 },
                        direct_tag_bits: (!niche).then_some(42),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(1),
                    },
                ]
                .into_boxed_slice(),
            },
        },
    ]
}

#[test]
fn storage_verifier_variant_physical_encoding_does_not_certify_value_validity() {
    // Bool's physical byte admits niche bit pattern 2 structurally, but this is
    // not proof that pattern 2 is a valid ordinary Bool or that a variant is live.
    check(&variants_rows(true), LIMITS).unwrap();
    check(&variants_rows(false), LIMITS).unwrap();
    for mutation in 0..4 {
        let mut rows = variants_rows(false);
        let StorageLayoutKindV1::Variants { variants, .. } = &mut rows[2].kind else {
            unreachable!()
        };
        match mutation {
            0 => variants[1].direct_tag_bits = Some(256),
            1 => variants[1].direct_tag_bits = Some(255),
            2 => variants[1].discriminant = u128::MAX,
            _ => variants[1].direct_tag_bits = None,
        }
        rejects(&rows, StorageLayoutProblemV1::Variant);
    }
    for mutation in 0..4 {
        let mut rows = variants_rows(true);
        let StorageLayoutKindV1::Variants { encoding, variants } = &mut rows[2].kind else {
            unreachable!()
        };
        let StorageVariantEncodingV1::Niche {
            untagged_variant,
            last_niche_variant,
            niche_start,
            ..
        } = encoding
        else {
            unreachable!()
        };
        match mutation {
            0 => *untagged_variant = 1,
            1 => *last_niche_variant = 2,
            2 => *niche_start = 256,
            _ => variants[1].direct_tag_bits = Some(2),
        }
        rejects(&rows, StorageLayoutProblemV1::Variant);
    }
}

#[test]
fn storage_verifier_niche_coverage_exempts_only_declared_uninhabited_variants() {
    for uninhabited in [false, true] {
        let mut rows = variants_rows(true);
        let StorageLayoutKindV1::Variants { variants, .. } = &mut rows[2].kind else {
            unreachable!()
        };
        *variants = vec![
            StorageVariantV1 {
                discriminant: 0,
                direct_tag_bits: None,
                uninhabited: false,
                layout: StorageLayoutIdV1(1),
            },
            StorageVariantV1 {
                discriminant: 1,
                direct_tag_bits: None,
                uninhabited: false,
                layout: StorageLayoutIdV1(1),
            },
            StorageVariantV1 {
                discriminant: 2,
                direct_tag_bits: None,
                uninhabited,
                layout: StorageLayoutIdV1(1),
            },
        ]
        .into_boxed_slice();
        if uninhabited {
            check(&rows, LIMITS).unwrap();
        } else {
            rejects(&rows, StorageLayoutProblemV1::Variant);
        }
    }
}

#[test]
fn storage_verifier_record_sort_is_prepaid_n_log_n_and_preserves_source_order() {
    const N: usize = 64;
    const LOG: usize = 6;
    const SORT: usize = 8 * N * LOG;
    const BEFORE_SORT: usize = 11 + 5 * N;
    const COMPLETE: usize = 21 + 7 * N + SORT;
    let fields = (0..N)
        .rev()
        .map(|offset| field(offset as u64, 0))
        .collect::<Vec<_>>();
    let rows = [
        scalar(ScalarType::U8, 1, 1),
        record(N as u64, 1, fields.clone()),
    ];
    for limit in [BEFORE_SORT + SORT - 1, COMPLETE - 1, COMPLETE] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result = check_storage_layouts_v1(&rows, LIMITS, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        let StorageLayoutKindV1::Record(unchanged) = &rows[1].kind else {
            unreachable!()
        };
        assert_eq!(unchanged.as_ref(), fields.as_slice());
        if limit == COMPLETE {
            result.unwrap();
            assert_eq!(budget.work(), COMPLETE);
        } else {
            let (accepted, denied) = if limit == COMPLETE - 1 {
                (COMPLETE - 1, COMPLETE)
            } else {
                (BEFORE_SORT, BEFORE_SORT + SORT)
            };
            assert_eq!(budget.work(), accepted);
            assert!(
                matches!(result, Err(StorageLayoutErrorV1::Resource(ResourceError::Work(error))) if error.actual() == denied)
            );
        }
    }
}

#[test]
fn storage_verifier_forward_tag_width_is_checked_before_bit_arithmetic() {
    for size in [0, 17, 32] {
        let rows = vec![
            StorageLayoutV1 {
                size: 32,
                alignment: 1,
                kind: StorageLayoutKindV1::Variants {
                    encoding: StorageVariantEncodingV1::Direct { tag: field(0, 1) },
                    variants: vec![StorageVariantV1 {
                        discriminant: 0,
                        direct_tag_bits: Some(0),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(2),
                    }]
                    .into_boxed_slice(),
                },
            },
            scalar(ScalarType::Index, size, 1),
            record(0, 1, vec![]),
        ];
        rejects(&rows, StorageLayoutProblemV1::Variant);
    }
}

#[test]
fn storage_verifier_positional_ids_resolve_only_against_their_borrowed_table() {
    let a = [scalar(ScalarType::U32, 4, 4)];
    let b = [scalar(ScalarType::U64, 8, 8)];
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 100_000);
    let checked_a = check_storage_layouts_v1(&a, LIMITS, &mut budget).unwrap();
    let checked_b = check_storage_layouts_v1(&b, LIMITS, &mut budget).unwrap();
    assert_eq!(checked_a.row(StorageLayoutIdV1(0)).unwrap().size, 4);
    assert_eq!(checked_b.row(StorageLayoutIdV1(0)).unwrap().size, 8);
    assert_eq!(checked_a.row(StorageLayoutIdV1(1)), None);
    assert_eq!(budget.storage(), 0);
}

// Independently listed simultaneous logical header premises, not receipt-derived
// totals and not a portable assertion about optimized machine stack slots.
fn expected_headers() -> usize {
    size_of::<Scratch<'_, '_>>()
        + 2 * size_of::<Result<Scratch<'_, '_>, StorageLayoutErrorV1>>()
        + 2 * size_of::<Vec<u8>>()
        + 2 * size_of::<Vec<Frame>>()
        + 2 * size_of::<Vec<usize>>()
        + 2 * size_of::<Vec<(u64, u64)>>()
        + 2 * size_of::<Vec<u128>>()
        + 2 * size_of::<Result<Vec<u8>, StorageLayoutErrorV1>>()
            .max(size_of::<Result<Vec<Frame>, StorageLayoutErrorV1>>())
            .max(size_of::<Result<Vec<usize>, StorageLayoutErrorV1>>())
            .max(size_of::<Result<Vec<(u64, u64)>, StorageLayoutErrorV1>>())
            .max(size_of::<Result<Vec<u128>, StorageLayoutErrorV1>>())
        + 2 * size_of::<Result<(), StorageLayoutErrorV1>>()
        + 2 * size_of::<Frame>()
        + 2 * size_of::<Result<(), ResourceError>>()
        + 2 * size_of::<Result<StructurallyCheckedStorageLayoutsV1<'_>, StorageLayoutErrorV1>>()
}

#[test]
fn storage_verifier_single_row_host_capacity_premises_are_explicit() {
    let mut colors = Vec::<u8>::new();
    let mut frames = Vec::<Frame>::new();
    let mut heights = Vec::<usize>::new();
    colors.try_reserve_exact(1).unwrap();
    frames.try_reserve_exact(1).unwrap();
    heights.try_reserve_exact(1).unwrap();
    assert_eq!(
        (colors.capacity(), frames.capacity(), heights.capacity()),
        (1, 1, 1)
    );
    assert_eq!(headers().unwrap(), expected_headers());
}

#[test]
fn storage_verifier_single_scalar_all_work_cutoffs_follow_declared_debits() {
    // Entry; graph; row header; ranges/keys allocation; scalar;
    // colors allocation/init; frames allocation;
    // heights allocation/init; root; leaf. The three-unit header debit is atomic.
    const DEBITS: [usize; 13] = [1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    assert_eq!(DEBITS.iter().sum::<usize>(), 15);
    for limit in 0..=15 {
        let mut prefix = 0;
        let mut denied = None;
        for debit in DEBITS {
            if prefix + debit > limit {
                denied = Some(prefix + debit);
                break;
            }
            prefix += debit;
        }
        let mut work = Work::new(limit);
        {
            let mut budget = Budget::new(&mut work, 100_000);
            budget.reserve_storage(FLOOR).unwrap();
            let rows = [scalar(ScalarType::U64, 8, 8)];
            let result = check_storage_layouts_v1(&rows, LIMITS, &mut budget);
            assert_eq!(budget.work(), prefix);
            assert_eq!(budget.storage(), FLOOR);
            match denied {
                Some(actual) => assert!(
                    matches!(result, Err(StorageLayoutErrorV1::Resource(ResourceError::Work(error))) if error.actual() == actual && error.limit() == limit)
                ),
                None => {
                    result.unwrap();
                }
            }
        }
        assert_eq!(work.failed_work(), denied);
    }
}

#[test]
fn storage_verifier_single_scalar_exact_and_one_short_storage_restore_owner_floor() {
    let peak =
        FLOOR + expected_headers() + size_of::<u8>() + size_of::<Frame>() + size_of::<usize>();
    for limit in [peak - 1, peak] {
        let mut work = Work::new(15);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let rows = [scalar(ScalarType::U64, 8, 8)];
        let result = check_storage_layouts_v1(&rows, LIMITS, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        if limit == peak {
            result.unwrap();
            assert_eq!(budget.work(), 15);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(
                matches!(result, Err(StorageLayoutErrorV1::Resource(ResourceError::Storage(error))) if error.actual() == peak && error.limit() == limit)
            );
            assert_eq!(budget.work(), 12);
            assert_eq!(budget.peak_storage(), peak - size_of::<usize>());
            assert_eq!(budget.failed_storage(), Some(peak));
        }
    }
}

#[test]
fn storage_verifier_prior_ledger_denials_are_not_reset_by_cleanup() {
    let peak =
        FLOOR + expected_headers() + size_of::<u8>() + size_of::<Frame>() + size_of::<usize>();
    let mut work = Work::new(15);
    assert!(work.charge_work(16).is_err());
    {
        let mut budget = Budget::new(&mut work, peak);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.reserve_storage(peak).is_err());
        let rows = [scalar(ScalarType::U64, 8, 8)];
        check_storage_layouts_v1(&rows, LIMITS, &mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), Some(FLOOR + peak));
    }
    assert_eq!(work.failed_work(), Some(16));
}

#[test]
fn storage_verifier_empty_table_and_header_refusal_preserve_exact_floor() {
    let header = expected_headers();
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, FLOOR + header);
    budget.reserve_storage(FLOOR).unwrap();
    check_storage_layouts_v1(
        &[],
        StorageLayoutLimitsV1 {
            rows: 0,
            edges: 0,
            containment_depth: 0,
            object_bytes: 0,
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (7, FLOOR, FLOOR + header)
    );
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, FLOOR + header - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(check_storage_layouts_v1(&[], LIMITS, &mut budget),
        Err(StorageLayoutErrorV1::Resource(ResourceError::Storage(error)))
            if error.actual() == FLOOR + header));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, FLOOR, FLOOR)
    );
}
