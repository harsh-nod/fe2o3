//! Session-local N2 integration only; the fake backend provides no hardware evidence.
use super::*;

fn configured(bytes: u64, records: usize) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_device_backing_budget_v1(
            device,
            vm,
            Gfx942DeviceBackingBudgetV1::new(bytes, records).unwrap(),
        )
        .unwrap();
    engine
}

fn usage(engine: &SharedMemoryEngine<FakeBackend>) -> Gfx942DeviceBackingUsageV1 {
    engine.device_backing_account.as_ref().unwrap().usage()
}

fn assert_debit(engine: &SharedMemoryEngine<FakeBackend>, bytes: u64, records: u64) {
    let observed = usage(engine);
    assert_eq!(observed.used_backing_bytes, bytes);
    assert_eq!(observed.used_allocation_records, records);
    assert_eq!(observed.reserved_records, 0);
}

fn assert_quarantined_without_new_native_work(engine: &mut SharedMemoryEngine<FakeBackend>) {
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let calls = (
        engine.backend.currentness_calls,
        engine.backend.reserve_va_calls,
        engine.backend.alloc_calls,
        engine.backend.multi_map_inputs.len(),
        engine.backend.multi_unmap_inputs.len(),
    );
    let (device, vm) = device_vm(1);
    assert!(matches!(
        engine.allocate_device_memory(device, vm, 4096, 4096),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(
        calls,
        (
            engine.backend.currentness_calls,
            engine.backend.reserve_va_calls,
            engine.backend.alloc_calls,
            engine.backend.multi_map_inputs.len(),
            engine.backend.multi_unmap_inputs.len(),
        )
    );
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn n2_internal_xgmi_partial_prefix_unwind_closes_spare_capacity() {
    let (device, vm) = device_vm(1);
    for unmap in [false, true] {
        let mut engine = configured(8192, 2);
        let lease = engine
            .allocate_device_memory_with_flags(
                device,
                vm,
                4096,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        let (lease, mapping) = if unmap {
            let mapping = engine
                .map_device_memory_to_gpus(lease, Box::new([7, 9]))
                .ok()
                .unwrap();
            engine.backend.multi_unmap_script = vec![(1, true)];
            engine.backend.panic_multi_unmap_at = Some(2);
            (None, Some(mapping))
        } else {
            engine.backend.multi_map_script = vec![(1, true)];
            engine.backend.panic_multi_map_at = Some(2);
            (Some(lease), None)
        };
        let before = usage(&engine);
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(mapping) = mapping {
                engine
                    .unmap_device_memory_from_gpus(mapping)
                    .map(drop)
                    .ok()
                    .unwrap();
            } else {
                engine
                    .map_device_memory_to_gpus(lease.unwrap(), Box::new([7, 9]))
                    .map(drop)
                    .ok()
                    .unwrap();
            }
        }))
        .expect_err("interrupted second native prefix must retain its panic");
        let operation = if unmap {
            "unmap_gpu_ids"
        } else {
            "map_gpu_ids"
        };
        assert_eq!(
            payload.downcast_ref::<(&'static str, &'static str)>(),
            Some(&("N2 native panic", operation))
        );
        let inputs = if unmap {
            &engine.backend.multi_unmap_inputs
        } else {
            &engine.backend.multi_map_inputs
        };
        assert_eq!(inputs, &[(vec![7, 9], 0), (vec![7, 9], 1)]);
        assert_eq!(
            engine.device_memory[0].phase,
            DeviceMemoryPhaseV1::Ambiguous
        );
        assert_eq!(usage(&engine), before);
        assert_debit(&engine, 4096, 1);
        assert_quarantined_without_new_native_work(&mut engine);
    }
}

#[test]
fn n2_paired_xgmi_unwind_guards_either_account_and_peer_post_currentness() {
    let (device, vm) = device_vm(1);
    for (local_configured, peer_configured) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        for unmap in [false, true] {
            for peer_post in [false, true] {
                let mut local = if local_configured {
                    configured(8192, 2)
                } else {
                    acquired()
                };
                let mut peer = if peer_configured {
                    configured(8192, 2)
                } else {
                    acquired()
                };
                let lease = local
                    .allocate_device_memory_with_flags(
                        device,
                        vm,
                        4096,
                        4096,
                        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                    )
                    .unwrap();
                let _peer_lease = peer.allocate_device_memory(device, vm, 4096, 4096).unwrap();
                let (lease, mapping) = if unmap {
                    (
                        None,
                        Some(
                            local
                                .map_device_memory_to_gpus(lease, Box::new([7, 9]))
                                .ok()
                                .unwrap(),
                        ),
                    )
                } else {
                    (Some(lease), None)
                };
                if peer_post {
                    peer.backend.panic_currentness_at = Some(peer.backend.currentness_calls + 2);
                } else if unmap {
                    local.backend.multi_unmap_script = vec![(1, true)];
                    local.backend.panic_multi_unmap_at = Some(2);
                } else {
                    local.backend.multi_map_script = vec![(1, true)];
                    local.backend.panic_multi_map_at = Some(2);
                }
                let local_usage = local
                    .device_backing_account
                    .as_ref()
                    .map(|account| account.usage());
                let peer_usage = peer
                    .device_backing_account
                    .as_ref()
                    .map(|account| account.usage());
                let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_device_backing_pair_unwind_quarantine(
                        &mut local,
                        &mut peer,
                        |local, peer| {
                            peer.check_currentness().unwrap();
                            if let Some(mapping) = mapping {
                                let result = local.unmap_device_memory_from_gpus(mapping);
                                let peer_post = peer.check_currentness();
                                finish_xgmi_unmap_with_peer_post(result, peer_post)
                                    .map(drop)
                                    .ok()
                                    .unwrap();
                            } else {
                                let result = local
                                    .map_device_memory_to_gpus(lease.unwrap(), Box::new([7, 9]));
                                let peer_post = peer.check_currentness();
                                finish_xgmi_map_with_peer_post(result, peer_post)
                                    .map(drop)
                                    .ok()
                                    .unwrap();
                            }
                        },
                    );
                }))
                .expect_err("paired native panic must retain the original payload");
                let operation = if peer_post {
                    "currentness"
                } else if unmap {
                    "unmap_gpu_ids"
                } else {
                    "map_gpu_ids"
                };
                assert_eq!(
                    payload.downcast_ref::<(&'static str, &'static str)>(),
                    Some(&("N2 native panic", operation))
                );
                assert_eq!(
                    local
                        .device_backing_account
                        .as_ref()
                        .map(|account| account.usage()),
                    local_usage
                );
                assert_eq!(
                    peer.device_backing_account
                        .as_ref()
                        .map(|account| account.usage()),
                    peer_usage
                );
                assert_eq!(
                    local.device_memory[0].phase,
                    if peer_post {
                        if unmap {
                            DeviceMemoryPhaseV1::Unmapped
                        } else {
                            DeviceMemoryPhaseV1::Mapped
                        }
                    } else {
                        DeviceMemoryPhaseV1::Ambiguous
                    }
                );
                if local_configured || peer_configured {
                    assert_quarantined_without_new_native_work(&mut local);
                    assert_quarantined_without_new_native_work(&mut peer);
                } else {
                    assert_eq!(local.phase(), SharedMemorySessionPhaseV1::Active);
                    assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Active);
                    assert_eq!(local.backend.free_calls + peer.backend.free_calls, 0);
                    assert_eq!(
                        local.backend.release_va_calls + peer.backend.release_va_calls,
                        0
                    );
                }
            }
        }
    }
}

