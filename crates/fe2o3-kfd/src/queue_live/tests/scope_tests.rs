use super::*;

#[test]
fn retained_device_scope_requires_exact_live_queue_phase_and_unpoisoned_owners() {
    use super::super::ComputeAqlQueuePhaseV1::*;
    for phase in [
        None,
        Some(Planned),
        Some(CreatePending),
        Some(Active),
        Some(UpdatePending),
        Some(DisablePending),
        Some(Disabled),
        Some(DestroyPending),
        Some(CancelledBeforeCreate),
        Some(Destroyed),
        Some(Ambiguous),
    ] {
        for terminal in [false, true] {
            for poisoned in [false, true] {
                assert_eq!(
                    super::retained_device_queue_is_active_v1(terminal, poisoned, phase),
                    !terminal && !poisoned && phase == Some(Active)
                );
            }
        }
    }
}

#[test]
fn device_pool_configuration_is_optional_immutable_and_history_is_irreversible() {
    let limits = Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap();
    let mut configuration = SdmaDevicePoolConfigurationV1::default();
    assert!(configuration.limits.is_none());
    configuration.configure(limits).unwrap();
    assert_eq!(configuration.limits, Some(limits));
    assert!(configuration.configure(limits).is_err());
    assert!(
        configuration
            .configure(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap())
            .is_err()
    );
    configuration.begin_activity();
    assert!(configuration.configure(limits).is_err());
    assert_eq!(configuration.limits, Some(limits));

    let mut configuration = SdmaDevicePoolConfigurationV1::default();
    configuration.begin_activity();
    configuration.begin_activity();
    assert!(configuration.configure(limits).is_err());
    assert!(configuration.limits.is_none());
}

#[test]
fn device_pool_every_sdma_resource_attempt_closes_configuration_even_on_rejection() {
    let queue = test_queue_key(801, 1);
    for attempt in 0..17 {
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        if attempt < 6 {
            // A deliberately unavailable native owner rejects enable before native entry.
            session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
        }
        match attempt {
            0 => assert!(session.enable_sdma_copy_engine().is_err()),
            1 => assert!(
                session
                    .enable_gfx942_directional_sdma_copy_engines()
                    .is_err()
            ),
            2 => assert!(
                session
                    .enable_gfx942_sdma_copy_engine_on_engine_index(0)
                    .is_err()
            ),
            3 => assert!(session.enable_gfx942_striped_sdma_copy_engines(2).is_err()),
            4 => assert!(
                session
                    .enable_gfx942_two_native_sdma_logical_mux_v2(2)
                    .is_err()
            ),
            5 => assert!(
                session
                    .enable_gfx942_directional_and_striped_sdma_copy_engines_v1(2)
                    .is_err()
            ),
            6 => assert!(session.allocate_sdma_host_buffer(16).is_err()),
            7 => assert!(session.allocate_sdma_device_buffer(16, 4).is_err()),
            8 => assert!(session.allocate_sdma_pooled_host_buffer(16).is_err()),
            9 => assert!(session.allocate_sdma_pooled_device_buffer(16, 0).is_err()),
            10 => assert!(
                session
                    .checkout_sdma_pool(
                        Gfx942SdmaBufferKindV1::DeviceLocal,
                        16,
                        4,
                        Some(fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL.bits()),
                    )
                    .is_err()
            ),
            11 => {
                let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 1);
                assert!(session.recycle_sdma_buffer(device).is_err());
            }
            12 => assert!(session.trim_sdma_memory_pool().is_err()),
            13 => {
                let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 1);
                assert!(session.release_sdma_buffer(device).is_err());
            }
            14 => assert!(
                session
                    .allocate_sdma_public_device_buffer_v1(16, 4)
                    .is_err()
            ),
            15 => assert!(
                session
                    .allocate_sdma_pooled_public_device_buffer_v1(16, 0)
                    .is_err()
            ),
            16 => assert!(
                session
                    .allocate_sdma_pooled_public_device_buffer_classified_v1(16, 0)
                    .is_err()
            ),
            _ => unreachable!(),
        }
        assert!(
            session.sdma_device_pool.activity_started,
            "attempt {attempt}"
        );
        assert!(
            session
                .sdma_device_pool
                .configure(Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap())
                .is_err()
        );
        assert!(session.sdma_device_pool.limits.is_none());
        assert!(
            session
                .configure_sdma_host_pool_v1(Gfx942HostPoolLimitsV1::new(4096, 1).unwrap())
                .is_err()
        );
        assert!(session.sdma_host_pool_limits.is_none());
        if matches!(attempt, 11 | 13) {
            assert!(session.terminal_poisoned && session.sdma_recycle.is_some());
            // These engine-less synthetic fixtures deliberately retain their roots.
            // Separate constructed-owner subprocess tests require terminal Drop abort.
            std::mem::forget(session);
        }
    }
}

