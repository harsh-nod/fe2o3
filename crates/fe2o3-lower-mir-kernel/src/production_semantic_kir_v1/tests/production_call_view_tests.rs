use super::*;

#[path = "production_call_transport_tests.rs"]
mod transport_tests;

#[path = "production_call_result_tests.rs"]
mod result_tests;

#[path = "production_canonical_calls_tests.rs"]
mod canonical_calls_tests;

#[test]
fn call_result_store_substitution_rejects_even_when_kir_types_still_verify() {
    for result in [ArgumentCallResult::Retained, ArgumentCallResult::Projected] {
        let source = call_owner(false, result);
        let limits = ProductionSemanticKirLimitsV1::default();
        let (mut graph, rows) = lower_argument_owner(&source, limits).unwrap();
        let call = rows
            .call_returns
            .iter()
            .find(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. }))
            .unwrap();
        let association = rows
            .lowered_functions
            .iter()
            .find(|row| {
                row.correspondence_owner == call.correspondence_owner
                    && row.semantic_function == call.semantic_function
            })
            .unwrap();
        let block = graph
            .functions
            .iter_mut()
            .find(|function| function.id == association.kernel_ir_function)
            .unwrap()
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.id.0 == call.semantic_block.index())
            .unwrap();
        let SemanticKirCallReturnKindV1::Call {
            call_operation,
            destination_end,
            ..
        } = call.kind
        else {
            unreachable!();
        };
        let OperationKind::Call { arguments, .. } = &block.operations[call_operation as usize].kind
        else {
            unreachable!();
        };
        let replacement = arguments[0];
        let OperationKind::Store { value, .. } =
            &mut block.operations[destination_end as usize - 1].kind
        else {
            unreachable!();
        };
        *value = replacement;
        verify_module(&graph).unwrap();
        check_both(
            &source,
            &graph,
            source.source_semantic().roots(),
            limits.max_blocks,
            &rows,
            Expected::CorrespondenceMismatch,
        );
    }
}

#[test]
fn additional_resultless_call_cannot_hide_in_a_valid_terminator_span() {
    let source = call_owner(false, ArgumentCallResult::Zero);
    let limits = ProductionSemanticKirLimitsV1::default();
    let (mut graph, mut rows) = lower_argument_owner(&source, limits).unwrap();
    let site = rows.call_returns[0];
    let SemanticKirCallReturnKindV1::Call { call_operation, .. } = site.kind else {
        panic!("expected root call");
    };
    let association = rows
        .lowered_functions
        .iter()
        .find(|row| {
            row.correspondence_owner == site.correspondence_owner
                && row.semantic_function == site.semantic_function
        })
        .unwrap();
    let block = graph
        .functions
        .iter_mut()
        .find(|function| function.id == association.kernel_ir_function)
        .unwrap()
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id.0 == site.semantic_block.index())
        .unwrap();
    block
        .operations
        .push(block.operations[call_operation as usize].clone());
    let span = rows
        .terminator_operation_spans
        .iter_mut()
        .find(|span| {
            span.correspondence_owner == site.correspondence_owner
                && span.semantic_function == site.semantic_function
                && span.semantic_block == site.semantic_block
        })
        .unwrap();
    span.operation_count += 1;
    verify_module(&graph).unwrap();
    check_both(
        &source,
        &graph,
        source.source_semantic().roots(),
        limits.max_blocks,
        &rows,
        Expected::CorrespondenceMismatch,
    );
}

