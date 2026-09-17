use super::*;
use crate::production_ranked_projection_v1::{
    ProductionRankedRootInputV1, project_and_verify_ranked_materialized_semantic_mir_v1,
    source_launch_roster_for_ranked_inputs_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1};

include!("publication_model_types_v1.rs");
include!("publication_model_request_v1.rs");

struct Pair {
    owner: ProductionFormalMemoryOwnerV1,
    middle: Vec<u8>,
    formal: Vec<u8>,
    semantic: [u8; 32],
    kir: ProductionCanonicalKernelIrIdentityV1,
}

fn admitted_model_pair(padding: bool) -> Pair {
    admitted_model_pair_from_request(request(padding))
}

fn project_admitted_publication_model(
    request: InertSemanticMirRequestV1,
) -> Result<
    (
        [u8; 32],
        crate::production_ranked_projection_v1::ProductionRankedSemanticProgramV1,
    ),
    Box<crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1>,
> {
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let semantic = *admitted.semantic_sha256().as_bytes();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = fe2o3_artifacts::LaunchContract::new(
        1,
        fe2o3_artifacts::BlockSize::Exact(fe2o3_artifacts::Dimensions::new(128, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(2, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let roots = [ProductionRankedRootInputV1::new(
        "publication_model_pair",
        [226; 32],
        &launch,
    )];
    let source_launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &roots).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let materialized =
        fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            source_launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(
            materialized
                .executable_storage()
                .retained_storage()
                .checked_add(materialized.assert_origin_storage().payload_storage())
                .unwrap(),
        )
        .unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &roots,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
    )
    .map_err(Box::new)?;
    Ok((semantic, program))
}

fn admitted_model_pair_from_request(request: InertSemanticMirRequestV1) -> Pair {
    let (semantic, program) = project_admitted_publication_model(request).unwrap();
    assert!(program.all_kernel_checks_are_clean());
    let (receipt, verification) = program
        .into_verified_roster_receipt()
        .unwrap()
        .into_module_verified_receipt()
        .unwrap();
    let middle = verification.roots()[0]
        .verification()
        .middle_end_evidence()
        .canonical_bytes()
        .to_vec();
    assert!(middle.starts_with(b"F2MEV6\0\0"));
    let lowered = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
    lowered.verify_equivalence().unwrap();
    let kir = lowered.canonical_kernel_ir_identity();
    let owner = ProductionFormalMemoryOwnerV1::try_admit(lowered).unwrap();
    let formal =
        prepare_formal_admission_payload_v5(&owner, true, &middle, &semantic, kir).unwrap();
    Pair {
        owner,
        middle,
        formal,
        semantic,
        kir,
    }
}

// Mutants alter inert records only. Both starting owners were independently
// built from admitted semantic models through the unmodified production path.
fn reseal_middle(bytes: &mut [u8]) {
    use sha2::{Digest, Sha256};
    let end = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V6\0");
    hash.update(u64::try_from(end).unwrap().to_le_bytes());
    hash.update(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash.finalize());
    fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(bytes).unwrap();
}

#[test]
fn publication_admitted_model_constructs_matched_live_v6_and_formal_v5() {
    let pair = admitted_model_pair(false);
    let middle = fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(&pair.middle).unwrap();
    let formal =
        fe2o3_lower_mir_kernel::InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&pair.formal)
            .unwrap();
    assert_eq!(middle.source_semantic_identity(), &pair.semantic);
    assert_eq!(formal.source_semantic_identity(), &pair.semantic);
    assert_eq!(
        middle.live_ranked_graph_identity(),
        formal.ranked_graph_identity()
    );
    assert_eq!(
        middle
            .static_publication()
            .unwrap()
            .potentially_conflicting_cell_pairs(),
        128
    );
    assert_eq!(
        formal
            .static_publication()
            .unwrap()
            .ranked_potentially_conflicting_cell_pairs(),
        128
    );
    assert_eq!(formal.unresolved_inter_invocation_conflict_count(), 0);
    assert!(!formal.grants_authority());
    assert!(!middle.grants_artifact_or_launch_authority());
    assert_eq!(
        validate_formal_middle_end_pair_v5(&pair.middle, &pair.formal, &pair.semantic).unwrap(),
        pair.kir
    );
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&pair.owner).is_err());
    assert!(InertProductionMiddleEndEvidenceV5::decode(&pair.middle).is_err());
    assert!(
        prepare_formal_admission_payload_v5(
            &pair.owner,
            false,
            &pair.middle,
            &pair.semantic,
            pair.kir
        )
        .is_err()
    );
}

