//! Real record/account and model-loan transitions with a fake native backend.
//! These tests do not execute the Linux queue facade or qualify GPU behavior.
use super::*;
use crate::sdma::pool_policy::{
    DevicePoolDispositionV1, DevicePoolPolicyErrorV1, device_pool_recycle_decision_with_v1,
    device_pool_usage_with_v1,
};
use crate::sdma::{
    Gfx942DevicePoolLimitsV1, Gfx942DevicePoolUsageV1, Gfx942SdmaBufferStorageV1,
    Gfx942SdmaBufferV1,
};
use fe2o3_runtime_model::{QueueGenerationV1, QueueInstanceIdV1, QueueKeyV1};

fn owner(fixture: &BackingConstructorFixture) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: fixture.vm,
        id: QueueInstanceIdV1(31),
        generation: QueueGenerationV1(1),
    }
}

fn mapped(
    fixture: &mut BackingConstructorFixture,
    bytes: u64,
    alignment: u64,
) -> Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1> {
    let lease = fixture
        .engine
        .allocate_device_memory(fixture.device.model_key(), fixture.vm, bytes, alignment)
        .unwrap();
    fixture.engine.map_device_memory(lease).unwrap()
}

fn buffer(fixture: &mut BackingConstructorFixture, bytes: u64) -> Gfx942SdmaBufferV1 {
    let lease = mapped(fixture, bytes, 4);
    Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        owner(fixture),
        1,
        bytes,
    )
}

fn usage(
    fixture: &BackingConstructorFixture,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
) -> Result<Gfx942DevicePoolUsageV1, DevicePoolPolicyErrorV1> {
    device_pool_usage_with_v1(owner(fixture), limits, cached, &mut |lease| {
        fixture
            .engine
            .device_pool_backing_bytes_v1(lease, fixture.device.model_key(), fixture.vm)
    })
}

fn decision(
    fixture: &BackingConstructorFixture,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
) -> Result<DevicePoolDispositionV1, DevicePoolPolicyErrorV1> {
    device_pool_recycle_decision_with_v1(owner(fixture), limits, cached, candidate, &mut |lease| {
        fixture
            .engine
            .device_pool_backing_bytes_v1(lease, fixture.device.model_key(), fixture.vm)
    })
}

fn dispose(
    fixture: &mut BackingConstructorFixture,
    buffer: Gfx942SdmaBufferV1,
) -> Result<(), MemorySessionError> {
    let (Gfx942SdmaBufferStorageV1::Device(lease), _, _, _) = buffer.into_bridge_parts() else {
        panic!("device-only test input")
    };
    let lease = fixture.engine.unmap_device_memory(lease)?;
    fixture.engine.release_device_memory(lease)
}

fn native_calls(
    fixture: &BackingConstructorFixture,
) -> (usize, usize, usize, usize, usize, usize, usize) {
    let backend = &fixture.engine.backend;
    (
        backend.currentness_calls,
        backend.reserve_va_calls,
        backend.alloc_calls,
        backend.map_gpu_calls,
        backend.unmap_gpu_calls,
        backend.free_calls,
        backend.release_va_calls,
    )
}

#[test]
fn device_pool_exact_projection_is_padded_inert_and_independent_of_n2_configuration() {
    for configured in [false, true] {
        let budget = configured.then(|| Gfx942DeviceBackingBudgetV1::new(16384, 3).unwrap());
        let mut fixture = BackingConstructorFixture::new(budget);
        let lease = mapped(&mut fixture, 4100, 4096);
        assert_eq!(lease.layout().requested_bytes(), 4100);
        assert_eq!(lease.layout().backing_bytes(), 8192);
        let before_calls = native_calls(&fixture);
        let before_usage = fixture.usage();
        for _ in 0..3 {
            assert_eq!(
                fixture
                    .engine
                    .device_pool_backing_bytes_v1(&lease, fixture.device.model_key(), fixture.vm)
                    .unwrap(),
                8192
            );
        }
        assert_eq!(native_calls(&fixture), before_calls);
        assert_eq!(fixture.usage(), before_usage);
        let candidate = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            owner(&fixture),
            1,
            4100,
        );
        let small = Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap();
        assert_eq!(
            decision(&fixture, small, &[], &candidate),
            Ok(DevicePoolDispositionV1::Dispose)
        );
        let sufficient = Gfx942DevicePoolLimitsV1::new(8192, 1).unwrap();
        assert_eq!(
            decision(&fixture, sufficient, &[], &candidate),
            Ok(DevicePoolDispositionV1::Cache)
        );
        assert_eq!(native_calls(&fixture), before_calls);
        assert_eq!(fixture.usage(), before_usage);
        dispose(&mut fixture, candidate).unwrap();
        if configured {
            assert_eq!(fixture.usage().unwrap().used_backing_bytes, 0);
        } else {
            assert!(fixture.usage().is_none());
        }
    }
}

