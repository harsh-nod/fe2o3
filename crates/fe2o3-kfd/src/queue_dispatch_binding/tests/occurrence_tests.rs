use super::*;

#[test]
fn r66_published_occurrence_observation_preserves_digest_and_owner_state() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let identity = owner.reserve(queue, test_completion_roster_v1(1)).unwrap();
    let completion = test_completion_occurrence_v1(1);
    assert!(
        r66_retained_published_occurrence_observation_v1(&owner, identity, completion).is_none()
    );
    owner.mark_published(identity, completion).unwrap();
    let snapshot = owner.clone();
    // Preserve the original R66 byte sequence, including its domain separator.
    let mut prior_hash = Sha256::new();
    prior_hash.update(b"fe2o3.r66.retained-persistent-dispatch.v1\0");
    prior_hash.update(completion.roster_sha256);
    prior_hash.update(identity.recipe_occurrence.to_le_bytes());
    prior_hash.update([u8::try_from(identity.slot_index).unwrap()]);
    prior_hash.update(identity.slot_generation.to_le_bytes());
    prior_hash.update(identity.dispatch_generation.to_le_bytes());
    let expected: [u8; 32] = prior_hash.finalize().into();
    for _ in 0..8 {
        assert_eq!(
            r66_retained_published_occurrence_observation_v1(&owner, identity, completion),
            Some(expected)
        );
        assert_eq!(owner, snapshot);
    }
    owner.complete_epoch(identity, completion).unwrap();
    let completed = owner.clone();
    assert!(
        r66_retained_published_occurrence_observation_v1(&owner, identity, completion).is_none()
    );
    assert_eq!(owner, completed);
    owner.recycle_epoch(identity, completion).unwrap();
    let recycled = owner.clone();
    assert!(
        r66_retained_published_occurrence_observation_v1(&owner, identity, completion).is_none()
    );
    assert_eq!(owner, recycled);
    owner.poison();
    let poisoned = owner.clone();
    assert!(
        r66_retained_published_occurrence_observation_v1(&owner, identity, completion).is_none()
    );
    assert_eq!(owner, poisoned);
}

