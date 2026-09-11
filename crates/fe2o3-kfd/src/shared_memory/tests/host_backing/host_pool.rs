//! Production policy and native record checks with a fake backend, not GPU acceptance.
use super::*;
use crate::sdma::host_pool_policy::{
    Gfx942HostPoolLimitsV1, Gfx942HostPoolUsageV1, HostPoolDispositionV1, HostPoolPolicyErrorV1,
    host_pool_recycle_decision_with_v1, host_pool_usage_with_v1,
};

fn owner() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: device_vm(1).1,
        id: QueueInstanceIdV1(33),
        generation: QueueGenerationV1(1),
    }
}

fn limits(bytes: u64, records: usize) -> Gfx942HostPoolLimitsV1 {
    Gfx942HostPoolLimitsV1::new(bytes, records).unwrap()
}

fn buffer(engine: &mut SharedMemoryEngine<FakeBackend>, bytes: usize) -> Gfx942SdmaBufferV1 {
    Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Host(mapped(engine, bytes)),
        owner(),
        1,
        bytes as u64,
    )
}

fn forged_token(engine: &SharedMemoryEngine<FakeBackend>, index: usize) -> HostMapped {
    let record = &engine.allocations[index];
    HostMapped {
        session_id: engine.session_id,
        id: record.id,
        generation: record.generation,
        layout: record.layout,
        marker: PhantomData,
    }
}

fn forged_buffer(
    engine: &SharedMemoryEngine<FakeBackend>,
    index: usize,
    pool_generation: u64,
) -> Gfx942SdmaBufferV1 {
    let token = forged_token(engine, index);
    let bytes = token.layout.requested_bytes as u64;
    Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Host(token),
        owner(),
        pool_generation,
        bytes,
    )
}

fn occupancy(
    engine: &SharedMemoryEngine<FakeBackend>,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
) -> Result<Gfx942HostPoolUsageV1, HostPoolPolicyErrorV1> {
    let (device, vm) = device_vm(1);
    engine
        .validate_host_pool_domain_v1(device, vm)
        .map_err(|_| HostPoolPolicyErrorV1::InvalidRoster)?;
    host_pool_usage_with_v1(owner(), limits, cached, &mut |token| {
        engine.host_pool_backing_bytes_v1(token, device, vm)
    })
}

fn decision(
    engine: &SharedMemoryEngine<FakeBackend>,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
) -> Result<HostPoolDispositionV1, HostPoolPolicyErrorV1> {
    let (device, vm) = device_vm(1);
    engine
        .validate_host_pool_domain_v1(device, vm)
        .map_err(|_| HostPoolPolicyErrorV1::InvalidRoster)?;
    host_pool_recycle_decision_with_v1(owner(), limits, cached, candidate, &mut |token| {
        engine.host_pool_backing_bytes_v1(token, device, vm)
    })
}

fn dispose(
    engine: &mut SharedMemoryEngine<FakeBackend>,
    buffer: Gfx942SdmaBufferV1,
) -> Result<(), MemorySessionError> {
    let (Gfx942SdmaBufferStorageV1::Host(token), _, _, _) = buffer.into_bridge_parts() else {
        panic!("host fixture");
    };
    let token = engine.unmap_mutable(token)?;
    engine.release(token, SharedAllocationPhaseV1::CpuWritable)
}

#[test]
fn host_pool_limits_accept_zero_and_reject_outside_native_envelope() {
    let maximum = limits(8 << 30, 256);
    assert_eq!(maximum.max_cached_backing_bytes(), 8 << 30);
    assert_eq!(maximum.max_cached_buffers(), 256);
    assert!(Gfx942HostPoolLimitsV1::new((8 << 30) + 1, 256).is_none());
    assert!(Gfx942HostPoolLimitsV1::new(0, 257).is_none());
    assert!(Gfx942HostPoolLimitsV1::new(u64::MAX, usize::MAX).is_none());
    assert_eq!(limits(0, 0).max_cached_buffers(), 0);
}

