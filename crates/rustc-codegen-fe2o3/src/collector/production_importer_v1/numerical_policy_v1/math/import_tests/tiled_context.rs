//! Real checked-in tiled source through authenticated import and Context SSA.
//! Failure graph is diagnostic only; this callback does not claim ranked/KIR proof.
use super::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

pub(super) fn input() -> Input {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/tiled_gemm_general_v1/src/lib.rs");
    assert!(
        path.is_file(),
        "actual tiled GEMM crate source must be present"
    );
    Input::File(path)
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
) {
    let typed_roots = closure.rederive_typed_descriptor_roots(tcx).unwrap();
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("import the actual tiled kernel and its original Context initializer");
    let [root] = imported.semantic_mir.roots() else {
        panic!("one actual registered tiled root");
    };
    let root = *root;
    let root_source = &imported.semantic_mir.functions()[root.index() as usize];
    let binding = root_source
        .kernel_entry()
        .unwrap()
        .kernel_binding_identity();
    let typed = typed_roots
        .iter()
        .find(|typed| typed.kernel_binding_bytes() == *binding.as_bytes())
        .expect("actual typed descriptor matches the selected semantic root");
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            imported.semantic_mir,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("actual tiled source reaches retained SSA planning");
    owner.verify_replay().unwrap();
    // Same fixed bound as production ranked entry; no enlarged test budget.
    let max_work = fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_OPERATIONS * 16;
    let relation = imported
        .kernel_contexts
        .checked_ranked_entry(&owner, root, typed.source_launch().unwrap(), max_work)
        .unwrap_or_else(|error| {
            panic!(
                "actual tiled Context entry failed: {error:?}\n{}",
                ordered_context::failure_graph::snapshot(&owner, root),
            )
        });
    assert!(
        relation.is_some(),
        "retain actual authenticated entry-transfer custody"
    );
}

#[test]
#[ignore = "requires cached actual AMD metadata; no Cargo or dependency rebuild"]
fn tiled_context_entry_source_gfx942() {
    run_registered_source(
        "gfx942",
        "tiled_context::tiled_context_entry_source_gfx942",
        false,
        false,
        SourceCase::TiledContext,
    );
}
