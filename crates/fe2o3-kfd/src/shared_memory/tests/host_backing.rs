//! Actual native records and model ownership with a fake backend, not Linux/GPU evidence.
mod borrowed_initialization;

use super::*;
use crate::sdma::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1};

type HostCpu = SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>;
type HostMapped = SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>;

fn budget(bytes: u64, records: usize) -> Gfx942HostVisibleBackingBudgetV1 {
    Gfx942HostVisibleBackingBudgetV1::new(bytes, records).unwrap()
}

fn configured(bytes: u64, records: usize) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_host_visible_backing_budget_v1(device, vm, budget(bytes, records))
        .unwrap();
    engine
}

fn usage(engine: &SharedMemoryEngine<FakeBackend>) -> Gfx942HostVisibleBackingUsageV1 {
    engine.host_backing_account.as_ref().unwrap().usage()
}

fn debit(engine: &SharedMemoryEngine<FakeBackend>, bytes: u64, records: u64) {
    let usage = usage(engine);
    assert_eq!(usage.used_backing_bytes, bytes);
    assert_eq!(usage.used_allocation_records, records);
    assert_eq!(usage.reserved_records, 0);
}

fn calls(engine: &SharedMemoryEngine<FakeBackend>) -> [usize; 9] {
    let backend = &engine.backend;
    [
        backend.currentness_calls,
        backend.operational_currentness_calls,
        backend.reserve_va_calls,
        backend.alloc_calls,
        backend.map_cpu_calls,
        backend.map_gpu_calls,
        backend.unmap_gpu_calls,
        backend.free_calls,
        backend.release_va_calls,
    ]
}

fn closed(engine: &mut SharedMemoryEngine<FakeBackend>) {
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let before = calls(engine);
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    let token = HostCpu {
        session_id: engine.session_id,
        id: 1,
        generation: 1,
        layout: profile_layout::<HostVisibleCoherentGttV1>(1).unwrap(),
        marker: PhantomData,
    };
    assert!(matches!(
        engine.with_bytes(&token, SharedAllocationPhaseV1::CpuWritable, |_| ()),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(engine), before);
}

fn mapped(engine: &mut SharedMemoryEngine<FakeBackend>, bytes: usize) -> HostMapped {
    let token = engine.allocate::<HostVisibleCoherentGttV1>(bytes).unwrap();
    engine.map_mutable(token).unwrap()
}

fn panic_payload(
    payload: &(dyn core::any::Any + Send),
    prefix: &'static str,
    operation: &'static str,
) {
    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&(prefix, operation))
    );
}

