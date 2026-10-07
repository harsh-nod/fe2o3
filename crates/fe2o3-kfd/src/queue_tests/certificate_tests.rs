use super::*;

#[test]
fn foundation_certificate_rejects_binding_generation_and_revision_substitution() {
    let mut fixture = fixture();
    let issuer = fixture.foundation.issuer().unwrap();
    let vm = fixture.vm.model_key();
    assert!(
        fixture
            .foundation
            .authenticate(2, fixture.device, vm, issuer)
            .is_err()
    );
    assert!(
        fixture
            .foundation
            .authenticate(1, fixture.device, vm, issuer + 1)
            .is_err()
    );
    assert!(
        fixture
            .foundation
            .authenticate(
                1,
                fixture.device,
                VmKeyV1 {
                    device: vm.device,
                    id: VmIdV1(vm.id.0 + 1),
                },
                issuer,
            )
            .is_err()
    );

    let (_, other_device) = DeviceIdentityStateV1::new(domain())
        .register_device_model_only(correlation(), DeviceGenerationV1(2))
        .unwrap();
    assert!(
        fixture
            .foundation
            .authenticate(1, other_device, vm, issuer)
            .is_err()
    );
    assert!(
        fixture
            .foundation
            .begin_live_loan(1, fixture.device, vm, issuer, 2)
            .is_err()
    );

    let memory = fixture.foundation.memory().clone();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(memory)
        .unwrap();
    let starting_revision = fixture
        .foundation
        .begin_live_loan(1, fixture.device, vm, issuer, 1)
        .unwrap();
    fixture.foundation.certificate.as_mut().unwrap().revision = starting_revision - 1;
    assert!(
        fixture
            .foundation
            .authenticate_live_loan(1, fixture.device, vm, issuer, 1, starting_revision,)
            .is_err()
    );
}

#[test]
fn foundation_certificate_rejects_active_vm_from_a_different_selected_device() {
    let identity = DeviceIdentityStateV1::new(domain());
    let (identity, first_device) = identity
        .register_device_model_only(correlation(), DeviceGenerationV1(1))
        .unwrap();
    let (identity, second_device) = identity
        .register_device_model_only(
            correlation_for(0x7ced_1647_a296_545c, 6),
            DeviceGenerationV1(1),
        )
        .unwrap();
    let first_correlation = first_device.correlation();
    let (identity, first_vm) = identity
        .register_vm_model_only(
            first_device,
            UntrustedVmObservationV1 {
                domain_id: domain(),
                device: first_device.model_key(),
                vm_id: VmIdV1(19),
                kfd_gpu_id: first_correlation.kfd_gpu_id(),
                render_node: first_correlation.render_node(),
                pci: first_correlation.identity().pci,
            },
        )
        .unwrap();
    let memory = MemoryLifecycleStateV1::new_monotonic_non_reusable(domain())
        .next(MemoryTransitionV1::AcquireVm {
            admission: first_vm,
            mapping_devices: vec![first_device],
            handle: UntrustedVmHandleObservationV1(19),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x20_0000,
            },
        })
        .unwrap();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    assert!(
        foundation
            .mint_invariant_certificate(1, second_device, first_vm.model_key())
            .is_err()
    );
    assert!(!foundation.is_certified_for_test());
}

