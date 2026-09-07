use alloc::{vec, vec::Vec};

use super::r57_three_binding_compute::*;

const DEVICE: u32 = 7;
const VM: u64 = 11;
const QUEUE: u64 = 13;

fn identity(index: usize) -> R57AllocationIdentityV1 {
    R57AllocationIdentityV1 {
        allocation: 100 + index as u64,
        storage: 200 + index as u64,
        allocation_generation: 300 + index as u64,
        device: DEVICE,
        vm: VM,
        byte_len: 4096,
        memory_kind: R57MemoryKindV1::PersistentHbm,
    }
}

fn owner(index: usize, initialized: bool) -> R57OwnedAllocationV1 {
    R57OwnedAllocationV1::new_model_only(
        400 + index as u64,
        identity(index),
        initialized,
        500 + index as u64,
    )
    .unwrap()
}

fn owners(output_initialized: bool) -> Vec<R57OwnedAllocationV1> {
    vec![owner(0, true), owner(1, true), owner(2, output_initialized)]
}

fn snapshots(output_initialized: bool) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
    [
        owner(0, true).snapshot_model_only(),
        owner(1, true).snapshot_model_only(),
        owner(2, output_initialized).snapshot_model_only(),
    ]
}

fn spec(index: usize) -> R57BindingSpecV1 {
    let identity = identity(index);
    R57BindingSpecV1 {
        ordinal: index as u8,
        allocation: identity.allocation,
        storage: identity.storage,
        allocation_generation: identity.allocation_generation,
        byte_offset: 0,
        byte_len: identity.byte_len,
        role: if index < 2 {
            R57BindingRoleV1::Read
        } else {
            R57BindingRoleV1::Write
        },
        effect: if index < 2 {
            R57BindingEffectV1::ReadOnly
        } else {
            R57BindingEffectV1::WriteOnly
        },
        requires_initialized: index < 2,
    }
}

fn plan() -> R57ThreeBindingPlanV1 {
    R57ThreeBindingPlanV1 {
        device: DEVICE,
        vm: VM,
        queue: QUEUE,
        queue_generation: 17,
        fixed_binder_authority: 19,
        kernel: 23,
        dispatch: 29,
        transaction_generation: 31,
        frontier_before: 37,
        completion_signal: 41,
        wait_packet: R57PacketIdentityV1 {
            packet: 43,
            kind: R57PacketKindV1::WaitForPrior,
            order: 0,
        },
        dispatch_packet: R57PacketIdentityV1 {
            packet: 47,
            kind: R57PacketKindV1::Dispatch,
            order: 1,
        },
        bindings: [spec(0), spec(1), spec(2)],
    }
}

fn prepared(output_initialized: bool) -> R57PreparedThreeBindingV1 {
    r57_prepare_three_binding_compute_model_only(plan(), owners(output_initialized)).unwrap()
}

fn published(output_initialized: bool) -> R57PublishedThreeBindingV1 {
    match r57_publish_three_binding_compute_model_only(
        prepared(output_initialized),
        true,
        R57PublicationScriptV1::Complete,
    ) {
        R57PublishOutcomeV1::Published(published) => published,
        _ => panic!("valid publication did not publish"),
    }
}

fn completion() -> R57CompletionIdentityV1 {
    let plan = plan();
    R57CompletionIdentityV1 {
        queue: plan.queue,
        queue_generation: plan.queue_generation,
        dispatch: plan.dispatch,
        transaction_generation: plan.transaction_generation,
        completion_signal: plan.completion_signal,
        frontier_after: plan.frontier_after_model_only().unwrap(),
    }
}

#[test]
fn exact_three_boundary_admits_a_fixed_three_owner_shape() {
    let prepared = prepared(false);
    assert_eq!(prepared.owner_snapshots_model_only().len(), 3);
    assert_eq!(prepared.owner_snapshots_model_only(), snapshots(false));
    assert_eq!(prepared.opening_model_only(), snapshots(false));
}

#[test]
fn every_nearby_wrong_cardinality_is_rejected_with_exact_owners() {
    for count in 0..=6 {
        if count == R57_BINDING_COUNT_V1 {
            continue;
        }
        let supplied: Vec<_> = (0..count).map(|index| owner(index % 3, true)).collect();
        let expected: Vec<_> = supplied
            .iter()
            .map(R57OwnedAllocationV1::snapshot_model_only)
            .collect();
        let failure = r57_prepare_three_binding_compute_model_only(plan(), supplied).unwrap_err();
        assert_eq!(failure.error, R57PreparationErrorV1::BindingCount);
        assert_eq!(failure.owner_snapshots_model_only(), expected);
        assert_eq!(failure.into_owners_model_only().len(), count);
    }
}