#[test]
fn n1_defaults_and_excluded_profiles_keep_original_allocation_behavior() {
    let mut engine = acquired();
    let host = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
    assert!(engine.host_backing_account.is_none());
    assert!(engine.allocations[0].host_backing_charge.is_none());
    engine
        .release(host, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();

    fn excluded<P: GttProfileV1>(engine: &mut SharedMemoryEngine<FakeBackend>) {
        let token = engine.allocate::<P>(4096).unwrap();
        assert!(
            engine
                .allocations
                .last()
                .unwrap()
                .host_backing_charge
                .is_none()
        );
        debit(engine, 0, 0);
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
    }
    let mut engine = configured(4096, 1);
    excluded::<KernargGttV1>(&mut engine);
    excluded::<AqlQueueGttV1>(&mut engine);
    excluded::<ExecutableGttV1>(&mut engine);
    excluded::<ExecutableAqlQueueProbeGttV1>(&mut engine);
    excluded::<UserptrAqlQueueProbeGttV1>(&mut engine);
    excluded::<UserptrAqlControlGttV1>(&mut engine);
    assert!(!engine.host_backing_activity_started);
    let (device, vm) = device_vm(1);
    engine
        .configure_device_backing_budget_v1(
            device,
            vm,
            Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap(),
        )
        .unwrap();
    let device_lease = engine.allocate_device_memory(device, vm, 17, 4).unwrap();
    debit(&engine, 0, 0);
    let host = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
    debit(&engine, 4096, 1);
    assert_eq!(
        engine
            .device_backing_account
            .as_ref()
            .unwrap()
            .usage()
            .used_backing_bytes,
        4096
    );
    engine
        .release(host, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(
        engine
            .device_backing_account
            .as_ref()
            .unwrap()
            .usage()
            .used_backing_bytes,
        4096
    );
    engine.release_device_memory(device_lease).unwrap();
    debit(&engine, 0, 0);
}

#[test]
fn n1_bootstrap_profile_order_and_real_completion_arena_size_obey_both_limits() {
    use crate::queue::completion::COMPLETION_SIGNAL_ARENA_BYTES_V1;

    let arena_bytes = COMPLETION_SIGNAL_ARENA_BYTES_V1 as u64;
    for limits in [
        (arena_bytes - 4096, 2),
        (arena_bytes, 2),
        (arena_bytes + 4096, 1),
    ] {
        let mut engine = configured(limits.0, limits.1);
        // Match the ordinary allocation profile order, not Linux queue creation
        // or completion-signal initialization: those remain hardware/facade gates.
        let ring = engine.allocate::<AqlQueueGttV1>(4096).unwrap();
        let control = engine.allocate::<UserptrAqlControlGttV1>(4096).unwrap();
        debit(&engine, 0, 0);
        let before_arena = calls(&engine);
        let arena = engine.allocate::<HostVisibleCoherentGttV1>(COMPLETION_SIGNAL_ARENA_BYTES_V1);
        if limits.0 < arena_bytes {
            assert!(matches!(
                arena,
                Err(MemorySessionError::HostVisibleBackingCredits(_))
            ));
            assert_eq!(calls(&engine), before_arena);
            assert_eq!(engine.allocations.len(), 2);
            assert!(
                engine
                    .allocations
                    .iter()
                    .all(|record| record.handle.is_some())
            );
            debit(&engine, 0, 0);
        } else {
            let mut arena = arena.unwrap();
            assert_eq!(
                arena.layout().cpu_mapping_bytes(),
                COMPLETION_SIGNAL_ARENA_BYTES_V1
            );
            debit(&engine, arena_bytes, 1);
            engine
                .with_bytes_mut(&mut arena, |bytes| bytes.fill(0))
                .unwrap();
            let arena = engine.map_mutable(arena).unwrap();
            let before_payload = calls(&engine);
            assert!(matches!(
                engine.allocate::<HostVisibleCoherentGttV1>(1),
                Err(MemorySessionError::HostVisibleBackingCredits(_))
            ));
            assert_eq!(calls(&engine), before_payload);
            debit(&engine, arena_bytes, 1);
            let arena = engine.unmap_mutable(arena).unwrap();
            engine
                .release(arena, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
            debit(&engine, 0, 0);
        }
        engine
            .release(control, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine
            .release(ring, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        debit(&engine, 0, 0);
    }
}

#[test]
fn n1_padding_and_each_capacity_dimension_reject_before_native_effects() {
    for limits in [(8192, 3), (16384, 1)] {
        let mut engine = configured(limits.0, limits.1);
        let token = engine.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        assert_eq!(token.layout().requested_bytes(), 4100);
        assert_eq!(token.layout().cpu_mapping_bytes(), 8192);
        debit(&engine, 8192, 1);
        let before = calls(&engine);
        let retained = usage(&engine);
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::HostVisibleBackingCredits(_))
        ));
        assert_eq!(calls(&engine), before);
        assert_eq!(usage(&engine), retained);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        debit(&engine, 0, 0);
    }
    let mut engine = configured(8192, 2);
    let before = calls(&engine);
    for bytes in [
        0,
        usize::MAX,
        (MAX_SHARED_GTT_SINGLE_CPU_BYTES_V1 + 1) as usize,
    ] {
        assert!(engine.allocate::<HostVisibleCoherentGttV1>(bytes).is_err());
        debit(&engine, 0, 0);
    }
    assert_eq!(calls(&engine), before);
}

#[test]
fn n1_configuration_rejects_replacement_domain_history_and_failed_preflight() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(8192, 2);
    let before = calls(&engine);
    assert!(
        engine
            .configure_host_visible_backing_budget_v1(device, vm, budget(4096, 1))
            .is_err()
    );
    assert_eq!(calls(&engine), before);
    for kind in 0..4 {
        let mut engine = acquired();
        match kind {
            0 => {
                assert!(engine.allocate::<HostVisibleCoherentGttV1>(0).is_err());
            }
            1 => {
                let token = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
                engine
                    .release(token, SharedAllocationPhaseV1::CpuWritable)
                    .unwrap();
            }
            2 => {
                let _retained = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
            }
            _ => {}
        }
        let mut candidate_vm = vm;
        if kind == 3 {
            candidate_vm.device.generation.0 += 1;
        }
        let before = calls(&engine);
        assert!(
            engine
                .configure_host_visible_backing_budget_v1(device, candidate_vm, budget(8192, 2))
                .is_err()
        );
        assert_eq!(calls(&engine), before);
        assert!(engine.host_backing_account.is_none());
    }
}

#[test]
fn n1_pre_effect_currentness_error_or_panic_cancels_only_unissued_credit() {
    for panic in [false, true] {
        let mut engine = configured(8192, 2);
        if panic {
            engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 1);
        } else {
            engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 1);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.allocate::<HostVisibleCoherentGttV1>(17)
        }));
        if panic {
            panic_payload(
                result.err().unwrap().as_ref(),
                "N2 native panic",
                "currentness",
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        debit(&engine, 0, 0);
        assert_eq!(engine.backend.reserve_va_calls, 0);
        closed(&mut engine);
    }
}

