use super::*;

#[test]
fn auxiliary_lane_reuse_advances_generation_and_rejects_substitution() {
    let session = test_queue_key(17, 3);
    let other_session = test_queue_key(19, 3);
    let mut slots = Vec::<AuxiliaryComputeLaneSlotV1<&'static str>>::new();
    let first = prepare_auxiliary_compute_lane_slot_v1(&slots).unwrap();
    assert_eq!(
        first,
        PreparedAuxiliaryComputeLaneSlotV1 {
            index: 0,
            generation: 1,
            append: true,
        }
    );
    slots.try_reserve_exact(1).unwrap();
    let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, first).unwrap();
    install_auxiliary_compute_lane_slot_v1(destination, "first");
    let first_handle = ComputeAqlQueueLaneV1 {
        session,
        ordinal: 1,
        generation: first.generation,
    };
    assert!(matches!(
        admit_compute_lane_v1(session, &slots, first_handle),
        Ok(AdmittedComputeLaneV1::Auxiliary(0))
    ));
    assert!(matches!(
        admit_compute_lane_v1(other_session, &slots, first_handle),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute queue lane session substitution"
        ))
    ));

    assert_eq!(slots[0].state.take(), Some("first"));
    let replacement = prepare_auxiliary_compute_lane_slot_v1(&slots).unwrap();
    assert_eq!(replacement.index, first.index);
    assert_eq!(replacement.generation, first.generation + 1);
    assert!(!replacement.append);
    let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, replacement).unwrap();
    install_auxiliary_compute_lane_slot_v1(destination, "replacement");

    assert!(matches!(
        admit_compute_lane_v1(session, &slots, first_handle),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "stale compute queue lane"
        ))
    ));
    let replacement_handle = ComputeAqlQueueLaneV1 {
        session,
        ordinal: 1,
        generation: replacement.generation,
    };
    assert!(matches!(
        admit_compute_lane_v1(session, &slots, replacement_handle),
        Ok(AdmittedComputeLaneV1::Auxiliary(0))
    ));
    assert_eq!(slots[0].state, Some("replacement"));
}

#[test]
fn sdma_dispatch_content_check_rejects_length_and_digest_substitution() {
    let bytes = b"exact initialized bytes";
    let role = Gfx942DeviceContentRoleV1::new([0x5a; 32], 7).unwrap();
    let descriptor = Gfx942DeviceContentDescriptorV1::from_bytes(role, bytes).unwrap();
    let digest: [u8; 32] = Sha256::digest(bytes).into();

    assert!(content_descriptor_matches_bytes(descriptor, bytes));
    assert!(content_descriptor_matches_sha256(
        descriptor,
        bytes.len() as u64,
        digest
    ));
    assert!(!content_descriptor_matches_bytes(
        descriptor,
        b"exact initialized byte"
    ));

    let mut substituted = bytes.to_vec();
    substituted[0] ^= 1;
    assert!(!content_descriptor_matches_bytes(descriptor, &substituted));
    let substituted_digest: [u8; 32] = Sha256::digest(&substituted).into();
    assert!(!content_descriptor_matches_sha256(
        descriptor,
        bytes.len() as u64,
        substituted_digest
    ));
    assert!(!content_descriptor_matches_sha256(
        descriptor,
        bytes.len() as u64 - 1,
        digest
    ));
}

#[test]
fn persistent_ready_promotion_uses_only_bound_certificate_under_currentness() {
    let source = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let promotion = source
        .split("pub fn promote_full_h2d_to_persistent_compute_ready_v1")
        .nth(1)
        .unwrap()
        .split("fn terminal_persistent_retained_control_replay_after_detach_v1")
        .next()
        .unwrap();
    assert!(promotion.contains("certified_full_host_content_sha256"));
    assert_eq!(
        promotion
            .matches("check_queue_operational_currentness")
            .count(),
        2
    );
    assert!(!promotion.contains("sha256_host_buffer"));
    assert!(!promotion.contains("Sha256::digest"));
}

