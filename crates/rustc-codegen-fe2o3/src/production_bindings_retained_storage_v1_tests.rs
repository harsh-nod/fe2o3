//! Inert original-constructor and accounting controls, NOT authenticated-owner
//! runtime coverage. No AuthenticatedProductionBindings/ProtectedV3 is fabricated.
use super::*;
use fe2o3_compiler_ffi::{CodeObjectVersion, DeviceTargetV1};
use std::path::Path;

fn limits(bytes: Option<usize>, items: usize) -> LogicalStorageLimitsV1 {
    LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    }
}
fn envelope() -> CompilerFfiEnvelopeV1 {
    CompilerFfiEnvelopeV1::for_module_without_device_ffi(
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CodeObjectVersion::V5,
    )
    .unwrap()
}
fn ffi_report(envelope: &CompilerFfiEnvelopeV1) -> fe2o3_compiler_ffi::CompilerFfiLogicalStorageV1 {
    envelope
        .logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
            max_bytes: None,
            max_items: 64,
        })
        .unwrap()
}
fn counter_error(error: LogicalStorageErrorV1) -> BindingsStorageErrorV1 {
    BindingsStorageErrorV1::Counter(error)
}

#[test]
fn standalone_header_once_and_initial_refusals() {
    let header = size_of::<AuthenticatedProductionBindings>();
    let report = Account::new(header, limits(Some(header), 1))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(
        report,
        BindingsRetainedStorageV1 {
            header_bytes: header,
            heap_bytes: 0,
            total_bytes: header,
            visited_items: 1,
        }
    );
    assert_eq!(
        size_of::<BindingsRetainedStorageV1>(),
        4 * size_of::<usize>()
    );
    assert_eq!(
        Account::new(header, limits(Some(header - 1), 1)).err(),
        Some(counter_error(LogicalStorageErrorV1::ByteLimit))
    );
    assert_eq!(
        Account::new(header, limits(Some(header), 0)).err(),
        Some(counter_error(LogicalStorageErrorV1::ItemLimit))
    );
}

#[test]
fn checked_extents_preserve_prefix_on_overflow_and_byte_refusal() {
    let mut account = Account::new(7, limits(Some(17), 9)).unwrap();
    account.extent(5, 2).unwrap();
    assert_eq!(
        account.extent(usize::MAX, 2),
        Err(counter_error(LogicalStorageErrorV1::Arithmetic))
    );
    assert_eq!((account.counter.bytes(), account.counter.items()), (17, 2));
    assert_eq!(
        account.extent(1, 1),
        Err(counter_error(LogicalStorageErrorV1::ByteLimit))
    );
    assert_eq!((account.counter.bytes(), account.counter.items()), (17, 2));
}

#[test]
fn zero_byte_visits_and_item_overflow_are_not_free() {
    let mut short = Account::new(0, limits(Some(0), 1)).unwrap();
    assert_eq!(
        short.extent(usize::MAX, 0),
        Err(counter_error(LogicalStorageErrorV1::ItemLimit))
    );
    assert_eq!((short.counter.bytes(), short.counter.items()), (0, 1));
    let mut overflow = Account::new(0, limits(None, usize::MAX)).unwrap();
    overflow.counter.charge(0, usize::MAX - 1).unwrap();
    assert_eq!(
        overflow.visit(),
        Err(counter_error(LogicalStorageErrorV1::Arithmetic))
    );
    assert_eq!(overflow.counter.items(), usize::MAX);
}

#[test]
fn ffi_none_still_charges_option_visit_and_preserves_prefix() {
    let mut account = Account::new(7, limits(Some(10), 3)).unwrap();
    account.extent(3, 1).unwrap();
    account.ffi(None).unwrap();
    assert_eq!((account.counter.bytes(), account.counter.items()), (10, 3));
    assert_eq!(
        account.ffi(None),
        Err(counter_error(LogicalStorageErrorV1::ItemLimit))
    );
    assert_eq!((account.counter.bytes(), account.counter.items()), (10, 3));
}

#[test]
fn ffi_some_adds_heap_only_with_exact_remaining_byte_and_item_quotas() {
    let envelope = envelope();
    let child = ffi_report(&envelope);
    assert_eq!(child.header_bytes(), size_of::<CompilerFfiEnvelopeV1>());
    assert!(child.heap_bytes() > 0);
    let header = size_of::<AuthenticatedProductionBindings>();
    let prefix = 19;
    let total = header + prefix + child.heap_bytes();
    let items = 3 + child.visited_items(); // header, prefix, Option, then child
    let mut account = Account::new(header, limits(Some(total), items)).unwrap();
    account.extent(prefix, 1).unwrap();
    account.ffi(Some(&envelope)).unwrap();
    assert_eq!(
        account.finish().unwrap(),
        BindingsRetainedStorageV1 {
            header_bytes: header,
            heap_bytes: prefix + child.heap_bytes(),
            total_bytes: total,
            visited_items: items,
        }
    );
    assert_eq!(ffi_report(&envelope), child);
}

#[test]
fn ffi_short_bytes_cannot_reset_or_spend_existing_prefix() {
    let envelope = envelope();
    let child = ffi_report(&envelope);
    let header = size_of::<AuthenticatedProductionBindings>();
    let prefix = 19;
    let mut account = Account::new(
        header,
        limits(Some(header + prefix + child.heap_bytes() - 1), 64),
    )
    .unwrap();
    account.extent(prefix, 1).unwrap();
    assert_eq!(
        account.ffi(Some(&envelope)),
        Err(counter_error(LogicalStorageErrorV1::ByteLimit))
    );
    // Only the outer Option visit survives: delegated partial state is not
    // exposed and MUST NOT be interpreted as a successful measured prefix.
    assert_eq!(
        (account.counter.bytes(), account.counter.items()),
        (header + prefix, 3)
    );
}

