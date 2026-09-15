use super::*;

fn replay<'a, 'tcx>(tcx: TyCtxt<'tcx>, auth: &Authentication<'a, 'tcx>) -> MappedPlan<'a> {
    let mut work = 1_048_576;
    SourcePlan::observe(tcx, auth, &mut work)
        .unwrap()
        .replay(tcx, auth, &mut work)
        .unwrap()
}

pub(in super::super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) {
    let mut work = 1_048_576;
    let mut bytes = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::CanonicalBytes);
    let bound = replay(tcx, auth)
        .bind_canonical_footer(owner, &mut bytes, &mut work)
        .unwrap();
    assert!(!bound.into_flows(owner).unwrap().is_empty());

    for mutation in 0..4 {
        let mut changed = replay(tcx, auth);
        match mutation {
            0 => changed.flows[0].source_binding[0] ^= 1,
            1 => {
                changed.flows[0].capture_field =
                    changed.flows[0].capture_field.checked_add(1).unwrap()
            }
            2 => {
                assert!(changed.flows[0].workgroup_borrows.pop().is_some());
            }
            3 => {
                let old = changed.flows[0].workgroup_local.index();
                changed.flows[0].workgroup_local =
                    SemanticLocalIdV1::from_index(u32::from(old == 0));
            }
            _ => unreachable!(),
        }
        let mut work = 1_048_576;
        let mut bytes = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::CanonicalBytes);
        assert!(matches!(
            changed.check_canonical_footer(&mut bytes, &mut work),
            Err(PlanError::Source(Error::Source(
                "transpose canonical source row changed"
            )))
        ));
    }
    let mut omitted = replay(tcx, auth);
    omitted.flows.clear();
    assert!(matches!(
        omitted.check_canonical_footer(&mut bytes, &mut work),
        Err(PlanError::Source(Error::Source(
            "transpose canonical source roster changed"
        )))
    ));

    let mut changed = replay(tcx, auth);
    changed.flows[0].bodies[0].1 = changed.flows[0].bodies[1].1;
    assert!(matches!(
        changed.check_canonical_footer(&mut bytes, &mut work),
        Err(PlanError::Source(Error::Source(
            "transpose canonical body owner changed"
        )))
    ));

    let mut zero = 0;
    assert!(matches!(
        replay(tcx, auth).check_canonical_footer(&mut bytes, &mut zero),
        Err(PlanError::Source(Error::Work))
    ));
    assert_eq!(zero, 0);
}