#[test]
fn sdma_allocation_classified_pool_hit_preserves_owner_generation_and_legacy_behavior() {
    for host in [false, true] {
        for classified in [false, true] {
            let queue = test_queue_key(802, 1);
            let mut session = persistent_compute_cancellation_test_session(queue, None, None);
            // These public pool transitions have no Linux engine or native effect.
            session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
            let (device, host_buffer) = crate::sdma::persistent_sdma_buffers_for_test(queue, 2);
            let identity = if host {
                host_buffer.storage_identity()
            } else {
                device.storage_identity()
            };
            session.sdma_outstanding_buffers = 2;
            session.recycle_sdma_buffer(device).unwrap();
            session.recycle_sdma_buffer(host_buffer).unwrap();
            let recycled = match (host, classified) {
                (true, true) => session
                    .allocate_sdma_pooled_host_buffer_classified_v1(16)
                    .unwrap(),
                (false, true) => session
                    .allocate_sdma_pooled_device_buffer_classified_v1(16, 4)
                    .unwrap(),
                (true, false) => session.allocate_sdma_pooled_host_buffer(16).unwrap(),
                (false, false) => session.allocate_sdma_pooled_device_buffer(16, 4).unwrap(),
            };
            assert_eq!(recycled.storage_identity(), identity);
            assert_eq!(recycled.pool_generation(), 2);
            assert_eq!(recycled.requested_bytes(), 16);
            assert_eq!(session.sdma_outstanding_buffers, 1);
            assert_eq!(session.sdma_pool_reuse_count, 1);
            assert_eq!(session.sdma_pool_free.len(), 1);
            assert!(session.sdma_device_pool.activity_started && !session.terminal_poisoned);
            session.recycle_sdma_buffer(recycled).unwrap();
            assert_eq!(session.sdma_outstanding_buffers, 0);
            assert_eq!(session.sdma_pool_free.len(), 2);
        }
    }
}

