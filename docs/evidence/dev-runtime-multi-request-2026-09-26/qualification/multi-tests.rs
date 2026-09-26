use super::*;
use crate::{RuntimeAllocationDeviceAdmissionV1 as Entry, RuntimeAllocationAdmissionProfileV1 as Profile,
    RuntimeContextV1, RuntimeResourceKindV1 as K, RuntimeResourceVectorV1 as V,
    RuntimeRequestAllocationResultV1 as ResultV1};

fn child(entry: Entry) -> KfdRuntimeBackendV1 {
    let key = entry.backend_device_v1();
    let mut backend = KfdRuntimeBackendV1::qualification_composed_v1(entry);
    backend.description.backend_device = key;
    backend
}

fn fixture(a: &Entry, b: &Entry) -> KfdMultiDeviceRuntimeBackendV1 {
    KfdMultiDeviceRuntimeBackendV1::from_backends(vec![child(a.clone()), child(b.clone())]).unwrap()
}

fn assert_profile(backend: &KfdMultiDeviceRuntimeBackendV1, expected: &[&Entry]) {
    let Profile::Required(entries) = backend.allocation_admission_profile_v1().unwrap() else {
        panic!("required policy lost");
    };
    assert_eq!(entries.len(), expected.len());
    for (entry, expected) in entries.iter().zip(expected) {
        assert_eq!(entry.backend_device_v1(), expected.backend_device_v1());
        assert_eq!(entry.model(), expected.model());
        assert!(entry.account().shares_account_with_v1(expected.account()));
    }
}

#[test]
fn qualification_multi_context_uses_exact_leaf_and_shared_root_through_disposal() {
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 7);
    let b = Entry::qualification_entry_v1(&root, 8);
    let mut context = RuntimeContextV1::open(fixture(&a, &b)).unwrap();
    let root_baseline = root.usage_v1();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for device in devices {
        assert!(context.configure_allocation_admission_v1(device, 1024, 16).is_err());
    }
    let left = context.allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 24, 8).unwrap();
    assert_eq!(a.account().usage_v1().used, V::ZERO.with(K::RequestedAllocationBytes, 24).with(K::AllocationRecords, 1));
    assert_eq!(b.account().usage_v1().used, V::ZERO);
    let right = context.allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 40, 8).unwrap();
    assert_eq!(b.account().usage_v1().used, V::ZERO.with(K::RequestedAllocationBytes, 40).with(K::AllocationRecords, 1));
    assert_eq!(root.usage_v1().used.get(K::RequestedAllocationBytes), 64);
    let backend = context.backend_mut_for_test_v1();
    assert_eq!(backend.allocations.len(), 2);
    assert_eq!(backend.children[0].allocations.len(), 1);
    assert_eq!(backend.children[1].allocations.len(), 1);
    context.release_allocation(left).unwrap();
    assert_eq!(a.account().usage_v1().used, V::ZERO);
    assert_eq!(b.account().usage_v1().retained_records, 1);
    context.release_allocation(right).unwrap();
    let mut backend = context.shutdown().unwrap();
    assert_profile(&backend, &[&a, &b]);
    for key in [7, 8] {
        assert!(matches!(backend.allocate_v1(key, RuntimeMemoryKindV1::HostVisible, 8, 8),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported));
    }
    backend.shutdown_native_v1().unwrap();
    assert_profile(&backend, &[&a, &b]);
    assert_eq!(root.usage_v1().used.get(K::RequestedAllocationBytes), 0);
    assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 0);
    for entry in [&a, &b] {
        let usage = entry.account().usage_v1();
        assert_eq!(usage.used, V::ZERO);
        assert_eq!((usage.reserved_records, usage.retained_records, usage.quarantined_records), (0, 0, 0));
    }
    // The root's retained control/bootstrap record is not an allocation request.
    assert_eq!(root.usage_v1(), root_baseline);
}

#[test]
fn qualification_multi_roster_rejects_mixed_missing_alias_and_duplicate_bindings() {
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 7);
    let b = Entry::qualification_entry_v1(&root, 8);
    let alias = Entry::qualification_v1(8, a.model(), a.account().clone());
    for case in 0..5 {
        let right = match case {
            0 => { let mut legacy = KfdRuntimeBackendV1::mock(); legacy.description.backend_device = 8; legacy },
            1 => { let mut missing = child(b.clone()); missing.composed_request_binding = None; missing },
            2 => child(alias.clone()),
            3 => child(a.clone()),
            4 => { let mut wrong = child(b.clone()); wrong.rooted_backing = None; wrong },
            _ => unreachable!(),
        };
        assert!(matches!(KfdMultiDeviceRuntimeBackendV1::from_backends(vec![child(a.clone()), right]),
            Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    }
    let mut backend = fixture(&a, &b);
    backend.children[1].composed_request_binding = None;
    assert!(backend.allocation_admission_profile_v1().is_err());
    assert!(backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8).is_err());
    assert_eq!(backend.next_handle, 1);
}

