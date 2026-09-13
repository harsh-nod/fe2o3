use super::*;
use pliron::{
    attribute::AttrObj,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    combine::{Parser, eof},
    operation::verify_operation,
    parsable::{Parsable, parse_from_str},
    printable::Printable,
};

include!("verification_tests.rs");

fn context() -> Context {
    let mut ctx = Context::new();
    crate::register_dialect(&mut ctx).unwrap();
    ctx
}

fn integer(ctx: &Context, width: u32, signed: bool) -> TypeHandle {
    IntegerType::get(
        ctx,
        width,
        if signed {
            Signedness::Signed
        } else {
            Signedness::Unsigned
        },
    )
    .into()
}

fn build_empty(
    ctx: &mut Context,
    ty: TypeHandle,
    kind: SwitchKeyKindAttrV3,
    keys: Vec<u64>,
) -> std::result::Result<SwitchOpV3, SwitchErrorV3> {
    let source = BasicBlock::new(ctx, None, vec![ty]);
    let selector = source.deref(ctx).get_argument(0);
    let target = BasicBlock::new(ctx, None, vec![]);
    let edges = (0..=keys.len())
        .map(|_| SwitchEdgeV3::new(target, vec![]))
        .collect();
    SwitchOpV3::try_new(ctx, selector, kind, keys, edges)
}

#[test]
fn switch_v3_every_signed_and_unsigned_key_width_retains_exact_bits() {
    use SwitchKeyKindAttrV3 as K;
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
        let high = 1u64 << (width - 1);
        let max = u64::MAX >> (64 - width);
        let keys = if signed {
            vec![high, max, 0, high - 1]
        } else {
            vec![0, high, max]
        };
        let ty = integer(ctx, width, signed);
        let operation = build_empty(ctx, ty, kind, keys.clone()).unwrap();
        assert_eq!(operation.cases(ctx).unwrap().bits(), keys);
        operation.verify(ctx).unwrap();
        operation.verify_interfaces(ctx).unwrap();
    }
}

#[test]
fn switch_v3_legacy_order_and_128_bit_selector_are_not_narrowed() {
    let ctx = &mut context();
    for (width, signed) in [(64, false), (64, true), (128, false), (128, true)] {
        let ty = integer(ctx, width, signed);
        let keys = vec![u64::MAX, 0, 1 << 63, 3];
        let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::LegacyU64, keys.clone()).unwrap();
        assert_eq!(operation.cases(ctx).unwrap().bits(), keys);
        assert_eq!(operation.selector(ctx).unwrap().get_type(ctx), ty);
    }
}

#[test]
fn switch_v3_empty_typed_wide_and_physical_index_are_explicit() {
    let ctx = &mut context();
    for signed in [false, true] {
        let ty = integer(ctx, 128, signed);
        let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::EmptyTyped, vec![]).unwrap();
        assert_eq!(operation.get_operation().deref(ctx).get_num_successors(), 1);
    }
    let index = IndexType::get(ctx).into();
    let operation = build_empty(
        ctx,
        index,
        SwitchKeyKindAttrV3::Index,
        vec![0, 1 << 63, u64::MAX],
    )
    .unwrap();
    assert_eq!(operation.selector(ctx).unwrap().get_type(ctx), index);
}

#[test]
fn switch_v3_rejects_wrong_type_duplicate_unsorted_and_unrepresentable_keys() {
    use SwitchKeyKindAttrV3 as K;
    let ctx = &mut context();
    for (width, signed, kind, keys) in [
        (8, false, K::LegacyU64, vec![256]),
        (8, true, K::LegacyU64, vec![u64::MAX]),
        (64, false, K::LegacyU64, vec![1, 1]),
        (64, false, K::U64, vec![2, 1]),
        (64, true, K::I64, vec![0, u64::MAX]),
        (8, true, K::I8, vec![128, 128]),
        (8, true, K::I8, vec![256]),
        (32, false, K::I32, vec![0]),
        (128, false, K::U64, vec![0]),
        (64, false, K::EmptyTyped, vec![0]),
    ] {
        let ty = integer(ctx, width, signed);
        assert!(build_empty(ctx, ty, kind, keys).is_err());
    }
    let boolean = IntegerType::get(ctx, 1, Signedness::Signless).into();
    assert!(build_empty(ctx, boolean, K::EmptyTyped, vec![]).is_err());
}