#[test]
#[allow(clippy::result_large_err)] // Recovery custody stays inline, as in production.
fn n2_paired_xgmi_recoverable_prefix_errors_preserve_exact_owners() {
    let (device, vm) = device_vm(1);
    for unmap in [false, true] {
        let mut local = configured(8192, 2);
        let mut peer = configured(8192, 2);
        let lease = local
            .allocate_device_memory_with_flags(
                device,
                vm,
                4096,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        let mapping = if unmap {
            let mapping = local
                .map_device_memory_to_gpus(lease, Box::new([7, 9]))
                .ok()
                .unwrap();
            local.backend.multi_unmap_script = vec![(1, true), (1, true)];
            let failure =
                with_device_backing_pair_unwind_quarantine(&mut local, &mut peer, |local, peer| {
                    let result = local.unmap_device_memory_from_gpus(mapping);
                    let peer_post = peer.check_currentness();
                    finish_xgmi_unmap_with_peer_post(result, peer_post)
                })
                .err()
                .unwrap();
            assert!(matches!(
                failure.error,
                MemorySessionError::Injected("multi_unmap_gpu")
            ));
            let Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(mapping) = failure.recovery else {
                panic!("partial owner lost")
            };
            assert_eq!((mapping.mapped_prefix, mapping.unmapped_prefix), (2, 1));
            mapping
        } else {
            local.backend.multi_map_script = vec![(1, true), (1, true)];
            let failure =
                with_device_backing_pair_unwind_quarantine(&mut local, &mut peer, |local, peer| {
                    let result = local.map_device_memory_to_gpus(lease, Box::new([7, 9]));
                    let peer_post = peer.check_currentness();
                    finish_xgmi_map_with_peer_post(result, peer_post)
                })
                .err()
                .unwrap();
            assert!(matches!(
                failure.error,
                MemorySessionError::Injected("multi_map_gpu")
            ));
            let Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) = failure.recovery else {
                panic!("partial owner lost")
            };
            assert_eq!((mapping.mapped_prefix, mapping.unmapped_prefix), (1, 0));
            mapping
        };
        assert_eq!(mapping.gpu_ids.as_ref(), &[7, 9]);
        assert!(!mapping.unmap_indeterminate);
        assert_eq!(local.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Active);
        assert_debit(&local, 4096, 1);
        assert_debit(&peer, 0, 0);
        let lease = local.unmap_device_memory_from_gpus(mapping).ok().unwrap();
        local.release_device_memory(lease).unwrap();
        assert_debit(&local, 0, 0);
    }
}

