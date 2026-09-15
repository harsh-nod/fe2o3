use super::tests::{operands, populated};
use super::*;

#[test]
fn helper_retained_facts_keep_exact_keys_sites_values_and_capacity_owner() {
    let operands = operands(37);
    let (builder, cells, mut work) = populated(&operands, MAX_CELLS);
    let allocation = (builder.records.as_ptr(), builder.records.capacity());
    let facts = builder.finish(cells, &mut work).unwrap();
    assert_eq!(facts.records.len(), operands.len());
    assert_eq!(
        facts.cells.live,
        BUILDER_CELLS + RETAINED_CELLS + allocation.1 * RECORD_CELLS
    );
    for (index, operand) in operands.iter().enumerate() {
        assert_eq!(
            facts.at(operand, index % 3, index, &mut 0).unwrap(),
            Some(UnsignedRangeProofV1::exact(index as u128))
        );
        assert_eq!(
            facts.at(operand, index % 3 + 1, index, &mut 0).unwrap(),
            None
        );
        assert_eq!(
            facts.at(operand, index % 3, index + 1, &mut 0).unwrap(),
            None
        );
        assert_eq!(
            facts
                .at(&operand.clone(), index % 3, index, &mut 0)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        (facts.records.as_ptr(), facts.records.capacity()),
        allocation
    );
}

#[test]
fn helper_retained_facts_transfer_keeps_both_headers_without_second_payload() {
    let operands = operands(17);
    let (builder, mut cells, mut work) = populated(&operands, MAX_CELLS);
    let tail = builder.retained_cells().unwrap() + RETAINED_CELLS;
    let allocation = (builder.records.as_ptr(), builder.records.capacity());
    cells.limit = tail;
    let facts = builder.finish(cells, &mut work).unwrap();
    assert_eq!(facts.cells.live, tail);
    assert_eq!(
        (facts.records.as_ptr(), facts.records.capacity()),
        allocation
    );
    for limit in [tail - 1, tail - RETAINED_CELLS] {
        let (builder, mut cells, mut work) = populated(&operands, MAX_CELLS);
        cells.limit = limit;
        assert!(matches!(
            builder.finish(cells, &mut work),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                STORAGE_LIMIT
            ))
        ));
    }
}

#[test]
fn helper_retained_facts_reject_leftover_builder_owners_and_exhausted_work() {
    let operands = operands(5);
    let (builder, mut cells, mut work) = populated(&operands, MAX_CELLS);
    cells.reserve(1).unwrap();
    assert!(matches!(
        builder.finish(cells, &mut work),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "helper range builder still owns non-fact storage"
        ))
    ));
    let (builder, cells, _) = populated(&operands, MAX_CELLS);
    let mut exhausted = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        builder.finish(cells, &mut exhausted),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    let (builder, cells, mut work) = populated(&operands, MAX_CELLS);
    let facts = builder.finish(cells, &mut work).unwrap();
    let mut used = 0;
    facts.at(&operands[0], 0, 0, &mut used).unwrap();
    assert!(used > 0);
    let mut exact = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - used;
    facts.at(&operands[0], 0, 0, &mut exact).unwrap();
    assert_eq!(exact, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert!(facts.at(&operands[0], 0, 0, &mut exact).is_err());
}

#[test]
fn helper_retained_facts_sort_matches_standard_order_with_exact_work_debits() {
    let operands = operands(65);
    for count in 0..=operands.len() {
        for shift in 0..count.max(1) {
            let mut records = operands[..count]
                .iter()
                .enumerate()
                .map(|(index, operand)| Record {
                    operand,
                    block: index,
                    statement: 0,
                    ordinal: index,
                    range: UnsignedRangeProofV1::exact(index as u128),
                })
                .collect::<Vec<_>>();
            let mut expected = records.clone();
            expected.sort_by_key(Record::key);
            if count != 0 {
                records.rotate_left(shift);
            }
            records.reverse();
            let original = records.clone();
            let mut used = 0;
            sort_records(&mut records, &mut used).unwrap();
            assert_eq!(records, expected);
            let mut exact = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - used;
            let mut replay = original.clone();
            sort_records(&mut replay, &mut exact).unwrap();
            assert_eq!(exact, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
            assert_eq!(replay, expected);
            if used != 0 {
                let mut short = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - used + 1;
                assert!(sort_records(&mut original.clone(), &mut short).is_err());
            }
        }
    }
}