#[test]
fn exhausted_vacant_slot_is_skipped_without_mutating_failed_admission() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    owner.slots[0].slot_generation = u64::MAX;
    let identity = owner.reserve(queue, test_completion_roster_v1(1)).unwrap();
    assert_eq!(identity.slot_index, 1);
    assert_eq!(identity.slot_generation, 1);

    owner.cancel_epoch(identity).unwrap();
    for slot in owner.slots.iter_mut() {
        slot.slot_generation = u64::MAX;
    }
    let snapshot = owner.clone();
    assert!(matches!(
        owner.reserve(queue, test_completion_roster_v1(2)),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert_eq!(owner, snapshot);
}

#[test]
fn host_retained_physical_lane_a_b_c_epochs_coexist_and_retire_out_of_observation_order() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let mut epochs = Vec::new();
    for generation in 1..=3 {
        let identity = owner
            .reserve(queue, test_completion_roster_v1(generation))
            .unwrap();
        let completion = test_completion_occurrence_v1(generation);
        owner.mark_published(identity, completion).unwrap();
        epochs.push((identity, completion));
    }
    assert!(matches!(
        owner.ensure_prepared(),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    for index in [1, 2, 0] {
        let (identity, completion) = epochs[index];
        owner.complete_epoch(identity, completion).unwrap();
        owner.recycle_epoch(identity, completion).unwrap();
    }
    assert_eq!(owner.returned_generation().unwrap(), 3);
}

#[test]
fn out_of_order_host_retirement_keeps_the_monotonic_completed_frontier() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let mut epochs = Vec::new();
    for generation in 1..=8 {
        let identity = owner
            .reserve(queue, test_completion_roster_v1(generation))
            .unwrap();
        let completion = test_completion_occurrence_v1(generation);
        owner.mark_published(identity, completion).unwrap();
        epochs.push((identity, completion));
    }
    for (identity, completion) in epochs.iter().rev().copied() {
        owner.complete_epoch(identity, completion).unwrap();
        owner.recycle_epoch(identity, completion).unwrap();
    }
    assert_eq!(owner.returned_generation().unwrap(), 8);
    assert!(owner.ensure_prepared().is_ok());
    for (identity, completion) in epochs {
        assert!(owner.complete_epoch(identity, completion).is_err());
        assert!(owner.recycle_epoch(identity, completion).is_err());
    }
}

#[test]
fn every_live_epoch_phase_blocks_every_owner_global_recipe_transition() {
    let queue = test_dispatch_queue_v1();
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let identity = owner.reserve(queue, test_completion_roster_v1(1)).unwrap();
    let completion = test_completion_occurrence_v1(1);
    for live in [
        owner.clone(),
        {
            let mut published = owner.clone();
            published.mark_published(identity, completion).unwrap();
            published
        },
        {
            let mut completed = owner.clone();
            completed.mark_published(identity, completion).unwrap();
            completed.complete_epoch(identity, completion).unwrap();
            completed
        },
    ] {
        assert!(matches!(
            live.ensure_prepared(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert!(matches!(
            live.returned_generation(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert!(matches!(
            live.returning_destroy_generation(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
    }
    owner.cancel_epoch(identity).unwrap();
    assert_eq!(owner.returning_destroy_generation().unwrap(), 0);
}

#[test]
fn actual_program_and_packet_mutations_change_persistent_control_identity() {
    let mut image = include_bytes!(
        "../../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
    )
    .to_vec();
    let identity =
        |image: &[u8], signature, grid_x, ordering, dynamic_group_segment_bytes, scalar| {
            actual_persistent_control_test_identity(
                image,
                signature,
                grid_x,
                ordering,
                dynamic_group_segment_bytes,
                scalar,
                0,
                4096,
            )
            .unwrap()
        };
    let baseline = identity(
        &image,
        [0x71; 32],
        1024,
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        1024,
    );
    assert_eq!(
        baseline,
        identity(
            &image,
            [0x71; 32],
            1024,
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            1024,
        )
    );

    let entry_offset = usize::try_from(
        actual_persistent_control_test_program(&image, [0x71; 32])
            .selected_binding()
            .entry_file_offset(),
    )
    .unwrap();
    image[entry_offset] ^= 1;
    let changed_code = identity(
        &image,
        [0x71; 32],
        1024,
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        1024,
    );
    image[entry_offset] ^= 1;

    for changed in [
        changed_code,
        identity(
            &image,
            [0x72; 32],
            1024,
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            1024,
        ),
        identity(
            &image,
            [0x71; 32],
            2048,
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            1024,
        ),
        identity(
            &image,
            [0x71; 32],
            1024,
            AqlDispatchOrderingV1::WaitForPrior,
            64,
            1024,
        ),
        identity(
            &image,
            [0x71; 32],
            1024,
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            2048,
        ),
    ] {
        assert_ne!(baseline, changed);
    }

    assert!(matches!(
        actual_persistent_control_test_identity(
            &image,
            [0x71; 32],
            1024,
            AqlDispatchOrderingV1::Independent,
            0,
            1024,
            0,
            4096,
        ),
        Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            detail: "multi-inflight recipe requires wait-for-prior ordering",
            ..
        })
    ));

    for (data_byte_offset, byte_len) in [(1, 4096), (0, 4095)] {
        assert!(matches!(
            actual_persistent_control_test_identity(
                &image,
                [0x71; 32],
                1024,
                AqlDispatchOrderingV1::WaitForPrior,
                0,
                1024,
                data_byte_offset,
                byte_len,
            ),
            Err(Gfx942DispatchBindingErrorV1::InvalidData { .. })
        ));
    }
}

#[test]
fn persistent_control_replay_requires_exact_identity_and_recycled_generation() {
    let queue = persistent_control_test_queue(19);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 101);
    let expected = persistent_control_test_identity(queue, device.storage_identity());
    let mut generation = DispatchGenerationOwnerV1::new().unwrap();

    assert!(matches!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
            ),
            0,
            1,
            &generation,
            expected,
            1,
        ),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let first = generation.next().unwrap();
    generation.commit_begin(first);
    generation.complete(first).unwrap();
    generation.recycle(first).unwrap();
    assert_eq!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
            ),
            0,
            1,
            &generation,
            expected,
            first,
        )
        .unwrap(),
        DeviceDataEffectV1::ReadWrite
    );
    assert!(matches!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
            ),
            0,
            1,
            &generation,
            expected,
            first + 1,
        ),
        Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
    ));
    for (authorities, premises) in [(1, 1), (0, 0), (0, 2)] {
        assert!(matches!(
            validate_persistent_control_replay_v1(
                PersistentFixedDispatchControlStateV1::DataDetached(
                    BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
                ),
                authorities,
                premises,
                &generation,
                expected,
                first,
            ),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
    }
}

#[test]
fn persistent_control_replay_rejects_every_bound_identity_substitution() {
    let queue = persistent_control_test_queue(23);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 202);
    let (other_device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 203);
    let expected = persistent_control_test_identity(queue, device.storage_identity());
    let mut generation = DispatchGenerationOwnerV1::new().unwrap();
    let current = generation.next().unwrap();
    generation.commit_begin(current);
    generation.complete(current).unwrap();
    generation.recycle(current).unwrap();

    let substitutions = vec![
        PersistentFixedDispatchControlIdentityV1 {
            queue: persistent_control_test_queue(24),
            ..expected
        },
        PersistentFixedDispatchControlIdentityV1 {
            semantic_sha256: [0x52; 32],
            ..expected
        },
        PersistentFixedDispatchControlIdentityV1 {
            content_role: Gfx942DeviceContentRoleV1::new([0x62; 32], 0).unwrap(),
            ..expected
        },
        PersistentFixedDispatchControlIdentityV1 {
            data_layout: Gfx942FixedDispatchDataLayoutV1::device_local(8192, 4096),
            ..expected
        },
        PersistentFixedDispatchControlIdentityV1 {
            data_storage: other_device.storage_identity(),
            ..expected
        },
        PersistentFixedDispatchControlIdentityV1 {
            effect: DeviceDataEffectV1::ReadOnly,
            ..expected
        },
    ];
    for substituted in substitutions {
        assert!(
            validate_persistent_control_replay_v1(
                PersistentFixedDispatchControlStateV1::DataDetached(
                    BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
                ),
                0,
                1,
                &generation,
                substituted,
                current,
            )
            .is_err()
        );
    }
    assert!(matches!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::Ordinary,
            0,
            1,
            &generation,
            expected,
            current,
        ),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
}

#[test]
fn persistent_control_generation_advances_across_replay_cycles() {
    let queue = persistent_control_test_queue(29);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 303);
    let expected = persistent_control_test_identity(queue, device.storage_identity());
    let mut generation = DispatchGenerationOwnerV1::new().unwrap();
    for expected_generation in 1..=4 {
        let current = generation.next().unwrap();
        assert_eq!(current, expected_generation);
        generation.commit_begin(current);
        generation.complete(current).unwrap();
        generation.recycle(current).unwrap();
        assert_eq!(
            validate_persistent_control_replay_v1(
                PersistentFixedDispatchControlStateV1::DataDetached(
                    BoundedPersistentFixedDispatchControlIdentityV1::from_single(expected)
                ),
                0,
                1,
                &generation,
                expected,
                current,
            )
            .unwrap(),
            DeviceDataEffectV1::ReadWrite
        );
    }
}

