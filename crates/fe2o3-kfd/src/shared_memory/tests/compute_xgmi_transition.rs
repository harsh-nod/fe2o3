//! Actual borrowed mapping driver with fault-injected native leaves.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn fixture(public: bool) -> (SharedMemoryEngine<FakeBackend>, ComputeXgmiBufferV1) {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            4095,
            4096,
            if public {
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
            } else {
                KfdAllocMemoryFlags::DEVICE_LOCAL
            },
        )
        .unwrap();
    let local = engine.map_device_memory(lease).unwrap();
    (engine, ComputeXgmiBufferV1::new(local, Box::from([7, 9])))
}

fn run(
    engine: &mut SharedMemoryEngine<FakeBackend>,
    root: &mut ComputeXgmiBufferV1,
    stage: usize,
) -> Result<(), MemorySessionError> {
    crate::shared_memory::compute_xgmi_transition::with_compute_transition(engine, |engine| {
        match stage {
            0 => root.local_unmap(engine),
            1 => root.peer_map(engine),
            2 => root.peer_unmap(engine),
            3 => root.local_map(engine),
            _ => unreachable!(),
        }
    })
}

fn owners(root: &ComputeXgmiBufferV1) -> usize {
    usize::from(root.local.is_some())
        + usize::from(root.unmapped.is_some())
        + usize::from(root.peer.is_some())
}

#[test]
fn compute_xgmi_borrowed_roundtrip_preserves_exact_identity_and_requested_extent() {
    let (mut engine, mut root) = fixture(true);
    let identity = root.local.as_ref().unwrap().storage_identity();
    for stage in 0..4 {
        run(&mut engine, &mut root, stage).unwrap();
        assert_eq!(owners(&root), 1);
        assert!(root.progress[stage].attempted);
        assert_eq!(root.progress[stage].returned_success, Some(true));
    }
    let local = root.take_local().unwrap();
    assert_eq!(local.storage_identity(), identity);
    assert_eq!(local.layout().requested_bytes(), 4095);
    assert_eq!(engine.backend.multi_map_inputs, [(vec![7, 9], 0)]);
    assert_eq!(engine.backend.multi_unmap_inputs, [(vec![7, 9], 0)]);
    let lease = engine.unmap_device_memory(local).unwrap();
    engine.release_device_memory(lease).unwrap();
}

#[test]
fn compute_xgmi_borrowed_private_and_bad_rosters_reject_before_native_mapping() {
    for (public, roster) in [
        (false, vec![7, 9]),
        (true, vec![7]),
        (true, vec![9, 7]),
        (true, vec![8, 9]),
    ] {
        let (mut engine, mut root) = fixture(public);
        run(&mut engine, &mut root, 0).unwrap();
        root.roster = Some(roster.into_boxed_slice());
        let calls = engine.backend.currentness_calls;
        assert!(run(&mut engine, &mut root, 1).is_err());
        assert_eq!(engine.backend.currentness_calls, calls);
        assert!(engine.backend.multi_map_inputs.is_empty());
        assert_eq!(owners(&root), 1);
        assert!(root.unmapped.is_some());
        assert!(!root.progress[1].attempted);
    }
}

#[test]
fn compute_xgmi_borrowed_exact_pair_rejects_foreign_third_gpu_without_effects() {
    for mapped in [false, true] {
        let (mut engine, mut root) = fixture(true);
        if mapped {
            run(&mut engine, &mut root, 0).unwrap();
            run(&mut engine, &mut root, 1).unwrap();
        }
        let calls = engine.backend.currentness_calls;
        assert!(root.require_exact_roster([7, 9], !mapped).is_ok());
        for expected in [[7, 11], [9, 7], [9, 11], [7, 7]] {
            assert!(root.require_exact_roster(expected, !mapped).is_err());
        }
        assert_eq!(engine.backend.currentness_calls, calls);
        assert_eq!(owners(&root), 1);
        assert_eq!(
            root.peer
                .as_ref()
                .is_some_and(|peer| peer.is_fully_mapped()),
            mapped
        );
    }
}

