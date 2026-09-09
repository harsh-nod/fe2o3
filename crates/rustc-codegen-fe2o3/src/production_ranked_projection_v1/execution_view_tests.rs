fn expanded_ranked_fixture_v1(omit_effect: bool) -> (ProductionSemanticSsaOwnerV1, usize) {
    let baseline = neutral_ranked_program_v1();
    let expected_effects = baseline.roots[0].executable_effect_sources.len() * 2;
    let source = baseline.semantic_ssa_owner.source_semantic();
    let original = &source.functions()[0];
    // Defined callables occupy the function-indexed prefix. Inserting the
    // helper at index 1 shifts every original intrinsic call by one.
    let helper_blocks = if omit_effect {
        let entry = &original.blocks()[original.entry().index() as usize];
        vec![
            SemanticBasicBlockV1::new(
                entry.identity(),
                entry.source(),
                Vec::new(),
                SemanticTerminatorV1::new(
                    entry.terminator().source(),
                    SemanticTerminatorKindV1::Return,
                ),
            )
            .unwrap(),
        ]
    } else {
        original
            .blocks()
            .iter()
            .map(|current| {
                let kind = match current.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(call.callee().index() + 1),
                            call.arguments().to_vec(),
                            call.destination().cloned(),
                            call.unwind(),
                        )
                        .unwrap(),
                    ),
                    kind => kind.clone(),
                };
                SemanticBasicBlockV1::new(
                    current.identity(),
                    current.source(),
                    current.statements().to_vec(),
                    SemanticTerminatorV1::new(current.terminator().source(), kind),
                )
                .unwrap()
            })
            .collect()
    };
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(20)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        Vec::new(),
        SemanticAbiValueV1::new(NEUTRAL_UNIT_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        original.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        helper_abi,
        original.locals().to_vec(),
        original.entry(),
        helper_blocks,
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(10)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(11)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(12)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(13)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(14)),
        original.source(),
        original.abi().clone(),
        vec![
            local(10, NEUTRAL_UNIT_TYPE, SemanticLocalRoleV1::Return),
            local(11, NEUTRAL_UNIT_TYPE, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                10,
                vec![],
                neutral_test_call_v1(1, vec![], 1, NEUTRAL_UNIT_TYPE, 1),
            ),
            block(
                11,
                vec![],
                neutral_test_call_v1(1, vec![], 1, NEUTRAL_UNIT_TYPE, 2),
            ),
            block(12, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut callables = source.callables().to_vec();
    assert_eq!(callables.len(), 4);
    callables.insert(
        1,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    );
    if omit_effect {
        callables.truncate(2);
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![root, helper],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    (
        ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
        expected_effects,
    )
}

fn project_expanded_ranked_fixture_v1(
    owner: ProductionSemanticSsaOwnerV1,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    project_and_verify_ranked_semantic_mir_v1(
        owner,
        &[ranked_root_input_1d("neutral_generated_hostile", 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
}

#[test]
fn ranked_execution_view_projects_each_memory_helper_instance() {
    let (owner, expected_effects) = expanded_ranked_fixture_v1(false);
    let original_bytes = owner.source_semantic().canonical_encoding().to_vec();
    let root_id = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root_id).unwrap();
    assert!(view.has_expanded_calls());
    let view_identity = *view.identity();
    let helper_function = SemanticFunctionIdV1::from_index(1);
    let expected_blocks = view
        .block_origins()
        .iter()
        .enumerate()
        .filter(|(_, origin)| origin.function() == helper_function && origin.block().index() == 2)
        .map(|(index, _)| index as u32)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_blocks.len(), 2);

    let program = project_expanded_ranked_fixture_v1(owner).unwrap();
    assert_eq!(
        program
            .semantic_ssa_owner
            .source_semantic()
            .canonical_encoding(),
        original_bytes
    );
    let root = &program.roots[0];
    assert!(root.all_kernel_checks_are_clean());
    assert_eq!(root.executable_effect_sources.len(), expected_effects);
    assert_eq!(
        root.executable_effect_sources
            .iter()
            .map(|source| source.semantic_block())
            .collect::<BTreeSet<_>>(),
        expected_blocks,
    );
    assert_eq!(
        root.semantic_u32_induction.execution_view_identity(),
        Some(&view_identity)
    );
    assert!(
        fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1::from_report(
            &root.semantic_u32_induction,
        )
        .is_err()
    );

    let mut receipt = program.into_verified_roster_receipt().unwrap();
    receipt.verify_equivalence().unwrap();
    let source = receipt.semantic_ssa_owner.source_semantic();
    receipt.source_order_roots[0]
        .verification
        .semantic_u32_induction =
        fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(source, root_id).unwrap();
    assert!(matches!(
        receipt.verify_equivalence(),
        Err(ProductionRankedVerificationErrorV1::RosterMetadata(
            "changed per-root semantic induction custody"
        )),
    ));
}

#[test]
fn ranked_execution_view_does_not_neutralize_missing_helper_effects() {
    let (owner, _) = expanded_ranked_fixture_v1(true);
    assert!(matches!(
        project_expanded_ranked_fixture_v1(owner),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access"
        )),
    ));
}

#[test]
fn ranked_execution_view_identity_rejects_substitution_and_erasure() {
    let mut records = [ranked_roster_identity_record(
        "alpha",
        b"kernel_alpha",
        0xa1,
        4,
        0x14,
        1,
        0x31,
    )];
    records[0].induction_execution_view_identity = Some(bytes(0x80));
    let (identity, order) = derive_ranked_kernel_roster_identity_v1(&records).unwrap();
    for changed in [Some(bytes(0x81)), None] {
        records[0].induction_execution_view_identity = changed;
        assert!(matches!(
            require_exact_ranked_kernel_roster_identity_v1(&records, identity, &order),
            Err(ProductionRankedVerificationErrorV1::RosterIdentity),
        ));
    }
}
