#[derive(Clone, Copy)]
enum MixedPureCseFixtureV26 {
    Shared,
    ReadModifyWrite,
    Duplicate,
    Different,
}

fn mixed_pure_cse_resource_v26(error: ProductionMixedSourceHandoffErrorV26) -> ArgumentResourceV1 {
    fn nested(error: &(dyn std::error::Error + 'static)) -> ArgumentResourceV1 {
        let mut current = Some(error);
        while let Some(error) = current {
            if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
                return *resource;
            }
            if let Some(work) =
                error.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>()
            {
                return ArgumentResourceV1::Work(*work);
            }
            current = error.source();
        }
        panic!("typed resource refusal required: {error:?}");
    }
    fn source(error: ProductionSourceOwnedViewErrorV18) -> ArgumentResourceV1 {
        source_slot_tests::original_repeated_source_resource_v29(error)
    }
    fn check(error: ProductionMixedSourceCheckErrorV26) -> ArgumentResourceV1 {
        match error {
            ProductionMixedSourceCheckErrorV26::Source(error) => source(error),
            ProductionMixedSourceCheckErrorV26::Ranked(error) => nested(&error),
            ProductionMixedSourceCheckErrorV26::Native(
                ProductionSourceNativeLifecycleErrorV18::Source(error)
                | ProductionSourceNativeLifecycleErrorV18::SourceAfterNative {
                    source: error, ..
                },
            ) => source(error),
            ProductionMixedSourceCheckErrorV26::Native(error) => nested(&error),
        }
    }
    match error {
        ProductionMixedSourceHandoffErrorV26::Check(error) => check(error),
        ProductionMixedSourceHandoffErrorV26::Optimization(
            ProductionSourceOptimizationErrorV18::Source(error),
        ) => source(error),
        ProductionMixedSourceHandoffErrorV26::Optimization(
            ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error),
            ),
        ) => check(error),
        ProductionMixedSourceHandoffErrorV26::Optimization(error) => nested(&error),
    }
}

fn assert_mixed_pure_cse_short_v26(
    error: ProductionMixedSourceHandoffErrorV26,
    work_short: bool,
    work_limit: usize,
    storage_limit: usize,
) {
    match mixed_pure_cse_resource_v26(error) {
        ArgumentResourceV1::Work(error) if work_short => {
            assert_eq!(error.limit(), work_limit);
            assert!(error.actual() > work_limit);
        }
        ArgumentResourceV1::Storage(error) if !work_short => {
            assert_eq!(error.limit(), storage_limit);
            assert!(error.actual() > storage_limit);
        }
        other => panic!("wrong one-short refusal phase: {other:?}"),
    }
}