#[test]
fn compute_xgmi_borrowed_mapping_rejects_foreign_allocation_generation_device_and_vm() {
    for field in 0..4 {
        let (mut engine, mut root) = fixture(true);
        run(&mut engine, &mut root, 0).unwrap();
        let lease = root.unmapped.as_mut().unwrap();
        match field {
            0 => lease.id += 1,
            1 => lease.generation += 1,
            2 => lease.device.generation.0 += 1,
            3 => lease.vm.id.0 += 1,
            _ => unreachable!(),
        }
        let calls = engine.backend.currentness_calls;
        assert!(run(&mut engine, &mut root, 1).is_err());
        assert_eq!(engine.backend.currentness_calls, calls);
        assert!(engine.backend.multi_map_inputs.is_empty());
        assert!(!root.progress[1].attempted);
        assert_eq!(owners(&root), 1);
    }
}

#[test]
fn compute_xgmi_borrowed_peer_prefix_errno_and_malformed_results_never_retry() {
    for stage in [1, 2] {
        for prefix in 0..=3 {
            for errno in [false, true] {
                if prefix == 2 && !errno {
                    continue;
                }
                let (mut engine, mut root) = fixture(true);
                for prior in 0..stage {
                    run(&mut engine, &mut root, prior).unwrap();
                }
                if stage == 1 {
                    engine.backend.multi_map_script = vec![(prefix, errno)];
                } else {
                    engine.backend.multi_unmap_script = vec![(prefix, errno)];
                }
                assert!(run(&mut engine, &mut root, stage).is_err());
                assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
                assert_eq!(owners(&root), 1);
                let mapping = root.peer.as_ref().unwrap();
                assert!(!mapping.is_fully_mapped());
                assert_eq!(root.progress[stage].returned_map_prefix, Some(prefix));
                assert_eq!(root.progress[stage].returned_success, Some(!errno));
                let calls = if stage == 1 {
                    &engine.backend.multi_map_inputs
                } else {
                    &engine.backend.multi_unmap_inputs
                };
                assert_eq!(calls, &[(vec![7, 9], 0)]);
                assert_eq!(engine.backend.free_calls, 0);
                let native_calls = (
                    engine.backend.multi_map_inputs.len(),
                    engine.backend.multi_unmap_inputs.len(),
                );
                assert!(run(&mut engine, &mut root, stage).is_err());
                assert_eq!(
                    native_calls,
                    (
                        engine.backend.multi_map_inputs.len(),
                        engine.backend.multi_unmap_inputs.len()
                    )
                );
                assert_eq!(owners(&root), 1);
            }
        }
    }
}

#[test]
fn compute_xgmi_borrowed_native_unwind_retains_token_and_attempt_marker() {
    for stage in 0..4 {
        let (mut engine, mut root) = fixture(true);
        for prior in 0..stage {
            run(&mut engine, &mut root, prior).unwrap();
        }
        match stage {
            0 => engine.backend.panic_operation = Some("unmap_gpu"),
            1 => engine.backend.panic_multi_map_at = Some(1),
            2 => engine.backend.panic_multi_unmap_at = Some(1),
            3 => engine.backend.panic_operation = Some("map_gpu"),
            _ => unreachable!(),
        }
        assert!(catch_unwind(AssertUnwindSafe(|| run(&mut engine, &mut root, stage))).is_err());
        assert_eq!(owners(&root), 1);
        assert!(root.progress[stage].attempted);
        assert_eq!(root.progress[stage].returned_success, None);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, 0);
    }
}

#[test]
fn compute_xgmi_borrowed_currentness_fault_matrix_keeps_completed_unmap_authority() {
    for stage in 0..4 {
        for closing in [false, true] {
            for panic in [false, true] {
                let (mut engine, mut root) = fixture(true);
                for prior in 0..stage {
                    run(&mut engine, &mut root, prior).unwrap();
                }
                let at = engine.backend.currentness_calls + if closing { 2 } else { 1 };
                if panic {
                    engine.backend.panic_currentness_at = Some(at);
                } else {
                    engine.backend.fail_currentness_at = Some(at);
                }
                let result = catch_unwind(AssertUnwindSafe(|| run(&mut engine, &mut root, stage)));
                assert!(if panic {
                    result.is_err()
                } else {
                    result.unwrap().is_err()
                });
                assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
                assert_eq!(owners(&root), 1);
                assert_eq!(root.progress[stage].attempted, closing);
                assert_eq!(
                    root.progress[stage].returned_success,
                    closing.then_some(true)
                );
                if closing && stage == 2 {
                    assert!(root.peer.is_none());
                    assert!(root.unmapped.is_some());
                    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
                }
                assert_eq!(engine.backend.free_calls, 0);
            }
        }
    }
}
