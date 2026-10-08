use super::*;
use std::cell::Cell;

struct Disposal<'a> {
    dropped: &'a Cell<usize>,
    panic: bool,
}
impl Drop for Disposal<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
        assert!(!self.panic, "scripted disposal");
    }
}

#[test]
fn copy_pending_retains_carrier_and_refuses_ordinary_progress_or_cancellation() {
    let dropped = Cell::new(0);
    let mut slot = Lifecycle::<_, ()>::new(Disposal {
        dropped: &dropped,
        panic: false,
    });
    slot.phase = Phase::RetainedProducer;
    assert!(
        !slot
            .advance_copy(|_| Ok::<_, ()>(false), |_| panic!("pending decoder"))
            .unwrap()
    );
    assert_eq!(slot.phase, Phase::Copying);
    assert!(slot.unsettled() && slot.value.is_some() && slot.outcome.is_none());
    assert!(
        !slot
            .advance::<()>(
                |_, _| panic!("ordinary completion"),
                |_| panic!("ordinary decoder")
            )
            .unwrap()
    );
    assert!(
        !slot
            .cancel_before_adoption(|| -> Result<(), ()> { panic!("late release") })
            .unwrap()
    );
    assert!(
        !slot
            .cancel_unpublished(
                &mut (),
                |_| -> Result<(), ()> { panic!("late retirement") },
                |_| panic!("late hold release")
            )
            .unwrap()
    );
    assert_eq!(dropped.get(), 0);
    assert!(slot.advance_copy(|_| Ok::<_, ()>(true), drop).unwrap());
    assert_eq!(slot.phase, Phase::Settled);
    assert_eq!(dropped.get(), 1);
    assert!(
        !slot
            .advance_copy::<()>(|_| panic!("duplicate copy"), |_| panic!("duplicate decode"))
            .unwrap()
    );
}

#[test]
fn copy_failure_and_unwind_remain_unknown_without_decoding_or_retry() {
    for unwind in [false, true] {
        let dropped = Cell::new(0);
        let mut slot = Lifecycle::<_, ()>::new(Disposal {
            dropped: &dropped,
            panic: false,
        });
        slot.phase = Phase::Copying;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            slot.advance_copy(
                |_| {
                    assert!(!unwind, "scripted copy ambiguity");
                    Err::<bool, ()>(())
                },
                |_| panic!("ambiguous decoder"),
            )
        }));
        assert!(if unwind {
            result.is_err()
        } else {
            matches!(result, Ok(Err(())))
        });
        assert_eq!(slot.phase, Phase::Unknown);
        assert!(slot.unsettled() && slot.value.is_some() && slot.outcome.is_none());
        assert_eq!(dropped.get(), 0);
        assert!(
            !slot
                .advance_copy::<()>(|_| panic!("retry"), |_| panic!("decode"))
                .unwrap()
        );
        std::mem::forget(slot);
    }
}
