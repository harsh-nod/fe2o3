use super::*;
use rustc_middle::mir::Promoted;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    helper: Helper<'tcx>,
    source: &Body<'tcx>,
    foreign: Instance<'tcx>,
) {
    let accepts = |body: &Body<'tcx>| prove_body(tcx, instance, body, helper).is_some();
    assert!(accepts(source));
    for owner in [
        foreign.def,
        rustc_middle::ty::InstanceKind::Intrinsic(instance.def_id()),
    ] {
        let mut changed = source.clone();
        changed.source.instance = owner;
        assert!(reviewed_body(tcx, &changed, helper));
        assert!(!accepts(&changed), "different source owner");
    }
    let mut changed = source.clone();
    changed.source.promoted = Some(Promoted::from_usize(0));
    assert!(reviewed_body(tcx, &changed, helper));
    assert!(!accepts(&changed), "promoted source");
    let mut changed = source.clone();
    // Mark only the cloned negative fixture; emitting a real diagnostic would
    // abort the shared rustc callback before the remaining mutations run.
    #[allow(deprecated)]
    let error = rustc_span::ErrorGuaranteed::unchecked_error_guaranteed();
    changed.tainted_by_errors = Some(error);
    assert!(reviewed_body(tcx, &changed, helper));
    assert!(!accepts(&changed), "error-tainted source");

    let debug = source
        .var_debug_info
        .first()
        .expect("pinned argument debug info");
    for count in [MAX_DEBUG_INFO - 1, MAX_DEBUG_INFO, MAX_DEBUG_INFO + 1] {
        let mut changed = source.clone();
        changed.var_debug_info = vec![debug.clone(); count];
        assert!(reviewed_body(tcx, &changed, helper));
        assert_eq!(
            accepts(&changed),
            count <= MAX_DEBUG_INFO,
            "debug count={count}"
        );
    }
    let annotation = ty::CanonicalUserTypeAnnotation {
        user_ty: Box::new(ty::CanonicalUserType {
            max_universe: ty::UniverseIndex::ROOT,
            var_kinds: ty::List::empty(),
            value: ty::UserType::new(ty::UserTypeKind::Ty(helper.input(tcx))),
        }),
        span: source.span,
        inferred_ty: helper.input(tcx),
    };
    for count in [
        MAX_USER_TYPE_ANNOTATIONS - 1,
        MAX_USER_TYPE_ANNOTATIONS,
        MAX_USER_TYPE_ANNOTATIONS + 1,
    ] {
        let mut changed = source.clone();
        changed.user_type_annotations.raw.clear();
        for _ in 0..count {
            changed.user_type_annotations.push(annotation.clone());
        }
        assert!(reviewed_body(tcx, &changed, helper));
        assert_eq!(
            accepts(&changed),
            count <= MAX_USER_TYPE_ANNOTATIONS,
            "annotation count={count}"
        );
    }
}
