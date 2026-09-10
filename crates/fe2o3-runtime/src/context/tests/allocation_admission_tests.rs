use super::*;
use crate::{RuntimeResourceCreditUsageV1, RuntimeResourceKindV1 as K, RuntimeResourceVectorV1};

fn configured(bytes: u64, records: usize) -> (RuntimeContextV1<MockBackend>, RuntimeDeviceIdV1) {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_v1(device, bytes, records)
        .unwrap();
    (context, device)
}

fn usage(
    context: &RuntimeContextV1<MockBackend>,
    device: RuntimeDeviceIdV1,
) -> RuntimeResourceCreditUsageV1 {
    context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap()
}

fn allocate(
    context: &mut RuntimeContextV1<MockBackend>,
    device: RuntimeDeviceIdV1,
    bytes: u64,
) -> Result<RuntimeAllocationIdV1, RuntimeErrorV1<MockError>> {
    context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, bytes, 8)
}

#[test]
fn allocation_credit_vector_rejection_is_atomic_and_precedes_backend_entry() {
    let (mut context, device) = configured(64, 2);
    let first = allocate(&mut context, device, 32).unwrap();
    let before = usage(&context, device);
    assert!(matches!(
        allocate(&mut context, device, 33),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(context.backend().allocation_calls, 1);
    assert_eq!(usage(&context, device), before);
    let second = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let full = usage(&context, device);
    assert_eq!(full.used.get(K::RequestedAllocationBytes), 64);
    assert_eq!(full.used.get(K::AllocationRecords), 2);
    assert_eq!(full.used.get(K::ResidentDeviceAllocationBytes), 0);
    assert_eq!(full.used.get(K::ResidentHostAllocationBytes), 0);
    assert!(matches!(
        allocate(&mut context, device, 1),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(usage(&context, device), full);
    context.release_allocation(first).unwrap();
    assert_eq!(
        usage(&context, device)
            .used
            .get(K::RequestedAllocationBytes),
        32
    );
    let third = allocate(&mut context, device, 32).unwrap();
    assert_ne!(first, third);
    let before = usage(&context, device);
    assert!(context.release_allocation(first).is_err());
    assert_eq!(usage(&context, device), before);
    context.release_allocation(second).unwrap();
    context.release_allocation(third).unwrap();
    assert_eq!(usage(&context, device).used, RuntimeResourceVectorV1::ZERO);
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_record_limit_is_independent_of_byte_limit() {
    let (mut context, device) = configured(1024, 1);
    allocate(&mut context, device, 1).unwrap();
    let before = usage(&context, device);
    assert!(allocate(&mut context, device, 1).is_err());
    assert_eq!(context.backend().allocation_calls, 1);
    assert_eq!(usage(&context, device), before);
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_credit_devices_and_contexts_are_isolated() {
    let (mut context, first) = configured(8, 1);
    let second = context.devices()[1].id();
    context
        .configure_allocation_admission_v1(second, 16, 2)
        .unwrap();
    allocate(&mut context, first, 8).unwrap();
    allocate(&mut context, second, 16).unwrap();
    assert_eq!(
        usage(&context, first).used.get(K::RequestedAllocationBytes),
        8
    );
    assert_eq!(
        usage(&context, second)
            .used
            .get(K::RequestedAllocationBytes),
        16
    );
    let (other, foreign) = configured(8, 1);
    assert_eq!(
        context.allocation_admission_usage_v1(foreign),
        Err(RuntimeValidationErrorV1::UnknownDevice)
    );
    assert!(
        context
            .configure_allocation_admission_v1(foreign, 8, 1)
            .is_err()
    );
    assert_eq!(usage(&other, foreign).used, RuntimeResourceVectorV1::ZERO);
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_credit_configuration_cannot_forget_live_or_quarantined_custody() {
    let (mut context, device) = configured(16, 2);
    let allocation = allocate(&mut context, device, 8).unwrap();
    assert!(matches!(
        context.configure_allocation_admission_v1(device, 32, 4),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    context.release_allocation(allocation).unwrap();
    context
        .configure_allocation_admission_v1(device, 32, 4)
        .unwrap();
    let before = usage(&context, device);
    assert!(
        context
            .configure_allocation_admission_v1(device, 32, 0)
            .is_err()
    );
    assert!(
        context
            .configure_allocation_admission_v1(device, 32, usize::MAX)
            .is_err()
    );
    assert_eq!(usage(&context, device), before);
    context.backend.allocation_failure = MockMemoryFailure::Quiescent;
    assert!(matches!(
        allocate(&mut context, device, 8),
        Err(RuntimeErrorV1::BackendQuiescent(_))
    ));
    assert!(!context.is_terminal());
    assert_eq!(usage(&context, device).quarantined_records, 1);
    assert!(
        context
            .configure_allocation_admission_v1(device, 32, 4)
            .is_err()
    );
    let report = context.cleanup();
    assert!(report.retained().is_empty());
    assert_eq!(report.allocation_credit_records_v1(), 1);
    assert!(!report.is_complete());
    assert!(context.shutdown().is_err());
}

#[test]
fn allocation_credit_profile_cannot_be_installed_over_unaccounted_handles() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let allocation = allocate(&mut context, device, 8).unwrap();
    assert!(
        context
            .configure_allocation_admission_v1(device, 8, 1)
            .is_err()
    );
    assert!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .is_none()
    );
    context.release_allocation(allocation).unwrap();
    context
        .configure_allocation_admission_v1(device, 8, 1)
        .unwrap();
    let token = context.reserve_graph_v1(1).unwrap();
    assert!(matches!(
        context.configure_allocation_admission_v1(device, 8, 1),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    context.close_graph_issue_v1(token).unwrap();
    context.release_graph_v1(token).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_credit_definite_rejection_refunds_but_other_failures_do_not() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
    ] {
        let (mut context, device) = configured(8, 1);
        context.backend.allocation_failure = failure;
        assert!(allocate(&mut context, device, 8).is_err());
        assert!(context.allocations.is_empty());
        let after = usage(&context, device);
        if failure == MockMemoryFailure::Rejected {
            assert_eq!(after.used, RuntimeResourceVectorV1::ZERO);
            assert_eq!(after.quarantined_records, 0);
            assert!(context.backend.memory.is_empty());
            allocate(&mut context, device, 8).unwrap();
            assert!(context.cleanup().is_complete());
        } else {
            assert_eq!(after.used.get(K::RequestedAllocationBytes), 8);
            assert_eq!(after.quarantined_records, 1);
            assert!(allocate(&mut context, device, 1).is_err());
            assert_eq!(context.backend().allocation_calls, 1);
            assert!(!context.cleanup().is_complete());
        }
    }
}

#[test]
fn allocation_credit_release_failures_preserve_exact_charge_and_success_refunds_once() {
    for through_cleanup in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
        ] {
            let (mut context, device) = configured(8, 1);
            let allocation = allocate(&mut context, device, 8).unwrap();
            context.backend.release_allocation_failure = failure;
            if through_cleanup {
                assert!(!context.cleanup().is_complete());
            } else {
                assert!(context.release_allocation(allocation).is_err());
            }
            assert_eq!(
                usage(&context, device)
                    .used
                    .get(K::RequestedAllocationBytes),
                8
            );
            assert_eq!(context.allocations.len(), 1);
            assert!(allocate(&mut context, device, 1).is_err());
            assert_eq!(context.backend().allocation_calls, 1);
            if failure == MockMemoryFailure::Terminal {
                assert_eq!(usage(&context, device).quarantined_records, 1);
                assert!(context.is_terminal());
            } else {
                assert_eq!(usage(&context, device).retained_records, 1);
                context.release_allocation(allocation).unwrap();
                assert_eq!(usage(&context, device).used, RuntimeResourceVectorV1::ZERO);
                assert!(context.release_allocation(allocation).is_err());
                assert!(context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn allocation_credit_invalid_backend_handles_retain_terminal_custody() {
    for duplicate in [false, true] {
        let (mut context, device) = configured(16, 2);
        let handle = if duplicate {
            let allocation = allocate(&mut context, device, 8).unwrap();
            context.allocations[&allocation].backend_allocation
        } else {
            0
        };
        context.backend.handle_override = Some((MockHandleKind::Allocation, handle));
        assert!(matches!(
            allocate(&mut context, device, 8),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        assert!(context.is_terminal());
        let expected = if duplicate { 2 } else { 1 };
        assert_eq!(usage(&context, device).retained_records, expected);
        let report = context.cleanup();
        assert_eq!(report.allocation_credit_records_v1(), expected);
        assert!(!report.is_complete());
    }
}

#[test]
fn allocation_credit_adapter_panic_seals_context_and_never_refunds() {
    for operation in 0..3 {
        let (mut context, device) = configured(8, 1);
        let result = if operation == 0 {
            context.backend.allocation_failure = MockMemoryFailure::Panic;
            catch_unwind(AssertUnwindSafe(|| {
                let _ = allocate(&mut context, device, 8);
            }))
        } else {
            let allocation = allocate(&mut context, device, 8).unwrap();
            context.backend.release_allocation_failure = MockMemoryFailure::Panic;
            catch_unwind(AssertUnwindSafe(|| {
                if operation == 1 {
                    let _ = context.release_allocation(allocation);
                } else {
                    let _ = context.cleanup();
                }
            }))
        };
        assert!(result.is_err());
        assert!(context.is_terminal());
        assert_eq!(
            usage(&context, device)
                .used
                .get(K::RequestedAllocationBytes),
            8
        );
        assert_eq!(usage(&context, device).quarantined_records, 1);
        assert!(!context.cleanup().is_complete());
    }
}

#[test]
fn allocation_credit_preflight_errors_do_not_enter_backend_or_debit() {
    let (mut context, device) = configured(0, 1);
    assert!(allocate(&mut context, device, 1).is_err());
    assert!(allocate(&mut context, device, 0).is_err());
    assert!(
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 1, 3)
            .is_err()
    );
    assert_eq!(context.backend().allocation_calls, 0);
    assert_eq!(usage(&context, device).used, RuntimeResourceVectorV1::ZERO);
    assert!(context.cleanup().is_complete());
}
