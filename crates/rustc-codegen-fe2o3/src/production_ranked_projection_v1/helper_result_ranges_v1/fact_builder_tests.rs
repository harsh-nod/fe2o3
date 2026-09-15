use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantV1, SemanticConstantValueV1, SemanticScalarValueV1,
};

pub(super) fn operands(count: usize) -> Vec<SemanticOperandV1> {
    (0..count)
        .map(|index| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(0),
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(index as u128, 8).unwrap(),
                ),
            ))
        })
        .collect()
}

pub(super) fn populated(
    operands: &[SemanticOperandV1],
    limit: usize,
) -> (FactBuilderV1, WorkingCells, usize) {
    let mut cells = WorkingCells::new(limit);
    let mut work = 0;
    let mut builder = FactBuilderV1::new(&mut cells, &mut work).unwrap();
    for (index, operand) in operands.iter().enumerate().rev() {
        builder
            .insert(
                operand,
                (index % 3, index),
                UnsignedRangeProofV1::exact(index as u128),
                &mut cells,
                &mut work,
            )
            .unwrap();
    }
    (builder, cells, work)
}

#[test]
fn fact_builder41_last_observation_matches_hashmap_without_changing_source_keys() {
    let operands = operands(65);
    for count in 0..=operands.len() {
        let mut cells = WorkingCells::new(MAX_CELLS);
        let mut work = 0;
        let mut builder = FactBuilderV1::new(&mut cells, &mut work).unwrap();
        let mut expected = HashMap::new();
        for round in 0..3 {
            for index in (0..count).rev() {
                let operand = &operands[(index + round) % count];
                let site = (round, index);
                let range = UnsignedRangeProofV1::exact((round * count + index) as u128);
                builder
                    .insert(operand, site, range, &mut cells, &mut work)
                    .unwrap();
                expected.insert(operand as *const _, (site, range));
                assert_eq!(cells.live, builder.retained_cells().unwrap());
            }
        }
        let allocation = (builder.records.as_ptr(), builder.records.capacity());
        let facts = builder.finish(cells, &mut work).unwrap();
        assert_eq!(
            (facts.records.as_ptr(), facts.records.capacity()),
            allocation
        );
        assert_eq!(facts.records.len(), expected.len());
        assert!(
            facts
                .records
                .windows(2)
                .all(|pair| pair[0].operand < pair[1].operand)
        );
        for operand in &operands[..count] {
            let &(site, range) = &expected[&(operand as *const _)];
            assert_eq!(
                facts.at(operand, site.0, site.1, &mut work).unwrap(),
                Some(range)
            );
            assert_eq!(
                facts.at(operand, site.0 + 1, site.1, &mut work).unwrap(),
                None
            );
            assert_eq!(
                facts.at(operand, site.0, site.1 + 1, &mut work).unwrap(),
                None
            );
            assert_eq!(
                facts
                    .at(&operand.clone(), site.0, site.1, &mut work)
                    .unwrap(),
                None
            );
        }
        assert_eq!(
            facts.cells.live,
            BUILDER_CELLS + RETAINED_CELLS + allocation.1 * RECORD_CELLS
        );
    }
}

#[test]
fn fact_builder41_growth_charges_both_capacities_and_releases_only_old_owner() {
    let operands = operands(3);
    let (mut builder, mut cells, mut work) = populated(&operands[..2], MAX_CELLS);
    cells.reserve(9).unwrap();
    let inherited = cells.live;
    let old_capacity = builder.records.capacity();
    let scope = std::mem::size_of::<working_scope_v1::WorkingScopeV1<'_>>()
        .div_ceil(std::mem::size_of::<usize>());
    builder
        .insert(
            &operands[2],
            (1, 2),
            UnsignedRangeProofV1::exact(2),
            &mut cells,
            &mut work,
        )
        .unwrap();
    assert!(builder.records.capacity() > old_capacity);
    assert_eq!(
        cells.peak,
        inherited + scope + BUILDER_CELLS + builder.records.capacity() * RECORD_CELLS
    );
    assert_eq!(cells.live, 9 + builder.retained_cells().unwrap());
    let peak = cells.peak;
    let (mut exact_builder, mut exact, mut exact_work) = populated(&operands[..2], peak);
    exact.reserve(9).unwrap();
    exact_builder
        .insert(
            &operands[2],
            (1, 2),
            UnsignedRangeProofV1::exact(2),
            &mut exact,
            &mut exact_work,
        )
        .unwrap();
    assert_eq!(exact.peak, peak);
    let (mut short_builder, mut short, mut short_work) = populated(&operands[..2], peak - 1);
    short.reserve(9).unwrap();
    let before = short_builder.records.clone();
    let allocation = (
        short_builder.records.as_ptr(),
        short_builder.records.capacity(),
    );
    assert!(matches!(
        short_builder.insert(
            &operands[2],
            (1, 2),
            UnsignedRangeProofV1::exact(2),
            &mut short,
            &mut short_work
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            STORAGE_LIMIT
        ))
    ));
    assert_eq!(short.live, inherited);
    assert_eq!(short_builder.records, before);
    assert_eq!(
        (
            short_builder.records.as_ptr(),
            short_builder.records.capacity()
        ),
        allocation
    );
}

