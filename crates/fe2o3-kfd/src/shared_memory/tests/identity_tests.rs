use super::*;

#[test]
fn compute_coexistence_identity_extraction_preserves_incarnations() {
    let mut identity = local_mapping_for_persistent_sdma_test(41).storage_identity();
    let facts = identity.coexistence_facts_v1().unwrap();
    assert_eq!(facts.allocation_id, 41);
    assert_eq!(facts.generation, 1);
    assert_eq!(facts.physical_device, identity.vm.device.physical.0);
    identity.generation = 2;
    assert_eq!(identity.coexistence_facts_v1().unwrap().generation, 2);
    identity.device.generation.0 += 1;
    assert!(identity.coexistence_facts_v1().is_none());
    let host = mapped_host_for_persistent_sdma_test(1, 4096).storage_identity();
    let mut other = host;
    other.id = 2;
    assert!(host.same_retained_session_v1(other));
    other.session_id += 1;
    assert!(!host.same_retained_session_v1(other));
    other = host;
    other.generation = 0;
    assert!(!host.same_retained_session_v1(other));
}

#[test]
fn retained_device_scope_domain_rejects_vm_and_device_generation_substitutions() {
    let (_, _, device, vm) = transferred_model_foundation();
    assert!(retained_device_domain_matches_v1(vm, vm, device, device));
    let mut wrong_vm = vm;
    wrong_vm.id = VmIdV1(vm.id.0 + 1);
    assert!(!retained_device_domain_matches_v1(
        wrong_vm, vm, device, device
    ));
    wrong_vm = vm;
    wrong_vm.device.generation = model::DeviceGenerationV1(2);
    assert!(!retained_device_domain_matches_v1(
        wrong_vm, wrong_vm, device, device
    ));
    let (_, replacement) = model::DeviceIdentityStateV1::new(model_domain())
        .register_device_model_only(model_correlation(), model::DeviceGenerationV1(2))
        .unwrap();
    assert!(!retained_device_domain_matches_v1(
        vm,
        vm,
        device,
        replacement
    ));
    assert!(!retained_device_domain_matches_v1(
        vm,
        vm,
        replacement,
        replacement
    ));
}

#[test]
fn shared_profile_manifest_is_frozen() {
    let digest = Sha256::digest(SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1);
    let mut digest_hex = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.iter().copied() {
        digest_hex.push(char::from(HEX[usize::from(byte >> 4)]));
        digest_hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    assert_eq!(digest_hex, SHARED_GTT_MEMORY_PROFILE_SHA256_V1);
    assert_eq!(digest.as_slice(), SHARED_GTT_MEMORY_PROFILE_SHA256_BYTES_V1);
    assert!(
        SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1
            .contains(fe2o3_kfd_uapi::KFD_USERPTR_MEMORY_SCHEMA_MANIFEST_SHA256)
    );
    assert!(
        SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1
            .contains(fe2o3_kfd_uapi::KFD_USERPTR_QUEUE_CONTROL_SCHEMA_MANIFEST_SHA256)
    );
}

#[test]
fn live_queue_model_loan_requires_the_exact_move_only_token() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let issuer = foundation
        .mint_invariant_certificate(7, device, vm)
        .unwrap();
    let mut ownership = QueueModelOwnershipV1::new();
    ownership.transfer_to_queue(issuer).unwrap();
    let loan = ownership
        .begin_live_loan(7, &mut foundation, device, vm)
        .unwrap();

    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::SessionOwnedLiveLoan {
            issuer,
            generation: 1,
        }
    );
    assert!(ownership.transfer_to_queue(issuer).is_err());
    assert!(
        ownership
            .begin_live_loan(7, &mut foundation, device, vm)
            .is_err()
    );
    assert!(ownership.restore_to_session(issuer).is_err());
    assert!(
        ownership
            .finish_live_loan(
                7,
                &foundation,
                device,
                vm,
                LiveQueueModelFoundationLoanV1 {
                    session_id: 8,
                    issuer,
                    generation: 1,
                    starting_revision: loan.starting_revision,
                },
            )
            .is_err()
    );
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::SessionOwnedLiveLoan {
            issuer,
            generation: 1,
        }
    );

    ownership
        .finish_live_loan(7, &foundation, device, vm, loan)
        .unwrap();
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::QueueOwned { issuer }
    );
    ownership.restore_to_session(issuer).unwrap();
    assert_eq!(ownership.phase, QueueModelOwnershipPhaseV1::SessionOwned);
}

