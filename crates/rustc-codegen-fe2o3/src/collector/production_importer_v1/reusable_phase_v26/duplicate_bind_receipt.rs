//! Test-only child of reusable_phase_v26, called after the actual positive
//! source import and checked SSA transition. Not a canonical-byte mutation.
use super::*;

pub(super) fn duplicate_bind_receipt<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) {
    owner
        .verify_replay()
        .expect("actual admitted SSA owner must replay before mutation");
    AuthenticatedProductionKernelContextsV1::validate_reusable_phase_source_ssa(
        Some(contexts),
        owner,
    )
    .expect("the real live source/SSA transition must succeed before the negative");
    let semantic = owner.source_semantic();
    let [root] = semantic.roots() else {
        panic!("actual phase fixture has one root");
    };
    let view = owner.execution_view_for_root(*root).unwrap();
    let original = execution_source::CheckedSource::observe(tcx, plan, contexts, semantic)
        .expect("unmodified source must authenticate before any receipt mutation");
    original
        .bind_expansion(owner.execution_expansion(), view)
        .expect("unmodified source occurrence roster must replay");

    let mut changed =
        execution_source::CheckedSource::observe(tcx, plan, contexts, semantic).unwrap();
    let execution_source::Protocols::Owned(protocols) = &mut changed.protocols else {
        panic!("live test observation must own its original protocol roster");
    };
    assert_eq!(protocols.len(), 2);
    assert_eq!(protocols[0].binds.len(), 1);
    let first = &protocols[0].binds[0];
    let duplicate = source_protocol::Bind {
        site: first.site,
        storage: first.storage,
        lease_binding: first.lease_binding,
    };
    protocols[0].binds.push(duplicate);
    match changed.bind_expansion(owner.execution_expansion(), view) {
        Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "phase execution omitted or reused an expanded occurrence",
        )) => {}
        Err(error) => panic!("duplicate admitted-source receipt hit the wrong boundary: {error:?}"),
        Ok(_) => panic!("duplicate Bind receipt crossed the checked occurrence boundary"),
    }
    owner
        .verify_replay()
        .expect("test mutation must not alter the real semantic/SSA owner");
}