#[test]
fn coordinated_return_graph_and_anchor_substitution_still_requires_full_replay() {
    let source = call_owner(false, ArgumentCallResult::Scalar);
    let limits = ProductionSemanticKirLimitsV1::default();
    let roster = argument_launch_roster(&source);
    let roots = materialization_launch_roots_v1(&source, &roster).unwrap();
    let (mut graph, mut rows) = lower_module(&source, limits, Some(&roots)).unwrap();
    let helper = graph
        .functions
        .iter_mut()
        .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
        .unwrap();
    let body = helper.body.as_mut().unwrap();
    let replacement = body.parameters[0];
    for block in &mut body.blocks {
        if let Some(Terminator::Return { values }) = &mut block.terminator {
            assert_ne!(values[0], replacement);
            values[0] = replacement;
        }
    }
    verify_module(&graph).unwrap();
    check_both(
        &source,
        &graph,
        source.source_semantic().roots(),
        limits.max_blocks,
        &rows,
        Expected::CorrespondenceMismatch,
    );
    for row in &mut rows.call_returns {
        if row.semantic_function.index() == 0 {
            let SemanticKirCallReturnKindV1::Return { components } = row.kind else {
                unreachable!();
            };
            for component in &mut rows.call_result_components[components.range().unwrap()] {
                let CallResultComponentV1::Return { input, .. } = component else {
                    unreachable!();
                };
                *input = replacement;
            }
        }
    }
    // Agreement between a graph and its anchors is deliberately insufficient.
    check_both(
        &source,
        &graph,
        source.source_semantic().roots(),
        limits.max_blocks,
        &rows,
        Expected::Accepted,
    );
    let owner = ProductionSemanticKirOwnerV1 {
        canonical_kernel_ir: ProductionCanonicalKernelIrV1::from_module(graph.clone()).unwrap(),
        semantic_ssa: source,
        module: RetainedProductionKirModuleV1::Legacy(graph),
        correspondence: rows,
        limits,
        launch_roots: Some(roots),
        generic_checks: Box::new([]),
    };
    assert!(matches!(
        owner.verify_equivalence(),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

fn call_owner(expanded: bool, result: ArgumentCallResult) -> ProductionSemanticSsaOwnerV1 {
    argument_call_owner(
        expanded,
        ArgumentTupleShape::Mixed,
        true,
        true,
        true,
        result,
        true,
    )
}

fn materialize(source: ProductionSemanticSsaOwnerV1) -> ProductionPreRankedKirOwnerV1 {
    let roster = argument_launch_roster(&source);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source,
        roster,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn inspect(
    view: &mut ProductionCallViewV1<'_, '_>,
    result: ArgumentCallResult,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(view.callee().association().semantic_function.index(), 0);
    let OperationKind::Call { arguments, .. } = &view.operation().kind else {
        panic!("not Call");
    };
    for (slot, actual) in arguments.iter().enumerate() {
        let joined = view.physical(slot)?.unwrap();
        assert_eq!(joined.caller_value(), *actual);
        assert_eq!(joined.parameter().slot(), slot);
        assert_eq!(joined.parameter().ty(), &Type::Scalar(ScalarType::U32));
    }
    assert!(view.physical(arguments.len())?.is_none());
    let mut nodes = 0;
    let mut zeros = 0;
    view.visit_arguments(|node| {
        nodes += 1;
        assert!(matches!(node.operand(), SemanticOperandV1::Copy(_)));
        zeros += usize::from(matches!(
            node.parameter().coverage(),
            ProductionArgumentCoverageV1::Zero
        ));
        Ok(())
    })?;
    assert!(nodes > 3 && zeros > 0);
    let scalar = result != ArgumentCallResult::Zero;
    assert_eq!(view.operation().results.len(), usize::from(scalar));
    match (result, view.destination()) {
        (
            ArgumentCallResult::Zero | ArgumentCallResult::Scalar,
            ProductionCallDestinationV1::Local,
        ) => {}
        (ArgumentCallResult::Retained, ProductionCallDestinationV1::Retained(store))
        | (ArgumentCallResult::Projected, ProductionCallDestinationV1::Projected { store, .. }) => {
            assert!(
                matches!(store.kind, OperationKind::Store { value, .. } if value == view.operation().results[0].id)
            );
            assert!(view.result_transport(0).is_none());
        }
        _ => panic!("destination classification changed"),
    }
    let mut returns = 0;
    view.visit_returns(|site| {
        returns += 1;
        assert_eq!(site.component_count(), usize::from(scalar));
        assert_eq!(site.input(0).is_some(), scalar);
        assert!(site.conversion(0).is_none());
        assert!(matches!(
            site.block().terminator,
            Some(Terminator::Return { .. })
        ));
        Ok(())
    })?;
    assert_eq!(returns, if scalar { 2 } else { 1 });
    if scalar {
        assert_eq!(
            view.physical(0)?.unwrap().caller_value(),
            view.physical(2)?.unwrap().caller_value()
        );
        assert_ne!(
            view.physical(0)?.unwrap().parameter().value(),
            view.physical(2)?.unwrap().parameter().value()
        );
    }
    let _ = view.edge_definitions()?;
    let _ = view.edge_arguments()?;
    Ok(())
}

#[test]
fn checked_calls_join_packed_expanded_shared_roots_and_all_destinations() {
    for expanded in [false, true] {
        for result in [
            ArgumentCallResult::Zero,
            ArgumentCallResult::Scalar,
            ArgumentCallResult::Retained,
            ArgumentCallResult::Projected,
        ] {
            let owner = materialize(call_owner(expanded, result));
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(23).unwrap();
            for root in [1, 2].map(SemanticFunctionIdV1::from_index) {
                for block in [0, 1].map(SemanticBlockIdV1::from_index) {
                    owner
                        .with_checked_call_v1(root, root, block, &mut budget, |view| {
                            assert_eq!(view.caller().correspondence_owner, root);
                            inspect(view, result)
                        })
                        .unwrap();
                    assert_eq!(budget.storage(), 23);
                }
            }
        }
    }
}

#[test]
fn checked_call_query_exact_budgets_callback_errors_and_foreign_sites_restore_floor() {
    let owner = materialize(call_owner(false, ArgumentCallResult::Scalar));
    let root = SemanticFunctionIdV1::from_index(1);
    let block = SemanticBlockIdV1::from_index(1);
    let run = |budget: &mut ArgumentBudgetV1<'_>| {
        owner.with_checked_call_v1(root, root, block, budget, |view| {
            inspect(view, ArgumentCallResult::Scalar)
        })
    };
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    run(&mut budget).unwrap();
    let required = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), 23);
    for (work_limit, storage_limit, accepted) in [
        (required.0, required.1, true),
        (required.0 - 1, required.1, false),
        (required.0, required.1 - 1, false),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        let result = run(&mut budget);
        if accepted {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
        assert_eq!(budget.storage(), 23);
    }
    let result: Result<(), _> = owner.with_checked_call_v1(root, root, block, &mut budget, |_| {
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), 23);
    for (caller, block) in [(2, 0), (1, 2), (1, u32::MAX)] {
        assert!(matches!(
            owner.with_checked_call_v1(
                root,
                SemanticFunctionIdV1::from_index(caller),
                SemanticBlockIdV1::from_index(block),
                &mut budget,
                |_| -> Result<(), ProductionSemanticKirErrorV1> {
                    panic!("invalid site reached consumer")
                }
            ),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn production_call_checker_rejects_missing_duplicate_foreign_and_changed_anchors() {
    for result in [
        ArgumentCallResult::Zero,
        ArgumentCallResult::Scalar,
        ArgumentCallResult::Retained,
        ArgumentCallResult::Projected,
    ] {
        let source = call_owner(false, result);
        let limits = ProductionSemanticKirLimitsV1::default();
        let (module, original) = lower_argument_owner(&source, limits).unwrap();
        let check = |rows: &SemanticKirCorrespondenceV1, expected| {
            check_both(
                &source,
                &module,
                source.source_semantic().roots(),
                limits.max_blocks,
                rows,
                expected,
            )
        };
        check(&original, Expected::Accepted);
        for mutation in 0..9 {
            let mut changed = original.clone();
            let call = changed
                .call_returns
                .iter()
                .position(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. }))
                .unwrap();
            let returned = changed
                .call_returns
                .iter()
                .position(|row| {
                    row.semantic_function.index() == 0
                        && matches!(row.kind, SemanticKirCallReturnKindV1::Return { .. })
                })
                .unwrap();
            match mutation {
                0 => changed.call_returns = changed.call_returns[1..].into(),
                1 => changed.call_returns[call + 1] = changed.call_returns[call],
                2 => {
                    changed.call_returns[call].correspondence_owner =
                        SemanticFunctionIdV1::from_index(99)
                }
                3 => {
                    changed.call_returns[call].semantic_function =
                        SemanticFunctionIdV1::from_index(0)
                }
                4 => changed.call_returns[call].semantic_block = SemanticBlockIdV1::from_index(99),
                5..=7 => {
                    let SemanticKirCallReturnKindV1::Call {
                        arguments_first,
                        call_operation,
                        destination_end,
                        ..
                    } = &mut changed.call_returns[call].kind
                    else {
                        unreachable!();
                    };
                    match mutation {
                        5 => *arguments_first = *call_operation + 1,
                        6 => *call_operation += 1,
                        _ => *destination_end = *call_operation,
                    }
                }
                8 => {
                    let SemanticKirCallReturnKindV1::Return { components } =
                        &mut changed.call_returns[returned].kind
                    else {
                        unreachable!();
                    };
                    *components = if components.count != 0 {
                        CallComponentSpanV1::EMPTY
                    } else {
                        CallComponentSpanV1 { first: 0, count: 1 }
                    };
                }
                _ => unreachable!(),
            }
            check(&changed, Expected::CorrespondenceMismatch);
        }
        if matches!(
            result,
            ArgumentCallResult::Retained | ArgumentCallResult::Projected
        ) {
            let mut changed = original.clone();
            for row in &mut changed.call_returns {
                if let SemanticKirCallReturnKindV1::Call { destination, .. } = &mut row.kind {
                    *destination = match *destination {
                        SemanticKirCallDestinationV1::Retained { pointer, access } => {
                            SemanticKirCallDestinationV1::Projected { pointer, access }
                        }
                        SemanticKirCallDestinationV1::Projected { pointer, access } => {
                            SemanticKirCallDestinationV1::Retained { pointer, access }
                        }
                        _ => unreachable!(),
                    };
                }
            }
            check(&changed, Expected::CorrespondenceMismatch);
        }
    }
}