#[test]
fn failed_live_mutation_retains_one_reclaimable_model_loan() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let issuer = foundation
        .mint_invariant_certificate(11, device, vm)
        .unwrap();
    let mut ownership = QueueModelOwnershipV1::new();
    ownership.transfer_to_queue(issuer).unwrap();
    let loan = ownership
        .begin_live_loan(11, &mut foundation, device, vm)
        .unwrap();

    // A concrete mutation failure leaves the unique foundation session-owned.
    assert!(ownership.transfer_to_queue(issuer).is_err());
    assert!(ownership.restore_to_session(issuer).is_err());
    ownership
        .finish_live_loan(11, &foundation, device, vm, loan)
        .unwrap();

    let next = ownership
        .begin_live_loan(11, &mut foundation, device, vm)
        .unwrap();
    assert_eq!(next.generation, 2);
    ownership
        .finish_live_loan(11, &foundation, device, vm, next)
        .unwrap();
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::QueueOwned { issuer }
    );
}

#[test]
fn wrong_transfer_phase_does_not_mint_or_mutate_foundation_certificate() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let mut ownership = QueueModelOwnershipV1::new();
    ownership.transfer_to_queue(77).unwrap();

    assert!(
        ownership
            .certify_and_transfer_to_queue(&mut foundation, 31, device, vm)
            .is_err()
    );
    assert!(!foundation.is_certified_for_test());
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::QueueOwned { issuer: 77 }
    );
}

#[test]
fn final_restore_revokes_certificate_and_allows_fresh_queue_transfer() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let domain = memory.domain_id();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let mut session_foundation = QueueModelFoundationV1::empty(domain);
    let mut ownership = QueueModelOwnershipV1::new();
    let first = ownership
        .certify_and_transfer_to_queue(&mut foundation, 37, device, vm)
        .unwrap();
    let first_loan = ownership
        .loan_foundation(37, &mut session_foundation, &mut foundation, device, vm)
        .unwrap();
    let stale_first_loan = LiveQueueModelFoundationLoanV1 {
        session_id: first_loan.session_id,
        issuer: first_loan.issuer,
        generation: first_loan.generation,
        starting_revision: first_loan.starting_revision,
    };
    ownership
        .reclaim_foundation(
            37,
            &mut session_foundation,
            &mut foundation,
            device,
            vm,
            first_loan,
        )
        .unwrap();
    foundation.validate_full(37, device, vm, first).unwrap();
    ownership.restore_to_session(first).unwrap();
    foundation
        .revoke_invariant_certificate(37, device, vm, first)
        .unwrap();
    assert!(!foundation.is_certified_for_test());

    let second = ownership
        .certify_and_transfer_to_queue(&mut foundation, 37, device, vm)
        .unwrap();
    assert_ne!(first, second);
    let second_loan = ownership
        .loan_foundation(37, &mut session_foundation, &mut foundation, device, vm)
        .unwrap();
    assert_eq!(second_loan.generation, 1);
    assert_eq!(second_loan.issuer, second);
    assert!(
        ownership
            .reclaim_foundation(
                37,
                &mut session_foundation,
                &mut foundation,
                device,
                vm,
                stale_first_loan,
            )
            .is_err()
    );
    ownership
        .reclaim_foundation(
            37,
            &mut session_foundation,
            &mut foundation,
            device,
            vm,
            second_loan,
        )
        .unwrap();
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::QueueOwned { issuer: second }
    );
}