#[test]
fn device_pool_projection_rejects_each_substituted_or_uncertain_record_without_native_effects() {
    for mutation in 0..16 {
        let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
        let mut fixture = BackingConstructorFixture::new(Some(budget));
        let mut lease = mapped(&mut fixture, 17, 4);
        let mut expected_device = fixture.device.model_key();
        let mut expected_vm = fixture.vm;
        let mut retained_charge = None;
        match mutation {
            0 => lease.id += 1,
            1 => lease.generation += 1,
            2 => lease.device.generation.0 += 1,
            3 => lease.vm.id.0 += 1,
            4 => lease.layout.requested_bytes += 1,
            5 => expected_device.generation.0 += 1,
            6 => expected_vm.id.0 += 1,
            7 => fixture.engine.device_memory[0].phase = DeviceMemoryPhaseV1::Unmapped,
            8 => fixture.engine.device_memory[0].phase = DeviceMemoryPhaseV1::Ambiguous,
            9 => fixture.engine.device_memory[0].handle = None,
            10 => fixture.engine.device_memory[0].reservation = None,
            11 => fixture.engine.device_memory[0].free_attempted = true,
            12 => retained_charge = fixture.engine.device_memory[0].backing_charge.take(),
            13 => {
                fixture.engine.device_backing_account = Some(
                    DeviceBackingAccountV1::new(
                        fixture.engine.session_id,
                        fixture.device.model_key(),
                        fixture.vm,
                        budget,
                    )
                    .unwrap(),
                );
            }
            14 => fixture.engine.session_id += 1,
            15 => fixture.engine.phase = SharedMemorySessionPhaseV1::Quarantined,
            _ => unreachable!(),
        }
        let before_calls = native_calls(&fixture);
        let before_usage = fixture.usage();
        for _ in 0..2 {
            assert!(
                fixture
                    .engine
                    .device_pool_backing_bytes_v1(&lease, expected_device, expected_vm,)
                    .is_err(),
                "mutation {mutation}"
            );
        }
        assert_eq!(native_calls(&fixture), before_calls);
        assert_eq!(fixture.usage(), before_usage);
        drop(retained_charge);
    }
}