#[test]
fn exact_role_effect_initialization_and_order_contract_is_required() {
    let mut mutations = Vec::new();
    let mut value = plan();
    value.bindings[0].ordinal = 1;
    mutations.push(value);
    let mut value = plan();
    value.bindings[0].role = R57BindingRoleV1::Write;
    mutations.push(value);
    let mut value = plan();
    value.bindings[0].effect = R57BindingEffectV1::WriteOnly;
    mutations.push(value);
    let mut value = plan();
    value.bindings[0].requires_initialized = false;
    mutations.push(value);
    let mut value = plan();
    value.bindings[1].ordinal = 0;
    mutations.push(value);
    let mut value = plan();
    value.bindings[1].role = R57BindingRoleV1::Write;
    mutations.push(value);
    let mut value = plan();
    value.bindings[1].effect = R57BindingEffectV1::WriteOnly;
    mutations.push(value);
    let mut value = plan();
    value.bindings[1].requires_initialized = false;
    mutations.push(value);
    let mut value = plan();
    value.bindings[2].ordinal = 1;
    mutations.push(value);
    let mut value = plan();
    value.bindings[2].role = R57BindingRoleV1::Read;
    mutations.push(value);
    let mut value = plan();
    value.bindings[2].effect = R57BindingEffectV1::ReadOnly;
    mutations.push(value);
    let mut value = plan();
    value.bindings[2].requires_initialized = true;
    mutations.push(value);

    for mutation in mutations {
        let failure =
            r57_prepare_three_binding_compute_model_only(mutation, owners(false)).unwrap_err();
        assert_eq!(failure.error, R57PreparationErrorV1::InvalidPlan);
        assert_eq!(failure.owner_snapshots_model_only(), snapshots(false));
    }
}

#[test]
fn allocation_storage_and_generation_binding_substitutions_are_rejected() {
    for field in 0..3 {
        let mut mutation = plan();
        match field {
            0 => mutation.bindings[1].allocation += 1_000,
            1 => mutation.bindings[1].storage += 1_000,
            _ => mutation.bindings[1].allocation_generation += 1_000,
        }
        let failure =
            r57_prepare_three_binding_compute_model_only(mutation, owners(false)).unwrap_err();
        assert_eq!(failure.error, R57PreparationErrorV1::BindingIdentity);
        assert_eq!(failure.owner_snapshots_model_only(), snapshots(false));
    }
}

#[test]
fn all_three_bindings_require_the_same_device_and_vm() {
    for index in 0..3 {
        let mut supplied = owners(false);
        let original = supplied.remove(index);
        let mut changed = original.snapshot_model_only().identity;
        changed.device += 1;
        supplied.insert(
            index,
            R57OwnedAllocationV1::new_model_only(
                original.snapshot_model_only().owner_occurrence,
                changed,
                original.snapshot_model_only().initialized,
                original.snapshot_model_only().content_generation,
            )
            .unwrap(),
        );
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(plan(), supplied)
                .unwrap_err()
                .error,
            R57PreparationErrorV1::CrossDevice
        );

        let mut supplied = owners(false);
        let original = supplied.remove(index);
        let mut changed = original.snapshot_model_only().identity;
        changed.vm += 1;
        supplied.insert(
            index,
            R57OwnedAllocationV1::new_model_only(
                original.snapshot_model_only().owner_occurrence,
                changed,
                original.snapshot_model_only().initialized,
                original.snapshot_model_only().content_generation,
            )
            .unwrap(),
        );
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(plan(), supplied)
                .unwrap_err()
                .error,
            R57PreparationErrorV1::CrossVm
        );
    }
}

#[test]
fn all_bindings_must_cover_their_full_extent() {
    for index in 0..3 {
        let mut offset = plan();
        offset.bindings[index].byte_offset = 1;
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(offset, owners(false))
                .unwrap_err()
                .error,
            R57PreparationErrorV1::NonFullExtent
        );

        let mut short = plan();
        short.bindings[index].byte_len -= 1;
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(short, owners(false))
                .unwrap_err()
                .error,
            R57PreparationErrorV1::InvalidPlan
        );
    }
}

