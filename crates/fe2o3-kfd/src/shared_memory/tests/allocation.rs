//! Pending native outputs use the real allocation sequencer with a fake backend.

use super::*;
use crate::shared_memory::allocation::PendingAllocationStageV1 as Stage;

fn fixture() -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_host_visible_backing_budget_v1(
            device,
            vm,
            Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 256).unwrap(),
        )
        .unwrap();
    engine
        .configure_device_backing_budget_v1(
            device,
            vm,
            Gfx942DeviceBackingBudgetV1::new(1 << 20, 16).unwrap(),
        )
        .unwrap();
    engine
}

fn calls(engine: &SharedMemoryEngine<FakeBackend>) -> [usize; 9] {
    let b = &engine.backend;
    [
        b.currentness_calls,
        b.operational_currentness_calls,
        b.reserve_va_calls,
        b.alloc_calls,
        b.map_cpu_calls,
        b.map_gpu_calls,
        b.unmap_gpu_calls,
        b.free_calls,
        b.release_va_calls,
    ]
}

fn assert_closed(engine: &mut SharedMemoryEngine<FakeBackend>) {
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let before = calls(engine);
    let usage = engine.host_backing_account.as_ref().map(|a| a.usage());
    assert!(matches!(
        engine.allocate::<ExecutableGttV1>(17),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(17),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(engine), before);
    assert_eq!(
        engine.host_backing_account.as_ref().map(|a| a.usage()),
        usage
    );
}

fn assert_pending<P: GttProfileV1>(
    engine: &SharedMemoryEngine<FakeBackend>,
    stage: Stage,
    reservation: bool,
    output: bool,
    mapping: bool,
) {
    let pending = engine
        .pending_allocation
        .as_ref()
        .expect("exact pending owner");
    assert_eq!(pending.id, 1);
    assert_eq!(pending.record_slot, 0);
    assert_eq!(pending.profile, P::PROFILE);
    assert_eq!(pending.layout, profile_layout::<P>(4096).unwrap());
    assert_eq!(pending.stage, stage);
    assert_eq!(pending.reservation.is_some(), reservation);
    assert_eq!(pending.allocation_output.is_some(), output);
    assert_eq!(pending.mapping.is_some(), mapping);
    if reservation {
        assert_eq!(pending.reservation, Some((0x2_0000, 4096)));
    }
    if output {
        assert_eq!(
            pending.allocation_output,
            engine.backend.last_allocation_output
        );
    }
    if let Some(mapping) = &pending.mapping {
        assert_eq!(mapping.address, 0x2_0000);
        assert_eq!(mapping.bytes.len(), 4096);
    }
    assert!(pending.host_backing_charge.is_none());
    assert!(engine.allocations.is_empty());
    assert!(engine.allocation_record_slots.is_empty());
    assert_eq!(engine.retained_gpu_va_bytes, 4096);
    assert_eq!(engine.next_id, 2);
    if let Some(account) = &engine.host_backing_account {
        let usage = account.usage();
        assert_eq!(usage.reserved_records, 0);
        assert_eq!(
            usage.used_backing_bytes,
            if is_host_backing_profile::<P>() {
                4096
            } else {
                0
            }
        );
        assert_eq!(
            usage.quarantined_records,
            usize::from(is_host_backing_profile::<P>())
        );
    }
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

fn native_faults<P: GttProfileV1>() {
    for panic in [false, true] {
        for (operation, stage, reservation, output, mapping) in [
            ("reserve_va", Stage::ReserveVa, false, false, false),
            ("alloc", Stage::Allocate, true, !panic, false),
            ("map_cpu", Stage::MapCpu, true, true, false),
            (
                "prepare_cpu_mapping",
                Stage::PrepareCpuMapping,
                true,
                true,
                true,
            ),
        ] {
            let mut engine = fixture();
            if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.allocate::<P>(4096)
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_pending::<P>(&engine, stage, reservation, output, mapping);
            assert_closed(&mut engine);
        }
    }
}

#[test]
fn pending_allocation_native_faults_retain_every_returned_control_output() {
    native_faults::<ExecutableGttV1>();
    native_faults::<KernargGttV1>();
    native_faults::<HostVisibleCoherentGttV1>();
    native_faults::<AqlQueueGttV1>();
}

fn currentness_faults<P: GttProfileV1>() {
    for panic in [false, true] {
        for delta in 1..=3 {
            let mut engine = fixture();
            let before = calls(&engine);
            if panic {
                engine.backend.panic_currentness_at = Some(before[0] + delta);
            } else {
                engine.backend.fail_currentness_at = Some(before[0] + delta);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.allocate::<P>(4096)
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(engine.backend.currentness_calls, before[0] + delta);
            if delta == 1 {
                assert!(engine.pending_allocation.is_none());
                assert_eq!(engine.next_id, 1);
                assert_eq!(engine.retained_gpu_va_bytes, 0);
                assert_eq!(engine.backend.reserve_va_calls, 0);
                assert_eq!(
                    engine
                        .host_backing_account
                        .as_ref()
                        .unwrap()
                        .usage()
                        .used_backing_bytes,
                    0
                );
                assert_eq!(
                    engine
                        .host_backing_account
                        .as_ref()
                        .unwrap()
                        .usage()
                        .reserved_records,
                    0
                );
            } else {
                assert_pending::<P>(
                    &engine,
                    if delta == 2 {
                        Stage::CheckAllocation
                    } else {
                        Stage::CheckMapping
                    },
                    true,
                    true,
                    delta == 3,
                );
                assert_closed(&mut engine);
            }
        }
    }
}

#[test]
fn pending_allocation_currentness_faults_preserve_original_panic_and_prefix() {
    currentness_faults::<ExecutableGttV1>();
    currentness_faults::<KernargGttV1>();
    currentness_faults::<HostVisibleCoherentGttV1>();
}

#[test]
fn pending_allocation_errno_preserves_nonzero_handle_and_all_raw_fields() {
    let mut engine = fixture();
    engine.backend.alloc_oom = true;
    assert!(engine.allocate::<ExecutableGttV1>(4096).is_err());
    assert_pending::<ExecutableGttV1>(&engine, Stage::Allocate, true, true, false);
    let output = engine
        .pending_allocation
        .as_ref()
        .unwrap()
        .allocation_output
        .unwrap();
    assert_eq!(output.handle, 1);
    assert_eq!(output.mmap_offset, 0x41_000);
    assert_closed(&mut engine);
}

#[test]
fn pending_allocation_malformed_output_is_retained_without_authority() {
    let mutations: [fn(&mut KfdIoctlAllocMemoryOfGpuArgs); 8] = [
        |a| a.va_addr += 4096,
        |a| a.size += 4096,
        |a| a.gpu_id += 1,
        |a| a.flags ^= 1,
        |a| a.handle = 0,
        |a| a.mmap_offset = 0,
        |a| a.mmap_offset += 1,
        |a| {
            a.va_addr = 0;
            a.handle = u64::MAX;
            a.flags = 0;
        },
    ];
    for mutate in mutations {
        let mut engine = fixture();
        engine.backend.allocation_output_mutator = Some(mutate);
        assert!(matches!(
            engine.allocate::<ExecutableGttV1>(4096),
            Err(MemorySessionError::KernelResultMalformed(
                "shared ALLOC_MEMORY_OF_GPU output"
            ))
        ));
        assert_pending::<ExecutableGttV1>(&engine, Stage::ValidateAllocation, true, true, false);
        assert_eq!(engine.backend.map_cpu_calls, 0);
        assert_closed(&mut engine);
    }
}

#[test]
fn pending_allocation_invalid_reservation_and_mapping_keep_original_objects() {
    for address in [0, 17, 0xffff_ffff_ffff_f000] {
        let mut engine = fixture();
        engine.backend.fixed_va = Some(address);
        assert!(engine.allocate::<KernargGttV1>(4096).is_err());
        let pending = engine.pending_allocation.as_ref().unwrap();
        assert_eq!(pending.stage, Stage::ValidateReservation);
        assert_eq!(pending.reservation, Some((address, 4096)));
        assert!(pending.allocation_output.is_none());
        assert_eq!(engine.backend.alloc_calls, 0);
        assert_closed(&mut engine);
    }
    let mut engine = fixture();
    engine.backend.corrupt_mapping_address = true;
    assert!(engine.allocate::<ExecutableGttV1>(4096).is_err());
    let pending = engine.pending_allocation.as_ref().unwrap();
    assert_eq!(pending.stage, Stage::ValidateMapping);
    assert_eq!(pending.reservation, Some((0x2_0000, 4096)));
    let mapping = pending.mapping.as_ref().unwrap();
    assert_eq!(mapping.address, 0x2_1000);
    assert!(mapping.writable);
    assert_eq!(
        pending.allocation_output,
        engine.backend.last_allocation_output
    );
    assert_closed(&mut engine);
}

#[derive(Debug, Eq, PartialEq)]
struct RecordSnapshot {
    id: u64,
    generation: u64,
    profile: SharedGttProfileV1,
    layout: SharedGttAllocationLayoutV1,
    gpu_va: u64,
    mmap_offset: u64,
    handle: Option<u64>,
    phase: SharedAllocationPhaseV1,
    reservation: Option<(u64, usize)>,
    mapping: Option<(u64, usize, usize, bool, bool)>,
    charged: bool,
}

fn records(engine: &SharedMemoryEngine<FakeBackend>) -> Vec<RecordSnapshot> {
    engine
        .allocations
        .iter()
        .map(|r| RecordSnapshot {
            id: r.id,
            generation: r.generation,
            profile: r.profile,
            layout: r.layout,
            gpu_va: r.gpu_va,
            mmap_offset: r.mmap_offset,
            handle: r.handle,
            phase: r.phase,
            reservation: r.reservation,
            mapping: r.mapping.as_ref().map(|m| {
                (
                    m.address,
                    m.bytes.as_ptr() as usize,
                    m.bytes.len(),
                    m.active,
                    m.writable,
                )
            }),
            charged: r.host_backing_charge.is_some(),
        })
        .collect()
}

#[test]
fn pending_allocation_collision_preserves_preexisting_records_and_charges() {
    for fault in 0..3 {
        let mut engine = fixture();
        let _host = engine.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        let before = records(&engine);
        let usage = engine.host_backing_account.as_ref().unwrap().usage();
        match fault {
            0 => engine.backend.fixed_va = Some(before[0].gpu_va),
            1 => engine.backend.allocation_output_mutator = Some(|a| a.handle = 1),
            _ => engine.backend.allocation_output_mutator = Some(|a| a.mmap_offset = 0x41_000),
        }
        assert!(engine.allocate::<ExecutableGttV1>(4096).is_err());
        let pending = engine.pending_allocation.as_ref().unwrap();
        assert_eq!(pending.id, 2);
        assert_eq!(pending.record_slot, 1);
        assert_eq!(
            pending.stage,
            if fault == 0 {
                Stage::ValidateReservation
            } else {
                Stage::ValidateAllocation
            }
        );
        assert_eq!(records(&engine), before);
        assert_eq!(engine.host_backing_account.as_ref().unwrap().usage(), usage);
        assert_eq!(engine.retained_gpu_va_bytes, 8192);
        assert_closed(&mut engine);
    }
}

#[test]
fn pending_allocation_success_moves_exact_resources_and_charge_into_record() {
    fn success<P: GttProfileV1>(bytes: usize) {
        let mut engine = fixture();
        let token = engine.allocate::<P>(bytes).unwrap();
        assert!(engine.pending_allocation.is_none());
        let record = &engine.allocations[0];
        assert_eq!(record.id, token.id);
        assert_eq!(record.profile, P::PROFILE);
        assert_eq!(record.layout, token.layout);
        assert_eq!(record.reservation, Some((0x2_0000, 4096)));
        assert_eq!(record.mapping.as_ref().unwrap().address, 0x2_0000);
        assert_eq!(record.mapping.as_ref().unwrap().bytes.len(), 4096);
        assert!(record.mapping.as_ref().unwrap().writable);
        assert_eq!(
            record.handle,
            engine.backend.last_allocation_output.map(|a| a.handle)
        );
        assert_eq!(
            record.host_backing_charge.is_some(),
            is_host_backing_profile::<P>()
        );
        assert_eq!(engine.allocation_record_slots.get(&token.id), Some(&0));
        assert_eq!(engine.next_id, 2);
        assert_eq!(engine.retained_gpu_va_bytes, 4096);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        assert_eq!(engine.retained_gpu_va_bytes, 0);
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .usage()
                .used_backing_bytes,
            0
        );
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .usage()
                .quarantined_records,
            0
        );
        assert_eq!(engine.backend.free_calls, 1);
        assert!(engine.allocations[0].is_fully_released());
    }
    success::<HostVisibleCoherentGttV1>(17);
    success::<ExecutableGttV1>(17);
    success::<KernargGttV1>(17);
    success::<AqlQueueGttV1>(4096);
    success::<UserptrAqlControlGttV1>(4096);
    success::<UserptrAqlQueueProbeGttV1>(4096);
}

#[test]
fn pending_allocation_pure_rejection_consumes_no_slot_identity_or_native_budget() {
    for fault in 0..4 {
        let mut engine = fixture();
        let bytes = if fault == 0 { 0 } else { 4096 };
        match fault {
            1 => engine.next_id = u64::MAX,
            2 => engine.retained_gpu_va_bytes = MAX_SHARED_GTT_GPU_VA_BYTES_V1,
            3 => engine.retained_gpu_va_bytes = u64::MAX,
            _ => (),
        }
        let before = (calls(&engine), engine.next_id, engine.retained_gpu_va_bytes);
        assert!(engine.allocate::<HostVisibleCoherentGttV1>(bytes).is_err());
        assert_eq!(
            (calls(&engine), engine.next_id, engine.retained_gpu_va_bytes),
            before
        );
        assert!(engine.pending_allocation.is_none());
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .usage()
                .used_backing_bytes,
            0
        );
    }
}

