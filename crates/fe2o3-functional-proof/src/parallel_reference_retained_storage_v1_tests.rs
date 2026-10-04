use super::super::{
    COMPLETE_GPU_HIERARCHY_V1, ParallelHierarchyLevelV1, ParallelNumericalPolicyV1,
    ParallelScheduleRelationV1,
};
use super::*;
use fe2o3_proof_contracts::DigestV1;

fn d(tag: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([tag; 32])
}

fn fixture() -> ParallelReferenceContractV1 {
    let relation = |tag| {
        ParallelOutputRelationV1::new(
            d(tag),
            d(tag + 1),
            d(3),
            d(tag + 2),
            d(tag + 3),
            d(tag + 4),
            ParallelScheduleRelationV1::PointwiseBijection,
            ParallelNumericalPolicyV1::ExactBitVector,
            COMPLETE_GPU_HIERARCHY_V1.to_vec(),
            None,
            d(4),
        )
        .unwrap()
    };
    ParallelReferenceContractV1::new(d(5), d(6), vec![relation(10), relation(20)]).unwrap()
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

fn expected(owner: &ParallelReferenceContractV1) -> usize {
    owner.relations.len() * size_of::<ParallelOutputRelationV1>()
        + owner
            .relations
            .iter()
            .map(|r| r.hierarchy.len() * size_of::<ParallelHierarchyLevelV1>())
            .sum::<usize>()
}

#[test]
fn ordinary_constructor_visits_boxed_row_headers_and_each_hierarchy_once() {
    let owner = fixture();
    let before = owner.canonical_sha256();
    let mut rows = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|count, width| {
            rows.push((count, width));
            Ok::<(), Refusal>(())
        })
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (2, size_of::<ParallelOutputRelationV1>()),
            (4, size_of::<ParallelHierarchyLevelV1>()),
            (4, size_of::<ParallelHierarchyLevelV1>()),
        ]
    );
    assert_eq!(owner.canonical_sha256(), before);
}

#[test]
fn exact_enclosing_header_and_bounds_with_checked_callback() {
    let owner = fixture();
    let header = size_of::<ParallelReferenceContractV1>();
    let total = 11 + header + expected(&owner);
    let mut state = (11 + header, 2);
    owner
        .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, total, 5))
        .unwrap();
    assert_eq!(state, (total, 5));
    for (bytes, items, refusal) in [(total - 1, 5, Refusal::Bytes), (total, 4, Refusal::Items)] {
        let mut state = (11 + header, 2);
        assert_eq!(
            owner.visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, bytes, items)),
            Err(refusal)
        );
        assert!(state.0 < total);
        assert_eq!(state.1, 4); // accepted prefix is not a complete observation
    }
}

#[test]
fn first_refusal_prevents_all_nested_visits() {
    let owner = fixture();
    let mut calls = 0;
    assert_eq!(
        owner.visit_retained_heap_storage_v1(|_, _| {
            calls += 1;
            Err::<(), _>("root allocation refused")
        }),
        Err("root allocation refused")
    );
    assert_eq!(calls, 1);
}

#[test]
fn nested_refusal_stops_before_the_next_relation_and_addition_is_checked() {
    let owner = fixture();
    let mut calls = 0;
    assert_eq!(
        owner.visit_retained_heap_storage_v1(|_, _| {
            calls += 1;
            if calls == 2 {
                Err("hierarchy refused")
            } else {
                Ok(())
            }
        }),
        Err("hierarchy refused")
    );
    assert_eq!(calls, 2);
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
fn equal_clones_retain_separate_outer_and_nested_allocations() {
    let first = fixture();
    let second = first.clone();
    assert_eq!(first, second);
    assert_ne!(first.relations.as_ptr(), second.relations.as_ptr());
    for (a, b) in first.relations.iter().zip(second.relations.iter()) {
        assert_ne!(a.hierarchy.as_ptr(), b.hierarchy.as_ptr());
    }
    let total = 2 * expected(&first);
    let mut state = (0, 0);
    for owner in [&first, &second] {
        owner
            .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, total, 6))
            .unwrap();
    }
    assert_eq!(state, (total, 6));
}
