fn unit_local_backend_stage_candidates_v1(
    program: ProductionRankedSemanticProgramV1,
) -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1>,
) {
    let ProductionRankedSemanticProgramV1 {
        materialized,
        roots,
        phase: _phase,
    } = program;
    let roots = roots
        .into_vec()
        .into_iter()
        .map(|root| {
            fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1::new(
                root.semantic_root,
                root.source_rank,
                root.verification
                    .into_ordinary()
                    .expect("ordinary test root")
                    .0,
                root.ranked_ir,
                root.access_sources,
                root.executable_effect_sources,
            )
        })
        .collect();
    (materialized, roots)
}

#[test]
fn unit_local_backend_ranked_stage_checks_real_n_without_opening_legacy_receipt() {
    let owner = materialized_unit_local_helper_v1();
    let identity = *owner.executable().canonical().identity();
    assert_eq!(unit_local_ranked_private_stores_v1(&owner), 9);
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &[ranked_root_input_1d(A_NAME, 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    assert!(program.roots[0].all_kernel_checks_are_clean());
    assert!(program.roots[0].access_sources.is_empty());
    assert!(program.roots[0].executable_effect_sources.is_empty());
    let (owner, roots) = unit_local_backend_stage_candidates_v1(program);

    // Real projector -> backend hook prefix only. No authenticated Verus roster
    // is manufactured to enter into_module_verified_receipt in this test.
    validate_unit_local_ranked_stage_v1(&owner, &roots).unwrap();
    assert_eq!(*owner.executable().canonical().identity(), identity);
    assert_eq!(unit_local_ranked_private_stores_v1(&owner), 9);
    assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
    assert!(!owner.grants_artifact_or_launch_authority());
    assert!(matches!(
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            owner, roots,
        ),
        Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
            consumer: "materialized ranked receipt",
        })
    ));
}

#[test]
fn unit_local_backend_ranked_stage_requires_complete_ordered_roots() {
    let (owner, inputs) = unit_local_ranked_two_root_two_call_fixture_v1();
    let identity = *owner.executable().canonical().identity();
    let stores = unit_local_ranked_private_stores_v1(&owner);
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    let (owner, mut roots) = unit_local_backend_stage_candidates_v1(program);
    assert_eq!(roots.len(), 2);
    validate_unit_local_ranked_stage_v1(&owner, &roots).unwrap();

    let second = roots.pop().unwrap();
    let missing = validate_unit_local_ranked_stage_v1(&owner, &roots);
    assert!(
        matches!(
            &missing,
            Err(ProductionRankedVerificationErrorV1::Custody(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                    detail: "ranked projection roster is not a complete semantic root bijection",
                    ..
                }
            ))
        ),
        "{missing:?}"
    );
    roots.push(second);
    roots.reverse();
    let reordered = validate_unit_local_ranked_stage_v1(&owner, &roots);
    assert!(
        matches!(
            &reordered,
            Err(ProductionRankedVerificationErrorV1::Custody(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                    detail: "ranked projection roster has a duplicate, missing, or invalid root identity",
                    ..
                }
            ))
        ),
        "{reordered:?}"
    );
    roots.reverse();
    validate_unit_local_ranked_stage_v1(&owner, &roots).unwrap();
    assert_eq!(*owner.executable().canonical().identity(), identity);
    assert_eq!(unit_local_ranked_private_stores_v1(&owner), stores);
}

#[test]
fn unit_local_backend_ranked_stage_leaves_raw_empty_receipt_route_unchanged() {
    let owner = materialized_helper_v1(false);
    assert_eq!(
        owner.helper_source_policy_v1(),
        fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
    );
    // The new stage must not validate or admit legacy roots. The unchanged
    // constructor below still owns that route's complete roster checks.
    validate_unit_local_ranked_stage_v1(&owner, &[]).unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        owner,
        &[ranked_root_input_1d(A_NAME, 247, 64)],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    let (owner, roots) = unit_local_backend_stage_candidates_v1(program);
    validate_unit_local_ranked_stage_v1(&owner, &roots).unwrap();
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            owner, roots,
        )
        .unwrap();
    assert_eq!(receipt.root_count(), 1);
}
