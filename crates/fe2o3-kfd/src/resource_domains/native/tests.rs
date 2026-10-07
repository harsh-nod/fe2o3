use super::*;
use fe2o3_runtime_model::{DeviceGenerationV1, PhysicalDeviceIdV1};

pub(crate) fn root(host: u64, device: u64, records: u64) -> Gfx942NativeBackingRootV1 {
    Gfx942NativeBackingRootV1::new(
        ResourceVectorV1::ZERO
            .with(ResourceKindV1::ControlResidentBytes, 1 << 20)
            .with(ResourceKindV1::ResidentHostAllocationBytes, host)
            .with(ResourceKindV1::ResidentDeviceAllocationBytes, device)
            .with(ResourceKindV1::AllocationRecords, records),
        2,
        24,
        32,
    )
    .unwrap()
}

pub(crate) fn budget(
    host_records: usize,
    device_records: usize,
    combined: usize,
) -> Gfx942NativeBackingSessionBudgetV1 {
    Gfx942NativeBackingSessionBudgetV1::new(
        Gfx942HostVisibleBackingBudgetV1::new(32768, host_records).unwrap(),
        Gfx942DeviceBackingBudgetV1::new(32768, device_records).unwrap(),
        combined,
    )
    .unwrap()
}

pub(crate) fn device_budget(records: usize) -> Gfx942NativeBackingDeviceBudgetV1 {
    Gfx942NativeBackingDeviceBudgetV1::new(65536, 65536, records).unwrap()
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

pub(crate) fn admission(
    root: &Gfx942NativeBackingRootV1,
    device: DeviceKeyV1,
    parent: Gfx942NativeBackingDeviceBudgetV1,
    session: Gfx942NativeBackingSessionBudgetV1,
) -> Gfx942NativeBackingAdmissionV1 {
    root.admit(identity(1), device, parent, session).unwrap()
}

pub(crate) fn observe_lifetime(
    root: &Gfx942NativeBackingRootV1,
) -> impl Fn() -> Option<ResourceCreditUsageV1> + use<> {
    let weak = Arc::downgrade(&root.0.0);
    move || {
        weak.upgrade()
            .map(|inner| Gfx942HostBackingRootV1(inner).usage_v1())
    }
}

#[test]
fn compound_backing_second_class_failure_reaps_unpublished_session_and_device() {
    for domains in 6..=8 {
        let root = Gfx942NativeBackingRootV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1 << 20),
            2,
            domains,
            32,
        )
        .unwrap();
        let first = admission(&root, key(1), device_budget(8), budget(4, 4, 8));
        let before = root.usage_v1();
        assert!(matches!(
            root.admit(identity(2), key(2), device_budget(8), budget(4, 4, 8)),
            Err(ResourceCreditErrorV1::DomainCapacity)
        ));
        assert_eq!(root.usage_v1(), before);
        assert!(root.device_usage_v1(2).unwrap().is_none());
        drop(first);
        drop(
            root.admit(identity(2), key(2), device_budget(8), budget(4, 4, 8))
                .unwrap(),
        );
        drop(admission(&root, key(3), device_budget(8), budget(4, 4, 8)));
    }
}

#[test]
fn compound_backing_exact_bootstrap_and_unused_admission_lifetime() {
    let bytes = Gfx942NativeBackingRootV1::bootstrap_bytes_v1(1, 5, 8).unwrap();
    assert!(Gfx942NativeBackingRootV1::bootstrap_bytes_v1(1, 4, 8).is_err());
    assert!(matches!(
        Gfx942NativeBackingRootV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes - 1),
            1,
            5,
            8
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let root = Gfx942NativeBackingRootV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        1,
        5,
        8,
    )
    .unwrap();
    assert_eq!(
        root.usage_v1()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        bytes
    );
    let observe = observe_lifetime(&root);
    let admission = admission(&root, key(1), device_budget(8), budget(4, 4, 8));
    assert!(admission.coherent());
    drop(root);
    assert!(observe().is_some());
    drop(admission);
    assert!(observe().is_none());
}

#[test]
fn compound_backing_cross_session_or_foreign_root_class_substitution_rejects() {
    for foreign in [false, true] {
        let root = root(65536, 65536, 8);
        let other = if foreign {
            self::root(65536, 65536, 8)
        } else {
            root.clone()
        };
        let mut a = admission(&root, key(1), device_budget(8), budget(4, 4, 8));
        let mut b = admission(&other, key(1), device_budget(8), budget(4, 4, 8));
        std::mem::swap(&mut a.device, &mut b.device);
        assert!(!a.coherent());
        assert!(matches!(
            a.into_parts(),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        assert!(!b.coherent());
        drop(b);
        assert_eq!(
            root.usage_v1().used.get(ResourceKindV1::AllocationRecords),
            0
        );
        drop(admission(&root, key(2), device_budget(8), budget(4, 4, 8)));
    }
}

#[test]
fn compound_backing_physical_parent_is_canonical_and_budgets_immutable() {
    let root = root(65536, 65536, 8);
    let a = admission(&root, key(1), device_budget(3), budget(2, 2, 3));
    let b = admission(&root, key(2), device_budget(3), budget(2, 2, 3));
    assert!(a.host.account.shares_root_with(&b.device.account));
    assert!(
        !a.host
            .session
            .as_ref()
            .unwrap()
            .shares_ledger_with(b.host.session.as_ref().unwrap())
    );
    assert_eq!(root.0.0.devices.lock().unwrap().iter().flatten().count(), 1);
    for changed in [
        Gfx942NativeBackingDeviceBudgetV1::new(32768, 65536, 3).unwrap(),
        Gfx942NativeBackingDeviceBudgetV1::new(65536, 32768, 3).unwrap(),
        device_budget(4),
    ] {
        assert!(matches!(
            root.admit(identity(1), key(3), changed, budget(2, 2, 3)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
    }
}