#[test]
fn n1_every_pre_record_native_failure_retains_quarantined_charge() {
    for panic in [false, true] {
        for operation in [
            "reserve_va",
            "alloc",
            "map_cpu",
            "prepare_cpu_mapping",
            "post_alloc",
            "post_mapping",
        ] {
            let mut engine = configured(8192, 2);
            let currentness = match operation {
                "post_alloc" => Some(2),
                "post_mapping" => Some(3),
                _ => None,
            };
            if let Some(delta) = currentness {
                if panic {
                    engine.backend.panic_currentness_at =
                        Some(engine.backend.currentness_calls + delta);
                } else {
                    engine.backend.fail_currentness_at =
                        Some(engine.backend.currentness_calls + delta);
                }
            } else if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.allocate::<HostVisibleCoherentGttV1>(17)
            }));
            if panic {
                panic_payload(
                    result.err().unwrap().as_ref(),
                    "N2 native panic",
                    if currentness.is_some() {
                        "currentness"
                    } else {
                        operation
                    },
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            debit(&engine, 4096, 1);
            assert_eq!(usage(&engine).quarantined_records, 1);
            assert!(engine.allocations.is_empty());
            assert_eq!(engine.backend.free_calls, 0);
            assert_eq!(engine.backend.release_va_calls, 0);
            closed(&mut engine);
        }
    }
    for corruption in 0..3 {
        let mut engine = configured(8192, 2);
        match corruption {
            0 => engine.backend.corrupt_flags = true,
            1 => engine.backend.corrupt_mapping_address = true,
            _ => engine.backend.fixed_va = Some(17),
        }
        assert!(engine.allocate::<HostVisibleCoherentGttV1>(17).is_err());
        debit(&engine, 4096, 1);
        assert_eq!(usage(&engine).quarantined_records, 1);
        closed(&mut engine);
    }
}