#[test]
fn publication_admitted_model_literal_bound_requires_actual_flags_length() {
    use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1;
    use fe2o3_pliron::{
        ProductionRankedCompileErrorV1, ProductionSessionErrorV1, RankedBoundsFindingV1,
    };

    // Unlike the default cell < flags.len() guard, this original source guard
    // still admits cell 127 when the actual flags extent is only 127.
    let error = match project_admitted_publication_model(request_with_flag_bound(false, 127, true))
    {
        Ok(_) => panic!("the short flag allocation passed mandatory bounds"),
        Err(error) => error,
    };
    let ProductionRankedProjectionErrorV1::Compile { error, .. } = *error else {
        panic!("the structurally admitted negative did not reach ranked checks");
    };
    let ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(bounds)) =
        *error
    else {
        panic!("the negative failed outside mandatory ranked bounds");
    };
    let mut reads = 0;
    let mut writes = 0;
    for finding in bounds.report().findings() {
        let access = match finding {
            RankedBoundsFindingV1::UnprovedBound { access, .. }
            | RankedBoundsFindingV1::StaticOutOfBounds { access, .. }
            | RankedBoundsFindingV1::PresburgerOutOfBounds { access, .. } => access,
            _ => panic!("unexpected non-access bounds failure: {finding:?}"),
        };
        match access {
            dialect_kernel::AccessKindAttr::AtomicRead => reads += 1,
            dialect_kernel::AccessKindAttr::AtomicWrite => writes += 1,
            _ => panic!("the independent payload bounds unexpectedly failed"),
        }
    }
    assert_eq!((reads, writes), (1, 2));
}

#[test]
fn publication_admitted_model_literal_bound_accepts_exact_flags_length() {
    let pair = admitted_model_pair_from_request(request_with_flag_bound(false, 128, true));
    assert_eq!(
        validate_formal_middle_end_pair_v5(&pair.middle, &pair.formal, &pair.semantic).unwrap(),
        pair.kir,
    );
}

#[test]
fn publication_admitted_model_pair_rejects_crosswired_source_graph_sites_and_kir() {
    let left = admitted_model_pair(false);
    let right = admitted_model_pair(true);
    assert_ne!(left.semantic, right.semantic);
    assert_ne!(left.kir, right.kir);
    assert!(
        validate_formal_middle_end_pair_v5(&left.middle, &left.formal, &right.semantic).is_err()
    );
    assert!(
        validate_formal_middle_end_pair_v5(&right.middle, &left.formal, &left.semantic).is_err()
    );
    assert!(
        prepare_formal_admission_payload_v5(
            &left.owner,
            true,
            &left.middle,
            &left.semantic,
            right.kir
        )
        .is_err()
    );
    let left_decoded =
        fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(&left.middle).unwrap();
    let right_decoded =
        fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(&right.middle).unwrap();
    assert_ne!(
        left_decoded.live_ranked_graph_identity(),
        right_decoded.live_ranked_graph_identity()
    );
    assert_ne!(
        left_decoded.static_publication().unwrap().sites(),
        right_decoded.static_publication().unwrap().sites()
    );
    let graph_start = left.middle.len() - 32 - 88 - 32;
    let mut foreign_graph = left.middle.clone();
    assert_eq!(
        &foreign_graph[graph_start..graph_start + 32],
        left_decoded.live_ranked_graph_identity()
    );
    foreign_graph[graph_start..graph_start + 32]
        .copy_from_slice(right_decoded.live_ranked_graph_identity());
    reseal_middle(&mut foreign_graph);
    assert!(
        validate_formal_middle_end_pair_v5(&foreign_graph, &left.formal, &left.semantic).is_err()
    );
    let mut foreign_sites = left.middle.clone();
    let left_sites = left.middle.len() - 32 - 88 + 24;
    let right_sites = right.middle.len() - 32 - 88 + 24;
    foreign_sites[left_sites..left_sites + 48]
        .copy_from_slice(&right.middle[right_sites..right_sites + 48]);
    reseal_middle(&mut foreign_sites);
    assert!(
        validate_formal_middle_end_pair_v5(&foreign_sites, &left.formal, &left.semantic).is_err()
    );
}