#[test]
fn n2_public_xgmi_pair_guard_encloses_native_and_peer_post_checks() {
    // This checks Linux wiring only; fake pair tests do not admit a real route.
    let source = include_str!("../../shared_memory.rs");
    for (name, native, finish) in [
        (
            "map_gfx942_device_memory_for_xgmi_peer",
            "self.engine.map_device_memory_to_gpus(lease, roster.into())",
            "finish_xgmi_map_with_peer_post(result, peer_post)",
        ),
        (
            "unmap_gfx942_device_memory_from_xgmi_peer",
            "self.engine.unmap_device_memory_from_gpus(mapping)",
            "finish_xgmi_unmap_with_peer_post(result, peer_post)",
        ),
    ] {
        let wrapper = source.split(&format!("pub fn {name}(")).nth(1).unwrap();
        let (wrapper, inner) = wrapper.split_once(&format!("fn {name}_inner(")).unwrap();
        assert!(wrapper.contains("with_device_backing_pair_unwind_quarantine(self, peer,"));
        assert!(wrapper.contains(&format!("session.{name}_inner(peer, route,")));
        let inner = inner.split("\n    pub(crate) fn").next().unwrap();
        assert!(inner.contains(native));
        assert!(inner.contains("let peer_post = peer.engine.check_currentness();"));
        assert!(inner.contains(finish));
    }
}