#[test]
fn persistent_compute_auxiliary_quiescence_accepts_exact_detached_idle_states() {
    let max = super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
    assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
        true,
        None,
        2,
        Some(7),
        2,
        None,
    ));
    assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
        true,
        None,
        0,
        Some(7),
        0,
        Some(0),
    ));
    assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
        true,
        None,
        0,
        Some(0),
        0,
        Some(0),
    ));
    assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
        true,
        Some(true),
        0,
        None,
        0,
        None,
    ));
    for facts in [
        (false, None, 0, Some(7), 0, Some(0)),
        (true, Some(false), 0, None, 0, None),
        (true, Some(true), 1, None, 1, None),
        (true, Some(true), 0, Some(7), 0, Some(0)),
        (true, None, 1, Some(0), 1, Some(0)),
        (true, None, 2, Some(7), 1, None),
        (true, None, max + 1, Some(7), max + 1, None),
        (true, None, 1, Some(7), 1, Some(2)),
    ] {
        assert!(!auxiliary_compute_lane_quiescence_from_facts_v1(
            facts.0, facts.1, facts.2, facts.3, facts.4, facts.5,
        ));
    }
}

#[test]
fn persistent_compute_blocks_every_generic_recycled_dispatch_data_access() {
    for operation in [
        GenericRecycledDispatchAccessV1::Read,
        GenericRecycledDispatchAccessV1::ReadInto,
        GenericRecycledDispatchAccessV1::InitializedReadInto,
        GenericRecycledDispatchAccessV1::Snapshot,
        GenericRecycledDispatchAccessV1::Overwrite,
    ] {
        assert!(matches!(
            admit_generic_recycled_dispatch_access(false, true, operation),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert!(matches!(
            admit_generic_recycled_dispatch_access(false, false, operation),
            Ok(observed) if observed == operation
        ));
        for persistent_compute_attached in [false, true] {
            assert!(matches!(
                admit_generic_recycled_dispatch_access(
                    true,
                    persistent_compute_attached,
                    operation,
                ),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
        }
    }
}

#[test]
fn injected_persistent_bind_cancellation_failures_preserve_reserved_and_prepared_leases() {
    let mut reserved_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5201),
    );
    let mut prepared_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5202),
    );
    let mut foreign_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5203),
    );
    let request = |byte_len| {
        Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
            0,
            byte_len,
        )
        .unwrap()
    };

    let reserved = reserved_owner
        .reserve(request(reserved_owner.byte_len()), None)
        .unwrap();
    let reserved = cancel_persistent_compute_reserved_v1(&mut foreign_owner, reserved)
        .expect_err("foreign owner must return exact reserved lease");
    let reserved = match classify_persistent_bind_cancellation_v1(Err(reserved)) {
        PersistentBindCancellationDispositionV1::Terminal(reserved) => reserved,
        PersistentBindCancellationDispositionV1::Retryable => {
            panic!("failed reserved cancellation must never be retryable")
        }
    };
    assert_eq!(reserved_owner.live_use_count(), 1);
    cancel_persistent_compute_reserved_v1(&mut reserved_owner, reserved).unwrap();

    let reserved = prepared_owner
        .reserve(request(prepared_owner.byte_len()), None)
        .unwrap();
    let prepared = prepared_owner.prepare(reserved).unwrap();
    let prepared = cancel_persistent_compute_prepared_v1(&mut foreign_owner, prepared)
        .expect_err("foreign owner must return exact prepared lease");
    let prepared = match classify_persistent_bind_cancellation_v1(Err(prepared)) {
        PersistentBindCancellationDispositionV1::Terminal(prepared) => prepared,
        PersistentBindCancellationDispositionV1::Retryable => {
            panic!("failed prepared cancellation must never be retryable")
        }
    };
    assert_eq!(prepared_owner.live_use_count(), 1);
    cancel_persistent_compute_prepared_v1(&mut prepared_owner, prepared).unwrap();
    assert!(matches!(
        classify_persistent_bind_cancellation_v1::<()>(Ok(())),
        PersistentBindCancellationDispositionV1::Retryable
    ));

    let production = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let reserved_terminal = production
        .split("match classify_persistent_bind_cancellation_v1")
        .nth(1)
        .unwrap()
        .split("if let Some(dispatch)")
        .next()
        .unwrap();
    assert!(
        reserved_terminal.contains("PersistentBindCancellationDispositionV1::Terminal(reserved)")
    );
    assert!(reserved_terminal.contains("PersistentComputeUseStateV1::Reserved(reserved)"));
    assert!(reserved_terminal.contains("ProcessTeardown"));
    let prepared_terminal = production
        .split("Err(error) => match classify_persistent_bind_cancellation_v1")
        .nth(1)
        .unwrap()
        .split("let (mut allocation")
        .next()
        .unwrap();
    assert!(
        prepared_terminal.contains("PersistentBindCancellationDispositionV1::Terminal(prepared)")
    );
    assert!(prepared_terminal.contains("PersistentComputeUseStateV1::Prepared(prepared)"));
    assert!(prepared_terminal.contains("ProcessTeardown"));
}

