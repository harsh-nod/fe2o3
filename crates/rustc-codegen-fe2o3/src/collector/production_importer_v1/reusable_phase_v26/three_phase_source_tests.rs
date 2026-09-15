//! New source input; original two-phase fixtures and callbacks stay unchanged.
use super::*;
use crate::production_ranked_projection_v1::{
    ProductionRankedRootInputV1, project_and_verify_ranked_semantic_mir_with_contexts_v1,
};
use crate::production_target_v1::RetainedProductionTargetV1;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "three_phase_epoch_tests.rs"]
mod epochs;
#[path = "three_phase_occurrence_tests.rs"]
mod occurrences;

pub(in crate::collector::production_importer_v1) const SOURCE: &str =
    include_str!("three_phase_source.rs");

pub(in crate::collector::production_importer_v1) fn check(tcx: TyCtxt<'_>) {
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("registered three-phase Rust source");
    let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
    let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots).unwrap();
    let closure_types = closure
        .collection
        .functions
        .iter()
        .filter_map(|function| function.closure_plan.as_ref())
        .flat_map(|plan| plan.authenticated_closure_type_identities())
        .map(SemanticTypeIdentityV1::from_sha256)
        .collect::<std::collections::BTreeSet<_>>();
    let plan = build_production_semantic_preflight_plan_v1(
        tcx,
        canonical_target_layout_v1(observed.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .unwrap();
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("complete original three-phase canonical import");
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    let typed_roots =
        crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(typed_roots, mir)
            .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed_roots,
        mir,
    )
    .unwrap();
    mir.require_complete_external_entries().unwrap();
    let exact = AdmittedInertSemanticMirV1::decode_exact_v26_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(exact.canonical_encoding(), mir.canonical_encoding());
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(exact, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("complete three-phase consuming SSA transport");
    ssa.verify_replay().unwrap();
    AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(
        Some(&imported.kernel_contexts),
        &ssa,
    )
    .expect("same live source seal and SSA owner");
    let source = execution_source::CheckedSource::observe(
        tcx,
        &plan,
        &imported.kernel_contexts,
        ssa.source_semantic(),
    )
    .unwrap();
    assert_eq!(source.protocols.len(), 3);
    let [root] = ssa.source_semantic().roots() else {
        panic!("one source root");
    };
    let checked = source
        .bind_expansion(
            ssa.execution_expansion(),
            ssa.execution_view_for_root(*root).unwrap(),
        )
        .unwrap()
        .bind_ssa(&ssa)
        .unwrap();
    assert!(std::ptr::eq(checked.owner, &ssa));
    let expected = occurrences::collect(&checked);
    drop(checked);
    let source_identity = *ssa.source_semantic().semantic_sha256().as_bytes();
    let inputs = typed_roots
        .iter()
        .map(|root| {
            ProductionRankedRootInputV1::new(
                root.logical_name(),
                root.kernel_binding_bytes(),
                root.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = project_and_verify_ranked_semantic_mir_with_contexts_v1(
        ssa,
        &inputs,
        &imported.reference_effect_bindings,
        Some(&imported.kernel_contexts),
    )
    .expect("unchanged complete ranked checks on real three-phase source");
    assert!(ranked.all_kernel_checks_are_clean());
    let source = imported
        .kernel_contexts
        .into_consumed_source_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            &typed_roots,
        )
        .unwrap();
    let roster = ranked.into_verified_roster_receipt().unwrap();
    roster.verify_equivalence().unwrap();
    assert!(!roster.grants_artifact_or_launch_authority());
    let (receipt, checks) = roster.into_module_verified_receipt().unwrap();
    assert!(checks.every_functional_verification_is_coherent());
    let lowered = source
        .lower(
            receipt,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        )
        .expect("original three-phase source must emit complete KIR");
    lowered.verify_equivalence().unwrap();
    assert_eq!(lowered.correspondence().semantic_sha256(), &source_identity);
    assert!(lowered.canonical_kernel_ir_v13().is_none());
    assert_eq!(
        lowered.canonical_kernel_ir().version(),
        fe2o3_kernel_ir::CanonicalKernelIrVersionV1::V14
    );
    lowered.canonical_kernel_ir().revalidate().unwrap();
    epochs::check(lowered.module(), lowered.correspondence(), &expected);
}
