use super::*;
use fe2o3_runtime_model::{DeviceGenerationV1, PhysicalDeviceIdV1};

pub(crate) fn root(bytes: u64, records: u64) -> Gfx942HostBackingRootV1 {
    Gfx942HostBackingRootV1::new(
        ResourceVectorV1::ZERO
            .with(ResourceKindV1::ControlResidentBytes, 1 << 20)
            .with(ResourceKindV1::ResidentHostAllocationBytes, bytes)
            .with(ResourceKindV1::AllocationRecords, records),
        2,
        16,
        32,
    )
    .unwrap()
}

pub(crate) fn observe_lifetime(
    root: &Gfx942HostBackingRootV1,
) -> impl Fn() -> Option<ResourceCreditUsageV1> + use<> {
    let weak = Arc::downgrade(&root.0);
    move || {
        weak.upgrade()
            .map(|inner| Gfx942HostBackingRootV1(inner).usage_v1())
    }
}

fn identity(uid: u64) -> Identity {
    Identity {
        unique_id: uid,
        pci: PciAddressV1 {
            domain: 0,
            bus: uid as u8,
            device: 0,
            function: 0,
        },
    }
}

fn key(generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(9),
        generation: DeviceGenerationV1(generation),
    }
}

pub(crate) fn budget(bytes: u64, records: usize) -> Gfx942HostVisibleBackingBudgetV1 {
    Gfx942HostVisibleBackingBudgetV1::new(bytes, records).unwrap()
}

pub(crate) fn admission(
    root: &Gfx942HostBackingRootV1,
    device: DeviceKeyV1,
    parent: Gfx942HostVisibleBackingBudgetV1,
    leaf: Gfx942HostVisibleBackingBudgetV1,
) -> Gfx942HostBackingAdmissionV1 {
    root.admit(identity(1), device, parent, leaf).unwrap()
}

fn charge(bytes: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(ResourceKindV1::ResidentHostAllocationBytes, bytes)
        .with(ResourceKindV1::AllocationRecords, 1)
}

