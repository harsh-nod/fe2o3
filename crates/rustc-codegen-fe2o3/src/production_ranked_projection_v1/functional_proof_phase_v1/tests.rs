use fe2o3_mir_model::semantic_mir_v1::*;

use super::*;

mod fixture;
use fixture::source_fixture;

#[test]
fn phase_preflight_accepts_only_complete_homogeneous_phase_markers() {
    use FunctionalProofPhaseKindV1::{Completed, Prepared, Source};
    for expected in [Source, Prepared, Completed] {
        require_phase_roster([Some(expected), Some(expected)], expected).unwrap();
        for absent in [vec![], vec![None], vec![Some(expected), None]] {
            assert!(require_phase_roster(absent, expected).is_err());
        }
        for other in [Source, Prepared, Completed]
            .into_iter()
            .filter(|kind| *kind != expected)
        {
            assert!(require_phase_roster([Some(other)], expected).is_err());
            assert!(require_phase_roster([Some(expected), Some(other)], expected).is_err());
        }
    }
}

#[test]
fn actual_ranked_owners_retain_exact_identity_order_and_lineage_without_a_runtime() {
    let (owner, roster) = source_fixture();
    let evidence = roster
        .roots()
        .iter()
        .map(|root| {
            root.verification()
                .middle_end_evidence()
                .canonical_bytes()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let induction = roster
        .roots()
        .iter()
        .map(|root| root.verification().semantic_u32_induction().clone())
        .collect::<Vec<_>>();
    roster.require_roster_identity().unwrap();
    roster.require_source_owner(&owner).unwrap();
    roster
        .require_kernel_roster(&owner.module().kernels)
        .unwrap();
    assert_eq!(roster.canonical_kernel_order(), &[1, 0]);
    assert_eq!(roster.roots()[0].logical_name(), "phase_alpha");
    for ((root, bytes), report) in roster.roots().iter().zip(evidence).zip(induction) {
        let verification = root.verification();
        assert_eq!(verification.middle_end_evidence().canonical_bytes(), bytes);
        assert_eq!(verification.semantic_u32_induction(), &report);
        assert!(verification.aggregate_verus_execution().is_none());
        assert!(verification.prepared_final_execution().is_none());
        assert!(!verification.has_authenticated_functional_verification());
    }
}

#[test]
fn structural_evidence_alone_cannot_enter_any_functional_phase_or_terminal_roster() {
    let (_, roster) = source_fixture();
    // Preserve the old optional coherence query, but never confuse it with a
    // complete authenticated functional proof roster.
    assert!(roster.every_functional_verification_is_coherent());
    for phase in [
        FunctionalProofPhaseKindV1::Source,
        FunctionalProofPhaseKindV1::Prepared,
        FunctionalProofPhaseKindV1::Completed,
    ] {
        assert!(roster.require_functional_phase(phase).is_err());
    }
    assert!(roster.into_final_graph_functional_roster().is_err());
    let (_, roster) = source_fixture();
    assert!(
        roster
            .into_completed_final_graph_functional_roster()
            .is_err()
    );
}

#[test]
fn missing_extra_reordered_and_substituted_actual_roots_fail_exact_roster_checks() {
    let (_, mut roster) = source_fixture();
    let identity = roster.canonical_roster_identity;
    roster.canonical_roster_identity = ProductionRankedKernelRosterIdentityV1([0; 32]);
    assert!(roster.require_roster_identity().is_err());
    roster.canonical_roster_identity = identity;
    roster.canonical_kernel_order.swap(0, 1);
    assert!(roster.require_roster_identity().is_err());
    roster.canonical_kernel_order.swap(0, 1);
    roster.roots.swap(0, 1);
    assert!(roster.require_roster_identity().is_err());
    roster.roots.swap(0, 1);
    let name = std::mem::replace(&mut roster.roots[0].logical_name, "substituted".into());
    assert!(roster.require_roster_identity().is_err());
    roster.roots[0].logical_name = name;
    let mut roots = roster.roots.into_vec();
    let root = roots.pop().unwrap();
    roster.roots = roots.into_boxed_slice();
    assert!(roster.require_roster_identity().is_err());
    let mut roots = roster.roots.into_vec();
    roots.push(root);
    let (_, extra) = source_fixture();
    roots.extend(extra.roots.into_vec());
    roster.roots = roots.into_boxed_slice();
    assert!(roster.require_roster_identity().is_err());
}

#[test]
fn every_original_identity_record_field_remains_bound() {
    let (_, roster) = source_fixture();
    let records = roster
        .roots
        .iter()
        .map(authenticated_identity_record)
        .collect::<Vec<_>>();
    let mutations: &[fn(&mut RankedRosterIdentityRecordV1<'_>)] = &[
        |r| r.logical_name = "other",
        |r| r.export_symbol = b"other",
        |r| r.semantic_root = SemanticFunctionIdV1::from_index(8),
        |r| r.semantic_root_identity = SemanticFunctionIdentityV1::from_sha256([88; 32]),
        |r| r.kernel_binding = [88; 32],
        |r| r.source_rank = 3,
        |r| r.middle_end_identity_sha256 = [88; 32],
        |r| r.middle_end_identity_byte_len += 1,
        |r| r.induction_semantic_mir_sha256 = [88; 32],
        |r| r.induction_function = SemanticFunctionIdV1::from_index(8),
        |r| r.induction_function_identity = SemanticFunctionIdentityV1::from_sha256([88; 32]),
        |r| r.induction_execution_view_identity = Some([88; 32]),
        |r| r.induction_checked_additions_examined += 1,
        |r| r.induction_certificate_count += 1,
        |r| r.induction_work_units += 1,
    ];
    for mutate in mutations {
        let mut changed = records.clone();
        mutate(&mut changed[0]);
        assert!(
            require_exact_ranked_kernel_roster_identity_v1(
                &changed,
                roster.canonical_roster_identity(),
                roster.canonical_kernel_order(),
            )
            .is_err()
        );
    }
}

#[test]
fn actual_source_owner_join_rejects_cross_wired_root_metadata_and_induction() {
    let (owner, mut roster) = source_fixture();
    let binding = roster.roots[0].kernel_binding;
    roster.roots[0].kernel_binding = [99; 32];
    assert!(roster.require_source_owner(&owner).is_err());
    roster.roots[0].kernel_binding = binding;
    roster.roots.swap(0, 1);
    assert!(roster.require_source_owner(&owner).is_err());
    roster.roots.swap(0, 1);
    let (first, rest) = roster.roots.split_first_mut().unwrap();
    std::mem::swap(
        &mut first.verification.semantic_u32_induction,
        &mut rest[0].verification.semantic_u32_induction,
    );
    assert!(roster.require_source_owner(&owner).is_err());
}

#[test]
fn canonical_roster_join_requires_exact_export_entry_rank_and_completeness() {
    use fe2o3_kernel_ir::{FunctionId, LaunchDomain, LaunchExtent};
    let (owner, roster) = source_fixture();
    let kernels = &owner.module().kernels;
    roster.require_kernel_roster(kernels).unwrap();
    assert!(roster.require_kernel_roster(&kernels[..1]).is_err());
    let mut substituted = kernels.to_vec();
    substituted[0] = substituted[1].clone();
    assert!(roster.require_kernel_roster(&substituted).is_err());
    let mut substituted = kernels.to_vec();
    substituted[0].entry = FunctionId::new("substituted");
    assert!(roster.require_kernel_roster(&substituted).is_err());
    let mut substituted = kernels.to_vec();
    substituted[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Static(1),
        y: LaunchExtent::Static(1),
    };
    assert!(roster.require_kernel_roster(&substituted).is_err());
    let mut extra = kernels.to_vec();
    extra.push(kernels[0].clone());
    assert!(roster.require_kernel_roster(&extra).is_err());
}

#[test]
fn phase_completion_and_completed_extraction_require_no_runtime_parameter() {
    let _: fn(
        AuthenticatedRankedVerificationRosterV1,
        &mut ProductionVerifiedFinalGraphV1,
    ) -> Result<AuthenticatedRankedVerificationRosterV1, Failure> =
        AuthenticatedRankedVerificationRosterV1::complete_final_graph_functional;
    let _: fn(
        AuthenticatedRankedVerificationRosterV1,
    ) -> Result<AuthenticatedCompletedFinalGraphFunctionalRosterV1, Failure> =
        AuthenticatedRankedVerificationRosterV1::into_completed_final_graph_functional_roster;
    let _: fn(
        AuthenticatedCompletedFinalGraphFunctionalRootV1,
    ) -> ProductionFinalGraphFunctionalRefinementExecutionV2 =
        AuthenticatedCompletedFinalGraphFunctionalRootV1::into_execution;
    let _: for<'a> fn(
        &'a AuthenticatedRankedVerificationV5,
    ) -> Option<
        &'a fe2o3_verifier::ProductionMirPlironPerCompilationVerusExecutionV1,
    > = AuthenticatedRankedVerificationV5::aggregate_verus_execution;
}