#[test]
fn foundation_certificate_rejects_same_key_cross_state_correlation_substitution() {
    let identity = DeviceIdentityStateV1::new(domain());
    let (identity, state_device) = identity
        .register_device_model_only(correlation(), DeviceGenerationV1(1))
        .unwrap();
    let state_correlation = state_device.correlation();
    let (identity, state_vm) = identity
        .register_vm_model_only(
            state_device,
            UntrustedVmObservationV1 {
                domain_id: domain(),
                device: state_device.model_key(),
                vm_id: VmIdV1(29),
                kfd_gpu_id: state_correlation.kfd_gpu_id(),
                render_node: state_correlation.render_node(),
                pci: state_correlation.identity().pci,
            },
        )
        .unwrap();
    let (_, substituted_device) = DeviceIdentityStateV1::new(domain())
        .register_device_model_only(
            correlation_for(0x6ced_1647_a296_545c, 6),
            DeviceGenerationV1(1),
        )
        .unwrap();
    assert_eq!(state_device.model_key(), substituted_device.model_key());
    assert_ne!(state_device.correlation(), substituted_device.correlation());

    let memory = MemoryLifecycleStateV1::new_monotonic_non_reusable(domain())
        .next(MemoryTransitionV1::AcquireVm {
            admission: state_vm,
            mapping_devices: vec![state_device],
            handle: UntrustedVmHandleObservationV1(29),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x20_0000,
            },
        })
        .unwrap();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    assert!(
        foundation
            .mint_invariant_certificate(1, substituted_device, state_vm.model_key(),)
            .is_err()
    );
    assert!(!foundation.is_certified_for_test());
}

#[test]
fn live_certificate_checks_do_not_repeat_global_validation() {
    let before = queue_foundation_full_validation_count_v1();
    let mut fixture = fixture();
    let issuer = fixture.foundation.issuer().unwrap();
    let vm = fixture.vm.model_key();
    assert_eq!(queue_foundation_full_validation_count_v1(), before + 1);

    for generation in 1..=32 {
        let memory = fixture.foundation.memory().clone();
        fixture
            .foundation
            .replace_memory_after_sealed_transition(memory)
            .unwrap();
        let starting_revision = fixture
            .foundation
            .begin_live_loan(1, fixture.device, vm, issuer, generation)
            .unwrap();
        fixture
            .foundation
            .authenticate_live_loan(1, fixture.device, vm, issuer, generation, starting_revision)
            .unwrap();
    }
    assert_eq!(queue_foundation_full_validation_count_v1(), before + 1);

    fixture
        .foundation
        .validate_full(1, fixture.device, vm, issuer)
        .unwrap();
    assert_eq!(queue_foundation_full_validation_count_v1(), before + 2);
}

#[test]
fn retained_control_revision_reseal_authentication_rejects_identity_and_seal_corruption() {
    let mut fixture = fixture();
    let issuer = fixture.foundation.issuer().unwrap();
    let vm = fixture.vm.model_key();
    fixture
        .foundation
        .set_certificate_revision_for_test(17)
        .unwrap();
    assert_eq!(
        fixture
            .foundation
            .authenticate(1, fixture.device, vm, issuer),
        Ok(())
    );
    let before = fixture.foundation.certificate_snapshot_for_test();
    for corrupt_seal in [false, true] {
        let certificate = fixture.foundation.certificate.as_mut().unwrap();
        if corrupt_seal {
            certificate.revision_seal ^= 1;
        } else {
            certificate.session_id ^= 1;
        }
        assert_eq!(
            fixture
                .foundation
                .authenticate(1, fixture.device, vm, issuer),
            Err("queue foundation certificate binding")
        );
        let certificate = fixture.foundation.certificate.as_mut().unwrap();
        if corrupt_seal {
            certificate.revision_seal ^= 1;
        } else {
            certificate.session_id ^= 1;
        }
        assert_eq!(fixture.foundation.certificate_snapshot_for_test(), before);
        assert_eq!(
            fixture
                .foundation
                .authenticate(1, fixture.device, vm, issuer),
            Ok(())
        );
    }
}