#[test]
fn never_published_detached_generation_rebinds_as_fresh_owner() {
    let mut generation = DispatchGenerationOwnerV1::after_detached(0).unwrap();
    assert_eq!(generation.next().unwrap(), 1);
    generation.commit_begin(1);
    generation.cancel(1).unwrap();
    assert_eq!(generation.returning_destroy_generation().unwrap(), 0);

    let recycled = DispatchGenerationOwnerV1::after_detached(7).unwrap();
    assert_eq!(recycled.next().unwrap(), 8);
    assert!(matches!(
        DispatchGenerationOwnerV1::after_detached(u64::MAX),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
}

#[test]
fn persistent_control_detached_data_blocks_returning_destroy_but_not_ordinary_release() {
    let queue = persistent_control_test_queue(31);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 404);
    let identity = persistent_control_test_identity(queue, device.storage_identity());
    assert!(
        validate_returning_destroy_control_state_v1(
            PersistentFixedDispatchControlStateV1::Ordinary,
            1,
            1,
        )
        .is_ok()
    );
    assert!(
        validate_returning_destroy_control_state_v1(
            PersistentFixedDispatchControlStateV1::Attached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            1,
            1,
        )
        .is_ok()
    );
    assert!(matches!(
        validate_returning_destroy_control_state_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            0,
            1,
        ),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert!(matches!(
        validate_returning_destroy_control_state_v1(
            PersistentFixedDispatchControlStateV1::Attached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            0,
            1,
        ),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));

    // Ordinary queue destruction consumes the owner through `release`,
    // which deliberately accepts detached data because its separate SDMA
    // owner releases that authority. Only code and kernarg remain here.
    let mut generation = DispatchGenerationOwnerV1::new().unwrap();
    let current = generation.next().unwrap();
    generation.commit_begin(current);
    generation.complete(current).unwrap();
    generation.recycle(current).unwrap();
    assert!(generation.ensure_prepared().is_ok());

    assert!(
        validate_detached_persistent_control_release_state_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            0,
            1,
            current,
            current,
        )
        .is_ok()
    );
    for (state, authorities, premises, returned, expected) in [
        (
            PersistentFixedDispatchControlStateV1::Attached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity),
            ),
            0,
            1,
            current,
            current,
        ),
        (
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity),
            ),
            1,
            1,
            current,
            current,
        ),
        (
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity),
            ),
            0,
            0,
            current,
            current,
        ),
        (
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity),
            ),
            0,
            1,
            current,
            current + 1,
        ),
    ] {
        assert!(
            validate_detached_persistent_control_release_state_v1(
                state,
                authorities,
                premises,
                returned,
                expected,
            )
            .is_err()
        );
    }
}

