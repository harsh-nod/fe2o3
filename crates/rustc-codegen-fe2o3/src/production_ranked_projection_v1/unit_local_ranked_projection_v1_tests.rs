#[test]
fn source_launch_memory_free_raw_empty_helper_uses_the_normal_projector() {
    let owner = materialized_helper_v1(false);
    assert_eq!(
        owner.helper_source_policy_v1(),
        fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
    );
    assert_eq!(owner.empty_effect_helpers().iter().count(), 1);
    let identity = *owner.executable().canonical().identity();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &[ranked_root_input_1d(A_NAME, 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    assert_eq!(program.roots.len(), 1);
    let root = &program.roots[0];
    assert!(root.all_kernel_checks_are_clean());
    assert!(root.access_sources.is_empty());
    assert!(root.executable_effect_sources.is_empty());
    assert!(
        root.verification
            .ordinary()
            .expect("ordinary test root")
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .any(|operation| matches!(
                operation,
                ProductionRankedOperationV1::ExecutionLayout { .. }
            ))
    );
    assert!(
        !root
            .verification
            .ordinary()
            .expect("ordinary test root")
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .any(|operation| matches!(
                operation,
                ProductionRankedOperationV1::InvocationIndex { .. }
                    | ProductionRankedOperationV1::Access { .. }
                    | ProductionRankedOperationV1::AtomicAccess { .. }
                    | ProductionRankedOperationV1::AllocationEffect { .. }
            ))
    );
    assert_eq!(
        *program.materialized.executable().canonical().identity(),
        identity
    );
    assert_eq!(
        program.materialized.empty_effect_helpers().iter().count(),
        1
    );
    assert!(!program.materialized.grants_artifact_or_launch_authority());
}

#[test]
fn source_launch_memory_free_unresolved_call_keeps_its_exact_error() {
    let owner = materialized_unit_local_helper_v1();
    let semantic = owner.semantic_ssa().source_semantic();
    let selection = semantic
        .select_kernel_body_for_root_v1(semantic.roots()[0])
        .unwrap();
    let effects = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(),
        semantic.functions(),
        semantic.callables(),
    )
    .unwrap();
    assert!(!effects.is_exact_empty(SemanticFunctionIdV1::from_index(1)));
    // Isolated negative consumer test: the genuine source owner is paired with
    // a component checker that cannot supply a local-call relation. This does
    // not manufacture an accepted projection or bypass the production query.
    let result = with_budgeted_component_facts_v1(|facts| {
        project_and_verify_ranked_root_v1(
            semantic,
            &effects,
            selection,
            &ranked_root_input_1d(A_NAME, 247, 64),
            owner.source_launch().roots()[0],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            facts,
        )
    });
    assert!(matches!(
        result,
        Err(
            ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                block: 0,
                callee: 1,
                tail: false,
                ..
            }
        )
    ));
}

fn unit_local_ranked_private_stores_v1(
    owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
) -> usize {
    owner
        .executable()
        .module()
        .functions
        .iter()
        .map(|function| {
            function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .map(|block| {
                    block.operations.iter().filter(|operation| {
                matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Store { access, .. }
                    if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)
            }).count()
                })
                .sum::<usize>()
        })
        .sum()
}

#[test]
fn unit_local_ranked_summary_join_keeps_local_obligations_in_both_orders() {
    // Aggregation algebra only: no forged source/call row or mixed-policy owner.
    for initial in [
        DefinedCallableEmptyEffectDecisionV1::Rejected,
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly,
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar,
    ] {
        for local_first in [false, true] {
            let mut decision = initial;
            if local_first {
                decision = DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall;
            }
            materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut decision).unwrap();
            if !local_first {
                decision = DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall;
            }
            materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut decision).unwrap();
            assert_eq!(
                decision,
                DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall
            );
            let summaries = DefinedCallableEmptyEffectSummariesV1 {
                decisions: vec![decision].into_boxed_slice(),
            };
            assert!(!summaries.is_exact_empty(SemanticFunctionIdV1::from_index(0)));
            assert!(
                !summaries.is_exact_empty_deterministic_scalar(SemanticFunctionIdV1::from_index(0))
            );
        }
    }
    let mut unfinished = DefinedCallableEmptyEffectDecisionV1::Unknown;
    assert!(matches!(
        materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut unfinished),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "materialized helper effect fact has an unfinished source summary",
        ))
    ));
    assert_eq!(unfinished, DefinedCallableEmptyEffectDecisionV1::Unknown);
}