#[test]
fn pending_allocation_capacity_and_released_slot_reuse_remain_bounded() {
    let mut engine = fixture();
    let mut tokens = Vec::new();
    for _ in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
        tokens.push(engine.allocate::<KernargGttV1>(4096).unwrap());
    }
    let before = (
        calls(&engine),
        engine.next_id,
        engine.retained_gpu_va_bytes,
        records(&engine),
    );
    assert!(matches!(
        engine.allocate::<ExecutableGttV1>(4096),
        Err(MemorySessionError::SharedAllocationCapacity { .. })
    ));
    assert_eq!(
        (
            calls(&engine),
            engine.next_id,
            engine.retained_gpu_va_bytes,
            records(&engine)
        ),
        before
    );
    assert!(engine.pending_allocation.is_none());
    let removed = tokens.remove(37);
    engine
        .release(removed, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    let before = records(&engine);
    let token = engine.allocate::<ExecutableGttV1>(4096).unwrap();
    assert_eq!(engine.allocations.len(), MAX_SHARED_GTT_ALLOCATIONS_V1);
    assert_eq!(engine.allocation_record_slots.get(&token.id), Some(&37));
    let after = records(&engine);
    for i in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
        if i != 37 {
            assert_eq!(after[i], before[i]);
        }
    }
    assert!(engine.pending_allocation.is_none());
}