#[test]
fn write_only_replay_accepts_uninitialized_exact_storage() {
    assert!(persistent_replay_initialization_is_admitted_v1(
        DeviceDataEffectV1::WriteOnly,
        false,
        false,
    ));
    assert!(!persistent_replay_initialization_is_admitted_v1(
        DeviceDataEffectV1::ReadOnly,
        false,
        true,
    ));
    assert!(!persistent_replay_initialization_is_admitted_v1(
        DeviceDataEffectV1::ReadOnly,
        false,
        false,
    ));
    assert!(!persistent_replay_initialization_is_admitted_v1(
        DeviceDataEffectV1::ReadWrite,
        false,
        false,
    ));
}

#[test]
fn cancelled_replay_preserves_predecessor_and_cannot_double_reattach() {
    let queue = persistent_control_test_queue(37);
    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 505);
    let identity = persistent_control_test_identity(queue, device.storage_identity());
    let mut generation = DispatchGenerationOwnerV1::new().unwrap();
    for expected in 1..=7 {
        let current = generation.next().unwrap();
        assert_eq!(current, expected);
        generation.commit_begin(current);
        generation.complete(current).unwrap();
        generation.recycle(current).unwrap();
    }
    assert_eq!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::DataDetached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            0,
            1,
            &generation,
            identity,
            7,
        )
        .unwrap(),
        DeviceDataEffectV1::ReadWrite
    );
    let replay = generation.next().unwrap();
    assert_eq!(replay, 8);
    generation.commit_begin(replay);
    generation.cancel(replay).unwrap();
    assert_eq!(generation.returning_destroy_generation().unwrap(), 7);
    assert!(matches!(
        validate_persistent_control_replay_v1(
            PersistentFixedDispatchControlStateV1::Attached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            1,
            1,
            &generation,
            identity,
            7,
        ),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert!(
        validate_returning_destroy_control_state_v1(
            PersistentFixedDispatchControlStateV1::Attached(
                BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
            ),
            1,
            1,
        )
        .is_ok()
    );
}

#[test]
fn replacement_owner_advances_from_exact_recycled_generation() {
    let mut first = DispatchGenerationOwnerV1::new().unwrap();
    let first_generation = first.next().unwrap();
    first.commit_begin(first_generation);
    first.complete(first_generation).unwrap();
    first.recycle(first_generation).unwrap();

    let mut second =
        DispatchGenerationOwnerV1::after_recycled(first.returned_generation().unwrap()).unwrap();
    let second_generation = second.next().unwrap();
    assert_eq!(first_generation, 1);
    assert_eq!(second_generation, 2);
    second.commit_begin(second_generation);
    second.complete(second_generation).unwrap();
    second.recycle(second_generation).unwrap();
    let writable = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::WriteOnly,
        &[(64, 64)],
    );
    let stale = Gfx942CompletedDispatchReadRequestV1::new(first_generation, 0, 64, 64);
    assert!(validate_completed_read_request(&second, &[writable], stale).is_err());

    let third =
        DispatchGenerationOwnerV1::after_recycled(second.returned_generation().unwrap()).unwrap();
    assert_eq!(third.next().unwrap(), 3);
    assert!(matches!(
        DispatchGenerationOwnerV1::after_recycled(0),
        Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
    ));
    for exhausted in [u64::MAX - 1, u64::MAX] {
        assert!(matches!(
            DispatchGenerationOwnerV1::after_recycled(exhausted),
            Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
        ));
    }
    assert_eq!(
        DispatchGenerationOwnerV1::after_recycled(u64::MAX - 2)
            .unwrap()
            .next()
            .unwrap(),
        u64::MAX - 1
    );
}