#[test]
fn both_read_inputs_must_be_initialized() {
    for index in 0..2 {
        let mut supplied = owners(false);
        supplied[index] = owner(index, false);
        let failure = r57_prepare_three_binding_compute_model_only(plan(), supplied).unwrap_err();
        assert_eq!(failure.error, R57PreparationErrorV1::InputUninitialized);
    }
}

#[test]
fn write_only_output_may_open_initialized_or_uninitialized() {
    assert!(r57_prepare_three_binding_compute_model_only(plan(), owners(false)).is_ok());
    assert!(r57_prepare_three_binding_compute_model_only(plan(), owners(true)).is_ok());
}

#[test]
fn output_content_generation_must_have_one_successor() {
    let mut supplied = owners(false);
    let snapshot = supplied[2].snapshot_model_only();
    supplied[2] = R57OwnedAllocationV1::new_model_only(
        snapshot.owner_occurrence,
        snapshot.identity,
        snapshot.initialized,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(
        r57_prepare_three_binding_compute_model_only(plan(), supplied)
            .unwrap_err()
            .error,
        R57PreparationErrorV1::OutputGenerationExhausted
    );
}

#[test]
fn distinct_allocation_and_storage_identities_are_required() {
    let mut duplicate_allocation = plan();
    duplicate_allocation.bindings[2].allocation = duplicate_allocation.bindings[0].allocation;
    assert_eq!(
        r57_prepare_three_binding_compute_model_only(duplicate_allocation, owners(false))
            .unwrap_err()
            .error,
        R57PreparationErrorV1::InvalidPlan
    );
    let mut duplicate_storage = plan();
    duplicate_storage.bindings[2].storage = duplicate_storage.bindings[0].storage;
    assert_eq!(
        r57_prepare_three_binding_compute_model_only(duplicate_storage, owners(false))
            .unwrap_err()
            .error,
        R57PreparationErrorV1::InvalidPlan
    );
}

#[test]
fn owner_occurrences_are_pairwise_distinct_even_with_distinct_storage() {
    let mut supplied = owners(false);
    let original = supplied[2].snapshot_model_only();
    supplied[2] = R57OwnedAllocationV1::new_model_only(
        supplied[0].snapshot_model_only().owner_occurrence,
        original.identity,
        original.initialized,
        original.content_generation,
    )
    .unwrap();
    let expected: Vec<_> = supplied
        .iter()
        .map(R57OwnedAllocationV1::snapshot_model_only)
        .collect();
    let failure = r57_prepare_three_binding_compute_model_only(plan(), supplied).unwrap_err();
    assert_eq!(
        failure.error,
        R57PreparationErrorV1::DuplicateOwnerOccurrence
    );
    assert_eq!(failure.owner_snapshots_model_only(), expected);
}

#[test]
fn elementwise_profile_requires_equal_full_extents() {
    for index in 0..3 {
        let mut unequal = plan();
        unequal.bindings[index].byte_len *= 2;
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(unequal, owners(false))
                .unwrap_err()
                .error,
            R57PreparationErrorV1::InvalidPlan
        );
    }
}

#[test]
fn packet_plan_is_exactly_wait_then_dispatch() {
    let plan = plan();
    assert_eq!(R57_PACKET_COUNT_V1, 2);
    assert_eq!(plan.frontier_after_model_only(), Some(39));
    assert_eq!(plan.wait_packet.kind, R57PacketKindV1::WaitForPrior);
    assert_eq!(plan.wait_packet.order, 0);
    assert_eq!(plan.dispatch_packet.kind, R57PacketKindV1::Dispatch);
    assert_eq!(plan.dispatch_packet.order, 1);
}

#[test]
fn malformed_packet_identities_orders_and_frontier_are_rejected() {
    let mut mutations = Vec::new();
    let mut value = plan();
    value.wait_packet.kind = R57PacketKindV1::Dispatch;
    mutations.push(value);
    let mut value = plan();
    value.wait_packet.order = 1;
    mutations.push(value);
    let mut value = plan();
    value.dispatch_packet.kind = R57PacketKindV1::WaitForPrior;
    mutations.push(value);
    let mut value = plan();
    value.dispatch_packet.order = 0;
    mutations.push(value);
    let mut value = plan();
    value.dispatch_packet.packet = value.wait_packet.packet;
    mutations.push(value);
    let mut value = plan();
    value.frontier_before = u64::MAX - 1;
    mutations.push(value);
    for mutation in mutations {
        assert_eq!(
            r57_prepare_three_binding_compute_model_only(mutation, owners(false))
                .unwrap_err()
                .error,
            R57PreparationErrorV1::InvalidPlan
        );
    }
}

