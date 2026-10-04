use super::*;
use dialect_gpu::switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3};
use dialect_kernel::{AnalysisSplitControlCountAttr, IndexConstantOp, IndexType};
use pliron::{
    attribute::AttrObj,
    builtin::types::{IntegerType, Signedness},
    dialect::DialectName,
    op::Op,
    parsable::{Parsable, parse_from_str},
    r#type::{TypeHandle, Typed},
};

fn context() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(&mut context, &DialectName::try_new("kernel").unwrap())
        .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn assert_edges(context: &Context, operation: Ptr<Operation>, expected: &[&[Value]]) {
    let control = ControlViewV1::observe(context, operation).unwrap();
    assert_eq!(control.successor_count(), expected.len());
    for (ordinal, arguments) in expected.iter().enumerate() {
        let edge = control.edge(ordinal).unwrap();
        assert_eq!(edge.ordinal, ordinal);
        assert_eq!(
            edge.target(),
            operation.deref(context).get_successor(ordinal)
        );
        assert_eq!(edge.argument_count(), arguments.len());
        for (index, incoming) in arguments.iter().enumerate() {
            assert_eq!(
                edge.argument_at(index).unwrap(),
                (*incoming, edge.target().deref(context).get_argument(index))
            );
        }
        assert_eq!(
            edge.argument_at(arguments.len()),
            Err(ControlErrorV1::ArgumentOrdinal)
        );
    }
    assert!(matches!(
        control.edge(expected.len()),
        Err(ControlErrorV1::EdgeOrdinal)
    ));
}

#[test]
fn all_fixed_edge_families_preserve_payloads_and_only_less_than_yields_a_guard() {
    let context = &mut context();
    let index = IndexType::get(context).into();
    let empty = BasicBlock::new(context, None, vec![]);
    let join = BasicBlock::new(context, None, vec![index, index]);
    let a = IndexConstantOp::new(context, 7).result(context);
    let b = IndexConstantOp::new(context, 9).result(context);
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let source = BasicBlock::new(context, None, vec![boolean]);
    let condition = source.deref(context).get_argument(0);
    let single = [
        BranchArgsOp::new(context, vec![a, b], join).get_operation(),
        GpuBranchOp::new(context, join, vec![a, b]).get_operation(),
    ];
    for operation in single {
        assert_edges(context, operation, &[&[a, b]]);
    }
    let plain = BranchOp::new(context, empty);
    assert_edges(context, plain.get_operation(), &[&[]]);
    let less = IndexLessThanBranchArgsOp::new(context, a, b, vec![a, b], vec![b, a], join, join);
    let equal = IndexEqualBranchArgsOp::new(context, a, b, vec![a, b], vec![b, a], join, join);
    let split = AnalysisSplitOp::new_with_control_and_arguments(
        context,
        vec![b, a, b],
        vec![a, b],
        vec![b, a],
        join,
        join,
    );
    let cond = CondBranchOp::new(context, condition, join, vec![a, b], join, vec![b, a]);
    for operation in [
        less.get_operation(),
        equal.get_operation(),
        split.get_operation(),
        cond.get_operation(),
    ] {
        assert_edges(context, operation, &[&[a, b], &[b, a]]);
        let control = ControlViewV1::observe(context, operation).unwrap();
        assert_eq!(
            control.edge(0).unwrap().index_less_than_guard(),
            (operation == less.get_operation()).then_some((a, b))
        );
        assert_eq!(control.edge(1).unwrap().index_less_than_guard(), None);
    }
    let less = IndexLessThanBranchOp::new(context, a, b, empty, empty);
    let equal = IndexEqualBranchOp::new(context, a, b, empty, empty);
    for operation in [less.get_operation(), equal.get_operation()] {
        assert_edges(context, operation, &[&[], &[]]);
        let control = ControlViewV1::observe(context, operation).unwrap();
        assert_eq!(
            control.edge(0).unwrap().index_less_than_guard(),
            (operation == less.get_operation()).then_some((a, b))
        );
    }
}

