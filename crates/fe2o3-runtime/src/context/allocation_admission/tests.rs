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

#[test]
fn retained_dispatch_context_lookup_matrix_preserves_ordinary_and_accounted_paths() {
    use fe2o3_resource_accounting::ResourceCreditAccountV1;
    for domain in [false, true] {
        for bytes in [0, 1, u64::MAX] {
            let device = RuntimeDeviceIdV1::new(7, 3);
            let foreign_generation = RuntimeDeviceIdV1::new(8, 3);
            let other_device = RuntimeDeviceIdV1::new(7, 4);
            let id = RuntimeAllocationIdV1::new(7, 5);
            let missing_id = RuntimeAllocationIdV1::new(8, 5);
            let expected = request_charge(bytes);
            let root = domain.then(|| {
                ResourceCreditAccountV1::new_root(
                    expected.with(RuntimeResourceKindV1::ControlResidentBytes, 1 << 20),
                    4,
                    4,
                )
                .unwrap()
            });
            let account = match &root {
                Some(root) => {
                    RuntimeResourceCreditAccountV1::in_domain(device, root, expected, 1).unwrap()
                }
                None => RuntimeResourceCreditAccountV1::new(device, expected, 1).unwrap(),
            };
            let mut admission = ContextAllocationAdmissionV1::default();
            // Neither lookup present is ordinary-mode allowance, not a credit.
            assert!(admission.has_expected_credit(id, device, bytes));
            admission.attach(id, Some(account.reserve(expected).unwrap().retain()));
            assert!(!admission.has_expected_credit(id, device, bytes));
            admission.accounts.insert(device, account.clone());
            assert!(!admission.has_expected_credit(missing_id, device, bytes));
            for impostor in [foreign_generation, other_device] {
                admission.accounts.insert(impostor, account.clone());
                assert!(!admission.has_expected_credit(id, impostor, bytes));
            }
            let before = account.usage();
            let root_before = root.as_ref().map(ResourceCreditAccountV1::usage);
            for _ in 0..3 {
                assert!(admission.has_expected_credit(id, device, bytes));
                assert!(!admission.has_expected_credit(id, device, bytes.wrapping_add(1)));
            }
            assert_eq!(account.usage(), before);
            assert_eq!(
                root.as_ref().map(ResourceCreditAccountV1::usage),
                root_before
            );
            admission.release_disposed(id).unwrap();
            assert!(!admission.has_expected_credit(id, device, bytes));
            assert_eq!(account.usage().used, RuntimeResourceVectorV1::ZERO);
        }
    }
}
