use super::*;

fn recording() -> (Rc<RefCell<Vec<Event>>>, Sink) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    (
        events,
        Rc::new(move |event| output.borrow_mut().push(event)),
    )
}

fn outcomes(events: &[Event]) -> Vec<PhaseOutcome> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Finished { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect()
}

#[test]
fn phase_guard_preserves_success_error_and_original_panic() {
    let (events, sink) = recording();
    with_sink(sink, || {
        let success = || {
            let guard = begin(Route::Direct, Phase::TargetBindAndAdmit);
            guard.complete();
            Ok::<_, u32>(17)
        };
        assert_eq!(success(), Ok(17));
        let refusal = || {
            let _guard = begin(Route::Direct, Phase::Optimizer);
            Err::<(), _>(23)?;
            Ok(())
        };
        assert_eq!(refusal(), Err(23));
        let panic = std::panic::catch_unwind(|| {
            let _guard = begin(Route::Direct, Phase::FinalAdmission);
            std::panic::panic_any(31_u32);
        })
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&31));
    });
    assert_eq!(
        outcomes(&events.borrow()),
        [
            PhaseOutcome::Complete,
            PhaseOutcome::Refused,
            PhaseOutcome::Panicked
        ]
    );
    assert_eq!(events.borrow().len(), 6);
    assert!(begin(Route::Direct, Phase::Optimizer).0.is_none());
}

#[test]
fn nested_and_panicked_scopes_restore_thread_local_observers() {
    let (outer, outer_sink) = recording();
    let (inner, inner_sink) = recording();
    with_sink(outer_sink, || {
        begin(Route::Direct, Phase::Optimizer).complete();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_sink(inner_sink, || {
                let _guard = begin(Route::SilentUnitErased, Phase::RankedSourceReplay);
                panic!("nested compiler failure");
            });
        }));
        assert!(panic.is_err());
        begin(Route::Direct, Phase::FinalAdmission).complete();
        std::thread::spawn(|| {
            assert!(begin(Route::Direct, Phase::Optimizer).0.is_none());
        })
        .join()
        .unwrap();
    });
    assert_eq!(outcomes(&outer.borrow()), [PhaseOutcome::Complete; 2]);
    assert_eq!(outcomes(&inner.borrow()), [PhaseOutcome::Panicked]);
    assert!(begin(Route::Direct, Phase::Optimizer).0.is_none());
}

#[test]
fn observer_panics_are_disabled_without_replacing_result_or_unwind() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let value = with_sink(
        Rc::new(move |_| {
            count.set(count.get() + 1);
            panic!("observer only");
        }),
        || {
            begin(Route::Direct, Phase::Optimizer).complete();
            begin(Route::Direct, Phase::FinalAdmission).complete();
            Err::<(), _>(41)
        },
    );
    assert_eq!(value, Err(41));
    assert_eq!(calls.get(), 1);
    let panic = std::panic::catch_unwind(|| {
        with_sink(
            Rc::new(|event| {
                if matches!(event, Event::Finished { .. }) {
                    panic!("observer during unwind");
                }
            }),
            || {
                let _guard = begin(Route::Direct, Phase::Optimizer);
                std::panic::panic_any(43_u32);
            },
        );
    })
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<u32>(), Some(&43));
}

#[test]
fn panicking_observer_payload_and_capture_drop_are_isolated() {
    struct BadDrop;
    impl Drop for BadDrop {
        fn drop(&mut self) {
            panic!("observer payload drop");
        }
    }
    with_sink(Rc::new(|_| std::panic::panic_any(BadDrop)), || {
        begin(Route::Direct, Phase::Optimizer).complete();
    });
    let capture = BadDrop;
    assert_eq!(
        with_sink(
            Rc::new(move |_| {
                let _ = &capture;
            }),
            || 47
        ),
        47
    );
    assert!(begin(Route::Direct, Phase::Optimizer).0.is_none());
}

#[test]
fn reentrant_observer_is_disabled_without_borrow_panic_or_recursion() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    with_sink(
        Rc::new(move |_| {
            count.set(count.get() + 1);
            begin(Route::Direct, Phase::Optimizer).complete();
        }),
        || {
            begin(Route::Direct, Phase::Optimizer).complete();
            begin(Route::Direct, Phase::FinalAdmission).complete();
        },
    );
    assert_eq!(calls.get(), 1);
}
