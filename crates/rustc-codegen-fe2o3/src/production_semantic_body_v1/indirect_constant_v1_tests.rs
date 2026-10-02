use super::*;
use rustc_abi::Integer;
use std::cell::Cell;

fn view<'a>(
    raw: &'a [u8],
    initialized: &'a [bool],
    remaining: &'a Cell<usize>,
    endian: Endian,
) -> Bytes<'a, impl Fn(usize) -> bool + 'a, impl FnMut(usize) -> Result<()> + 'a> {
    assert_eq!(raw.len(), initialized.len());
    Bytes {
        raw,
        initialized: move |index| initialized[index],
        charge: move |work| {
            let next = remaining.get().checked_sub(work).ok_or(
                ProductionSemanticBodyErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: work as u64,
                    maximum: remaining.get() as u64,
                },
            )?;
            remaining.set(next);
            Ok(())
        },
        endian,
        block: Some(7),
        statement: Some(9),
    }
}

#[test]
fn initialized_payload_and_padding_are_preserved_dead_bytes_are_canonical() {
    let raw = [3, 0xaa, 0xff, 0x12, 0x34, 0x56, 0x78, 0x99];
    let initialized = [true, true, false, true, true, false, false, true];
    let remaining = Cell::new(11);
    let mut value = view(&raw, &initialized, &remaining, Endian::Little);
    assert_eq!(value.integer(0, Size::from_bytes(1)).unwrap(), 3);
    assert_eq!(value.integer(3, Size::from_bytes(2)).unwrap(), 0x3412);
    assert_eq!(
        value.finish().unwrap(),
        [3, 0xaa, 0, 0x12, 0x34, 0, 0, 0x99]
    );
    assert_eq!(remaining.get(), 0);
}

#[test]
fn any_uninitialized_live_byte_or_tag_refuses_with_source_coordinates() {
    let raw = [0_u8; 16];
    for missing in 0..raw.len() {
        let mut initialized = [true; 16];
        initialized[missing] = false;
        let remaining = Cell::new(16);
        let error = view(&raw, &initialized, &remaining, Endian::Little)
            .integer(0, Size::from_bytes(16))
            .unwrap_err();
        assert!(matches!(error, ProductionSemanticBodyErrorV1::Unsupported {
            block: Some(7), statement: Some(9), ref construct,
        } if construct.contains("uninitialized live field or tag")));
    }
}

#[test]
fn scalar_validity_ranges_cannot_be_relaxed_by_padding_canonicalization() {
    let initialized = [true];
    for (raw, range, valid) in [
        ([0], WrappingRange { start: 0, end: 1 }, true),
        ([1], WrappingRange { start: 0, end: 1 }, true),
        ([2], WrappingRange { start: 0, end: 1 }, false),
        ([0], WrappingRange { start: 1, end: 255 }, false),
        ([255], WrappingRange { start: 1, end: 255 }, true),
        ([0], WrappingRange { start: 250, end: 2 }, true),
        ([3], WrappingRange { start: 250, end: 2 }, false),
    ] {
        let remaining = Cell::new(1);
        assert_eq!(
            view(&raw, &initialized, &remaining, Endian::Little)
                .valid_scalar(0, Size::from_bytes(1), range)
                .is_ok(),
            valid
        );
    }
}

#[test]
fn integer_projection_has_exact_endianness_width_and_allocation_bounds() {
    let raw = [0x12, 0x34];
    for (endian, expected) in [(Endian::Little, 0x3412), (Endian::Big, 0x1234)] {
        let remaining = Cell::new(2);
        assert_eq!(
            view(&raw, &[true; 2], &remaining, endian)
                .integer(0, Size::from_bytes(2))
                .unwrap(),
            expected
        );
    }
    for (offset, length) in [(0, 0), (0, 17), (1, 2), (usize::MAX, 1)] {
        let remaining = Cell::new(100);
        assert!(
            view(&raw, &[true; 2], &remaining, Endian::Little)
                .integer(offset, Size::from_bytes(length))
                .is_err()
        );
        assert_eq!(remaining.get(), 100);
    }
}

#[test]
fn exact_and_one_short_work_limits_use_the_supplied_ledger() {
    for limit in 0..=5 {
        let remaining = Cell::new(limit);
        let mut value = view(&[7, 0xff], &[true, false], &remaining, Endian::Little);
        let result = (|| {
            value.node(0)?;
            value.integer(0, Size::from_bytes(1))?;
            value.finish()
        })();
        assert_eq!(result.is_ok(), limit >= 4);
        if limit < 4 {
            assert!(matches!(
                result.unwrap_err(),
                ProductionSemanticBodyErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    ..
                }
            ));
        } else {
            assert_eq!(result.unwrap(), [7, 0]);
            assert_eq!(remaining.get(), limit - 4);
        }
    }
}

