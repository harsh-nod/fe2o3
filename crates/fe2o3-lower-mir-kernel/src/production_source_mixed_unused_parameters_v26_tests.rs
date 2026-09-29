fn run_mixed_unused_parameters_v26(
    exclusive: bool,
    used: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionMixedSourceHandoffErrorV26>,
    usize,
    usize,
    bool,
) {
    let owner = if exclusive {
        issued_descriptor_role_owner_with_access_v18(DescriptorRoleSourceV18::Constant, used)
    } else {
        assert!(used);
        descriptor_source_owner(DescriptorCase::READ)
    };
    let abi = if exclusive {
        issued_descriptor_role_abi_v18(&owner)
    } else {
        kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner)
    };
    let semantic = owner.source_semantic();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let roots = abi.roots();
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut reached = false;
    let result = (|| -> Result<(), ProductionMixedSourceHandoffErrorV26> {
        let prepared =
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let launches = [fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }];
            let handoff = source.conditional_mixed_worklist_output_v26(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            assert_eq!(handoff.output(budget)?.execution().policy_version(), 9);
            let premises = handoff.runtime_premises(budget)?;
            let occurrences = handoff.runtime_occurrences(budget)?;
            assert_eq!(
                premises.len(),
                2,
                "both original slice declarations survive"
            );
            for argument in 0..2 {
                let premise = premises
                    .iter()
                    .find(|row| row.original_argument() == argument)
                    .expect("complete original slice roster");
                let counts = premise.access_counts();
                assert_eq!(premise.root(), 0);
                assert_eq!(premise.source_exclusive_contract(), exclusive);
                if argument == 1 || !used {
                    assert_eq!(counts, [0, 0]);
                    assert!(!premise.requires_valid_aligned_extent());
                    assert!(!premise.requires_initialized_extent());
                    assert!(!premise.requires_exclusive_nonoverlapping_runtime_binding());
                    assert!(!premise.requires_exact_launch_binding());
                    assert_eq!(premise.invocation_axis(), None);
                } else {
                    assert!(counts[0] + counts[1] > 0);
                    assert!(premise.requires_valid_aligned_extent());
                }
                assert_eq!(
                    occurrences
                        .iter()
                        .filter(|row| {
                            premises[row.premise_index()].original_argument() == argument
                        })
                        .count(),
                    counts[0] + counts[1]
                );
            }
            assert!(!handoff.runtime_requirements_are_discharged());
            assert!(!handoff.grants_artifact_or_launch_authority());
            assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
            handoff.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            reached = true;
            Ok::<_, ProductionMixedSourceHandoffErrorV26>(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), reached)
}

#[test]
fn mixed_source_census_retains_unused_shared_and_exclusive_parameters() {
    for (exclusive, used) in [(false, true), (true, true), (true, false)] {
        let (result, _, _, reached) = run_mixed_unused_parameters_v26(
            exclusive,
            used,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(
            result.is_ok(),
            "exclusive={exclusive}, used={used}: {result:?}"
        );
        assert!(reached);
    }
}

#[test]
fn mixed_source_unused_parameter_census_has_exact_transaction_limits() {
    for (exclusive, used) in [(false, true), (true, false)] {
        let (result, work, peak, reached) = run_mixed_unused_parameters_v26(
            exclusive,
            used,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "{result:?}");
        assert!(reached);
        let (result, exact_work, exact_peak, reached) =
            run_mixed_unused_parameters_v26(exclusive, used, work, peak);
        assert!(result.is_ok(), "{result:?}");
        assert!(reached);
        assert_eq!((exact_work, exact_peak), (work, peak));
        for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
            let (result, _, _, _) = run_mixed_unused_parameters_v26(exclusive, used, work, storage);
            assert!(
                result.is_err(),
                "one-short complete slice census was admitted"
            );
        }
    }
}

#[test]
fn slice_parameter_census_refuses_missing_and_same_count_rebound_unused_declarations() {
    for fault in 0..5 {
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_slice_parameter_census_refusal_v26(
                    original, optimized, budget, fault, &reached,
                )
            },
        );
        assert!(reached.get(), "fault={fault}: {result:?}");
        assert!(result.is_err(), "fault={fault}");
    }
}
