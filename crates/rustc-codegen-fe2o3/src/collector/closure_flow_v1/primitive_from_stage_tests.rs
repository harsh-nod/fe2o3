use super::*;

pub(in crate::collector) fn replay<'tcx>(
    tcx: TyCtxt<'tcx>,
    callee: Instance<'tcx>,
    work: &mut SourceClosureWorkV1,
) {
    let before = work.validation_work_for_test();
    validate_boundary_v1(tcx, callee, &CollectionResult::default(), work).unwrap();
    assert!(work.validation_work_for_test() > before);
}
