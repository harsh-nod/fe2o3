use super::super::{
    SemanticCollectiveKindV1, SemanticCoverageBindingV1, SemanticEvaluationOrderV1,
    SemanticLoopDirectionV1, SemanticNumericalPolicyV1, SemanticScalarTypeV1,
};
use super::*;
use fe2o3_proof_contracts::DigestV1;

fn d(tag: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([tag; 32])
}

fn root(tag: u8, domain: u8) -> SemanticTypedRootV1 {
    SemanticTypedRootV1::new(
        d(tag),
        d(30),
        d(domain),
        SemanticScalarTypeV1::Unsigned(32),
        SemanticNumericalPolicyV1::ExactBitVector,
    )
    .unwrap()
}

fn full() -> MirPlironSemanticContractV1 {
    MirPlironSemanticContractV1::new(
        d(1),
        d(2),
        d(3),
        vec![
            SemanticFiniteDomainV1::new(d(10), vec![SemanticFiniteExtentV1::Static(16)]).unwrap(),
            SemanticFiniteDomainV1::new(
                d(11),
                vec![
                    SemanticFiniteExtentV1::Static(2),
                    SemanticFiniteExtentV1::Dynamic {
                        symbol: 7,
                        inclusive_upper_bound: 8,
                    },
                ],
            )
            .unwrap(),
        ],
        vec![root(20, 10), root(21, 10), root(22, 11), root(23, 11)],
        vec![
            SemanticLoopContractV1::new(
                d(60),
                0,
                1,
                2,
                d(10),
                d(20),
                d(21),
                d(20),
                d(21),
                d(30),
                d(31),
                SemanticLoopDirectionV1::Increasing,
                16,
            )
            .unwrap(),
        ],
        vec![
            SemanticCollectiveContractV1::new(
                d(70),
                SemanticCollectiveKindV1::FiniteFold,
                d(71),
                d(11),
                d(11),
                d(22),
                d(23),
                d(22),
                d(23),
                16,
                16,
                SemanticEvaluationOrderV1::SequentialAscending,
                SemanticCoverageBindingV1::CollectiveContributions,
            )
            .unwrap(),
        ],
        vec![
            SemanticOutputContractV1::new(d(40), d(50), d(10), d(20), d(21), vec![d(20), d(21)])
                .unwrap(),
            SemanticOutputContractV1::new(d(41), d(51), d(11), d(22), d(23), vec![d(22)]).unwrap(),
        ],
    )
    .unwrap()
}

fn minimal(spare: usize) -> MirPlironSemanticContractV1 {
    let mut extents = Vec::with_capacity(spare);
    extents.push(SemanticFiniteExtentV1::Static(16));
    let mut domains = Vec::with_capacity(spare);
    domains.push(SemanticFiniteDomainV1::new(d(10), extents).unwrap());
    let mut roots = Vec::with_capacity(spare);
    roots.extend([root(20, 10), root(21, 10)]);
    let mut outputs = Vec::with_capacity(spare);
    outputs.push(
        SemanticOutputContractV1::new(d(40), d(50), d(10), d(20), d(21), Vec::with_capacity(spare))
            .unwrap(),
    );
    MirPlironSemanticContractV1::new(
        d(1),
        d(2),
        d(3),
        domains,
        roots,
        Vec::with_capacity(spare),
        Vec::with_capacity(spare),
        outputs,
    )
    .unwrap()
}

fn expected(owner: &MirPlironSemanticContractV1) -> (usize, usize) {
    let mut bytes = owner.domains.len() * size_of::<SemanticFiniteDomainV1>()
        + owner.typed_roots.len() * size_of::<SemanticTypedRootV1>()
        + owner.loops.len() * size_of::<SemanticLoopContractV1>()
        + owner.collectives.len() * size_of::<SemanticCollectiveContractV1>()
        + owner.outputs.len() * size_of::<SemanticOutputContractV1>();
    for domain in &owner.domains {
        bytes += domain.extents.len() * size_of::<SemanticFiniteExtentV1>();
    }
    for output in &owner.outputs {
        bytes += output.auxiliary_roots.len() * size_of::<DigestV1>();
    }
    (bytes, 5 + owner.domains.len() + owner.outputs.len())
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

#[test]
fn original_public_constructors_cover_all_five_and_both_nested_branches() {
    let owner = full();
    let before = owner.clone();
    let before_hash = owner.canonical_sha256();
    let mut rows = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|n, w| {
            rows.push((n, w));
            Ok::<(), Refusal>(())
        })
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (2, size_of::<SemanticFiniteDomainV1>()),
            (1, size_of::<SemanticFiniteExtentV1>()),
            (2, size_of::<SemanticFiniteExtentV1>()),
            (4, size_of::<SemanticTypedRootV1>()),
            (1, size_of::<SemanticLoopContractV1>()),
            (1, size_of::<SemanticCollectiveContractV1>()),
            (2, size_of::<SemanticOutputContractV1>()),
            (2, size_of::<DigestV1>()),
            (1, size_of::<DigestV1>()),
        ]
    );
    assert_eq!(owner, before);
    assert_eq!(owner.canonical_sha256(), before_hash);
}

