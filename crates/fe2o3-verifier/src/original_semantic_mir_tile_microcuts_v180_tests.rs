use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const LIMIT: usize = 128 * 1024 * 1024;

fn run(
    work: usize,
    storage: usize,
    body: impl FnOnce(&mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        budget.reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)?;
        let mut out = Writer::new(&mut budget)?;
        body(&mut out)
    })();
    (result, budget.work(), budget.peak_storage())
}

fn cut(edges: Range<usize>) -> Cut {
    Cut {
        instance: 0,
        block: Block::from_index(0),
        candidates: 0..0,
        edges,
    }
}

#[test]
fn tile_microcuts_intersection_uses_exact_ordered_boundaries_not_bounding_ranges() {
    run(LIMIT, LIMIT, |out| {
        let first = Cursor {
            block: 4,
            operation: Some(20),
            prefix: 0,
        };
        let interior = Cursor {
            block: 4,
            operation: Some(21),
            prefix: 1,
        };
        let end = Cursor {
            block: 4,
            operation: None,
            prefix: 2,
        };
        assert!(!intersects(&[first, end], &[interior], out)?);
        assert!(intersects(&[first, end, end], &[end], out)?);
        assert!(matches!(
            intersects(
                &[first],
                &[Cursor {
                    operation: Some(19),
                    ..first
                }],
                out
            ),
            Err(Error::Statement(
                "expanded micro-cut differs from its original source or target owner"
            ))
        ));
        assert!(!intersects(&[], &[first], out)?);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn tile_microcuts_zero_rank_handles_duplicate_edges_and_nonphysical_order() {
    run(LIMIT, LIMIT, |out| {
        let cuts = [cut(0..2), cut(2..2), cut(2..3)];
        let rank = ranks(&cuts, &[(0, 1), (0, 1), (2, 0)], out)?;
        assert!(rank[2] > rank[0] && rank[0] > rank[1]);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn tile_microcuts_zero_cycles_refuse_but_distinct_target_cursor_loops_do_not() {
    run(LIMIT, LIMIT, |out| {
        assert!(matches!(
            ranks(&[cut(0..1)], &[(0, 0)], out),
            Err(Error::Statement(
                "expanded source cuts contain a possible zero-target-step cycle"
            ))
        ));
        assert!(matches!(
            ranks(&[cut(0..1), cut(1..2)], &[(0, 1), (1, 0)], out),
            Err(Error::Statement(
                "expanded source cuts contain a possible zero-target-step cycle"
            ))
        ));
        let a = Cursor {
            block: 3,
            operation: Some(7),
            prefix: 0,
        };
        let b = Cursor {
            block: 3,
            operation: Some(8),
            prefix: 1,
        };
        assert!(!intersects(&[a], &[b], out)? && !intersects(&[b], &[a], out)?);
        assert_eq!(ranks(&[cut(0..0), cut(0..0)], &[], out)?.len(), 2);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn tile_microcuts_rank_has_exact_and_one_short_resource_boundaries() {
    let body = |out: &mut Writer<'_, '_>| {
        let cuts = [cut(0..1), cut(1..2), cut(2..2)];
        let rank = ranks(&cuts, &[(0, 1), (1, 2)], out)?;
        assert!(rank[0] > rank[1] && rank[1] > rank[2]);
        Ok(())
    };
    let baseline = run(LIMIT, LIMIT, body);
    baseline.0.unwrap();
    run(baseline.1, baseline.2, body).0.unwrap();
    assert!(matches!(run(baseline.1 - 1, baseline.2, body).0,
        Err(Error::Resource(Resource::Work(error))) if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(run(baseline.1, baseline.2 - 1, body).0,
        Err(Error::Resource(Resource::Storage(error))) if error.actual() == baseline.2 && error.limit() == baseline.2 - 1));
}
