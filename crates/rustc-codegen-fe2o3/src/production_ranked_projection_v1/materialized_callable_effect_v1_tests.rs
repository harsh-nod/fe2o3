// Reuse the genuine semantic owner fixtures, not a fabricated purity flag.

fn materialized_aggregate_helper_v1() -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    materialized_helper_v1(true)
}

fn materialized_helper_v1(
    aggregate: bool,
) -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    let root = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![],
        vec![
            block(
                120,
                vec![],
                neutral_test_call_v1(1, vec![typed_constant(A_U32, 3, 4)], 1, A_U32, 1),
            ),
            block(121, vec![], SemanticTerminatorKindV1::Return),
        ],
        false,
    );
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(248)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(131)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(132)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(133)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(134)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(135)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(
                neutral_plain_direct_abi_value_v1(A_U32),
            )],
            neutral_plain_direct_abi_value_v1(A_U32),
        )
        .unwrap(),
        {
            let mut locals = vec![
                local(136, A_U32, SemanticLocalRoleV1::Return),
                local(137, A_U32, SemanticLocalRoleV1::Argument(0)),
            ];
            if aggregate {
                locals.push(local(138, A_ARRAY, SemanticLocalRoleV1::Temporary));
            }
            locals
        },
        SemanticBlockIdV1::from_index(0),
        vec![block(
            139,
            {
                let mut statements = vec![
                    typed_assignment(
                        2,
                        A_ARRAY,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Array,
                                vec![typed_constant(A_U32, 3, 4); 8],
                            )
                            .unwrap(),
                        ),
                    ),
                    typed_assignment(0, A_U32, SemanticRvalueKindV1::Use(typed_operand(1, A_U32))),
                ];
                if !aggregate {
                    statements.remove(0);
                }
                statements
            },
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    assertion_materialized_functions(assertion_types(), vec![root, helper])
}

fn materialized_helper_shared_roots_v1() -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    let original = materialized_aggregate_helper_v1();
    let semantic = original.semantic_ssa().source_semantic();
    let root = &semantic.functions()[0];
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
        root.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"shared_helper_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(246)),
        root.kernel_entry().unwrap().source_contract(),
    ));
    let mut functions = semantic.functions().to_vec();
    functions.push(second);
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
    materialize_ranked_fixture_v1(
        ssa,
        &[
            ranked_root_input_1d(A_NAME, 247, 64),
            ranked_root_input_1d("shared_helper_second", 246, 64),
        ],
    )
    .unwrap()
}

#[test]
fn materialized_helper_effects_use_canonical_not_semantic_function_ordinals() {
    let owner = materialized_helper_shared_roots_v1();
    let helper = owner.empty_effect_helpers().iter().next().unwrap();
    assert_eq!(helper.semantic_function().index(), 1);
    let actual_ordinal = owner
        .executable()
        .module()
        .functions
        .iter()
        .position(|function| &function.id == helper.kernel_ir_function())
        .unwrap();
    assert_ne!(actual_ordinal, helper.semantic_function().index() as usize);
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    canonical_assertion_facts_v1::with_canonical_assertions_v1(&owner, |session| {
        let effects = session.callable_effect_summaries(&source)?;
        assert!(effects.is_exact_empty(helper.semantic_function()));
        assert!(!effects.is_exact_empty_deterministic_scalar(helper.semantic_function()));
        Ok(())
    })
    .unwrap();
}