#[test]
fn switch_v3_duplicate_destinations_retain_separate_payload_positions() {
    let ctx = &mut context();
    let ty = integer(ctx, 64, false);
    let source = BasicBlock::new(ctx, None, vec![ty, ty, ty]);
    let selector = source.deref(ctx).get_argument(0);
    let a = source.deref(ctx).get_argument(1);
    let b = source.deref(ctx).get_argument(2);
    let target = BasicBlock::new(ctx, None, vec![ty, ty]);
    let operation = SwitchOpV3::try_new(
        ctx,
        selector,
        SwitchKeyKindAttrV3::U64,
        vec![0, 1],
        vec![
            SwitchEdgeV3::new(target, vec![a, b]),
            SwitchEdgeV3::new(target, vec![b, a]),
            SwitchEdgeV3::new(target, vec![a, a]),
        ],
    )
    .unwrap();
    assert_eq!(operation.successor_operand_range(ctx, 0), Some(1..3));
    assert_eq!(operation.successor_operand_range(ctx, 1), Some(3..5));
    assert_eq!(operation.successor_operand_range(ctx, 2), Some(5..7));
    assert_eq!(operation.successor_operands(ctx, 0), vec![a, b]);
    assert_eq!(operation.successor_operands(ctx, 1), vec![b, a]);
    assert_eq!(operation.successor_operands(ctx, 2), vec![a, a]);
    assert_eq!(operation.successor_operand_range(ctx, 3), None);
    operation.verify_interfaces(ctx).unwrap();
}

#[test]
fn switch_v3_per_edge_payloads_are_not_an_aggregate_64_argument_limit() {
    let ctx = &mut context();
    let ty = integer(ctx, 32, false);
    let source = BasicBlock::new(ctx, None, vec![ty; 65]);
    let selector = source.deref(ctx).get_argument(0);
    let first = BasicBlock::new(ctx, None, vec![ty; 32]);
    let second = BasicBlock::new(ctx, None, vec![ty; 32]);
    let default = BasicBlock::new(ctx, None, vec![ty; 64]);
    let arguments =
        |range: Range<usize>| range.map(|i| source.deref(ctx).get_argument(i)).collect();
    let edges = vec![
        SwitchEdgeV3::new(first, arguments(1..33)),
        SwitchEdgeV3::new(second, arguments(33..65)),
        SwitchEdgeV3::new(default, arguments(1..65)),
    ];
    let operation =
        SwitchOpV3::try_new(ctx, selector, SwitchKeyKindAttrV3::U32, vec![0, 1], edges).unwrap();
    assert_eq!(operation.get_operation().deref(ctx).get_num_operands(), 129);
    assert_eq!(operation.get_operation().deref(ctx).get_num_results(), 0);
    operation.verify_interfaces(ctx).unwrap();
}

#[test]
fn switch_v3_branch_interface_edits_preserve_later_edge_offsets_and_uses() {
    let ctx = &mut context();
    let ty = integer(ctx, 32, false);
    let source = BasicBlock::new(ctx, None, vec![ty, ty]);
    let selector = source.deref(ctx).get_argument(0);
    let value = source.deref(ctx).get_argument(1);
    let first = BasicBlock::new(ctx, None, vec![]);
    let second = BasicBlock::new(ctx, None, vec![ty]);
    let operation = SwitchOpV3::try_new(
        ctx,
        selector,
        SwitchKeyKindAttrV3::U32,
        vec![0],
        vec![
            SwitchEdgeV3::new(first, vec![]),
            SwitchEdgeV3::new(second, vec![value]),
        ],
    )
    .unwrap();
    assert_eq!(operation.add_successor_operand(ctx, 0, value), 0);
    assert_eq!(operation.successor_operand_range(ctx, 1), Some(2..3));
    assert_eq!(operation.successor_operands(ctx, 0), vec![value]);
    assert_eq!(operation.remove_successor_operand(ctx, 0, 0), value);
    assert_eq!(operation.successor_operand_range(ctx, 1), Some(1..2));
    assert_eq!(value.num_uses(ctx), 1);
    operation.verify(ctx).unwrap();
    operation.verify_interfaces(ctx).unwrap();
}

#[test]
fn switch_v3_malformed_offsets_and_generated_segments_reject_without_indexing_panics() {
    let ctx = &mut context();
    for offsets in [
        vec![],
        vec![0],
        vec![1, 1],
        vec![0, 1],
        vec![0, 0, 0],
        vec![2, 1],
    ] {
        let ty = integer(ctx, 32, false);
        let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::EmptyTyped, vec![]).unwrap();
        operation.get_operation().deref_mut(ctx).attributes.set(
            "gpu_switch_offsets".try_into().unwrap(),
            SwitchSuccessorOffsetsAttrV3(offsets),
        );
        assert!(operation.verify(ctx).is_err());
        let _ = operation.successor_operands(ctx, 0);
    }
    for sizes in [
        vec![],
        vec![1],
        vec![1, 1],
        vec![u32::MAX, 1],
        vec![1, u32::MAX],
        vec![1, 0, 0],
    ] {
        let ty = integer(ctx, 32, false);
        let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::EmptyTyped, vec![]).unwrap();
        operation.set_operand_segment_sizes(ctx, OperandSegmentSizesAttr(sizes));
        assert!(operation.verify(ctx).is_err());
        assert!(operation.verify_interfaces(ctx).is_err());
    }
}

