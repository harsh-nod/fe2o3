use super::*;
use ResourceKindV1 as K;
use fe2o3_runtime_model::{DeviceGenerationV1, PhysicalDeviceIdV1};

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

fn root_capacity(bytes: u64, records: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(K::ControlResidentBytes, 1 << 20)
        .with(K::RequestedAllocationBytes, bytes)
        .with(K::ResidentHostAllocationBytes, bytes)
        .with(K::ResidentDeviceAllocationBytes, bytes)
        .with(K::AllocationRecords, records)
}

pub(crate) fn root() -> Gfx942ComposedBackingRootV1 {
    Gfx942ComposedBackingRootV1::new(root_capacity(65536, 32), 2, 24, 40).unwrap()
}

pub(crate) fn device_budget() -> Gfx942ComposedBackingDeviceBudgetV1 {
    Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 16).unwrap()
}

pub(crate) fn budget(records: usize) -> Gfx942ComposedBackingSessionBudgetV1 {
    bounded_budget(8, records)
}

fn bounded_budget(class_records: usize, records: usize) -> Gfx942ComposedBackingSessionBudgetV1 {
    Gfx942ComposedBackingSessionBudgetV1::new(
        Gfx942AllocationRequestBudgetV1::new(32768, class_records).unwrap(),
        Gfx942HostVisibleBackingBudgetV1::new(32768, class_records).unwrap(),
        Gfx942DeviceBackingBudgetV1::new(32768, class_records).unwrap(),
        records,
    )
    .unwrap()
}

fn admission(root: &Gfx942ComposedBackingRootV1) -> Gfx942ComposedBackingAdmissionV1 {
    root.admit(identity(1), key(1), device_budget(), budget(8))
        .unwrap()
}

#[test]
fn composed_request_account_identity_distinguishes_siblings_and_roots() {
    let root = root();
    let first = admission(&root);
    let sibling = admission(&root);
    let other_root = Gfx942ComposedBackingRootV1::new(root_capacity(65536, 32), 2, 24, 40).unwrap();
    let foreign = admission(&other_root);
    let account = first.request_account_v1();
    assert!(account.shares_account_with_v1(&account.clone()));
    assert!(!account.shares_account_with_v1(sibling.request_account_v1()));
    assert!(!account.shares_account_with_v1(foreign.request_account_v1()));
    assert_eq!(account.usage_v1().used, ResourceVectorV1::ZERO);
}

pub(crate) fn observer(
    root: &Gfx942ComposedBackingRootV1,
) -> impl Fn() -> Option<ResourceCreditUsageV1> + use<> {
    let weak = Arc::downgrade(&root.0.0);
    move || {
        weak.upgrade()
            .map(|inner| Gfx942HostBackingRootV1(inner).usage_v1())
    }
}

pub(crate) fn admission_for_device(
    root: &Gfx942ComposedBackingRootV1,
    device: DeviceKeyV1,
    parent: Gfx942ComposedBackingDeviceBudgetV1,
    budget: Gfx942ComposedBackingSessionBudgetV1,
) -> Gfx942ComposedBackingAdmissionV1 {
    root.admit(identity(1), device, parent, budget).unwrap()
}

fn class_account(a: &Gfx942ComposedBackingAdmissionV1, class: usize) -> &ResourceCreditAccountV1 {
    match class {
        0 => &a.request.0.account,
        1 => &a.host.account,
        2 => &a.device.account,
        _ => unreachable!(),
    }
}

fn class_charge(class: usize, bytes: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(
            [
                K::RequestedAllocationBytes,
                K::ResidentHostAllocationBytes,
                K::ResidentDeviceAllocationBytes,
            ][class],
            bytes,
        )
        .with(K::AllocationRecords, 1)
}