#[test]
fn n1_exact_charge_survives_cpu_and_mapped_access_until_full_disposal() {
    let mut engine = configured(8192, 2);
    let mut token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
    let identity = token.storage_identity();
    let before = usage(&engine);
    engine
        .with_bytes_mut(&mut token, |bytes| bytes.fill(5))
        .unwrap();
    assert_eq!(
        engine
            .with_bytes(&token, SharedAllocationPhaseV1::CpuWritable, |bytes| bytes
                [0])
            .unwrap(),
        5
    );
    let mut token = engine.map_mutable(token).unwrap();
    engine
        .overwrite_mapped_host_visible_subrange(&mut token, 3, &[7, 8])
        .unwrap();
    let mut captured = [0; 2];
    engine
        .copy_mapped_host_visible_subrange_into(&token, 3, &mut captured)
        .unwrap();
    assert_eq!(captured, [7, 8]);
    assert_eq!(
        engine
            .copy_mapped_host_visible_subrange(&token, 3, 2)
            .unwrap()
            .as_ref(),
        &[7, 8]
    );
    engine
        .overwrite_full_mapped_host_visible_and_sha256(&mut token, &[9; 17], 3)
        .unwrap();
    engine
        .overwrite_mapped_host_visible_subrange_in_current_scope(&mut token, 0, &[4])
        .unwrap();
    assert_eq!(usage(&engine), before);
    assert_eq!(token.storage_identity(), identity);
    let token = engine.unmap_mutable(token).unwrap();
    assert_eq!(usage(&engine), before);
    engine
        .release(token, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    debit(&engine, 0, 0);
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
}

#[test]
fn n1_cpu_callback_panic_keeps_original_payload_without_post_currentness() {
    for write in [false, true] {
        let mut engine = configured(8192, 2);
        let mut token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        let before = engine.backend.currentness_calls;
        engine.backend.panic_currentness_at = Some(before + 2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if write {
                engine
                    .with_bytes_mut(&mut token, |_| {
                        std::panic::panic_any(("N1 callback", "write"))
                    })
                    .map(|_: ()| ())
            } else {
                engine
                    .with_bytes(&token, SharedAllocationPhaseV1::CpuWritable, |_| {
                        std::panic::panic_any(("N1 callback", "read"))
                    })
                    .map(|_: ()| ())
            }
        }));
        panic_payload(
            result.err().unwrap().as_ref(),
            "N1 callback",
            if write { "write" } else { "read" },
        );
        assert_eq!(engine.backend.currentness_calls, before + 1);
        debit(&engine, 4096, 1);
        closed(&mut engine);
    }
}

#[test]
fn n1_cpu_and_mapped_post_currentness_panic_retains_written_record() {
    for mapped_access in [false, true] {
        let mut engine = configured(8192, 2);
        let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        let result = if mapped_access {
            let mut token = engine.map_mutable(token).unwrap();
            engine.backend.panic_operational_currentness_at =
                Some(engine.backend.operational_currentness_calls + 2);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.overwrite_mapped_host_visible_subrange(&mut token, 0, &[7])
            }))
        } else {
            let mut token = token;
            engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 2);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.with_bytes_mut(&mut token, |bytes| bytes[0] = 7)
            }))
        };
        if mapped_access {
            panic_payload(
                result.err().unwrap().as_ref(),
                "N1 native panic",
                "operational_currentness",
            );
        } else {
            panic_payload(
                result.err().unwrap().as_ref(),
                "N2 native panic",
                "currentness",
            );
        }
        assert_eq!(engine.allocations[0].mapping.as_ref().unwrap().bytes[0], 7);
        debit(&engine, 4096, 1);
        closed(&mut engine);
    }
}

#[test]
fn n1_mapped_data_access_panics_retain_charge_and_close_spare_capacity() {
    for operation in 0..5 {
        let mut engine = configured(8192, 2);
        let mut token = mapped(&mut engine, 17);
        let access = if operation < 2 {
            "with_bytes"
        } else {
            "with_bytes_mut"
        };
        engine.allocations[0].mapping.as_mut().unwrap().panic_access = Some(access);
        let before = usage(&engine);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match operation {
            0 => engine
                .copy_mapped_host_visible_subrange(&token, 0, 1)
                .map(|_| ()),
            1 => engine.copy_mapped_host_visible_subrange_into(&token, 0, &mut [0]),
            2 => engine.overwrite_mapped_host_visible_subrange(&mut token, 0, &[1]),
            3 => engine
                .overwrite_full_mapped_host_visible_and_sha256(&mut token, &[1; 17], 3)
                .map(|_| ()),
            _ => {
                engine.overwrite_mapped_host_visible_subrange_in_current_scope(&mut token, 0, &[1])
            }
        }));
        panic_payload(result.err().unwrap().as_ref(), "N2 native panic", access);
        assert_eq!(usage(&engine), before);
        closed(&mut engine);
    }
}

