use super::*;

#[test]
fn actual_publication_owners_and_barrier_are_send_without_unsafe_impls() {
    fn send<T: Send + 'static>() {}
    send::<crate::CompilerModuleHandoffCurrentnessLeaseV5>();
    send::<crate::CompilerModuleHandoffConsumptionTokenV5>();
    send::<ArtifactLockRetirementBarrierV1>();
}

#[test]
fn retirement_refuses_every_active_spawn_and_retries_after_the_last_drop() {
    let coordinator = isolated_coordinator(0);
    let first = coordinator.try_begin_spawn().unwrap();
    let second = coordinator.try_begin_spawn().unwrap();
    assert_eq!(
        coordinator.try_begin_retirement().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
    assert_eq!(coordinator.state().active_spawns, 2);
    drop(first);
    assert_eq!(
        coordinator.try_begin_retirement().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
    assert_eq!(coordinator.state().active_spawns, 1);
    drop(second);
    let barrier = coordinator.try_begin_retirement().unwrap();
    assert_eq!(
        coordinator.try_begin_spawn().unwrap_err(),
        ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress
    );
    assert_eq!(
        coordinator.try_begin_retirement().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
    assert_eq!(coordinator.state().active_spawns, 0);
    drop(barrier);
    drop(coordinator.try_begin_spawn().unwrap());
}

#[test]
fn retirement_mutex_contention_is_nonblocking_and_does_not_change_obligations() {
    let coordinator = isolated_coordinator(1);
    let state = coordinator.state();
    let (result_tx, result_rx) = mpsc::channel();
    let contender = thread::spawn(move || {
        result_tx.send(coordinator.try_begin_retirement()).unwrap();
    });
    let result = result_rx.recv_timeout(COMPLETION_TIMEOUT);
    assert_eq!(state.active_spawns, 1);
    assert_eq!(state.active_releases, 0);
    assert!(!state.retirement);
    drop(state);
    contender.join().unwrap();
    assert_eq!(
        result.unwrap().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
}

#[test]
fn transferred_barrier_excludes_legacy_spawn_until_drop() {
    let coordinator = isolated_coordinator(0);
    let barrier = coordinator.try_begin_retirement().unwrap();
    assert!(coordinator.state.try_lock().is_ok());
    let (started_tx, started_rx) = mpsc::channel();
    let (spawned_tx, spawned_rx) = mpsc::channel();
    let spawner = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let _spawn = coordinator.begin_spawn();
        spawned_tx.send(()).unwrap();
    });
    started_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    assert_eq!(
        spawned_rx.recv_timeout(BLOCKED_TIMEOUT),
        Err(RecvTimeoutError::Timeout)
    );
    thread::spawn(move || drop(barrier)).join().unwrap();
    spawned_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    spawner.join().unwrap();
    assert_eq!(coordinator.state().active_spawns, 0);
}

#[test]
fn descriptor_destructors_hold_no_coordinator_mutex_and_unwind_clears_only_their_count() {
    let coordinator = isolated_coordinator(0);
    for explicit in [false, true] {
        let barrier = explicit.then(|| coordinator.try_begin_retirement().unwrap());
        let result = catch_unwind(|| {
            coordinator.release_lock_descriptors(|| {
                let state = coordinator
                    .state
                    .try_lock()
                    .expect("destructor holds mutex");
                assert_eq!(state.active_spawns, 0);
                assert_eq!(state.active_releases, 1);
                assert_eq!(state.retirement, explicit);
                drop(state);
                panic!("modeled descriptor destructor unwind");
            });
        });
        assert!(result.is_err());
        assert!(!coordinator.state.is_poisoned());
        assert_eq!(coordinator.state().active_releases, 0);
        assert_eq!(coordinator.state().retirement, explicit);
        drop(barrier);
        drop(coordinator.try_begin_spawn().unwrap());
    }
}

#[test]
fn dropping_barrier_during_descriptor_release_still_excludes_spawn() {
    let coordinator = isolated_coordinator(0);
    let barrier = coordinator.try_begin_retirement().unwrap();
    let (closing_tx, closing_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let closer = thread::spawn(move || {
        coordinator.release_lock_descriptors(|| {
            closing_tx.send(()).unwrap();
            finish_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
        });
    });
    closing_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    drop(barrier);
    assert_eq!(
        coordinator.try_begin_retirement().unwrap_err(),
        ArtifactLockRetirementBarrierErrorV1::Busy
    );
    let (spawned_tx, spawned_rx) = mpsc::channel();
    let spawner = thread::spawn(move || {
        let _spawn = coordinator.try_begin_spawn().unwrap();
        spawned_tx.send(()).unwrap();
    });
    assert_eq!(
        spawned_rx.recv_timeout(BLOCKED_TIMEOUT),
        Err(RecvTimeoutError::Timeout)
    );
    finish_tx.send(()).unwrap();
    closer.join().unwrap();
    spawned_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    spawner.join().unwrap();
    drop(coordinator.try_begin_retirement().unwrap());
}

#[test]
fn barrier_error_and_unwind_release_exclusion_without_poisoning() {
    let coordinator = isolated_coordinator(0);
    let refusal = || -> Result<(), &'static str> {
        let _barrier = coordinator.try_begin_retirement().unwrap();
        Err("refused")
    };
    assert_eq!(refusal(), Err("refused"));
    assert!(!coordinator.state().retirement);
    assert!(
        catch_unwind(|| {
            let _barrier = coordinator.try_begin_retirement().unwrap();
            panic!("modeled pre-retirement unwind");
        })
        .is_err()
    );
    assert!(!coordinator.state.is_poisoned());
    assert!(!coordinator.state().retirement);
    drop(coordinator.try_begin_spawn().unwrap());
}

#[test]
fn foreign_barrier_drop_skips_inherited_mutex_and_state_reset_clears_exclusion() {
    let coordinator = isolated_coordinator(0);
    let current_pid = process::id();
    let origin_pid = current_pid.wrapping_add(1);
    let inherited = ArtifactLockRetirementBarrierV1 {
        coordinator,
        origin_pid,
    };
    let mut state = coordinator.state();
    state.pid = origin_pid;
    state.retirement = true;
    state.active_spawns = 2;
    state.active_releases = 3;
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let dropper = thread::spawn(move || {
        drop(inherited);
        dropped_tx.send(()).unwrap();
    });
    let result = dropped_rx.recv_timeout(COMPLETION_TIMEOUT);
    assert_eq!(state.pid, origin_pid);
    assert!(state.retirement);
    assert_eq!(state.active_spawns, 2);
    assert_eq!(state.active_releases, 3);
    drop(state);
    dropper.join().unwrap();
    assert_eq!(result, Ok(()), "foreign Drop tried to take the mutex");
    let barrier = coordinator.try_begin_retirement().unwrap();
    assert_eq!(barrier.origin_pid, current_pid);
    assert_eq!(coordinator.state().active_spawns, 0);
    assert_eq!(coordinator.state().active_releases, 0);
    drop(barrier);
}

#[cfg(target_os = "linux")]
#[path = "process_spawn_retirement_actual_tests.rs"]
mod actual;