#[test]
fn unit_local_ranked_production_projection_preserves_actual_private_effects() {
    let owner = materialized_unit_local_helper_v1();
    let identity = *owner.executable().canonical().identity();
    assert_eq!(unit_local_ranked_private_stores_v1(&owner), 9);
    assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &[ranked_root_input_1d(A_NAME, 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
    )
    .unwrap();
    assert_eq!(program.roots.len(), 1);
    assert!(program.roots[0].all_kernel_checks_are_clean());
    assert!(program.roots[0].access_sources.is_empty());
    assert!(program.roots[0].executable_effect_sources.is_empty());
    assert_eq!(
        *program.materialized.executable().canonical().identity(),
        identity
    );
    assert_eq!(
        unit_local_ranked_private_stores_v1(&program.materialized),
        9
    );
    assert_eq!(
        program.materialized.empty_effect_helpers().iter().count(),
        0
    );
    assert!(!program.materialized.grants_artifact_or_launch_authority());
    assert!(matches!(
        fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            program.materialized, vec![],
        ),
        Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
            consumer: "materialized ranked receipt",
        })
    ));
}

#[test]
fn unit_local_ranked_join_does_not_mint_raw_empty_or_scalar_effects() {
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectsV1, CanonicalKirInventoryV1,
    };
    let owner = materialized_unit_local_helper_v1();
    let source = RankedProjectionSourceV1::from_materialized_checked(&owner).unwrap();
    let helper = SemanticFunctionIdV1::from_index(1);
    with_projection_source_budget_v1(&source, |budget| {
        let floor = budget.storage();
        let (inventory, inventory_storage) =
            CanonicalKirInventoryV1::derive(owner.executable(), budget).unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        let (effects, effects_storage) =
            CanonicalKirCallEffectsV1::derive(&inventory, budget).unwrap();
        budget
            .reserve_storage(effects_storage.retained_storage())
            .unwrap();
        owner
            .with_checked_unit_local_source_v1(&inventory, budget, |view, budget| {
                let association = view
                    .association(SemanticFunctionIdV1::from_index(0), helper, budget)?
                    .unwrap();
                let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(association.physical_function()).unwrap(),
                );
                assert_ne!(
                    effects.decision(coordinate, budget).unwrap(),
                    CanonicalKirCallEffectDecisionV1::CompleteEmpty
                );
                assert_eq!(association.call_count(), 1);
                Ok(())
            })
            .unwrap();
        drop(effects);
        budget
            .release_storage(effects_storage.retained_storage())
            .unwrap();
        drop(inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            let summaries = session.callable_effect_summaries(&source)?;
            assert!(!summaries.is_exact_empty(helper));
            assert!(!summaries.is_exact_empty_deterministic_scalar(helper));
            // Tail calls are outside the newly consumed ordinary-call rule.
            assert!(matches!(
                require_bounds_neutral_callable(
                    owner.semantic_ssa().source_semantic().callables(),
                    &summaries,
                    SemanticCallableIdV1::from_index(1),
                    0,
                    SemanticSourceProvenanceV1::unavailable(),
                    true,
                ),
                Err(ProductionRankedProjectionErrorV1::UnresolvedCallableEffect { tail: true, .. })
            ));
            Ok(())
        })
    })
    .unwrap();
}