#[test]
fn qualification_multi_wrong_witness_rejects_before_routing_and_credit_mutation() {
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 7);
    let b = Entry::qualification_entry_v1(&root, 8);
    let foreign_root = Entry::qualification_root_v1();
    let foreign = Entry::qualification_entry_v1(&foreign_root, 7);
    let mut backend = fixture(&a, &b);
    let context = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id();
    for (entry, selected, claimed) in [(&a, 8, 16), (&b, 7, 16), (&a, 7, 17), (&foreign, 7, 16)] {
        let credit = entry.account().reserve_v1(16).unwrap().retain();
        let before = entry.account().usage_v1();
        let witness = entry.qualification_witness_v1(device, &credit, 16);
        assert!(matches!(backend.allocate_with_request_v1(selected, RuntimeMemoryKindV1::HostVisible, claimed, 8, witness),
            ResultV1::Outcome(Err(RuntimeBackendFailureV1::Rejected(_)))));
        assert_eq!(entry.account().usage_v1(), before);
        assert_eq!(backend.next_handle, 1);
        assert_eq!(backend.allocations.capacity(), 0);
        assert!(backend.children.iter().all(|child| child.next_handle == 1 && child.allocations.is_empty()));
        credit.release_after_rejection().unwrap();
    }
}

#[test]
fn qualification_multi_shutdown_retry_preserves_exact_roster_and_policy() {
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 7);
    let b = Entry::qualification_entry_v1(&root, 8);
    let mut backend = fixture(&a, &b);
    let stream = backend.children[0].create_stream_v1(7).unwrap();
    assert!(matches!(backend.shutdown_native_v1(), Err(RuntimeBackendFailureV1::Rejected(_))));
    assert!(!backend.children[0].queue_retired && backend.children[1].queue_retired);
    assert_profile(&backend, &[&a, &b]);
    backend.children[1].sdma_enabled = true;
    backend.children[0].destroy_stream_v1(stream).unwrap();
    for _ in 0..2 {
        backend.shutdown_native_v1().unwrap();
        assert_profile(&backend, &[&a, &b]);
        let next = backend.next_handle;
        assert!(backend.allocate_with_outcome_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8).is_err());
        assert_eq!(backend.next_handle, next);
    }
}

#[test]
fn qualification_multi_root_lifetime_follows_context_and_returned_backend() {
    let (mut context, observe) = {
        let root = Entry::qualification_root_v1();
        let a = Entry::qualification_entry_v1(&root, 7);
        let b = Entry::qualification_entry_v1(&root, 8);
        (RuntimeContextV1::open(fixture(&a, &b)).unwrap(), root.qualification_observer_v1())
    };
    let left = context.allocate(context.devices()[0].id(), RuntimeMemoryKindV1::HostVisible, 16, 8).unwrap();
    let right = context.allocate(context.devices()[1].id(), RuntimeMemoryKindV1::HostVisible, 24, 8).unwrap();
    assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 40);
    context.release_allocation(left).unwrap();
    context.release_allocation(right).unwrap();
    let backend = context.shutdown().unwrap();
    assert!(observe().is_some());
    drop(backend);
    assert!(observe().is_none());
}

#[test]
fn qualification_multi_quarantine_never_disappears_from_required_roster_or_root() {
    let observe = {
        let root = Entry::qualification_root_v1();
        let a = Entry::qualification_entry_v1(&root, 7);
        let b = Entry::qualification_entry_v1(&root, 8);
        let backend = fixture(&a, &b);
        b.account().reserve_v1(19).unwrap().retain().quarantine();
        assert!(backend.allocation_admission_profile_v1().is_err());
        assert_eq!(a.account().usage_v1().quarantined_records, 0);
        assert_eq!(b.account().usage_v1().quarantined_records, 1);
        assert_eq!(backend.device_children.len(), 2);
        root.qualification_observer_v1()
    };
    let retained = observe().expect("quarantine retains root without external owners");
    assert_eq!(retained.quarantined_records, 1);
    assert_eq!(retained.used.get(K::RequestedAllocationBytes), 19);
}
