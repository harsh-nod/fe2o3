//! Run from the existing two-phase rustc callback after all original source,
//! custody, replay and mutation assertions. No standalone record issues phases.
use super::*;
use fe2o3_kernel_ir::{OperationKind, ReusablePhaseOperationV1 as PhaseOp};

pub(super) fn lower(
    imported: crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1,
    ssa: fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
) {
    assert_original_epoch_and_relay_namespaces(&ssa);
    let expected_begin_brands = begin_brand_regression::collect(&ssa);
    let expected_barriers = synchronization_regression::collect(&ssa);
    use crate::production_ranked_projection_v1::{
        ProductionRankedRootInputV1, project_and_verify_ranked_semantic_mir_with_contexts_v1,
    };
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
    .expect("actual phase source must reach the unchanged complete ranked checks");
    assert!(ranked.all_kernel_checks_are_clean());
    let source = imported
        .kernel_contexts
        .into_consumed_source_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            typed_roots,
        )
        .expect("exact Context and live source seal must move together into lowering");
    let roster = ranked.into_verified_roster_receipt().unwrap();
    roster.verify_equivalence().unwrap();
    assert!(!roster.grants_artifact_or_launch_authority());
    let (receipt, ranked_checks) = roster.into_module_verified_receipt().unwrap();
    assert!(ranked_checks.every_functional_verification_is_coherent());
    let lowered = source
        .lower(
            receipt,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        )
        .expect("actual two-phase source requires complete consumed-source KIR emission");
    lowered.verify_equivalence().unwrap();
    assert_eq!(lowered.correspondence().semantic_sha256(), &source_identity);
    assert!(lowered.canonical_kernel_ir_v13().is_none());
    assert_eq!(
        lowered.canonical_kernel_ir().version(),
        fe2o3_kernel_ir::CanonicalKernelIrVersionV1::V14
    );
    lowered.canonical_kernel_ir().revalidate().unwrap();
    begin_brand_regression::check(lowered.module(), &expected_begin_brands);
    synchronization_regression::check(
        lowered.module(),
        lowered.correspondence(),
        &expected_barriers,
    );
    epoch_regression::check(lowered.module());
    let mut counts = [0usize; 8];
    let mut begins = Vec::new();
    let mut ended_owners = Vec::new();
    let mut ended_storage = Vec::new();
    let mut binds = Vec::new();
    for function in &lowered.module().functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                let OperationKind::ReusablePhase(contract) = &operation.kind else {
                    continue;
                };
                let index = match contract.operation {
                    PhaseOp::OwnerConvert { .. } => 0,
                    PhaseOp::Begin { .. } => {
                        begins.push(contract.operands[0]);
                        1
                    }
                    PhaseOp::Bind { .. } => {
                        binds.push(contract.operands[2]);
                        2
                    }
                    PhaseOp::Seal { .. } => 3,
                    PhaseOp::RelayClosure { .. } => 4,
                    PhaseOp::RelayDrop { .. } => 5,
                    PhaseOp::CloseStorage { .. } => 6,
                    PhaseOp::End { storage_count: 1 } => {
                        assert_eq!(operation.results.len(), 2);
                        ended_owners.push(operation.results[0].id);
                        ended_storage.push(operation.results[1].id);
                        7
                    }
                    PhaseOp::End { .. } => {
                        panic!("actual fixture changed its exact allocation roster")
                    }
                };
                counts[index] += 1;
            }
        }
    }
    assert_eq!(counts, [1, 2, 2, 2, 2, 2, 2, 2]);
    assert_eq!(
        begins[1], ended_owners[0],
        "second Begin must consume the actual first End result"
    );
    assert_eq!(
        binds[1], ended_storage[0],
        "second Bind must consume the restored exact allocation"
    );
    assert_ne!(begins[0], begins[1]);
    assert_ne!(binds[0], binds[1]);
    assert_ne!(ended_owners[0], ended_owners[1]);
    assert_ne!(ended_storage[0], ended_storage[1]);
}

fn assert_original_epoch_and_relay_namespaces(ssa: &fe2o3_pliron::ProductionSemanticSsaOwnerV1) {
    use fe2o3_mir_model::SsaResolvedEventV1;

    ssa.verify_replay().unwrap();
    let root = ssa.source_semantic().roots()[0];
    let view = ssa.execution_view_for_root(root).unwrap();
    let plan = ssa.execution_plan_for_root(root).unwrap();
    assert_eq!(plan.function_identity(), view.body().identity());
    let locals = view.body().locals();
    let relays = plan.defined_reusable_phase_relays();
    assert_eq!(
        relays.len(),
        2,
        "original two-phase fixture must retain both completion relays"
    );
    let mut definitions = [0usize; 2];
    let mut original_definitions = 0;
    for block in plan.plan().reverse_postorder() {
        for (_, event) in plan.plan().resolved_events(*block).unwrap() {
            let SsaResolvedEventV1::Define { variable, .. } = event else {
                continue;
            };
            let variable_index = variable.get() as usize;
            if locals.get(variable_index).is_some() {
                original_definitions += 1;
                continue;
            }
            let index = variable_index.checked_sub(locals.len()).unwrap();
            let relay = relays.get(index).expect("unknown generated SSA variable");
            assert_eq!(relay.variable(), *variable);
            assert_eq!(relay.pack_block().index(), block.get());
            assert_eq!(variable_index, locals.len() + index);
            definitions[index] += 1;
        }
    }
    assert_eq!(definitions, [1, 1]);
    assert!(
        original_definitions > 0,
        "relay scan must not replace the original source roster"
    );
    // The unchanged emission checks below require both actual cursor consumptions
    // and second-phase reuse of the first End's Workgroup and storage results.
}

#[path = "emission_begin_brand_tests.rs"]
mod begin_brand_regression;

#[path = "emission_synchronization_tests.rs"]
mod synchronization_regression;

#[path = "emission_epoch_tests.rs"]
mod epoch_regression;
