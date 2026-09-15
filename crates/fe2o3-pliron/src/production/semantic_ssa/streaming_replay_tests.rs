use super::*;

fn two_root_semantic() -> AdmittedInertSemanticMirV1 {
    let base = admitted_single_function_semantic();
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(190)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(191)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(192)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(193)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(194)),
        base.functions()[0].source(),
        base.functions()[0].abi().clone(),
        vec![test_local(198, 0, SemanticLocalRoleV1::Return)],
        SemanticBlockIdV1::from_index(0),
        vec![
            test_block(
                201,
                vec![],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            test_block(202, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(
        fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticLinkSymbolV1::new(
                b"semantic_ssa_second_root".to_vec(),
            )
            .unwrap(),
            fe2o3_mir_model::semantic_mir_v1::SemanticKernelBindingIdentityV1::from_sha256(
                test_bytes(199),
            ),
            fe2o3_mir_model::semantic_mir_v1::SemanticKernelSourceContractV1::new(None, None, None)
                .unwrap(),
        ),
    );
    InertSemanticMirRequestV1::new(
        base.target(),
        base.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![base.functions()[0].clone(), second],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn two_root_owner() -> ProductionSemanticSsaOwnerV1 {
    let source = ProductionSemanticMirOwnerV1::try_new(
        two_root_semantic(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn block_limits(blocks: usize) -> ProductionSemanticSsaLimitsV1 {
    let production = ProductionSemanticSsaModuleLimitsV1::production();
    ProductionSemanticSsaLimitsV1::with_module_limits(
        SsaPlannerLimitsV1::default(),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            production.max_variables(),
            blocks,
            production.max_edges(),
            production.max_events(),
            production.max_edge_definitions(),
            production.max_output_items(),
            production.max_storage_words(),
            production.max_work_units(),
        )
        .unwrap(),
    )
}

fn late_block_error() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource: SsaPlannerResourceV1::Blocks,
        required: 3,
        limit: 2,
    }
}

// Independent copy of the pre-streaming V1 byte grammar, not the new hash helpers.
fn legacy_function_bytes(bytes: &mut Vec<u8>, function: &ProductionSemanticSsaFunctionPlanV1) {
    bytes.extend_from_slice(&function.function.index().to_le_bytes());
    bytes.extend_from_slice(function.function_identity.as_bytes());
    bytes.extend_from_slice(function.plan.identity().as_bytes());
    let resources = function.plan.resources();
    for word in [
        resources.input_blocks(),
        resources.reachable_blocks(),
        resources.pruned_blocks(),
        resources.input_edges(),
        resources.input_events(),
        resources.input_edge_definitions(),
        resources.generated_definitions(),
        resources.output_items(),
        resources.storage_words(),
        resources.work_units(),
        function.partial_moves.projected_moves(),
        function.partial_moves.state_entries(),
        function.partial_moves.work_units(),
        function.auxiliary_resources.storage_words,
        function.auxiliary_resources.work_units,
    ] {
        bytes.extend_from_slice(&u64::try_from(word).unwrap().to_le_bytes());
    }
    for variables in [
        &function.implicit_entry_variables,
        &function.retained_cross_edge_variables,
    ] {
        bytes.extend_from_slice(&u64::try_from(variables.len()).unwrap().to_le_bytes());
        for variable in variables {
            bytes.extend_from_slice(&variable.get().to_le_bytes());
        }
    }
}

fn legacy_identity(
    source: &[u8; 32],
    plans: &[ProductionSemanticSsaFunctionPlanV1],
    summary: ProductionSemanticSsaSummaryV1,
) -> [u8; 32] {
    let domain = b"fe2o3.production-semantic-ssa-owner.v1\0";
    let mut bytes = domain.to_vec();
    bytes.extend_from_slice(source);
    bytes.extend_from_slice(&u64::try_from(plans.len()).unwrap().to_le_bytes());
    for plan in plans {
        legacy_function_bytes(&mut bytes, plan);
    }
    for word in [
        summary.function_count(),
        summary.promotable_variables(),
        summary.memory_variables(),
        summary.input_blocks(),
        summary.reachable_blocks(),
        summary.pruned_blocks(),
        summary.input_edges(),
        summary.input_events(),
        summary.input_edge_definitions(),
        summary.generated_definitions(),
        summary.output_items(),
        summary.storage_words(),
        summary.work_units(),
    ] {
        bytes.extend_from_slice(&u64::try_from(word).unwrap().to_le_bytes());
    }
    let variable_count: usize = plans
        .iter()
        .map(|plan| plan.implicit_entry_variables.len() + plan.retained_cross_edge_variables.len())
        .sum();
    // Function: ordinal4 + identities64 + fifteen words120 + two counts16.
    // Summary: thirteen words104. Variable IDs remain u32, not pointer-sized.
    assert_eq!(
        bytes.len(),
        domain.len() + 32 + 8 + plans.len() * 204 + variable_count * 4 + 104
    );
    Sha256::digest(bytes).into()
}

#[test]
fn streaming_replay_preserves_two_root_plans_and_legacy_identity() {
    let owner = two_root_owner();
    assert_eq!(owner.plans.len(), 2);
    assert_eq!(owner.summary.input_blocks(), 3);
    assert_eq!(owner.summary.input_edges(), 1);
    let expected = legacy_identity(&owner.source_semantic_sha256, &owner.plans, owner.summary);
    assert_eq!(owner.identity.as_bytes(), &expected);
    let plan_allocation = owner.plans.as_ptr();
    let source_allocation = owner.source_semantic().functions().as_ptr();
    let original_plans = owner.plans.clone();
    for _ in 0..3 {
        owner.verify_replay().unwrap();
        assert_eq!(owner.plans.as_ptr(), plan_allocation);
        assert_eq!(
            owner.source_semantic().functions().as_ptr(),
            source_allocation
        );
        assert_eq!(owner.plans, original_plans);
        assert_eq!(owner.identity.as_bytes(), &expected);
    }
}

#[test]
fn streaming_identity_keeps_nonempty_variable_lists_and_empty_frame_grammar() {
    let owner = two_root_owner();
    // Codec-only rows exercise nonempty list encoding; they do not create an owner.
    let mut rows = owner.plans.clone();
    rows[0].implicit_entry_variables = Box::new([SsaVariableIdV1::new(0), SsaVariableIdV1::new(9)]);
    rows[1].retained_cross_edge_variables =
        Box::new([SsaVariableIdV1::new(1), SsaVariableIdV1::new(7)]);
    for plans in [rows.as_ref(), &[]] {
        let expected = legacy_identity(&owner.source_semantic_sha256, plans, owner.summary);
        assert_eq!(
            derive_semantic_ssa_identity_v1(&owner.source_semantic_sha256, plans, owner.summary)
                .as_bytes(),
            &expected
        );
        let mut digest = begin_semantic_ssa_identity_v1(&owner.source_semantic_sha256, plans.len());
        for plan in plans {
            hash_semantic_ssa_function_plan_v1(&mut digest, plan);
        }
        assert_eq!(
            finish_semantic_ssa_identity_v1(digest, owner.summary).as_bytes(),
            &expected
        );
    }
}

#[test]
fn streaming_replay_checks_exact_retained_fields_not_only_stored_identities() {
    for field in 0..8 {
        let mut owner = two_root_owner();
        let original_identity = owner.identity;
        let original_plan_identity = owner.plans[0].plan.identity();
        match field {
            0 => owner.plans[0].function = SemanticFunctionIdV1::from_index(1),
            1 => {
                owner.plans[0].function_identity =
                    SemanticFunctionIdentityV1::from_sha256(test_bytes(230));
            }
            2 => owner.plans[0].partial_moves.projected_moves += 1,
            3 => owner.plans[0].partial_moves.state_entries += 1,
            4 => owner.plans[0].auxiliary_resources.storage_words += 1,
            5 => owner.plans[0].auxiliary_resources.work_units += 1,
            6 => owner.plans[0].implicit_entry_variables = Box::new([SsaVariableIdV1::new(0)]),
            7 => {
                owner.plans[0].retained_cross_edge_variables = Box::new([SsaVariableIdV1::new(0)]);
            }
            _ => unreachable!(),
        }
        // Fresh-source hashing still equals the unchanged outer identity. Only
        // comparing the retained fields against the fresh plan rejects this.
        assert_eq!(owner.identity, original_identity);
        assert_eq!(owner.plans[0].plan.identity(), original_plan_identity);
        assert_eq!(
            owner.verify_replay(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
            "retained field {field}"
        );
    }
}

#[test]
fn streaming_replay_rejects_plan_count_order_summary_and_identity_changes() {
    for change in 0..6 {
        let mut owner = two_root_owner();
        match change {
            0 => owner.plans = Box::new([]),
            1 => owner.plans = vec![owner.plans[0].clone()].into_boxed_slice(),
            2 => owner.plans.swap(0, 1),
            3 => owner.summary.input_blocks += 1,
            4 => owner.identity.0[0] ^= 1,
            5 => owner.source_semantic_sha256[0] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            owner.verify_replay(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
            "retained change {change}"
        );
    }
}

#[test]
fn streaming_replay_keeps_late_construction_error_ahead_of_earlier_mismatch() {
    for mismatch in [false, true] {
        let mut owner = two_root_owner();
        owner.limits = block_limits(2);
        if mismatch {
            owner.plans[0].auxiliary_resources.work_units += 1;
        }
        assert_eq!(owner.verify_replay(), Err(late_block_error()));
    }
    let mut owner = two_root_owner();
    owner.limits = block_limits(2);
    owner.source_semantic_sha256[0] ^= 1;
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}

#[test]
fn streaming_plan_walk_visits_each_function_after_its_limits_and_before_the_next() {
    let semantic = two_root_semantic();
    let mut seen = Vec::new();
    assert_eq!(
        walk_semantic_ssa_plans_v1(&semantic, block_limits(2), |plan| {
            seen.push((plan.function.index(), plan.plan.resources().input_blocks()));
        }),
        Err(late_block_error())
    );
    assert_eq!(seen, [(0, 1)]);
    seen.clear();
    let summary = walk_semantic_ssa_plans_v1(&semantic, block_limits(3), |plan| {
        seen.push((plan.function.index(), plan.plan.resources().input_blocks()));
    })
    .unwrap();
    assert_eq!(seen, [(0, 1), (1, 2)]);
    assert_eq!(summary.input_blocks(), 3);
}
