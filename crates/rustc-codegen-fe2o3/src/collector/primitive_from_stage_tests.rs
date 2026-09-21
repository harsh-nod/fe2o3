//! Real collector and boundary methods, not an authenticated kernel-root fixture.
use super::*;

pub(crate) fn collect_and_replay<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    block: BasicBlock,
) -> crate::rustc_semantic_plan_v1::SourceClosureWorkV1 {
    let mut collector = DeviceCollector::new(
        tcx,
        false,
        Vec::new(),
        "host-test-no-launch".into(),
        capture_context_producers_v1(tcx).unwrap(),
    );
    collector.closure_work.charge(17).unwrap();
    let TerminatorKind::Call { func, .. } = &body.basic_blocks[block].terminator().kind else {
        unreachable!()
    };
    collector
        .process_call_operand(func, &caller, body, block)
        .unwrap();
    assert!(collector.closure_work.validation_work_for_test() > 17);
    assert!(collector.result.is_empty());
    assert!(collector.worklist.is_empty());
    assert!(collector.reachable_unsafe_calls.is_empty());
    assert!(collector.ffi_declarations.is_empty());
    let callee = crate::closure_profile_v1::resolve_direct_call(tcx, caller, func).unwrap();
    closure_flow_v1::primitive_from_stage_tests::replay(tcx, callee, &mut collector.closure_work);
    collector.closure_work
}

pub(crate) fn reject_original_panic<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) {
    let mut collector = DeviceCollector::new(
        tcx,
        false,
        Vec::new(),
        "host-test-no-launch".into(),
        capture_context_producers_v1(tcx).unwrap(),
    );
    let body = tcx.instance_mir(caller.def);
    let mut calls = 0;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        if let TerminatorKind::Call { func, .. } = &data.terminator().kind {
            let error = collector
                .process_call_operand(func, &caller, body, block)
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("device code reaches a panic path")
            );
            calls += 1;
        }
    }
    assert_eq!(calls, 1);
}