#[test]
fn materialized_empty_effects_do_not_substitute_for_call_result_control() {
    for aggregate in [false, true] {
        let owner = materialized_helper_v1(aggregate);
        let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let semantic = source.semantic_ssa().source_semantic();
        let function = &semantic.functions()[0];
        let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind() else {
            panic!("root call");
        };
        canonical_assertion_facts_v1::with_canonical_assertions_v1(&owner, |session| {
            let effects = session.callable_effect_summaries(&source)?;
            let helper = SemanticFunctionIdV1::from_index(1);
            assert!(effects.is_exact_empty(helper));
            assert_eq!(
                effects.is_exact_empty_deterministic_scalar(helper),
                !aggregate
            );
            let count = function.locals().len();
            let constants = constant_locals(function)?;
            let definitions = local_definition_counts(function);
            let indices = vec![None; count];
            let allocations = vec![None; count];
            let predicates = vec![None; count];
            let mut slots = vec![None; count];
            let mut next_argument = 0;
            let mut operations = vec![];
            let mut next_value = 0;
            let mut projector = DeterministicScalarProjectorV1::new(
                semantic.types(),
                semantic.callables(),
                &effects,
                function,
                &constants,
                &definitions,
                &indices,
                &allocations,
                &predicates,
                &mut slots,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            )?;
            assert_eq!(
                projector.resolve_call(call)?.is_some(),
                !aggregate,
                "executable emptiness alone cannot authorize deterministic call-result control"
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn materialized_aggregate_helper_effects_do_not_grant_scalar_value_equivalence() {
    let owner = materialized_aggregate_helper_v1();
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    let semantic = source.semantic_ssa().source_semantic();
    let helper = SemanticFunctionIdV1::from_index(1);
    let old = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(),
        semantic.functions(),
        semantic.callables(),
    )
    .unwrap();
    assert!(!old.is_exact_empty(helper));
    assert_eq!(source.owner().empty_effect_helpers().iter().count(), 1);

    canonical_assertion_facts_v1::with_canonical_assertions_v1(&owner, |session| {
        let floor = session.retained_floor_for_test_v1();
        let facts = session.callable_effect_summaries(&source)?;
        assert_eq!(session.retained_floor_for_test_v1(), floor);
        session.callable_effect_summaries(&source)?;
        assert_eq!(session.retained_floor_for_test_v1(), floor);
        assert!(facts.is_exact_empty(helper));
        assert!(!facts.is_exact_empty_deterministic_scalar(helper));
        assert_eq!(
            facts.decisions[0], old.decisions[0],
            "kernel roots gain no helper fact"
        );
        require_bounds_neutral_callable(
            semantic.callables(),
            &facts,
            SemanticCallableIdV1::from_index(1),
            0,
            SemanticSourceProvenanceV1::unavailable(),
            false,
        )
    })
    .unwrap();
}

#[test]
fn materialized_helper_effect_query_releases_report_before_outer_scope_cleanup() {
    let owner = materialized_aggregate_helper_v1();
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    canonical_assertion_facts_v1::with_canonical_assertions_v1(&owner, |session| {
        let floor = session.retained_floor_for_test_v1();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        session.callable_effect_summaries_with_query_budget_v1(&source, &mut budget)?;
        let (cost, peak) = (budget.work(), budget.peak_storage());
        assert!(peak > floor);
        assert_eq!(budget.storage(), floor);
        for limit in [cost - 1, cost] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, peak);
            budget.reserve_storage(floor).unwrap();
            let result =
                session.callable_effect_summaries_with_query_budget_v1(&source, &mut budget);
            assert_eq!(result.is_ok(), limit == cost);
            assert_eq!(
                budget.peak_storage(),
                peak,
                "failure occurred after report allocation"
            );
            assert_eq!(
                budget.storage(),
                floor,
                "outer scope cannot hide a report leak"
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn materialized_helper_effect_roster_scan_has_an_exact_work_boundary() {
    let owner = materialized_aggregate_helper_v1();
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    let floor = owner.executable_storage().retained_storage()
        + owner.assert_origin_storage().payload_storage();
    let run = |budget: &mut Budget<'_>| {
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            session.callable_effect_summaries(&source)
        })
    };
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    run(&mut budget).unwrap();
    let (cost, peak) = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, success) in [
        (cost, peak, true),
        (cost - 1, peak, false),
        (cost, peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = run(&mut budget);
        assert_eq!(budget.storage(), floor);
        if success {
            assert!(
                result
                    .unwrap()
                    .is_exact_empty(SemanticFunctionIdV1::from_index(1))
            );
        } else {
            assert!(
                result.is_err(),
                "complete effect closure must fit the shared ledger"
            );
        }
    }
}

#[test]
fn materialized_helper_effects_reject_equal_bytes_from_another_owner() {
    let owner = materialized_aggregate_helper_v1();
    let foreign = materialized_aggregate_helper_v1();
    assert_eq!(
        owner.executable().canonical().identity(),
        foreign.executable().canonical().identity()
    );
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    let result = canonical_assertion_facts_v1::with_canonical_assertions_v1(&foreign, |session| {
        session.callable_effect_summaries(&source)
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "materialized helper effects belong to another executable owner"
        ))
    ));
}
