use super::*;

#[test]
fn initialized_readback_requires_recycle_exact_generation_and_all_epochs_vacant() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    let mut premise = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadOnly,
        &[],
    );
    premise.fully_initialized = true;
    let premises = [premise];
    let request = Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 0, 256);
    assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_err());
    owner.commit_begin(generation);
    assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_err());
    owner.complete(generation).unwrap();
    assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_err());
    owner.recycle(generation).unwrap();
    assert_eq!(
        validate_completed_initialized_read_request(&owner, &premises, request).unwrap(),
        generation
    );
    for index in 0..GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1 {
        owner.slots[index].phase = DispatchEpochPhaseV1::Reserved {
            dispatch_generation: generation + 1,
            expected_roster: test_completion_roster_v1(generation + 1),
        };
        assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_err());
        owner.slots[index].phase = DispatchEpochPhaseV1::Vacant;
    }
    let next = owner.next().unwrap();
    owner.commit_begin(next);
    owner.complete(next).unwrap();
    owner.recycle(next).unwrap();
    assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_err());
    let current = Gfx942CompletedDispatchReadRequestV1::new(next, 0, 0, 256);
    assert!(validate_completed_initialized_read_request(&owner, &premises, current).is_ok());
    owner.poison();
    assert!(validate_completed_initialized_read_request(&owner, &premises, current).is_err());
}

#[test]
fn initialized_readback_uses_sealed_extent_without_promoting_inspected_effects() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    owner.commit_begin(generation);
    owner.complete(generation).unwrap();
    owner.recycle(generation).unwrap();
    let request = Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 0, 256);
    for effect in [
        None,
        Some(DeviceDataEffectV1::ReadOnly),
        Some(DeviceDataEffectV1::ReadWrite),
        Some(DeviceDataEffectV1::WriteOnly),
    ] {
        for kind in [
            Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
            Gfx942FixedDispatchDataKindV1::DeviceLocal,
        ] {
            for initialized in [false, true] {
                let mut premise = readback_premise(kind, DeviceDataEffectV1::ReadOnly, &[]);
                premise.effect = effect;
                premise.fully_initialized = initialized;
                let premises = [premise];
                let before = (premises[0].effect, premises[0].fully_initialized);
                assert_eq!(
                    validate_completed_initialized_read_request(&owner, &premises, request).is_ok(),
                    kind == Gfx942FixedDispatchDataKindV1::HostVisibleCoherent && initialized
                );
                assert!(validate_completed_read_request(&owner, &premises, request).is_err());
                assert_eq!(before, (premises[0].effect, premises[0].fully_initialized));
            }
        }
    }
}

#[test]
fn initialized_readback_rejects_stale_ordinals_lengths_and_overflow() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    owner.commit_begin(generation);
    owner.complete(generation).unwrap();
    owner.recycle(generation).unwrap();
    let mut premise = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadOnly,
        &[],
    );
    premise.fully_initialized = true;
    let premises = [premise];
    for (epoch, index, offset, length) in [
        (0, 0, 0, 256),
        (generation + 1, 0, 0, 256),
        (generation, 1, 0, 256),
        (generation, 0, 0, 0),
        (generation, 0, 0, 257),
        (generation, 0, 256, 1),
        (generation, 0, u64::MAX, 2),
        (generation, 0, 1, u64::MAX),
    ] {
        assert!(
            validate_completed_initialized_read_request(
                &owner,
                &premises,
                Gfx942CompletedDispatchReadRequestV1::new(epoch, index, offset, length)
            )
            .is_err()
        );
    }
    let request = Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 63, 193);
    assert!(validate_completed_initialized_read_request(&owner, &premises, request).is_ok());
    for length in [0, 192, 194, usize::MAX] {
        assert!(validate_completed_read_destination(request, length).is_err());
    }
    assert!(validate_completed_read_destination(request, 193).is_ok());
}