#[test]
fn certificate_revision_capacity_rejects_boundaries_without_model_mutation() {
    let mut fixture = fixture();
    let original = fixture.foundation.memory().clone();

    fixture
        .foundation
        .set_certificate_revision_for_test(u64::MAX - 1)
        .unwrap();
    assert_eq!(
        fixture.foundation.preflight_memory_transition_revisions(2),
        Err("queue foundation certificate revision exhausted")
    );
    assert_eq!(fixture.foundation.memory(), &original);
    fixture.foundation.authenticate_origin().unwrap();

    fixture
        .foundation
        .set_certificate_revision_for_test(u64::MAX)
        .unwrap();
    assert_eq!(
        fixture.foundation.preflight_memory_transition_revisions(1),
        Err("queue foundation certificate revision exhausted")
    );
    assert_eq!(fixture.foundation.memory(), &original);
    fixture.foundation.authenticate_origin().unwrap();

    fixture
        .foundation
        .set_certificate_revision_for_test(u64::MAX - 1)
        .unwrap();
    fixture
        .foundation
        .preflight_memory_transition_revisions(1)
        .unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(original.clone())
        .unwrap();
    fixture.foundation.authenticate_origin().unwrap();
    assert!(
        fixture
            .foundation
            .preflight_memory_transition_revisions(1)
            .is_err()
    );
    assert_eq!(fixture.foundation.memory(), &original);
}

#[test]
fn complete_lifecycle_projects_exact_history_and_releases_only_explicitly() {
    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        success(Mutation::None),
        success(Mutation::None),
    ]);
    let configuration = QueueConfigurationIdV1::from_untrusted_digest(digest(40));
    engine
        .update(
            key,
            configuration,
            admit_kfd_queue_percentage(75).unwrap(),
            admit_kfd_queue_priority(9).unwrap(),
        )
        .unwrap();
    engine.disable(key).unwrap();
    engine.destroy(key).unwrap();
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Destroyed));
    assert_eq!(engine.native_queue_id(key), Some(23));
    assert_eq!(engine.model.queues()[0].configuration, configuration);
    let summary = engine.journal_summary();
    assert_eq!(summary.queues, 1);
    assert_eq!(summary.history, 9);
    assert_eq!(summary.live_publications, 4);
    assert_eq!(summary.ambiguous, 0);
    assert!(!summary.authority_poisoned);
    let calls = engine.backend.calls.borrow();
    assert_eq!(calls.len(), 4);
    assert!(matches!(calls[0], LoggedCall::Create(args) if args.queue_id == u32::MAX));
    assert!(
        matches!(calls[1], LoggedCall::Update(args) if args.queue_id == 23 && args.queue_percentage == 75)
    );
    assert!(
        matches!(calls[2], LoggedCall::Update(args) if args.queue_id == 23 && args.ring_base_address == 0 && args.queue_percentage == 0)
    );
    assert!(matches!(calls[3], LoggedCall::Destroy(args) if args.queue_id == 23));
    drop(calls);
    let _authority = engine.release_destroyed_resources(key).unwrap();
    assert_eq!(engine.journal_summary().live_publications, 0);
    let backend = engine.into_backend().unwrap();
    assert_eq!(backend.calls.borrow().len(), 4);
}

#[test]
fn destroy_rejects_exhausted_release_revision_before_native_call() {
    let (mut engine, key) = active_engine(Vec::new());
    engine
        .foundation
        .set_certificate_revision_for_test(u64::MAX)
        .unwrap();
    let calls_before = engine.backend.calls.borrow().len();
    let memory_before = engine.foundation.memory().clone();

    assert_eq!(
        engine.destroy(key),
        Err(NativeQueueAdapterErrorV1::ModelProjection)
    );
    assert_eq!(engine.backend.calls.borrow().len(), calls_before);
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    assert_eq!(engine.foundation.memory(), &memory_before);
    engine.foundation.authenticate_origin().unwrap();

    let (mut engine, key) = active_engine(vec![success(Mutation::None)]);
    engine
        .foundation
        .set_certificate_revision_for_test(u64::MAX - 1)
        .unwrap();
    engine.destroy(key).unwrap();
    let calls_after_destroy = engine.backend.calls.borrow().len();
    assert!(matches!(
        engine.backend.calls.borrow().last(),
        Some(LoggedCall::Destroy(_))
    ));
    let _authority = engine.release_destroyed_resources(key).unwrap();
    assert_eq!(engine.backend.calls.borrow().len(), calls_after_destroy);
    assert_eq!(engine.journal_summary().live_publications, 0);
    engine.foundation.authenticate_origin().unwrap();
}