#[test]
fn prepublication_currentness_failure_restores_exact_fixed_owner_roster() {
    for script in [
        R57PublicationScriptV1::Complete,
        R57PublicationScriptV1::RejectBeforePublication,
        R57PublicationScriptV1::AmbiguousAfterWaitPublication,
        R57PublicationScriptV1::AmbiguousAfterDispatchPublication,
    ] {
        match r57_publish_three_binding_compute_model_only(prepared(false), false, script) {
            R57PublishOutcomeV1::Restored(restored) => {
                assert_eq!(
                    restored.reason,
                    R57RestorationReasonV1::OpeningCurrentnessRejected
                );
                assert_eq!(restored.owner_snapshots_model_only(), snapshots(false));
                assert_eq!(restored.opening_model_only(), snapshots(false));
                assert_eq!(restored.into_owners_model_only().len(), 3);
            }
            _ => panic!("currentness failure did not restore"),
        }
    }
}

#[test]
fn native_prepublication_rejection_restores_all_and_only_exact_owners() {
    match r57_publish_three_binding_compute_model_only(
        prepared(false),
        true,
        R57PublicationScriptV1::RejectBeforePublication,
    ) {
        R57PublishOutcomeV1::Restored(restored) => {
            assert_eq!(
                restored.reason,
                R57RestorationReasonV1::NativeRejectedBeforeEffect
            );
            assert_eq!(restored.owner_snapshots_model_only(), snapshots(false));
            assert_eq!(restored.opening_model_only(), snapshots(false));
        }
        _ => panic!("prepublication rejection did not restore"),
    }
}

#[test]
fn each_postpublication_ambiguity_quarantines_all_three_with_exact_prefix() {
    for (script, prefix, reason) in [
        (
            R57PublicationScriptV1::AmbiguousAfterWaitPublication,
            1,
            R57QuarantineReasonV1::AmbiguousAfterWaitPublication,
        ),
        (
            R57PublicationScriptV1::AmbiguousAfterDispatchPublication,
            2,
            R57QuarantineReasonV1::AmbiguousAfterDispatchPublication,
        ),
    ] {
        match r57_publish_three_binding_compute_model_only(prepared(false), true, script) {
            R57PublishOutcomeV1::Quarantined(quarantined) => {
                assert_eq!(quarantined.reason, reason);
                assert_eq!(quarantined.published_packet_prefix, prefix);
                assert_eq!(quarantined.owner_snapshots_model_only(), snapshots(false));
                assert_eq!(quarantined.opening_model_only(), snapshots(false));
            }
            _ => panic!("ambiguous publication did not quarantine"),
        }
    }
}

#[test]
fn quarantine_is_absorbing_and_retains_exact_identity() {
    let quarantined = match r57_publish_three_binding_compute_model_only(
        prepared(false),
        true,
        R57PublicationScriptV1::AmbiguousAfterWaitPublication,
    ) {
        R57PublishOutcomeV1::Quarantined(quarantined) => quarantined,
        _ => panic!("ambiguous publication did not quarantine"),
    };
    let before = quarantined.owner_snapshots_model_only();
    let plan = quarantined.plan_model_only();
    let prefix = quarantined.published_packet_prefix;
    let observed_again = quarantined.observe_again_model_only();
    assert_eq!(observed_again.owner_snapshots_model_only(), before);
    assert_eq!(observed_again.plan_model_only(), plan);
    assert_eq!(observed_again.published_packet_prefix, prefix);
}

#[test]
fn successful_publication_retains_all_owner_and_plan_identities() {
    let published = published(false);
    assert_eq!(published.plan_model_only(), plan());
    assert_eq!(published.owner_snapshots_model_only(), snapshots(false));
}

#[test]
fn timeout_retains_the_complete_published_transaction() {
    let published = published(false);
    let before = published.owner_snapshots_model_only();
    let plan = published.plan_model_only();
    match r57_wait_three_binding_compute_model_only(
        published,
        true,
        R57CompletionObservationV1::Timeout,
    ) {
        R57WaitOutcomeV1::Pending(pending) => {
            assert_eq!(pending.owner_snapshots_model_only(), before);
            assert_eq!(pending.plan_model_only(), plan);
        }
        _ => panic!("timeout did not remain pending"),
    }
}

