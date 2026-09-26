use super::*;

#[test]
fn required_empty_roster_does_not_mean_legacy() {
    let context = RuntimeContextV1::open(crate::KfdRuntimeBackendV1::mock()).unwrap();
    assert!(matches!(
        ContextAllocationAdmissionV1::from_profile(
            context.devices(),
            RuntimeAllocationAdmissionProfileV1::Required(Vec::new())
        ),
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    ));
    assert!(
        ContextAllocationAdmissionV1::from_profile(
            context.devices(),
            RuntimeAllocationAdmissionProfileV1::Legacy
        )
        .unwrap()
        .accounts
        .is_empty()
    );
}

#[test]
fn retained_charge_context_presence_exact_extent_and_device_brand() {
    let device = RuntimeDeviceIdV1::new(1, 1);
    let other = RuntimeDeviceIdV1::new(1, 2);
    let foreign = RuntimeDeviceIdV1::new(2, 1);
    let id = RuntimeAllocationIdV1::new(1, 3);
    let mut admission = ContextAllocationAdmissionV1::default();
    assert!(admission.has_expected_credit(id, device, 64));
    let account = RuntimeResourceCreditAccountV1::new(device, request_charge(64), 2).unwrap();
    admission.accounts.insert(device, account.clone());
    assert!(!admission.has_expected_credit(id, device, 64));
    admission.attach(
        id,
        Some(account.reserve(request_charge(64)).unwrap().retain()),
    );
    assert!(admission.has_expected_credit(id, device, 64));
    assert!(!admission.has_expected_credit(id, device, 8));
    assert!(!admission.has_expected_credit(id, other, 64));
    assert!(!admission.has_expected_credit(id, foreign, 64));
    // Even the same underlying account cannot substitute a device brand.
    admission.accounts.insert(other, account.clone());
    admission.accounts.insert(foreign, account.clone());
    assert!(!admission.has_expected_credit(id, other, 64));
    assert!(!admission.has_expected_credit(id, foreign, 64));
    admission.accounts.remove(&device);
    assert!(!admission.has_expected_credit(id, device, 64));
    admission.release_disposed(id).unwrap();
    assert_eq!(account.usage().used, RuntimeResourceVectorV1::ZERO);
}