#[test]
fn n2_padding_and_both_capacity_dimensions_reject_before_native_effects() {
    let (device, vm) = device_vm(1);
    for (budget_bytes, budget_records, second_bytes) in
        [(4096, 2, 1), (8192, 1, 1), (8192, 2, 4097)]
    {
        let mut engine = configured(budget_bytes, budget_records);
        let first = engine.allocate_device_memory(device, vm, 1, 1).unwrap();
        assert_debit(&engine, 4096, 1);
        assert_eq!(first.layout.requested_bytes, 1);
        assert_eq!(first.layout.backing_bytes, 4096);
        let calls = (
            engine.backend.currentness_calls,
            engine.backend.reserve_va_calls,
            engine.backend.alloc_calls,
        );
        assert!(matches!(
            engine.allocate_device_memory(device, vm, second_bytes, 1),
            Err(MemorySessionError::DeviceBackingCredits(_))
        ));
        assert_eq!(
            (
                engine.backend.currentness_calls,
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls
            ),
            calls
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_debit(&engine, 4096, 1);
        engine.release_device_memory(first).unwrap();
        assert_debit(&engine, 0, 0);
    }
}

#[test]
fn n2_invalid_layout_overflow_and_domain_do_not_reserve_or_issue() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(8192, 2);
    let calls = engine.backend.currentness_calls;
    for (bytes, alignment) in [
        (0, 4096),
        (u64::MAX, 4096),
        (4096, 0),
        (4096, 3),
        (4096, 8192),
    ] {
        assert!(
            engine
                .allocate_device_memory(device, vm, bytes, alignment)
                .is_err()
        );
    }
    for coordinate in 0..4 {
        let mut other_device = device;
        let mut other_vm = vm;
        match coordinate {
            0 => other_device.physical.0 += 1,
            1 => other_device.generation.0 += 1,
            2 => other_vm.id.0 += 1,
            3 => {
                other_device.generation.0 += 1;
                other_vm.device = other_device;
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            engine.allocate_device_memory(other_device, other_vm, 4096, 4096),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
    }
    engine.retained_device_memory_bytes = u64::MAX;
    assert!(matches!(
        engine.allocate_device_memory(device, vm, 4096, 4096),
        Err(MemorySessionError::SizeOverflow)
    ));
    assert_eq!(engine.backend.currentness_calls, calls);
    assert_eq!(engine.backend.reserve_va_calls, 0);
    assert_eq!(engine.backend.alloc_calls, 0);
    assert_debit(&engine, 0, 0);
}

#[test]
fn n2_configuration_is_single_use_fresh_and_irreversibly_closed() {
    let (device, vm) = device_vm(1);
    let budget = Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap();
    let mut engine = configured(4096, 1);
    let before = usage(&engine);
    let calls = engine.backend.currentness_calls;
    assert!(
        engine
            .configure_device_backing_budget_v1(device, vm, budget)
            .is_err()
    );
    assert_eq!(usage(&engine), before);
    assert_eq!(engine.backend.currentness_calls, calls);
    for state in 0..4 {
        let mut engine = acquired();
        match state {
            0 => engine.device_backing_configuration_closed = true,
            1 => engine.device_backing_activity_started = true,
            2 => {
                let lease = engine
                    .allocate_device_memory(device, vm, 4096, 4096)
                    .unwrap();
                engine.release_device_memory(lease).unwrap();
            }
            3 => engine.phase = SharedMemorySessionPhaseV1::Quarantined,
            _ => unreachable!(),
        }
        let calls = engine.backend.currentness_calls;
        assert!(
            engine
                .configure_device_backing_budget_v1(device, vm, budget)
                .is_err()
        );
        assert!(engine.device_backing_account.is_none());
        assert_eq!(engine.backend.currentness_calls, calls);
    }
    let mut engine = acquired();
    let (wrong_device, _) = device_vm(2);
    let calls = engine.backend.currentness_calls;
    assert!(
        engine
            .configure_device_backing_budget_v1(wrong_device, vm, budget)
            .is_err()
    );
    assert!(engine.device_backing_account.is_none());
    assert_eq!(engine.backend.currentness_calls, calls);
}

#[test]
fn n2_pre_effect_currentness_cancels_only_unissued_reservation() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(4096, 1);
    engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 1);
    assert!(
        engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .is_err()
    );
    assert_debit(&engine, 0, 0);
    assert_eq!(usage(&engine).quarantined_records, 0);
    assert!(!engine.device_backing_activity_started);
    assert!(engine.device_memory.is_empty());
    assert_eq!(engine.backend.reserve_va_calls, 0);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
}