#[test]
fn device_pool_both_startup_orders_keep_exact_n2_charge_across_model_loans_reuse_and_disposal() {
    let budget = Gfx942DeviceBackingBudgetV1::new(24576, 4).unwrap();
    let limits = Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap();
    for compute_first in [false, true] {
        let mut fixture = BackingConstructorFixture::new(Some(budget));
        let compute = compute_first.then(|| fixture.mapped_device());
        let mut queue = match compute.as_ref() {
            Some(compute) => fixture.transfer(&[compute]).unwrap(),
            None => fixture.transfer(&[]).unwrap(),
        };
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
        let mut first = buffer(&mut fixture, 17);
        let identity = first.storage_identity();
        let charged = fixture.usage().unwrap();
        assert_eq!(
            decision(&fixture, limits, &[], &first),
            Ok(DevicePoolDispositionV1::Cache)
        );
        first.advance_pool_generation().unwrap();
        let mut cached = vec![first];
        assert_eq!(
            usage(&fixture, limits, &cached)
                .unwrap()
                .cached_backing_bytes,
            4096
        );
        assert_eq!(usage(&fixture, limits, &cached).unwrap().cached_buffers, 1);
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
        assert_eq!(fixture.usage(), Some(charged));
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

        let mut reused = cached.pop().unwrap();
        reused.set_logical_bytes(8);
        assert_eq!(reused.storage_identity(), identity);
        assert_eq!(reused.pool_generation(), 2);
        assert_eq!(
            usage(&fixture, limits, &cached)
                .unwrap()
                .cached_backing_bytes,
            0
        );
        assert_eq!(fixture.usage(), Some(charged));
        assert_eq!(
            decision(&fixture, limits, &cached, &reused),
            Ok(DevicePoolDispositionV1::Cache)
        );
        reused.advance_pool_generation().unwrap();
        assert_eq!(reused.pool_generation(), 3);
        cached.push(reused);
        assert_eq!(fixture.usage(), Some(charged));

        let incoming = buffer(&mut fixture, 4100);
        assert_eq!(
            decision(&fixture, limits, &cached, &incoming),
            Ok(DevicePoolDispositionV1::Dispose)
        );
        assert_eq!(
            fixture.usage().unwrap().used_backing_bytes,
            charged.used_backing_bytes + 8192
        );
        dispose(&mut fixture, incoming).unwrap();
        assert_eq!(fixture.usage(), Some(charged));
        dispose(&mut fixture, cached.pop().unwrap()).unwrap();
        assert_eq!(usage(&fixture, limits, &cached).unwrap().cached_buffers, 0);
        let remaining = if compute_first { 4096 } else { 0 };
        assert_eq!(fixture.usage().unwrap().used_backing_bytes, remaining);
        assert_eq!(fixture.engine.backend.free_calls, 2);
        assert_eq!(fixture.engine.backend.release_va_calls, 2);
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
        if let Some(compute) = compute {
            let lease = fixture
                .engine
                .unmap_device_memory(compute.into_lease())
                .unwrap();
            fixture.engine.release_device_memory(lease).unwrap();
        }
        assert_eq!(fixture.usage().unwrap().used_backing_bytes, 0);
        assert_eq!(fixture.usage().unwrap().used_allocation_records, 0);
    }
}

#[test]
fn device_pool_rejected_disposal_retains_n2_charge_and_prevents_reuse() {
    for panic in [false, true] {
        for operation in [
            "unmap_gpu",
            "free",
            "release_va_reservation",
            "final_currentness",
        ] {
            let mut fixture = BackingConstructorFixture::new(Some(
                Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap(),
            ));
            let incoming = buffer(&mut fixture, 17);
            let limits = Gfx942DevicePoolLimitsV1::new(0, 0).unwrap();
            assert_eq!(
                decision(&fixture, limits, &[], &incoming),
                Ok(DevicePoolDispositionV1::Dispose)
            );
            let charged = fixture.usage().unwrap();
            if operation == "final_currentness" {
                // unmap: pre/post; release: pre/post-free/post-VA.
                let final_call = fixture.engine.backend.currentness_calls + 5;
                if panic {
                    fixture.engine.backend.panic_currentness_at = Some(final_call);
                } else {
                    fixture.engine.backend.fail_currentness_at = Some(final_call);
                }
            } else if panic {
                fixture.engine.backend.panic_operation = Some(operation);
            } else {
                fixture.engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispose(&mut fixture, incoming)
            }));
            if panic {
                let payload = result.expect_err("native panic must be preserved");
                let expected = if operation == "final_currentness" {
                    "currentness"
                } else {
                    operation
                };
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", expected))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(
                fixture.engine.phase(),
                SharedMemorySessionPhaseV1::Quarantined
            );
            assert_eq!(fixture.usage(), Some(charged));
            let calls = native_calls(&fixture);
            assert!(
                fixture
                    .engine
                    .allocate_device_memory(fixture.device.model_key(), fixture.vm, 17, 4,)
                    .is_err()
            );
            assert_eq!(native_calls(&fixture), calls);
        }
    }
}
