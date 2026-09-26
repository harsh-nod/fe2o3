thread_local! {
    static ORIGINAL_FOLDED_INDEX_FAULT_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ORIGINAL_FOLDED_INDEX_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn inspect_original_folded_index_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut rows = Vec::new();
    for (instance, output) in emitted
        .iter()
        .enumerate()
        .filter_map(|(i, row)| row.as_ref().map(|row| (i, row)))
    {
        let source = instances
            .instance(ProductionCallInstanceIdV1(instance))
            .unwrap()
            .declaration();
        for (ordinal, effect) in output.private_arrays.effects.iter().enumerate() {
            let PrivateArrayIndexV1::Local {
                original,
                physical_type,
                local,
                ..
            } = effect.original_index
            else {
                continue;
            };
            assert_eq!(physical_type, ScalarType::U32);
            let original_index = source.locals().get(local as usize).unwrap();
            assert_eq!(
                original_index.ty(),
                match effect.original_index {
                    PrivateArrayIndexV1::Local { semantic_type, .. } => semantic_type,
                    _ => unreachable!(),
                }
            );
            let body = output.function.body.as_ref().unwrap();
            let original_operation = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.results.iter().any(|row| row.id == original))
                .unwrap();
            assert_eq!(
                original_operation.kind,
                OperationKind::Constant(Constant::U32(0))
            );
            let offset_operation = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.results.iter().any(|row| row.id == effect.offset))
                .unwrap();
            assert_eq!(
                offset_operation.kind,
                OperationKind::Constant(Constant::Index(0))
            );
            assert_ne!(original, effect.offset);
            rows.push((instance, ordinal, original));
        }
    }
    assert_eq!(
        rows.len(),
        2,
        "both original helper selectors are actually folded"
    );
    assert_ne!(rows[0].0, rows[1].0);
    assert_ne!(
        rows[0].2, rows[1].2,
        "identical literals do not share original invocation identity"
    );
    let fault = ORIGINAL_FOLDED_INDEX_FAULT_V29.get();
    if fault != 0 {
        let output = emitted[rows[0].0].as_mut().unwrap();
        let PrivateArrayIndexV1::Local { original, .. } =
            &mut output.private_arrays.effects[rows[0].1].original_index
        else {
            unreachable!()
        };
        *original = rows[1].2;
        if fault == 2 {
            // Change the definition that the immutable per-instance selector
            // claim actually names. Merely changing the legacy effect record
            // above does not change that source-owned claim.
            let definition = output
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .flat_map(|operation| &mut operation.results)
                .find(|result| result.id == rows[0].2)
                .unwrap();
            definition.id = rows[1].2;
        } else {
            assert_eq!(fault, 1);
        }
    }
    ORIGINAL_FOLDED_INDEX_VISITS_V29.set(ORIGINAL_FOLDED_INDEX_VISITS_V29.get() + 1);
    OBSERVED.set(OBSERVED.get() + 1);
    Ok(())
}

#[test]
fn original_folded_selectors_finish_repeated_and_branched_helpers_without_cast_nodes() {
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(0);
    for branches in [false, true] {
        ORIGINAL_FOLDED_INDEX_VISITS_V29.set(0);
        let (result, _, _, completed) = run_original_source_fixture_v29(
            || fixtures::array_owner(branches),
            branches,
            false,
            1,
            inspect_original_folded_index_v29,
            10_000_000,
            10_000_000,
        );
        assert!(
            result.is_ok() && completed,
            "branches={branches}: {result:?}"
        );
        assert_eq!(OBSERVED.get(), 3);
        assert_eq!(ORIGINAL_FOLDED_INDEX_VISITS_V29.get(), 3);
    }
}

#[test]
fn original_folded_selector_does_not_accept_another_invocations_equal_literal() {
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(0);
    let (positive, _, _, completed) = run_original_source_fixture_v29(
        || fixtures::array_owner(false),
        false,
        false,
        1,
        inspect_original_folded_index_v29,
        10_000_000,
        10_000_000,
    );
    assert!(positive.is_ok() && completed, "{positive:?}");
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(2);
    ORIGINAL_FOLDED_INDEX_VISITS_V29.set(0);
    let (refused, _, _, completed) = run_original_source_fixture_v29(
        || fixtures::array_owner(false),
        false,
        false,
        1,
        inspect_original_folded_index_v29,
        10_000_000,
        10_000_000,
    );
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(0);
    assert!(!completed, "{refused:?}");
    assert_eq!(ORIGINAL_FOLDED_INDEX_VISITS_V29.get(), 1);
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "execution availability differs from its source SSA instance",
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

#[test]
fn original_folded_selector_ignores_only_the_legacy_unconsumed_index_annotation() {
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(1);
    ORIGINAL_FOLDED_INDEX_VISITS_V29.set(0);
    let (result, _, _, completed) = run_original_source_fixture_v29(
        || fixtures::array_owner(false),
        false,
        false,
        1,
        inspect_original_folded_index_v29,
        10_000_000,
        10_000_000,
    );
    ORIGINAL_FOLDED_INDEX_FAULT_V29.set(0);
    assert!(result.is_ok() && completed, "{result:?}");
    assert_eq!(ORIGINAL_FOLDED_INDEX_VISITS_V29.get(), 3);
    assert_eq!(OBSERVED.get(), 3);
}