#[test]
fn switch_v3_failed_payload_or_key_admission_registers_no_new_uses() {
    let ctx = &mut context();
    let ty = integer(ctx, 32, false);
    let source = BasicBlock::new(ctx, None, vec![ty]);
    let selector = source.deref(ctx).get_argument(0);
    let target = BasicBlock::new(ctx, None, vec![ty]);
    let result = SwitchOpV3::try_new(
        ctx,
        selector,
        SwitchKeyKindAttrV3::EmptyTyped,
        vec![],
        vec![SwitchEdgeV3::new(target, vec![])],
    );
    assert!(matches!(result, Err(SwitchErrorV3::Payload)));
    assert_eq!(selector.num_uses(ctx), 0);
    assert!(target.uses(ctx).is_empty());
}

#[test]
fn switch_v3_case_and_edge_wire_ceilings_are_exact() {
    let ctx = &mut context();
    let ty = integer(ctx, 64, false);
    let keys = (0..MAX_SWITCH_CASES_V3 as u64).collect::<Vec<_>>();
    validate_keys(ctx, ty, SwitchKeyKindAttrV3::U64, &keys).unwrap();
    let mut over = keys;
    over.push(MAX_SWITCH_CASES_V3 as u64);
    assert_eq!(
        validate_keys(ctx, ty, SwitchKeyKindAttrV3::U64, &over),
        Err(SwitchErrorV3::Limit)
    );
    assert!(valid_offsets(&[0, MAX_SWITCH_EDGE_ARGUMENTS_V3 as u32]));
    assert!(!valid_offsets(&[
        0,
        MAX_SWITCH_EDGE_ARGUMENTS_V3 as u32 + 1
    ]));
}

#[test]
fn switch_v3_rejects_extra_metadata_before_inspecting_large_payloads() {
    let ctx = &mut context();
    let ty = integer(ctx, 64, false);
    let operation = build_empty(ctx, ty, SwitchKeyKindAttrV3::EmptyTyped, vec![]).unwrap();
    let key = "arbitrary_extra_payload".try_into().unwrap();
    // This payload independently fails its attribute verifier. The operation
    // must instead reject its envelope before visiting that payload or keys.
    operation
        .get_operation()
        .deref_mut(ctx)
        .attributes
        .set(key, SwitchCaseBitsAttrV3(vec![0; MAX_SWITCH_CASES_V3 + 1]));
    let error = operation.verify(ctx).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exactly four structural attributes")
    );
    Operation::erase(operation.get_operation(), ctx);
}

#[test]
fn switch_v3_attributes_and_complete_function_round_trip_without_source_metadata() {
    let ctx = &mut context();
    for attribute in [
        Box::new(SwitchKeyKindAttrV3::I64) as AttrObj,
        Box::new(SwitchCaseBitsAttrV3(vec![1 << 63, u64::MAX, 0])),
        Box::new(SwitchSuccessorOffsetsAttrV3(vec![0, 1, 2, 2])),
    ] {
        let text = attribute.disp(ctx).to_string();
        let parsed = parse_from_str(AttrObj::parser(()).skip(eof()), ctx, &text).unwrap();
        assert_eq!(text, parsed.disp(ctx).to_string());
    }
    let ty = integer(ctx, 64, true);
    let signature = FunctionType::get(ctx, vec![ty], vec![]);
    let function = FuncOp::new(
        ctx,
        "native_switch_roundtrip".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(ctx);
    let selector = entry.deref(ctx).get_argument(0);
    let target = BasicBlock::new(ctx, Some("exit".try_into().unwrap()), vec![]);
    target.insert_at_back(function.get_region(ctx), ctx);
    let operation = SwitchOpV3::try_new(
        ctx,
        selector,
        SwitchKeyKindAttrV3::I64,
        vec![1 << 63, u64::MAX],
        (0..3).map(|_| SwitchEdgeV3::new(target, vec![])).collect(),
    )
    .unwrap();
    operation.get_operation().insert_at_back(entry, ctx);
    crate::optimization_v1::ReturnOp::new(ctx, vec![])
        .get_operation()
        .insert_at_back(target, ctx);
    verify_operation(function.get_operation(), ctx).unwrap();
    let text = function.get_operation().disp(ctx).to_string();
    assert!(text.contains("gpu.switch_v3"));
    assert!(!text.contains("preserved_terminator"));
    let parsed = parse_from_str(Operation::top_level_parser(), ctx, &text).unwrap();
    verify_operation(parsed, ctx).unwrap();
}
