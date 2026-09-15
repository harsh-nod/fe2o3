//! Run only from the existing real two-phase callback with a freshly rebound
//! private source seal. This tests the handoff lifetime, not KIR completion.
use super::*;
use std::cell::Cell;

pub(super) fn check(checked: CheckedSsa<'_, '_>, case: usize) {
    let expected_owner = checked.owner;
    let expected_view = checked.expansion.view;
    let mut emission = checked.into_emission_owner().unwrap();
    assert_eq!(emission.rows.len(), 15);
    if case == 3 {
        emission.checked.expansion.source.remaining_work = 15;
    } else if case == 4 {
        emission.checked.expansion.source.remaining_work = 16;
    }
    let initial_work = emission.checked.expansion.source.remaining_work;
    let rows = Cell::new(0usize);
    let finished = Cell::new(false);
    let outcome = emission.consume_metered(
        (),
        |actual, _row, (), work| {
            assert!(std::ptr::eq(actual.owner, expected_owner));
            assert!(std::ptr::eq(actual.expansion.view, expected_view));
            rows.set(rows.get() + 1);
            assert_eq!(actual.expansion.source.remaining_work, 0);
            assert_eq!(*work, initial_work - rows.get());
            if case == 1 && rows.get() == 8 {
                return Err(rejected("test row emission failure"));
            }
            Ok(())
        },
        |actual, (), work| {
            assert!(!finished.replace(true));
            assert_eq!(rows.get(), 15);
            assert!(std::ptr::eq(actual.owner, expected_owner));
            assert!(std::ptr::eq(actual.expansion.view, expected_view));
            assert_eq!(*work, initial_work.checked_sub(16).unwrap());
            assert_eq!(actual.expansion.source.remaining_work, 0);
            if case == 2 {
                return Err(rejected("test final emission failure"));
            }
            Ok(rows.get())
        },
    );
    use super::super::super::super::ProductionSemanticImportErrorV1 as E;
    match case {
        0 | 4 => {
            assert_eq!(outcome.unwrap(), 15);
            assert!(finished.get());
        }
        1 => {
            assert_eq!(rows.get(), 8);
            assert!(!finished.get());
            assert!(matches!(
                outcome,
                Err(E::KernelContextBinding("test row emission failure"))
            ));
        }
        2 => {
            assert_eq!(rows.get(), 15);
            assert!(finished.get());
            assert!(matches!(
                outcome,
                Err(E::KernelContextBinding("test final emission failure"))
            ));
        }
        3 => {
            assert_eq!(rows.get(), 15);
            assert!(!finished.get());
            assert!(matches!(
                outcome,
                Err(E::KernelContextBinding("phase live source-work ceiling"))
            ));
        }
        _ => panic!("unknown handoff completion case"),
    }
}