#[test]
fn exhausted_work_refuses_before_reading_initialization_or_copying_bytes() {
    let reads = Cell::new(0);
    let remaining = Cell::new(1_usize);
    let mut value = Bytes {
        raw: &[1, 2],
        initialized: |_| {
            reads.set(reads.get() + 1);
            true
        },
        charge: |work| {
            remaining
                .get()
                .checked_sub(work)
                .map(|left| remaining.set(left))
                .ok_or(ProductionSemanticBodyErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: work as u64,
                    maximum: remaining.get() as u64,
                })
        },
        endian: Endian::Little,
        block: None,
        statement: None,
    };
    assert!(value.integer(0, Size::from_bytes(2)).is_err());
    assert!(value.finish().is_err());
    assert_eq!(reads.get(), 0);
}

#[test]
fn existing_layout_depth_bound_is_inclusive_and_charged() {
    let remaining = Cell::new(2);
    let mut value = view(&[], &[], &remaining, Endian::Little);
    value.node(MAX_SEMANTIC_LAYOUT_DEPTH_V1).unwrap();
    assert!(matches!(
        value.node(MAX_SEMANTIC_LAYOUT_DEPTH_V1 + 1).unwrap_err(),
        ProductionSemanticBodyErrorV1::Unsupported {
            block: Some(7),
            statement: Some(9),
            ..
        }
    ));
    assert_eq!(remaining.get(), 0);
}

#[test]
fn direct_enum_tag_conversion_preserves_signed_discriminants() {
    assert_eq!(
        direct_discriminant(0xff, Primitive::Int(Integer::I8, true), Size::from_bytes(8)),
        Some(u64::MAX.into())
    );
    assert_eq!(
        direct_discriminant(
            0xff,
            Primitive::Int(Integer::I8, false),
            Size::from_bytes(8)
        ),
        Some(255)
    );
    assert_eq!(
        direct_discriminant(3, Primitive::Int(Integer::I8, false), Size::from_bytes(1)),
        Some(3)
    );
    assert_eq!(
        direct_discriminant(0, Primitive::Int(Integer::I8, false), Size::ZERO),
        None
    );
    assert_eq!(
        direct_discriminant(0, Primitive::Int(Integer::I8, false), Size::from_bytes(17)),
        None
    );
}

#[test]
fn exact_variant_selection_rejects_missing_ambiguous_and_unfunded_tags() {
    for (tag, expected) in [(3, Some(0)), (11, Some(1)), (5, None)] {
        let remaining = Cell::new(2);
        let result = view(&[], &[], &remaining, Endian::Little).direct_variant(
            tag,
            [(VariantIdx::from_u32(0), 3), (VariantIdx::from_u32(1), 11)].into_iter(),
        );
        assert_eq!(result.ok().map(|index| index.as_u32()), expected);
        assert_eq!(remaining.get(), 0);
    }
    for (budget, duplicate) in [(1, false), (2, true)] {
        let remaining = Cell::new(budget);
        assert!(
            view(&[], &[], &remaining, Endian::Little)
                .direct_variant(
                    3,
                    [
                        (VariantIdx::from_u32(0), 3),
                        (VariantIdx::from_u32(1), if duplicate { 3 } else { 11 })
                    ]
                    .into_iter()
                )
                .is_err()
        );
    }
}

#[test]
fn raw_pointer_and_union_scalar_representations_remain_refused() {
    let remaining = Cell::new(0);
    let value = view(&[], &[], &remaining, Endian::Little);
    for scalar in [
        Scalar::Union {
            value: Primitive::Int(Integer::I64, false),
        },
        Scalar::Initialized {
            value: Primitive::Pointer(rustc_abi::AddressSpace::ZERO),
            valid_range: WrappingRange {
                start: 0,
                end: u64::MAX.into(),
            },
        },
    ] {
        assert!(value.scalar_representation(scalar).is_err());
    }
}

#[test]
fn niche_selection_uses_finite_width_wrapping_and_refuses_invalid_rosters() {
    assert_eq!(
        niche_variant(255, Size::from_bytes(1), 255, 1, 2, 0),
        Some(1)
    );
    assert_eq!(niche_variant(0, Size::from_bytes(1), 255, 1, 2, 0), Some(2));
    assert_eq!(niche_variant(1, Size::from_bytes(1), 255, 1, 2, 0), Some(0));
    assert_eq!(
        niche_variant(u128::MAX, Size::from_bytes(16), u128::MAX, 1, 1, 0),
        Some(1)
    );
    for (size, first, last, untagged) in [(0, 1, 1, 0), (17, 1, 1, 0), (1, 2, 1, 0)] {
        assert_eq!(
            niche_variant(0, Size::from_bytes(size), 0, first, last, untagged),
            None
        );
    }
}

#[test]
fn niche_range_may_span_the_untagged_variant_but_its_encoded_slot_is_dead() {
    for (start, cases) in [
        (
            2,
            [
                (2, Some(0)),
                (3, None),
                (4, Some(2)),
                (0, Some(1)),
                (1, Some(1)),
            ],
        ),
        (
            255,
            [
                (255, Some(0)),
                (0, None),
                (1, Some(2)),
                (2, Some(1)),
                (254, Some(1)),
            ],
        ),
    ] {
        for (bits, expected) in cases {
            assert_eq!(
                niche_variant(bits, Size::from_bytes(1), start, 0, 2, 1),
                expected,
                "start {start}, tag {bits}",
            );
        }
    }
}