#[test]
fn n2_reservation_failure_before_record_quarantines_the_debit() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(4096, 1);
    engine.backend.fail_operation = Some("reserve_va");
    assert!(
        engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .is_err()
    );
    assert_debit(&engine, 4096, 1);
    assert_eq!(usage(&engine).quarantined_records, 1);
    assert!(engine.device_memory.is_empty());
    assert!(engine.device_backing_activity_started);
    assert_eq!(engine.retained_device_memory_bytes, 0);
    assert_eq!(engine.backend.reserve_va_calls, 1);
    assert_eq!(engine.backend.alloc_calls, 0);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
}

#[test]
fn n2_malformed_allocation_and_native_errors_keep_record_charge() {
    let (device, vm) = device_vm(1);
    for failure in 0..7 {
        let mut engine = configured(8192, 2);
        match failure {
            0 => engine.backend.fixed_va = Some(u64::MAX - 2047),
            1 => engine.backend.fixed_va = Some(0x2_0001),
            2 => engine.backend.alloc_oom = true,
            3 => engine.backend.corrupt_flags = true,
            4 => engine.backend.fail_operation = Some("alloc"),
            5 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2),
            6 => {
                let first = engine
                    .allocate_device_memory(device, vm, 4096, 4096)
                    .unwrap();
                engine.backend.fixed_va = Some(engine.device_memory[0].gpu_va);
                drop(first);
            }
            _ => unreachable!(),
        }
        assert!(
            engine
                .allocate_device_memory(device, vm, 4096, 4096)
                .is_err()
        );
        let records = if failure == 6 { 2 } else { 1 };
        assert_debit(&engine, records * 4096, records);
        assert_eq!(usage(&engine).retained_records, records as usize);
        assert_eq!(usage(&engine).quarantined_records, 0);
        assert!(
            engine
                .device_memory
                .iter()
                .all(|record| record.backing_charge.is_some())
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}

#[test]
fn n2_mapping_transitions_and_lease_drop_never_refund_backing() {
    let (device, vm) = device_vm(1);
    for flags in [
        KfdAllocMemoryFlags::DEVICE_LOCAL,
        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
    ] {
        let mut engine = configured(4096, 1);
        let lease = engine
            .allocate_device_memory_with_flags(device, vm, 17, 4, flags)
            .unwrap();
        assert_debit(&engine, 4096, 1);
        let lease = engine.map_device_memory(lease).unwrap();
        assert_debit(&engine, 4096, 1);
        let lease = engine.unmap_device_memory(lease).unwrap();
        assert_debit(&engine, 4096, 1);
        drop(lease);
        assert_debit(&engine, 4096, 1);
        assert_eq!(engine.backend.free_calls, 0);
        let account = engine.device_backing_account.take().unwrap();
        drop(engine);
        assert_eq!(account.usage().used_backing_bytes, 4096);
        assert_eq!(account.usage().quarantined_records, 1);
    }
}

#[test]
fn n2_map_unmap_failure_matrix_preserves_debit() {
    let (device, vm) = device_vm(1);
    for failure in 0..6 {
        let mut engine = configured(4096, 1);
        let lease = engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        if failure < 3 {
            match failure {
                0 => engine.backend.map_progress = 0,
                1 => engine.backend.map_errno = true,
                2 => {
                    engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2)
                }
                _ => unreachable!(),
            }
            assert!(engine.map_device_memory(lease).is_err());
        } else {
            let lease = engine.map_device_memory(lease).unwrap();
            match failure {
                3 => engine.backend.unmap_progress = 0,
                4 => engine.backend.unmap_errno = true,
                5 => {
                    engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2)
                }
                _ => unreachable!(),
            }
            assert!(engine.unmap_device_memory(lease).is_err());
        }
        assert_debit(&engine, 4096, 1);
        assert_eq!(usage(&engine).retained_records, 1);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}

#[test]
fn n2_disposal_failure_matrix_never_refunds_or_retries() {
    let (device, vm) = device_vm(1);
    for failure in 0..6 {
        let mut engine = configured(4096, 1);
        let lease = engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        match failure {
            0 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 1),
            1 => engine.backend.fail_operation = Some("free"),
            2 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2),
            3 => engine.backend.fail_operation = Some("release_va_reservation"),
            4 => engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 3),
            5 => engine.retained_device_memory_bytes = 0,
            _ => unreachable!(),
        }
        assert!(engine.release_device_memory(lease).is_err());
        assert_debit(&engine, 4096, 1);
        assert_eq!(usage(&engine).retained_records, 1);
        assert!(!engine.device_memory[0].is_fully_released());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, usize::from(failure != 0));
        assert_eq!(engine.backend.release_va_calls, usize::from(failure >= 3));
    }
}