#[test]
fn n1_gpu_mapping_errors_and_panics_never_refund_or_allow_retry() {
    for unmap in [false, true] {
        for panic in [false, true] {
            for boundary in 0..3 {
                let mut engine = configured(8192, 2);
                let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
                let (cpu, gpu) = if unmap {
                    (None, Some(engine.map_mutable(token).unwrap()))
                } else {
                    (Some(token), None)
                };
                let operation = if boundary == 1 {
                    if unmap { "unmap_gpu" } else { "map_gpu" }
                } else {
                    "currentness"
                };
                if boundary == 1 {
                    if panic {
                        engine.backend.panic_operation = Some(operation);
                    } else {
                        engine.backend.fail_operation = Some(operation);
                    }
                } else {
                    let at = engine.backend.currentness_calls + if boundary == 0 { 1 } else { 2 };
                    if panic {
                        engine.backend.panic_currentness_at = Some(at);
                    } else {
                        engine.backend.fail_currentness_at = Some(at);
                    }
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if let Some(token) = gpu {
                        engine.unmap_mutable(token).map(|_| ())
                    } else {
                        engine.map_mutable(cpu.unwrap()).map(|_| ())
                    }
                }));
                if panic {
                    panic_payload(result.err().unwrap().as_ref(), "N2 native panic", operation);
                } else {
                    assert!(result.unwrap().is_err());
                }
                debit(&engine, 4096, 1);
                assert!(engine.allocations[0].host_backing_charge.is_some());
                closed(&mut engine);
            }
        }
    }
}

#[test]
fn n1_disposal_failure_matrix_retains_debit_through_final_currentness() {
    for panic in [false, true] {
        for boundary in 0..7 {
            let mut engine = configured(8192, 2);
            let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
            let operation = match boundary {
                1 => "unmap_cpu",
                3 => "free",
                5 => "release_va_reservation",
                _ => "currentness",
            };
            if boundary % 2 == 0 {
                let at = engine.backend.currentness_calls + boundary / 2 + 1;
                if panic {
                    engine.backend.panic_currentness_at = Some(at);
                } else {
                    engine.backend.fail_currentness_at = Some(at);
                }
            } else if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.release(token, SharedAllocationPhaseV1::CpuWritable)
            }));
            if panic {
                panic_payload(result.err().unwrap().as_ref(), "N2 native panic", operation);
            } else {
                assert!(result.unwrap().is_err());
            }
            debit(&engine, 4096, 1);
            assert!(engine.allocations[0].host_backing_charge.is_some());
            closed(&mut engine);
        }
    }
    let mut engine = configured(8192, 2);
    let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
    engine.retained_gpu_va_bytes = 0;
    assert!(
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
    debit(&engine, 4096, 1);
    assert!(!engine.allocations[0].is_fully_released());
    closed(&mut engine);
}