#[test]
fn composed_request_budget_bounds_and_exact_bootstrap() {
    for (bytes, records) in [(0, 1), (1, 0), (1, MAX_RESOURCE_CREDIT_RECORDS_V1 + 1)] {
        assert!(Gfx942AllocationRequestBudgetV1::new(bytes, records).is_none());
    }
    assert!(
        Gfx942AllocationRequestBudgetV1::new(u64::MAX, MAX_RESOURCE_CREDIT_RECORDS_V1).is_some()
    );
    for (r, h, d, n) in [
        (0, 1, 1, 1),
        (1, 0, 1, 1),
        (1, 1, 0, 1),
        (1, 1, 1, 0),
        (1, 1, 1, MAX_RESOURCE_CREDIT_RECORDS_V1 + 1),
    ] {
        assert!(Gfx942ComposedBackingDeviceBudgetV1::new(r, h, d, n).is_none());
    }
    let b = budget(3);
    assert_eq!(b.max_combined_records(), 3);
    assert_eq!(b.request_budget().max_requests(), 8);
    assert_eq!(b.request_budget().max_requested_bytes(), 32768);
    for invalid in [0, MAX_RESOURCE_CREDIT_RECORDS_V1 + 1] {
        assert!(
            Gfx942ComposedBackingSessionBudgetV1::new(b.request, b.host, b.device, invalid)
                .is_none()
        );
    }
    let bytes = Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(1, 6, 8).unwrap();
    assert!(Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(1, 5, 8).is_err());
    assert!(Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(usize::MAX, 6, 8).is_err());
    assert!(matches!(
        Gfx942ComposedBackingRootV1::new(
            root_capacity(65536, 8).with(K::ControlResidentBytes, bytes - 1),
            1,
            6,
            8
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let root = Gfx942ComposedBackingRootV1::new(
        root_capacity(65536, 8).with(K::ControlResidentBytes, bytes),
        1,
        6,
        8,
    )
    .unwrap();
    assert_eq!(root.usage_v1().used.get(K::ControlResidentBytes), bytes);
    assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 0);
    assert_eq!(root.usage_v1().retained_records, 1);
    let observe = observer(&root);
    let a = root
        .admit(
            identity(1),
            key(1),
            Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 8).unwrap(),
            budget(8),
        )
        .unwrap();
    assert!(a.coherent());
    let clone = a.request_account_v1().clone();
    drop(root);
    drop(a);
    assert!(observe().is_some());
    drop(clone);
    assert!(observe().is_none());
}

#[test]
fn composed_request_partial_mint_never_publishes_or_leaks_domains() {
    // Exhaust respectively before session, request, host and device child creation.
    for domains in 7..=10 {
        let root =
            Gfx942ComposedBackingRootV1::new(root_capacity(65536, 32), 2, domains, 40).unwrap();
        let first = admission(&root);
        let before = root.usage_v1();
        for _ in 0..3 {
            assert!(matches!(
                root.admit(identity(2), key(2), device_budget(), budget(8)),
                Err(ResourceCreditErrorV1::DomainCapacity)
            ));
            assert_eq!(root.usage_v1(), before);
            assert!(root.device_usage_v1(2).unwrap().is_none());
        }
        drop(first);
        drop(
            root.admit(identity(2), key(2), device_budget(), budget(8))
                .unwrap(),
        );
        drop(admission(&root));
    }
}

#[test]
fn composed_request_canonical_parent_and_all_device_limits_are_immutable() {
    let root = root();
    let a = admission(&root);
    let b = root
        .admit(identity(1), key(2), device_budget(), budget(8))
        .unwrap();
    assert!(a.coherent() && b.coherent());
    assert!(a.request.0.account.shares_root_with(&b.request.0.account));
    assert!(!a.request.0.session.shares_ledger_with(&b.request.0.session));
    assert_eq!(root.0.0.devices.lock().unwrap().iter().flatten().count(), 1);
    for changed in [
        Gfx942ComposedBackingDeviceBudgetV1::new(32768, 65536, 65536, 16).unwrap(),
        Gfx942ComposedBackingDeviceBudgetV1::new(65536, 32768, 65536, 16).unwrap(),
        Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 32768, 16).unwrap(),
        Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 15).unwrap(),
    ] {
        assert!(matches!(
            root.admit(identity(1), key(3), changed, budget(8)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
    }
    for alias in [
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
            root.admit(alias, key(3), device_budget(), budget(8)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
    }
    assert_eq!(root.0.0.devices.lock().unwrap().iter().flatten().count(), 1);
}

#[test]
fn composed_request_cross_session_root_and_generation_substitutions_reject() {
    for foreign in [false, true] {
        let root = root();
        let other = if foreign { self::root() } else { root.clone() };
        let mut a = admission(&root);
        let mut b = admission(&other);
        std::mem::swap(&mut a.request, &mut b.request);
        assert!(!a.coherent() && !b.coherent());
        std::mem::swap(&mut a.request, &mut b.request);
        std::mem::swap(&mut a.host, &mut b.host);
        assert!(!a.coherent() && !b.coherent());
        std::mem::swap(&mut a.host, &mut b.host);
        std::mem::swap(&mut a.device, &mut b.device);
        assert!(!a.coherent() && !b.coherent());
        std::mem::swap(&mut a.device, &mut b.device);
        a.device.generation = key(2);
        assert!(!a.coherent());
        a.device.generation = key(1);
        assert!(a.coherent() && b.coherent());
        let credit = a.request.reserve_v1(8).unwrap().retain();
        let equal = a.request.reserve_v1(8).unwrap().retain();
        assert!(!b.request.matches_retained_charge_v1(&credit, 8));
        assert!(a.request.clone().matches_retained_charge_v1(&credit, 8));
        assert!(a.request.matches_retained_charge_v1(&equal, 8));
        assert!(!a.request.matches_retained_charge_v1(&credit, 7));
        assert!(!a.request.matches_retained_charge_v1(&credit, 9));
        credit.release_after_disposal().unwrap();
        equal.release_after_rejection().unwrap();
    }
}

#[test]
fn composed_request_native_intake_rejects_incoherent_parts() {
    for foreign in [false, true] {
        for class in 0..3 {
            let root = root();
            let other = if foreign { self::root() } else { root.clone() };
            let mut a = admission(&root);
            let mut b = admission(&other);
            match class {
                0 => std::mem::swap(&mut a.request, &mut b.request),
                1 => std::mem::swap(&mut a.host, &mut b.host),
                2 => std::mem::swap(&mut a.device, &mut b.device),
                _ => unreachable!(),
            }
            assert!(matches!(
                a.into_parts(key(1)),
                Err(ResourceCreditErrorV1::Invariant)
            ));
            assert!(matches!(
                b.into_parts(key(1)),
                Err(ResourceCreditErrorV1::Invariant)
            ));
            assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 0);
            assert_eq!(other.usage_v1().used.get(K::AllocationRecords), 0);
        }
    }
}

#[test]
fn composed_request_three_classes_contend_for_one_combined_record_limit() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let root = root();
        let a = root
            .admit(identity(1), key(1), device_budget(), budget(2))
            .unwrap();
        let first = class_account(&a, order[0])
            .reserve(class_charge(order[0], 4))
            .unwrap()
            .retain();
        let second = class_account(&a, order[1])
            .reserve(class_charge(order[1], 4))
            .unwrap()
            .retain();
        let before = root.usage_v1();
        assert!(matches!(
            class_account(&a, order[2]).reserve(class_charge(order[2], 4)),
            Err(ResourceCreditErrorV1::Capacity | ResourceCreditErrorV1::RecordCapacity)
        ));
        assert_eq!(root.usage_v1(), before);
        assert_eq!(
            a.request.session_usage_v1().used.get(K::AllocationRecords),
            2
        );
        first.release_after_disposal().unwrap();
        drop(
            class_account(&a, order[2])
                .reserve(class_charge(order[2], 4))
                .unwrap(),
        );
        second.release_after_disposal().unwrap();
        assert_eq!(a.request.session_usage_v1().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn composed_request_class_device_root_and_generic_record_ceilings_apply() {
    for class in 0..3 {
        let root = root();
        let a = admission(&root);
        let first = class_account(&a, class)
            .reserve(class_charge(class, 32768))
            .unwrap()
            .retain();
        assert!(matches!(
            class_account(&a, class).reserve(class_charge(class, 1)),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        first.release_after_disposal().unwrap();
    }
    for parent_limit in [false, true] {
        let root = Gfx942ComposedBackingRootV1::new(
            root_capacity(if parent_limit { 65536 } else { 8 }, 32),
            2,
            24,
            40,
        )
        .unwrap();
        let parent = if parent_limit {
            Gfx942ComposedBackingDeviceBudgetV1::new(8, 65536, 65536, 16).unwrap()
        } else {
            device_budget()
        };
        let a = root.admit(identity(1), key(1), parent, budget(8)).unwrap();
        let b = root
            .admit(
                identity(if parent_limit { 1 } else { 2 }),
                key(2),
                parent,
                budget(8),
            )
            .unwrap();
        let first = a.request.reserve_v1(8).unwrap().retain();
        assert!(matches!(
            b.request.reserve_v1(1),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        first.release_after_disposal().unwrap();
        drop(b.request.reserve_v1(8).unwrap());
    }
    let root = Gfx942ComposedBackingRootV1::new(root_capacity(65536, 32), 1, 6, 2).unwrap();
    let a = root
        .admit(
            identity(1),
            key(1),
            Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 2).unwrap(),
            bounded_budget(2, 2),
        )
        .unwrap();
    let first = a.request.reserve_v1(8).unwrap().retain();
    assert!(matches!(
        a.request.reserve_v1(1),
        Err(ResourceCreditErrorV1::RecordCapacity)
    ));
    assert_eq!(root.usage_v1().used.get(K::AllocationRecords), 1);
    assert_eq!(root.usage_v1().retained_records, 2);
    first.release_after_disposal().unwrap();
}

#[test]
fn composed_request_cold_reservation_keeps_registry_then_cancels_cleanly() {
    let root = root();
    let observe = observer(&root);
    let a = admission(&root);
    let account = a.request.clone();
    let reservation = account.reserve_v1(8).unwrap();
    drop(account.clone());
    assert!(root.0.0.quarantine_anchor.lock().unwrap().is_none());
    drop(root);
    drop(a);
    drop(account);
    assert_eq!(observe().unwrap().reserved_records, 1);
    drop(reservation);
    assert!(observe().is_none());
}

#[test]
fn composed_request_cold_retained_credit_releases_or_preserves_registry() {
    for outcome in 0..4 {
        let root = root();
        let observe = observer(&root);
        let a = admission(&root);
        let credit = a.request.reserve_v1(8).unwrap().retain();
        let bootstrap = root.usage_v1().used.get(K::ControlResidentBytes);
        drop(root);
        drop(a);
        assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 8);
        match outcome {
            0 => credit.release_after_disposal().unwrap(),
            1 => credit.release_after_rejection().unwrap(),
            2 => credit.quarantine(),
            3 => drop(credit),
            _ => unreachable!(),
        }
        if outcome < 2 {
            assert!(observe().is_none());
        } else {
            let usage = observe().unwrap();
            assert_eq!(usage.quarantined_records, 1);
            assert_eq!(usage.used.get(K::RequestedAllocationBytes), 8);
            assert_eq!(usage.used.get(K::ControlResidentBytes), bootstrap);
            assert_eq!(usage.used.get(K::ResidentHostAllocationBytes), 0);
            assert_eq!(usage.used.get(K::ResidentDeviceAllocationBytes), 0);
        }
    }
}

#[test]
fn composed_request_unwind_cancels_reserved_but_quarantines_retained() {
    for retained in [false, true] {
        let root = root();
        let observe = observer(&root);
        let a = admission(&root);
        let reservation = a.request.reserve_v1(8).unwrap();
        drop(root);
        drop(a);
        assert!(
            std::panic::catch_unwind(move || {
                if retained {
                    let _credit = reservation.retain();
                    panic!("retained fixture unwind");
                }
                let _reserved = reservation;
                panic!("reserved fixture unwind");
            })
            .is_err()
        );
        if retained {
            assert_eq!(observe().unwrap().quarantined_records, 1);
        } else {
            assert!(observe().is_none());
        }
    }
}

#[test]
fn composed_request_batch_is_atomic_and_pending_members_keep_registry() {
    let root = root();
    let observe = observer(&root);
    let a = admission(&root);
    let before = root.usage_v1();
    assert!(matches!(
        a.request.reserve_batch_v1(&[]),
        Err(ResourceCreditErrorV1::InvalidRecordCapacity)
    ));
    assert!(matches!(
        a.request.reserve_batch_v1(&[32768, 1]),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    assert_eq!(root.usage_v1(), before);
    let mut batch = a.request.reserve_batch_v1(&[3, 5, 0]).unwrap();
    assert_eq!(batch.size_hint(), (3, Some(3)));
    let credit = batch.next().unwrap().retain();
    drop(root);
    drop(a);
    assert_eq!(observe().unwrap().reserved_records, 2);
    credit.release_after_disposal().unwrap();
    assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 5);
    assert_eq!(batch.len(), 2);
    let zero = batch.nth(1).unwrap().retain();
    assert_eq!(batch.len(), 0);
    assert!(batch.next().is_none() && batch.next().is_none());
    drop(batch);
    assert_eq!(observe().unwrap().used.get(K::AllocationRecords), 1);
    zero.release_after_disposal().unwrap();
    assert!(observe().is_none());
}

#[test]
fn composed_request_partial_batch_drop_and_native_bundle_drop_order() {
    for native_first in [false, true] {
        let root = root();
        let observe = observer(&root);
        let a = admission(&root);
        let mut batch = a.request.reserve_batch_v1(&[3, 5]).unwrap();
        let credit = batch.next().unwrap().retain();
        let Gfx942ComposedBackingAdmissionV1 {
            request,
            host,
            device,
            ..
        } = a;
        drop(request);
        drop(root);
        if native_first {
            drop((host, device));
            drop(batch);
            assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 3);
            credit.release_after_disposal().unwrap();
        } else {
            drop(batch);
            credit.release_after_disposal().unwrap();
            assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 0);
            drop((host, device));
        }
        assert!(observe().is_none());
    }
}

#[test]
fn composed_request_quarantine_retains_ancestor_pressure_without_global_poison() {
    for spare in [false, true] {
        let root = Gfx942ComposedBackingRootV1::new(
            root_capacity(65536, 32).with(K::RequestedAllocationBytes, if spare { 16 } else { 8 }),
            2,
            24,
            40,
        )
        .unwrap();
        let a = admission(&root);
        let b = admission(&root);
        a.request.reserve_v1(8).unwrap().retain().quarantine();
        assert_eq!(a.request.session_usage_v1().quarantined_records, 1);
        assert_eq!(b.request.session_usage_v1().quarantined_records, 0);
        let before = root.usage_v1();
        // Ledger quarantine retains capacity; native session sealing is a separate contract.
        assert!(!before.poisoned);
        drop(a.host.account.reserve(class_charge(1, 4)).unwrap());
        if spare {
            drop(b.request.reserve_v1(4).unwrap());
        } else {
            assert!(matches!(
                b.request.reserve_v1(4),
                Err(ResourceCreditErrorV1::Capacity)
            ));
        }
        assert_eq!(root.usage_v1(), before);
    }
}

#[test]
fn composed_request_native_sibling_quarantine_preserves_typed_registry() {
    for class in 1..=2 {
        let root = root();
        let observe = observer(&root);
        let a = admission(&root);
        class_account(&a, class)
            .reserve(class_charge(class, 4))
            .unwrap()
            .retain()
            .quarantine();
        assert_eq!(a.request.usage_v1().used, ResourceVectorV1::ZERO);
        assert_eq!(a.request.session_usage_v1().quarantined_records, 1);
        drop(root);
        drop(a);
        let used = observe().unwrap().used;
        assert_eq!(used.get(K::RequestedAllocationBytes), 0);
        assert_eq!(used.get(K::AllocationRecords), 1);
        assert_eq!(
            used.get(
                [
                    K::ResidentHostAllocationBytes,
                    K::ResidentDeviceAllocationBytes
                ][class - 1]
            ),
            4
        );
    }
}
