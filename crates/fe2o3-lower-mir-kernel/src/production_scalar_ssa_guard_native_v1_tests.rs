use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1 as BlockRef, CanonicalKirOperationRefV1 as OperationRef,
};

fn with_native(
    run: impl FnOnce(&ProductionScalarSsaEmissionOwnerV1, &SourceReport, &mut Budget<'_>) -> Result<()>,
) {
    let (owner, source) = materialize(false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    resources::scoped(&mut budget, |budget| run(&owner, &source, budget)).unwrap();
    assert_eq!(budget.storage(), floor);
}

fn operation_view<'a>(actual: &OperationRef<'_>, operation: &'a Operation) -> OperationRef<'a> {
    OperationRef {
        coordinate: actual.coordinate,
        operation,
        results: actual.results.clone(),
        operands: actual.operands.clone(),
        effects: actual.effects.clone(),
    }
}

fn block_view<'a>(actual: &'a BlockRef<'a>, terminator: &'a Terminator) -> BlockRef<'a> {
    BlockRef {
        coordinate: actual.coordinate,
        block: actual.block,
        terminator,
        parameters: actual.parameters.clone(),
        operations: actual.operations.clone(),
        terminator_uses: actual.terminator_uses.clone(),
        edges: actual.edges.clone(),
    }
}

#[test]
fn exact_native_comparison_rejects_operand_order_predicate_width_and_coordinate_changes() {
    with_native(|owner, source, budget| {
        let (report, receipt) = run(owner, source, budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let fact = joined(report.rows()[0].outcome())?;
        let (inventory, receipt) = Inventory::derive(owner.original().executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let actual = inventory.operations().iter().find(|row|
            matches!(fact.condition(), Definition::Result { operation, .. } if operation == row.coordinate)).unwrap();
        let OperationKind::Compare { lhs, rhs, .. } = actual.operation.kind else {
            return Err(Error::Mismatch("genuine fixture comparison"));
        };
        assert_ne!(lhs, rhs);
        let header = fact.then_edge().source;
        let expected = recipe::check_comparison(actual, header, lhs, rhs, budget)?;
        assert_eq!(expected.0, fact.condition());
        assert!(matches!(
            recipe::check_comparison(actual, header, rhs, lhs, budget),
            Err(Error::Mismatch(
                "guard actual N ordered comparison operands"
            ))
        ));
        assert!(matches!(
            recipe::check_comparison(actual, fact.exit(), lhs, rhs, budget),
            Err(Error::Mismatch(
                "guard actual N ordered comparison operands"
            ))
        ));
        // These are inert hostile component inputs, never admitted graph owners.
        budget.reserve_storage(size_of::<Operation>())?;
        let mut results = Vec::new();
        reserve(&mut results, 1, budget)?;
        push(&mut results, actual.operation.results[0].clone(), budget)?;
        let mut wrong = Operation::new(
            results,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs,
                rhs,
            },
        );
        assert!(matches!(
            recipe::check_comparison(&operation_view(actual, &wrong), header, lhs, rhs, budget),
            Err(Error::Mismatch("guard actual N LessThan recipe"))
        ));
        wrong.kind = OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        };
        wrong.results[0].ty = Type::Scalar(fe2o3_kernel_ir::ScalarType::U32);
        assert!(matches!(
            recipe::check_comparison(&operation_view(actual, &wrong), header, lhs, rhs, budget),
            Err(Error::Mismatch(
                "guard actual N ordered comparison operands"
            ))
        ));
        wrong.results.clear();
        assert!(matches!(
            recipe::check_comparison(&operation_view(actual, &wrong), header, lhs, rhs, budget),
            Err(Error::Mismatch("guard single Bool compare result"))
        ));
        Ok(())
    });
}

#[test]
fn exact_native_branch_rejects_wrong_condition_swapped_polarity_and_edge_ranges() {
    with_native(|owner, source, budget| {
        let (report, receipt) = run(owner, source, budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let fact = joined(report.rows()[0].outcome())?;
        let mut incoming = Incoming::prepare(owner, budget)?;
        let (inventory, receipt) = Inventory::derive(owner.original().executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        incoming.populate(&inventory, budget)?;
        let header = inventory
            .blocks()
            .iter()
            .find(|row| row.coordinate == fact.then_edge().source)
            .unwrap();
        let body = inventory
            .blocks()
            .iter()
            .find(|row| row.coordinate == fact.body())
            .unwrap();
        let exit = inventory
            .blocks()
            .iter()
            .find(|row| row.coordinate == fact.exit())
            .unwrap();
        let Terminator::ConditionalBranch { condition, .. } = header.terminator else {
            return Err(Error::Mismatch("genuine fixture conditional branch"));
        };
        let expected = recipe::check_branch(
            header, body, exit, *condition, &inventory, &incoming, budget,
        )?;
        assert_eq!(expected, (fact.then_edge(), fact.else_edge()));
        assert!(matches!(
            recipe::check_branch(
                header,
                body,
                exit,
                ValueId(u32::MAX),
                &inventory,
                &incoming,
                budget
            ),
            Err(Error::Mismatch(
                "guard actual N polarity/unique predecessor"
            ))
        ));
        assert!(matches!(
            recipe::check_branch(
                header, exit, body, *condition, &inventory, &incoming, budget
            ),
            Err(Error::Mismatch(
                "guard actual N polarity/unique predecessor"
            ))
        ));
        assert!(matches!(
            recipe::check_branch(
                header, body, body, *condition, &inventory, &incoming, budget
            ),
            Err(Error::Mismatch(
                "guard actual N polarity/unique predecessor"
            ))
        ));
        let inverse = Terminator::ConditionalBranch {
            condition: *condition,
            then_target: exit.block.id,
            then_arguments: Vec::new(),
            else_target: body.block.id,
            else_arguments: Vec::new(),
        };
        let mut wrong = block_view(header, &inverse);
        assert!(matches!(
            recipe::check_branch(
                &wrong, body, exit, *condition, &inventory, &incoming, budget
            ),
            Err(Error::Mismatch(
                "guard actual N polarity/unique predecessor"
            ))
        ));
        wrong.terminator = header.terminator;
        wrong.edges.end = wrong.edges.start + 1;
        assert!(matches!(
            recipe::check_branch(
                &wrong, body, exit, *condition, &inventory, &incoming, budget
            ),
            Err(Error::Mismatch("guard actual N two edge occurrences"))
        ));
        wrong.edges.end = usize::MAX;
        assert!(matches!(
            recipe::check_branch(
                &wrong, body, exit, *condition, &inventory, &incoming, budget
            ),
            Err(Error::Mismatch("guard actual N branch edge range"))
        ));
        Ok(())
    });
}