#[test]
fn host_pool_padded_cost_is_inert_with_or_without_n1_and_pressure_disposes() {
    for configured_n1 in [false, true] {
        let mut engine = if configured_n1 {
            configured(16384, 3)
        } else {
            acquired()
        };
        let candidate = buffer(&mut engine, 4097);
        assert_eq!(candidate.physical_bytes(), 4097);
        let before_calls = calls(&engine);
        let before_debit = engine
            .host_backing_account
            .as_ref()
            .map(HostBackingAccountV1::usage);
        for _ in 0..3 {
            assert_eq!(
                decision(&engine, limits(8191, 1), &[], &candidate),
                Ok(HostPoolDispositionV1::Dispose)
            );
            assert_eq!(
                decision(&engine, limits(8192, 1), &[], &candidate),
                Ok(HostPoolDispositionV1::Cache)
            );
        }
        assert_eq!(calls(&engine), before_calls);
        assert_eq!(
            engine
                .host_backing_account
                .as_ref()
                .map(HostBackingAccountV1::usage),
            before_debit
        );
        let cached = vec![candidate];
        assert_eq!(
            occupancy(&engine, limits(8192, 1), &cached)
                .unwrap()
                .cached_backing_bytes,
            8192
        );
        let candidate = cached.into_iter().next().unwrap();
        dispose(&mut engine, candidate).unwrap();
        if configured_n1 {
            debit(&engine, 0, 0);
        }
        assert_eq!(engine.backend.free_calls, 1);
    }
}

#[test]
fn host_pool_byte_record_and_zero_ceilings_are_independent() {
    let mut engine = configured(16384, 4);
    let cached = vec![buffer(&mut engine, 1)];
    let candidate = buffer(&mut engine, 4097);
    for (bytes, records, expected) in [
        (12288, 2, HostPoolDispositionV1::Cache),
        (12287, 2, HostPoolDispositionV1::Dispose),
        (12288, 1, HostPoolDispositionV1::Dispose),
    ] {
        assert_eq!(
            decision(&engine, limits(bytes, records), &cached, &candidate),
            Ok(expected)
        );
    }
    for limit in [limits(0, 0), limits(0, 256), limits(8 << 30, 0)] {
        assert_eq!(
            decision(&engine, limit, &[], &candidate),
            Ok(HostPoolDispositionV1::Dispose)
        );
        assert_eq!(occupancy(&engine, limit, &[]).unwrap().cached_buffers, 0);
        assert_eq!(
            decision(&engine, limit, &cached, &candidate),
            Err(HostPoolPolicyErrorV1::InvalidRoster)
        );
    }
    debit(&engine, 12288, 2);
}

#[test]
fn host_pool_full_roster_checks_last_ordinal_and_generation_independent_aliases() {
    let mut engine = configured(256 * 4096, 256);
    let mut cached: Vec<_> = (0..256).map(|_| buffer(&mut engine, 1)).collect();
    let limit = limits(256 * 4096, 256);
    assert_eq!(
        occupancy(&engine, limit, &cached).unwrap().cached_buffers,
        256
    );
    let last = cached.pop().unwrap();
    let mut alias = forged_buffer(&engine, 0, 1);
    alias.advance_pool_generation().unwrap();
    cached.push(alias);
    assert_eq!(
        occupancy(&engine, limit, &cached),
        Err(HostPoolPolicyErrorV1::InvalidRoster)
    );
    cached.pop();
    let valid_alias = forged_buffer(&engine, 0, 9);
    assert_eq!(
        decision(&engine, limits(255 * 4096, 255), &cached, &valid_alias),
        Err(HostPoolPolicyErrorV1::InvalidCandidate)
    );
    let mut token = forged_token(&engine, 0);
    token.generation += 1;
    let alias = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Host(token),
        owner(),
        1,
        1,
    );
    assert_eq!(
        decision(&engine, limit, &cached, &alias),
        Err(HostPoolPolicyErrorV1::InvalidCandidate)
    );
    cached.push(last);
    let extra = forged_buffer(&engine, 0, 1);
    cached.push(extra);
    assert_eq!(
        occupancy(&engine, limit, &cached),
        Err(HostPoolPolicyErrorV1::InvalidRoster)
    );
    debit(&engine, 256 * 4096, 256);
}

#[test]
fn host_pool_invalid_candidate_never_becomes_pressure_disposal() {
    for mutation in 0..6 {
        let mut engine = configured(8192, 2);
        let mut token = mapped(&mut engine, 17);
        let mut candidate_owner = owner();
        let mut pool_generation = 1;
        let mut bytes = 17;
        match mutation {
            0 => candidate_owner.generation.0 += 1,
            1 => pool_generation = 0,
            2 => bytes = 0,
            3 => bytes = 18,
            4 => token.generation += 1,
            5 => token.session_id += 1,
            _ => unreachable!(),
        }
        let candidate = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(token),
            candidate_owner,
            pool_generation,
            bytes,
        );
        let before = calls(&engine);
        assert_eq!(
            decision(&engine, limits(0, 0), &[], &candidate),
            Err(HostPoolPolicyErrorV1::InvalidCandidate)
        );
        assert_eq!(calls(&engine), before);
        debit(&engine, 4096, 1);
    }
}