#[test]
fn switch_edges_keep_original_ordinals_without_narrowing_or_reading_keys() {
    use SwitchKeyKindAttrV3 as K;
    let context = &mut context();
    for (width, signed, kind, keys) in [
        (128, false, K::LegacyU64, vec![u64::MAX, 0]),
        (
            64,
            true,
            K::I64,
            vec![1 << 63, u64::MAX, 0, i64::MAX as u64],
        ),
        (128, true, K::EmptyTyped, vec![]),
        (128, false, K::LegacyU64, vec![]),
    ] {
        let ty: TypeHandle = IntegerType::get(
            context,
            width,
            if signed {
                Signedness::Signed
            } else {
                Signedness::Unsigned
            },
        )
        .into();
        let source = BasicBlock::new(context, None, vec![ty; 3]);
        let selector = source.deref(context).get_argument(0);
        let a = source.deref(context).get_argument(1);
        let b = source.deref(context).get_argument(2);
        let join = BasicBlock::new(context, None, vec![ty; 2]);
        let tuples = (0..=keys.len())
            .map(|i| if i % 2 == 0 { vec![a, b] } else { vec![b, a] })
            .collect::<Vec<_>>();
        let edges = tuples
            .iter()
            .map(|args| SwitchEdgeV3::new(join, args.clone()))
            .collect();
        let switch = SwitchOpV3::try_new(context, selector, kind, keys.clone(), edges).unwrap();
        assert_edges(
            context,
            switch.get_operation(),
            &tuples.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        );
        let control = ControlViewV1::observe(context, switch.get_operation()).unwrap();
        for ordinal in 0..control.successor_count() {
            assert_eq!(control.edge(ordinal).unwrap().index_less_than_guard(), None);
        }
        assert_eq!(switch.selector(context).unwrap().get_type(context), ty);
        assert_eq!(switch.kind(context), Some(kind));
        assert_eq!(switch.cases(context).unwrap().bits(), keys);
    }
}

#[test]
fn malformed_segments_and_unclamped_split_count_fail_closed() {
    let context = &mut context();
    let index = IndexType::get(context).into();
    let join = BasicBlock::new(context, None, vec![index]);
    let a = IndexConstantOp::new(context, 0).result(context);
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let source = BasicBlock::new(context, None, vec![boolean]);
    let condition = source.deref(context).get_argument(0);
    let cond = CondBranchOp::new(context, condition, join, vec![a], join, vec![a]);
    for segments in [
        vec![],
        vec![0, 1, 1],
        vec![1, 1],
        vec![1, u32::MAX, 1],
        vec![1, 0, 2],
    ] {
        cond.get_operation().deref_mut(context).attributes.set(
            ATTR_KEY_OPERAND_SEGMENT_SIZES.clone(),
            OperandSegmentSizesAttr(segments),
        );
        let view = ControlViewV1::observe(context, cond.get_operation());
        assert!(view.is_err() || view.unwrap().edge(0).is_err());
    }
    cond.get_operation()
        .deref_mut(context)
        .attributes
        .0
        .remove(&ATTR_KEY_OPERAND_SEGMENT_SIZES);
    assert!(ControlViewV1::observe(context, cond.get_operation()).is_err());
    let split = AnalysisSplitOp::new_with_arguments(context, vec![a], vec![a], join, join);
    split.set_attr_kernel_analysis_split_control_count(
        context,
        AnalysisSplitControlCountAttr(u32::MAX),
    );
    assert!(ControlViewV1::observe(context, split.get_operation()).is_err());
}

#[test]
fn unknown_zero_successor_operations_are_not_exits() {
    let context = &mut context();
    let constant = IndexConstantOp::new(context, 0);
    assert!(ControlViewV1::observe(context, constant.get_operation()).is_err());
    let ret = ReturnOp::new(context);
    let trap = TrapOp::new(context);
    let gpu_return = GpuReturnOp::new(context, vec![]);
    for operation in [
        ret.get_operation(),
        trap.get_operation(),
        gpu_return.get_operation(),
    ] {
        assert_edges(context, operation, &[]);
    }
}

#[test]
fn malformed_switch_offsets_reject_before_any_out_of_range_payload_read() {
    let context = &mut context();
    let ty = IntegerType::get(context, 64, Signedness::Unsigned).into();
    let source = BasicBlock::new(context, None, vec![ty]);
    let value = source.deref(context).get_argument(0);
    let join = BasicBlock::new(context, None, vec![ty]);
    let switch = SwitchOpV3::try_new(
        context,
        value,
        SwitchKeyKindAttrV3::LegacyU64,
        vec![0, 1],
        (0..3)
            .map(|_| SwitchEdgeV3::new(join, vec![value]))
            .collect(),
    )
    .unwrap();
    for offsets in [
        "[]",
        "[0, 1, 3]",
        "[1, 1, 2, 3]",
        "[0, 1, 2, 4]",
        "[0, 2, 1, 3]",
        "[0, 4294967295, 2, 3]",
        "[0, 0, 2, 3]",
    ] {
        let attribute = parse_from_str(
            AttrObj::parser(()),
            context,
            &format!("gpu.switch_successor_offsets_v3 {offsets}"),
        )
        .unwrap();
        switch
            .get_operation()
            .deref_mut(context)
            .attributes
            .0
            .insert("gpu_switch_offsets".try_into().unwrap(), attribute);
        if let Ok(control) = ControlViewV1::observe(context, switch.get_operation()) {
            assert!(
                (0..control.successor_count()).any(|ordinal| control.edge(ordinal).is_err()),
                "malformed offsets accepted: {offsets}"
            );
        }
    }
    switch
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .remove(&"gpu_switch_offsets".try_into().unwrap());
    assert!(ControlViewV1::observe(context, switch.get_operation()).is_err());
}