#[test]
fn pending_allocation_userptr_retains_only_returned_mapping_and_raw_output() {
    for panic in [false, true] {
        for operation in ["prepare_userptr", "prepare_cpu_mapping", "alloc_userptr"] {
            let mut engine = fixture();
            if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.allocate::<UserptrAqlControlGttV1>(4096)
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_pending::<UserptrAqlControlGttV1>(
                &engine,
                if operation == "alloc_userptr" {
                    Stage::Allocate
                } else {
                    Stage::PrepareUserptr
                },
                true,
                operation == "alloc_userptr" && !panic,
                operation == "alloc_userptr",
            );
            assert_closed(&mut engine);
        }
    }
}

#[test]
fn pending_allocation_userptr_currentness_retains_returned_mapping_before_alloc() {
    for panic in [false, true] {
        for delta in 1..=4 {
            let mut engine = fixture();
            let before = calls(&engine);
            if panic {
                engine.backend.panic_currentness_at = Some(before[0] + delta);
            } else {
                engine.backend.fail_currentness_at = Some(before[0] + delta);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.allocate::<UserptrAqlControlGttV1>(4096)
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(engine.backend.currentness_calls, before[0] + delta);
            if delta == 1 {
                assert!(engine.pending_allocation.is_none());
                assert_eq!(engine.backend.reserve_va_calls, 0);
                assert_eq!(engine.next_id, 1);
                assert_eq!(engine.retained_gpu_va_bytes, 0);
            } else {
                let stage = match delta {
                    2 => Stage::CheckUserptr,
                    3 => Stage::CheckAllocation,
                    _ => Stage::CheckMapping,
                };
                assert_pending::<UserptrAqlControlGttV1>(&engine, stage, true, delta > 2, true);
                assert!(
                    engine
                        .pending_allocation
                        .as_ref()
                        .unwrap()
                        .mapping
                        .as_ref()
                        .unwrap()
                        .writable
                );
                assert_eq!(engine.backend.alloc_calls, usize::from(delta > 2));
                assert_closed(&mut engine);
            }
        }
    }
}

#[test]
fn pending_allocation_failed_released_slot_reuse_keeps_all_committed_records() {
    for panic in [false, true] {
        let mut engine = fixture();
        let mut tokens = Vec::new();
        for _ in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
            tokens.push(engine.allocate::<KernargGttV1>(4096).unwrap());
        }
        engine
            .release(tokens.remove(37), SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        let before = records(&engine);
        let index = engine.allocation_record_slots.clone();
        let old_va = engine.retained_gpu_va_bytes;
        if panic {
            engine.backend.panic_operation = Some("prepare_cpu_mapping");
        } else {
            engine.backend.fail_operation = Some("prepare_cpu_mapping");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.allocate::<ExecutableGttV1>(4096)
        }));
        if panic {
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "prepare_cpu_mapping"))
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        let pending = engine.pending_allocation.as_ref().unwrap();
        assert_eq!(pending.stage, Stage::PrepareCpuMapping);
        assert_eq!(pending.record_slot, 37);
        assert_eq!(pending.id, MAX_SHARED_GTT_ALLOCATIONS_V1 as u64 + 1);
        assert!(pending.reservation.is_some());
        assert!(pending.mapping.is_some());
        assert_eq!(
            pending.allocation_output,
            engine.backend.last_allocation_output
        );
        assert_eq!(records(&engine), before);
        assert_eq!(engine.allocation_record_slots, index);
        assert!(engine.allocations[37].is_fully_released());
        assert_eq!(engine.retained_gpu_va_bytes, old_va + 4096);
        assert_closed(&mut engine);
    }
}