#[test]
fn empty_collections_still_visit_without_inventing_payload_bytes() {
    let owner = minimal(0);
    let mut rows = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|n, w| {
            rows.push((n, w));
            Ok::<(), Refusal>(())
        })
        .unwrap();
    assert_eq!(rows.len(), 7);
    assert_eq!(rows[3], (0, size_of::<SemanticLoopContractV1>()));
    assert_eq!(rows[4], (0, size_of::<SemanticCollectiveContractV1>()));
    assert_eq!(rows[6], (0, size_of::<DigestV1>()));
    let exact = expected(&owner);
    assert_eq!(rows.iter().map(|(n, w)| n * w).sum::<usize>(), exact.0);
    assert_eq!(exact.1, 7);
}

#[test]
fn enclosing_header_is_once_and_exact_shared_bounds_refuse_one_under() {
    let owner = full();
    let (heap, visits) = expected(&owner);
    let prefix = 13 + size_of::<MirPlironSemanticContractV1>();
    let total = prefix + heap;
    let mut state = (prefix, 2);
    owner
        .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, total, visits + 2))
        .unwrap();
    assert_eq!(state, (total, visits + 2));
    for (max_bytes, max_items, refusal) in [
        (total - 1, visits + 2, Refusal::Bytes),
        (total, visits + 1, Refusal::Items),
    ] {
        let mut state = (prefix, 2);
        assert_eq!(
            owner.visit_retained_heap_storage_v1(|n, w| {
                charge(&mut state, n, w, max_bytes, max_items)
            }),
            Err(refusal)
        );
        assert_eq!(state.1, visits + 1);
        assert!(state.0 < total); // retained prefix is not a complete observation
    }
}

#[test]
fn every_callback_refusal_stops_before_any_later_branch() {
    let owner = full();
    let visits = expected(&owner).1;
    for stop in 1..=visits {
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
}

#[test]
fn equal_clones_own_distinct_allocation_trees_and_both_are_counted() {
    let first = full();
    let second = first.clone();
    assert_eq!(first, second);
    assert_ne!(first.domains.as_ptr(), second.domains.as_ptr());
    assert_ne!(first.typed_roots.as_ptr(), second.typed_roots.as_ptr());
    assert_ne!(first.loops.as_ptr(), second.loops.as_ptr());
    assert_ne!(first.collectives.as_ptr(), second.collectives.as_ptr());
    assert_ne!(first.outputs.as_ptr(), second.outputs.as_ptr());
    for (a, b) in first.domains.iter().zip(second.domains.iter()) {
        assert_ne!(a.extents.as_ptr(), b.extents.as_ptr());
    }
    for (a, b) in first.outputs.iter().zip(second.outputs.iter()) {
        assert_ne!(a.auxiliary_roots.as_ptr(), b.auxiliary_roots.as_ptr());
    }
    let a = expected(&first);
    let b = expected(&second);
    let mut state = (0, 0);
    for owner in [&first, &second] {
        owner
            .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, a.0 + b.0, a.1 + b.1))
            .unwrap();
    }
    assert_eq!(state, (a.0 + b.0, a.1 + b.1));
}

#[test]
fn consumed_vector_spare_capacity_is_not_a_retained_box_extent() {
    let compact = minimal(0);
    let formerly_spare = minimal(64);
    assert_eq!(compact, formerly_spare);
    let exact = expected(&compact);
    assert_eq!(expected(&formerly_spare), exact);
    let mut state = (0, 0);
    formerly_spare
        .visit_retained_heap_storage_v1(|n, w| charge(&mut state, n, w, exact.0, exact.1))
        .unwrap();
    assert_eq!(state, exact);
}

#[test]
fn checked_arithmetic_refuses_instead_of_saturating() {
    let owner = full();
    let mut state = (usize::MAX, 0);
    assert_eq!(
        owner.visit_retained_heap_storage_v1(|n, w| {
            charge(&mut state, n, w, usize::MAX, usize::MAX)
        }),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (usize::MAX, 0));
    let mut state = (0, usize::MAX);
    assert_eq!(
        owner.visit_retained_heap_storage_v1(|n, w| {
            charge(&mut state, n, w, usize::MAX, usize::MAX)
        }),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (0, usize::MAX));
    assert_eq!(
        charge(&mut (0, 0), usize::MAX, 2, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
}
