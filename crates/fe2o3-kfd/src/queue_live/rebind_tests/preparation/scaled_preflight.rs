use super::*;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

#[test]
fn scaled_rebind_credit_exhaustion_preserves_continuation_and_exact_inputs() {
    for pristine in [false, true] {
        for record_exhaustion in [false, true] {
            let (fixture, data) = fixture();
            let account = ResourceCreditAccountV1::new(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 4 << 20),
                if record_exhaustion { 1 } else { 2 },
            )
            .unwrap();
            let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
            let continuation = pristine
                .then(|| PristineDispatchContinuationV1::from_fresh_capacity_for_test(&capacity));
            let competing = account
                .reserve(ResourceVectorV1::ZERO.with(
                    ResourceKindV1::ControlResidentBytes,
                    if record_exhaustion { 1 } else { 4 << 20 },
                ))
                .unwrap();
            let mut key = test_queue_key(710, 4);
            key.vm = fixture.foundation.identity().vms()[0].key;
            let mut session = persistent_compute_cancellation_test_session(key, None, None);
            session.dispatch_capacity = capacity;
            session.observation.ring_bytes = 4096;
            session.detached_dispatch_generation = (!pristine).then_some(7);
            session.unpublished_dispatch.continuation = continuation;
            session.detached_data_count = data.len();
            session.detached_data_identities = fixed_dispatch_storage_identities(&data);
            session.detached_next_insertion_index = Some(data.len());
            let continuation = session
                .unpublished_dispatch
                .continuation
                .as_ref()
                .map(std::ptr::from_ref);
            let identities = session.detached_data_identities.clone();
            let data_storage = (data.as_ptr(), data.len(), data.capacity());
            let before = fixture.memory.observation();
            let usage = account.usage();
            let root = LiveRebindRootV1::new(
                programs(),
                [packet(0)],
                data,
                session.detached_dispatch_generation,
            );
            let address = std::ptr::from_ref(&*root);
            let retained = RefCell::new(None);
            let _ = take_dispatch_terminal_process_gate_record_v1();
            let result = session.settle_fixed_dispatch_rebind_with_v1(
                root,
                |_, _, _, _, _, _| panic!("exhausted credit entered preparation"),
                |_, _| panic!("exhausted credit entered validation"),
                |root| *retained.borrow_mut() = Some(root),
            );
            assert!(matches!(
                result.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. }
                )))
            ));
            assert!(!result.transport && !session.terminal_poisoned);
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert_eq!(
                session
                    .unpublished_dispatch
                    .continuation
                    .as_ref()
                    .map(std::ptr::from_ref),
                continuation
            );
            assert_eq!(
                session.detached_dispatch_generation,
                (!pristine).then_some(7)
            );
            assert_eq!(session.detached_data_identities, identities);
            assert_eq!(session.detached_data_count, data_storage.1);
            if let Some(continuation) = &session.unpublished_dispatch.continuation {
                assert_eq!(continuation.next_generation_for_test(), 1);
                assert!(continuation.matches_capacity(&session.dispatch_capacity));
            }
            assert_eq!(session.detached_next_insertion_index, Some(data_storage.1));
            let retained = retained.into_inner().unwrap();
            assert_eq!(std::ptr::from_ref(&*retained), address);
            let data = retained.data.as_ref().unwrap();
            assert_eq!((data.as_ptr(), data.len(), data.capacity()), data_storage);
            assert_eq!(fixed_dispatch_storage_identities(data), identities);
            assert!(retained.packets.is_some());
            assert!(retained.preparation.is_none() && retained.prepared_generation.is_none());
            assert!(retained.continuation.is_none());
            assert_eq!(fixture.memory.observation(), before);
            assert_eq!(account.usage(), usage);
            drop(retained);
            drop(competing);
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        }
    }
}

#[test]
fn scaled_rebind_post_entry_failures_retain_the_preallocated_debit() {
    for pristine in [false, true] {
        for generation_failure in [false, true] {
            for panic in [false, true] {
                let (mut fixture, data) = fixture();
                let account = ResourceCreditAccountV1::new(
                    ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 4 << 20),
                    1,
                )
                .unwrap();
                let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
                let continuation = pristine.then(|| {
                    PristineDispatchContinuationV1::from_fresh_capacity_for_test(&capacity)
                });
                let mut key = test_queue_key(711, 4);
                key.vm = fixture.foundation.identity().vms()[0].key;
                let mut session = persistent_compute_cancellation_test_session(key, None, None);
                session.dispatch_capacity = capacity.clone();
                session.observation.ring_bytes = 4096;
                session.detached_dispatch_generation = (!pristine).then_some(7);
                session.unpublished_dispatch.continuation = continuation;
                session.detached_data_count = data.len();
                session.detached_data_identities = fixed_dispatch_storage_identities(&data);
                session.detached_next_insertion_index = Some(data.len());
                let root = LiveRebindRootV1::new(
                    programs(),
                    [packet(0)],
                    data,
                    session.detached_dispatch_generation,
                );
                let retained = RefCell::new(None);
                let reserved_usage = RefCell::new(None);
                let _ = take_dispatch_terminal_process_gate_record_v1();
                let result = session.settle_fixed_dispatch_rebind_with_v1(
                root,
                |_, programs, preparation, predecessor, continuation, prepared| {
                    assert_eq!(account.usage().retained_records, 1);
                    *reserved_usage.borrow_mut() = Some(account.usage());
                    assert!(prepared.is_some());
                    if !generation_failure {
                        assert!(!panic, "opening panic");
                        return Err(ComputeAqlQueueSessionErrorV1::Contract("opening fault"));
                    }
                    preparation.primary_inject_stage_v1(PreparationStageV1::Generation, panic);
                    match predecessor {
                        Some(previous) => crate::queue::dispatch_binding::prepare_public_fixed_dispatch_resources_after_detach_with_capacity_in_place(
                            &mut fixture.memory, programs, preparation, previous, &capacity, prepared,
                        ),
                        None => prepare_public_fixed_dispatch_resources_after_pristine_abort_in_place_v1(
                            &mut fixture.memory, programs, preparation, continuation, prepared,
                        ),
                    }.map_err(Into::into)
                },
                |_, _| panic!("failed preparation entered validation"),
                |root| *retained.borrow_mut() = Some(root),
            );
                assert_eq!(result.result.is_err(), panic);
                assert!(!matches!(result.result, Ok(Ok(()))));
                assert!(result.transport && session.terminal_poisoned);
                assert_eq!(
                    take_dispatch_terminal_process_gate_record_v1(),
                    pristine || panic
                );
                let retained = retained.into_inner().unwrap();
                assert_eq!(retained.prepared_generation.is_some(), !generation_failure);
                assert_eq!(
                    retained.continuation.is_some(),
                    pristine && !generation_failure
                );
                assert_eq!(account.usage().retained_records, 1);
                assert_eq!(account.usage(), reserved_usage.into_inner().unwrap());
                // Only CPU fixtures are dropped here; native terminal owners stay retained.
                drop(retained);
                assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
            }
        }
    }
}
