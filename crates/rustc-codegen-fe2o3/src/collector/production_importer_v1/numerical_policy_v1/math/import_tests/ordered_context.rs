//! Actual source custody and retained parameter SSA, not a fabricated Context issuer.
use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaResolvedEventV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "ordered_context/failure_graph.rs"]
pub(super) mod failure_graph;

pub(super) const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, KernelLaunch, KernelTarget, StrictIeee, kernel};

fn attention_phase<Kernel, Target: KernelTarget, Launch: KernelLaunch>(
    context: &mut KernelContext<'_, Kernel, Target, Launch>, x: f32, y: f32, z: f32,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    context.with_workgroup(|_workgroup| {
        let _values = [
            math.sqrt_f32(x), math.mul_add_f32(x, y, z), math.floor_f32(x),
            math.ceil_f32(x), math.trunc_f32(x), math.round_ties_even_f32(x),
            math.sin_f32(x), math.cos_f32(x), math.exp_f32(x), math.exp2_f32(x),
            math.ln_f32(x), math.log2_f32(x), math.log10_f32(x),
        ];
    });
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn policy_math_ordered_context(mut context: KernelContext<'_>, x: f32, y: f32, z: f32) {
    let _before = context.invocation().index_1d().get();
    attention_phase(&mut context, x, y, z);
    let _after = context.invocation().index_1d().get();
}
"#;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
) {
    let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect(
                "import the registered mutable-helper/shared-getter/exclusive-workgroup source",
            );
    let source = &imported.semantic_mir;
    let [root] = source.roots() else {
        panic!("one registered source root");
    };
    let context = imported
        .kernel_contexts
        .roots
        .iter()
        .find(|context| context.selected_root == *root)
        .unwrap();
    let transfer = context
        .entry_transfer
        .expect("retain authenticated original initializer-to-helper custody");
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        source.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("ordered closed reborrows preserve the actual parameter definition");
    owner.verify_replay().unwrap();
    assert_eq!(
        owner.source_semantic().canonical_encoding(),
        source.canonical_encoding()
    );
    let relation = transfer
        .checked_ssa_relation(&owner, *root, 65_536)
        .unwrap_or_else(|error| {
            panic!(
                "the unchanged source/issuer/parameter/lifetime relation must now replay: {error:?}\n{}",
                failure_graph::snapshot(&owner, *root),
            )
        });
    let view = owner.execution_view_for_root(*root).unwrap();
    let plan = owner.execution_plan_for_root(*root).unwrap();
    assert!(relation.matches_body(view.body()));
    let local = relation.destination().index();
    assert!(
        plan.plan()
            .promoted_variables()
            .binary_search_by_key(&local, |v| v.get())
            .is_ok()
    );
    assert!(
        !plan
            .implicit_entry_variables()
            .iter()
            .any(|v| v.get() == local)
    );
    assert!(
        !plan
            .frame_initializations()
            .iter()
            .any(|entry| entry.local().index() == local)
    );
    assert_eq!(
        plan.plan()
            .resolved_events(SsaBlockIdV1::new(relation.block().index()))
            .unwrap()
            .iter()
            .filter(
                |(_, event)| matches!(event, SsaResolvedEventV1::Define { variable, value }
            if variable.get() == local && *value == relation.parameter_value())
            )
            .count(),
        1
    );
    let statement = &view.body().blocks()[relation.block().index() as usize].statements()
        [relation.statement() as usize];
    assert!(
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
        if assignment.destination().local() == relation.destination()
            && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value))
                if value.value() == &SemanticConstantValueV1::ZeroSized))
    );
    let error = transfer
        .checked_ssa_relation(&owner, *root, 0)
        .err()
        .unwrap();
    assert!(
        matches!(error, fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "Context entry SSA relation changed or exceeded its work bound")
    );
    // Existing all-13 bridge/result checks and missing-getter negative remain.
    ssa_results::check(source);
    // Real descriptor roster, ranked checks, original context custody and all
    // existing KIR nominal/occurrence/lifetime mutations, with no synthetic roots.
    lowering::check(imported, typed_roots);
}

#[test]
#[ignore = "requires fresh cached AMD metadata; invokes rustc only, never Cargo"]
fn ordered_context_source_kir_gfx942() {
    run_source(
        "gfx942",
        "ordered_context::ordered_context_source_kir_gfx942",
        false,
        false,
        true,
    );
}

#[test]
#[ignore = "requires fresh cached AMD metadata; invokes rustc only, never Cargo"]
fn ordered_context_source_kir_gfx950() {
    run_source(
        "gfx950",
        "ordered_context::ordered_context_source_kir_gfx950",
        false,
        false,
        true,
    );
}