#[test]
fn host_pool_exact_record_projection_rejects_substitutions_without_native_effects() {
    for mutation in 0..22 {
        let mut engine = configured(8192, 2);
        let mut token = mapped(&mut engine, 17);
        let (mut device, mut vm) = device_vm(1);
        let mut retained_charge = None;
        match mutation {
            0 => token.session_id += 1,
            1 => token.id += 1,
            2 => token.generation += 1,
            3 => token.layout.requested_bytes += 1,
            4 => token.layout.cpu_mapping_bytes *= 2,
            5 => token.layout.gpu_va_bytes *= 2,
            6 => token.layout.uapi_flags ^= 1,
            7 => engine.allocations[0].profile = SharedGttProfileV1::Kernarg,
            8 => engine.allocations[0].userptr = true,
            9 => engine.allocations[0].phase = SharedAllocationPhaseV1::CpuWritable,
            10 => engine.allocations[0].mapping = None,
            11 => engine.allocations[0].handle = None,
            12 => engine.allocations[0].reservation = None,
            13 => engine.allocations[0].free_attempted = true,
            14 => retained_charge = engine.allocations[0].host_backing_charge.take(),
            15 => {
                engine.host_backing_account = Some(
                    HostBackingAccountV1::new(engine.session_id, device, vm, budget(8192, 2))
                        .unwrap(),
                )
            }
            16 => engine.session_id += 1,
            17 => engine.phase = SharedMemorySessionPhaseV1::Quarantined,
            18 => device.generation.0 += 1,
            19 => vm.id.0 += 1,
            20 => engine.host_backing_account = None,
            21 => {
                token.layout.profile = SharedGttProfileV1::Kernarg;
                engine.allocations[0].layout = token.layout;
            }
            _ => unreachable!(),
        }
        let before = calls(&engine);
        assert!(
            engine
                .host_pool_backing_bytes_v1(&token, device, vm)
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(calls(&engine), before);
        drop(retained_charge);
    }
}

#[test]
fn host_pool_unconfigured_record_still_requires_canonical_layout() {
    for mutation in 0..3 {
        let mut engine = acquired();
        let mut token = mapped(&mut engine, 17);
        match mutation {
            0 => token.layout.cpu_mapping_bytes *= 2,
            1 => token.layout.gpu_va_bytes *= 2,
            2 => token.layout.uapi_flags ^= 1,
            _ => unreachable!(),
        }
        engine.allocations[0].layout = token.layout;
        let (device, vm) = device_vm(1);
        assert!(
            engine
                .host_pool_backing_bytes_v1(&token, device, vm)
                .is_err()
        );
    }
}

#[test]
fn host_pool_empty_roster_rejects_foreign_session_account_and_invalid_domain() {
    for mutation in 0..6 {
        let mut engine = configured(8192, 2);
        let (mut device, mut vm) = device_vm(1);
        match mutation {
            0 => {
                engine.host_backing_account = Some(
                    HostBackingAccountV1::new(engine.session_id + 1, device, vm, budget(8192, 2))
                        .unwrap(),
                )
            }
            1 => engine.session_id = 0,
            2 => device.generation.0 = 0,
            3 => vm.id.0 = 0,
            4 => vm.device.generation.0 += 1,
            5 => engine.phase = SharedMemorySessionPhaseV1::Quarantined,
            _ => unreachable!(),
        }
        let before = calls(&engine);
        assert!(engine.validate_host_pool_domain_v1(device, vm).is_err());
        assert_eq!(calls(&engine), before);
    }
}

#[test]
fn host_pool_mixed_roster_excludes_device_cost_but_bounds_shape() {
    let mut engine = configured(8192, 2);
    let (device, vm) = device_vm(1);
    let lease = engine.allocate_device_memory(device, vm, 17, 4).unwrap();
    let lease = engine.map_device_memory(lease).unwrap();
    let device_buffer = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        owner(),
        1,
        17,
    );
    let mut cached = vec![device_buffer, buffer(&mut engine, 17)];
    assert_eq!(
        occupancy(&engine, limits(4096, 1), &cached)
            .unwrap()
            .cached_buffers,
        1
    );
    assert_eq!(
        decision(&engine, limits(4096, 1), &[], &cached[0]),
        Err(HostPoolPolicyErrorV1::InvalidCandidate)
    );
    let (storage, mut candidate_owner, generation, bytes) =
        cached.swap_remove(0).into_bridge_parts();
    candidate_owner.generation.0 += 1;
    cached.push(Gfx942SdmaBufferV1::from_bridge_parts(
        storage,
        candidate_owner,
        generation,
        bytes,
    ));
    assert_eq!(
        occupancy(&engine, limits(4096, 1), &cached),
        Err(HostPoolPolicyErrorV1::InvalidRoster)
    );
}

