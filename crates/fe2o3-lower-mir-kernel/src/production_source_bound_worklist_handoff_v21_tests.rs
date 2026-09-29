fn launch_inputs_v21(count: usize) -> Vec<fe2o3_kernel_ir::CanonicalFormalLaunchInputV19> {
    vec![
        fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::PhysicalEnvelope(
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [192, 1, 1]
            }
        );
        count
    ]
}

#[test]
fn bound_worklist_checks_nonempty_private_memory_and_retains_nominal_output() {
    for (factory, changed) in [
        (private_entry_neutral_owner_v20 as fn() -> _, true),
        (private_entry_non_neutral_owner_v20, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                assert!(
                    private_entry_typed_memory_census_v20(source.canonical(budget)?)
                        .into_iter()
                        .all(|count| count > 0)
                );
                let launches = launch_inputs_v21(source.root_count(budget)?);
                let handoff = source.checked_bound_private_worklist_output_v21(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert!(
                    private_entry_typed_memory_census_v20(output.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                assert_eq!(output.execution().policy_version(), 9);
                assert_eq!(output.report().passes()[0].changed(), changed);
                assert_eq!(
                    output.input_audit_bytes(),
                    source.canonical(budget)?.canonical_bytes()
                );
                let mut reads = 0;
                let mut writes = 0;
                for op in output
                    .owner()
                    .module()
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|b| &b.blocks)
                    .flat_map(|b| &b.operations)
                {
                    reads += usize::from(matches!(
                        op.kind,
                        OperationKind::Load { .. }
                            | OperationKind::Storage(
                                fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }
                            )
                    ));
                    writes += usize::from(matches!(
                        op.kind,
                        OperationKind::Store { .. }
                            | OperationKind::Storage(
                                fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }
                            )
                    ));
                }
                assert!(reads > 0 && writes > 0);
                assert_eq!(
                    handoff.formal_context_v21(budget)?,
                    (
                        launches.as_slice(),
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64
                    )
                );
                assert!(!handoff.ranked_verification_is_complete());
                assert!(!handoff.grants_artifact_or_launch_authority());
                assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionBoundPrivateHandoffErrorV21>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn bound_worklist_foreign_account_cannot_observe_or_dispose_actual_owner() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_neutral_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = launch_inputs_v21(source.root_count(budget)?);
        let handoff = source.checked_bound_private_worklist_output_v21(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            budget,
        )?;
        let retained = handoff.retained_storage(budget)?;
        let original_floor = budget.storage();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        foreign.reserve_storage(original_floor)?;
        assert!(matches!(
            handoff.output(&foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(matches!(
            handoff.discard(&mut foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(foreign.storage(), original_floor);
        assert_eq!(budget.storage(), original_floor);
        // The actual owner has been dropped, but the rejected custody operation
        // must not refund its original account. The containing scope stays denied.
        assert!(retained > 0);
        completed.set(true);
        Ok::<_, ProductionBoundPrivateHandoffErrorV21>(())
    });
    assert!(completed.get());
    assert!(matches!(
        result,
        Err(ProductionBoundPrivateHandoffErrorV21::Check(
            ProductionBoundPrivateCheckErrorV21::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            )
        ))
    ));
    assert!(budget.storage() > MODULE_FLOOR);
}
