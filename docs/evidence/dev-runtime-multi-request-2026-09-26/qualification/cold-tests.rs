use super::*;
use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;

#[test]
fn qualification_composed_multi_cold_settlement_refunds_only_selected_leaf_and_reuses_route() {
    for kind in [RuntimeMemoryKindV1::HostVisible, RuntimeMemoryKindV1::DeviceLocal] {
        let root = Entry::qualification_root_v1();
        let entries = [Entry::qualification_entry_v1(&root, 7), Entry::qualification_entry_v1(&root, 8)];
        let root_baseline = root.usage_v1();
        let mut backend = cold_multi_fixture(kind);
        let mut steps = vec![reject(kind, 8, false)];
        steps.extend(success(kind));
        if kind == RuntimeMemoryKindV1::DeviceLocal {
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice, 0, 8,
                ScriptedFailureModeV1::Success,
            ));
            steps.push(ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success));
        }
        steps.extend([
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        backend.children[0].scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
        for (child, entry) in backend.children.iter_mut().zip(&entries) {
            child.composed_request_binding = Some(entry.clone());
            child.rooted_backing = Some(crate::kfd_backend::native_budget::RootedBackingV1::Composed(None));
        }
        backend.request_policy = crate::kfd_backend::multi_admission::MultiRequestPolicyV1::Required;
        let mut context = ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
        let left = context.devices()[0].id();
        let before = entries[0].account().usage_v1();
        let other = entries[1].account().usage_v1();
        assert!(matches!(context.allocate(left, kind, 8, 8),
            Err(crate::RuntimeErrorV1::BackendQuiescent(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity));
        assert!(!context.is_terminal());
        assert_eq!(entries[0].account().usage_v1(), before);
        assert_eq!(entries[1].account().usage_v1(), other);
        assert!(context.backend_mut_for_test_v1().allocations.is_empty());
        assert_eq!(context.backend_mut_for_test_v1().next_handle, 1);
        let allocation = context.allocate(left, kind, 8, 8).unwrap();
        assert_eq!(entries[0].account().usage_v1().retained_records, 1);
        assert_eq!(entries[1].account().usage_v1(), other);
        let backend = context.backend_mut_for_test_v1();
        assert_eq!(backend.allocations[&1].child, 0);
        assert_eq!(backend.allocations[&1].local, 2);
        assert_eq!(backend.next_handle, 2);
        assert!(backend.children[1].allocations.is_empty());
        let retained = entries[0].account().usage_v1();
        assert!(matches!(context.release_allocation(allocation), Err(crate::RuntimeErrorV1::BackendQuiescent(_))));
        assert_eq!(entries[0].account().usage_v1(), retained);
        assert_eq!(entries[1].account().usage_v1(), other);
        assert_eq!(context.backend_mut_for_test_v1().allocations.len(), 1);
        assert_eq!(context.backend_mut_for_test_v1().children[0].allocations.len(), 1);
        context.release_allocation(allocation).unwrap();
        assert!(context.backend_mut_for_test_v1().allocations.is_empty());
        for entry in &entries {
            let usage = entry.account().usage_v1();
            assert_eq!(usage.used, crate::RuntimeResourceVectorV1::ZERO);
            assert_eq!((usage.reserved_records, usage.retained_records, usage.quarantined_records), (0, 0, 0));
        }
        let usage = root.usage_v1();
        assert_eq!(usage.used.get(crate::RuntimeResourceKindV1::RequestedAllocationBytes), 0);
        assert_eq!(usage.used.get(crate::RuntimeResourceKindV1::AllocationRecords), 0);
        assert_eq!(usage, root_baseline);
        for child in &context.backend_mut_for_test_v1().children {
            assert!(child.allocations.is_empty());
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!((driver.remaining_steps(), driver.live_owner_count(), driver.unexpected_drops()), (0, 0, 0));
        }
        let mut backend = ManuallyDrop::into_inner(context).shutdown().unwrap();
        backend.shutdown_native_v1().unwrap();
        assert!(backend.children.iter().all(|child| child.queue_retired));
        drop(backend);
    }
}