#[test]
fn unit_local_ranked_call_rejects_foreign_objects_and_missing_sites() {
    let owner = materialized_unit_local_helper_v1();
    let foreign = materialized_unit_local_helper_v1();
    assert_eq!(
        owner.executable().canonical().identity(),
        foreign.executable().canonical().identity()
    );
    let semantic = owner.semantic_ssa().source_semantic();
    let SemanticTerminatorKindV1::Call(call) =
        semantic.functions()[0].blocks()[0].terminator().kind()
    else {
        panic!("actual source call");
    };
    let SemanticTerminatorKindV1::Call(foreign_call) =
        foreign.semantic_ssa().source_semantic().functions()[0].blocks()[0]
            .terminator()
            .kind()
    else {
        panic!("foreign source call");
    };
    let copied_call = call.clone();
    let source = RankedProjectionSourceV1::from_materialized_checked(&owner).unwrap();
    with_projection_source_budget_v1(&source, |budget| {
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            let floor = session.retained_floor_for_test_v1();
            {
                let mut facts = session.for_source(SemanticFunctionIdV1::from_index(0), SemanticFunctionIdV1::from_index(0));
                facts.require_unit_local_call(0, call, SemanticSourceProvenanceV1::unavailable())?;
                for substituted in [&copied_call, foreign_call] {
                    assert!(matches!(facts.require_unit_local_call(0, substituted, SemanticSourceProvenanceV1::unavailable()),
                        Err(ProductionRankedProjectionErrorV1::StructuralValidation(
                            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch
                        ))));
                }
            }
            for (root, caller, block) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
                let mut facts = session.for_source(SemanticFunctionIdV1::from_index(root), SemanticFunctionIdV1::from_index(caller));
                assert!(matches!(facts.require_unit_local_call(block, call, SemanticSourceProvenanceV1::unavailable()),
                    Err(ProductionRankedProjectionErrorV1::UnresolvedCallableEffect { tail: false, callee: 1, .. })));
            }
            assert_eq!(session.retained_floor_for_test_v1(), floor);
            Ok(())
        })
    }).unwrap();
}

#[test]
fn unit_local_ranked_call_query_exact_budget_boundary_and_cleanup() {
    let owner = materialized_unit_local_helper_v1();
    let name_bytes = owner
        .executable()
        .module()
        .functions
        .iter()
        .flat_map(|function| &function.body.as_ref().unwrap().blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            fe2o3_kernel_ir::OperationKind::Call { callee, .. } => Some(callee.as_str().len()),
            _ => None,
        })
        .unwrap();
    // Backend conversion1 + existing scope17 + guard5 + singleton probe4
    // + exact joins48 + both native callee names.
    let exact = 75 + 2 * name_bytes;
    let SemanticTerminatorKindV1::Call(call) =
        owner.semantic_ssa().source_semantic().functions()[0].blocks()[0]
            .terminator()
            .kind()
    else {
        panic!("actual source call");
    };
    let source = RankedProjectionSourceV1::from_materialized_checked(&owner).unwrap();
    with_projection_source_budget_v1(&source, |budget| {
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            let floor = session.retained_floor_for_test_v1();
            let run = |budget: &mut Budget<'_>| {
                session.for_source_with_query_budget_v1(
                    budget, SemanticFunctionIdV1::from_index(0), SemanticFunctionIdV1::from_index(0),
                ).require_unit_local_call(0, call, SemanticSourceProvenanceV1::unavailable())
            };
            let peak = floor + std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionUnitLocalSourceV1<'_>>();
            for work_limit in [exact - 1, exact] {
                let mut work = Work::new(work_limit);
                let mut query_budget = Budget::new(&mut work, peak);
                query_budget.reserve_storage(floor).unwrap();
                let result = run(&mut query_budget);
                if work_limit == exact {
                    result?;
                    assert_eq!(query_budget.work(), exact);
                } else {
                    assert!(matches!(result, Err(ProductionRankedProjectionErrorV1::StructuralValidation(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                        )
                    )) if error.actual() == exact && error.limit() == work_limit));
                    assert_eq!(query_budget.work(), 75);
                }
                assert_eq!(query_budget.storage(), floor);
                assert_eq!(query_budget.peak_storage(), peak);
                assert_eq!(query_budget.failed_storage(), None);
            }
            let mut work = Work::new(exact);
            let mut query_budget = Budget::new(&mut work, peak - 1);
            query_budget.reserve_storage(floor).unwrap();
            assert!(matches!(run(&mut query_budget), Err(ProductionRankedProjectionErrorV1::StructuralValidation(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                )
            )) if error.actual() == peak && error.limit() == peak - 1));
            assert_eq!((query_budget.work(), query_budget.storage(), query_budget.peak_storage()), (18, floor, floor));
            assert_eq!(query_budget.failed_storage(), Some(peak));
            Ok(())
        })
    }).unwrap();
}

