//! Synthetic result-owner controls; never source or performance evidence.
use super::*;
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::mem::size_of;

type Walk<T> = fn(&T, &mut Counter) -> std::result::Result<(), Error>;
fn measure<T>(value: &T, walk: Walk<T>) -> (usize, usize) {
    let mut c = Counter::new(Limits {
        max_bytes: None,
        max_items: 100_000,
    });
    c.charge(size_of::<T>(), 1).unwrap();
    walk(value, &mut c).unwrap();
    (c.bytes(), c.items())
}
fn limits<T>(value: &T, walk: Walk<T>) {
    let (bytes, items) = measure(value, walk);
    let mut exact = Counter::new(Limits {
        max_bytes: Some(bytes),
        max_items: items,
    });
    exact.charge(size_of::<T>(), 1).unwrap();
    walk(value, &mut exact).unwrap();
    for (budget, expected) in [
        (
            Limits {
                max_bytes: Some(bytes - 1),
                max_items: items,
            },
            Error::ByteLimit,
        ),
        (
            Limits {
                max_bytes: None,
                max_items: items - 1,
            },
            Error::ItemLimit,
        ),
        (
            Limits {
                max_bytes: Some(0),
                max_items: items,
            },
            Error::ByteLimit,
        ),
        (
            Limits {
                max_bytes: None,
                max_items: 0,
            },
            Error::ItemLimit,
        ),
    ] {
        let mut c = Counter::new(budget);
        let actual = c
            .charge(size_of::<T>(), 1)
            .and_then(|()| walk(value, &mut c));
        assert_eq!(actual, Err(expected));
    }
}
fn selector_heap(value: &AuthoringRegionSelectorV1) -> usize {
    value.bundle_identity.capacity()
        + value.canonical_kir_digest.capacity()
        + value.target.capacity()
        + value.operations.capacity() * size_of::<AuthoringOperationCoordinateV1>()
}
fn values_heap(values: &Vec<AuthoringValueV1>) -> usize {
    values.capacity() * size_of::<AuthoringValueV1>()
        + values
            .iter()
            .map(|value| value.ty.capacity())
            .sum::<usize>()
}
fn operation_heap(value: &AuthoringOperationV1) -> usize {
    value.function_name.capacity()
        + value.semantic_detail.as_ref().map_or(0, String::capacity)
        + value.mnemonic.as_ref().map_or(0, String::capacity)
        + value.inline_assembly_source.as_ref().map_or(0, |source| {
            source.frontend_unit.capacity()
                + source.function.capacity()
                + source.contract.capacity()
                + source.statement.capacity()
        })
        + values_heap(&value.inputs)
        + values_heap(&value.results)
        + value.local_memory_effects.capacity() * size_of::<String>()
        + value
            .local_memory_effects
            .iter()
            .map(String::capacity)
            .sum::<usize>()
        + value.source_spans.capacity() * size_of::<AuthoringSourceSpanV1>()
        + value
            .source_spans
            .iter()
            .map(|span| {
                span.file_identity.capacity()
                    + span.display_path.capacity()
                    + span.byte_start.capacity()
                    + span.byte_end.capacity()
            })
            .sum::<usize>()
}
fn operations_heap(values: &Vec<AuthoringOperationV1>) -> usize {
    values.capacity() * size_of::<AuthoringOperationV1>()
        + values.iter().map(operation_heap).sum::<usize>()
}

#[test]
fn summary_all_owned_strings_and_capability_spare_slots_are_counted() {
    let mut result = snapshot(false).summary();
    let before = serde_json::to_vec(&result).unwrap();
    for text in [
        &mut result.bundle_identity,
        &mut result.bundle_subject_identity,
        &mut result.canonical_kir_digest,
        &mut result.canonical_kir_bytes,
        &mut result.target,
        &mut result.source_map_identity,
        &mut result.semantic_mir_identity,
        &mut result.rustc_identity_inventory_receipt_sha256,
        &mut result.rustc_identity_inventory_receipt_bytes,
        &mut result.rustc_preflight_plan_receipt_sha256,
        &mut result.rustc_preflight_plan_receipt_bytes,
    ] {
        text.reserve(43);
    }
    result.capabilities.reserve(17);
    let expected = size_of::<AuthoringSnapshotSummaryV1>()
        + [
            &result.bundle_identity,
            &result.bundle_subject_identity,
            &result.canonical_kir_digest,
            &result.canonical_kir_bytes,
            &result.target,
            &result.source_map_identity,
            &result.semantic_mir_identity,
            &result.rustc_identity_inventory_receipt_sha256,
            &result.rustc_identity_inventory_receipt_bytes,
            &result.rustc_preflight_plan_receipt_sha256,
            &result.rustc_preflight_plan_receipt_bytes,
        ]
        .into_iter()
        .map(String::capacity)
        .sum::<usize>()
        + result.capabilities.capacity() * size_of::<AuthoringCapabilityV1>();
    assert_eq!(
        measure(&result, AuthoringSnapshotSummaryV1::charge_retained_heap_v1).0,
        expected
    );
    limits(&result, AuthoringSnapshotSummaryV1::charge_retained_heap_v1);
    assert_eq!(serde_json::to_vec(&result).unwrap(), before);
    assert_eq!(result.authority, AUTHORITY);
}