#[test]
fn exact_completion_advances_frontier_and_only_output_content_state() {
    let opening = snapshots(false);
    let completed = match r57_wait_three_binding_compute_model_only(
        published(false),
        true,
        R57CompletionObservationV1::Completed(completion()),
    ) {
        R57WaitOutcomeV1::Completed(completed) => completed,
        _ => panic!("exact completion did not complete"),
    };
    assert_eq!(completed.completion.frontier_after, 39);
    assert_eq!(completed.opening_model_only(), opening);
    let closed = completed.owner_snapshots_model_only();
    for index in 0..3 {
        assert_eq!(
            closed[index].owner_occurrence,
            opening[index].owner_occurrence
        );
        assert_eq!(closed[index].identity, opening[index].identity);
    }
    assert_eq!(closed[0].initialized, opening[0].initialized);
    assert_eq!(closed[1].initialized, opening[1].initialized);
    assert_eq!(closed[0].content_generation, opening[0].content_generation);
    assert_eq!(closed[1].content_generation, opening[1].content_generation);
    assert!(closed[2].initialized);
    assert_eq!(
        closed[2].content_generation,
        opening[2].content_generation + 1
    );
    assert_eq!(completed.into_owners_model_only().len(), 3);
}

#[test]
fn initialized_output_still_advances_exactly_one_content_generation() {
    let opening = snapshots(true);
    let completed = match r57_wait_three_binding_compute_model_only(
        published(true),
        true,
        R57CompletionObservationV1::Completed(completion()),
    ) {
        R57WaitOutcomeV1::Completed(completed) => completed,
        _ => panic!("exact completion did not complete"),
    };
    let closed = completed.owner_snapshots_model_only();
    assert!(opening[2].initialized);
    assert!(closed[2].initialized);
    assert_eq!(
        closed[2].content_generation,
        opening[2].content_generation + 1
    );
}

#[test]
fn completion_after_any_number_of_timeouts_is_still_exact() {
    let mut published = published(false);
    for _ in 0..4 {
        published = match r57_wait_three_binding_compute_model_only(
            published,
            true,
            R57CompletionObservationV1::Timeout,
        ) {
            R57WaitOutcomeV1::Pending(pending) => pending,
            _ => panic!("timeout did not remain pending"),
        };
    }
    assert!(matches!(
        r57_wait_three_binding_compute_model_only(
            published,
            true,
            R57CompletionObservationV1::Completed(completion())
        ),
        R57WaitOutcomeV1::Completed(_)
    ));
}

#[test]
fn every_completion_coordinate_is_authenticated() {
    for field in 0..6 {
        let mut wrong = completion();
        match field {
            0 => wrong.queue += 1,
            1 => wrong.queue_generation += 1,
            2 => wrong.dispatch += 1,
            3 => wrong.transaction_generation += 1,
            4 => wrong.completion_signal += 1,
            _ => wrong.frontier_after += 1,
        }
        match r57_wait_three_binding_compute_model_only(
            published(false),
            true,
            R57CompletionObservationV1::Completed(wrong),
        ) {
            R57WaitOutcomeV1::Quarantined(quarantined) => {
                assert_eq!(
                    quarantined.reason,
                    R57QuarantineReasonV1::CompletionIdentityMismatch
                );
                assert_eq!(quarantined.published_packet_prefix, R57_PACKET_COUNT_V1);
                assert_eq!(quarantined.owner_snapshots_model_only(), snapshots(false));
            }
            _ => panic!("mismatched completion did not quarantine"),
        }
    }
}

#[test]
fn closing_currentness_loss_quarantines_all_three_after_both_packets() {
    match r57_wait_three_binding_compute_model_only(
        published(false),
        false,
        R57CompletionObservationV1::Completed(completion()),
    ) {
        R57WaitOutcomeV1::Quarantined(quarantined) => {
            assert_eq!(
                quarantined.reason,
                R57QuarantineReasonV1::ClosingCurrentnessRejected
            );
            assert_eq!(quarantined.published_packet_prefix, R57_PACKET_COUNT_V1);
            assert_eq!(quarantined.owner_snapshots_model_only(), snapshots(false));
        }
        _ => panic!("closing-currentness loss did not quarantine"),
    }
}

#[test]
fn invalid_owner_construction_is_closed() {
    let valid = identity(0);
    assert!(R57OwnedAllocationV1::new_model_only(0, valid, true, 1).is_none());
    assert!(R57OwnedAllocationV1::new_model_only(1, valid, true, 0).is_none());
    let mut host = valid;
    host.memory_kind = R57MemoryKindV1::Host;
    assert!(R57OwnedAllocationV1::new_model_only(1, host, true, 1).is_none());
}
