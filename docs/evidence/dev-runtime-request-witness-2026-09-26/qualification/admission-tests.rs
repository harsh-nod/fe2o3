use super::*;
use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;

#[test]
fn qualification_complete_roster_rejects_aliases_and_preserves_device_order() {
    let context = RuntimeContextV1::open(crate::KfdRuntimeBackendV1::mock()).unwrap();
    let first = context.devices()[0].clone();
    let mut second = first.clone();
    second.id = RuntimeDeviceIdV1::new(first.id.context_generation, 2);
    second.backend_device = 8;
    let devices = [first, second];
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 7);
    let b = Entry::qualification_entry_v1(&root, 8);
    let alias = Entry::qualification_v1(8, a.model(), a.account().clone());
    let duplicate_key = Entry::qualification_entry_v1(&root, 7);
    let unknown_key = Entry::qualification_v1(9, b.model(), b.account().clone());
    for roster in [
        vec![],
        vec![a.clone()],
        vec![a.clone(), a.clone()],
        vec![a.clone(), alias],
        vec![a.clone(), duplicate_key],
        vec![a.clone(), unknown_key],
        vec![a.clone(), b.clone(), a.clone()],
    ] {
        assert!(matches!(
            ContextAllocationAdmissionV1::from_profile(
                &devices,
                RuntimeAllocationAdmissionProfileV1::Required(roster)
            ),
            Err(RuntimeValidationErrorV1::InvalidBackendDescription)
        ));
    }
    let admission = ContextAllocationAdmissionV1::from_profile(
        &devices,
        RuntimeAllocationAdmissionProfileV1::Required(vec![b.clone(), a.clone()]),
    )
    .unwrap();
    for (device, expected) in devices.iter().zip([&a, &b]) {
        let credit = admission.reserve(device.id, 17).unwrap().unwrap();
        let witness = admission
            .witness(device.id, Some(&credit), 17)
            .unwrap()
            .unwrap();
        assert!(witness.matches_v1(expected, 17));
        assert_eq!(admission.retained_records(), 1);
        credit.release_after_rejection().unwrap();
    }
    assert_eq!(admission.retained_records(), 0);
}

#[test]
fn qualification_witness_rejects_wrong_context_brand_leaf_and_extent() {
    let context = RuntimeContextV1::open(crate::KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id;
    let root = Entry::qualification_root_v1();
    let entry = Entry::qualification_entry_v1(&root, 7);
    let mut admission = ContextAllocationAdmissionV1::from_profile(
        context.devices(),
        RuntimeAllocationAdmissionProfileV1::Required(vec![entry.clone()]),
    )
    .unwrap();
    let credit = admission.reserve(device, 17).unwrap().unwrap();
    assert!(admission.witness(device, None, 17).is_err());
    assert!(admission.witness(device, Some(&credit), 18).is_err());
    let sibling = Entry::qualification_entry_v1(&root, 7);
    let foreign_root = Entry::qualification_root_v1();
    let foreign = Entry::qualification_entry_v1(&foreign_root, 7);
    let different_key = Entry::qualification_v1(8, entry.model(), entry.account().clone());
    for expected in [&sibling, &foreign, &different_key] {
        assert!(
            !admission
                .witness(device, Some(&credit), 17)
                .unwrap()
                .unwrap()
                .matches_v1(expected, 17)
        );
    }
    for other in [
        RuntimeDeviceIdV1::new(device.context_generation + 1, device.local),
        RuntimeDeviceIdV1::new(device.context_generation, device.local + 1),
    ] {
        admission
            .accounts
            .insert(other, admission.accounts[&device].clone());
        assert!(admission.witness(other, Some(&credit), 17).is_err());
    }
    credit.release_after_disposal().unwrap();
}

#[test]
fn qualification_cold_context_and_returned_backend_preserve_root_custody() {
    let (mut context, observe) = {
        let root = Entry::qualification_root_v1();
        let entry = Entry::qualification_entry_v1(&root, 7);
        let observe = root.qualification_observer_v1();
        let backend = crate::KfdRuntimeBackendV1::qualification_composed_v1(entry);
        (RuntimeContextV1::open(backend).unwrap(), observe)
    };
    let device = context.devices()[0].id();
    assert!(observe().is_some());
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 4)
        .unwrap();
    assert_eq!(
        observe()
            .unwrap()
            .used
            .get(RuntimeResourceKindV1::RequestedAllocationBytes),
        32
    );
    context.release_allocation(allocation).unwrap();
    let backend = context.shutdown().unwrap();
    assert!(observe().is_some());
    drop(backend);
    assert!(observe().is_none());
}

#[test]
fn qualification_session_quarantine_blocks_profile_and_new_witnesses() {
    let context = RuntimeContextV1::open(crate::KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id;
    let root = Entry::qualification_root_v1();
    let entry = Entry::qualification_entry_v1(&root, 7);
    let admission = ContextAllocationAdmissionV1::from_profile(
        context.devices(),
        RuntimeAllocationAdmissionProfileV1::Required(vec![entry.clone()]),
    )
    .unwrap();
    let credit = admission.reserve(device, 17).unwrap().unwrap();
    entry.account().reserve_v1(1).unwrap().retain().quarantine();
    assert!(admission.witness(device, Some(&credit), 17).is_err());
    assert!(matches!(
        admission.reserve(device, 1),
        Err(RuntimeResourceCreditErrorV1::Invariant)
    ));
    assert!(
        ContextAllocationAdmissionV1::from_profile(
            context.devices(),
            RuntimeAllocationAdmissionProfileV1::Required(vec![entry.clone()])
        )
        .is_err()
    );
    credit.release_after_rejection().unwrap();
    assert_eq!(entry.account().usage_v1().quarantined_records, 1);
}

#[test]
fn qualification_kfd_context_retains_refunds_and_preserves_policy_after_shutdown() {
    let root = Entry::qualification_root_v1();
    let entry = Entry::qualification_entry_v1(&root, 7);
    let backend = crate::KfdRuntimeBackendV1::qualification_composed_v1(entry.clone());
    let mut context = RuntimeContextV1::open(backend).unwrap();
    let device = context.devices()[0].id;
    assert!(matches!(
        context.configure_allocation_admission_v1(device, 64, 2),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        let id = context.allocate(device, kind, 32, 4).unwrap();
        assert_eq!(entry.account().usage_v1().retained_records, 1);
        assert_eq!(entry.account().usage_v1().used, request_charge(32));
        context.release_allocation(id).unwrap();
        assert_eq!(
            entry.account().usage_v1().used,
            RuntimeResourceVectorV1::ZERO
        );
    }
    let mut backend = context.shutdown().unwrap();
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 4),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        backend.allocate_with_outcome_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 4),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    for policy in 0..3 {
        let mut backend = crate::KfdRuntimeBackendV1::qualification_composed_v1(entry.clone());
        backend.qualification_policy_mismatch_v1(policy);
        core::mem::forget(backend);
    }
}