#[test]
fn retained_destroy_preserves_native_request_outcome_and_rejects_reentry() {
    for status in [
        QueueSyscallStatusV1::Succeeded,
        QueueSyscallStatusV1::FailedNoEffect,
        QueueSyscallStatusV1::Indeterminate,
    ] {
        for malformed in [false, true] {
            let (mut engine, key) = active_engine(vec![outcome(
                status,
                if malformed {
                    Mutation::DestroyQueueId
                } else {
                    Mutation::None
                },
            )]);
            let mut progress = NativeQueueDestroyProgressV1::default();
            let result = engine.destroy_retaining(key, &mut progress);
            assert_eq!(
                result.is_ok(),
                status == QueueSyscallStatusV1::Succeeded && !malformed
            );
            assert_eq!(
                progress,
                NativeQueueDestroyProgressV1 {
                    started: true,
                    attempted: true,
                    request: Some(KfdIoctlDestroyQueueArgs::new(23)),
                    returned: Some((
                        KfdIoctlDestroyQueueArgs::new(if malformed { 22 } else { 23 }),
                        status
                    ))
                }
            );
            assert!(engine.resource(key).unwrap().authority.is_some());
            let before = progress;
            let calls = engine.backend.calls.borrow().len();
            assert_eq!(
                engine.destroy_retaining(key, &mut progress),
                Err(NativeQueueAdapterErrorV1::InvalidPhase)
            );
            assert_eq!(progress, before);
            assert_eq!(engine.backend.calls.borrow().len(), calls);
        }
    }
}

#[test]
fn retained_destroy_keeps_attempt_on_panic_and_outcome_after_currentness_loss() {
    for panic in [false, true] {
        let (mut engine, key) = active_engine(vec![success(Mutation::None)]);
        engine.backend.panic_destroy = panic;
        if !panic {
            engine.backend.fail_currentness_at = Some(engine.backend.currentness_calls + 2);
        }
        let mut progress = NativeQueueDestroyProgressV1::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.destroy_retaining(key, &mut progress)
        }));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"retained DESTROY panic")
            );
        } else {
            assert_eq!(
                result.unwrap(),
                Err(NativeQueueAdapterErrorV1::Currentness(
                    "scripted currentness loss"
                ))
            );
        }
        assert!(progress.started && progress.attempted);
        assert_eq!(progress.request, Some(KfdIoctlDestroyQueueArgs::new(23)));
        assert_eq!(
            progress.returned,
            (!panic).then_some((
                KfdIoctlDestroyQueueArgs::new(23),
                QueueSyscallStatusV1::Succeeded
            ))
        );
        assert!(engine.resource(key).unwrap().authority.is_some());
        let before = progress;
        let calls = engine.backend.calls.borrow().len();
        assert_eq!(
            engine.destroy_retaining(key, &mut progress),
            Err(NativeQueueAdapterErrorV1::InvalidPhase)
        );
        assert_eq!(progress, before);
        assert_eq!(engine.backend.calls.borrow().len(), calls);
    }
}

#[test]
fn retained_destroy_exhaustion_keeps_all_owners_without_native_attempt() {
    let (mut engine, key) = active_engine(Vec::new());
    engine
        .foundation
        .set_certificate_revision_for_test(u64::MAX)
        .unwrap();
    let mut progress = NativeQueueDestroyProgressV1::default();
    let calls = engine.backend.calls.borrow().len();
    assert_eq!(
        engine.destroy_retaining(key, &mut progress),
        Err(NativeQueueAdapterErrorV1::ModelProjection)
    );
    assert_eq!(
        progress,
        NativeQueueDestroyProgressV1 {
            started: true,
            ..Default::default()
        }
    );
    assert_eq!(engine.backend.calls.borrow().len(), calls);
    assert!(engine.resource(key).unwrap().authority.is_some());
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
}