#[test]
fn n2_constructor_default_has_no_configuration_effects() {
    let mut fixture = BackingConstructorFixture::new(None);
    let legacy = acquired();
    assert!(fixture.usage().is_none());
    assert_eq!(
        fixture.engine.backend.currentness_calls,
        legacy.backend.currentness_calls
    );
    assert_eq!(fixture.engine.backend.operations, legacy.backend.operations);
    assert!(!fixture.engine.device_backing_configuration_closed);
    assert!(!fixture.engine.device_backing_activity_started);
    let authority = fixture.mapped_device();
    let mut queue = fixture.transfer(&[&authority]).unwrap();
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
    assert!(fixture.usage().is_none());
    assert_eq!(fixture.engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn n2_constructor_both_orders_preserve_exact_charge_through_real_queue_loan() {
    let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
    for compute_first in [false, true] {
        let mut fixture = BackingConstructorFixture::new(Some(budget));
        let mut authority = compute_first.then(|| fixture.mapped_device());
        let mut queue = if let Some(authority) = authority.as_ref() {
            fixture.transfer(&[authority]).unwrap()
        } else {
            fixture.transfer(&[]).unwrap()
        };
        let queue_usage = fixture.usage();
        let calls = fixture.engine.backend.currentness_calls;
        assert!(fixture.configure(Some(budget)).is_err());
        assert_eq!(fixture.engine.backend.currentness_calls, calls);
        assert_eq!(fixture.usage(), queue_usage);
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
        assert!(fixture.configure(Some(budget)).is_err());
        if !compute_first {
            authority = Some(fixture.mapped_device());
        }
        let account = fixture.engine.device_backing_account.as_ref().unwrap();
        let record = &fixture.engine.device_memory[0];
        assert!(record.backing_charge.as_ref().unwrap().matches(
            account,
            fixture.engine.session_id,
            fixture.device.model_key(),
            fixture.vm,
            record.id,
            record.generation,
            record.layout,
        ));
        let retained = fixture.usage().unwrap();
        assert_eq!(retained.budget, budget);
        assert_eq!(retained.used_backing_bytes, 4096);
        assert_eq!(retained.used_allocation_records, 1);
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
        assert_eq!(fixture.usage(), Some(retained));
        fixture
            .ownership
            .restore_foundation(
                &mut fixture.engine,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        assert!(!fixture.foundation.is_certified_for_test());
        assert!(fixture.ownership.is_session_owned());
        assert!(fixture.engine.device_backing_configuration_closed);
        assert!(fixture.configure(Some(budget)).is_err());
        assert_eq!(fixture.usage(), Some(retained));
        let unmapped = fixture
            .engine
            .unmap_device_memory(authority.unwrap().lease)
            .unwrap();
        fixture.engine.release_device_memory(unmapped).unwrap();
        let disposed = fixture.usage().unwrap();
        assert_eq!(disposed.budget, budget);
        assert_eq!(disposed.used_backing_bytes, 0);
        assert_eq!(disposed.used_allocation_records, 0);
        assert!(fixture.configure(Some(budget)).is_err());
    }
}

#[test]
fn n2_constructor_unconfigured_transfer_cannot_reopen_configuration_after_restore() {
    let mut fixture = BackingConstructorFixture::new(None);
    let budget = Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap();
    let mut queue = fixture.transfer(&[]).unwrap();
    assert!(fixture.configure(Some(budget)).is_err());
    fixture
        .ownership
        .restore_foundation(
            &mut fixture.engine,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    let calls = fixture.engine.backend.currentness_calls;
    assert!(fixture.configure(Some(budget)).is_err());
    assert_eq!(fixture.engine.backend.currentness_calls, calls);
    assert!(fixture.usage().is_none());
    assert_eq!(fixture.engine.backend.reserve_va_calls, 0);
}

#[test]
fn n2_constructor_configuration_rejects_foreign_currentness_and_prior_activity() {
    let budget = Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap();
    for case in 0..4 {
        let mut fixture = BackingConstructorFixture::new(None);
        match case {
            0 => fixture.vm.device.generation.0 += 1,
            1 => {
                fixture.engine.backend.fail_currentness_at =
                    Some(fixture.engine.backend.currentness_calls + 1);
            }
            2 => {
                fixture.configure(Some(budget)).unwrap();
            }
            3 => {
                let lease = fixture
                    .engine
                    .allocate_device_memory(fixture.device.model_key(), fixture.vm, 17, 4)
                    .unwrap();
                fixture.engine.release_device_memory(lease).unwrap();
            }
            _ => unreachable!(),
        }
        let usage = fixture.usage();
        let calls = (
            fixture.engine.backend.currentness_calls,
            fixture.engine.backend.reserve_va_calls,
            fixture.engine.backend.alloc_calls,
        );
        assert!(fixture.configure(Some(budget)).is_err());
        assert_eq!(fixture.usage(), usage);
        assert_eq!(
            fixture.engine.backend.currentness_calls,
            calls.0 + usize::from(case == 1)
        );
        assert_eq!(fixture.engine.backend.reserve_va_calls, calls.1);
        assert_eq!(fixture.engine.backend.alloc_calls, calls.2);
    }
}

#[test]
fn n2_constructor_partial_native_failure_retains_account_in_both_orders() {
    let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
    for compute_first in [false, true] {
        for (operation, panic) in [
            ("reserve_va", false),
            ("alloc", false),
            ("reserve_va", true),
            ("alloc", true),
        ] {
            let mut fixture = BackingConstructorFixture::new(Some(budget));
            let mut queue = (!compute_first).then(|| fixture.transfer(&[]).unwrap());
            let loan = queue.as_mut().map(|queue| {
                fixture
                    .ownership
                    .loan_foundation(
                        fixture.engine.session_id,
                        &mut fixture.foundation,
                        queue,
                        fixture.device,
                        fixture.vm,
                    )
                    .unwrap()
            });
            if panic {
                fixture.engine.backend.panic_operation = Some(operation);
            } else {
                fixture.engine.backend.fail_operation = Some(operation);
            }
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fixture
                    .engine
                    .allocate_device_memory(fixture.device.model_key(), fixture.vm, 17, 4)
            }));
            if panic {
                let payload = outcome.expect_err("native constructor panic must escape");
                assert_eq!(
                    payload.downcast_ref::<(&'static str, &'static str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(outcome.unwrap().is_err());
            }
            assert_eq!(
                fixture.engine.phase(),
                SharedMemorySessionPhaseV1::Quarantined
            );
            let failed = fixture.usage().unwrap();
            assert_eq!(failed.used_backing_bytes, 4096);
            assert_eq!(failed.used_allocation_records, 1);
            if let Some(loan) = loan {
                fixture
                    .ownership
                    .reclaim_foundation(
                        fixture.engine.session_id,
                        &mut fixture.foundation,
                        queue.as_mut().unwrap(),
                        fixture.device,
                        fixture.vm,
                        loan,
                    )
                    .unwrap();
            }
            let reserve_calls = fixture.engine.backend.reserve_va_calls;
            assert!(
                fixture
                    .engine
                    .allocate_device_memory(fixture.device.model_key(), fixture.vm, 17, 4,)
                    .is_err()
            );
            assert_eq!(fixture.engine.backend.reserve_va_calls, reserve_calls);
            assert_eq!(fixture.engine.backend.free_calls, 0);
            assert_eq!(fixture.engine.backend.release_va_calls, 0);
            let account = fixture.engine.device_backing_account.take().unwrap();
            drop(fixture);
            assert_eq!(account.usage().used_backing_bytes, 4096);
            assert_eq!(account.usage().used_allocation_records, 1);
            assert_eq!(account.usage().quarantined_records, 1);
        }
    }
}

#[test]
fn n2_constructor_foreign_restore_retains_charge_and_quarantines_session() {
    let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
    let mut fixture = BackingConstructorFixture::new(Some(budget));
    let authority = fixture.mapped_device();
    let mut queue = fixture.transfer(&[&authority]).unwrap();
    let retained = fixture.usage();
    let mut foreign_vm = fixture.vm;
    foreign_vm.id.0 += 1;
    assert!(
        fixture
            .ownership
            .restore_foundation(
                &mut fixture.engine,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                foreign_vm,
            )
            .is_err()
    );
    assert_eq!(fixture.usage(), retained);
    assert_eq!(
        fixture.engine.phase(),
        SharedMemorySessionPhaseV1::Quarantined
    );
    assert!(!fixture.ownership.is_session_owned());
    assert!(fixture.configure(Some(budget)).is_err());
    assert_eq!(fixture.engine.backend.free_calls, 0);
    assert_eq!(fixture.engine.backend.release_va_calls, 0);
    let account = fixture.engine.device_backing_account.take().unwrap();
    drop(fixture);
    assert_eq!(account.usage().used_backing_bytes, 4096);
    assert_eq!(account.usage().quarantined_records, 1);
}

#[test]
fn live_foundation_loan_reclaims_concrete_allocation_lifecycle_updates() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let domain = memory.domain_id();
    let mut queue_foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let issuer = queue_foundation
        .mint_invariant_certificate(17, device, vm)
        .unwrap();
    let mut session_foundation = QueueModelFoundationV1::empty(domain);
    let mut ownership = QueueModelOwnershipV1::new();
    ownership.transfer_to_queue(issuer).unwrap();

    let loan = ownership
        .loan_foundation(
            17,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
        )
        .unwrap();
    assert_eq!(session_foundation.identity().devices().len(), 1);
    assert!(queue_foundation.identity().devices().is_empty());

    let (reservation, allocation, mapping) = model_keys(vm, 41, 1);
    let layout = profile_layout::<HostVisibleCoherentGttV1>(4096).unwrap();
    let model = project_allocation(
        session_foundation.memory(),
        reservation,
        allocation,
        0x2_0000,
        layout,
        41,
        MemoryKindV1::HostVisibleCoherent,
    )
    .unwrap();
    session_foundation
        .replace_memory_after_sealed_transition(model)
        .unwrap();
    let model = project_map(session_foundation.memory(), mapping, device).unwrap();
    session_foundation
        .replace_memory_after_sealed_transition(model)
        .unwrap();
    ownership
        .reclaim_foundation(
            17,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
            loan,
        )
        .unwrap();
    assert_eq!(queue_foundation.identity().devices().len(), 1);
    assert!(session_foundation.identity().devices().is_empty());
    assert_eq!(
        queue_foundation.memory().mappings()[0].state,
        model::MemoryMappingStateV1::Mapped
    );

    let loan = ownership
        .loan_foundation(
            17,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
        )
        .unwrap();
    let model = project_unmap(session_foundation.memory(), mapping).unwrap();
    session_foundation
        .replace_memory_after_sealed_transition(model)
        .unwrap();
    let model = project_release(
        session_foundation.memory(),
        reservation,
        allocation,
        mapping,
    )
    .unwrap();
    session_foundation
        .replace_memory_after_sealed_transition(model)
        .unwrap();
    ownership
        .reclaim_foundation(
            17,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
            loan,
        )
        .unwrap();
    assert_eq!(
        queue_foundation.memory().mappings()[0].state,
        model::MemoryMappingStateV1::Released
    );
    assert_eq!(
        queue_foundation.memory().allocations()[0].state,
        model::MemoryAllocationStateV1::Released
    );
    assert_eq!(
        queue_foundation.memory().reservations()[0].state,
        model::VaReservationStateV1::Released
    );
}

#[test]
fn certified_native_memory_effects_preflight_exact_revision_budgets() {
    for (revision, needed) in [(u64::MAX - 1, 2), (u64::MAX, 1)] {
        let (identity, memory, device, vm) = transferred_model_foundation();
        let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
        let issuer = foundation
            .mint_invariant_certificate(59, device, vm)
            .unwrap();
        foundation
            .set_certificate_revision_for_test(revision)
            .unwrap();
        let process_poisoned = Cell::new(false);
        let mut engine = acquired();
        let operations_before = engine.backend.operations.clone();
        let failure = preflight_queue_foundation_native_memory_transition_v1(
            &foundation,
            &mut engine,
            needed,
            || process_poisoned.set(true),
        );
        assert!(matches!(
            failure,
            Err(MemorySessionError::Model(
                "queue foundation certificate revision exhausted"
            ))
        ));
        assert_eq!(engine.backend.operations, operations_before);
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(process_poisoned.get());
        assert_eq!(foundation.memory().validate_global_invariants(), Ok(()));
        foundation.authenticate(59, device, vm, issuer).unwrap();
    }

    let source = include_str!("../../shared_memory.rs");
    // Allocation and mapping now use the production transition helper;
    // its revision/native-effect ordering is exercised by transitions tests.
    for (start, end, helper) in [
        (
            "fn allocate_profile<P:",
            "pub fn with_bytes<",
            "transitions::allocate_v1",
        ),
        (
            "pub fn map_to_gpu<P:",
            "pub fn map_executable_to_gpu",
            "transitions::map_mutable_v1",
        ),
        (
            "pub fn map_executable_to_gpu",
            "pub fn unmap_from_gpu<P:",
            "transitions::map_executable_v1",
        ),
    ] {
        let body = source
            .split(start)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap();
        assert!(body.contains(helper), "{start} must delegate to {helper}");
    }
    let cases = [
        (
            "pub fn unmap_from_gpu<P:",
            "pub fn unmap_executable_from_gpu",
            1,
            "self.engine.unmap_mutable",
        ),
        (
            "pub fn unmap_executable_from_gpu",
            "pub fn release<P:",
            1,
            "self.engine.unmap_executable",
        ),
        (
            "fn release_with_phase<P:",
            "fn commit_unmap_projection",
            1,
            "self.engine.release",
        ),
    ];
    for (start, end, budget, native) in cases {
        let body = source
            .split(start)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap();
        let preflight = body
            .find(&format!(
                "preflight_native_memory_transition_revisions({budget})"
            ))
            .unwrap();
        let native = body.find(native).unwrap();
        assert!(preflight < native, "{start} must preflight before {native}");
    }
}

#[test]
fn live_foundation_loan_survives_operation_and_retake_rejection_without_loss() {
    let (identity, memory, device, vm) = transferred_model_foundation();
    let expected = memory.clone();
    let domain = memory.domain_id();
    let mut queue_foundation = QueueModelFoundationV1::uncertified(identity, memory);
    let issuer = queue_foundation
        .mint_invariant_certificate(23, device, vm)
        .unwrap();
    let mut session_foundation = QueueModelFoundationV1::empty(domain);
    let mut ownership = QueueModelOwnershipV1::new();
    ownership.transfer_to_queue(issuer).unwrap();

    let loan = ownership
        .loan_foundation(
            23,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
        )
        .unwrap();
    let (_, _, missing_mapping) = model_keys(vm, 99, 1);
    assert!(project_unmap(session_foundation.memory(), missing_mapping).is_err());
    assert!(
        ownership
            .reclaim_foundation(
                23,
                &mut session_foundation,
                &mut queue_foundation,
                device,
                vm,
                LiveQueueModelFoundationLoanV1 {
                    session_id: 24,
                    issuer,
                    generation: loan.generation,
                    starting_revision: loan.starting_revision,
                },
            )
            .is_err()
    );
    assert_eq!(session_foundation.identity().devices().len(), 1);
    assert!(queue_foundation.identity().devices().is_empty());

    ownership
        .reclaim_foundation(
            23,
            &mut session_foundation,
            &mut queue_foundation,
            device,
            vm,
            loan,
        )
        .unwrap();
    assert_eq!(queue_foundation.memory(), &expected);
    assert_eq!(queue_foundation.identity().devices().len(), 1);
    assert!(session_foundation.identity().devices().is_empty());
    assert_eq!(
        ownership.phase,
        QueueModelOwnershipPhaseV1::QueueOwned { issuer }
    );
}

#[test]
fn panic_before_and_after_allocation_map_projection_remains_retakeable() {
    for completed_steps in 0..=2 {
        let (identity, memory, device, vm) = transferred_model_foundation();
        let domain = memory.domain_id();
        let mut queue_foundation = QueueModelFoundationV1::uncertified(identity, memory);
        let issuer = queue_foundation
            .mint_invariant_certificate(43, device, vm)
            .unwrap();
        let mut session_foundation = QueueModelFoundationV1::empty(domain);
        let mut ownership = QueueModelOwnershipV1::new();
        ownership.transfer_to_queue(issuer).unwrap();
        let mut retained_loan = None;
        let (reservation, allocation, mapping) = model_keys(vm, 51 + completed_steps, 1);
        let layout = profile_layout::<HostVisibleCoherentGttV1>(4096).unwrap();

        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            retained_loan = Some(
                ownership
                    .loan_foundation(
                        43,
                        &mut session_foundation,
                        &mut queue_foundation,
                        device,
                        vm,
                    )
                    .unwrap(),
            );
            if completed_steps >= 1 {
                let model = project_allocation(
                    session_foundation.memory(),
                    reservation,
                    allocation,
                    0x3_0000,
                    layout,
                    51 + completed_steps,
                    MemoryKindV1::HostVisibleCoherent,
                )
                .unwrap();
                session_foundation
                    .replace_memory_after_sealed_transition(model)
                    .unwrap();
            }
            if completed_steps >= 2 {
                let model = project_map(session_foundation.memory(), mapping, device).unwrap();
                session_foundation
                    .replace_memory_after_sealed_transition(model)
                    .unwrap();
            }
            std::panic::panic_any(("injected live model panic", completed_steps));
        }))
        .expect_err("injected panic must escape the model mutation body");
        assert_eq!(
            caught.downcast_ref::<(&'static str, u64)>(),
            Some(&("injected live model panic", completed_steps))
        );
        ownership
            .reclaim_foundation(
                43,
                &mut session_foundation,
                &mut queue_foundation,
                device,
                vm,
                retained_loan.take().unwrap(),
            )
            .unwrap();
        assert!(
            queue_foundation
                .memory()
                .validate_global_invariants()
                .is_ok()
        );
        assert_eq!(
            queue_foundation.memory().allocations().len(),
            usize::from(completed_steps >= 1)
        );
        assert_eq!(
            queue_foundation.memory().mappings().len(),
            usize::from(completed_steps >= 2)
        );
    }
}