#[test]
fn n2_unwind_before_and_after_record_retains_debit_and_original_payload() {
    let (device, vm) = device_vm(1);
    for operation in [
        "currentness",
        "reserve_va",
        "alloc",
        "free",
        "release_va_reservation",
    ] {
        let mut engine = configured(4096, 1);
        let lease = matches!(operation, "free" | "release_va_reservation").then(|| {
            engine
                .allocate_device_memory(device, vm, 4096, 4096)
                .unwrap()
        });
        engine.backend.panic_operation = Some(operation);
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match lease {
            Some(lease) => engine.release_device_memory(lease),
            None => engine
                .allocate_device_memory(device, vm, 4096, 4096)
                .map(drop),
        }))
        .expect_err("injected native unwind must escape");
        assert_eq!(
            payload.downcast_ref::<(&'static str, &'static str)>(),
            Some(&("N2 native panic", operation))
        );
        let pre_effect = operation == "currentness";
        assert_debit(
            &engine,
            if pre_effect { 0 } else { 4096 },
            u64::from(!pre_effect),
        );
        assert_eq!(
            usage(&engine).quarantined_records,
            usize::from(operation == "reserve_va")
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let calls = (
            engine.backend.reserve_va_calls,
            engine.backend.alloc_calls,
            engine.backend.free_calls,
            engine.backend.release_va_calls,
        );
        engine.backend.panic_operation = None;
        assert!(matches!(
            engine.allocate_device_memory(device, vm, 4096, 4096),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!(
            (
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls,
                engine.backend.free_calls,
                engine.backend.release_va_calls
            ),
            calls
        );
    }
}

#[test]
fn n2_exact_disposal_reuses_credit_not_stale_native_identity() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(4096, 1);
    let mut prior_id = 0;
    for _ in 0..MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 + 2 {
        let lease = engine.allocate_device_memory(device, vm, 1, 1).unwrap();
        assert!(lease.id > prior_id);
        prior_id = lease.id;
        let stale = Gfx942DeviceMemoryLeaseV1 {
            id: lease.id,
            generation: lease.generation,
            device,
            vm,
            layout: lease.layout,
            marker: PhantomData,
        };
        assert_debit(&engine, 4096, 1);
        engine.release_device_memory(lease).unwrap();
        assert_debit(&engine, 0, 0);
        assert_eq!(usage(&engine).retained_records, 0);
        let free_calls = engine.backend.free_calls;
        assert!(matches!(
            engine.release_device_memory(stale),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert_eq!(engine.backend.free_calls, free_calls);
    }
    assert_eq!(
        engine.device_memory.len(),
        MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
    );
}

#[test]
fn n2_substituted_charge_cannot_reach_native_disposal() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(8192, 2);
    let first = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let second = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let charge = engine.device_memory[0].backing_charge.take();
    engine.device_memory[0].backing_charge = engine.device_memory[1].backing_charge.take();
    engine.device_memory[1].backing_charge = charge;
    assert!(matches!(
        engine.release_device_memory(first),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_debit(&engine, 8192, 2);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    drop(second);
}

#[test]
fn n2_stale_lease_coordinates_do_not_release_or_refund() {
    let (device, vm) = device_vm(1);
    let mut engine = configured(4096, 1);
    let lease = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let calls = engine.backend.currentness_calls;
    for coordinate in 0..7 {
        let mut stale = Gfx942DeviceMemoryLeaseV1 {
            id: lease.id,
            generation: lease.generation,
            device,
            vm,
            layout: lease.layout,
            marker: PhantomData,
        };
        match coordinate {
            0 => stale.id += 1,
            1 => stale.generation += 1,
            2 => stale.device.physical.0 += 1,
            3 => {
                stale.device.generation.0 += 1;
                stale.vm.device = stale.device;
            }
            4 => stale.vm.id.0 += 1,
            5 => stale.layout.requested_bytes -= 1,
            6 => stale.layout.backing_bytes += 4096,
            _ => unreachable!(),
        }
        assert!(matches!(
            engine.release_device_memory(stale),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert_debit(&engine, 4096, 1);
        assert_eq!(engine.backend.currentness_calls, calls);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    }
    engine.release_device_memory(lease).unwrap();
    assert_debit(&engine, 0, 0);
}

#[test]
fn n2_map_unmap_unwind_closes_spare_capacity_at_every_native_boundary() {
    let (device, vm) = device_vm(1);
    for unmap in [false, true] {
        for boundary in 0..3 {
            let mut engine = configured(8192, 2);
            let lease = engine
                .allocate_device_memory(device, vm, 4096, 4096)
                .unwrap();
            let (unmapped, mapped) = if unmap {
                (None, Some(engine.map_device_memory(lease).unwrap()))
            } else {
                (Some(lease), None)
            };
            let operation = if boundary == 1 {
                if unmap { "unmap_gpu" } else { "map_gpu" }
            } else {
                "currentness"
            };
            if boundary == 1 {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.panic_currentness_at =
                    Some(engine.backend.currentness_calls + if boundary == 0 { 1 } else { 2 });
            }
            let before = usage(&engine);
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(lease) = unmapped {
                    engine.map_device_memory(lease).map(drop)
                } else {
                    engine.unmap_device_memory(mapped.unwrap()).map(drop)
                }
            }))
            .expect_err("native mapping panic must preserve its payload");
            assert_eq!(
                payload.downcast_ref::<(&'static str, &'static str)>(),
                Some(&("N2 native panic", operation))
            );
            assert_eq!(usage(&engine), before);
            assert_debit(&engine, 4096, 1);
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(
                engine.device_memory[0].phase,
                if boundary == 0 {
                    if unmap {
                        DeviceMemoryPhaseV1::Mapped
                    } else {
                        DeviceMemoryPhaseV1::Unmapped
                    }
                } else {
                    DeviceMemoryPhaseV1::Ambiguous
                }
            );
            let calls = (
                engine.backend.currentness_calls,
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls,
                engine.backend.map_gpu_calls,
                engine.backend.unmap_gpu_calls,
            );
            engine.backend.panic_operation = None;
            engine.backend.panic_currentness_at = None;
            assert!(matches!(
                engine.allocate_device_memory(device, vm, 4096, 4096),
                Err(MemorySessionError::SharedSessionQuarantined)
            ));
            assert_eq!(
                (
                    engine.backend.currentness_calls,
                    engine.backend.reserve_va_calls,
                    engine.backend.alloc_calls,
                    engine.backend.map_gpu_calls,
                    engine.backend.unmap_gpu_calls
                ),
                calls
            );
            assert_eq!(engine.backend.free_calls, 0);
            assert_eq!(engine.backend.release_va_calls, 0);
        }
    }
}

#[test]
fn n2_initialization_unwind_retains_mapping_and_rejects_new_native_work() {
    let (device, vm) = device_vm(1);
    for operation in [
        "pre_currentness",
        "map_cpu",
        "prepare_cpu_mapping",
        "with_bytes_mut",
        "with_bytes",
        "unmap_cpu",
        "post_currentness",
        "map_gpu",
        "gpu_post_currentness",
    ] {
        let mut engine = configured(8192, 2);
        let bytes = vec![0x5a; 4096];
        let descriptor = content(&bytes);
        let source = validate_initialization_source(bytes.into_boxed_slice(), descriptor).unwrap();
        let lease = engine
            .allocate_device_memory_with_flags(
                device,
                vm,
                4096,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        let expected_operation = match operation {
            "pre_currentness" => {
                engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 1);
                "currentness"
            }
            "post_currentness" => {
                engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 2);
                "currentness"
            }
            "gpu_post_currentness" => {
                engine.backend.panic_currentness_at = Some(engine.backend.currentness_calls + 4);
                "currentness"
            }
            native => {
                engine.backend.panic_operation = Some(native);
                native
            }
        };
        let before = usage(&engine);
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.initialize_public_device_memory(lease, source)
        }))
        .expect_err("configured initialization unwind must escape");
        assert_eq!(
            payload.downcast_ref::<(&'static str, &'static str)>(),
            Some(&("N2 native panic", expected_operation))
        );
        assert_eq!(usage(&engine), before);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(
            engine.device_memory[0].mapping.is_some(),
            matches!(
                operation,
                "prepare_cpu_mapping" | "with_bytes_mut" | "with_bytes" | "unmap_cpu"
            )
        );
        let calls = (
            engine.backend.currentness_calls,
            engine.backend.reserve_va_calls,
            engine.backend.alloc_calls,
            engine.backend.map_cpu_calls,
            engine.backend.map_gpu_calls,
        );
        engine.backend.panic_operation = None;
        engine.backend.panic_currentness_at = None;
        assert!(matches!(
            engine.allocate_device_memory(device, vm, 4096, 4096),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!(
            (
                engine.backend.currentness_calls,
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls,
                engine.backend.map_cpu_calls,
                engine.backend.map_gpu_calls
            ),
            calls
        );
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }
}

