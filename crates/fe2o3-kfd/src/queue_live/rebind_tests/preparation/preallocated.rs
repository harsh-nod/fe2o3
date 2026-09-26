//! Borrowed public reservations handed into the actual rebind sequencer.

use super::*;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

fn capacity() -> (Gfx942FixedDispatchCapacityV1, ResourceCreditAccountV1) {
    let probe = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 4 << 20),
        1,
    )
    .unwrap();
    let token = Gfx942FixedDispatchCapacityV1::qualification_1024(probe.clone())
        .preallocate_fresh_v1::<1>()
        .unwrap();
    let budget = probe.usage().used;
    drop(token);
    let account = ResourceCreditAccountV1::new(budget, 1).unwrap();
    (
        Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone()),
        account,
    )
}

#[test]
fn scaled_preallocated_detached_and_pristine_rebind_transfer_one_table_through_validation() {
    for pristine in [false, true] {
        let (mut fixture, data) = fixture();
        let (capacity, account) = capacity();
        let mut key = test_queue_key(810, 4);
        key.vm = fixture.foundation.identity().vms()[0].key;
        let mut session = persistent_compute_cancellation_test_session(key, None, None);
        session.dispatch_capacity = capacity.clone();
        session.observation.ring_bytes = 4096;
        session.detached_dispatch_generation = (!pristine).then_some(7);
        session.unpublished_dispatch.continuation = pristine
            .then(|| PristineDispatchContinuationV1::from_fresh_capacity_for_test(&capacity));
        session.detached_data_count = data.len();
        session.detached_data_identities = fixed_dispatch_storage_identities(&data);
        session.detached_next_insertion_index = Some(data.len());
        let memory_before = fixture.memory.observation();
        let lane = session.primary_compute_lane_v1();
        let token = session
            .preallocate_next_fixed_dispatch_v1::<1>(lane)
            .unwrap();
        let usage = account.usage();
        assert_eq!(usage.retained_records, 1);
        assert_eq!(fixture.memory.observation(), memory_before);
        assert!(
            session
                .preallocate_next_fixed_dispatch_v1::<1>(lane)
                .is_err()
        );
        assert_eq!(account.usage(), usage);
        let mut root = LiveRebindRootV1::new(
            programs(),
            [packet(0)],
            data,
            session.detached_dispatch_generation,
        );
        root.prepared_generation = token;
        let fixture = RefCell::new(&mut fixture);
        let result = session.settle_fixed_dispatch_rebind_with_v1(
            root,
            |_, programs, preparation, predecessor, continuation, prepared| {
                assert_eq!(account.usage(), usage);
                let mut f = fixture.borrow_mut();
                let f = &mut **f;
                let loan = f.memory.primary_loan(&mut f.foundation)?;
                let operation = match predecessor {
                    Some(previous) => crate::queue::dispatch_binding::prepare_public_fixed_dispatch_resources_after_detach_with_capacity_in_place(
                        &mut f.memory, programs, preparation, previous, &capacity, prepared,
                    ),
                    None => prepare_public_fixed_dispatch_resources_after_pristine_abort_in_place_v1(
                        &mut f.memory, programs, preparation, continuation, prepared,
                    ),
                };
                f.memory.primary_reclaim(&mut f.foundation, loan)?;
                operation.map_err(Into::into)
            },
            |_, preparation| {
                let authorities = preparation.completed()?.device_authorities_inline_v1();
                fixture.borrow_mut().memory.primary_validate_live_dispatch_memory_v1(&authorities)
                    .map_err(Into::into)
            },
            |_| panic!("valid preallocation must install"),
        );
        assert!(matches!(result.result, Ok(Ok(()))) && !result.transport);
        assert!(!session.terminal_poisoned);
        assert!(session.unpublished_dispatch.is_clear());
        assert!(session.detached_dispatch_generation.is_none());
        assert_eq!(session.detached_data_count, 0);
        assert!(session.detached_data_identities.is_empty());
        assert!(session.detached_next_insertion_index.is_none());
        let dispatch = session.dispatch.as_mut().unwrap();
        assert_eq!(
            dispatch.primary_fixture_next_generation_v1(),
            if pristine { 1 } else { 8 }
        );
        dispatch.primary_fixture_exercise_capacity_v1(&capacity);
        assert_eq!(account.usage(), usage);
        drop(session);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn scaled_preallocated_rebind_rejects_fresh_wrong_queue_and_stale_tokens_before_entry() {
    for case in 0..4 {
        let (fixture, data) = fixture();
        let (capacity, account) = capacity();
        let mut key = test_queue_key(811, 4);
        key.vm = fixture.foundation.identity().vms()[0].key;
        let mut session = persistent_compute_cancellation_test_session(key, None, None);
        session.dispatch_capacity = capacity.clone();
        session.observation.ring_bytes = 4096;
        session.detached_dispatch_generation = Some(0);
        session.detached_data_count = data.len();
        session.detached_data_identities = fixed_dispatch_storage_identities(&data);
        session.detached_next_insertion_index = Some(data.len());
        let token = if case == 3 {
            let mut auxiliary_key = key;
            auxiliary_key.id.0 += 1;
            let mut auxiliary = compute_lane_state_for_multi_inflight_test(auxiliary_key);
            auxiliary.detached_dispatch_generation = Some(0);
            session
                .auxiliary_compute_lanes
                .push(AuxiliaryComputeLaneSlotV1 {
                    generation: 9,
                    state: Some(auxiliary),
                });
            session
                .preallocate_next_fixed_dispatch_v1::<1>(ComputeAqlQueueLaneV1 {
                    session: session.compute_lane_session,
                    ordinal: 1,
                    generation: 9,
                })
                .unwrap()
        } else if case == 0 {
            capacity.preallocate_fresh_v1::<1>().unwrap()
        } else {
            session
                .preallocate_next_fixed_dispatch_v1::<1>(session.primary_compute_lane_v1())
                .unwrap()
        };
        if case == 1 {
            session.key.generation.0 += 1;
        }
        if case == 2 {
            session.detached_dispatch_generation = Some(7);
        }
        let usage = account.usage();
        let before = fixture.memory.observation();
        let storage = (data.as_ptr(), data.len(), data.capacity());
        let mut root = LiveRebindRootV1::new(
            programs(),
            [packet(0)],
            data,
            session.detached_dispatch_generation,
        );
        root.prepared_generation = token;
        let retained = RefCell::new(None);
        let _ = take_dispatch_terminal_process_gate_record_v1();
        let result = session.settle_fixed_dispatch_rebind_with_v1(
            root,
            |_, _, _, _, _, _| panic!("invalid token entered preparation"),
            |_, _| panic!("invalid token entered validation"),
            |root| *retained.borrow_mut() = Some(root),
        );
        assert!(!result.transport);
        match (case, result.result) {
            (
                0 | 1 | 3,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::WrongQueueGeneration,
                ))),
            )
            | (
                2,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase,
                ))),
            ) => {}
            (_, other) => panic!("unexpected preallocation rejection: {other:?}"),
        }
        assert!(!session.terminal_poisoned && !take_dispatch_terminal_process_gate_record_v1());
        let root = retained.into_inner().unwrap();
        let data = root.data.as_ref().unwrap();
        assert_eq!((data.as_ptr(), data.len(), data.capacity()), storage);
        assert!(root.prepared_generation.is_some() && root.preparation.is_none());
        assert_eq!(fixture.memory.observation(), before);
        assert_eq!(account.usage(), usage);
        drop(root);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn scaled_borrowed_preallocation_selects_exact_lane_without_mutating_parent() {
    use super::super::data_release::{poison_snapshot, selected_parent, snapshot};
    for ordinal in 0..3 {
        let (mut session, lane) = selected_parent(ordinal);
        let (capacity, account) = capacity();
        session.dispatch_capacity = capacity;
        let key = if ordinal == 0 {
            session.key
        } else {
            session.auxiliary_compute_lanes[ordinal - 1]
                .state
                .as_ref()
                .unwrap()
                .key
        };
        let before = snapshot(&session);
        let poison = poison_snapshot(&session);
        let usage = account.usage();
        let mut stale = lane;
        stale.generation += 1;
        assert!(
            matches!(session.preallocate_next_fixed_dispatch_v1::<1>(stale),
            Err(ComputeAqlQueueSessionErrorV1::Contract(message))
                if message == if ordinal == 0 { "stale primary compute queue lane" }
                    else { "stale compute queue lane" })
        );
        assert_eq!(account.usage(), usage);
        let token = session
            .preallocate_next_fixed_dispatch_v1::<1>(lane)
            .unwrap();
        PreparedDispatchGenerationV1::validate_target(&token, Some(key)).unwrap();
        if ordinal != 0 {
            assert!(matches!(
                PreparedDispatchGenerationV1::validate_target(&token, Some(session.key)),
                Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration)
            ));
        }
        let reserved = account.usage();
        assert_eq!(reserved.retained_records, 1);
        assert!(matches!(
            session.preallocate_next_fixed_dispatch_v1::<1>(lane),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. }
            ))
        ));
        assert_eq!(account.usage(), reserved);
        assert_eq!(snapshot(&session), before);
        assert_eq!(poison_snapshot(&session), poison);
        drop(token);
        assert_eq!(account.usage(), usage);
    }
}
