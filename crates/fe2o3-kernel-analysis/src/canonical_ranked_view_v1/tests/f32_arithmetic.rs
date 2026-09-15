use super::*;

fn expression_module(kind: BinaryOp, bits: u32) -> Module {
    let mut module = scalar_module("exact_float_expression");
    operations(&mut module).extend([
        KirOp::effect_free(
            ValueDef::new(ValueId(12), Type::F32),
            OperationKind::Constant(Constant::F32Bits(bits)),
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(13), Type::F32),
            OperationKind::Binary {
                op: kind,
                lhs: ValueId(2),
                rhs: ValueId(12),
            },
        ),
    ]);
    module
}

#[test]
fn f32_arithmetic_preserves_operator_operands_and_exceptional_bits() {
    for (source, expected) in [
        (BinaryOp::Add, SemanticTypedBinaryKindAttr::Add),
        (BinaryOp::Subtract, SemanticTypedBinaryKindAttr::Subtract),
        (BinaryOp::Multiply, SemanticTypedBinaryKindAttr::Multiply),
        (BinaryOp::Divide, SemanticTypedBinaryKindAttr::Divide),
        (BinaryOp::Remainder, SemanticTypedBinaryKindAttr::Remainder),
    ] {
        for bits in [0x8000_0000, 0x7fc0_1234, 0x3f80_0000] {
            let (canonical, plan) = prepare(expression_module(source, bits));
            let mut context = setup();
            let view = plan.materialize(&mut context).unwrap();
            let binary = find::<SemanticTypedBinaryOp>(&context, &view);
            let parameter = find::<SemanticTypedSymbolOp>(&context, &view);
            let constant = find::<SemanticTypedConstantOp>(&context, &view);
            assert_eq!(binary.kind(&context), Some(expected));
            assert_eq!(binary.scalar(&context), parameter.scalar(&context));
            assert_eq!(binary.lhs(&context), parameter.result(&context));
            assert_eq!(binary.rhs(&context), constant.result(&context));
            assert_eq!(constant.bits(&context), Some(u64::from(bits)));
            assert!(view.writes().is_empty());
            view.revalidate(&context, &canonical, 17).unwrap();
        }
    }
}

#[test]
fn f32_arithmetic_does_not_contract_multiply_add() {
    let mut module = expression_module(BinaryOp::Multiply, 0x3f80_0001);
    operations(&mut module).push(KirOp::effect_free(
        ValueDef::new(ValueId(14), Type::F32),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(13),
            rhs: ValueId(2),
        },
    ));
    let (canonical, plan) = prepare(module);
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    let binaries = view
        .graph
        .live_operations
        .iter()
        .filter_map(|op| Operation::get_op::<SemanticTypedBinaryOp>(*op, &context))
        .collect::<Vec<_>>();
    assert_eq!(binaries.len(), 2);
    assert_eq!(
        binaries[0].kind(&context),
        Some(SemanticTypedBinaryKindAttr::Multiply)
    );
    assert_eq!(
        binaries[1].kind(&context),
        Some(SemanticTypedBinaryKindAttr::Add)
    );
    assert_eq!(binaries[1].lhs(&context), binaries[0].result(&context));
    view.revalidate(&context, &canonical, 17).unwrap();
}

#[test]
fn f32_arithmetic_projection_does_not_supply_write_contracts() {
    let mut module = expression_module(BinaryOp::Add, 0x3f80_0000);
    let mut store = operations(&mut fill_module("unused")).pop().unwrap();
    let OperationKind::GuardedStore { value, .. } = &mut store.kind else {
        unreachable!()
    };
    *value = ValueId(13);
    operations(&mut module).push(store);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let plan = structural_plan(&canonical).unwrap();
    assert_eq!(plan.writes[0].rhs(), ValueId(13));
    assert!(matches!(
        prepare_canonical_ranked_view_v1(
            &canonical,
            17,
            &FunctionId::new("exact_float_expression")
        ),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. }),
    ));
}

#[test]
fn f32_arithmetic_operand_and_operator_mutations_invalidate_the_view() {
    for change_operand in [false, true] {
        let (canonical, plan) = prepare(expression_module(BinaryOp::Divide, 0x3f80_0000));
        let mut context = setup();
        let view = plan.materialize(&mut context).unwrap();
        let binary = find::<SemanticTypedBinaryOp>(&context, &view);
        if change_operand {
            Operation::replace_operand(binary.get_operation(), &context, 0, binary.rhs(&context));
        } else {
            binary.set_attr_kernel_semantic_typed_binary_kind(
                &context,
                SemanticTypedBinaryKindAttr::Multiply,
            );
        }
        assert!(matches!(
            view.revalidate(&context, &canonical, 17),
            Err(CanonicalRankedViewErrorV1::MutationEpochChanged),
        ));
    }
}
