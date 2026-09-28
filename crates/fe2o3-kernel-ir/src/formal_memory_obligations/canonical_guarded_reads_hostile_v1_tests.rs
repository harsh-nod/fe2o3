use super::*;

fn no_local_read(module: Module) {
    let graph = owner(module);
    run(&graph, |view, budget| {
        assert!(matches!(
            view.read_at(coordinate(1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV1::NotProved(_)
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn true_or_does_not_supply_either_operand_as_truth() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind =
        OperationKind::Binary {
            op: BinaryOp::BitOr,
            lhs: ValueId(7),
            rhs: ValueId(8),
        };
    no_local_read(module);
}

#[test]
fn false_edge_and_parallel_destination_bypass_do_not_dominate() {
    for parallel in [false, true] {
        let mut module = fixture();
        let body = module.functions[0].body.as_mut().unwrap();
        let Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } = body.blocks[0].terminator.as_mut().unwrap()
        else {
            unreachable!()
        };
        *else_target = BlockId(20);
        if !parallel {
            *then_target = BlockId(30);
        }
        no_local_read(module);
    }
}

#[test]
fn unrelated_false_overflow_does_not_prove_selected_checked_operation() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        2,
        Operation::checked_binary(
            ValueDef::new(ValueId(13), Type::INDEX),
            ValueDef::new(ValueId(14), Type::BOOL),
            crate::CheckedBinaryOperator::Multiply,
            ValueId(1),
            ValueId(2),
        ),
    );
    body.blocks[0].operations[4].kind = OperationKind::Unary {
        op: crate::UnaryOp::Not,
        operand: ValueId(14),
    };
    let graph = owner(module);
    run(&graph, |view, budget| {
        assert!(matches!(
            view.read_at(coordinate(1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(_)
        ));
        assert!(
            view.no_wrap_at(coordinate(1, 2), coordinate(0, 1), budget)?
                .is_none()
        );
        let other = view
            .no_wrap_at(coordinate(1, 2), coordinate(0, 2), budget)?
            .expect("only actual guarded overflow");
        assert_eq!(other.value(), ValueId(13));
        assert_eq!(other.overflow(), ValueId(14));
        Ok(())
    })
    .unwrap();
}

#[test]
fn replacing_not_overflow_by_overflow_does_not_prove_no_wrap() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind =
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(7),
            rhs: ValueId(6),
        };
    let graph = owner(module);
    run(&graph, |view, budget| {
        assert!(
            view.no_wrap_at(coordinate(1, 2), coordinate(0, 1), budget)?
                .is_none()
        );
        assert!(
            view.true_at(coordinate(1, 2), ValueId(6), budget)?
                .is_some()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn wrong_offset_and_unsupported_alignment_are_not_local_read_facts() {
    let mut wrong_offset = fixture();
    let OperationKind::GetElementPointer { offset, .. } =
        &mut wrong_offset.functions[0].body.as_mut().unwrap().blocks[1].operations[1].kind
    else {
        unreachable!()
    };
    *offset = ValueId(1);
    no_local_read(wrong_offset);
    let mut alignment = fixture();
    let OperationKind::Load { access, .. } =
        &mut alignment.functions[0].body.as_mut().unwrap().blocks[1].operations[2].kind
    else {
        unreachable!()
    };
    *access = MemoryAccess::new(AddressSpace::Global, 8);
    no_local_read(alignment);
}

#[test]
fn exact_owner_not_equal_bytes_is_the_fact_subject() {
    let first = owner(fixture());
    let second = owner(fixture());
    assert_eq!(first.canonical().identity(), second.canonical().identity());
    run(&first, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(fact) =
            view.read_at(coordinate(1, 2), budget)?
        else {
            panic!("accepted baseline");
        };
        assert!(std::ptr::eq(fact.owner(), &first));
        assert!(!std::ptr::eq(fact.owner(), &second));
        Ok(())
    })
    .unwrap();
}

#[test]
fn invalid_query_is_sticky_even_when_callback_ignores_it() {
    let graph = owner(fixture());
    let invalid = coordinate(100, 0);
    let result = run(&graph, |view, budget| {
        assert!(
            matches!(view.read_at(invalid, budget), Err(Failure::Coordinate(actual)) if actual == invalid)
        );
        Ok(())
    });
    assert!(matches!(result, Err(Failure::Coordinate(actual)) if actual == invalid));
}