#[test]
fn n2_borrowed_public_access_unwind_cannot_replace_retained_cpu_mapping() {
    let (device, vm) = device_vm(1);
    for write in [false, true] {
        for operation in [
            "pre_currentness",
            "map_cpu",
            "prepare_cpu_mapping",
            "with_bytes_mut",
            "access",
            "unmap_cpu",
            "post_currentness",
        ] {
            let mut engine = configured(8192, 2);
            let lease = engine
                .allocate_device_memory_with_flags(
                    device,
                    vm,
                    4096,
                    4096,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            let expected_operation = match operation {
                "pre_currentness" => {
                    engine.backend.panic_currentness_at =
                        Some(engine.backend.currentness_calls + 1);
                    "currentness"
                }
                "post_currentness" => {
                    engine.backend.panic_currentness_at =
                        Some(engine.backend.currentness_calls + 2);
                    "currentness"
                }
                native => {
                    engine.backend.panic_operation = Some(native);
                    native
                }
            };
            let before = usage(&engine);
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.with_unmapped_public_device_memory(&lease, |bytes| {
                    if write {
                        bytes[0] = 0xa5;
                    }
                    if operation == "access" {
                        std::panic::panic_any(("N2 native panic", "access"));
                    }
                    bytes[0]
                })
            }))
            .expect_err("configured borrowed access unwind must escape");
            assert_eq!(
                payload.downcast_ref::<(&'static str, &'static str)>(),
                Some(&("N2 native panic", expected_operation))
            );
            assert_eq!(usage(&engine), before);
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
            let retained_mapping = matches!(
                operation,
                "prepare_cpu_mapping" | "with_bytes_mut" | "access" | "unmap_cpu"
            );
            assert_eq!(engine.device_memory[0].mapping.is_some(), retained_mapping);
            let calls = (
                engine.backend.currentness_calls,
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls,
                engine.backend.map_cpu_calls,
                engine.backend.operations.len(),
            );
            engine.backend.panic_operation = None;
            engine.backend.panic_currentness_at = None;
            assert!(matches!(
                engine.with_unmapped_public_device_memory(&lease, |_| ()),
                Err(MemorySessionError::SharedSessionQuarantined)
            ));
            assert!(matches!(
                engine.allocate_device_memory(device, vm, 4096, 4096),
                Err(MemorySessionError::SharedSessionQuarantined)
            ));
            assert_eq!(
                (
                    engine.backend.currentness_calls,
                    engine.backend.reserve_va_calls,
                    engine.backend.alloc_calls,
                    engine.backend.map_cpu_calls,
                    engine.backend.operations.len()
                ),
                calls
            );
            assert_eq!(engine.device_memory[0].mapping.is_some(), retained_mapping);
            assert_eq!(engine.backend.free_calls, 0);
            assert_eq!(engine.backend.release_va_calls, 0);
        }
    }
}
