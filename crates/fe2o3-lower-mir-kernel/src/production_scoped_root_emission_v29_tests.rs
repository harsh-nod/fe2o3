use super::*;

pub(super) fn emit_checked(
    source: &ExecutionLifecycleSourceV29<'_>,
    launch: &ProductionSourceLaunchRosterV1,
    root: RootInput<'_>,
    groups: u32,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = crate::with_checked_context_root_v29(
        source.owner,
        launch,
        root,
        budget,
        |checked, budget| {
            let mut closure = ReachableClosureBudgetV1::new(limits.max_blocks);
            let mut private = PrivateArrayLazyBudgetV1::new(1, limits.max_operations);
            let output = emit_pending_scoped_root_v29(
                checked,
                source,
                limits,
                &mut closure,
                &mut private,
                PrivateArrayPayloadV1::default(),
                budget,
            );
            assert!(
                private.active.is_none(),
                "array-free roots must keep lazy accounting inactive"
            );
            Ok(output)
        },
    )
    .map_err(|error| match error {
        crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
        _ => execution_lifecycle_error_v29(),
    })?;
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            assert_eq!(
                budget.storage(),
                floor,
                "failed assembly must release only its own reservations"
            );
            return Err(error);
        }
    };
    assert_eq!(output.ledger, budget.work_ledger_identity_v1());
    assert_eq!(budget.storage() - floor, output.retained_emission_storage);
    assert!(output.pending.additional_storage_bytes <= output.retained_emission_storage);
    assert_eq!(output.kernel.entry.as_str(), "lifecycle_fixture");
    assert_eq!(output.pending.function.id, output.kernel.entry);
    assert_eq!(
        output.kernel.workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    assert_eq!(
        output.kernel.domain,
        LaunchDomain::D1 {
            x: if groups == 1 {
                LaunchExtent::Static(64)
            } else {
                LaunchExtent::Dynamic
            },
        }
    );
    assert_eq!(output.pending.sidecars.rows.len(), 3);
    assert_eq!(output.pending.coordinates.root, ROOT);
    let body = output.pending.function.body.as_ref().unwrap();
    let mut blocks = BTreeSet::new();
    let mut values: BTreeSet<_> = body.parameters.iter().copied().collect();
    for block in &body.blocks {
        assert!(blocks.insert(block.id));
        for value in &block.parameters {
            assert!(values.insert(value.id));
        }
        for operation in &block.operations {
            if let OperationKind::Call { callee, .. } = &operation.kind {
                assert!(!callee.as_str().starts_with("__fe2o3_execution_"));
            }
            for value in &operation.results {
                assert!(values.insert(value.id));
            }
        }
    }
    let mut observations = Vec::new();
    let mut operations = 0;
    for (index, row) in output.pending.sidecars.rows.iter().enumerate() {
        assert_eq!(row.source_call_instance.unwrap().index(), index);
        assert!(row.instance_assert_origins.is_some());
        let events = row.lifecycle_events.as_ref().unwrap();
        assert_eq!(events.instance.index(), index);
        observations.extend_from_slice(&events.rows);
        operations += row.emitted_operations + events.rows.len();
    }
    assert!(operations <= limits.max_operations);
    assert!(matches!(
        observations[0].kind,
        DeferredLifecycleKindV29::Issue { .. }
    ));
    assert_eq!(observations[0].original_block, BlockId(0));
    let retained = output.retained_emission_storage;
    drop(output);
    budget.release_storage(retained)?;
    assert_eq!(budget.storage(), floor);
    Ok(observations)
}

fn fault(groups: u32, limits: ProductionSemanticKirLimitsV1) -> Fault {
    Fault::Orchestrated { groups, limits }
}

#[test]
fn checked_root_orchestrates_real_launch_and_retains_lifecycle_sidecars() {
    for branches in [false, true] {
        for groups in [1, 2, 7] {
            let result = run_lifecycle(
                branches,
                fault(groups, ProductionSemanticKirLimitsV1::default()),
                10_000_000,
                10_000_000,
            )
            .0
            .unwrap();
            assert_eq!(result.len(), if branches { 4 } else { 3 });
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Issue { .. }))
                    .count(),
                1
            );
            assert_eq!(
                result
                    .iter()
                    .filter(|row| matches!(row.kind, DeferredLifecycleKindV29::Derive { .. }))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn checked_root_orchestration_obeys_exact_ledger_boundaries() {
    let mode = fault(2, ProductionSemanticKirLimitsV1::default());
    let (result, work, storage) = run_lifecycle(true, mode, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(run_lifecycle(true, mode, work, storage).0.is_ok());
    assert!(run_lifecycle(true, mode, work - 1, storage).0.is_err());
    assert!(run_lifecycle(true, mode, work, storage - 1).0.is_err());
}

#[test]
fn checked_root_orchestration_bounds_whole_expansion_before_emission() {
    for limits in [
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 128, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 1, 128, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 0, 1024),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 128, 0),
    ] {
        assert!(
            matches!(
                run_lifecycle(true, fault(2, limits), 10_000_000, 10_000_000).0,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ),
            "{limits:?}"
        );
    }
}