#[test]
fn pending_allocation_unconfigured_failures_still_quarantine_and_retain() {
    for panic in [false, true] {
        let mut engine = acquired();
        if panic {
            engine.backend.panic_operation = Some("prepare_cpu_mapping");
        } else {
            engine.backend.fail_operation = Some("prepare_cpu_mapping");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.allocate::<HostVisibleCoherentGttV1>(4096)
        }));
        if panic {
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "prepare_cpu_mapping"))
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_pending::<HostVisibleCoherentGttV1>(
            &engine,
            Stage::PrepareCpuMapping,
            true,
            true,
            true,
        );
        assert_closed(&mut engine);
    }
}

#[test]
fn pending_allocation_control_failure_keeps_existing_host_and_device_charges() {
    for panic in [false, true] {
        let mut engine = fixture();
        let _host = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        let (device, vm) = device_vm(1);
        let lease = engine.allocate_device_memory(device, vm, 17, 4).unwrap();
        let _lease = engine.map_device_memory(lease).unwrap();
        let host_before = records(&engine);
        let host_usage = engine.host_backing_account.as_ref().unwrap().usage();
        let device_usage = engine.device_backing_account.as_ref().unwrap().usage();
        let snapshot = |engine: &SharedMemoryEngine<FakeBackend>| {
            assert_eq!(engine.device_memory.len(), 1);
            let r = &engine.device_memory[0];
            (
                r.id,
                r.generation,
                r.device,
                r.vm,
                r.layout,
                r.gpu_va,
                r.handle,
                r.phase,
                r.reservation,
                r.backing_charge.is_some(),
            )
        };
        let device_before = snapshot(&engine);
        if panic {
            engine.backend.panic_operation = Some("prepare_cpu_mapping");
        } else {
            engine.backend.fail_operation = Some("prepare_cpu_mapping");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.allocate::<KernargGttV1>(4096)
        }));
        if panic {
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "prepare_cpu_mapping"))
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(records(&engine), host_before);
        assert_eq!(snapshot(&engine), device_before);
        assert_eq!(
            engine.host_backing_account.as_ref().unwrap().usage(),
            host_usage
        );
        assert_eq!(
            engine.device_backing_account.as_ref().unwrap().usage(),
            device_usage
        );
        let pending = engine.pending_allocation.as_ref().unwrap();
        assert_eq!(pending.id, 2);
        assert_eq!(pending.stage, Stage::PrepareCpuMapping);
        assert!(pending.reservation.is_some());
        assert!(pending.mapping.is_some());
        assert_closed(&mut engine);
    }
}