#[test]
fn fixed_batch_may_select_one_program_from_a_larger_inspected_roster() {
    let packets = [Gfx942FixedDispatchPacketV1::new(
        11,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        Vec::new().into_boxed_slice(),
        Vec::new().into_boxed_slice(),
    )];
    validate_packet_program_indices(12, &packets).unwrap();
    assert!(matches!(
        validate_packet_program_indices(11, &packets),
        Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet: 0,
            detail: "program index",
        })
    ));
}

#[test]
fn gfx950_loader_profile_cannot_enter_gfx942_dispatch_preparation() {
    validate_gfx942_executable_profile(AdmittedProfile::Gfx942XnackOffCov6).unwrap();
    assert!(matches!(
        validate_gfx942_executable_profile(AdmittedProfile::Gfx950XnackOffCov6),
        Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "executable target is not gfx942:xnack-"
        ))
    ));
}

#[test]
#[ignore = "requires a freshly compiled FE2O3_TEST_GFX950_COV6 and FE2O3_TEST_GFX950_KERNEL"]
fn real_gfx950_kernel_rejects_before_fixed_dispatch_data_preparation() {
    let bytes =
        std::fs::read(std::env::var("FE2O3_TEST_GFX950_COV6").expect("set FE2O3_TEST_GFX950_COV6"))
            .unwrap();
    let name = std::env::var("FE2O3_TEST_GFX950_KERNEL").expect("set FE2O3_TEST_GFX950_KERNEL");
    let kernel = fe2o3_amdhsa_loader::validate(&bytes, AdmittedProfile::Gfx950XnackOffCov6)
        .unwrap()
        .bind_kernel(&name)
        .unwrap();
    let programs = [kernel];
    let packets = [Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        Vec::new().into_boxed_slice(),
        Vec::new().into_boxed_slice(),
    )];
    assert!(matches!(
        plan_public_fixed_dispatch_resources(&programs, &packets, &[], &[]),
        Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "executable target is not gfx942:xnack-"
        ))
    ));
    assert!(matches!(
        preflight_gfx942_fixed_dispatch_replacement(4096, &programs, &packets, &[], 1),
        Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "executable target is not gfx942:xnack-"
        ))
    ));
}

#[test]
fn replacement_preflight_rejects_empty_programs_and_exhausted_generation_before_mutation() {
    let packets = [Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        Vec::new().into_boxed_slice(),
        Vec::new().into_boxed_slice(),
    )];
    assert!(matches!(
        plan_public_fixed_dispatch_resources(&[], &packets, &[], &[]),
        Err(Gfx942DispatchBindingErrorV1::ProgramCount { requested: 0, .. })
    ));
    assert!(matches!(
        preflight_gfx942_fixed_dispatch_replacement(4_096, &[], &packets, &[], u64::MAX - 1,),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert!(matches!(
        preflight_gfx942_fixed_dispatch_replacement(4_096, &[], &packets, &[], u64::MAX - 2,),
        Err(Gfx942DispatchBindingErrorV1::ProgramCount { requested: 0, .. })
    ));
}

#[test]
fn recycled_queue_can_admit_a_different_second_fixed_batch_generation() {
    let queue_generation = 19u64;
    let first = Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        vec![0, 0, 0, 1].into_boxed_slice(),
        Vec::new().into_boxed_slice(),
    );
    let second = [
        Gfx942FixedDispatchPacketV1::new(
            1,
            AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]).unwrap(),
            256,
            vec![0, 0, 0, 2].into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        ),
        Gfx942FixedDispatchPacketV1::new(
            0,
            AqlDispatchGeometryV1::new([32, 2, 1], [32, 1, 1]).unwrap(),
            0,
            vec![0, 0, 0, 3].into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        ),
    ];
    assert_ne!(first.program_index, second[0].program_index);
    assert_ne!(first.geometry, second[0].geometry);
    assert_ne!(first.kernarg_bytes, second[0].kernarg_bytes);
    validate_fixed_batch_ring::<1>(65_536).unwrap();
    validate_fixed_batch_ring::<2>(65_536).unwrap();

    let fully_initialized = true;
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let first_generation = owner.next().unwrap();
    owner.commit_begin(first_generation);
    owner.complete(first_generation).unwrap();
    owner.recycle(first_generation).unwrap();
    let second_generation = owner.next().unwrap();
    owner.commit_begin(second_generation);
    owner.complete(second_generation).unwrap();
    owner.recycle(second_generation).unwrap();

    assert_eq!(first_generation, 1);
    assert_eq!(second_generation, 2);
    assert_eq!(queue_generation, 19);
    assert!(fully_initialized);
    assert_eq!(second.len(), 2);
}

