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

pub(super) fn shared_root(
    bytes: u64,
    records: usize,
) -> fe2o3_resource_accounting::ResourceCreditAccountV1 {
    use fe2o3_resource_accounting::{ResourceCreditAccountV1, resource_domain_bootstrap_bytes_v1};
    ResourceCreditAccountV1::new_root(
        RuntimeResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, bytes)
            .with(K::AllocationRecords, records as u64)
            .with(
                K::ControlResidentBytes,
                resource_domain_bootstrap_bytes_v1(8, records).unwrap(),
            ),
        8,
        records,
    )
    .unwrap()
}

fn in_domain(
    parent: &fe2o3_resource_accounting::ResourceCreditAccountV1,
    bytes: u64,
    records: usize,
) -> (RuntimeContextV1<MockBackend>, RuntimeDeviceIdV1) {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_in_domain_v1(device, parent, bytes, records)
        .unwrap();
    (context, device)
}

#[test]
fn allocation_domain_two_contexts_contend_before_backend_entry_and_release_enables_retry() {
    let root = shared_root(8, 2);
    let baseline = root.usage();
    let parent = root
        .new_child(
            RuntimeResourceVectorV1::ZERO
                .with(K::RequestedAllocationBytes, 16)
                .with(K::AllocationRecords, 2),
            2,
        )
        .unwrap();
    let (mut a, da) = in_domain(&parent, 16, 2);
    let (mut b, db) = in_domain(&parent, 16, 2);
    let first = allocate(&mut a, da, 8).unwrap();
    let before = [root.usage(), parent.usage()];
    assert!(matches!(
        allocate(&mut b, db, 1),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(b.backend.allocation_calls, 0);
    assert_eq!([root.usage(), parent.usage()], before);
    assert_eq!(usage(&b, db).used, RuntimeResourceVectorV1::ZERO);
    a.release_allocation(first).unwrap();
    let second = allocate(&mut b, db, 8).unwrap();
    assert_eq!(b.backend.allocation_calls, 1);
    b.release_allocation(second).unwrap();
    assert!(a.cleanup().is_complete() && b.cleanup().is_complete());
    drop(a);
    drop(b);
    drop(parent);
    assert_eq!(root.usage(), baseline);
}

#[test]
fn allocation_domain_immutable_attachment_cannot_switch_to_an_unrelated_or_local_budget() {
    let root = shared_root(8, 2);
    let other = shared_root(100, 2);
    let (mut context, device) = in_domain(&root, 8, 2);
    let foreign = RuntimeContextV1::open(MockBackend::default())
        .unwrap()
        .devices()[0]
        .id();
    let before = root.usage();
    let other_before = other.usage();
    for target in [device, foreign] {
        let expected = if target == device {
            RuntimeValidationErrorV1::ContextReserved
        } else {
            RuntimeValidationErrorV1::UnknownDevice
        };
        assert!(
            matches!(context.configure_allocation_admission_in_domain_v1(target, &other, 100, 2), Err(RuntimeErrorV1::Validation(error)) if error == expected)
        );
        assert!(
            matches!(context.configure_allocation_admission_v1(target, 100, 2), Err(RuntimeErrorV1::Validation(error)) if error == expected)
        );
    }
    let allocation = allocate(&mut context, device, 8).unwrap();
    context.release_allocation(allocation).unwrap();
    assert!(
        context
            .configure_allocation_admission_v1(device, 100, 2)
            .is_err()
    );
    assert_eq!(root.usage(), before);
    assert_eq!(other.usage(), other_before);
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_domain_repeated_contexts_do_not_reset_quarantined_parent_usage() {
    for failure in [
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let root = shared_root(8, 2);
        let (mut previous, device) = in_domain(&root, 8, 2);
        previous.backend.allocation_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| allocate(&mut previous, device, 8)));
        if failure == MockMemoryFailure::Panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(root.usage().quarantined_records, 1);
        assert!(!previous.cleanup().is_complete());
        drop(previous);
        let before = root.usage();
        for _ in 0..4 {
            let (mut next, device) = in_domain(&root, 8, 2);
            assert!(allocate(&mut next, device, 1).is_err());
            assert_eq!(next.backend.allocation_calls, 0);
            assert!(next.cleanup().is_complete());
            drop(next);
            assert_eq!(root.usage(), before);
        }
    }
}

#[test]
fn allocation_domain_failed_disposal_retains_parent_credit_and_definite_rejection_refunds() {
    let root = shared_root(8, 2);
    let baseline = root.usage();
    let (mut a, da) = in_domain(&root, 8, 2);
    let (mut b, db) = in_domain(&root, 8, 2);
    a.backend.allocation_failure = MockMemoryFailure::Rejected;
    assert!(allocate(&mut a, da, 8).is_err());
    assert_eq!(root.usage(), baseline);
    let allocation = allocate(&mut a, da, 8).unwrap();
    a.backend.release_allocation_failure = MockMemoryFailure::Rejected;
    assert!(a.release_allocation(allocation).is_err());
    assert_eq!(root.usage().retained_records, 1);
    assert!(allocate(&mut b, db, 1).is_err());
    assert_eq!(b.backend.allocation_calls, 0);
    a.release_allocation(allocation).unwrap();
    let allocation = allocate(&mut b, db, 8).unwrap();
    b.release_allocation(allocation).unwrap();
    assert_eq!(root.usage(), baseline);
    assert!(a.cleanup().is_complete() && b.cleanup().is_complete());
}

#[test]
fn allocation_domain_every_ancestor_byte_and_record_limit_precedes_backend_entry() {
    for limiting in 0..3 {
        for records_only in [false, true] {
            let root_records = if records_only && limiting == 0 { 1 } else { 2 };
            let parent_records = if records_only && limiting == 1 {
                1
            } else {
                root_records
            };
            let leaf_records = if records_only && limiting == 2 {
                1
            } else {
                root_records
            };
            let limit = |level| {
                if !records_only && limiting == level {
                    8
                } else {
                    64
                }
            };
            let root = shared_root(limit(0), root_records);
            let parent = root
                .new_child(
                    RuntimeResourceVectorV1::ZERO
                        .with(K::RequestedAllocationBytes, limit(1))
                        .with(K::AllocationRecords, parent_records as u64),
                    parent_records,
                )
                .unwrap();
            let (mut context, device) = in_domain(&parent, limit(2), leaf_records);
            let allocation = allocate(&mut context, device, 8).unwrap();
            let before = (root.usage(), parent.usage(), usage(&context, device));
            assert!(matches!(
                allocate(&mut context, device, 1),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Capacity
                ))
            ));
            assert_eq!(context.backend.allocation_calls, 1);
            assert_eq!(
                (root.usage(), parent.usage(), usage(&context, device)),
                before
            );
            context.release_allocation(allocation).unwrap();
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn allocation_domain_roster_preflight_is_atomic_and_tokens_share_parent_lifetime() {
    let root = shared_root(8, 2);
    let (mut context, device) = in_domain(&root, 16, 2);
    let before = root.usage();
    assert!(
        context
            .allocation_admission
            .prepare_roster(device, &[4, 5])
            .is_err()
    );
    assert_eq!(root.usage(), before);
    assert_eq!(context.backend.allocation_calls, 0);
    let members = context
        .allocation_admission
        .prepare_roster(device, &[3, 5])
        .unwrap()
        .unwrap();
    assert_eq!(root.usage().reserved_records, 2);
    assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 8);
    assert!(allocate(&mut context, device, 1).is_err());
    assert_eq!(context.backend.allocation_calls, 0);
    drop(members);
    assert_eq!(root.usage(), before);
    let allocation = allocate(&mut context, device, 8).unwrap();
    context.release_allocation(allocation).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn allocation_domain_failed_attachment_preserves_parent_and_allows_valid_retry() {
    use fe2o3_resource_accounting::{ResourceCreditAccountV1, resource_domain_bootstrap_bytes_v1};
    for case in 0..3 {
        let capacity = RuntimeResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, 8)
            .with(K::AllocationRecords, 2);
        let parent = match case {
            0 => ResourceCreditAccountV1::new(capacity, 2).unwrap(),
            1 => ResourceCreditAccountV1::new_root(
                capacity.with(
                    K::ControlResidentBytes,
                    resource_domain_bootstrap_bytes_v1(1, 2).unwrap(),
                ),
                1,
                2,
            )
            .unwrap(),
            _ => shared_root(8, 2)
                .new_child(capacity, 2)
                .unwrap()
                .new_child(capacity, 2)
                .unwrap(),
        };
        let before = (parent.usage(), parent.root_usage());
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        assert!(matches!(
            context.configure_allocation_admission_in_domain_v1(device, &parent, 8, 2),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
        assert!(
            context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .is_none()
        );
        assert_eq!((parent.usage(), parent.root_usage()), before);
        assert_eq!(context.backend.allocation_calls, 0);
        let valid = shared_root(8, 2);
        context
            .configure_allocation_admission_in_domain_v1(device, &valid, 8, 2)
            .unwrap();
        let allocation = allocate(&mut context, device, 8).unwrap();
        context.release_allocation(allocation).unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn allocation_domain_terminal_or_panicking_disposal_keeps_replacement_context_blocked() {
    for failure in [MockMemoryFailure::Terminal, MockMemoryFailure::Panic] {
        let root = shared_root(8, 2);
        let (mut context, device) = in_domain(&root, 8, 2);
        let allocation = allocate(&mut context, device, 8).unwrap();
        context.backend.release_allocation_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| context.release_allocation(allocation)));
        if failure == MockMemoryFailure::Panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(_))
            ));
        }
        assert_eq!(root.usage().quarantined_records, 1);
        drop(context);
        let before = root.usage();
        let (mut next, device) = in_domain(&root, 8, 2);
        assert!(allocate(&mut next, device, 1).is_err());
        assert_eq!(next.backend.allocation_calls, 0);
        assert_eq!(root.usage(), before);
        assert!(next.cleanup().is_complete());
    }
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
            assert_eq!(context.backend.memory.len(), 1);
            assert_eq!(context.backend.memory.values().next().unwrap(), &[0; 8]);
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
        assert_eq!(usage(&context, device).retained_records, 0);
        assert_eq!(usage(&context, device).quarantined_records, expected);
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
