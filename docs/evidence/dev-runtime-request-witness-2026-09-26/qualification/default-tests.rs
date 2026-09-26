use super::*;

#[test]
fn qualification_default_request_hook_never_calls_legacy_allocation() {
    use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;
    let root = Entry::qualification_root_v1();
    let entry = Entry::qualification_entry_v1(&root, 10);
    let credit = entry.account().reserve_v1(17).unwrap().retain();
    let witness =
        RuntimeAllocationRequestWitnessV1::new(RuntimeDeviceIdV1::new(1, 1), &entry, &credit, 17);
    let mut backend = AllocationOnlyBackend::default();
    assert!(matches!(
        backend.allocate_with_request_v1(10, RuntimeMemoryKindV1::HostVisible, 17, 1, witness),
        RuntimeRequestAllocationResultV1::Unsupported
    ));
    assert_eq!(backend.attempts, 0);
    assert_eq!(backend.inner.allocation_calls, 0);
    assert_eq!(entry.account().usage_v1().retained_records, 1);
    credit.release_after_rejection().unwrap();
}
