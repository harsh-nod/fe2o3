use super::super::shapes::Shapes as ColdShapes;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "../../global_statement_index_v1/fixture.rs"]
mod fixture;
use fixture::{callable, ty};

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn facts(types: &[SemanticTypeDeclV1]) -> Vec<GlobalBf16BorrowV1> {
    [callable(0, 62), callable(15, 63), callable(0, 64)]
        .iter()
        .map(|callable| GlobalBf16BorrowV1::for_callable(types, callable).unwrap())
        .collect()
}

fn aggregate(index: u8, fields: &[u32]) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index; 32]),
        SemanticLayoutIdentityV1::from_sha256([index; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(fields.len() as u64 * 8),
            8,
            SemanticAggregateLayoutV1::new(
                (0..fields.len()).map(|i| i as u64 * 8).collect(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(fields.iter().copied().map(ty).collect()).unwrap(),
        ),
    )
}

fn graph() -> Vec<SemanticTypeDeclV1> {
    let mut types = fixture::types();
    // Same graph cases as Shapes89, not substitute source-export fixtures.
    for (index, fields) in [
        (30, vec![7, 7, 2]),
        (31, vec![2, 3]),
        (32, vec![30, 31, 30]),
        (33, vec![8]),
        (34, vec![34, 7]),
        (35, vec![7, u32::MAX]),
        (36, vec![37]),
        (37, vec![36]),
        (38, vec![7, 34]),
        (39, vec![]),
    ] {
        types.push(aggregate(index, &fields));
    }
    types
}

fn oracle(
    types: &[SemanticTypeDeclV1],
    facts: &[GlobalBf16BorrowV1],
    ty: SemanticTypeIdV1,
    path: &mut BTreeSet<SemanticTypeIdV1>,
) -> Result<bool, ()> {
    let decl = types.get(ty.index() as usize).ok_or(())?;
    if facts.iter().any(|fact| fact.pairs()[2].0 == ty) {
        return Ok(true);
    }
    if facts.iter().any(|fact| fact.pairs()[0].1 == ty) {
        return Ok(false);
    }
    let fields = match decl.shape() {
        SemanticTypeShapeV1::Aggregate(a) | SemanticTypeShapeV1::Tuple(a) => a.fields(),
        _ => return Ok(false),
    };
    if !path.insert(ty) {
        return Err(());
    }
    let mut found = false;
    for &field in fields {
        found |= oracle(types, facts, field, path)?;
    }
    path.remove(&ty);
    Ok(found)
}

fn cells<T>() -> usize {
    std::mem::size_of::<T>().div_ceil(std::mem::size_of::<usize>())
}

fn header_work() -> usize {
    let word = std::mem::size_of::<usize>();
    let bytes = std::mem::size_of::<&[SemanticTypeDeclV1]>()
        + std::mem::size_of::<Vec<u64>>()
        + std::mem::size_of::<bool>();
    let padded = bytes.div_ceil(word) * word;
    assert_eq!(std::mem::size_of::<Memo<'_>>(), padded);
    padded / word
}

fn tree_work(mut len: usize) -> usize {
    let mut work = 1;
    while len != 0 {
        work += 1;
        len /= 2;
    }
    work
}

fn work_failure(error: ProductionSemanticSsaErrorV1) {
    assert!(matches!(
        flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        }
    ));
}

#[test]
fn indexed_shapes_match_shapes89_and_recursive_oracle_on_all_original_graph_cases() {
    let types = graph();
    let facts = facts(&types);
    for index in (0..types.len() as u32).chain([u32::MAX]) {
        let expected = oracle(&types, &facts, ty(index), &mut BTreeSet::new());
        let mut cold_budget = budget(MAX_FLOW_WORK);
        let mut hot_budget = budget(MAX_FLOW_WORK);
        let mut cold = ColdShapes::new(&types, &facts, &mut cold_budget).unwrap();
        let mut hot = Shapes::new(&types, &facts, &mut hot_budget).unwrap();
        let original = cold.contains(ty(index), &mut cold_budget).map_err(|_| ());
        let indexed = hot.contains(ty(index), &mut hot_budget).map_err(|_| ());
        assert_eq!(indexed, expected, "type={index}");
        assert_eq!(indexed, original, "type={index}");
        if expected.is_ok() {
            for _ in 0..3 {
                assert_eq!(
                    hot.contains(ty(index), &mut hot_budget).map_err(|_| ()),
                    expected
                );
                assert_eq!(
                    cold.contains(ty(index), &mut cold_budget).map_err(|_| ()),
                    expected
                );
            }
        }
    }
}

#[test]
fn packed_coordinates_roundtrip_without_neighbor_alias_or_padding_access() {
    let mut types = fixture::types();
    types.resize(70, types[2].clone());
    let mut work = budget(MAX_FLOW_WORK);
    let mut memo = Memo::new(&types, &mut work).unwrap();
    let mut expected = vec![None; 70];
    for index in [0usize, 31, 32, 63, 64, 69, 1, 30, 33, 62, 65] {
        for value in [None, Some(false), Some(true)] {
            let before = work.remaining;
            memo.insert(ty(index as u32), value, &mut work).unwrap();
            assert_eq!(before - work.remaining, 2 * cells::<u64>());
            expected[index] = Some(value);
            for (local, &wanted) in expected.iter().enumerate() {
                let before = work.remaining;
                assert_eq!(memo.get(ty(local as u32), &mut work).unwrap(), wanted);
                assert_eq!(before - work.remaining, cells::<u64>());
            }
        }
    }
    for index in [70, 95, 96, u32::MAX] {
        let before = memo.words.clone();
        assert!(memo.get(ty(index), &mut work).is_err());
        assert!(memo.insert(ty(index), Some(true), &mut work).is_err());
        assert_eq!(memo.words, before);
    }
    let before = memo.words.clone();
    work_failure(
        memo.insert(ty(1), Some(true), &mut budget(2 * cells::<u64>() - 1))
            .unwrap_err(),
    );
    assert_eq!(memo.words, before);
}

#[test]
fn indexed_storage_counts_growth_old_new_capacity_and_initialization_independently() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    assert_eq!(TYPES_PER_WORD, 32);
    for len in [0usize, 1, 31, 32, 33, 64, 65, 2049] {
        let mut types = fixture::types();
        types.resize(len, types[2].clone());
        let mut work = budget(MAX_FLOW_WORK);
        let memo = Memo::new(&types, &mut work).unwrap();
        let count = len.div_ceil(32);
        let mut expected = header_work();
        let mut original: Vec<u64> = Vec::new();
        // Independent allocation trace, not a call to the production push helper.
        for _ in 0..count {
            if original.len() == original.capacity() {
                let requested = original.capacity().max(1) * 2;
                original.reserve_exact(requested - original.len());
                expected += (original.capacity() + original.len()) * cells::<u64>();
            }
            expected += cells::<u64>();
            original.push(0);
        }
        assert_eq!(memo.words, original);
        assert_eq!(memo.words.capacity(), original.capacity());
        assert_eq!(MAX_FLOW_WORK - work.remaining, expected);
        let mut exact = budget(expected);
        assert!(Memo::new(&types, &mut exact).is_ok());
        assert_eq!(exact.remaining, 0);
        match Memo::new(&types, &mut budget(expected - 1)) {
            Ok(_) => panic!("unpaid packed memo for {len} types"),
            Err(error) => work_failure(error),
        }
        let mut exact = budget(2 * expected);
        let shapes = Shapes::new(&types, &[], &mut exact).unwrap();
        assert_eq!(exact.remaining, 0);
        for memo in [&shapes.memo, &shapes.shared_memo] {
            assert_eq!(memo.words, original);
            assert_eq!(memo.words.capacity(), original.capacity());
            assert!(std::ptr::eq(memo.types, types.as_slice()));
        }
        if count != 0 {
            assert_ne!(
                shapes.memo.words.as_ptr(),
                shapes.shared_memo.words.as_ptr()
            );
        }
        match Shapes::new(&types, &[], &mut budget(2 * expected - 1)) {
            Ok(_) => panic!("unpaid second packed memo for {len} types"),
            Err(error) => work_failure(error),
        }
    }
}