#[test]
fn ffi_short_items_fail_inside_bounded_delegate_without_partial_merge() {
    let envelope = envelope();
    let child = ffi_report(&envelope);
    let header = size_of::<AuthenticatedProductionBindings>();
    let prefix = 19;
    let mut account = Account::new(header, limits(None, 3 + child.visited_items() - 1)).unwrap();
    account.extent(prefix, 1).unwrap();
    assert_eq!(
        account.ffi(Some(&envelope)),
        Err(counter_error(LogicalStorageErrorV1::ItemLimit))
    );
    assert_eq!(
        (account.counter.bytes(), account.counter.items()),
        (header + prefix, 3)
    );
}

#[test]
fn ffi_delegate_allowance_addition_overflow_refuses_before_child_walk() {
    let envelope = envelope();
    let mut account = Account::new(0, limits(Some(usize::MAX), 64)).unwrap();
    assert_eq!(
        account.ffi(Some(&envelope)),
        Err(counter_error(LogicalStorageErrorV1::Arithmetic))
    );
    assert_eq!((account.counter.bytes(), account.counter.items()), (0, 2));
}

#[test]
fn original_transaction_payloads_include_spare_native_path_capacity_and_no_headers() {
    let producer =
        ProducerIdentity::from_codegen("kernel", Some(Path::new("src/kernel.rs"))).unwrap();
    let mut output = PathBuf::from("output");
    output.reserve(137);
    let original_path = output.clone();
    let mut producer_heap = 0usize;
    let mut producer_items = 0usize;
    producer
        .visit_retained_heap_storage_v1(|n, w| {
            producer_heap = producer_heap
                .checked_add(n.checked_mul(w).unwrap())
                .unwrap();
            producer_items += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(producer_items, 3);
    let header = size_of::<AuthenticatedProductionBindings>();
    let heap = producer_heap + output.capacity();
    let mut account = Account::new(header, limits(Some(header + heap), 7)).unwrap();
    account
        .transaction_payload(&producer, &output, None)
        .unwrap();
    assert_eq!(
        account.finish().unwrap(),
        BindingsRetainedStorageV1 {
            header_bytes: header,
            heap_bytes: heap,
            total_bytes: header + heap,
            visited_items: 7,
        }
    );
    assert_eq!(output, original_path);
}

#[test]
fn transaction_producer_refusal_stops_before_path_and_ffi_and_keeps_prefix() {
    let producer = ProducerIdentity::from_codegen("kernel", None).unwrap();
    let output = PathBuf::new();
    let envelope = envelope();
    let mut account = Account::new(7, limits(Some(7), 3)).unwrap();
    // Header, transaction and producer root are accepted; the first String
    // has nonzero capacity and fails before subsequent producer/path/FFI work.
    assert_eq!(
        account.transaction_payload(&producer, &output, Some(&envelope)),
        Err(counter_error(LogicalStorageErrorV1::ByteLimit))
    );
    assert_eq!((account.counter.bytes(), account.counter.items()), (7, 3));
}

#[test]
fn original_extraction_marker_has_bounded_visit_without_constructing_protected_authority() {
    let custody = ProductionCompilerCustody::extraction_only();
    let mut account = Account::new(0, limits(Some(0), 2)).unwrap();
    account.extraction_only(&custody).unwrap();
    assert_eq!(account.counter.items(), 2);
    assert_eq!(
        account.extraction_only(&custody),
        Err(counter_error(LogicalStorageErrorV1::ItemLimit))
    );
    // ProtectedV3's exhaustive refusal arm is type checked, not executed by
    // manufacturing a capability or BuildAttempt.
    let _: fn(&mut Account, &ProductionCompilerCustody) -> StorageResult = Account::extraction_only;
}

#[test]
fn child_error_categories_are_preserved_without_success_or_zero_fallback() {
    assert_eq!(
        BindingsStorageErrorV1::from(CompilerFfiLogicalStorageErrorV1::Arithmetic),
        counter_error(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!(
        BindingsStorageErrorV1::from(CompilerFfiLogicalStorageErrorV1::ByteLimit),
        counter_error(LogicalStorageErrorV1::ByteLimit)
    );
    assert_eq!(
        BindingsStorageErrorV1::from(CompilerFfiLogicalStorageErrorV1::ItemLimit),
        counter_error(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!(
        BindingsStorageErrorV1::from(ReferenceRetainedStorageErrorV1::Counter(
            LogicalStorageErrorV1::ItemLimit
        )),
        counter_error(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!(
        BindingsStorageErrorV1::from(ReferenceRetainedStorageErrorV1::ExpressionDepthLimit),
        BindingsStorageErrorV1::ReferenceExpressionDepthLimit
    );
}

#[test]
fn actual_bindings_method_is_type_checked_without_fabricating_authenticated_owner() {
    let _: fn(
        &AuthenticatedProductionBindings,
        LogicalStorageLimitsV1,
    ) -> Result<BindingsRetainedStorageV1, BindingsStorageErrorV1> =
        AuthenticatedProductionBindings::logical_retained_storage_v1;
}