#[test]
fn completed_read_requests_reject_phase_generation_effect_bounds_and_overlap() {
    let writable = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::WriteOnly,
        &[(64, 64)],
    );
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    let request = Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 64, 64);
    assert!(validate_completed_read_request(&owner, &[writable], request).is_err());

    owner.commit_begin(generation);
    let writable = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::WriteOnly,
        &[(64, 64)],
    );
    assert!(validate_completed_read_request(&owner, &[writable], request).is_err());
    owner.complete(generation).unwrap();
    let writable = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::WriteOnly,
        &[(64, 64)],
    );
    assert!(validate_completed_read_request(&owner, &[writable], request).is_err());
    owner.recycle(generation).unwrap();

    let valid = || {
        readback_premise(
            Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
            DeviceDataEffectV1::WriteOnly,
            &[(64, 64)],
        )
    };
    assert_eq!(
        validate_completed_read_request(&owner, &[valid()], request).unwrap(),
        generation
    );
    assert!(validate_completed_read_destination(request, 64).is_ok());
    for rejected_len in [0, 63, 65, usize::MAX] {
        assert!(validate_completed_read_destination(request, rejected_len).is_err());
    }
    for rejected in [
        Gfx942CompletedDispatchReadRequestV1::new(0, 0, 64, 64),
        Gfx942CompletedDispatchReadRequestV1::new(generation + 1, 0, 64, 64),
        Gfx942CompletedDispatchReadRequestV1::new(generation, 1, 64, 64),
        Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 64, 0),
        Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 63, 1),
        Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 120, 16),
        Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 250, 16),
    ] {
        assert!(validate_completed_read_request(&owner, &[valid()], rejected).is_err());
    }

    let readonly = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadOnly,
        &[],
    );
    assert!(validate_completed_read_request(&owner, &[readonly], request).is_err());
    let mut untouched = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadOnly,
        &[],
    );
    untouched.effect = None;
    assert!(matches!(
        validate_completed_read_request(&owner, &[untouched], request),
        Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "completed read requires inspected write access",
        })
    ));
    let device = readback_premise(
        Gfx942FixedDispatchDataKindV1::DeviceLocal,
        DeviceDataEffectV1::WriteOnly,
        &[(64, 64)],
    );
    assert!(validate_completed_read_request(&owner, &[device], request).is_err());
    let overlap = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadWrite,
        &[(32, 64), (64, 64)],
    );
    assert!(validate_completed_read_request(&owner, &[overlap], request).is_err());
    let readwrite = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadWrite,
        &[(64, 64)],
    );
    assert!(validate_completed_read_request(&owner, &[readwrite], request).is_ok());

    let next = owner.next().unwrap();
    owner.commit_begin(next);
    owner.complete(next).unwrap();
    owner.recycle(next).unwrap();
    assert!(validate_completed_read_request(&owner, &[valid()], request).is_err());
    assert_eq!(
        validate_completed_read_request(
            &owner,
            &[valid()],
            Gfx942CompletedDispatchReadRequestV1::new(next, 0, 64, 64),
        )
        .unwrap(),
        next
    );
}

#[test]
fn completed_snapshot_requests_require_exact_recycled_declaration() {
    let mut owner = DispatchGenerationOwnerV1::new().unwrap();
    let generation = owner.next().unwrap();
    owner.commit_begin(generation);
    owner.complete(generation).unwrap();
    owner.recycle(generation).unwrap();

    let request = Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 0, 32, 128);
    assert_eq!(
        validate_completed_snapshot_request(
            &owner,
            &[snapshot_readback_premise(
                Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                true,
            )],
            request,
        )
        .unwrap(),
        generation
    );
    for rejected in [
        Gfx942CompletedDispatchSnapshotRequestV1::new(0, 0, 32, 128),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation + 1, 0, 32, 128),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 1, 32, 128),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 0, 32, 127),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 0, 33, 127),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 0, 32, 0),
        Gfx942CompletedDispatchSnapshotRequestV1::new(generation, 0, 250, 16),
    ] {
        assert!(
            validate_completed_snapshot_request(
                &owner,
                &[snapshot_readback_premise(
                    Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                    true,
                )],
                rejected,
            )
            .is_err()
        );
    }
    assert!(
        validate_completed_snapshot_request(
            &owner,
            &[snapshot_readback_premise(
                Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                false,
            )],
            request,
        )
        .is_err()
    );
    assert!(
        validate_completed_snapshot_request(
            &owner,
            &[snapshot_readback_premise(
                Gfx942FixedDispatchDataKindV1::DeviceLocal,
                true,
            )],
            request,
        )
        .is_err()
    );
    assert!(
        validate_completed_read_request(
            &owner,
            &[snapshot_readback_premise(
                Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                true,
            )],
            Gfx942CompletedDispatchReadRequestV1::new(generation, 0, 32, 128),
        )
        .is_err()
    );
    let mut untouched = readback_premise(
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        DeviceDataEffectV1::ReadOnly,
        &[],
    );
    untouched.effect = None;
    untouched.fully_initialized = true;
    assert!(matches!(
        validate_completed_snapshot_request(&owner, &[untouched], request),
        Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "completed snapshot requires one exact admitted range",
        })
    ));
}

#[test]
fn exhaustion_and_poison_from_each_phase_are_terminal_and_fail_closed() {
    let mut exhausted = DispatchGenerationOwnerV1::new().unwrap();
    exhausted.next_generation = u64::MAX;
    let before_slots = exhausted.slots.to_vec();
    let before_recycled = exhausted.recycled_generation;
    assert!(matches!(
        exhausted.next(),
        Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
    ));
    assert_eq!(&*exhausted.slots, before_slots);
    assert_eq!(exhausted.recycled_generation, before_recycled);

    for phase in 0..3 {
        let mut owner = DispatchGenerationOwnerV1::with_next_generation(7).unwrap();
        if phase >= 1 {
            owner.commit_begin(7);
        }
        if phase == 2 {
            owner.complete(7).unwrap();
        }
        owner.poison();
        assert!(owner.poisoned);
        assert!(matches!(
            owner.next(),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        assert!(matches!(
            owner.cancel(7),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        assert!(matches!(
            owner.complete(7),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        assert!(matches!(
            owner.recycle(7),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
    }
}
