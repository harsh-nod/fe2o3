//! Real detach/rebind sequencers and simulated memory, not Linux queue execution.

use super::*;
use crate::queue::dispatch_binding::prepare_public_fixed_dispatch_resources_after_detach_with_capacity_in_place;
use crate::queue::live::rebind::LiveRebindRootV1;
use crate::queue::live::tests::persistent_compute_cancellation_test_session;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

#[test]
fn scaled_attached_preallocation_survives_canceled_history_detach_and_rebind() {
    let probe = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 4 << 20),
        1,
    )
    .unwrap();
    let token = Gfx942FixedDispatchCapacityV1::qualification_1024(probe.clone())
        .preallocate_fresh_v1::<1>()
        .unwrap();
    let bytes = probe.usage().used.get(ResourceKindV1::ControlResidentBytes);
    drop(token);
    for tables in [1, 2] {
        let account = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, tables * bytes),
            usize::try_from(tables).unwrap(),
        )
        .unwrap();
        let capacity = Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
        let mut f = DetachFixture::new_with_capacity(0, 1, &capacity);
        let mut facade = persistent_compute_cancellation_test_session(f.key, None, None);
        facade.dispatch_capacity = capacity.clone();
        facade.observation.ring_bytes = 4096;
        facade.dispatch = f.context().dispatch().take();
        let owner = facade.dispatch.as_mut().unwrap();
        for _ in 0..2 {
            let (_, epoch) = owner.bind_templates::<1>(f.key).unwrap();
            owner.cancel_binding(epoch).unwrap();
        }
        assert_eq!(owner.ensure_returnable().unwrap(), f.generation);
        let next = owner.primary_fixture_next_generation_v1();
        assert!(next > f.generation + 1);
        let authorities = owner
            .device_authorities_inline_v1()
            .iter()
            .map(|authority| authority.storage_identity())
            .collect::<Vec<_>>();
        let before = f.memory().observation();
        assert_eq!(account.usage().retained_records, 1);
        let reserved =
            facade.preallocate_next_fixed_dispatch_v1::<1>(facade.primary_compute_lane_v1());
        assert_eq!(f.memory().observation(), before);
        let owner = facade.dispatch.as_ref().unwrap();
        assert_eq!(
            owner
                .device_authorities_inline_v1()
                .iter()
                .map(|authority| authority.storage_identity())
                .collect::<Vec<_>>(),
            authorities
        );
        assert_eq!(owner.primary_fixture_next_generation_v1(), next);
        assert_eq!(owner.ensure_returnable().unwrap(), f.generation);
        *f.context().dispatch() = facade.dispatch.take();
        if tables == 1 {
            assert!(matches!(
                reserved,
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. }
                ))
            ));
            assert_eq!(account.usage().retained_records, 1);
            assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (0, 0, 0));
            drop(f);
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
            continue;
        }
        let token = reserved.unwrap();
        assert_eq!(account.usage().retained_records, 2);
        let result = f.detach();
        assert!(!result.transport);
        let returned = result.result.unwrap().unwrap();
        assert_eq!(returned.dispatch_generation(), f.generation);
        let data = returned.into_data();
        assert_eq!(data_snapshot(&data), f.expected);
        assert_eq!((f.trace.loans, f.trace.entered, f.trace.poisons), (1, 1, 0));
        assert_eq!(account.usage().retained_records, 1);
        facade.detached_dispatch_generation = f.primary.generation.take();
        facade.detached_data_count = core::mem::take(&mut f.primary.count);
        facade.detached_data_identities = core::mem::take(&mut f.primary.identities);
        facade.detached_next_insertion_index = f.primary.next.take();
        let (programs, [packet, _, _]) = recipe();
        let mut root = LiveRebindRootV1::new(
            programs,
            [packet],
            data,
            facade.detached_dispatch_generation,
        );
        root.prepared_generation = token;
        let fixture = RefCell::new(&mut f);
        let result = facade.settle_fixed_dispatch_rebind_with_v1(
            root,
            |_, programs, preparation, predecessor, _, prepared| {
                let (operation, retake) = fixture
                    .borrow_mut()
                    .scope
                    .parent
                    .with_preparation_custody(|memory| {
                        prepare_public_fixed_dispatch_resources_after_detach_with_capacity_in_place(
                            memory,
                            programs,
                            preparation,
                            predecessor.unwrap(),
                            &capacity,
                            prepared,
                        )
                        .map_err(Into::into)
                    })?;
                retake?;
                operation
            },
            |_, preparation| {
                let authorities = preparation.completed()?.device_authorities_inline_v1();
                fixture
                    .borrow_mut()
                    .memory_mut()
                    .primary_validate_live_dispatch_memory_v1(&authorities)
                    .map_err(Into::into)
            },
            |_| panic!("valid preallocated rebind must install"),
        );
        assert!(matches!(result.result, Ok(Ok(()))) && !result.transport);
        assert!(!facade.terminal_poisoned);
        assert!(facade.detached_dispatch_generation.is_none());
        assert_eq!(facade.detached_data_count, 0);
        assert!(facade.detached_data_identities.is_empty());
        assert!(facade.detached_next_insertion_index.is_none());
        let dispatch = facade.dispatch.as_mut().unwrap();
        assert_eq!(
            dispatch.primary_fixture_next_generation_v1(),
            f.generation + 1
        );
        dispatch.primary_fixture_exercise_capacity_v1(&capacity);
        assert_eq!(account.usage().retained_records, 1);
        drop(facade);
        drop(f);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}
