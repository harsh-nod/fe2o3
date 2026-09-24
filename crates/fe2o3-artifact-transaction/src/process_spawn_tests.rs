use super::*;
use std::panic::catch_unwind;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);
const BLOCKED_TIMEOUT: Duration = Duration::from_millis(50);

fn isolated_coordinator(active_spawns: u64) -> &'static ArtifactProcessSpawnCoordinatorV1 {
    Box::leak(Box::new(ArtifactProcessSpawnCoordinatorV1 {
        state: Mutex::new(ArtifactProcessSpawnStateV1 {
            pid: process::id(),
            active_spawns,
        }),
        idle: Condvar::new(),
    }))
}

fn start_release(
    coordinator: &'static ArtifactProcessSpawnCoordinatorV1,
) -> (JoinHandle<()>, Receiver<()>) {
    let (started_tx, started_rx) = mpsc::channel();
    let (released_tx, released_rx) = mpsc::channel();
    let release = thread::spawn(move || {
        started_tx.send(()).unwrap();
        coordinator.release_lock_descriptors(|| released_tx.send(()).unwrap());
    });
    started_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    (release, released_rx)
}

#[test]
fn public_lease_and_legacy_wrapper_share_the_global_coordinator() {
    let lease = crate::try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let coordinator = ArtifactProcessSpawnCoordinatorV1::global();
    assert!(std::ptr::eq(lease.coordinator, coordinator));
    assert_eq!(lease.origin_pid, process::id());
    let result = crate::with_artifact_process_spawn_v1(|| {
        assert!(coordinator.state().active_spawns >= 2);
        Err::<(), _>("unchanged closure error")
    });
    assert_eq!(result, Err("unchanged closure error"));
}

#[test]
fn transferred_lease_blocks_release_until_its_custodian_drops_it() {
    let coordinator = isolated_coordinator(0);
    let lease = coordinator.try_begin_spawn().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (dispose_tx, dispose_rx) = mpsc::channel();
    let custodian = thread::spawn(move || {
        ready_tx.send(()).unwrap();
        dispose_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
        drop(lease);
    });
    ready_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    assert_eq!(coordinator.state().active_spawns, 1);
    let (release, released_rx) = start_release(coordinator);
    assert_eq!(
        released_rx.recv_timeout(BLOCKED_TIMEOUT),
        Err(RecvTimeoutError::Timeout)
    );
    dispose_tx.send(()).unwrap();
    custodian.join().unwrap();
    released_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    release.join().unwrap();
    assert_eq!(coordinator.state().active_spawns, 0);
}

#[test]
fn multiple_leases_each_retain_one_count_until_the_last_drop() {
    let coordinator = isolated_coordinator(0);
    let first = coordinator.try_begin_spawn().unwrap();
    let second = coordinator.try_begin_spawn().unwrap();
    assert_eq!(coordinator.state().active_spawns, 2);
    let (release, released_rx) = start_release(coordinator);
    assert_eq!(
        released_rx.recv_timeout(BLOCKED_TIMEOUT),
        Err(RecvTimeoutError::Timeout)
    );
    drop(first);
    assert_eq!(coordinator.state().active_spawns, 1);
    assert_eq!(
        released_rx.recv_timeout(BLOCKED_TIMEOUT),
        Err(RecvTimeoutError::Timeout)
    );
    drop(second);
    released_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    release.join().unwrap();
    assert_eq!(coordinator.state().active_spawns, 0);
}

#[test]
fn unwind_releases_only_the_lease_still_owned_by_the_unwinding_scope() {
    let coordinator = isolated_coordinator(0);
    let retained = coordinator.try_begin_spawn().unwrap();
    let result = catch_unwind(|| {
        let _unwinding = coordinator.try_begin_spawn().unwrap();
        assert_eq!(coordinator.state().active_spawns, 2);
        panic!("modeled failure before creating a child");
    });
    assert!(result.is_err());
    assert_eq!(coordinator.state().active_spawns, 1);
    drop(retained);
    assert_eq!(coordinator.state().active_spawns, 0);
    let (release, released_rx) = start_release(coordinator);
    released_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    release.join().unwrap();
}

#[test]
fn transferred_custody_survives_the_senders_unwind() {
    let coordinator = isolated_coordinator(0);
    let (custody_tx, custody_rx) = mpsc::channel();
    let result = catch_unwind(move || {
        let lease = coordinator.try_begin_spawn().unwrap();
        custody_tx.send(lease).unwrap();
        panic!("modeled failure after transferring cleanup custody");
    });
    assert!(result.is_err());
    let retained = custody_rx.recv_timeout(COMPLETION_TIMEOUT).unwrap();
    assert_eq!(coordinator.state().active_spawns, 1);
    drop(retained);
    assert_eq!(coordinator.state().active_spawns, 0);
}

#[test]
fn count_overflow_refuses_without_mutation_or_poisoning() {
    let coordinator = isolated_coordinator(u64::MAX - 1);
    let last = coordinator.try_begin_spawn().unwrap();
    assert_eq!(coordinator.state().active_spawns, u64::MAX);
    assert_eq!(
        coordinator.try_begin_spawn().unwrap_err(),
        ArtifactProcessSpawnLeaseErrorV1::CountOverflow
    );
    assert!(!coordinator.state.is_poisoned());
    assert_eq!(coordinator.state().active_spawns, u64::MAX);
    drop(last);
    assert_eq!(coordinator.state().active_spawns, u64::MAX - 1);
    let replacement = coordinator.try_begin_spawn().unwrap();
    drop(replacement);
    assert_eq!(coordinator.state().active_spawns, u64::MAX - 1);
}

#[test]
fn foreign_origin_drop_skips_the_inherited_mutex_and_preserves_counts() {
    let current_pid = process::id();
    let origin_pid = current_pid.wrapping_add(1);
    // Model both the untouched inherited state and state already reset for the child.
    for (state_pid, active_spawns) in [(origin_pid, 2), (current_pid, 1), (current_pid, 0)] {
        let coordinator = isolated_coordinator(active_spawns);
        let inherited = ArtifactProcessSpawnLeaseV1 {
            coordinator,
            origin_pid,
        };
        let mut state = coordinator.state.lock().unwrap();
        state.pid = state_pid;
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let dropper = thread::spawn(move || {
            drop(inherited);
            dropped_tx.send(()).unwrap();
        });
        let dropped = dropped_rx.recv_timeout(COMPLETION_TIMEOUT);
        let preserved = (state.pid, state.active_spawns);
        drop(state);
        dropper.join().unwrap();
        assert_eq!(dropped, Ok(()), "foreign Drop tried to take the mutex");
        assert_eq!(preserved, (state_pid, active_spawns));
    }
}