#[test]
fn admit_revision_exhaustion_retains_authority_in_terminal_engine() {
    let mut fixture = fixture();
    let authority = fixture.authority(10);
    let key = authority.0.plan.queue;
    fixture
        .foundation
        .set_certificate_revision_for_test(u64::MAX)
        .unwrap();
    let memory_before = fixture.foundation.memory().clone();
    let mut engine =
        NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, Vec::new())).unwrap();

    assert_eq!(
        engine.admit(authority),
        Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
    );
    assert!(engine.authority_poisoned);
    assert!(engine.model.queues().is_empty());
    assert_eq!(engine.foundation.memory(), &memory_before);
    assert_eq!(engine.resources.len(), 1);
    assert_eq!(engine.resources[0].key, key);
    assert!(engine.resources[0].authority.is_some());
    assert!(engine.backend.calls.borrow().is_empty());
    engine.foundation.authenticate_origin().unwrap();
}

#[test]
fn two_queue_keys_remain_independently_active_and_destroyable() {
    let mut fixture = fixture();
    let first = fixture.authority(10);
    let second = fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        fixture.foundation,
        vec![
            success(Mutation::CreateId(31)),
            success(Mutation::CreateId(32)),
            success(Mutation::None),
            success(Mutation::None),
        ],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    engine.create(first_key).unwrap();
    engine.create(second_key).unwrap();

    assert_eq!(
        engine.phase(first_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(
        engine.phase(second_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(engine.native_queue_id(first_key), Some(31));
    assert_eq!(engine.native_queue_id(second_key), Some(32));
    assert_eq!(engine.journal_summary().live_publications, 8);

    engine.destroy(second_key).unwrap();
    let _second = engine.release_destroyed_resources(second_key).unwrap();
    assert_eq!(
        engine.phase(first_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(engine.journal_summary().live_publications, 4);
    engine.destroy(first_key).unwrap();
    let _first = engine.release_destroyed_resources(first_key).unwrap();
    assert_eq!(engine.journal_summary().live_publications, 0);
    engine.into_backend().unwrap();
}

#[test]
fn backend_return_rejects_live_or_unreleased_queue_resources() {
    let (engine, _) = active_engine(Vec::new());
    assert_eq!(
        engine.into_backend().err().unwrap(),
        NativeQueueAdapterErrorV1::InvalidPhase
    );

    let fixture = fixture();
    let engine =
        NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, Vec::new())).unwrap();
    assert_eq!(
        engine.into_backend().err().unwrap(),
        NativeQueueAdapterErrorV1::InvalidPhase
    );
}

#[test]
fn queue_id_zero_and_positive_max_profile_id_are_both_admitted() {
    for (queue_id, mutation) in [
        (0, Mutation::CreateZero),
        (
            KFD_MAX_QUEUE_SLOTS_PER_PROCESS - 1,
            Mutation::CreateId(KFD_MAX_QUEUE_SLOTS_PER_PROCESS - 1),
        ),
    ] {
        let mut fixture = fixture();
        let authority = fixture.authority(10);
        let key = authority.0.plan.queue;
        let backend = FakeBackend::new(fixture.foundation, vec![success(mutation)]);
        let mut engine = NativeQueueEngineV1::new(backend).unwrap();
        engine.admit(authority).unwrap();
        engine.create(key).unwrap();
        assert_eq!(engine.native_queue_id(key), Some(queue_id));
        assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    }
}

#[test]
fn create_errno_semantics_and_malformed_outputs_fail_closed() {
    let cases = [
        (
            outcome(QueueSyscallStatusV1::Indeterminate, Mutation::None),
            NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Create),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::None),
            NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Create),
            ComputeAqlQueuePhaseV1::Planned,
        ),
        (
            success(Mutation::None),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateId(KFD_MAX_QUEUE_SLOTS_PER_PROCESS)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(0)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(
                28_851,
                KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES,
            ))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(28_852, 8))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(28_851, 1))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateRingSize),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE immutable inputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::CreateId(11)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE failed-no-effect outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
    ];
    for (script, expected_error, expected_phase) in cases {
        let must_poison = matches!(
            &expected_error,
            NativeQueueAdapterErrorV1::MalformedKernelResult(_, _)
        );
        let mut fixture = fixture();
        let authority = fixture.authority(10);
        let key = authority.0.plan.queue;
        let mut engine =
            NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, vec![script])).unwrap();
        engine.admit(authority).unwrap();
        assert_eq!(engine.create(key), Err(expected_error));
        assert_eq!(engine.phase(key), Some(expected_phase));
        assert_eq!(engine.journal_summary().live_publications, 4);
        assert_eq!(engine.journal_summary().authority_poisoned, must_poison);
        assert!(engine.release_destroyed_resources(key).is_err());
    }
}