#[test]
fn host_pool_checkout_and_recycle_retain_native_identity_and_n1_debit() {
    let mut engine = configured(12288, 3);
    let mut first = buffer(&mut engine, 4097);
    let identity = first.storage_identity();
    let before = usage(&engine);
    let limit = limits(8192, 1);
    assert_eq!(
        decision(&engine, limit, &[], &first),
        Ok(HostPoolDispositionV1::Cache)
    );
    first.advance_pool_generation().unwrap();
    let mut cached = vec![first];
    assert_eq!(
        occupancy(&engine, limit, &cached)
            .unwrap()
            .cached_backing_bytes,
        8192
    );
    let mut checked_out = cached.pop().unwrap();
    checked_out.set_logical_bytes(17);
    assert_eq!(
        occupancy(&engine, limit, &cached).unwrap().cached_buffers,
        0
    );
    assert_eq!(usage(&engine), before);
    assert_eq!(checked_out.storage_identity(), identity);
    assert_eq!(
        decision(&engine, limit, &cached, &checked_out),
        Ok(HostPoolDispositionV1::Cache)
    );
    checked_out.advance_pool_generation().unwrap();
    cached.push(checked_out);
    assert_eq!(
        occupancy(&engine, limit, &cached)
            .unwrap()
            .cached_backing_bytes,
        8192
    );
    let incoming = buffer(&mut engine, 17);
    assert_eq!(
        decision(&engine, limit, &cached, &incoming),
        Ok(HostPoolDispositionV1::Dispose)
    );
    dispose(&mut engine, incoming).unwrap();
    assert_eq!(usage(&engine), before);
    dispose(&mut engine, cached.pop().unwrap()).unwrap();
    debit(&engine, 0, 0);
}

#[test]
fn host_pool_pressure_and_partial_trim_keep_every_uncertain_disposal_charge() {
    for panic in [false, true] {
        for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
            let mut engine = configured(12288, 3);
            let first = buffer(&mut engine, 17);
            let candidate = buffer(&mut engine, 17);
            let later = buffer(&mut engine, 17);
            dispose(&mut engine, first).unwrap();
            debit(&engine, 8192, 2);
            assert_eq!(
                decision(&engine, limits(0, 0), &[], &candidate),
                Ok(HostPoolDispositionV1::Dispose)
            );
            if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispose(&mut engine, candidate)
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert!(outcome.unwrap().is_err());
            }
            debit(&engine, 8192, 2);
            assert_eq!(engine.phase, SharedMemorySessionPhaseV1::Quarantined);
            let before = calls(&engine);
            assert_eq!(
                decision(&engine, limits(8192, 2), &[], &later),
                Err(HostPoolPolicyErrorV1::InvalidRoster)
            );
            assert_eq!(calls(&engine), before);
        }
    }
}

#[test]
fn host_pool_disposal_currentness_errors_and_panics_never_refund() {
    // GPU unmap has two checks; CPU unmap/free/VA release adds four checks.
    for panic in [false, true] {
        for boundary in 1..=6 {
            let mut engine = configured(8192, 2);
            let candidate = buffer(&mut engine, 17);
            assert_eq!(
                decision(&engine, limits(0, 0), &[], &candidate),
                Ok(HostPoolDispositionV1::Dispose)
            );
            let at = engine.backend.currentness_calls + boundary;
            if panic {
                engine.backend.panic_currentness_at = Some(at);
            } else {
                engine.backend.fail_currentness_at = Some(at);
            }
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispose(&mut engine, candidate)
            }));
            if panic {
                assert!(outcome.is_err(), "boundary {boundary}");
            } else {
                assert!(outcome.unwrap().is_err(), "boundary {boundary}");
            }
            debit(&engine, 4096, 1);
            closed(&mut engine);
        }
    }
}
