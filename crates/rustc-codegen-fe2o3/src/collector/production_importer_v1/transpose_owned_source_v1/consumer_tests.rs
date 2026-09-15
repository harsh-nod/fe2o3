//! Called from the real-source opt-in test after its original positive checks.
use super::*;

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    owner: &ProductionSemanticSsaOwnerV1,
) {
    let view = owner
        .execution_view_for_root(auth.root.selected_root)
        .unwrap();
    let query = owner
        .source_query_for_root(auth.root.selected_root, view.body())
        .unwrap();
    let mut work = WORK;
    let bound = mapped(tcx, auth)
        .bind(owner, &mut work)
        .unwrap()
        .into_flows(owner)
        .unwrap();
    assert!(!bound.is_empty());
    for flow in bound {
        for use_ in [
            &flow.issue_partition,
            &flow.matrix_subgroup,
            &flow.matrix_epoch,
        ] {
            assert!(use_.belongs_to(&query));
            assert_eq!(use_.agreeing_uses(), 1);
            assert!(matches!(
                use_.operand(),
                SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
            ));
        }
        assert_eq!(
            flow.execution_sites[0],
            flow.issue_partition.site().block().index()
        );
        assert_eq!(flow.execution_sites[1], flow.stage.site.block().index());
        assert_eq!(flow.execution_sites[2], flow.publish.site.block().index());
        assert_eq!(
            flow.matrix_subgroup.site().block(),
            flow.matrix_epoch.site().block()
        );
        assert_ne!(
            flow.matrix_subgroup.site().statement(),
            flow.matrix_epoch.site().statement()
        );
    }

    let mut omitted = mapped(tcx, auth);
    omitted.flows.clear();
    let mut work = WORK;
    assert!(matches!(
        error(omitted.bind(owner, &mut work)),
        PlanError::Source(Error::Source(
            "transpose source occurrence roster incomplete",
        ))
    ));

    let mut duplicate = mapped(tcx, auth);
    let repeated = mapped(tcx, auth)
        .flows
        .pop()
        .expect("live source has a row");
    duplicate.flows.push(repeated);
    let mut work = WORK;
    assert!(matches!(
        error(duplicate.bind(owner, &mut work)),
        PlanError::Source(Error::Source("transpose source occurrence consumed twice",))
    ));
}