#[test]
fn every_noncreate_operation_classifies_failure_and_mutation_conservatively() {
    for status in [
        QueueSyscallStatusV1::FailedNoEffect,
        QueueSyscallStatusV1::Indeterminate,
    ] {
        let (mut engine, key) = active_engine(vec![outcome(status, Mutation::None)]);
        let error = engine
            .update(
                key,
                QueueConfigurationIdV1::from_untrusted_digest(digest(42)),
                admit_kfd_queue_percentage(50).unwrap(),
                admit_kfd_queue_priority(4).unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Update)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Update)
            }
        );
        assert_eq!(
            engine.phase(key),
            Some(if status == QueueSyscallStatusV1::FailedNoEffect {
                ComputeAqlQueuePhaseV1::Active
            } else {
                ComputeAqlQueuePhaseV1::Ambiguous
            })
        );

        let (mut engine, key) = active_engine(vec![outcome(status, Mutation::None)]);
        let error = engine.disable(key).unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Disable)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Disable)
            }
        );

        let (mut engine, key) = active_engine(vec![
            success(Mutation::None),
            outcome(status, Mutation::None),
        ]);
        engine.disable(key).unwrap();
        let error = engine.destroy(key).unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Destroy)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Destroy)
            }
        );
        assert_eq!(engine.journal_summary().live_publications, 4);
    }

    let (mut engine, key) = active_engine(vec![success(Mutation::UpdateQueueId)]);
    assert!(matches!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(44)),
            admit_kfd_queue_percentage(50).unwrap(),
            admit_kfd_queue_priority(4).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
            NativeQueueOperationV1::Update,
            _
        ))
    ));
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));

    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        success(Mutation::DestroyQueueId),
    ]);
    engine.disable(key).unwrap();
    assert!(matches!(
        engine.destroy(key),
        Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
            NativeQueueOperationV1::Destroy,
            _
        ))
    ));
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
}

#[test]
fn update_from_disabled_preserves_exact_resume_phase_on_no_effect() {
    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::None),
    ]);
    engine.disable(key).unwrap();
    let original = engine.model.queues()[0].configuration;
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Disabled));
    assert_eq!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(61)),
            admit_kfd_queue_percentage(90).unwrap(),
            admit_kfd_queue_priority(6).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::BackendFailedNoEffect(
            NativeQueueOperationV1::Update
        ))
    );
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Disabled));
    assert_eq!(engine.model.queues()[0].configuration, original);

    let (mut engine, key) = active_engine(vec![success(Mutation::None), success(Mutation::None)]);
    engine.disable(key).unwrap();
    let next = QueueConfigurationIdV1::from_untrusted_digest(digest(62));
    engine
        .update(
            key,
            next,
            admit_kfd_queue_percentage(90).unwrap(),
            admit_kfd_queue_priority(6).unwrap(),
        )
        .unwrap();
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    assert_eq!(engine.model.queues()[0].configuration, next);
}

