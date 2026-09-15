//! Invert retained loop membership without scanning every block/loop pair.

use super::{
    MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1, ProductionRankedProjectionErrorV1,
    ProjectedUniformInductionV1,
};

pub(super) fn build(
    block_count: usize,
    inductions: &[ProjectedUniformInductionV1],
) -> Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1> {
    from_memberships(
        block_count,
        inductions
            .iter()
            .map(|induction| induction.loop_blocks.as_slice()),
        MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1,
    )
}

fn from_memberships<'a>(
    block_count: usize,
    memberships: impl Iterator<Item = &'a [usize]> + Clone,
    limit: usize,
) -> Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1> {
    let storage = || {
        ProductionRankedProjectionErrorV1::Unsupported(
            "live induction table exceeds projected state limit",
        )
    };
    let allocation = || {
        ProductionRankedProjectionErrorV1::Unsupported(
            "live induction table storage cannot be reserved",
        )
    };
    // Counts coexist with all three-word row headers and every membership slot.
    // Source memberships are borrowed; no second payload or growable worklist exists.
    let mut cells = block_count
        .checked_mul(4)
        .filter(|&n| n <= limit)
        .ok_or_else(storage)?;
    let mut counts = Vec::new();
    counts
        .try_reserve_exact(block_count)
        .map_err(|_| allocation())?;
    counts.resize(block_count, 0usize);
    for blocks in memberships.clone() {
        cells = cells
            .checked_add(blocks.len())
            .filter(|&n| n <= limit)
            .ok_or_else(storage)?;
        let mut previous = None;
        for &block in blocks {
            if block >= block_count || previous.is_some_and(|p| p >= block) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "live induction membership is not a canonical source block set",
                ));
            }
            counts[block] += 1;
            previous = Some(block);
        }
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(block_count)
        .map_err(|_| allocation())?;
    for &count in &counts {
        let mut row = Vec::new();
        row.try_reserve_exact(count).map_err(|_| allocation())?;
        rows.push(row);
    }
    for (induction, blocks) in memberships.enumerate() {
        for &block in blocks {
            // Enumerating inductions preserves the original argument ordering.
            rows[block].push(induction);
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(
        blocks: usize,
        memberships: &[Vec<usize>],
        limit: usize,
    ) -> Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1> {
        from_memberships(blocks, memberships.iter().map(Vec::as_slice), limit)
    }

    #[test]
    fn live_induction_rows_preserve_nested_and_disjoint_argument_order() {
        let memberships = vec![vec![1, 2, 3, 4], vec![2, 3], vec![5], vec![]];
        assert_eq!(
            rows(7, &memberships, 35).unwrap(),
            vec![
                vec![],
                vec![0],
                vec![0, 1],
                vec![0, 1],
                vec![0],
                vec![2],
                vec![]
            ]
        );
    }

    #[test]
    fn live_induction_rows_match_independent_cartesian_reference() {
        for blocks in 0..=5 {
            for mask in 0usize..1 << (blocks * 3) {
                let memberships = (0..3)
                    .map(|i| {
                        (0..blocks)
                            .filter(|&b| mask & (1 << (i * blocks + b)) != 0)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let expected = (0..blocks)
                    .map(|b| {
                        memberships
                            .iter()
                            .enumerate()
                            .filter_map(|(i, members)| {
                                members.binary_search(&b).is_ok().then_some(i)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rows(blocks, &memberships, 100).unwrap(), expected);
            }
        }
    }

    #[test]
    fn live_induction_rows_bound_peak_headers_counts_and_memberships() {
        let memberships = vec![vec![0, 2], vec![2, 3], vec![0]];
        assert!(rows(4, &memberships, 21).is_ok());
        assert!(matches!(
            rows(4, &memberships, 20),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "live induction table exceeds projected state limit"
            ))
        ));
        assert!(rows(4, &[], 16).is_ok());
        assert!(rows(4, &[], 15).is_err());
        assert!(rows(usize::MAX, &[], usize::MAX).is_err());
        assert_eq!(rows(0, &[], 0).unwrap(), Vec::<Vec<usize>>::new());
    }

    #[test]
    fn live_induction_rows_reject_outside_duplicate_and_unsorted_memberships() {
        for blocks in [vec![3], vec![0, 0], vec![2, 1]] {
            assert!(matches!(
                rows(3, &[blocks], 100),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "live induction membership is not a canonical source block set"
                ))
            ));
        }
    }
}