#[test]
fn coherent_capture_preflight_binds_owner_kind_and_logical_extent() {
    let queue = test_queue_key(701, 1);
    let (device, mut host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 91);
    host.set_logical_bytes(32);
    assert_eq!(preflight_sdma_host_read_into_v1(&host, queue, 8, 8), Ok(()));
    for foreign in [test_queue_key(702, 1), test_queue_key(701, 2)] {
        assert_eq!(
            preflight_sdma_host_read_into_v1(&host, foreign, 8, 8),
            Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer)
        );
    }
    assert_eq!(
        preflight_sdma_host_read_into_v1(&device, queue, 0, 8),
        Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer)
    );
    for (offset, len) in [(0, 0), (25, 8), (u64::MAX, 8)] {
        assert_eq!(
            preflight_sdma_host_read_into_v1(&host, queue, offset, len),
            Err(Gfx942SdmaHostReadIntoErrorV1::InvalidRange)
        );
    }
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut destination = [0xa5; 8];
    assert_eq!(
        session.read_sdma_host_buffer_into_v1(&host, 8, &mut destination),
        Err(Gfx942SdmaHostReadIntoErrorV1::Unavailable)
    );
    assert_eq!(destination, [0xa5; 8]);
    assert!(!session.terminal_poisoned);
    session.terminal_poisoned = true;
    assert_eq!(
        session.read_sdma_host_buffer_into_v1(&host, 8, &mut destination),
        Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain)
    );
    assert_eq!(
        session.read_sdma_host_buffer_into_v1(&device, u64::MAX, &mut destination),
        Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain)
    );
    assert_eq!(destination, [0xa5; 8]);
}

#[test]
fn coherent_read_into_preserves_disabled_and_terminal_admission_order() {
    let queue = test_queue_key(701, 1);
    let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 91);
    for terminal in [false, true] {
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.terminal_poisoned = terminal;
        for buffer in [&host, &device] {
            for offset in [0, u64::MAX] {
                for len in [0, 8] {
                    let mut destination = [0xa5; 8];
                    let result =
                        session.read_sdma_host_buffer_into(buffer, offset, &mut destination[..len]);
                    assert!(matches!(
                        result,
                        Err(ComputeAqlQueueSessionErrorV1::Contract(detail))
                            if detail == if terminal {
                                "terminal queue session requires process teardown"
                            } else {
                                "SDMA copy engine is not enabled"
                            }
                    ));
                    assert_eq!(destination, [0xa5; 8]);
                    assert_eq!(session.terminal_poisoned, terminal);
                    assert!(session.engine.is_none());
                }
            }
        }
    }
}

#[test]
fn coherent_read_into_retake_panic_preserves_copied_bytes_and_poison() {
    let mut poison = (false, false);
    let mut destination = [0xff; 6];
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute_live_model_custody_v1(
            &mut poison,
            |_| Ok(()),
            |_| destination[1..5].copy_from_slice(&[1, 2, 3, 4]),
            |_, ()| -> Result<(), &'static str> { std::panic::panic_any("readback retake panic") },
            |poison| *poison = (true, true),
        )
    }))
    .unwrap_err();
    assert_eq!(
        caught.downcast_ref::<&str>(),
        Some(&"readback retake panic")
    );
    assert_eq!(destination, [0xff, 1, 2, 3, 4, 0xff]);
    assert_eq!(poison, (true, true));
}

