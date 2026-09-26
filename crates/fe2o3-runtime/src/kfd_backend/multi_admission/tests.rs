use super::*;

#[test]
fn multi_admission_rejects_invalid_device_counts_and_keys() {
    for count in [0, 1, crate::MAX_RUNTIME_DEVICES_V1 + 1] {
        assert!(matches!(reserve_device_index_v1(count), Err(error)
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    }
    for key in [0, 7] {
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = key;
        assert!(matches!(KfdMultiDeviceRuntimeBackendV1::from_backends(vec![
            KfdRuntimeBackendV1::mock(), right,
        ]), Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    }
}

#[test]
fn multi_admission_legacy_roster_is_complete_and_stable_after_shutdown() {
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend =
        KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right])
            .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            backend.allocation_admission_profile_v1().unwrap(),
            crate::RuntimeAllocationAdmissionProfileV1::Legacy
        ));
        assert_eq!(backend.device_children.len(), 2);
        backend.shutdown_native_v1().unwrap();
    }
    backend.device_children.remove(&8);
    assert!(backend.allocation_admission_profile_v1().is_err());
}

#[test]
fn multi_admission_composed_constructor_rejects_invalid_roster_before_opening_kfd() {
    let capacity = crate::RuntimeResourceVectorV1::ZERO.with(
        crate::RuntimeResourceKindV1::ControlResidentBytes,
        Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(2, 16, 32).unwrap(),
    );
    let root = Gfx942ComposedBackingRootV1::new(capacity, 2, 16, 32).unwrap();
    let device_budget = Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 32).unwrap();
    let session_budget = Gfx942ComposedBackingSessionBudgetV1::new(
        fe2o3_kfd::Gfx942AllocationRequestBudgetV1::new(32768, 8).unwrap(),
        Gfx942HostVisibleBackingBudgetV1::new(32768, 8).unwrap(),
        Gfx942DeviceBackingBudgetV1::new(32768, 8).unwrap(),
        24,
    )
    .unwrap();
    for keys in [[7, 0], [7, 7]] {
        let result = KfdMultiDeviceRuntimeBackendV1::open_composed_v1(
            keys.into_iter()
                .map(|key| (key, (), device_budget, session_budget))
                .collect(),
            &root,
            |()| panic!("invalid roster reached device opening"),
        );
        assert!(matches!(result, Err(error)
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    }
    // No device or authority is needed to prove cardinality preflight is first.
    let result = KfdMultiDeviceRuntimeBackendV1::open_default_with_composed_backing_root_v1(
        Vec::new(),
        &root,
    );
    assert!(matches!(result, Err(error)
        if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    let result = KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_and_composed_backing_root_v1(
        Vec::new(), &root,
    );
    assert!(matches!(result, Err(error)
        if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
}