#[test]
fn public_sdma_pool_matches_exact_flags_and_best_fit_without_relabeling() {
    for classified in [false, true] {
        let queue = test_queue_key(805, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
        let (private, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 2);
        let public = public_sdma_pool_buffer_for_test(queue, 3, 4096);
        let larger = public_sdma_pool_buffer_for_test(queue, 4, 8192);
        let private_id = private.storage_identity();
        let public_id = public.storage_identity();
        session.sdma_outstanding_buffers = 3;
        for buffer in [private, larger, public] {
            session.recycle_sdma_buffer(buffer).unwrap();
        }
        let public = if classified {
            session
                .allocate_sdma_pooled_public_device_buffer_classified_v1(16, 4)
                .unwrap()
        } else {
            session
                .allocate_sdma_pooled_public_device_buffer_v1(16, 4)
                .unwrap()
        };
        assert_eq!(public.storage_identity(), public_id);
        assert_eq!(
            public.device_allocation_flags_v1(),
            Some(fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC.bits())
        );
        let private = session.allocate_sdma_pooled_device_buffer(16, 4).unwrap();
        assert_eq!(private.storage_identity(), private_id);
        assert_eq!(
            private.device_allocation_flags_v1(),
            Some(fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL.bits())
        );
        for buffer in [public, private] {
            assert_eq!(buffer.pool_generation(), 2);
            assert_eq!(buffer.requested_bytes(), 16);
            session.recycle_sdma_buffer(buffer).unwrap();
        }
        assert_eq!(session.sdma_pool_reuse_count, 2);
        assert_eq!(session.sdma_outstanding_buffers, 0);
        assert_eq!(session.sdma_pool_free.len(), 3);
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn public_sdma_pool_miss_preserves_opposite_profile_owner_and_counters() {
    for request_public in [false, true] {
        let queue = test_queue_key(806, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
        let buffer = if request_public {
            crate::sdma::persistent_sdma_buffers_for_test(queue, 2).0
        } else {
            public_sdma_pool_buffer_for_test(queue, 3, 4096)
        };
        let identity = buffer.storage_identity();
        let flags = buffer.device_allocation_flags_v1();
        session.sdma_outstanding_buffers = 1;
        session.recycle_sdma_buffer(buffer).unwrap();
        let failure = if request_public {
            session.allocate_sdma_pooled_public_device_buffer_classified_v1(16, 4)
        } else {
            session.allocate_sdma_pooled_device_buffer_classified_v1(16, 4)
        }
        .err()
        .unwrap();
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
        ));
        assert_eq!(
            failure.disposition(),
            Gfx942SdmaAllocationDispositionV1::ProcessTeardown
        );
        assert_eq!(session.sdma_pool_free.len(), 1);
        assert_eq!(session.sdma_pool_free[0].storage_identity(), identity);
        assert_eq!(
            session.sdma_pool_free[0].device_allocation_flags_v1(),
            flags
        );
        assert_eq!(session.sdma_pool_free[0].pool_generation(), 2);
        assert_eq!(session.sdma_pool_reuse_count, 0);
        assert_eq!(session.sdma_outstanding_buffers, 0);
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn sdma_allocation_classified_pool_miss_keeps_original_error_and_legacy_state() {
    for host in [false, true] {
        for classified in [false, true] {
            let queue = test_queue_key(802, 1);
            let mut session = persistent_compute_cancellation_test_session(queue, None, None);
            session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
            let error = if classified {
                let failure = if host {
                    session.allocate_sdma_pooled_host_buffer_classified_v1(16)
                } else {
                    session.allocate_sdma_pooled_device_buffer_classified_v1(16, 4)
                }
                .err()
                .unwrap();
                assert_eq!(
                    failure.disposition(),
                    Gfx942SdmaAllocationDispositionV1::ProcessTeardown
                );
                failure.into_error()
            } else if host {
                session.allocate_sdma_pooled_host_buffer(16).err().unwrap()
            } else {
                session
                    .allocate_sdma_pooled_device_buffer(16, 4)
                    .err()
                    .unwrap()
            };
            assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
            ));
            assert!(session.sdma_device_pool.activity_started && !session.terminal_poisoned);
            assert!(session.sdma_pool_free.is_empty() && session.sdma_allocation.is_none());
            assert_eq!(session.sdma_outstanding_buffers, 0);
            assert_eq!(session.sdma_pool_reuse_count, 0);
        }
    }
}

#[test]
fn device_pool_default_preserves_recycle_checkout_and_host_cache_behavior() {
    let queue = test_queue_key(802, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    // No Linux owner is used: these legacy pool transitions require no native effect.
    session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
    let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 2);
    let identity = device.storage_identity();
    session.sdma_outstanding_buffers = 2;
    session.recycle_sdma_buffer(device).unwrap();
    session.recycle_sdma_buffer(host).unwrap();
    assert_eq!(session.sdma_device_pool_usage_v1().unwrap(), None);
    assert_eq!(session.sdma_host_pool_usage_v1().unwrap(), None);
    let recycled = session.allocate_sdma_pooled_device_buffer(16, 4).unwrap();
    assert_eq!(recycled.storage_identity(), identity);
    assert_eq!(recycled.pool_generation(), 2);
    assert_eq!(recycled.requested_bytes(), 16);
    assert_eq!(session.sdma_outstanding_buffers, 1);
    assert_eq!(session.sdma_pool_reuse_count, 1);
    assert_eq!(session.sdma_pool_free.len(), 1);
    session.sdma_device_pool.limits = Some(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap());
    let (_, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 3);
    session.sdma_outstanding_buffers += 1;
    session.recycle_sdma_buffer(host).unwrap();
    assert_eq!(session.sdma_pool_free.len(), 2);
    assert!(session.sdma_device_pool_usage_v1().is_err());
    // Missing native session cannot be mistaken for a configured zero usage result.
    assert!(!session.terminal_poisoned);
}

#[test]
fn host_pool_configured_missing_owner_and_terminal_observation_are_not_empty_usage() {
    let queue = test_queue_key(803, 1);
    let limits = Gfx942HostPoolLimitsV1::new(4096, 1).unwrap();
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    assert_eq!(session.sdma_host_pool_usage_v1().unwrap(), None);
    assert!(session.configure_sdma_host_pool_v1(limits).is_err());
    assert!(session.sdma_host_pool_limits.is_none());
    session.sdma_host_pool_limits = Some(limits);
    assert!(session.sdma_host_pool_usage_v1().is_err());
    assert!(session.configure_sdma_host_pool_v1(limits).is_err());
    session.terminal_poisoned = true;
    assert!(session.sdma_host_pool_usage_v1().is_err());
    assert!(session.configure_sdma_host_pool_v1(limits).is_err());
    assert_eq!(session.sdma_host_pool_limits, Some(limits));
}

#[test]
fn host_pool_invalid_native_roster_poison_precedes_checkout_or_trim_mutation() {
    for trim in [false, true] {
        let queue = test_queue_key(804, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
        let (_, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 3);
        session.sdma_pool_free.push(host);
        session.sdma_host_pool_limits = Some(Gfx942HostPoolLimitsV1::new(4096, 1).unwrap());
        if trim {
            assert!(session.trim_sdma_memory_pool().is_err());
        } else {
            assert!(
                session
                    .checkout_sdma_pool(Gfx942SdmaBufferKindV1::HostVisibleCoherent, 16, 1, None)
                    .is_err()
            );
        }
        assert!(session.terminal_poisoned);
        assert_eq!(session.sdma_pool_free.len(), 1);
        assert_eq!(session.sdma_outstanding_buffers, 0);
        assert_eq!(session.sdma_pool_reuse_count, 0);
    }
}

#[test]
fn host_pool_linux_wiring_uses_exact_domain_before_recycle_checkout_trim() {
    // Static wiring complements the fake native record tests, not Linux acceptance.
    let source = crate::queue::live_production_source_for_tests_v1();
    let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let config = production
        .split_once("pub fn configure_sdma_host_pool_v1(")
        .unwrap()
        .1
        .split_once("pub fn sdma_host_pool_usage_v1(")
        .unwrap()
        .0;
    assert!(config.contains("self.sdma_device_pool.activity_started"));
    assert!(config.contains("validate_host_pool_domain_v1(self.key.vm)"));
    assert!(!config.contains("begin_activity()"));
    let recycle = production
        .split_once("pub fn recycle_sdma_buffer(")
        .unwrap()
        .1
        .split_once("pub fn trim_sdma_memory_pool(")
        .unwrap()
        .0;
    assert!(recycle.contains("sdma_recycle::recycle_in_place(self, buffer, false)"));
    let recycle = include_str!("../../queue_live/sdma_recycle.rs");
    assert!(recycle.contains("host_pool_recycle_decision_v1("));
    assert!(recycle.contains("HostPoolDispositionV1::Dispose"));
    assert!(
        recycle.find("context.should_dispose()?").unwrap()
            < recycle.find("context.reserve_cache()").unwrap()
    );
    assert!(recycle.contains("release_data(root)"));
    for (function, mutation) in [
        ("fn checkout_sdma_pool(", "swap_remove("),
        (
            "pub fn trim_sdma_memory_pool(",
            "pool_trim::trim_in_place(self)",
        ),
    ] {
        let body = production.split_once(function).unwrap().1;
        assert!(
            body.find("self.validate_configured_host_pool_v1()?")
                .unwrap()
                < body.find(mutation).unwrap()
        );
    }
}

#[test]
fn device_pool_linux_wiring_keeps_exact_domain_disposal_and_no_second_byte_ledger() {
    // Source wiring only; real records/account/loans/disposal use the fake backend tests.
    let source = crate::queue::live_production_source_for_tests_v1();
    let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let config = production
        .split("pub fn configure_sdma_device_pool_v1(")
        .nth(1)
        .unwrap()
        .split("/// Reports padded backing")
        .next()
        .unwrap();
    assert!(config.contains("validate_device_pool_domain_v1(self.key.vm)"));
    assert!(config.contains("self.sdma_device_pool.configure(limits)"));
    for forbidden in [
        "retained_device_memory",
        "take_queue_model",
        "check_currentness",
        "configure_device_backing_budget",
    ] {
        assert!(!config.contains(forbidden));
    }
    let recycle = production
        .split("pub fn recycle_sdma_buffer(")
        .nth(1)
        .unwrap()
        .split("pub fn trim_sdma_memory_pool(")
        .next()
        .unwrap();
    assert!(recycle.contains("sdma_recycle::recycle_in_place(self, buffer, false)"));
    let recycle = include_str!("../../queue_live/sdma_recycle.rs");
    assert!(recycle.contains("device_pool_recycle_decision_v1("));
    assert!(recycle.contains("DevicePoolDispositionV1::Dispose"));
    assert!(
        recycle.find("context.should_dispose()?").unwrap()
            < recycle.find("context.reserve_cache()").unwrap()
    );
    assert!(recycle.contains("release_data(root)"));
    for operation in ["fn checkout_sdma_pool(", "pub fn trim_sdma_memory_pool("] {
        let body = production.split(operation).nth(1).unwrap();
        let first_mutation = body
            .find(if operation.starts_with("fn checkout") {
                "swap_remove("
            } else {
                "pool_trim::trim_in_place(self)"
            })
            .unwrap();
        assert!(
            body.find("self.validate_configured_device_pool_v1()?")
                .unwrap()
                < first_mutation
        );
    }
    let projection = include_str!("../../shared_memory.rs")
        .split("pub(crate) fn device_pool_backing_bytes_v1(")
        .nth(1)
        .unwrap()
        .split("pub fn retained_allocation_count")
        .next()
        .unwrap();
    assert!(!projection.contains("check_currentness"));
    assert!(!projection.contains("release_"));
}

#[test]
fn backing_constructor_forwarding_precedes_prepare_and_keeps_process_vm_envelope() {
    // Linux acquisition cannot run in this CPU test; the shared-memory
    // tests execute the configuration and ownership helpers with a fake backend.
    let source = crate::queue::live_production_source_for_tests_v1();
    let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let constructor = production
        .split("fn create_compute_aql_queue_with_runtime<T>(")
        .nth(1)
        .unwrap()
        .split("/// Runs one fresh-queue")
        .next()
        .unwrap();
    let acquisition_call = ".acquire_shared_gtt_memory_session_with_host_admission_v1(";
    let acquire = constructor.find(acquisition_call).unwrap();
    let argument = constructor[acquire + acquisition_call.len()..]
        .split_once(')')
        .unwrap()
        .0
        .trim();
    let arguments: Vec<_> = argument
        .split(',')
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
        .collect();
    assert_eq!(
        arguments,
        ["device_backing_budget", "host_visible_backing_budget"]
    );
    let root = constructor
        .find("PrimaryQueueConstructionV1::new(memory, None)")
        .unwrap();
    let prepare = constructor
        .find("capture_returned_preparation_v1(")
        .unwrap();
    let create = constructor.find("root.construct(").unwrap();
    assert!(acquire < root && root < prepare && prepare < create);
    let default_wrapper = production
        .split("pub(crate) fn create_compute_aql_queue_with<T>(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn create_compute_aql_queue_for_debug_target")
        .next()
        .unwrap();
    assert!(default_wrapper.contains("self.create_compute_aql_queue_with_runtime("));
    assert!(default_wrapper.contains("Gfx942FixedDispatchCapacityV1::default()"));
    assert!(
        constructor
            .find("root.dispatch_capacity = capacity")
            .unwrap()
            < prepare
    );
    assert!(production.contains(
        "self.create_compute_aql_queue_with_backing_budgets_v1(ring_bytes, budget, None)"
    ));

    let shared = include_str!("../../shared_memory.rs");
    let acquisition = shared
        .split("pub fn acquire_shared_gtt_memory_session_with_backing_budgets_v1(")
        .nth(1)
        .unwrap()
        .split("impl SharedGttMemorySessionV1 {")
        .next()
        .unwrap();
    let begin = acquisition
        .find("begin_process_vm_attempt(pid, gpu_id)?")
        .unwrap();
    let validate_rooted = acquisition
        .find("admission.matches_device_v1(&self)")
        .unwrap();
    let vm_identity = acquisition.find("NEXT_MODEL_VM_ID").unwrap();
    assert!(validate_rooted < vm_identity && vm_identity < begin);
    let composed_live = acquisition.find("!admission.is_live_v1()").unwrap();
    assert!(validate_rooted < composed_live && composed_live < vm_identity);
    let composed_wrapper = production
        .split("pub fn create_compute_aql_queue_with_composed_backing_v1(")
        .nth(1)
        .unwrap()
        .split("pub ")
        .next()
        .unwrap();
    assert!(composed_wrapper.contains("HostBackingAdmission::Composed(admission)"));
    assert!(composed_wrapper.contains("self.create_compute_aql_queue_with_runtime("));
    let bind = acquisition
        .find("engine.backend.bind_model_vm(VmIdV1(vm_id))?")
        .unwrap();
    let configure = acquisition
        .find(".configure_optional_device_backing_budget(")
        .unwrap();
    let configure_host = acquisition
        .find(".configure_optional_host_visible_backing_budget(")
        .unwrap();
    let finish = acquisition
        .find("finish_process_vm_attempt(result.is_ok(), pid, gpu_id)")
        .unwrap();
    let configure_composed = acquisition.find(".configure_composed_backing(").unwrap();
    assert!(bind < configure_composed && configure_composed < finish);
    assert!(
        begin < bind && bind < configure && configure < configure_host && configure_host < finish
    );
    assert!(
        shared
            .contains("self.acquire_shared_gtt_memory_session_with_device_backing_budget_v1(None)")
    );
    let usage = production
        .split("pub fn device_backing_usage_v1(")
        .nth(1)
        .unwrap()
        .split("/// Adds one generic")
        .next()
        .unwrap();
    assert!(usage.contains("engine.backend.session.device_backing_usage_v1()"));
    assert!(!usage.contains("check_currentness"));
    assert!(!usage.contains("with_live_queue_memory_model"));
}

#[test]
fn production_dependency_owner_lane_envelope_and_teardown_shape_is_sealed() {
    let source = crate::queue::live_production_source_for_tests_v1();
    let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let fixed = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let primary = include_str!("../../queue_live/construction_primary.rs");
    let environment = include_str!("../../queue_live/construction_primary/environment.rs");
    let auxiliary = include_str!("../../queue_live/construction_auxiliary.rs");
    assert!(!auxiliary.contains("ComputeDependencySessionOwnerV1::new("));
    assert!(!auxiliary.contains("create_dependency_owner("));
    assert_eq!(
        production
            .lines()
            .filter(|line| {
                line.trim()
                    == "dependency_owner: QueueOwnerSlotV1<ComputeDependencySessionOwnerV1>,"
            })
            .count(),
        1
    );
    assert_eq!(
        production
            .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
            .count()
            + primary
                .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                .count()
            + environment
                .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                .count(),
        1
    );
    assert_eq!(
        primary.matches("E::create_dependency_owner(key)").count(),
        1
    );
    let dependency_constructor = environment
        .split("fn create_dependency_owner(")
        .nth(1)
        .unwrap()
        .split("fn ")
        .next()
        .unwrap();
    assert_eq!(
        dependency_constructor
            .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
            .count(),
        1
    );
    let lane_state = production
        .split("struct ComputeAqlQueueLaneStateV1")
        .nth(1)
        .unwrap()
        .split("fn prepare_auxiliary_compute_lane_slot_v1")
        .next()
        .unwrap();
    assert!(!lane_state.contains("ComputeDependencySessionOwnerV1"));

    let envelope = production
        .split("fn with_dependency_target_lane_v1<R>")
        .nth(1)
        .unwrap()
        .split("pub fn submit_fixed_dispatch_with_dependencies_v1")
        .next()
        .unwrap();
    assert_eq!(envelope.matches("std::panic::catch_unwind").count(), 3);
    assert_eq!(
        envelope
            .matches("std::panic::resume_unwind(payload)")
            .count(),
        3
    );
    assert!(envelope.contains("self.swap_primary_compute_lane(&mut displaced_primary)"));
    assert!(envelope.contains("source.completion_owner"));

    let submit = production
        .split("pub fn submit_fixed_dispatch_with_dependencies_v1")
        .nth(1)
        .unwrap()
        .split("fn cancel_dependency_dispatch_generation_v1")
        .next()
        .unwrap();
    assert!(submit.contains("with_dependency_target_lane_v1"));
    assert!(submit.contains("source_owner"));
    let poll = production
        .split("pub fn poll_compute_dependency_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn release_compute_dependency_event_v1")
        .next()
        .unwrap();
    assert!(poll.contains("with_dependency_target_lane_v1"));
    assert!(poll.contains("release_after_dependent_completion"));
    assert!(poll.contains("permanently_poison_process_global_kfd_runtime_gate_v1"));
    assert_eq!(production.matches(".ensure_idle()").count(), 1);
    let auxiliary_destroy = production
        .split("pub fn destroy_auxiliary_compute_lane_v1(")
        .nth(1)
        .unwrap()
        .split("fn create_compute_aql_queue_inner(")
        .next()
        .unwrap();
    assert!(auxiliary_destroy.contains("auxiliary_release::release_in_place(self, lane)"));
    let auxiliary_release = include_str!("../../queue_live/auxiliary_release.rs");
    let auxiliary_preflight = auxiliary_release
        .split("pub(super) fn release_in_place<")
        .nth(1)
        .unwrap()
        .split("fn release_inner<")
        .next()
        .unwrap();
    assert_eq!(auxiliary_preflight.matches(".ensure_idle()").count(), 1);
    let idle = auxiliary_preflight.find(".ensure_idle()").unwrap();
    let owners = auxiliary_preflight
        .find("preflight_primary_owners_v1::<C::Environment>(")
        .unwrap();
    let retain = auxiliary_preflight
        .find("*parts.custody = Some(AuxiliaryReleaseCustodyV1::new(")
        .unwrap();
    let entry = auxiliary_preflight
        .find("catch_unwind(AssertUnwindSafe(|| release_inner(context)))")
        .unwrap();
    assert!(idle < owners && owners < retain && retain < entry);

    let native_submit = include_str!("../../queue_submit.rs");
    let callback_catcher = native_submit
        .split("fn catch_dependency_callback<T>")
        .nth(1)
        .unwrap()
        .split("impl<B: NativeAqlSubmissionBackendV1>")
        .next()
        .unwrap();
    assert!(callback_catcher.contains("Err(_) => Err(NativeAqlSubmissionErrorV1::CallbackPanic)"));
    assert!(submit.contains("std::panic::resume_unwind(payload)"));
    assert!(submit.contains("ComputeDependencyPublicationFailureV1::Terminal(custody)"));
    assert!(submit.contains("Box::new(Gfx942ComputeDependencyDispatchV1"));
    assert!(poll.contains("dispatch: Box<Gfx942ComputeDependencyDispatchV1>"));
    assert!(poll.contains("dispatch") && poll.contains(".published") && poll.contains(".take()"));
    assert!(poll.contains("Gfx942ComputeDependencyPollV1::Pending(dispatch)"));
    assert!(!poll.contains("Box::new(Gfx942ComputeDependencyDispatchV1"));
    assert!(envelope.contains("AdmittedComputeLaneV1::Auxiliary(source_index)"));

    let source_submit = fixed
        .split("fn submit_fixed_dispatch_with_dependency_events_operation_v1")
        .nth(1)
        .unwrap()
        .split("fn submit_with_dependency_events_classified_v1")
        .next()
        .unwrap();
    assert!(source_submit.contains("GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1"));
    assert!(source_submit.contains("dependency source packet count must be 1 through 8192"));
    assert!(submit.contains("AQL_MAX_DEPENDENCY_SIGNALS_V1"));
    assert!(submit.contains("dependency roster must contain 1 through 256 events"));
    assert!(submit.contains("dependency_owner.ensure_target_capacity()"));
    assert_eq!(MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1, 128);

    let recycle = fixed
        .split("pub fn recycle_fixed_dispatch<const N: usize>")
        .nth(1)
        .unwrap()
        .split("pub fn recycled_fixed_dispatch_generation")
        .next()
        .unwrap();
    // Seal both the shared retry predicate and the native caller's custody handling.
    let classifier = production
        .split("fn from_completion_failure(")
        .nth(1)
        .unwrap()
        .split("pub const fn error(")
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<String>();
    assert!(classifier.contains(concat!(
        "letretryable_completed=matches!(error,",
        "ComputeAqlQueueSessionErrorV1::Completion(",
        "Gfx942CompletionErrorV1::SignalPinned{..}))",
        ".then(||wrap_completed(completed,identity));"
    )));
    assert!(classifier.contains("Self{error,retryable_completed,}"));
    let recycle_inner = recycle
        .split("pub(in super::super) fn recycle_fixed_dispatch_inner<const N: usize>")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .collect::<String>();
    assert!(recycle_inner.contains(concat!(
        "Err((error,completion))=>{",
        "letfailure=Gfx942FixedDispatchRecycleFailureV1::",
        "from_completion_failure(error,completion,identity,);",
        "iffailure.retryable_completed.is_none()",
        "&&letSome(dispatch)=self.dispatch.as_mut(){dispatch.poison();}",
        "returnErr(failure);"
    )));
    assert!(recycle.contains("Gfx942DispatchBindingErrorV1::StaleDispatchGeneration"));
    assert!(recycle.contains("retryable_completed: None"));
}

#[test]
fn nonpanic_retake_failure_requests_local_and_process_terminal_poison() {
    let mut poison = (false, false);
    let mut destination = [0xff; 6];
    let ((), retake) = execute_live_model_custody_v1(
        &mut poison,
        |_| Ok(()),
        |_| destination[1..5].copy_from_slice(&[1, 2, 3, 4]),
        |_, ()| Err("injected retake failure"),
        |poison| *poison = (true, true),
    )
    .unwrap();
    assert_eq!(retake, Err("injected retake failure"));
    assert_eq!(poison, (true, true));
    assert_eq!(destination, [0xff, 1, 2, 3, 4, 0xff]);
    let source = crate::queue::live_production_source_for_tests_v1();
    let envelope = source
        .split("fn with_live_queue_memory_model_custody<R>")
        .nth(1)
        .unwrap()
        .split("fn with_sdma_queue_creation_custody_v1<R>")
        .next()
        .unwrap();
    assert!(envelope.contains("execute_live_model_custody_v1("));
    assert!(envelope.contains("session.poison_terminal()"));
    assert!(envelope.contains("permanently_poison_process_global_kfd_runtime_gate_v1()"));
}

#[test]
fn destroy_revision_preflight_preserves_precondition_order_and_is_terminal() {
    let source = crate::queue::live_production_source_for_tests_v1();
    let destroy = source
        .split("fn destroy_queue_and_event(")
        .nth(1)
        .unwrap()
        .split("fn complete_destroy<T>")
        .next()
        .unwrap();
    let releasable = destroy
        .find("self.completion_owner.ensure_releasable()")
        .unwrap();
    let mode = destroy
        .find("let (return_attached, detached_return) = match mode")
        .unwrap();
    let preflight = destroy
        .find("preflight_memory_transition_revisions(1)")
        .unwrap();
    let terminal = destroy[preflight..].find("self.poison_terminal()").unwrap() + preflight;
    let process = destroy[preflight..]
        .find("permanently_poison_process_global_kfd_runtime_gate_v1()")
        .unwrap()
        + preflight;
    let first_native = destroy.find("striped_sdma.destroy_queue(memory)").unwrap();
    assert!(releasable < mode);
    assert!(mode < preflight);
    assert!(preflight < terminal);
    assert!(terminal < process);
    assert!(process < first_native);
}

#[test]
fn retake_failure_does_not_replace_the_original_panic_payload() {
    let mut poisoned = false;
    let mut destination = [0xff; 6];
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute_live_model_custody_v1(
            &mut poisoned,
            |_| Ok(()),
            |_| -> () {
                destination[1..4].copy_from_slice(&[1, 2, 3]);
                std::panic::panic_any("original queue mutation panic")
            },
            |_, ()| Err("injected retake failure"),
            |poisoned| *poisoned = true,
        )
    }))
    .expect_err("panic resumption must escape the cleanup boundary");
    assert!(poisoned);
    assert_eq!(destination, [0xff, 1, 2, 3, 0xff, 0xff]);
    assert_eq!(
        caught.downcast_ref::<&str>(),
        Some(&"original queue mutation panic")
    );
    let helper = include_str!("../../queue_live/model_loan.rs");
    let operation = helper
        .find("catch_unwind(AssertUnwindSafe(|| operation(context)))")
        .unwrap();
    let retake = helper
        .find("catch_unwind(AssertUnwindSafe(|| retake(context, loan)))")
        .unwrap();
    let poison = helper[retake..].find("poison(context)").unwrap() + retake;
    let retain_secondary = helper.find("core::mem::forget(closing)").unwrap();
    assert!(operation < retake && retake < poison && poison < retain_secondary);

    let bind = crate::queue::fixed_dispatch_production_source_for_tests_v1()
        .split("pub fn bind_directional_persistent_fixed_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn submit_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    let panic_arm = bind
        .split("let prepared_dispatch = match prepared_dispatch")
        .nth(1)
        .unwrap()
        .split("let prepared_dispatch = match prepared_dispatch")
        .next()
        .unwrap();
    assert!(panic_arm.contains("quarantine_persistent_retained_control_replay_prepared_v1"));
    assert!(panic_arm.contains("state,"));
    assert!(!panic_arm.contains("state: PersistentComputeUseStateV1::Quarantined"));
}
