use super::*;

#[test]
fn xgmi_request_legacy_order_and_invalid_keys_are_explicit() {
    let policy = RequestPolicyV1::from_bindings([19, 7], [None, None]).unwrap();
    assert!(matches!(
        policy.profile([19, 7]).unwrap(),
        RuntimeAllocationAdmissionProfileV1::Legacy
    ));
    assert_eq!(
        policy.allocation_endpoint([19, 7], 19, 16, None).unwrap(),
        0
    );
    assert_eq!(policy.allocation_endpoint([19, 7], 7, 16, None).unwrap(), 1);
    assert!(matches!(policy.allocation_endpoint([19, 7], 23, 16, None),
        Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice));
    for keys in [[0, 7], [19, 0], [7, 7]] {
        assert!(RequestPolicyV1::from_bindings(keys, [None, None]).is_err());
        assert!(policy.profile(keys).is_err());
    }
}

#[test]
fn xgmi_request_public_constructor_rejects_invalid_ids_before_opening() {
    let capacity = crate::RuntimeResourceVectorV1::ZERO.with(
        crate::RuntimeResourceKindV1::ControlResidentBytes,
        Gfx942ComposedBackingRootV1::bootstrap_bytes_v1(2, 16, 32).unwrap(),
    );
    let root = Gfx942ComposedBackingRootV1::new(capacity, 2, 16, 32).unwrap();
    let device = Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 32).unwrap();
    let session = Gfx942ComposedBackingSessionBudgetV1::new(
        fe2o3_kfd::Gfx942AllocationRequestBudgetV1::new(32768, 8).unwrap(),
        Gfx942HostVisibleBackingBudgetV1::new(32768, 8).unwrap(),
        Gfx942DeviceBackingBudgetV1::new(32768, 8).unwrap(),
        24,
    )
    .unwrap();
    for keys in [[0, 7], [19, 0], [7, 7]] {
        assert!(
            matches!(KfdNativeXgmiRuntimeBackendV1::open_default_with_composed_backing_root_v1(
            keys[0], keys[1], &root, [device; 2], [session; 2],
        ), Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
    }
}

#[test]
fn xgmi_request_wiring_keeps_binding_checks_before_vm_and_witness_before_allocation() {
    let source = include_str!("../xgmi_request.rs");
    let binding = source
        .split("pub(super) fn from_admissions(")
        .nth(1)
        .unwrap()
        .split("fn from_bindings(")
        .next()
        .unwrap();
    assert!(
        binding.find("admission.matches_device_v1(device)").unwrap()
            < binding
                .find("RuntimeAllocationDeviceAdmissionV1::for_checked_device_v1(")
                .unwrap()
    );
    let allocation = source
        .split("pub(super) fn allocate_xgmi_request_v1(")
        .nth(1)
        .unwrap();
    assert!(
        allocation.find("self.require_live()?").unwrap()
            < allocation
                .find("self.request_policy.allocation_endpoint(")
                .unwrap()
    );
    assert!(
        allocation
            .find("self.request_policy.allocation_endpoint(")
            .unwrap()
            < allocation.find("xgmi_budget::allocate_record(").unwrap()
    );
    assert!(!allocation.contains("SettledNoOwner"));
    let backend = include_str!("../../kfd_backend.rs");
    let constructor = backend
        .split("fn from_checked_pair_with_admissions_v1(")
        .nth(1)
        .unwrap()
        .split("fn rejected(")
        .next()
        .unwrap();
    assert!(constructor.contains("xgmi_request::RequestPolicyV1::from_admissions"));
    assert!(
        constructor
            .contains(".acquire_shared_gtt_memory_session_with_composed_backing_v1(admission)")
    );
    let fields = backend
        .split("pub struct KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    assert!(fields.find("request_policy:").unwrap() > fields.find("directed_roots:").unwrap());
    let profile = backend
        .split("impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("fn allocate_with_request_v1(")
        .next()
        .unwrap();
    assert!(profile.contains("self.require_healthy_xgmi_v1()?"));
    assert!(!profile.contains("require_live"));
    assert!(profile.contains("self.request_policy"));
}