#[test]
fn coherent_read_into_queue_wiring_preserves_legacy_policy_without_output_allocation() {
    let source = crate::queue::live_production_source_for_tests_v1();
    let body = source
        .split("pub fn read_sdma_host_buffer_into(")
        .nth(1)
        .unwrap()
        .split("/// Reads one exact retained")
        .next()
        .unwrap();
    assert!(body.contains("self.require_sdma_enabled()?"));
    assert!(body.contains("self.with_live_queue_memory_model("));
    assert!(body.contains("crate::sdma::read_host_buffer_into_v1"));
    for forbidden in [
        "preflight_sdma_host_read_into_v1",
        "Gfx942SdmaHostReadIntoErrorV1",
        ".poll(",
        ".flush(",
        "read_host_buffer(",
        "to_vec(",
        "allocate_",
    ] {
        assert!(
            !body.contains(forbidden),
            "unexpected read path: {forbidden}"
        );
    }
}

#[test]
fn coherent_capture_queue_wiring_preserves_retake_and_terminal_boundary() {
    let source = crate::queue::live_production_source_for_tests_v1();
    let body = source
        .split("pub fn read_sdma_host_buffer_into_v1(")
        .nth(1)
        .unwrap()
        .split("/// Rebrands one fully initialized")
        .next()
        .unwrap();
    assert!(body.contains("preflight_sdma_host_read_into_v1"));
    assert!(body.contains("self.with_live_queue_memory_model("));
    assert!(body.contains("crate::sdma::read_host_buffer_into_v1"));
    assert!(body.contains("permanently_poison_process_global_kfd_runtime_gate_v1()"));
    for forbidden in [
        ".poll(",
        ".flush(",
        "read_host_buffer(",
        "to_string(",
        "to_vec(",
        "allocate_",
    ] {
        assert!(
            !body.contains(forbidden),
            "unexpected capture path: {forbidden}"
        );
    }
}

