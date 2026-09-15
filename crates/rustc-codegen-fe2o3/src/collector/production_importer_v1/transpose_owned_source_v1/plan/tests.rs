use super::*;

pub(in super::super) fn check<'tcx>(tcx: TyCtxt<'tcx>, auth: &Authentication<'_, 'tcx>) {
    for mutation in 0..6 {
        let mut work = 1_048_576;
        let mut observed = SourcePlan::observe(tcx, auth, &mut work).unwrap();
        assert_eq!(observed.flows.len(), 1);
        match mutation {
            0 => {
                observed.flows.clear();
            }
            1 => {
                let original = &observed.flows[0];
                // Test-only corruption of private replay state, not a public
                // source receipt factory or refreshed frontend authority.
                let duplicate = CheckedFlow {
                    nodes: original.nodes.clone(),
                    coordinates: original.coordinates.clone(),
                    source_binding: original.source_binding,
                };
                observed.flows.push(duplicate);
            }
            2 => {
                observed.flows[0].source_binding[0] ^= 1;
            }
            3 => {
                observed.flows[0].nodes.capture_field = u32::MAX;
            }
            4 => {
                assert!(observed.flows[0].nodes.borrows.pop().is_some());
            }
            5 => {
                let flow = &mut observed.flows[0].coordinates;
                std::mem::swap(&mut flow.issue, &mut flow.stage);
            }
            _ => unreachable!(),
        }
        let expected = if mutation < 2 {
            "transpose source roster changed on replay"
        } else {
            "transpose source row changed on replay"
        };
        match observed.replay(tcx, auth, &mut work) {
            Err(PlanError::Source(Error::Source(message))) => assert_eq!(message, expected),
            Err(other) => {
                panic!("source mutation {mutation} reached the wrong rejection: {other:?}")
            }
            Ok(_) => panic!("source mutation {mutation} was replayed as original custody"),
        }
        assert!(work > 0, "a replay mismatch is not a budget-only negative");
    }
}