#[test]
fn n1_foreign_tokens_and_substituted_account_never_release_or_refund() {
    for coordinate in 0..8 {
        let mut engine = configured(8192, 2);
        let mut token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        match coordinate {
            0 => token.session_id += 1,
            1 => token.id += 1,
            2 => token.generation += 1,
            3 => token.layout.requested_bytes += 1,
            4 => token.layout.cpu_mapping_bytes *= 2,
            5 => token.layout.gpu_va_bytes *= 2,
            6 => token.layout.uapi_flags ^= 1,
            _ => token.layout.profile = SharedGttProfileV1::Kernarg,
        }
        let before = calls(&engine);
        assert!(
            engine
                .release(token, SharedAllocationPhaseV1::CpuWritable)
                .is_err()
        );
        assert_eq!(calls(&engine), before);
        debit(&engine, 4096, 1);
    }
    for foreign_domain in [false, true] {
        let mut engine = configured(8192, 2);
        let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        let (device, mut vm) = device_vm(1);
        if foreign_domain {
            vm.id.0 += 1;
        }
        let foreign =
            HostBackingAccountV1::new(engine.session_id, device, vm, budget(8192, 2)).unwrap();
        let original = engine.host_backing_account.replace(foreign).unwrap();
        let before = calls(&engine);
        assert!(
            engine
                .release(token, SharedAllocationPhaseV1::CpuWritable)
                .is_err()
        );
        assert_eq!(calls(&engine), before);
        assert_eq!(original.usage().used_backing_bytes, 4096);
        assert_eq!(usage(&engine).used_backing_bytes, 0);
    }
}

#[test]
fn n1_confirmed_disposal_reuses_credit_and_record_slot_not_old_identity() {
    let mut engine = configured(4096, 1);
    let mut old = None;
    for _ in 0..=MAX_SHARED_GTT_ALLOCATIONS_V1 {
        let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
        if old.is_none() {
            old = Some(HostCpu {
                session_id: token.session_id,
                id: token.id,
                generation: token.generation,
                layout: token.layout,
                marker: PhantomData,
            });
        }
        debit(&engine, 4096, 1);
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        debit(&engine, 0, 0);
    }
    assert_eq!(engine.allocations.len(), MAX_SHARED_GTT_ALLOCATIONS_V1);
    let before = calls(&engine);
    assert!(
        engine
            .release(old.unwrap(), SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert_eq!(calls(&engine), before);
    debit(&engine, 0, 0);
}

#[test]
fn n1_every_signal_ingress_quarantines_original_backend_panic() {
    let operations = [
        "observe_aql_counters",
        "observe_aql_counters",
        "fetch_add_aql_write",
        "fetch_add_aql_write",
        "publish_sdma_write_release",
        "write_sdma_slot",
        "write_aql_slot",
        "write_aql_slot",
        "publish_aql_header",
        "publish_aql_header",
        "observe_i64_acquire",
        "observe_i64_acquire",
        "observe_aql_packet_header_acquire",
        "observe_completion_signal_acquire",
        "observe_completion_signal_state_acquire",
        "observe_completion_signal_acquire",
        "observe_completion_signal_acquire",
        "reset_completion_signal_release",
        "reset_completion_signal_release",
    ];
    for (case, operation) in operations.into_iter().enumerate() {
        let mut engine = configured(8192, 2);
        let mut token = mapped(&mut engine, 4096);
        engine.allocations[0].mapping.as_mut().unwrap().panic_access = Some(operation);
        let before = usage(&engine);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match case {
            0 => engine.observe_aql_counters(&mut token).map(|_| ()),
            1 => engine
                .observe_aql_counters_in_current_scope(&mut token)
                .map(|_| ()),
            2 => engine.fetch_add_aql_write(&mut token, 1).map(|_| ()),
            3 => engine
                .fetch_add_aql_write_in_current_scope(&mut token, 1)
                .map(|_| ()),
            4 => engine.publish_sdma_write_release_in_current_scope(&mut token, 0, 1),
            5 => engine.write_sdma_slot_in_current_scope(&mut token, 0, &[0; 64]),
            6 => engine.write_aql_slot(&mut token, 0, &[0; 64]),
            7 => engine.write_aql_slot_in_current_scope(&mut token, 0, &[0; 64]),
            8 => engine.publish_aql_header(&mut token, 0, 1),
            9 => engine.publish_aql_header_in_current_scope(&mut token, 0, 1),
            10 => engine.observe_i64_acquire(&mut token, 0).map(|_| ()),
            11 => engine
                .observe_i64_acquire_in_current_scope(&mut token, 0)
                .map(|_| ()),
            12 => engine.observe_aql_packet_header(&mut token, 0).map(|_| ()),
            13 => engine.observe_completion_signal(&mut token, 0).map(|_| ()),
            14 => engine
                .observe_completion_signal_state(&mut token, 0)
                .map(|_| ()),
            15 => engine
                .observe_completion_signal_in_current_scope(&mut token, 0)
                .map(|_| ()),
            16 => engine
                .observe_completion_signals_in_current_scope(&mut token, &[0])
                .map(|_| ()),
            17 => engine.reset_completion_signal(&mut token, 0),
            _ => engine.reset_completion_signal_in_current_scope(&mut token, 0),
        }));
        panic_payload(result.err().unwrap().as_ref(), "N1 mapped panic", operation);
        assert_eq!(usage(&engine), before);
        closed(&mut engine);
    }
}

