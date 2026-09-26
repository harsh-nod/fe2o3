//! Real construction sequencers with CPU native fixtures, not GPU execution.
use super::*;
use crate::queue::dispatch_binding::prepare_public_fixed_dispatch_resources_with_capacity_in_place;
use crate::queue::live::construction_auxiliary::AuxiliaryConstructionV1;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

pub(super) fn capacity() -> (Gfx942FixedDispatchCapacityV1, ResourceCreditAccountV1) {
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 4 << 20),
        4,
    )
    .unwrap();
    (
        Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone()),
        account,
    )
}

#[test]
fn scaled_primary_and_replacement_construct_with_exact_account_and_1024_slots() {
    for replacement in [false, true] {
        let (mut memory, _) = setup_memory();
        let (programs, [packet, _, _]) = recipe();
        let preparation = FixedDispatchPreparationCustodyV1::new([packet], memory.roster());
        let (capacity, account) = capacity();
        let destroyed =
            ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(43, 5);
        let mut root = Root::new_with(memory, (destroyed, 7, programs, preparation));
        root.dispatch_capacity = capacity.clone();
        let (mut root, result) = run_work(root, |root, entry| {
            if replacement {
                root.construct_replacement(entry, 65_536)
            } else {
                prepare_public_fixed_dispatch_resources_with_capacity_in_place(
                    root.memory.as_mut().unwrap(),
                    &root.preparation.2,
                    &mut root.preparation.3,
                    &root.dispatch_capacity,
                )?;
                root.dispatch = Some(root.preparation.3.take_completed()?);
                root.construct(
                    entry,
                    queue_resource_plan_for_test_v1(65_536),
                    65_536,
                    QueueRingBackingV1::AqlSpecial,
                    None,
                )
            }
        });
        assert!(result.is_ok());
        let complete = root.completed.as_mut().unwrap();
        assert_eq!(
            complete.dispatch_capacity.profile(),
            Gfx942FixedDispatchCapacityProfileV1::Qualification1024
        );
        let dispatch = complete.dispatch.as_mut().unwrap();
        assert_eq!(
            dispatch.primary_fixture_next_generation_v1(),
            if replacement { 8 } else { 1 }
        );
        dispatch.primary_fixture_exercise_capacity_v1(&capacity);
        assert_eq!(account.usage().retained_records, 1);
        drop(root);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn scaled_auxiliary_preparation_charges_shared_account_before_ring_allocation() {
    let (mut memory, t) = setup_memory();
    let (programs, [packet, _, _]) = recipe();
    let (capacity, account) = capacity();
    let mut auxiliary =
        AuxiliaryConstructionV1::<1, Fixture>::with_capacity([packet], capacity.clone());
    let poison = || {};
    let mut entry = UserptrConstructionEntryV1 {
        stage: None,
        poison: &poison,
    };
    auxiliary
        .prepare_dispatch(
            &mut memory,
            &mut entry,
            queue_resource_plan_for_test_v1(65_536),
            65_536,
            &programs,
            |memory| Ok(memory.roster()),
        )
        .unwrap();
    entry.stage = None;
    auxiliary
        .dispatch
        .as_mut()
        .unwrap()
        .primary_fixture_exercise_capacity_v1(&capacity);
    let payload = account
        .usage()
        .used
        .get(ResourceKindV1::ControlResidentBytes);
    assert!(payload > 0);
    assert_eq!(account.usage().retained_records, 1);
    drop(auxiliary);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert!(t.borrow().calls.contains(&"allocate-ring"));

    let (mut memory, t) = setup_memory();
    let (programs, [packet, _, _]) = recipe();
    let short = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, payload - 1),
        1,
    )
    .unwrap();
    let mut snapshot = PrimaryPreparationSnapshotV1::packets(core::slice::from_ref(&packet));
    let mut auxiliary = AuxiliaryConstructionV1::<1, Fixture>::with_capacity(
        [packet],
        Gfx942FixedDispatchCapacityV1::qualification_1024(short.clone()),
    );
    let before = memory.observation();
    assert!(
        auxiliary
            .prepare_dispatch(
                &mut memory,
                &mut entry,
                queue_resource_plan_for_test_v1(65_536),
                65_536,
                &programs,
                |memory| {
                    let data = memory.roster();
                    snapshot.capture_data_vector_v1(&data, data.capacity());
                    Ok(data)
                }
            )
            .is_err()
    );
    assert!(!t.borrow().calls.contains(&"allocate-ring"));
    assert_ne!(memory.observation().calls, before.calls);
    assert_eq!(&memory.observation().calls[5..], &before.calls[5..]);
    auxiliary
        .preparation
        .as_ref()
        .unwrap()
        .primary_assert_snapshot_v1(&memory, &snapshot, None);
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_auxiliary_rejects_multi_packet_before_initializer() {
    let (mut memory, t) = setup_memory();
    let (programs, packets) = recipe();
    let (capacity, account) = capacity();
    let mut auxiliary = AuxiliaryConstructionV1::<3, Fixture>::with_capacity(packets, capacity);
    let before = memory.observation();
    let poison = || {};
    let mut entry = UserptrConstructionEntryV1 {
        stage: None,
        poison: &poison,
    };
    assert!(
        auxiliary
            .prepare_dispatch(
                &mut memory,
                &mut entry,
                queue_resource_plan_for_test_v1(65_536),
                65_536,
                &programs,
                |_| panic!("initializer must not run")
            )
            .is_err()
    );
    assert_eq!(memory.observation(), before);
    assert!(t.borrow().calls.is_empty());
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}
