//! Invoked only with the real private result of the registered two-phase
//! source callback. No canonical fields or standalone SSA fixture issue it.
use super::*;

#[path = "emission_completion_tests.rs"]
mod completion;

pub(in super::super::super) fn completion_boundary(checked: CheckedSsa<'_, '_>, case: usize) {
    completion::check(checked, case);
}

pub(in super::super::super) fn positive(checked: CheckedSsa<'_, '_>) {
    assert_eq!(checked.phases.len(), 2);
    assert!(checked.phases.iter().all(|phase| phase.leases.len() == 1));
    let owner = checked.owner;
    let mut counts = [0usize; 8];
    checked.into_emission_owner().expect("actual source has a complete checked emission boundary")
        .consume(|actual, row| {
            assert!(std::ptr::eq(actual.owner, owner));
            assert!(row.phase() < 2);
            let index = match row.action() {
                Action::OwnerConvert => 0, Action::Begin => 1,
                Action::Bind { lease: 0 } => 2, Action::Seal => 3,
                Action::RelayClosure => 4, Action::RelayDrop => 5,
                Action::CloseStorage { lease: 0 } => 6, Action::End => 7,
                Action::Bind { .. } | Action::CloseStorage { .. } => panic!("fixture lease roster changed"),
            };
            counts[index] += 1;
            let boundary = row.boundary();
            assert_eq!(boundary.site().block().index(), boundary.point().block.get());
            match row.action() {
                Action::CloseStorage { .. } | Action::End => assert_eq!(boundary.kind(), BoundaryEvent::Kill),
                Action::RelayDrop => assert_eq!(boundary.kind(), BoundaryEvent::Use),
                _ => assert_eq!(boundary.kind(), BoundaryEvent::Define),
            }
            Ok(())
        }).expect("all fifteen original lifecycle boundaries must be consumed");
    assert_eq!(counts, [1, 2, 2, 2, 2, 2, 2, 2]);
}

pub(in super::super::super) fn substitution(mut checked: CheckedSsa<'_, '_>, case: usize) {
    assert_eq!(checked.phases.len(), 2);
    let expected = match case {
        0 => {
            checked.phases.pop();
            "phase emission lost its complete checked phase roster"
        }
        1 => {
            checked.phases[0].leases.pop();
            "phase emission lost its complete allocation roster"
        }
        2 => {
            checked.phases[0].emission_issue.value = checked.phases[0].converted_owner;
            "phase emission result changed its original checked binding"
        }
        3 => {
            checked.phases[0].emission_drop.kind = BoundaryEvent::Define;
            "phase emission lost its exact completion relay chain"
        }
        4 => {
            checked.phases[0].leases[0].emission_close.point.event += 1;
            "phase emission lease changed its checked definition or close"
        }
        5 => {
            checked.phases.swap(0, 1);
            "phase emission result changed its original checked binding"
        }
        6 => {
            checked.phases[0].completion = checked.phases[0].issued_phase;
            "phase emission result changed its original checked binding"
        }
        7 => {
            checked.expansion.source.remaining_work = 0;
            "phase live source-work ceiling"
        }
        _ => panic!("unknown exact source emission mutation"),
    };
    match checked.into_emission_owner() {
        Err(super::super::super::ProductionSemanticImportErrorV1::KernelContextBinding(actual)) => assert_eq!(actual, expected),
        Err(error) => panic!("expected exact {expected}, got {error:?}"),
        Ok(_) => panic!("substituted emission boundary was accepted: {expected}"),
    }
}
