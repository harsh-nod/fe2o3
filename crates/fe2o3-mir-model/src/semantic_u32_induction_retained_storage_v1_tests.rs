use super::super::SemanticU32InductionNoOverflowCertificateV1;
use super::*;

fn fixture(reachable: bool) -> SemanticU32InductionNoOverflowReportV1 {
    super::super::tests::retained_storage_test_fixture_v1(reachable)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Arithmetic,
    Bytes,
    Items,
}

fn charge(
    state: &mut (usize, usize),
    count: usize,
    width: usize,
    max_bytes: usize,
    max_items: usize,
) -> Result<(), Refusal> {
    let bytes = count.checked_mul(width).ok_or(Refusal::Arithmetic)?;
    let next_bytes = state.0.checked_add(bytes).ok_or(Refusal::Arithmetic)?;
    let next_items = state.1.checked_add(1).ok_or(Refusal::Arithmetic)?;
    if next_bytes > max_bytes {
        return Err(Refusal::Bytes);
    }
    if next_items > max_items {
        return Err(Refusal::Items);
    }
    *state = (next_bytes, next_items);
    Ok(())
}

fn expected(owner: &SemanticU32InductionNoOverflowReportV1) -> (usize, usize) {
    let certificates =
        owner.certificates.len() * size_of::<SemanticU32InductionNoOverflowCertificateV1>();
    match &owner.reachable_blocks {
        None => (certificates, 1),
        Some(scope) => (
            certificates + scope.blocks.capacity() * size_of::<bool>(),
            2,
        ),
    }
}

#[test]
fn original_ordinary_and_reachable_analyzers_keep_all_report_fields_unchanged() {
    for reachable in [false, true] {
        let owner = fixture(reachable);
        let before = owner.clone();
        assert!(!owner.certificates.is_empty());
        assert_eq!(owner.reachable_blocks.is_some(), reachable);
        let exact = expected(&owner);
        let mut state = (0, 0);
        owner
            .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, exact.0, exact.1))
            .unwrap();
        assert_eq!(state, exact);
        assert_eq!(owner, before);
    }
}

#[test]
fn reachable_mask_reports_actual_spare_capacity_not_block_count_or_length() {
    let mut owner = fixture(true);
    let scope = owner.reachable_blocks.as_mut().unwrap();
    let before_len = scope.blocks.len();
    let old_capacity = scope.blocks.capacity();
    scope.blocks.reserve_exact(old_capacity + 17);
    assert_eq!(scope.blocks.len(), before_len);
    assert!(scope.blocks.capacity() > old_capacity);
    let capacity = scope.blocks.capacity();
    let mut rows = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|n, w| {
            rows.push((n, w));
            Ok::<(), Refusal>(())
        })
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1], (capacity, size_of::<bool>()));
    assert!(capacity > before_len);
}

#[test]
fn exact_header_and_one_under_bounds_use_one_shared_checked_ledger() {
    let owner = fixture(true);
    let (heap, visits) = expected(&owner);
    let prefix = 7 + size_of::<SemanticU32InductionNoOverflowReportV1>();
    let total = prefix + heap;
    let mut state = (prefix, 2);
    owner
        .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, total, visits + 2))
        .unwrap();
    assert_eq!(state, (total, visits + 2));
    for (bytes, items, refusal) in [
        (total - 1, visits + 2, Refusal::Bytes),
        (total, visits + 1, Refusal::Items),
    ] {
        let mut state = (prefix, 2);
        assert_eq!(
            owner.visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, bytes, items)),
            Err(refusal)
        );
        assert_eq!(state.1, 3); // completed first callback, not a successful report
    }
}

#[test]
fn every_refusal_short_circuits_and_arithmetic_is_not_saturated() {
    let owner = fixture(true);
    for stop in [1, 2] {
        let mut calls = 0;
        assert_eq!(
            owner.visit_retained_heap_storage_v1(|_, _| {
                calls += 1;
                if calls == stop { Err(stop) } else { Ok(()) }
            }),
            Err(stop)
        );
        assert_eq!(calls, stop);
    }
    let mut state = (usize::MAX, 0);
    assert_eq!(
        owner.visit_retained_heap_storage_v1(|n, w| charge(
            &mut state,
            n,
            w,
            usize::MAX,
            usize::MAX
        )),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (usize::MAX, 0));
    assert_eq!(
        charge(&mut (0, 0), usize::MAX, 2, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
}

#[test]
fn cloned_reports_are_counted_as_distinct_owned_payloads() {
    let first = fixture(true);
    let second = first.clone();
    assert_eq!(first, second);
    assert_ne!(first.certificates.as_ptr(), second.certificates.as_ptr());
    assert_ne!(
        first.reachable_blocks.as_ref().unwrap().blocks.as_ptr(),
        second.reachable_blocks.as_ref().unwrap().blocks.as_ptr()
    );
    let a = expected(&first);
    let b = expected(&second); // cloning may change Vec capacity
    let mut state = (0, 0);
    for owner in [&first, &second] {
        owner
            .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, a.0 + b.0, a.1 + b.1))
            .unwrap();
    }
    assert_eq!(state, (a.0 + b.0, a.1 + b.1));
}