#[test]
fn stale_and_double_use_transitions_never_mutate_generation_state() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    owner.commit_begin(generation);
    for stale in [0, generation + 1, u64::MAX] {
        let before = owner.clone();
        assert!(matches!(
            owner.cancel(stale),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
        assert_eq!(owner, before);
        assert!(matches!(
            owner.complete(stale),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
        assert_eq!(owner, before);
        assert!(matches!(
            owner.recycle(stale),
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        ));
        assert_eq!(owner, before);
    }
    owner.complete(generation).unwrap();
    let completed = owner.clone();
    assert!(owner.complete(generation).is_err());
    assert_eq!(owner, completed);
    owner.recycle(generation).unwrap();
    let recycled = owner.clone();
    assert!(owner.recycle(generation).is_err());
    assert_eq!(owner, recycled);
    assert_eq!(owner.returned_generation().unwrap(), generation);

    let next = owner.next().unwrap();
    owner.commit_begin(next);
    assert!(owner.returned_generation().is_err());
    owner.cancel(next).unwrap();
    assert_eq!(owner.returned_generation().unwrap(), generation);
}

#[test]
fn retryable_cancel_preserves_last_recycle_for_returning_destroy() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let first_generation = owner.next().unwrap();
    owner.commit_begin(first_generation);
    owner.complete(first_generation).unwrap();
    owner.recycle(first_generation).unwrap();

    let retry_generation = owner.next().unwrap();
    owner.commit_begin(retry_generation);
    assert_eq!(first_generation, 1);
    assert_eq!(retry_generation, 2);
    assert!(owner.returned_generation().is_err());
    assert!(owner.returning_destroy_generation().is_err());

    // `cancel` is reached only for a classified no-effect submission
    // failure. The latest exact recycle remains the returned report.
    owner.cancel(retry_generation).unwrap();
    assert_eq!(owner.returned_generation().unwrap(), first_generation);
    assert_eq!(
        owner.returning_destroy_generation().unwrap(),
        first_generation
    );
}

#[test]
fn returning_destroy_admits_unpublished_or_exactly_recycled_only() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let unpublished = owner.clone();
    assert!(owner.returned_generation().is_err());
    assert_eq!(owner.returning_destroy_generation().unwrap(), 0);
    assert_eq!(owner, unpublished);

    let generation = owner.next().unwrap();
    owner.commit_begin(generation);
    let in_flight = owner.clone();
    assert!(owner.returned_generation().is_err());
    assert!(owner.returning_destroy_generation().is_err());
    assert_eq!(owner, in_flight);
    owner.complete(generation).unwrap();
    let completed = owner.clone();
    assert!(owner.returned_generation().is_err());
    assert!(owner.returning_destroy_generation().is_err());
    assert_eq!(owner, completed);
    assert!(owner.recycle(generation + 1).is_err());
    assert!(owner.returned_generation().is_err());
    owner.recycle(generation).unwrap();
    assert_eq!(owner.returned_generation().unwrap(), generation);
    assert_eq!(owner.returning_destroy_generation().unwrap(), generation);
}
