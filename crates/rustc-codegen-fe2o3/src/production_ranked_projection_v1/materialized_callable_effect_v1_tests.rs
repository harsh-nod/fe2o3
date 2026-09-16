// Reuse the genuine semantic owner fixtures, not a fabricated purity flag.

fn materialized_aggregate_helper_v1() -> fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1 {
    let root = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![],
        vec![
            block(120, vec![], neutral_test_call_v1(1, vec![], 1, A_U32, 1)),
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
            0,
            vec![],
            neutral_plain_direct_abi_value_v1(A_U32),
        )
        .unwrap(),
        vec![
            local(136, A_U32, SemanticLocalRoleV1::Return),
            local(137, A_ARRAY, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            138,
            vec![
                typed_assignment(
                    1,
                    A_ARRAY,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            vec![typed_constant(A_U32, 3, 4); 8],
                        )
                        .unwrap(),
                    ),
                ),
                typed_assignment(
                    0,
                    A_U32,
                    SemanticRvalueKindV1::Use(typed_constant(A_U32, 3, 4)),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    assertion_materialized_functions(assertion_types(), vec![root, helper])
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
    assert_eq!(source.empty_effect_helpers().iter().count(), 1);

    with_projection_source_budget_v1(&source, |budget| {
        let facts = derive_materialized_callable_effect_summaries_v1(&source, budget)?;
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
fn materialized_helper_effect_roster_scan_has_an_exact_work_boundary() {
    let owner = materialized_aggregate_helper_v1();
    let source = RankedProjectionSourceV1::from_legacy(&owner).unwrap();
    let rows = source.empty_effect_helpers().scanned_function_count();
    assert_eq!(rows, 2);
    for work_limit in [rows, rows - 1] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let result = derive_materialized_callable_effect_summaries_v1(&source, &mut budget);
        if work_limit == rows {
            assert!(
                result
                    .unwrap()
                    .is_exact_empty(SemanticFunctionIdV1::from_index(1))
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(_)
                ))
            ));
        }
    }
}
