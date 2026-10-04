use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, ControlFlowLimits, Signature, Terminator,
    Type, ValueId, VerificationFunctionStateV1,
};
use std::cell::Cell;

type Row = (u32, u8);

fn inversion_cases() -> [([Row; 5], usize); 4] {
    [
        ([(u32::MAX, 0), (7, 1), (0, 2), (7, 3), (1 << 31, 4)], 1),
        ([(0, 0), (u32::MAX, 1), (7, 2), (7, 3), (1 << 31, 4)], 2),
        ([(0, 0), (7, 1), (u32::MAX, 2), (7, 3), (1 << 31, 4)], 3),
        ([(0, 0), (7, 1), (7, 2), (u32::MAX, 3), (1 << 31, 4)], 4),
    ]
}

#[test]
fn trivial_radix_inputs_observe_no_keys_and_need_no_budget() {
    for mut rows in [vec![], vec![(u32::MAX, 7_u8)]] {
        let original = rows.clone();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        verification_radix_sort_u32_by_key_v1(&mut rows, usize::MAX, &mut budget, |_| {
            panic!("trivial input must not observe a key")
        })
        .unwrap();
        assert_eq!(rows, original);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn ordered_sparse_and_equal_keys_have_literal_work_without_scratch() {
    const COUNT: usize = 5;
    const WORK_FLOOR: usize = 11;
    const STORAGE_FLOOR: usize = 7;
    // One initial key read and four key-read/comparison pairs.
    const SCAN_WORK: usize = 1 + 2 * 4;
    for original in [
        [(0, 0), (7, 1), (7, 2), (1 << 31, 3), (u32::MAX, 4)],
        [
            (u32::MAX, 0),
            (u32::MAX, 1),
            (u32::MAX, 2),
            (u32::MAX, 3),
            (u32::MAX, 4),
        ],
    ] {
        let mut rows = original;
        let calls = Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_FLOOR + SCAN_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE_FLOOR);
        budget.charge_work(WORK_FLOOR).unwrap();
        budget.reserve_storage(STORAGE_FLOOR).unwrap();
        verification_radix_sort_u32_by_key_v1(&mut rows, 3, &mut budget, |row| {
            calls.set(calls.get() + 1);
            row.0
        })
        .unwrap();
        assert_eq!(rows, original);
        assert_eq!(calls.get(), COUNT);
        assert_eq!(budget.work(), WORK_FLOOR + SCAN_WORK);
        assert_eq!(budget.storage(), STORAGE_FLOOR);
        assert_eq!(budget.peak_storage(), STORAGE_FLOOR);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn every_denied_ordered_scan_prefix_stops_before_the_next_key_read() {
    let original = [(0, 0_u8), (7, 1), (7, 2), (1 << 31, 3), (u32::MAX, 4)];
    // (limit, accepted work, observed keys, rejected total).
    for (limit, accepted, observed, rejected) in [
        (0, 0, 0, 1),
        (1, 1, 1, 3),
        (2, 1, 1, 3),
        (3, 3, 2, 5),
        (4, 3, 2, 5),
        (5, 5, 3, 7),
        (6, 5, 3, 7),
        (7, 7, 4, 9),
        (8, 7, 4, 9),
    ] {
        let mut rows = original;
        let calls = Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result = verification_radix_sort_u32_by_key_v1(&mut rows, 3, &mut budget, |row| {
            calls.set(calls.get() + 1);
            row.0
        });
        assert!(matches!(
            result,
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == rejected && error.limit() == limit
        ));
        assert_eq!(rows, original);
        assert_eq!(calls.get(), observed);
        assert_eq!(budget.work(), accepted);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
        assert_eq!(work.failed_work(), Some(rejected));
    }
}

#[test]
fn inversion_positions_pay_the_scan_then_the_unchanged_stable_radix() {
    const COUNT: usize = 5;
    const FLOOR: usize = 11;
    const SCRATCH: usize = 3 * COUNT;
    // Initial copy plus four histogram/scatter/copy passes and bucket work.
    const RADIX_WORK: usize = COUNT + 4 * (3 * COUNT + 2 * 256);
    for (original, inversion) in inversion_cases() {
        let scan = 1 + 2 * inversion;
        let exact = scan + RADIX_WORK;
        let mut expected = original;
        expected.sort_by_key(|row| row.0);
        let mut rows = original;
        let calls = Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + SCRATCH);
        budget.reserve_storage(FLOOR).unwrap();
        verification_radix_sort_u32_by_key_v1(&mut rows, 3, &mut budget, |row| {
            calls.set(calls.get() + 1);
            row.0
        })
        .unwrap();
        assert_eq!(rows, expected);
        assert_eq!(calls.get(), inversion + 1 + 8 * COUNT);
        assert_eq!(budget.work(), exact);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + SCRATCH);
    }
}

#[test]
fn fallback_work_and_storage_denial_never_mutate_the_input() {
    const COUNT: usize = 5;
    const FLOOR: usize = 11;
    const SCRATCH: usize = 3 * COUNT;
    const RADIX_WORK: usize = COUNT + 4 * (3 * COUNT + 2 * 256);
    for (original, inversion) in inversion_cases() {
        let scan = 1 + 2 * inversion;
        let exact = scan + RADIX_WORK;
        for storage_failure in [false, true] {
            let work_limit = if storage_failure { exact } else { exact - 1 };
            let storage_limit = FLOOR + SCRATCH - usize::from(storage_failure);
            let mut rows = original;
            let calls = Cell::new(0);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = verification_radix_sort_u32_by_key_v1(&mut rows, 3, &mut budget, |row| {
                calls.set(calls.get() + 1);
                row.0
            });
            if storage_failure {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == FLOOR + SCRATCH && error.limit() == storage_limit
                ));
                assert_eq!(budget.work(), exact);
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == exact && error.limit() == work_limit
                ));
                assert_eq!(budget.work(), scan);
            }
            assert_eq!(rows, original);
            assert_eq!(calls.get(), inversion + 1);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR);
        }
    }
}

