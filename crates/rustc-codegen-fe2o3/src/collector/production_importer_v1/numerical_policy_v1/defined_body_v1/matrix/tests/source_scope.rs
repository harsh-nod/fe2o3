use super::*;

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, plan: &ProductionSemanticPreflightPlanV1<'tcx>) {
    let mut checked = [0; 2];
    for producer in plan.function_producers() {
        let instance = producer.instance;
        match source::kind(tcx, instance) {
            Some(source::Kind::Bind) => {
                let facts = source::bind(tcx, instance).unwrap();
                checked[0] += 1;
                let root_bind = replace_type(tcx, instance, 0, facts.identity.root.ty);
                assert_eq!(source::kind(tcx, root_bind), None);
                validate_policy_bind_source_v1(tcx, root_bind).unwrap();
                validate_source(tcx, root_bind).unwrap();
                assert!(
                    source::bind(tcx, root_bind).is_err(),
                    "a root-only Bind cannot produce a subgroup/epoch contract"
                );

                let wrong_policy = replace_type(tcx, root_bind, 2, tcx.types.u16);
                assert_eq!(source::kind(tcx, wrong_policy), Some(source::Kind::Bind));
                assert!(validate_source(tcx, wrong_policy).is_err());
                let wrong_root = replace_type(tcx, root_bind, 1, tcx.types.u16);
                assert_eq!(source::kind(tcx, wrong_root), Some(source::Kind::Bind));
                assert!(validate_source(tcx, wrong_root).is_err());
                let nominal_lookalike = replace_type(tcx, wrong_root, 0, tcx.types.u16);
                assert_eq!(
                    source::kind(tcx, nominal_lookalike),
                    Some(source::Kind::Bind)
                );
                assert!(validate_source(tcx, nominal_lookalike).is_err());
            }
            Some(source::Kind::Narrow) => {
                let facts = source::narrow(tcx, instance).unwrap();
                checked[1] += 1;
                let root_narrow = replace_type(tcx, instance, 0, facts.identity.root.ty);
                assert_eq!(source::kind(tcx, root_narrow), Some(source::Kind::Narrow));
                assert!(
                    validate_source(tcx, root_narrow).is_err(),
                    "narrowing cannot synthesize a subgroup width or epoch from a root brand"
                );
            }
            None => {}
        }
    }
    assert_eq!(checked, [1, 1], "retain both actual scoped source helpers");
}

fn replace_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ordinal: usize,
    replacement: Ty<'tcx>,
) -> Instance<'tcx> {
    let index = instance
        .args
        .iter()
        .enumerate()
        .filter(|(_, argument)| argument.as_type().is_some())
        .nth(ordinal)
        .unwrap()
        .0;
    let mut arguments = instance.args.to_vec();
    arguments[index] = replacement.into();
    Instance {
        args: tcx.mk_args(&arguments),
        ..instance
    }
}
