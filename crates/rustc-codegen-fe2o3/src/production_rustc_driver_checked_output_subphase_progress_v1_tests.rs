use super::*;
use subphases::{Event, Phase, PhaseOutcome, Route};

#[test]
fn callback_progress_remains_send_and_installs_on_the_callback_thread() {
    fn requires_send<T: Send>() {}
    requires_send::<CallbackProgress>();
    let progress = CallbackProgress::default();
    let progress = std::thread::spawn(move || {
        let mut progress = progress;
        progress
            .run(SourceStage::Policy4, || {
                subphases::begin(Route::Direct, Phase::TargetBindAndAdmit).complete();
                Ok::<_, ()>(())
            })
            .unwrap();
        progress
    })
    .join()
    .unwrap();
    assert_eq!(
        progress.state().snapshot.phases[0]
            .policy4
            .as_ref()
            .unwrap()
            .phases
            .len(),
        1
    );
}

#[test]
fn poisoned_progress_state_is_recovered_without_replacing_compiler_error() {
    let mut progress = CallbackProgress::default();
    let state = progress.state.clone();
    let poison = std::panic::catch_unwind(move || {
        let _guard = state.lock().unwrap();
        panic!("diagnostic state poison");
    });
    assert!(poison.is_err());
    assert_eq!(
        progress.run(SourceStage::Policy4, || {
            let _guard = subphases::begin(Route::Direct, Phase::TargetBindAndAdmit);
            Err::<(), _>(41)
        }),
        Err(41)
    );
    let state = progress.state();
    assert_eq!(state.snapshot.phases[0].outcome, Outcome::Refused);
    assert_eq!(
        state.snapshot.phases[0].policy4.as_ref().unwrap().phases[0].outcome,
        PhaseOutcome::Refused
    );
}

#[test]
fn observer_never_waits_for_a_progress_state_lock_held_by_the_action() {
    let mut progress = CallbackProgress::default();
    let state = progress.state.clone();
    assert_eq!(
        progress.run(SourceStage::Policy4, || {
            let held = state.lock().unwrap();
            let guard = subphases::begin(Route::Direct, Phase::TargetBindAndAdmit);
            drop(held);
            guard.complete();
            Ok::<_, ()>(43)
        }),
        Ok(43)
    );
    let state = progress.state();
    let child = state.snapshot.phases[0].policy4.as_ref().unwrap();
    assert!(child.phases.is_empty());
    assert!(child.unavailable.is_some());
}

#[test]
fn subphase_progress_is_live_and_keeps_actual_route_order() {
    for route in [Route::Direct, Route::SilentUnitErased] {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-subphase-live");
        let path = scratch.path().join("progress.json");
        let mut progress = CallbackProgress::new(Some(path.clone()));
        assert_eq!(
            progress.run(SourceStage::Policy4, || {
                for (ordinal, phase) in route.phases().into_iter().enumerate() {
                    let guard = subphases::begin(route, phase);
                    let snapshot: Snapshot =
                        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                    let parent = snapshot.active.unwrap();
                    assert_eq!(parent.stage, SourceStage::Policy4);
                    let child = parent.policy4.unwrap();
                    assert_eq!(child.route, route);
                    assert_eq!(child.phases.len(), ordinal);
                    assert_eq!(child.active.unwrap().phase, phase);
                    assert!(child.unavailable.is_none());
                    guard.complete();
                }
                Ok::<_, u32>(17)
            }),
            Ok(17)
        );
        progress.finish(Outcome::Complete);
        let snapshot: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(snapshot.active.is_none());
        let parent = &snapshot.phases[0];
        let detail = parent.policy4.as_ref().unwrap();
        assert!(detail.active.is_none());
        assert_eq!(detail.phases.len(), 5);
        assert_eq!(
            detail
                .phases
                .iter()
                .map(|row| row.phase)
                .collect::<Vec<_>>(),
            route.phases()
        );
        assert!(detail.phases.iter().all(|row| {
            row.outcome == PhaseOutcome::Complete && row.elapsed_millis <= parent.elapsed_millis
        }));
        assert!(!path.with_extension("json.tmp").exists());
    }
}