#[test]
fn cold_and_warm_costs_have_independent_exact_and_every_short_boundaries() {
    let types = fixture::types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    // Owner; root/pop/final memo reads; pending capacity/write; unchanged
    // classification; read-modify-write. One authenticated pair and barrier.
    let cold = 1
        + 3 * cells::<u64>()
        + 3 * cells::<(SemanticTypeIdV1, bool)>()
        + (4 + 2 + 2)
        + 2 * cells::<u64>();
    let warm = 1 + cells::<u64>();
    for limit in 0..=cold {
        let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
        let mut work = budget(limit);
        match shapes.contains(ty(7), &mut work) {
            Ok(selected) => {
                assert!(selected);
                assert_eq!(limit, cold);
                assert_eq!(work.remaining, 0);
            }
            Err(error) => {
                work_failure(error);
                assert!(shapes.memo.failed);
                assert!(matches!(
                    shapes.contains(ty(7), &mut budget(MAX_FLOW_WORK)),
                    Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
                ));
                if limit == cold - 1 {
                    assert_eq!(
                        shapes.memo.get(ty(7), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(Some(true))
                    );
                }
            }
        }
    }
    for limit in 0..=warm {
        let mut setup = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
        assert!(shapes.contains(ty(7), &mut setup).unwrap());
        let mut work = budget(limit);
        match shapes.contains(ty(7), &mut work) {
            Ok(selected) => {
                assert!(selected);
                assert_eq!(limit, warm);
                assert_eq!(work.remaining, 0);
            }
            Err(error) => {
                work_failure(error);
                assert!(shapes.contains(ty(7), &mut budget(MAX_FLOW_WORK)).is_err());
            }
        }
    }
}

#[test]
fn distinct_scalar_cold_cost_delta_includes_both_representations_full_storage() {
    let mut types = fixture::types();
    types.resize(96, types[2].clone());
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let mut old_work = budget(MAX_FLOW_WORK);
    let mut new_work = budget(MAX_FLOW_WORK);
    let mut old = ColdShapes::new(&types, &facts, &mut old_work).unwrap();
    let mut new = Shapes::new(&types, &facts, &mut new_work).unwrap();
    let slot = cells::<Option<(SemanticTypeIdV1, bool)>>();
    let word = std::mem::size_of::<usize>();
    let old_bytes = 64 * std::mem::size_of::<Option<(SemanticTypeIdV1, bool)>>()
        + std::mem::size_of::<&[SemanticTypeDeclV1]>()
        + std::mem::size_of::<bool>();
    let mut expected_old = 2 * old_bytes.div_ceil(word) + 8;
    // Each memo: three zero words, capacities 2 then 4, old live length 2.
    let mut expected_new = 2 * (header_work() + (2 + 4 + 2 + 3) * cells::<u64>()) + 8;
    for (n, index) in (30..94).enumerate() {
        assert!(!old.contains(ty(index), &mut old_work).unwrap());
        assert!(!new.contains(ty(index), &mut new_work).unwrap());
        expected_old += 3 * tree_work(n)
            + 3 * cells::<(SemanticTypeIdV1, bool)>()
            + 8
            + tree_work(n + 1)
            + 1
            + 2 * slot;
        expected_new += 1 + 5 * cells::<u64>() + 3 * cells::<(SemanticTypeIdV1, bool)>() + 8;
    }
    for _ in 0..32 {
        for index in 30..94 {
            assert!(!old.contains(ty(index), &mut old_work).unwrap());
            assert!(!new.contains(ty(index), &mut new_work).unwrap());
            expected_old += 1 + slot;
            expected_new += 1 + cells::<u64>();
        }
    }
    assert_eq!(MAX_FLOW_WORK - old_work.remaining, expected_old);
    assert_eq!(MAX_FLOW_WORK - new_work.remaining, expected_new);
    assert!(expected_new < expected_old);
}

#[test]
fn exact_owner_and_fact_scope_do_not_transfer_memo_verdicts() {
    let types = fixture::types();
    let foreign = types.clone();
    let facts = facts(&types);
    let mut work = budget(MAX_FLOW_WORK);
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    assert!(shapes.contains(ty(7), &mut work).unwrap());
    shapes.types = &foreign;
    let before = work.remaining;
    assert!(matches!(
        shapes.contains(ty(7), &mut work),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(before - work.remaining, 1);
    shapes.types = &types;
    assert!(shapes.contains(ty(7), &mut work).is_err());
    let mut separate = Shapes::new(&types, &[], &mut work).unwrap();
    assert!(!separate.contains(ty(7), &mut work).unwrap());
    let mut fresh = Shapes::new(&foreign, &facts, &mut work).unwrap();
    assert!(fresh.contains(ty(7), &mut work).unwrap());
}

#[test]
fn cycles_and_malformed_later_fields_cannot_hide_behind_selected_siblings() {
    let types = graph();
    let facts = facts(&types);
    for index in [34, 35, 36, 37, 38] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
        assert!(shapes.contains(ty(7), &mut work).unwrap());
        assert!(!shapes.contains(ty(8), &mut work).unwrap());
        assert!(!shapes.contains(ty(33), &mut work).unwrap());
        assert!(matches!(
            shapes.contains(ty(index), &mut work),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert!(shapes.memo.failed);
        assert!(shapes.contains(ty(7), &mut work).is_err());
    }
}

#[test]
fn duplicate_facts_and_pointee_and_field_interfaces_remain_exact() {
    let types = graph();
    let facts = facts(&types);
    let mut old_work = budget(MAX_FLOW_WORK);
    let mut new_work = budget(MAX_FLOW_WORK);
    let old = ColdShapes::new(&types, &facts, &mut old_work).unwrap();
    let new = Shapes::new(&types, &facts, &mut new_work).unwrap();
    assert_eq!(new.pairs.len(), 2);
    assert_eq!(new.barriers.len(), 2);
    for index in (0..types.len() as u32).chain([u32::MAX]) {
        let old_before = old_work.remaining;
        let new_before = new_work.remaining;
        assert_eq!(
            old.pointee(ty(index), &mut old_work).unwrap(),
            new.pointee(ty(index), &mut new_work).unwrap()
        );
        assert_eq!(
            old_before - old_work.remaining,
            new_before - new_work.remaining
        );
        assert_eq!(old.fields(ty(index)), new.fields(ty(index)));
    }
}

fn expected_charge_prefix(limit: usize, charges: &[usize]) -> (usize, Option<usize>) {
    let mut remaining = limit;
    for &charge in charges {
        let Some(next) = remaining.checked_sub(charge) else {
            return (remaining, Some(charge));
        };
        remaining = next;
    }
    (remaining, None)
}

fn assert_denied_charge(
    error: ProductionSemanticSsaErrorV1,
    limit: usize,
    remaining: usize,
    requested: usize,
) {
    match &error {
        ProductionSemanticSsaErrorV1::BorrowFlowWork {
            remaining_work_units,
            requested_work_units,
            phase_work_units,
            ..
        } => {
            assert_eq!(*remaining_work_units, remaining);
            assert_eq!(*requested_work_units, requested);
            assert_eq!(phase_work_units.iter().sum::<usize>(), limit - remaining);
        }
        _ => panic!("expected original work-limit wrapper"),
    }
    assert!(matches!(
        flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required,
            limit: actual,
        } if required == limit + 1 && actual == limit
    ));
}

fn nested_32_charge_trace() -> (Vec<usize>, usize) {
    let w = cells::<u64>();
    let p = cells::<(SemanticTypeIdV1, bool)>();
    // Explicit source trace, not production traversal or a measured run:
    // 32=[30,31,30], 30=[7,7,2], 31=[2,3]. Six types are classified.
    // Each push is separate; growth charges include new capacity + old live
    // length (2+0, 4+2, 8+4), then surplus 0 and the inserted cell.
    let mut charges = vec![1, w, 2 * p, 0, p]; // owner, root read/push
    charges.extend([w, 10, 2 * w, p, p, 6 * p, 0, p, p]); // enter32, push finish/children
    charges.extend([w, 10, 2 * w, p, 12 * p, 0, p, p, p]); // enter30, push finish/children
    charges.extend([w, 10, 2 * w]); // complete7
    charges.push(w); // duplicate7, already complete
    charges.extend([w, 10, 2 * w]); // complete2
    charges.extend([w, w, w, w, 2 * w]); // finish30: pop, all three field reads, write
    let after_descendant = charges.iter().sum();
    charges.extend([w, 10, 2 * w, p, p, p]); // enter31, push finish/children
    charges.push(w); // duplicate2, already complete
    charges.extend([w, 10, 2 * w]); // complete3
    charges.extend([w, w, w, 2 * w]); // finish31: pop, two fields, write
    charges.push(w); // duplicate30, already complete
    charges.extend([w, w, w, w, 2 * w, w]); // finish32, then final root read
    // 22 reads, nine read-modify-writes, 12 pushes and 20 growth/relocation
    // cells, six original 10-unit classifications, and the owner guard.
    assert_eq!(charges.iter().sum::<usize>(), 61 + 40 * w + 32 * p);
    (charges, after_descendant)
}

#[test]
fn nested_duplicate_32_has_independent_full_cost_and_every_short_budget_poison() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    let types = graph();
    let facts = facts(&types);
    let (charges, after_descendant) = nested_32_charge_trace();
    let total = charges.iter().sum::<usize>();
    assert_eq!(
        oracle(&types, &facts, ty(32), &mut BTreeSet::new()),
        Ok(true)
    );
    let mut cold_work = budget(MAX_FLOW_WORK);
    let mut cold = ColdShapes::new(&types, &facts, &mut cold_work).unwrap();
    assert!(cold.contains(ty(32), &mut cold_work).unwrap());
    let mut saw_completed_descendant_failure = false;
    for limit in 0..=total {
        let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
        let mut work = budget(limit);
        let (remaining, denied) = expected_charge_prefix(limit, &charges);
        let result = shapes.contains(ty(32), &mut work);
        assert_eq!(work.remaining, remaining, "limit={limit}");
        match (result, denied) {
            (Ok(value), None) => {
                assert!(value);
                assert_eq!(limit, total);
                assert_eq!(remaining, 0);
                assert!(!shapes.memo.failed);
                for (index, selected) in [
                    (2, false),
                    (3, false),
                    (7, true),
                    (30, true),
                    (31, false),
                    (32, true),
                ] {
                    assert_eq!(
                        shapes
                            .memo
                            .get(ty(index), &mut budget(MAX_FLOW_WORK))
                            .unwrap(),
                        Some(Some(selected))
                    );
                }
                assert_eq!(
                    shapes.memo.get(ty(34), &mut budget(MAX_FLOW_WORK)).unwrap(),
                    None
                );
            }
            (Err(error), Some(requested)) => {
                assert_denied_charge(error, limit, remaining, requested);
                assert!(shapes.memo.failed);
                if limit == after_descendant {
                    saw_completed_descendant_failure = true;
                    assert_eq!(remaining, 0);
                    assert_eq!(requested, cells::<u64>());
                    assert_eq!(
                        shapes.memo.get(ty(30), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(Some(true))
                    );
                    assert_eq!(
                        shapes.memo.get(ty(32), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(None)
                    );
                    assert_eq!(
                        shapes.memo.get(ty(31), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        None
                    );
                    let mut fresh = budget(MAX_FLOW_WORK);
                    assert!(matches!(
                        shapes.contains(ty(30), &mut fresh),
                        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
                    ));
                    assert_eq!(fresh.remaining, MAX_FLOW_WORK - 1);
                }
                if limit == total - cells::<u64>() {
                    assert_eq!(
                        shapes.memo.get(ty(32), &mut budget(MAX_FLOW_WORK)).unwrap(),
                        Some(Some(true))
                    );
                }
                let mut fresh = budget(MAX_FLOW_WORK);
                assert!(matches!(
                    shapes.contains(ty(32), &mut fresh),
                    Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
                ));
                assert_eq!(fresh.remaining, MAX_FLOW_WORK - 1);
            }
            _ => panic!("nested trace boundary mismatch at {limit}"),
        }
    }
    assert!(saw_completed_descendant_failure);
}

#[test]
fn indexed_multifact_constructor_includes_memo_and_every_short_total_budget() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    let types = graph();
    let facts = facts(&types);
    assert_eq!(types.len(), 40);
    assert_eq!(facts.len(), 3);
    // Raw then shared memo: each has capacity 2, surplus 0 and two writes.
    // Fact pair/barrier cardinalities before insertion are 0, 1, 2.
    let w = cells::<u64>();
    let charges = [
        header_work(),
        2 * w,
        0,
        w,
        w,
        header_work(),
        2 * w,
        0,
        w,
        w,
        8,
        10,
        12,
    ];
    let total = 2 * (header_work() + 4 * w) + 8 + 10 + 12;
    assert_eq!(charges.iter().sum::<usize>(), total);
    for limit in 0..=total {
        let mut work = budget(limit);
        let (remaining, denied) = expected_charge_prefix(limit, &charges);
        let result = Shapes::new(&types, &facts, &mut work);
        assert_eq!(work.remaining, remaining, "constructor limit={limit}");
        match (result, denied) {
            (Ok(shapes), None) => {
                assert_eq!(limit, total);
                assert_eq!(remaining, 0);
                assert_eq!(shapes.pairs.len(), 2);
                assert_eq!(shapes.barriers.len(), 2);
                assert_eq!(shapes.memo.words, vec![0; 2]);
                assert_eq!(shapes.shared_memo.words, vec![0; 2]);
                assert!(!shapes.memo.failed);
                assert!(std::ptr::eq(shapes.types, shapes.memo.types));
                assert!(std::ptr::eq(shapes.types, shapes.shared_memo.types));
                assert_eq!(
                    shapes.pointee(ty(7), &mut budget(MAX_FLOW_WORK)).unwrap(),
                    Some(ty(6))
                );
                assert_eq!(
                    shapes.pointee(ty(22), &mut budget(MAX_FLOW_WORK)).unwrap(),
                    Some(ty(21))
                );
            }
            (Err(error), Some(requested)) => {
                assert_denied_charge(error, limit, remaining, requested)
            }
            _ => panic!("constructor trace boundary mismatch at {limit}"),
        }
    }
}

#[test]
fn same_base_shortened_type_owner_rejects_warm_in_range_key_and_stays_poisoned() {
    let types = fixture::types();
    let facts = facts(&types);
    let mut work = budget(MAX_FLOW_WORK);
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    assert!(shapes.contains(ty(7), &mut work).unwrap());
    let words = shapes.memo.words.clone();
    let shorter = &types[..8];
    assert_eq!(shorter.as_ptr(), types.as_ptr());
    assert_ne!(shorter.len(), types.len());
    assert!(ty(7).index() < shorter.len() as u32);
    shapes.types = shorter;
    let before = work.remaining;
    assert!(matches!(
        shapes.contains(ty(7), &mut work),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(before - work.remaining, 1);
    assert!(shapes.memo.failed);
    assert_eq!(shapes.memo.words, words);
    shapes.types = &types;
    for index in [7, 2] {
        let mut fresh = budget(MAX_FLOW_WORK);
        assert!(matches!(
            shapes.contains(ty(index), &mut fresh),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(fresh.remaining, MAX_FLOW_WORK - 1);
        assert_eq!(shapes.memo.words, words);
    }
}

fn shared_graph() -> Vec<SemanticTypeDeclV1> {
    let mut types = graph();
    // Selected, unselected, reference chain, cycle, missing, bad field, self.
    for pointee in [30, 31, 40, 34, u32::MAX, 35, 46] {
        let index = types.len() as u8;
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([index; 32]),
            SemanticLayoutIdentityV1::from_sha256([index; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
    }
    types
}

fn shared_poisoned_retries(shapes: &mut Shapes<'_>) {
    assert!(shapes.memo.failed);
    let raw = shapes.memo.words.clone();
    let shared = shapes.shared_memo.words.clone();
    for index in [40, 41] {
        for mode in 0..3 {
            let mut work = budget(MAX_FLOW_WORK);
            let result = match mode {
                0 => shapes.contains(ty(index), &mut work),
                1 => shapes.transport_contains(ty(index), &mut work),
                _ => shapes
                    .shared_carrier_pointee(ty(index), &mut work)
                    .map(|p| p.is_some()),
            };
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
            ));
            assert_eq!(work.remaining, MAX_FLOW_WORK - 1);
        }
    }
    let mut work = budget(MAX_FLOW_WORK);
    assert!(matches!(
        shapes.for_each_transport_declaration(&[], &mut work, |_, _, _, _| {
            panic!("poisoned cache entered a declaration callback")
        }),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(work.remaining, MAX_FLOW_WORK - 2);
    assert_eq!(shapes.memo.words, raw);
    assert_eq!(shapes.shared_memo.words, shared);
}

#[test]
fn shared_cache121_misses_hits_and_every_short_budget_have_independent_charge_traces() {
    let types = shared_graph();
    let facts = facts(&types);
    let w = cells::<u64>();
    let mut denied_publish = false;
    let mut denied_hit_read = false;
    for (index, pointee) in [
        (40, Some(30)),
        (41, None),
        (42, None),
        (44, None),
        (46, None),
    ] {
        for warm in [false, true] {
            for transport in [false, true] {
                // Owner, optional raw transport read, pointer guard, shared read.
                let mut charges = vec![1];
                if transport {
                    charges.push(w);
                }
                charges.extend([1, w]);
                if warm {
                    if pointee.is_some() {
                        charges.push(1);
                    }
                } else {
                    charges.push(13);
                    if index == 40 || index == 41 {
                        charges.push(w);
                    }
                    charges.push(2 * w); // Publish only the completed shared verdict.
                }
                let total = charges.iter().sum::<usize>();
                for limit in 0..=total {
                    let mut setup = budget(MAX_FLOW_WORK);
                    let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
                    for raw in [30, 31, index] {
                        shapes.contains(ty(raw), &mut setup).unwrap();
                    }
                    if warm {
                        assert_eq!(
                            shapes
                                .shared_carrier_pointee(ty(index), &mut setup)
                                .unwrap(),
                            pointee.map(ty)
                        );
                    }
                    let raw = shapes.memo.words.clone();
                    let shared = shapes.shared_memo.words.clone();
                    let mut work = budget(limit);
                    let (remaining, denied) = expected_charge_prefix(limit, &charges);
                    let result = if transport {
                        shapes.transport_contains(ty(index), &mut work)
                    } else {
                        shapes
                            .shared_carrier_pointee(ty(index), &mut work)
                            .map(|actual| {
                                assert_eq!(actual, pointee.map(ty));
                                actual.is_some()
                            })
                    };
                    assert_eq!(
                        work.remaining, remaining,
                        "type={index} warm={warm} transport={transport} limit={limit}"
                    );
                    assert_eq!(shapes.memo.words, raw);
                    match (result, denied) {
                        (Ok(selected), None) => {
                            assert_eq!(selected, pointee.is_some());
                            assert_eq!(limit, total);
                            assert_eq!(remaining, 0);
                            assert_eq!(
                                shapes.shared_memo.get(ty(index), &mut setup).unwrap(),
                                Some(Some(selected))
                            );
                        }
                        (Err(error), Some(requested)) => {
                            assert_denied_charge(error, limit, remaining, requested);
                            assert_eq!(shapes.shared_memo.words, shared);
                            if limit == total - 1 {
                                if !warm {
                                    assert_eq!(requested, 2 * w);
                                    denied_publish = true;
                                } else if pointee.is_some() {
                                    assert_eq!(requested, 1);
                                    denied_hit_read = true;
                                }
                            }
                            shared_poisoned_retries(&mut shapes);
                        }
                        _ => panic!("shared cache charge trace mismatch"),
                    }
                }
            }
        }
    }
    assert!(denied_publish && denied_hit_read);
}

#[test]
fn shared_cache121_owner_and_fact_scope_cover_both_memos_and_poisoned_retries() {
    let types = shared_graph();
    let foreign = types.clone();
    let facts = facts(&types);
    for owner in [foreign.as_slice(), &types[..41]] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
        assert_eq!(
            shapes.shared_carrier_pointee(ty(40), &mut work).unwrap(),
            Some(ty(30))
        );
        assert_eq!(
            shapes.shared_carrier_pointee(ty(41), &mut work).unwrap(),
            None
        );
        let raw = shapes.memo.words.clone();
        let shared = shapes.shared_memo.words.clone();
        shapes.types = owner;
        let mut guard = budget(1);
        assert!(matches!(
            shapes.shared_carrier_pointee(ty(40), &mut guard),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(guard.remaining, 0);
        assert_eq!(shapes.memo.words, raw);
        assert_eq!(shapes.shared_memo.words, shared);
        shapes.types = &types;
        shared_poisoned_retries(&mut shapes);
    }
    for owner in [types.as_slice(), foreign.as_slice()] {
        for present in [false, true] {
            let mut work = budget(MAX_FLOW_WORK);
            let mut shapes =
                Shapes::new(owner, if present { &facts } else { &[] }, &mut work).unwrap();
            for _ in 0..2 {
                assert_eq!(
                    shapes.shared_carrier_pointee(ty(40), &mut work).unwrap(),
                    present.then_some(ty(30))
                );
                assert!(!shapes.contains(ty(40), &mut work).unwrap());
            }
        }
    }
}

#[test]
fn shared_cache121_pending_and_raw_errors_never_become_completed_negative_results() {
    let types = shared_graph();
    let facts = facts(&types);
    for index in [40, 43, 45] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
        if index == 40 {
            shapes
                .shared_memo
                .insert(ty(index), None, &mut work)
                .unwrap();
        }
        let shared = shapes.shared_memo.words.clone();
        assert!(matches!(
            shapes.shared_carrier_pointee(ty(index), &mut work),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(shapes.shared_memo.words, shared);
        shared_poisoned_retries(&mut shapes);
    }
    for index in [2, u32::MAX] {
        for limit in 0..=2 {
            let mut shapes = Shapes::new(&types, &facts, &mut budget(MAX_FLOW_WORK)).unwrap();
            let raw = shapes.memo.words.clone();
            let shared = shapes.shared_memo.words.clone();
            let mut work = budget(limit);
            let result = shapes.shared_carrier_pointee(ty(index), &mut work);
            assert_eq!(work.remaining, 0);
            assert_eq!(shapes.memo.words, raw);
            assert_eq!(shapes.shared_memo.words, shared);
            if limit == 2 {
                assert_eq!(result.unwrap(), None);
            } else {
                assert_denied_charge(result.unwrap_err(), limit, 0, 1);
                shared_poisoned_retries(&mut shapes);
            }
        }
    }
}

#[path = "declarations_tests.rs"]
mod declarations_tests;