#[test]
fn page_accounts_every_nested_optional_and_owned_container() {
    let snapshot = snapshot(true);
    let mut result = snapshot
        .operation_page(&snapshot.summary().bundle_identity, 0, 64)
        .unwrap();
    // Explicit public-result fixture values, not authenticated observations.
    result.operations[0].semantic_detail = Some("fixture-only-detail".into());
    result.operations[0]
        .local_memory_effects
        .push("fixture-only-effect".into());
    let before = serde_json::to_vec(&result).unwrap();
    result.bundle_identity.reserve(19);
    result.canonical_kir_digest.reserve(23);
    result.target.reserve(29);
    result.operations.reserve(31);
    for operation in &mut result.operations {
        operation.function_name.reserve(37);
        if let Some(text) = &mut operation.semantic_detail {
            text.reserve(41);
        }
        if let Some(text) = &mut operation.mnemonic {
            text.reserve(43);
        }
        if let Some(source) = &mut operation.inline_assembly_source {
            source.frontend_unit.reserve(47);
            source.function.reserve(53);
            source.contract.reserve(59);
            source.statement.reserve(61);
        }
        operation.inputs.reserve(5);
        operation.results.reserve(7);
        for value in operation.inputs.iter_mut().chain(&mut operation.results) {
            value.ty.reserve(67);
        }
        operation.local_memory_effects.reserve(11);
        for effect in &mut operation.local_memory_effects {
            effect.reserve(71);
        }
        operation.source_spans.reserve(13);
        for span in &mut operation.source_spans {
            span.file_identity.reserve(73);
            span.display_path.reserve(79);
            span.byte_start.reserve(83);
            span.byte_end.reserve(89);
        }
    }
    let expected = size_of::<AuthoringOperationPageV1>()
        + result.bundle_identity.capacity()
        + result.canonical_kir_digest.capacity()
        + result.target.capacity()
        + operations_heap(&result.operations);
    assert_eq!(
        measure(&result, AuthoringOperationPageV1::charge_retained_heap_v1).0,
        expected
    );
    limits(&result, AuthoringOperationPageV1::charge_retained_heap_v1);
    assert_eq!(serde_json::to_vec(&result).unwrap(), before);
    assert_eq!(result.authority, AUTHORITY);
}

#[test]
fn region_and_input_selector_are_independent_retained_owners() {
    let snapshot = snapshot(false);
    let input = selector(&snapshot, 2);
    let mut result = snapshot.select_region(&input).unwrap();
    assert_eq!(result.selector, input);
    assert_ne!(
        result.selector.bundle_identity.as_ptr(),
        input.bundle_identity.as_ptr()
    );
    let before = serde_json::to_vec(&result).unwrap();
    result.selector.operations.reserve(23);
    result.live_in.reserve(29);
    result.live_out.reserve(31);
    result.operations.reserve(37);
    let expected = size_of::<AuthoringRegionV1>()
        + selector_heap(&result.selector)
        + values_heap(&result.live_in)
        + values_heap(&result.live_out)
        + operations_heap(&result.operations);
    let region = measure(&result, AuthoringRegionV1::charge_retained_heap_v1);
    assert_eq!(region.0, expected);
    let mut c = Counter::new(Limits {
        max_bytes: None,
        max_items: 100_000,
    });
    c.charge(size_of::<AuthoringRegionSelectorV1>(), 1).unwrap();
    input.charge_retained_heap_v1(&mut c).unwrap();
    c.charge(size_of::<AuthoringRegionV1>(), 1).unwrap();
    result.charge_retained_heap_v1(&mut c).unwrap();
    assert_eq!(
        c.bytes(),
        size_of::<AuthoringRegionSelectorV1>() + selector_heap(&input) + region.0
    );
    limits(&input, AuthoringRegionSelectorV1::charge_retained_heap_v1);
    limits(&result, AuthoringRegionV1::charge_retained_heap_v1);
    assert_eq!(serde_json::to_vec(&result).unwrap(), before);
}

#[test]
fn candidate_source_capacity_is_not_its_serialized_length() {
    let snapshot = snapshot(true);
    let selected = selector(&snapshot, 2);
    let mut result = snapshot
        .materialize_typed_rust(&selected, "candidate")
        .unwrap();
    let before = serde_json::to_vec(&result).unwrap();
    result.source.reserve(4096);
    result.helper_name.reserve(101);
    result.selector.bundle_identity.reserve(103);
    result.live_in.reserve(17);
    result.live_out.reserve(19);
    let expected = size_of::<AuthoringRustCandidateV1>()
        + selector_heap(&result.selector)
        + result.helper_name.capacity()
        + result.source.capacity()
        + values_heap(&result.live_in)
        + values_heap(&result.live_out);
    assert_eq!(
        measure(&result, AuthoringRustCandidateV1::charge_retained_heap_v1).0,
        expected
    );
    limits(&result, AuthoringRustCandidateV1::charge_retained_heap_v1);
    assert_eq!(serde_json::to_vec(&result).unwrap(), before);
    assert_eq!(result.authority, AUTHORITY);
}

#[test]
fn snapshot_and_result_share_checked_aggregate_without_duplicate_headers() {
    let snapshot = snapshot(true);
    let result = snapshot.summary();
    let observed = snapshot
        .retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: 100_000,
        })
        .unwrap();
    let (result_bytes, result_items) =
        measure(&result, AuthoringSnapshotSummaryV1::charge_retained_heap_v1);
    let mut c = Counter::new(Limits {
        max_bytes: Some(observed.total_bytes + result_bytes),
        max_items: observed.visited_items + result_items,
    });
    c.charge(observed.total_bytes, observed.visited_items)
        .unwrap();
    c.charge(size_of::<AuthoringSnapshotSummaryV1>(), 1)
        .unwrap();
    result.charge_retained_heap_v1(&mut c).unwrap();
    assert_eq!(c.bytes(), observed.total_bytes + result_bytes);
    let mut overflow = Counter::new(Limits {
        max_bytes: None,
        max_items: usize::MAX,
    });
    overflow.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        result.charge_retained_heap_v1(&mut overflow),
        Err(Error::Arithmetic)
    );
}