#[test]
fn unused_scratch_arithmetic_is_skipped_only_after_an_ordered_proof() {
    for ordered in [true, false] {
        let original = if ordered {
            [(0, 0_u8), (u32::MAX, 1)]
        } else {
            [(u32::MAX, 0_u8), (0, 1)]
        };
        let mut rows = original;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result =
            verification_radix_sort_u32_by_key_v1(&mut rows, usize::MAX, &mut budget, |row| row.0);
        if ordered {
            assert_eq!(result, Ok(()));
        } else {
            assert!(matches!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
            ));
        }
        assert_eq!(rows, original);
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn bounded_key_corpus_matches_a_stable_sort_oracle() {
    const KEYS: [u32; 3] = [0, 7, u32::MAX];
    const COUNT: usize = 4;
    const RADIX_WORK: usize = COUNT + 4 * (3 * COUNT + 2 * 256);
    for encoded in 0..81 {
        let mut digits = encoded;
        let original: [Row; COUNT] = std::array::from_fn(|ordinal| {
            let key = KEYS[digits % KEYS.len()];
            digits /= KEYS.len();
            (key, ordinal as u8)
        });
        let inversion = original.windows(2).position(|pair| pair[0].0 > pair[1].0);
        let (exact_work, scratch) = match inversion {
            Some(pair) => (1 + 2 * (pair + 1) + RADIX_WORK, 3 * COUNT),
            None => (1 + 2 * (COUNT - 1), 0),
        };
        let mut expected = original;
        expected.sort_by_key(|row| row.0);
        let mut rows = original;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, scratch);
        verification_radix_sort_u32_by_key_v1(&mut rows, 3, &mut budget, |row| row.0).unwrap();
        assert_eq!(rows, expected, "encoded={encoded}");
        assert_eq!(budget.work(), exact_work);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), scratch);
    }
}

#[test]
fn ordered_definition_index_keeps_only_retained_rows_and_exact_boundaries() {
    const FLOOR: usize = 7;
    // Block fill1 + B/O census3 + definition fill4 + mode census/selector15
    // + ordered scan7 + terminal1. Sparse keys retain no dense table.
    const WORK: usize = 1 + 3 + 4 + (3 * 4 - 2) + 5 + 7 + 1;
    const RETAINED: usize = 3 + 4 * 6;
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "ordered",
        Signature::new(vec![Type::INDEX; 4], vec![]),
        vec![ValueId(0), ValueId(7), ValueId(1 << 31), ValueId(u32::MAX)],
        vec![block],
    );
    for (work_limit, storage_limit) in [
        (WORK, FLOOR + RETAINED),
        (WORK - 1, FLOOR + RETAINED),
        (WORK, FLOOR + RETAINED - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = VerificationFunctionStateV1::build(&function, &mut budget);
        if work_limit < WORK {
            assert!(matches!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                    if error.actual() == WORK && error.limit() == WORK - 1
            ));
            assert_eq!(budget.work(), WORK - 1);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + RETAINED);
        } else if storage_limit < FLOOR + RETAINED {
            assert!(matches!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                    if error.actual() == FLOOR + RETAINED && error.limit() == storage_limit
            ));
            assert_eq!(budget.work(), 8);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + 3);
        } else {
            let state = result.unwrap().unwrap();
            assert_eq!(
                state
                    .definition_rows()
                    .iter()
                    .map(|row| row.key)
                    .collect::<Vec<_>>(),
                [0, 7, 1 << 31, u32::MAX]
            );
            assert_eq!(budget.work(), WORK);
            assert_eq!(budget.storage(), FLOOR + RETAINED);
            assert_eq!(budget.peak_storage(), FLOOR + RETAINED);
            state.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn ordered_cfg_block_index_preserves_dominance_and_exact_resources() {
    const BLOCKS: usize = 2;
    // Existing index/reachability/RPO/idom/interval/reducibility phases,
    // replacing only the radix term with the three-unit ordered proof.
    const CFG_WORK: usize = (36 * BLOCKS - 7)
        + 3
        + (6 * BLOCKS + 1)
        + (13 * BLOCKS + 1)
        + (17 * BLOCKS - 6)
        + (18 * BLOCKS + 6)
        + (24 * BLOCKS - 4);
    const QUERY: usize = 2 * (1 + 1) + 4;
    const RETAINED: usize = 18 * BLOCKS - 7;
    const PEAK: usize = 24 * BLOCKS - 8;
    let mut first = BasicBlock::new(BlockId(0));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(1));
    last.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "ordered_cfg",
        Signature::new(vec![], vec![]),
        vec![],
        vec![first, last],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CFG_WORK + QUERY);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, PEAK);
    let flow = crate::analyze_control_flow_with_verification_budget_v1(
        &function,
        ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    assert_eq!(CFG_WORK, 222);
    assert_eq!(budget.work(), CFG_WORK);
    assert_eq!(budget.storage(), RETAINED);
    assert_eq!(budget.peak_storage(), PEAK);
    assert!(flow.dominates(BlockId(0), BlockId(1), &mut budget).unwrap());
    assert_eq!(budget.work(), CFG_WORK + QUERY);
    flow.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}