fn mixed_pure_cse_source_v26(duplicate: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::ReadValue);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    let temporary =
        |index| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], scalar).unwrap();
    let constant = |bits| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
        ))
    };
    let assign = |target, operation, left, right| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                temporary(target),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    },
                ),
            )),
        )
    };
    let mut locals = original.locals().to_vec();
    assert_eq!(locals.len(), 9);
    for identity in [81, 82, 83] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([identity; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[3].statements().to_vec();
    let store = statements.pop().unwrap();
    let SemanticStatementKindV1::Store(store) = store.kind() else {
        panic!("RMW store");
    };
    // Ordinary potentially trapping arithmetic is deliberately outside CSE.
    // These total bitwise operations exercise its actual admitted grammar.
    statements.push(assign(
        9,
        SemanticBinaryOpV1::BitXor,
        SemanticOperandV1::Copy(temporary(8)),
        constant(1),
    ));
    statements.push(assign(
        10,
        SemanticBinaryOpV1::BitXor,
        SemanticOperandV1::Copy(temporary(8)),
        constant(if duplicate { 1 } else { 2 }),
    ));
    statements.push(assign(
        11,
        SemanticBinaryOpV1::BitOr,
        SemanticOperandV1::Copy(temporary(9)),
        SemanticOperandV1::Copy(temporary(10)),
    ));
    statements.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            store.destination().clone(),
            SemanticOperandV1::Copy(temporary(11)),
            store.volatility(),
            store.atomic(),
        )),
    ));
    blocks[3] = SemanticBasicBlockV1::new(
        blocks[3].identity(),
        blocks[3].source(),
        statements,
        blocks[3].terminator().clone(),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run_mixed_pure_cse_handoff_v26(
    fixture: MixedPureCseFixtureV26,
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionMixedSourceHandoffErrorV26>,
    usize,
    usize,
    bool,
) {
    let exclusive = !matches!(fixture, MixedPureCseFixtureV26::Shared);
    let owner = match fixture {
        MixedPureCseFixtureV26::Shared => descriptor_source_owner(DescriptorCase::READ),
        MixedPureCseFixtureV26::ReadModifyWrite => {
            issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic)
        }
        MixedPureCseFixtureV26::Duplicate => mixed_pure_cse_source_v26(true),
        MixedPureCseFixtureV26::Different => mixed_pure_cse_source_v26(false),
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
    let mut completed = false;
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
            let mut launches = vec![
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1]
                };
                source.root_count(budget)?
            ];
            let width = if fault == 1 {
                fe2o3_kernel_ir::FormalIndexWidth::Unknown
            } else {
                fe2o3_kernel_ir::FormalIndexWidth::Bits64
            };
            if fault == 2 {
                launches.clear();
            }
            if fault == 3 {
                launches[0] = fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 2,
                    extents: [64, 2, 1],
                };
            }
            let handoff = source.conditional_mixed_pure_cse_output_v26(
                ProductionKernelArgumentAbiInputV18 {
                    roots: if fault == 4 { &[] } else { &roots },
                },
                &launches,
                width,
                budget,
            )?;
            let inspection = (|| -> Result<(), ProductionMixedSourceHandoffErrorV26> {
                if fault == 5 {
                    budget.charge_work(work_limit.checked_sub(budget.work()).unwrap())?;
                }
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                handoff.check_original_argument_abi_v26(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                let output = handoff.output(budget)?;
                assert_eq!(output.execution().policy_version(), 10);
                use fe2o3_pliron::PlironOptimizationPassV1 as Pass;
                assert_eq!(
                    output
                        .report()
                        .passes()
                        .iter()
                        .map(|pass| pass.pass())
                        .collect::<Vec<_>>(),
                    [
                        Pass::IntegerNeutralWorklistCanonicalization,
                        Pass::LocalPureCommonSubexpressionElimination,
                        Pass::DominancePureCommonSubexpressionElimination,
                        Pass::DeadCodeElimination,
                    ]
                );
                assert_eq!(
                    &output.execution().canonical_bytes()[..8],
                    &[10, 0, 1, 0, 4, 0, 18, 0]
                );
                let original = source.canonical(budget)?;
                let graph = output.owner();
                assert_eq!(
                    original.module().functions.len(),
                    graph.module().functions.len()
                );
                for (before, after) in original
                    .module()
                    .functions
                    .iter()
                    .zip(&graph.module().functions)
                {
                    match (&before.body, &after.body) {
                        (Some(before), Some(after)) => {
                            assert_eq!(before.blocks.len(), after.blocks.len());
                            for (before, after) in before.blocks.iter().zip(&after.blocks) {
                                assert_eq!(before.id, after.id);
                                assert_eq!(
                                    before.terminator.as_ref().unwrap().successors(),
                                    after.terminator.as_ref().unwrap().successors()
                                );
                            }
                        }
                        (None, None) => (),
                        _ => panic!("fixed pure policy changed function body roster"),
                    }
                }
                if matches!(
                    fixture,
                    MixedPureCseFixtureV26::Duplicate | MixedPureCseFixtureV26::Different
                ) {
                    let count = |module: &fe2o3_kernel_ir::Module| {
                        module
                            .functions
                            .iter()
                            .filter_map(|function| function.body.as_ref())
                            .flat_map(|body| &body.blocks)
                            .flat_map(|block| &block.operations)
                            .filter(|operation| {
                                matches!(operation.kind, OperationKind::Binary { .. })
                            })
                            .count()
                    };
                    assert_eq!(count(original.module()), 3);
                    assert_eq!(
                        count(graph.module()),
                        if matches!(fixture, MixedPureCseFixtureV26::Duplicate) {
                            2
                        } else {
                            3
                        }
                    );
                    if matches!(fixture, MixedPureCseFixtureV26::Duplicate) {
                        assert!(output.report().passes()[1].changed());
                    }
                }
                assert_eq!(
                    output.input_audit_bytes(),
                    source.canonical(budget)?.canonical_bytes()
                );
                let premises = handoff.runtime_premises(budget)?;
                let occurrences = handoff.runtime_occurrences(budget)?;
                assert_eq!(premises.len(), 2);
                let unused = premises
                    .iter()
                    .find(|row| row.original_argument() == 1)
                    .unwrap();
                assert_eq!(unused.access_counts(), [0, 0]);
                assert!(!unused.requires_valid_aligned_extent());
                assert!(!unused.requires_initialized_extent());
                assert!(!unused.requires_exclusive_nonoverlapping_runtime_binding());
                assert!(!occurrences.is_empty());
                let mut reads = 0usize;
                let mut writes = 0usize;
                for occurrence in occurrences {
                    let premise = &premises[occurrence.premise_index()];
                    assert_eq!(premise.launch(), launches[premise.root()]);
                    assert_eq!(premise.index_width(), width);
                    assert!(premise.requires_valid_aligned_extent());
                    assert!(occurrence.requires_address_formation_domain());
                    assert!(!occurrence.grants_artifact_or_launch_authority());
                    reads += usize::from(!occurrence.domain().writing());
                    writes += usize::from(occurrence.domain().writing());
                    if occurrence.domain().writing() {
                        assert!(premise.source_exclusive_contract());
                        assert!(premise.requires_exact_launch_binding());
                        assert!(premise.requires_exclusive_nonoverlapping_runtime_binding());
                        assert!(occurrence.invocation_projection().is_some());
                    } else {
                        assert!(premise.requires_initialized_extent());
                    }
                }
                assert!(reads > 0);
                assert_eq!(writes > 0, exclusive);
                assert_eq!(
                    premises.iter().map(|p| p.access_counts()[0]).sum::<usize>(),
                    reads
                );
                assert_eq!(
                    premises.iter().map(|p| p.access_counts()[1]).sum::<usize>(),
                    writes
                );
                assert!(!handoff.runtime_requirements_are_discharged());
                assert!(!handoff.ranked_verification_is_complete());
                assert!(!handoff.grants_artifact_or_launch_authority());
                assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                Ok(())
            })();
            let discarded = handoff.discard(budget);
            inspection?;
            discarded?;
            assert_eq!(budget.storage(), floor);
            completed = true;
            Ok::<_, ProductionMixedSourceHandoffErrorV26>(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn mixed_pure_cse_runs_policy10_for_genuine_shared_and_read_modify_write_sources() {
    for fixture in [
        MixedPureCseFixtureV26::Shared,
        MixedPureCseFixtureV26::ReadModifyWrite,
    ] {
        let (result, _, _, completed) = run_mixed_pure_cse_handoff_v26(
            fixture,
            0,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "{result:?}");
        assert!(completed);
    }
}

#[test]
fn mixed_pure_cse_merges_equal_live_bitwise_values_but_not_different_operands() {
    for fixture in [
        MixedPureCseFixtureV26::Duplicate,
        MixedPureCseFixtureV26::Different,
    ] {
        let (result, _, _, completed) = run_mixed_pure_cse_handoff_v26(
            fixture,
            0,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "{result:?}");
        assert!(completed);
    }
}

#[test]
fn mixed_pure_cse_refuses_unknown_width_missing_or_wrong_launch_and_incomplete_abi() {
    for fault in 1..=4 {
        let (result, _, _, completed) = run_mixed_pure_cse_handoff_v26(
            MixedPureCseFixtureV26::ReadModifyWrite,
            fault,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_err(), "fault={fault}");
        assert!(!completed);
    }
}

#[test]
fn mixed_pure_cse_has_exact_and_one_short_whole_transaction_resources() {
    let fixture = MixedPureCseFixtureV26::Duplicate;
    let (result, work, storage, completed) =
        run_mixed_pure_cse_handoff_v26(fixture, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
    let (result, exact_work, exact_storage, completed) =
        run_mixed_pure_cse_handoff_v26(fixture, 0, work, storage);
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
    assert_eq!((exact_work, exact_storage), (work, storage));
    for (work_limit, storage_limit) in [(work - 1, storage), (work, storage - 1)] {
        let (result, _, _, _) =
            run_mixed_pure_cse_handoff_v26(fixture, 0, work_limit, storage_limit);
        assert_mixed_pure_cse_short_v26(
            result.unwrap_err(),
            work_limit < work,
            work_limit,
            storage_limit,
        );
    }
}

#[test]
fn mixed_pure_cse_late_query_refusal_disposes_the_actual_checked_output() {
    for fixture in [
        MixedPureCseFixtureV26::Shared,
        MixedPureCseFixtureV26::ReadModifyWrite,
    ] {
        let (result, _, _, completed) = run_mixed_pure_cse_handoff_v26(
            fixture,
            5,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(!completed);
        assert!(
            matches!(
                result,
                Err(ProductionMixedSourceHandoffErrorV26::Check(
                    ProductionMixedSourceCheckErrorV26::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_))
                    )
                ))
            ),
            "{result:?}"
        );
    }
}