#[test]
fn rooted_n1_canonical_device_survives_generations_and_full_registry() {
    let root = root(8192, 4);
    let first = root
        .admit(identity(1), key(1), budget(4096, 2), budget(8192, 2))
        .unwrap();
    let retained = first.account.reserve(charge(4096)).unwrap().retain();
    drop(first);
    let other = root
        .admit(identity(2), key(2), budget(8192, 2), budget(8192, 2))
        .unwrap();
    let next = root
        .admit(identity(1), key(3), budget(4096, 2), budget(8192, 2))
        .unwrap();
    assert_eq!(root.0.devices.lock().unwrap().iter().flatten().count(), 2);
    assert!(matches!(
        next.account.reserve(charge(1)),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    assert_eq!(root.device_usage_v1(1).unwrap().unwrap().used, charge(4096));
    retained.release_after_disposal().unwrap();
    drop(next.account.reserve(charge(4096)).unwrap());
    drop(other);
    assert_eq!(
        root.device_usage_v1(1).unwrap().unwrap().used,
        ResourceVectorV1::ZERO
    );
}

#[test]
fn rooted_n1_conflicting_identity_or_limits_never_registers_an_alias() {
    let root = root(8192, 4);
    drop(
        root.admit(identity(1), key(1), budget(4096, 2), budget(4096, 2))
            .unwrap(),
    );
    let before = root.usage_v1();
    for id in [
        Identity {
            unique_id: 2,
            ..identity(1)
        },
        Identity {
            pci: identity(2).pci,
            ..identity(1)
        },
    ] {
        assert!(matches!(
            root.admit(id, key(2), budget(4096, 2), budget(4096, 2)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
    }
    assert!(matches!(
        root.admit(identity(1), key(2), budget(8192, 2), budget(4096, 2)),
        Err(ResourceCreditErrorV1::Invariant)
    ));
    assert_eq!(root.usage_v1(), before);
    assert_eq!(root.0.devices.lock().unwrap().iter().flatten().count(), 1);
}

#[test]
fn rooted_n1_admission_binds_exact_generation_and_identity() {
    let root = root(8192, 4);
    let admitted = admission(&root, key(1), budget(8192, 2), budget(4096, 1));
    assert!(admitted.matches_identity(identity(1), key(1)));
    assert!(!admitted.matches_identity(identity(1), key(2)));
    assert!(!admitted.matches_identity(identity(2), key(1)));
    let before = root.usage_v1();
    assert!(matches!(
        admitted.into_account(key(2)),
        Err(ResourceCreditErrorV1::Invariant)
    ));
    assert_eq!(root.usage_v1(), before);
}

#[test]
fn rooted_n1_unused_admission_retains_then_releases_typed_root() {
    let root = root(8192, 4);
    let observe = observe_lifetime(&root);
    let admitted = admission(&root, key(1), budget(8192, 2), budget(4096, 1));
    drop(root);
    assert!(observe().is_some());
    drop(admitted);
    assert!(observe().is_none());
}

#[test]
fn rooted_n1_distinct_devices_share_root_without_mixing_session_counts() {
    let root = root(4096, 1);
    let a = root
        .admit(identity(1), key(1), budget(8192, 2), budget(8192, 2))
        .unwrap();
    let b = root
        .admit(identity(2), key(2), budget(8192, 2), budget(8192, 2))
        .unwrap();
    let retained = a.account.reserve(charge(4096)).unwrap().retain();
    assert_eq!(a.account.usage().retained_records, 1);
    assert_eq!(root.usage_v1().retained_records, 2); // Registry metadata is separate.
    assert!(matches!(
        b.account.reserve(charge(1)),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    retained.release_after_disposal().unwrap();
    drop(b.account.reserve(charge(4096)).unwrap());
}

#[test]
fn rooted_n1_quarantine_blocks_reopened_session_under_same_device_parent() {
    let root = root(8192, 4);
    let a = admission(&root, key(1), budget(4096, 1), budget(4096, 1));
    a.account
        .reserve(charge(4096))
        .unwrap()
        .retain()
        .quarantine();
    drop(a);
    let b = admission(&root, key(2), budget(4096, 1), budget(4096, 1));
    assert_eq!(
        root.device_usage_v1(1)
            .unwrap()
            .unwrap()
            .quarantined_records,
        1
    );
    assert!(matches!(
        b.account.reserve(charge(1)),
        Err(ResourceCreditErrorV1::RecordCapacity) | Err(ResourceCreditErrorV1::Capacity)
    ));
}

#[test]
fn rooted_n1_concurrent_registration_reuses_one_canonical_parent() {
    let root = root(4096, 1);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let workers = (1..=2)
        .map(|generation| {
            let root = root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let a = admission(&root, key(generation), budget(4096, 1), budget(4096, 1));
                barrier.wait();
                let reservation = a.account.reserve(charge(4096));
                barrier.wait();
                reservation.is_ok()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        workers
            .into_iter()
            .map(|w| w.join().unwrap() as usize)
            .sum::<usize>(),
        1
    );
    assert_eq!(root.0.devices.lock().unwrap().iter().flatten().count(), 1);
}

#[test]
fn rooted_n1_bootstrap_and_domain_exhaustion_fail_without_partial_registration() {
    let bytes = Gfx942HostBackingRootV1::bootstrap_bytes_v1(1, 3, 4).unwrap();
    let exact = ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes);
    assert!(matches!(
        Gfx942HostBackingRootV1::new(
            exact.with(ResourceKindV1::ControlResidentBytes, bytes - 1),
            1,
            3,
            4
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let exact_root = Gfx942HostBackingRootV1::new(exact, 1, 3, 4).unwrap();
    assert_eq!(exact_root.usage_v1().used, exact);
    assert!(matches!(
        Gfx942HostBackingRootV1::new(ResourceVectorV1::ZERO, 1, 3, 4),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let capacity = ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1 << 20);
    let root = Gfx942HostBackingRootV1::new(capacity, 1, 3, 4).unwrap();
    let a = admission(&root, key(1), budget(4096, 1), budget(4096, 1));
    let before = root.usage_v1();
    assert!(matches!(
        root.admit(identity(1), key(2), budget(4096, 1), budget(4096, 1)),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    assert_eq!(root.usage_v1(), before);
    drop(a);
    drop(admission(&root, key(2), budget(4096, 1), budget(4096, 1)));
    assert!(
        root.usage_v1()
            .used
            .get(ResourceKindV1::ControlResidentBytes)
            > 0
    );
}

#[test]
fn rooted_n1_new_parent_is_reaped_when_its_session_leaf_cannot_fit() {
    let capacity = ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1 << 20);
    let root = Gfx942HostBackingRootV1::new(capacity, 2, 4, 8).unwrap();
    let a = admission(&root, key(1), budget(4096, 1), budget(4096, 1));
    let before = root.usage_v1();
    assert!(matches!(
        root.admit(identity(2), key(2), budget(4096, 1), budget(4096, 1)),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    assert_eq!(root.usage_v1(), before);
    assert!(root.device_usage_v1(2).unwrap().is_none());
    assert_eq!(root.0.devices.lock().unwrap().iter().flatten().count(), 1);
    drop(a);
    let b = root
        .admit(identity(2), key(2), budget(4096, 1), budget(4096, 1))
        .unwrap();
    assert_eq!(root.0.devices.lock().unwrap().iter().flatten().count(), 2);
    drop(b);
    drop(admission(&root, key(3), budget(4096, 1), budget(4096, 1)));
    assert_eq!(root.usage_v1(), before);
}