#[test]
fn scaled_persistent_bind_returns_exact_inputs_before_native_preparation() {
    use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceVectorV1};
    let queue = test_queue_key(710, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let account = ResourceCreditAccountV1::new(ResourceVectorV1::ZERO, 1).unwrap();
    session.dispatch_capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    let packet = || {
        Gfx942FixedDispatchPacketV1::new(
            0,
            fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        )
    };
    let role = Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap();
    let input = |id| {
        let allocation = persistent_compute_gate_test_allocation_v1(queue, id);
        let identity = allocation
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity();
        (
            Gfx942PersistentComputeInputV1::from_parts(
                allocation,
                PersistentComputeInitializationV1::AfterDispatch,
            ),
            identity,
        )
    };
    let (one, expected) = input(0x6100);
    let failure = session
        .bind_directional_persistent_fixed_dispatch_v1(Vec::new(), [packet()], one, role)
        .expect_err("scaled persistent bind must reject");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract("scaled capacity excludes persistent compute")
    ));
    let (_, custody) = failure.into_parts();
    let Gfx942PersistentComputeBindFailureCustodyV1::Retryable(one) = custody else {
        panic!("exact retryable input")
    };
    assert_eq!(
        one.into_parts()
            .0
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity(),
        expected
    );

    let [(a, a_id), (b, b_id), (c, c_id)] = [0x6101, 0x6102, 0x6103].map(input);
    let failure = session
        .bind_three_binding_directional_persistent_fixed_dispatch_v1(
            Vec::new(),
            [packet()],
            Gfx942ThreeBindingPersistentComputeInputsV1::new([a, b, c]),
            [role; 3],
        )
        .expect_err("scaled three-binding persistent bind must reject");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract("scaled capacity excludes persistent compute")
    ));
    let (_, custody) = failure.into_parts();
    let Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs) = custody else {
        panic!("exact retryable inputs")
    };
    for (input, expected) in inputs.into_inputs().into_iter().zip([a_id, b_id, c_id]) {
        assert_eq!(
            input
                .into_parts()
                .0
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity(),
            expected
        );
    }
    assert!(!session.terminal_poisoned);
    assert!(session.persistent_compute.is_none());
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn either_persistent_roster_cardinality_blocks_generic_queue_transitions() {
    for (queue_id, binding_count) in [(190, 1), (191, 3)] {
        let queue = test_queue_key(queue_id, 1);
        let mut session = persistent_compute_gate_test_session_v1(queue, binding_count);
        let roster_snapshot = |session: &ComputeAqlQueueSessionV1| {
            let attachment = session.persistent_compute.as_ref().unwrap();
            (
                attachment.binding,
                attachment
                    .entries
                    .iter()
                    .map(|entry| {
                        (
                            entry.storage_identity,
                            entry.initialization,
                            entry.effect,
                            entry.allocation.byte_len(),
                            entry.allocation.owner.live_use_count(),
                            entry.allocation.owner.quarantine_reason(),
                        )
                    })
                    .collect::<Vec<_>>(),
                session.next_persistent_compute_generation,
            )
        };
        let existing = roster_snapshot(&session);
        assert!(session.has_any_persistent_compute_attachment_v1());
        assert_eq!(
            session
                .persistent_compute
                .as_ref()
                .expect("gate fixture retains attachment")
                .entries
                .len(),
            binding_count
        );

        assert!(matches!(
            session.submit_fixed_dispatch::<1>(),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            ))
        ));
        assert!(matches!(
            session.submit_fixed_dispatch_classified_v1::<1>(),
            Err(
                Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )
                )
            )
        ));
        assert!(!session.terminal_poisoned);
        assert!(matches!(
            session.detach_recycled_fixed_dispatch(),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            ))
        ));
        assert!(matches!(
            session.release_retained_persistent_fixed_dispatch_control_v1(),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            ))
        ));
        assert!(matches!(
            session.recycled_fixed_dispatch_generation(),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            ))
        ));
        assert!(matches!(
            session.destroy_queue_and_event(QueueDestroyModeV1::Release),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute attachment must be restored before queue destruction"
            ))
        ));

        let allocation = persistent_compute_gate_test_allocation_v1(queue, 0x5400 + queue_id);
        let incoming = (
            allocation
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity(),
            allocation.byte_len(),
        );
        let input = Gfx942PersistentComputeInputV1::from_parts(
            allocation,
            PersistentComputeInitializationV1::AfterDispatch,
        );
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        let failure = match session.bind_directional_persistent_fixed_dispatch_v1(
            Vec::new(),
            [packet],
            input,
            Gfx942DeviceContentRoleV1::new([0x54; 32], 0).unwrap(),
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("a live persistent roster must reject generic rebind"),
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, custody) = failure.into_parts();
        let Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input) = custody else {
            panic!("occupied-slot rejection must return the exact incoming input")
        };
        let (allocation, initialization) = input.into_parts();
        let digest = initialization.authenticated_sha256();
        let initialized = initialization.is_fully_initialized();
        assert_eq!(digest, None);
        assert!(initialized);
        assert_eq!(allocation.attachment.queue, queue);
        assert_eq!(
            (
                allocation
                    .owner
                    .local_native_for_sdma()
                    .unwrap()
                    .storage_identity(),
                allocation.byte_len(),
            ),
            incoming
        );
        assert_eq!(roster_snapshot(&session), existing);
        assert!(session.has_any_persistent_compute_attachment_v1());
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn every_generic_linear_transition_checks_the_unified_persistent_roster_gate() {
    let fixed = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let production = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let guarded_spans = [
        (
            fixed,
            "pub fn detach_recycled_fixed_dispatch(\n        &mut self,",
            1,
            "pub fn release_retained_persistent_fixed_dispatch_control_v1",
        ),
        (
            fixed,
            "pub fn submit_fixed_dispatch<const N: usize>(\n        &mut self,",
            1,
            "fn submit_fixed_dispatch_with_dependency_events_inner_v1",
        ),
        (
            fixed,
            "pub fn poll_fixed_dispatch<const N: usize>(\n        &mut self,",
            1,
            "pub fn poll_fixed_dispatch_with_progress<const N: usize>",
        ),
        (
            fixed,
            "pub fn poll_fixed_dispatch_with_progress<const N: usize>(\n        &mut self,",
            1,
            "fn wait_fixed_dispatch_inner<const N: usize>",
        ),
        (
            fixed,
            "pub fn recycle_fixed_dispatch<const N: usize>(\n        &mut self,",
            1,
            "pub fn recycled_fixed_dispatch_generation",
        ),
        (
            production,
            "fn destroy_queue_and_event(\n        &mut self,",
            1,
            "fn complete_destroy<T>",
        ),
        (
            fixed,
            "pub fn bind_three_binding_directional_persistent_fixed_dispatch_v1",
            1,
            "pub fn bind_directional_persistent_fixed_dispatch_v1",
        ),
        (
            fixed,
            "pub fn bind_directional_persistent_fixed_dispatch_v1",
            1,
            "pub fn submit_directional_persistent_fixed_dispatch_v1",
        ),
    ];
    for (source, start, occurrence, end) in guarded_spans {
        let span = source
            .split(start)
            .nth(occurrence)
            .unwrap_or_else(|| panic!("missing audited transition: {start}"))
            .split(end)
            .next()
            .unwrap();
        assert!(
            span.contains("self.has_any_persistent_compute_attachment_v1()"),
            "transition bypasses unified persistent roster gate: {start}"
        );
    }
}