#[test]
fn subphase_progress_preserves_exact_refusal_panic_and_parent_history() {
    let mut progress = CallbackProgress::default();
    assert_eq!(
        progress.run(SourceStage::Policy4, || {
            let _guard = subphases::begin(Route::Direct, Phase::TargetBindAndAdmit);
            Err::<(), _>(23)
        }),
        Err(23)
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        progress.run::<(), ()>(SourceStage::Policy4, || {
            let _guard = subphases::begin(Route::SilentUnitErased, Phase::RankedSourceReplay);
            std::panic::panic_any(29_u32);
        })
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<u32>(), Some(&29));
    let state = progress.state();
    assert!(state.snapshot.active.is_none());
    assert_eq!(state.snapshot.phases.len(), 2);
    for (parent, route, expected) in [
        (
            &state.snapshot.phases[0],
            Route::Direct,
            PhaseOutcome::Refused,
        ),
        (
            &state.snapshot.phases[1],
            Route::SilentUnitErased,
            PhaseOutcome::Panicked,
        ),
    ] {
        let child = parent.policy4.as_ref().unwrap();
        assert_eq!(child.route, route);
        assert_eq!(child.phases.len(), 1);
        assert_eq!(child.phases[0].outcome, expected);
        assert!(child.active.is_none());
    }
    assert_eq!(state.snapshot.phases[0].outcome, Outcome::Refused);
    assert_eq!(state.snapshot.phases[1].outcome, Outcome::Panicked);
}

#[test]
fn historical_v1_progress_json_has_no_invented_policy4_observation() {
    let json = br#"{
        "schema":"fe2o3-test-callback-progress-v1",
        "elapsed_millis":19,"outcome":null,
        "active":{"stage":"policy4","started_millis":7},
        "phases":[{"stage":"ranked-checks","outcome":"complete","elapsed_millis":3}]
    }"#;
    let snapshot: Snapshot = serde_json::from_slice(json).unwrap();
    assert!(snapshot.active.as_ref().unwrap().policy4.is_none());
    assert!(snapshot.phases[0].policy4.is_none());
    let serialized = serde_json::to_value(&snapshot).unwrap();
    assert!(serialized["active"].get("policy4").is_none());
    assert!(serialized["phases"][0].get("policy4").is_none());
}

#[test]
fn wrong_or_excessive_events_disable_only_bounded_diagnostics() {
    let mut progress = CallbackProgress::default();
    progress.begin(SourceStage::Policy4);
    {
        let mut state = progress.state();
        for phase in Route::Direct.phases() {
            state.policy4_event(Event::Started {
                route: Route::Direct,
                phase,
            });
            state.policy4_event(Event::Finished {
                route: Route::Direct,
                phase,
                outcome: PhaseOutcome::Complete,
                elapsed_millis: 0,
            });
        }
        for _ in 0..100 {
            state.policy4_event(Event::Started {
                route: Route::SilentUnitErased,
                phase: Phase::RankedSourceReplay,
            });
        }
    }
    progress.end(Outcome::Complete);
    let state = progress.state();
    let parent = &state.snapshot.phases[0];
    assert_eq!(parent.outcome, Outcome::Complete);
    let detail = parent.policy4.as_ref().unwrap();
    assert_eq!(detail.phases.len(), 5);
    assert!(detail.unavailable.is_some());
}

#[test]
fn missing_parent_or_non_policy4_phase_receives_no_nested_observation() {
    let mut progress = CallbackProgress::default();
    progress.state().policy4_event(Event::Started {
        route: Route::Direct,
        phase: Phase::Optimizer,
    });
    progress
        .run(SourceStage::RankedChecks, || {
            subphases::begin(Route::Direct, Phase::Optimizer).complete();
            Ok::<_, ()>(())
        })
        .unwrap();
    let state = progress.state();
    assert_eq!(state.snapshot.phases.len(), 1);
    assert!(state.snapshot.phases[0].policy4.is_none());
}

#[test]
fn subphase_failed_or_stale_writes_do_not_change_compiler_result() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-subphase-io");
    for path in [
        scratch.path().join("absent/progress.json"),
        scratch.path().join("progress.json"),
    ] {
        if path.parent() == Some(scratch.path()) {
            std::fs::write(path.with_extension("json.tmp"), b"owned by a prior writer").unwrap();
        }
        let mut progress = CallbackProgress::new(Some(path.clone()));
        assert_eq!(
            progress.run(SourceStage::Policy4, || {
                let _guard = subphases::begin(Route::Direct, Phase::TargetBindAndAdmit);
                Err::<(), _>(37)
            }),
            Err(37)
        );
        let state = progress.state();
        assert_eq!(state.snapshot.phases[0].outcome, Outcome::Refused);
        assert_eq!(
            state.snapshot.phases[0]
                .policy4
                .as_ref()
                .unwrap()
                .phases
                .len(),
            1
        );
        if path.parent() == Some(scratch.path()) {
            assert_eq!(
                std::fs::read(path.with_extension("json.tmp")).unwrap(),
                b"owned by a prior writer"
            );
        }
    }
}