#[test]
fn fact_builder41_each_exhausted_growth_step_preserves_builder_and_live_reservation() {
    let operands = operands(3);
    let (mut builder, mut cells, mut work) = populated(&operands[..2], MAX_CELLS);
    let before = work;
    builder
        .insert(
            &operands[2],
            (0, 2),
            UnsignedRangeProofV1::exact(2),
            &mut cells,
            &mut work,
        )
        .unwrap();
    let delta = work - before;
    for remaining in 0..=delta {
        let (mut builder, mut cells, _) = populated(&operands[..2], MAX_CELLS);
        let original = builder.records.clone();
        let live = cells.live;
        let allocation = (builder.records.as_ptr(), builder.records.capacity());
        let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - remaining;
        let result = builder.insert(
            &operands[2],
            (0, 2),
            UnsignedRangeProofV1::exact(2),
            &mut cells,
            &mut work,
        );
        if remaining == delta {
            result.unwrap();
            assert_eq!(work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform induction CFG analysis exceeds its work limit"
                ))
            ));
            assert_eq!(cells.live, live);
            assert_eq!(builder.records, original);
            assert_eq!(
                (builder.records.as_ptr(), builder.records.capacity()),
                allocation
            );
            assert!(work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        }
    }
}

#[test]
fn fact_builder41_finalization_transfers_allocation_with_exact_work_boundary() {
    let operands = operands(37);
    let (builder, cells, mut work) = populated(&operands, MAX_CELLS);
    let before = work;
    let allocation = (builder.records.as_ptr(), builder.records.capacity());
    let facts = builder.finish(cells, &mut work).unwrap();
    let delta = work - before;
    assert_eq!(
        (facts.records.as_ptr(), facts.records.capacity()),
        allocation
    );
    for remaining in [delta, delta - 1] {
        let (builder, cells, _) = populated(&operands, MAX_CELLS);
        let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - remaining;
        let result = builder.finish(cells, &mut work);
        if remaining == delta {
            assert_eq!(result.unwrap().records, facts.records);
            assert_eq!(work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform induction CFG analysis exceeds its work limit"
                ))
            ));
        }
    }
}

#[test]
fn fact_builder41_empty_transfer_and_foreign_live_owner_are_checked() {
    let (builder, mut cells, mut work) = populated(&[], MAX_CELLS);
    cells.limit = cells.live + RETAINED_CELLS;
    let expected_peak = cells.limit;
    let facts = builder.finish(cells, &mut work).unwrap();
    assert_eq!(facts.records.capacity(), 0);
    assert_eq!(facts.cells.live, BUILDER_CELLS + RETAINED_CELLS);
    assert_eq!(facts.cells.peak, expected_peak);
    let (builder, mut cells, mut work) = populated(&[], MAX_CELLS);
    cells.limit = cells.live + RETAINED_CELLS - 1;
    assert!(matches!(
        builder.finish(cells, &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            STORAGE_LIMIT
        ))
    ));
    let (builder, mut cells, mut work) = populated(&[], MAX_CELLS);
    cells.reserve(1).unwrap();
    assert!(matches!(
        builder.finish(cells, &mut work),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "helper range builder still owns non-fact storage"
        ))
    ));
}

#[test]
fn fact_builder41_large_unique_corpus_keeps_existing_caps_and_indexed_lookup() {
    let operands = operands(4096);
    let (builder, cells, mut work) = populated(&operands, MAX_CELLS);
    let facts = builder.finish(cells, &mut work).unwrap();
    assert_eq!(facts.records.len(), operands.len());
    assert!(facts.cells.peak <= MAX_CELLS);
    assert!(work <= MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    for (index, operand) in operands.iter().enumerate() {
        let mut query = 0;
        assert_eq!(
            facts.at(operand, index % 3, index, &mut query).unwrap(),
            Some(UnsignedRangeProofV1::exact(index as u128))
        );
        assert!(query <= operands.len().ilog2() as usize + 1);
    }
    eprintln!(
        "BUILDER count={} record_cells={} owner_cells={} peak={} retained={} work={}",
        operands.len(),
        RECORD_CELLS,
        RETAINED_CELLS,
        facts.cells.peak,
        facts.cells.live,
        work
    );
}