fn configure_fixture(fixture: &mut BackingConstructorFixture, configured: bool) {
    fixture
        .ownership
        .configure_optional_host_visible_backing_budget(
            &mut fixture.engine,
            fixture.device,
            fixture.vm,
            configured.then(|| budget(16384, 4)),
        )
        .unwrap();
}

fn projected_host(fixture: &mut BackingConstructorFixture) -> HostMapped {
    let token = fixture
        .engine
        .allocate::<HostVisibleCoherentGttV1>(4100)
        .unwrap();
    let (id, generation, layout, base, handle) = fixture.engine.evidence(&token).unwrap();
    let (reservation, allocation, mapping) = model_keys(fixture.vm, id, generation);
    let projected = project_allocation(
        fixture.foundation.memory(),
        reservation,
        allocation,
        base,
        layout,
        handle,
        MemoryKindV1::HostVisibleCoherent,
    )
    .unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    let token = fixture.engine.map_mutable(token).unwrap();
    let projected = project_map(fixture.foundation.memory(), mapping, fixture.device).unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    token
}

fn projected_dispose(fixture: &mut BackingConstructorFixture, token: HostMapped) {
    let (reservation, allocation, mapping) = model_keys(fixture.vm, token.id, token.generation);
    let token = fixture.engine.unmap_mutable(token).unwrap();
    let projected = project_unmap(fixture.foundation.memory(), mapping).unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    let projected = project_release(
        fixture.foundation.memory(),
        reservation,
        allocation,
        mapping,
    )
    .unwrap();
    fixture
        .engine
        .release(token, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
}

#[test]
fn n1_actual_foundation_loans_and_pool_retags_keep_exact_charge_in_both_orders() {
    for compute_first in [false, true] {
        let mut fixture = BackingConstructorFixture::new(None);
        configure_fixture(&mut fixture, true);
        let compute = compute_first.then(|| fixture.mapped_device());
        let host_before = (!compute_first).then(|| projected_host(&mut fixture));
        let authorities = compute.as_ref().into_iter().collect::<Vec<_>>();
        let mut queue = fixture.transfer(&authorities).unwrap();
        assert!(fixture.engine.host_backing_configuration_closed);
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        let token = host_before.unwrap_or_else(|| projected_host(&mut fixture));
        let identity = token.storage_identity();
        let before = usage(&fixture.engine);
        assert_eq!(before.used_backing_bytes, 8192);
        assert_eq!(before.used_allocation_records, 1);
        let owner = QueueKeyV1 {
            vm: fixture.vm,
            id: QueueInstanceIdV1(33),
            generation: QueueGenerationV1(1),
        };
        let mut buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(token),
            owner,
            1,
            4100,
        );
        // Exercise the production move-only buffer generation/logical-extent
        // transitions; the fake fixture is not the Linux queue pool facade.
        buffer.advance_pool_generation().unwrap();
        buffer.set_logical_bytes(1024);
        let mut cached = vec![buffer];
        fixture
            .ownership
            .reclaim_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
                loan,
            )
            .unwrap();
        assert_eq!(usage(&fixture.engine), before);
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        let mut buffer = cached.pop().unwrap();
        buffer.advance_pool_generation().unwrap();
        buffer.set_logical_bytes(4100);
        let (Gfx942SdmaBufferStorageV1::Host(mut token), _, _, _) = buffer.into_bridge_parts()
        else {
            panic!("host fixture");
        };
        assert_eq!(token.storage_identity(), identity);
        fixture
            .engine
            .overwrite_mapped_host_visible_subrange(&mut token, 0, &[7])
            .unwrap();
        assert_eq!(usage(&fixture.engine), before);
        projected_dispose(&mut fixture, token);
        debit(&fixture.engine, 0, 0);
        fixture
            .ownership
            .reclaim_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
                loan,
            )
            .unwrap();
        fixture
            .ownership
            .restore_foundation(
                &mut fixture.engine,
                &mut fixture.foundation,
                queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        debit(&fixture.engine, 0, 0);
        let before_calls = calls(&fixture.engine);
        assert!(
            fixture
                .ownership
                .configure_optional_host_visible_backing_budget(
                    &mut fixture.engine,
                    fixture.device,
                    fixture.vm,
                    Some(budget(8192, 2))
                )
                .is_err()
        );
        assert_eq!(calls(&fixture.engine), before_calls);
    }
}