fn unit_local_ranked_two_root_two_call_fixture_v1() -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    let original = materialized_unit_local_helper_v1();
    let semantic = original.semantic_ssa().source_semantic();
    let root = &semantic.functions()[0];
    let blocks = vec![
        block(120, vec![], neutral_test_call_v1(1, vec![], 0, A_UNIT, 1)),
        block(121, vec![], neutral_test_call_v1(1, vec![], 0, A_UNIT, 2)),
        block(122, vec![], SemanticTerminatorKindV1::Return),
    ];
    let first = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks.clone(),
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(249)),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"unit_local_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(246)),
        root.kernel_entry().unwrap().source_contract(),
    ));
    let functions = vec![first, semantic.functions()[1].clone(), second];
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = vec![
        ranked_root_input_1d(A_NAME, 247, 64),
        ranked_root_input_1d("unit_local_second", 246, 64),
    ];
    (materialize_ranked_fixture_v1(ssa, &inputs).unwrap(), inputs)
}

#[test]
fn unit_local_ranked_production_two_roots_and_calls_are_independently_joined() {
    let (owner, inputs) = unit_local_ranked_two_root_two_call_fixture_v1();
    let identity = *owner.executable().canonical().identity();
    let stores = unit_local_ranked_private_stores_v1(&owner);
    let source = RankedProjectionSourceV1::from_materialized_checked(&owner).unwrap();
    with_projection_source_budget_v1(&source, |budget| {
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            let summaries = session.callable_effect_summaries(&source)?;
            assert!(!summaries.is_exact_empty(SemanticFunctionIdV1::from_index(1)));
            for root in [0, 2] {
                for block in [0, 1] {
                    let SemanticTerminatorKindV1::Call(call) =
                        owner.semantic_ssa().source_semantic().functions()[root].blocks()[block]
                            .terminator()
                            .kind()
                    else {
                        panic!("actual per-root call");
                    };
                    session
                        .for_source(
                            SemanticFunctionIdV1::from_index(root as u32),
                            SemanticFunctionIdV1::from_index(root as u32),
                        )
                        .require_unit_local_call(
                            block,
                            call,
                            SemanticSourceProvenanceV1::unavailable(),
                        )?;
                    assert!(matches!(
                        session
                            .for_source(
                                SemanticFunctionIdV1::from_index((2 - root) as u32),
                                SemanticFunctionIdV1::from_index(root as u32)
                            )
                            .require_unit_local_call(
                                block,
                                call,
                                SemanticSourceProvenanceV1::unavailable()
                            ),
                        Err(
                            ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                                tail: false,
                                ..
                            }
                        )
                    ));
                }
            }
            Ok(())
        })
    })
    .unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
    )
    .unwrap();
    assert_eq!(program.roots.len(), 2);
    for root in &program.roots {
        assert!(root.all_kernel_checks_are_clean());
        assert!(root.access_sources.is_empty());
        assert!(root.executable_effect_sources.is_empty());
    }
    assert!(stores > 0);
    assert_eq!(
        unit_local_ranked_private_stores_v1(&program.materialized),
        stores
    );
    assert_eq!(
        *program.materialized.executable().canonical().identity(),
        identity
    );
}
