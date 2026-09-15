use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, IndexConstantOp, IndexType,
    MAX_RANKED_MEMORY_RANK, RankedAccessOp, RankedViewOp, RankedViewType, SemanticScalarKindAttr,
    SemanticTypedConstantOp, SemanticTypedScalarV1,
};
use pliron::{
    builtin::{op_interfaces::SingleBlockRegionInterface, ops::ModuleOp},
    context::Context,
    dialect::DialectName,
    linked_list::ContainsLinkedList,
    op::{Op, verify_op},
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
    printable::Printable,
};

fn context() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(&mut context, &DialectName::try_new("kernel").unwrap())
        .unwrap();
    context
}

#[test]
fn stored_values_preserve_rank_indices_and_round_trip_plain_and_atomic_accesses() {
    for rank in [1, MAX_RANKED_MEMORY_RANK] {
        let context = &mut context();
        let module = ModuleOp::new(context, "stores".try_into().unwrap());
        let ty = RankedViewType::new(context, 32, true, vec![8; rank]).unwrap();
        let view = RankedViewOp::new(context, ty, vec![]).unwrap();
        let index = IndexConstantOp::new(context, 0);
        let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap();
        let value = SemanticTypedConstantOp::new(context, 1_f32.to_bits().into(), scalar);
        for op in [
            view.get_operation(),
            index.get_operation(),
            value.get_operation(),
        ] {
            module.append_operation(context, op, 0);
        }
        let indices = vec![index.result(context); rank];
        let plain = RankedAccessOp::new_value(
            context,
            AccessKindAttr::Write,
            view.result(context),
            indices.clone(),
            value.result(context),
        )
        .unwrap();
        let atomic = RankedAccessOp::new_atomic_value(
            context,
            AccessKindAttr::AtomicWrite,
            AtomicOrderingAttr::Release,
            AtomicScopeAttr::Device,
            view.result(context),
            indices.clone(),
            value.result(context),
        )
        .unwrap();
        for access in [plain, atomic] {
            verify_op(&access, context).unwrap();
            assert_eq!(access.indices(context), indices);
            assert_eq!(access.stored_value(context), Some(value.result(context)));
            assert_eq!(access.checked_success(context), None);
            assert_eq!(
                access.get_operation().deref(context).get_num_operands(),
                rank + 2
            );
            module.append_operation(context, access.get_operation(), 0);
        }
        assert_eq!(
            atomic.atomic_ordering(context),
            Some(AtomicOrderingAttr::Release)
        );
        assert_eq!(atomic.atomic_scope(context), Some(AtomicScopeAttr::Device));
        let printed = module.get_operation().disp(context).to_string();
        let parsed = parse_from_str(Operation::top_level_parser(), context, &printed).unwrap();
        verify_operation(parsed, context).unwrap();
        let parsed = ModuleOp::from_operation(parsed);
        let operations: Vec<_> = parsed
            .get_body(context, 0)
            .deref(context)
            .iter(context)
            .collect();
        let value = operations[2].deref(context).get_result(0);
        let index = operations[1].deref(context).get_result(0);
        let accesses: Vec<_> = operations
            .iter()
            .copied()
            .filter(|op| Operation::is_op::<RankedAccessOp>(*op, context))
            .map(RankedAccessOp::from_operation)
            .collect();
        assert_eq!(accesses.len(), 2);
        for access in &accesses {
            assert_eq!(access.stored_value(context), Some(value));
            assert_eq!(access.indices(context), vec![index; rank]);
        }
        assert_eq!(
            accesses[1].atomic_ordering(context),
            Some(AtomicOrderingAttr::Release)
        );
        assert_eq!(
            accesses[1].atomic_scope(context),
            Some(AtomicScopeAttr::Device)
        );
    }
}

#[test]
fn stored_values_reject_wrong_kind_type_and_surplus_operands() {
    let context = &mut context();
    let ty = RankedViewType::new(context, 32, true, vec![8]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![]).unwrap();
    let index = IndexConstantOp::new(context, 0).result(context);
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap();
    let value = SemanticTypedConstantOp::new(context, 0, scalar).result(context);
    assert!(
        RankedAccessOp::new_value(
            context,
            AccessKindAttr::Read,
            view.result(context),
            vec![index],
            value
        )
        .is_err()
    );
    assert!(
        RankedAccessOp::new_value(
            context,
            AccessKindAttr::Write,
            view.result(context),
            vec![index],
            index
        )
        .is_err()
    );
    assert!(
        RankedAccessOp::new_atomic_value(
            context,
            AccessKindAttr::AtomicRead,
            AtomicOrderingAttr::Acquire,
            AtomicScopeAttr::Device,
            view.result(context),
            vec![index],
            value,
        )
        .is_err()
    );
    let readonly = RankedViewType::new(context, 32, false, vec![8]).unwrap();
    let readonly = RankedViewOp::new(context, readonly, vec![]).unwrap();
    assert!(
        RankedAccessOp::new_value(
            context,
            AccessKindAttr::Write,
            readonly.result(context),
            vec![index],
            value
        )
        .is_err()
    );
    let access = RankedAccessOp::new_value(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![index],
        value,
    )
    .unwrap();
    let op = access.get_operation();
    Operation::replace_operand(op, context, 2, index);
    assert_eq!(access.stored_value(context), Some(index));
    assert!(verify_op(&access, context).is_err());
    Operation::replace_operand(op, context, 2, value);
    access.set_attr_kernel_access_kind(context, AccessKindAttr::Read);
    assert!(verify_op(&access, context).is_err());
    access.set_attr_kernel_access_kind(context, AccessKindAttr::Write);
    Operation::insert_operand(op, context, 3, value);
    assert!(verify_op(&access, context).is_err());
    Operation::remove_operand(op, context, 3);
    verify_op(&access, context).unwrap();
    value.set_type(context, IndexType::get(context).into());
    assert!(verify_op(&access, context).is_err());
    Operation::remove_operand(op, context, 2);
    // Legacy shape is still legal; effect analysis must reject the missing RHS.
    verify_op(&access, context).unwrap();
    assert_eq!(access.stored_value(context), None);
    assert_eq!(access.indices(context), vec![index]);
}