#[test]
fn n1_unconfigured_transfer_cannot_reopen_budget_after_restore_or_live_loan() {
    let mut fixture = BackingConstructorFixture::new(None);
    let before = calls(&fixture.engine);
    configure_fixture(&mut fixture, false);
    assert_eq!(calls(&fixture.engine), before);
    let mut queue = fixture.transfer(&[]).unwrap();
    let loan = fixture
        .ownership
        .loan_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    assert!(
        fixture
            .ownership
            .configure_optional_host_visible_backing_budget(
                &mut fixture.engine,
                fixture.device,
                fixture.vm,
                Some(budget(8192, 2))
            )
            .is_err()
    );
    fixture
        .ownership
        .reclaim_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
            loan,
        )
        .unwrap();
    fixture
        .ownership
        .restore_foundation(
            &mut fixture.engine,
            &mut fixture.foundation,
            queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    assert!(
        fixture
            .ownership
            .configure_optional_host_visible_backing_budget(
                &mut fixture.engine,
                fixture.device,
                fixture.vm,
                Some(budget(8192, 2))
            )
            .is_err()
    );
    assert!(fixture.engine.host_backing_account.is_none());
}

#[test]
#[allow(clippy::drop_non_drop)] // Explicitly relinquish token authority before observing retained custody.
fn n1_failed_retake_retains_live_debit_but_does_not_resurrect_disposed_backing() {
    for dispose in [false, true] {
        let mut fixture = BackingConstructorFixture::new(None);
        configure_fixture(&mut fixture, true);
        let token = projected_host(&mut fixture);
        let mut queue = fixture.transfer(&[]).unwrap();
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        if dispose {
            projected_dispose(&mut fixture, token);
        } else {
            drop(token);
        }
        let expected = if dispose { 0 } else { 8192 };
        debit(&fixture.engine, expected, u64::from(!dispose));
        let wrong_vm = VmKeyV1 {
            id: VmIdV1(fixture.vm.id.0 + 1),
            ..fixture.vm
        };
        assert!(
            fixture
                .ownership
                .reclaim_foundation(
                    fixture.engine.session_id,
                    &mut fixture.foundation,
                    &mut queue,
                    fixture.device,
                    wrong_vm,
                    loan
                )
                .is_err()
        );
        // The enclosing live-owner guard quarantines on retake rejection. Its
        // existing queue regression tests cover that Linux facade wiring.
        assert!(
            fixture
                .engine
                .quarantine::<()>(MemorySessionError::Model("test retake rejection"))
                .is_err()
        );
        debit(&fixture.engine, expected, u64::from(!dispose));
        closed(&mut fixture.engine);
    }
}
