// Shared source-ranked authentication; this does not authenticate optimized O.

/// Authenticates genuine source-ranked roots without consuming the retained N.
/// Functional custody remains optional exactly as in the historical roster;
/// coherence of an absent record does not satisfy a later presence requirement.
/// Source authentication retains its existing allocation and execution domains.
/// The caller keeps N and its receipts live on the original ledger; the lowerer
/// lends that ledger and retains its paid temporary reports through the callback.
fn with_authenticated_borrowed_ranked_source_roster_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    roots: Box<[ProductionRankedRootProgramV1]>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    body: impl for<'scope> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'scope>,
        &AuthenticatedRankedVerificationRosterV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, ProductionRankedVerificationErrorV1>,
) -> Result<T, ProductionRankedVerificationErrorV1> {
    let (source_order_roots, canonical_kernel_order, canonical_roster_identity) =
        authenticate_ranked_source_parts_v1(materialized, roots)?;
    verify_authenticated_ranked_source_parts_v1(
        materialized,
        &source_order_roots,
        canonical_roster_identity,
        &canonical_kernel_order,
    )?;
    let (lowering_roots, verification_roots) =
        split_authenticated_ranked_source_roots_v1(source_order_roots);
    materialized
        .with_borrowed_ranked_correspondence_v1(
            &lowering_roots,
            budget,
            |correspondence, budget| {
                if correspondence.root_count() != verification_roots.len() {
                    return Ok(Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                        "borrowed correspondence changed the canonical root roster",
                    )));
                }
                let verification = AuthenticatedRankedVerificationRosterV1 {
                    roots: verification_roots.into_boxed_slice(),
                    canonical_roster_identity,
                    canonical_kernel_order,
                };
                Ok(body(correspondence, &verification, budget))
            },
        )
        .map_err(ProductionRankedVerificationErrorV1::Custody)?
}

fn authenticate_ranked_source_parts_v1(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    roots: Box<[ProductionRankedRootProgramV1]>,
) -> Result<
    (
        Box<[ProductionRankedVerifiedRootCandidateV1]>,
        Box<[usize]>,
        ProductionRankedKernelRosterIdentityV1,
    ),
    ProductionRankedVerificationErrorV1,
> {
    materialized
        .semantic_ssa()
        .verify_replay()
        .map_err(ProductionRankedVerificationErrorV1::SemanticSsa)?;
    let semantic_owner = materialized.semantic_ssa().source_owner();
    let semantic_bindings = roots
        .iter()
        .map(ranked_root_program_semantic_binding_v1)
        .collect::<Vec<_>>();
    validate_ranked_roster_semantic_bindings_v1(semantic_owner, &semantic_bindings)?;

    let mut verified_roots = Vec::with_capacity(roots.len());
    for root in roots.into_vec() {
        fe2o3_lower_mir_kernel::validate_borrowed_ranked_semantic_projection_candidate_with_generated_effects_v1(
            semantic_owner,
            root.semantic_root,
            &root.lowering,
            &root.ranked_ir,
            &root.access_sources,
            &root.executable_effect_sources,
        )
        .map_err(ProductionRankedVerificationErrorV1::Custody)?;
        let ProductionRankedRootProgramV1 {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            semantic_u32_induction,
            lowering,
            ranked_ir,
            access_sources,
            executable_effect_sources,
        } = root;
        let verification = authenticate_ranked_root_v5(
            semantic_owner,
            &lowering,
            &ranked_ir,
            semantic_u32_induction,
        )?;
        verified_roots.push(ProductionRankedVerifiedRootCandidateV1 {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            lowering,
            ranked_ir,
            access_sources,
            executable_effect_sources,
            verification,
        });
    }
    let source_order_roots = verified_roots.into_boxed_slice();
    let records = ranked_roster_identity_records_v1(&source_order_roots);
    let (canonical_roster_identity, canonical_kernel_order) =
        derive_ranked_kernel_roster_identity_v1(&records)?;
    Ok((
        source_order_roots,
        canonical_kernel_order,
        canonical_roster_identity,
    ))
}

fn verify_authenticated_ranked_source_parts_v1(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    source_order_roots: &[ProductionRankedVerifiedRootCandidateV1],
    canonical_roster_identity: ProductionRankedKernelRosterIdentityV1,
    canonical_kernel_order: &[usize],
) -> Result<(), ProductionRankedVerificationErrorV1> {
    materialized
        .semantic_ssa()
        .verify_replay()
        .map_err(ProductionRankedVerificationErrorV1::SemanticSsa)?;
    let semantic_owner = materialized.semantic_ssa().source_owner();
    let semantic_bindings = source_order_roots
        .iter()
        .map(ranked_verified_root_semantic_binding_v1)
        .collect::<Vec<_>>();
    validate_ranked_roster_semantic_bindings_v1(semantic_owner, &semantic_bindings)?;
    for root in source_order_roots {
        fe2o3_lower_mir_kernel::validate_borrowed_ranked_semantic_projection_candidate_with_generated_effects_v1(
            semantic_owner,
            root.semantic_root,
            &root.lowering,
            &root.ranked_ir,
            &root.access_sources,
            &root.executable_effect_sources,
        )
        .map_err(ProductionRankedVerificationErrorV1::Custody)?;
        let revalidated = fe2o3_pliron::ProductionMiddleEndEvidenceV5::try_new(
            semantic_owner,
            &root.lowering,
            &root.ranked_ir,
        )
        .map_err(ProductionRankedVerificationErrorV1::MiddleEndEvidence)?;
        if revalidated.as_inert().canonical_bytes()
            != root
                .verification
                .middle_end_evidence
                .as_inert()
                .canonical_bytes()
            || root
                .verification
                .has_authenticated_functional_verification()
                != root
                    .lowering
                    .has_retained_policy_checked_refinement_staging()
            || !root
                .verification
                .retained_functional_verification_is_coherent()
        {
            return Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                "changed per-root ranked verification custody",
            ));
        }
        validate_ranked_root_induction_custody_v1(materialized.semantic_ssa(), root)?;
    }
    let records = ranked_roster_identity_records_v1(source_order_roots);
    require_exact_ranked_kernel_roster_identity_v1(
        &records,
        canonical_roster_identity,
        canonical_kernel_order,
    )
}

fn split_authenticated_ranked_source_roots_v1(
    source_order_roots: Box<[ProductionRankedVerifiedRootCandidateV1]>,
) -> (
    Vec<fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1>,
    Vec<AuthenticatedRankedVerificationRootV1>,
) {
    let mut lowering_roots = Vec::with_capacity(source_order_roots.len());
    let mut verification_roots = Vec::with_capacity(source_order_roots.len());
    for root in source_order_roots.into_vec() {
        let ProductionRankedVerifiedRootCandidateV1 {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            lowering,
            ranked_ir,
            access_sources,
            executable_effect_sources,
            verification,
        } = root;
        lowering_roots.push(
            fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1::new(
                semantic_root,
                source_rank,
                lowering,
                ranked_ir,
                access_sources,
                executable_effect_sources,
            ),
        );
        verification_roots.push(AuthenticatedRankedVerificationRootV1 {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            verification,
        });
    }
    (lowering_roots, verification_roots)
}