#[test]
fn persistent_bind_terminal_ingress_retains_self_but_returns_foreign_input() {
    let receiver = test_queue_key(291, 1);
    for source in [receiver, test_queue_key(292, 1)] {
        let mut session = persistent_compute_cancellation_test_session(receiver, None, None);
        session.poison_terminal();
        let allocation = persistent_compute_gate_test_allocation_v1(source, 0x9100);
        let identity = allocation
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity();
        let input = Gfx942PersistentComputeInputV1::from_parts(
            allocation,
            PersistentComputeInitializationV1::AfterDispatch,
        );
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        let failure = match session.bind_directional_persistent_fixed_dispatch_v1(
            Vec::new(),
            [packet],
            input,
            Gfx942DeviceContentRoleV1::new([0x54; 32], 0).unwrap(),
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("terminal ingress must reject"),
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        let input = match failure.into_parts().1 {
            Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input) => {
                assert_ne!(source, receiver);
                input
            }
            Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(terminal) => {
                assert_eq!(source, receiver);
                terminal.input.unwrap()
            }
        };
        let (allocation, initialization) = input.into_parts();
        let _ = initialization.authenticated_sha256();
        let _ = initialization.is_fully_initialized();
        assert_eq!(allocation.attachment.queue, source);
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity(),
            identity
        );
        assert!(session.persistent_compute.is_none());
        assert!(session.dispatch.is_none());
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn three_binding_false_closing_currentness_quarantines_all_owners_for_any_observation() {
    use super::super::super::completion::TestOnlyAmbiguousCompletionObservationV1;

    for (queue_id, observation) in [
        (192, TestOnlyAmbiguousCompletionObservationV1::Pending),
        (193, TestOnlyAmbiguousCompletionObservationV1::Completed),
    ] {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(queue_id, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch_owner =
            super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let batch = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch_owner,
                |generation| test_completion_template(queue, generation),
                |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Ok(queue_id)
                },
            )
            .unwrap();
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        session.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_three(
            ThreeBindingPersistentComputeAttachmentV1 {
                entries: [
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5500 + queue_id,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5600 + queue_id,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5700 + queue_id,
                        Gfx942PersistentComputeEffectV1::Write,
                    ),
                ],
                binding,
                predecessor_dispatch_generation: None,
                terminal_custody: None,
            },
        ));
        let receipt = Gfx942ThreeBindingPersistentComputeDispatchV1 {
            binding,
            batch,
            thread_affinity: PhantomData,
        };
        let failure = match session
            .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
                receipt,
                |_, identity, completion| {
                    dispatch_owner
                        .validate_published(identity, completion)
                        .is_ok()
                },
                |session, completion| {
                    session
                        .completion_owner
                        .observe_one_with_false_closing_currentness_for_test(
                            completion,
                            observation,
                        )
                        .map_err(|(error, completion)| (error.into(), completion))
                },
            ) {
            Err(failure) => failure,
            Ok(_) => panic!("false closing currentness cannot be retryable or complete"),
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Currentness)
        ));
        let (_, recovered) = failure.into_parts();
        assert!(recovered.is_none());
        assert!(session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        let attachment = session
            .three_binding_persistent_compute_attachment_v1()
            .expect("terminal queue retains all three owners");
        assert!(attachment.entries.iter().all(|entry| {
            matches!(entry.state, PersistentComputeUseStateV1::Quarantined)
                && entry.allocation.owner.quarantine_reason()
                    == Some(
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                    )
        }));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Published(_))
        ));
    }
}
