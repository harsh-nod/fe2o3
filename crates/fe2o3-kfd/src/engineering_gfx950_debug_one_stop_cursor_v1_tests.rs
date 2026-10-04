use super::*;
use std::cell::Cell;
fn steps() -> Vec<Gfx950DebugOneStopStepV1> {
    let mut cursor = Cursor::new(7);
    let mut rows = Vec::new();
    while cursor.phase() != Gfx950DebugOneStopPhaseV1::LocalBackingRetired {
        cursor
            .run(7, |step| {
                rows.push(step);
                Ok(())
            })
            .unwrap();
    }
    rows
}
#[test]
fn one_stop_cursor_has_exact_separate_publication_completion_and_nine_retirements() {
    let rows = steps();
    assert_eq!(rows.len(), 64);
    assert_eq!(rows[0], Gfx950DebugOneStopStepV1::ValidateFixedArtifact);
    assert_eq!(rows[4], Gfx950DebugOneStopStepV1::OwnedCheckpoint);
    assert_eq!(rows[10], Gfx950DebugOneStopStepV1::ObserveCompletion);
    assert_eq!(rows[12], Gfx950DebugOneStopStepV1::DestroyQueue);
    for ordinal in 0..9_u8 {
        let retire: Vec<_> = rows.iter().filter(|step| matches!(step,Gfx950DebugOneStopStepV1::RetireAllocation{ordinal:o,..} if *o==ordinal)).collect();
        assert_eq!(retire.len(), 5);
    }
}
#[test]
fn every_failed_step_is_sticky_and_never_invokes_next_operation() {
    let rows = steps();
    for (failed, step) in rows.iter().enumerate() {
        let mut c = Cursor::new(7);
        for _ in 0..failed {
            c.run(7, |_| Ok(())).unwrap();
        }
        let effect_before = c.publication_possible();
        assert!(c.run(7, |_| Err(E::Contract("injected"))).is_err());
        assert_eq!(c.phase(), Gfx950DebugOneStopPhaseV1::Attempting(*step));
        assert_eq!(c.publication_possible(), effect_before || failed >= 5);
        let ran = Cell::new(false);
        assert!(
            c.run(7, |_| {
                ran.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!ran.get());
    }
}
#[test]
fn every_panicked_step_retains_attempting_and_effect_possibility() {
    let rows = steps();
    for (failed, step) in rows.iter().enumerate() {
        let mut c = Cursor::new(7);
        for _ in 0..failed {
            c.run(7, |_| Ok(())).unwrap();
        }
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = c.run(7, |_| panic!("injected"));
        }));
        assert!(panic.is_err());
        assert_eq!(c.phase(), Gfx950DebugOneStopPhaseV1::Attempting(*step));
        assert_eq!(c.publication_possible(), failed >= 5);
        assert!(c.run(7, |_| Ok(())).is_err());
    }
}
#[test]
fn every_process_mismatch_precedes_operation_and_does_not_reset() {
    for prefix in 0..64 {
        let mut c = Cursor::new(7);
        for _ in 0..prefix {
            c.run(7, |_| Ok(())).unwrap();
        }
        let old = c.phase();
        assert!(matches!(
            c.run(8, |_| panic!("must not run")),
            Err(E::ProcessChanged)
        ));
        assert_eq!(c.phase(), old);
    }
    let mut zero = Cursor::new(0);
    assert!(matches!(
        zero.run(0, |_| panic!("must not run")),
        Err(E::ProcessChanged)
    ));
}
#[test]
fn no_step_after_terminal_and_publication_possible_is_not_cleared() {
    let mut c = Cursor::new(7);
    for _ in 0..64 {
        c.run(7, |_| Ok(())).unwrap();
    }
    assert!(c.publication_possible());
    assert!(c.run(7, |_| panic!("terminal must not run")).is_err());
}
