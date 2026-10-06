fn switch_constant(ctx: &Context, width: u32, signed: bool, bits: u128) -> AttrObj {
    use pliron::{
        builtin::attributes::IntegerAttr,
        utils::apint::{APInt, bw},
    };
    Box::new(IntegerAttr::new(
        IntegerType::get(
            ctx,
            width,
            if signed {
                Signedness::Signed
            } else {
                Signedness::Unsigned
            },
        ),
        APInt::from_u128(bits, bw(width as usize)),
    ))
}

#[test]
fn switch_v3_fold_selects_exact_signed_unsigned_and_legacy_occurrences() {
    use SwitchKeyKindAttrV3 as K;
    use pliron::opts::constants::BranchOpFoldInterface;
    let ctx = &mut context();
    for (width, signed, kind) in [
        (8, true, K::I8),
        (16, true, K::I16),
        (32, true, K::I32),
        (64, true, K::I64),
        (8, false, K::U8),
        (16, false, K::U16),
        (32, false, K::U32),
        (64, false, K::U64),
    ] {
        let high = 1_u64 << (width - 1);
        let max = u64::MAX >> (64 - width);
        let (keys, examples) = if signed {
            (
                vec![high, max, 0, high - 1],
                vec![(high, 0), (max, 1), (0, 2), (high - 1, 3), (1, 4)],
            )
        } else {
            (
                vec![0, high, max],
                vec![(0, 0), (high, 1), (max, 2), (1, 3)],
            )
        };
        let ty = integer(ctx, width, signed);
        let operation = build_empty(ctx, ty, kind, keys).unwrap();
        for (bits, ordinal) in examples {
            let operands = [Some(switch_constant(ctx, width, signed, u128::from(bits)))];
            assert_eq!(operation.selected_successor(ctx, &operands), Some(ordinal));
            assert_eq!(operation.check_fold(ctx, &operands).len(), 1);
        }
        assert_eq!(operation.selected_successor(ctx, &[None]), None);
        assert_eq!(
            operation.check_fold(ctx, &[None]).len(),
            operation.cases(ctx).unwrap().bits().len() + 1
        );
        let legacy = build_empty(ctx, ty, K::LegacyU64, vec![max, 0, 3, high]).unwrap();
        for (bits, ordinal) in [(max, 0), (0, 1), (3, 2), (high, 3), (2, 4)] {
            assert_eq!(
                legacy.selected_successor(
                    ctx,
                    &[Some(switch_constant(ctx, width, signed, u128::from(bits)))]
                ),
                Some(ordinal)
            );
        }
    }
}

#[test]
fn switch_v3_fold_128_bits_never_aliases_a_low_half_case() {
    use SwitchKeyKindAttrV3 as K;
    let ctx = &mut context();
    for signed in [false, true] {
        let ty = integer(ctx, 128, signed);
        let operation = build_empty(ctx, ty, K::LegacyU64, vec![3, u64::MAX]).unwrap();
        for (bits, ordinal) in [
            (3, 0),
            (u128::from(u64::MAX), 1),
            (7, 2),
            ((1_u128 << 64) | 3, 2),
            ((1_u128 << 127) | 3, 2),
            (u128::MAX, 2),
        ] {
            assert_eq!(
                operation.selected_successor(ctx, &[Some(switch_constant(ctx, 128, signed, bits))]),
                Some(ordinal)
            );
        }
        let empty = build_empty(ctx, ty, K::EmptyTyped, vec![]).unwrap();
        assert_eq!(empty.selected_successor(ctx, &[None]), Some(0));
        assert_eq!(
            empty.selected_successor(ctx, &[Some(switch_constant(ctx, 128, signed, u128::MAX))]),
            Some(0)
        );
    }
}

#[test]
fn switch_v3_fold_requires_the_exact_constant_representation() {
    use crate::optimization_v1::IndexAttr;
    let ctx = &mut context();
    let ty = integer(ctx, 32, false);
    let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::U32, vec![3]).unwrap();
    for operands in [
        vec![],
        vec![None],
        vec![Some(switch_constant(ctx, 64, false, 3))],
        vec![Some(switch_constant(ctx, 32, true, 3))],
        vec![Some(Box::new(IndexAttr(3)) as AttrObj)],
    ] {
        assert_eq!(operation.selected_successor(ctx, &operands), None);
    }
    let ty = IndexType::get(ctx).into();
    let index = build_empty(ctx, ty, SwitchKeyKindAttrV3::Index, vec![0, u64::MAX]).unwrap();
    for (bits, ordinal) in [(0, 0), (u64::MAX, 1), (3, 2)] {
        assert_eq!(
            index.selected_successor(ctx, &[Some(Box::new(IndexAttr(bits)))]),
            Some(ordinal)
        );
    }
    assert_eq!(
        index.selected_successor(ctx, &[Some(switch_constant(ctx, 64, false, 0))]),
        None
    );
}