#[test]
fn cumulative_history_capacity_rejects_before_currentness_or_ioctl() {
    const COMPLETED_UPDATES: usize = 126;
    let tail = vec![success(Mutation::None); COMPLETED_UPDATES];
    let (mut engine, key) = active_engine(tail);
    for index in 0..COMPLETED_UPDATES {
        engine
            .update(
                key,
                QueueConfigurationIdV1::from_untrusted_digest(digest(80 + index as u8)),
                admit_kfd_queue_percentage(50).unwrap(),
                admit_kfd_queue_priority(4).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(engine.model.history().len(), 255);
    let calls = engine.backend.calls.borrow().len();
    let currentness_calls = engine.backend.currentness_calls;
    let bootstrap_calls = engine.backend.bootstrap_calls.borrow().clone();
    let summary = engine.journal_summary();
    let resources = (engine.resources.as_ptr(), engine.resources.len());
    engine.backend.bootstrap_fault = Some((BootstrapCallV1::Opener, true));
    for _ in 0..2 {
        assert_eq!(
            engine.preflight_operation(),
            Err(NativeQueueAdapterErrorV1::JournalCapacity)
        );
        assert_eq!(engine.journal_summary(), summary);
        assert_eq!(
            (engine.resources.as_ptr(), engine.resources.len()),
            resources
        );
    }
    assert_eq!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(79)),
            admit_kfd_queue_percentage(50).unwrap(),
            admit_kfd_queue_priority(4).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::JournalCapacity)
    );
    assert_eq!(engine.backend.calls.borrow().len(), calls);
    assert_eq!(engine.backend.currentness_calls, currentness_calls);
    assert_eq!(*engine.backend.bootstrap_calls.borrow(), bootstrap_calls);
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    assert!(!engine.authority_poisoned);
    engine.authority_poisoned = true;
    assert_eq!(
        engine.preflight_operation(),
        Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
    );
    assert_eq!(*engine.backend.bootstrap_calls.borrow(), bootstrap_calls);
}

#[test]
fn borrowed_operation_preflight_reserves_retained_queue_history_without_observation() {
    const COMPLETED_UPDATES: usize = 125;
    let (mut engine, key) = active_engine(vec![success(Mutation::None); COMPLETED_UPDATES]);
    for index in 0..COMPLETED_UPDATES {
        engine
            .update(
                key,
                QueueConfigurationIdV1::from_untrusted_digest(digest(80 + index as u8)),
                admit_kfd_queue_percentage(50).unwrap(),
                admit_kfd_queue_priority(4).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(engine.model.history().len(), 253);
    let calls = engine.backend.calls.borrow().len();
    let currentness_calls = engine.backend.currentness_calls;
    let bootstrap_calls = engine.backend.bootstrap_calls.borrow().clone();
    let resources = (engine.resources.as_ptr(), engine.resources.len());
    engine.backend.bootstrap_fault = Some((BootstrapCallV1::Opener, true));
    assert_eq!(engine.preflight_operation(), Ok(()));
    engine
        .begin(QueueTransitionV1::BeginUpdate {
            queue: key,
            configuration: QueueConfigurationIdV1::from_untrusted_digest(digest(79)),
        })
        .unwrap();
    assert_eq!(engine.model.history().len(), 254);
    let summary = engine.journal_summary();
    assert_eq!(
        engine.preflight_operation(),
        Err(NativeQueueAdapterErrorV1::JournalCapacity)
    );
    assert_eq!(engine.journal_summary(), summary);
    assert_eq!(
        (engine.resources.as_ptr(), engine.resources.len()),
        resources
    );
    assert_eq!(engine.backend.calls.borrow().len(), calls);
    assert_eq!(engine.backend.currentness_calls, currentness_calls);
    assert_eq!(*engine.backend.bootstrap_calls.borrow(), bootstrap_calls);
    assert!(!engine.authority_poisoned);
}
