//! Synthetic DATA controls, never source admission or authentic authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 11;
#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Phase,
    origins: Vec<Option<u32>>,
    queue: Vec<usize>,
    head: usize,
    census: usize,
    work: usize,
    storage: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn probe(
    mut origins: Vec<Option<u32>>,
    edges: &[Vec<usize>],
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedExactOriginWorklistV1::new();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&mut origins, edges, "original conflict", &mut resources)?;
        assert!(pending.completed(&resources));
        match mode {
            0 => Ok(()),
            1 => Err(Error::Incomplete("callback")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match caught {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Incomplete("component panic"))
        }
    };
    assert_eq!(budget.storage(), FLOOR + owned);
    // Snapshotting below is synthetic test observation, outside candidate accounting.
    let observed = Probe {
        result,
        phase: pending.phase,
        origins,
        queue: pending.worklist.clone(),
        head: pending.head,
        census: pending.work,
        work: budget.work(),
        storage: budget.storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn original(
    mut origins: Vec<Option<u32>>,
    edges: &[Vec<usize>],
) -> (Vec<Option<u32>>, Result<()>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let result = propagate_exact_local_origins_with_resources_v1(
        &mut origins,
        edges,
        "original conflict",
        &mut Resources::new(&mut budget, &mut owned),
    );
    let work = budget.work();
    assert_eq!(budget.storage(), owned);
    // The unchanged return API has already dropped its local queue here.
    budget.release_storage(owned).unwrap();
    (origins, result, work, owned)
}
fn fixture() -> (Vec<Option<u32>>, Vec<Vec<usize>>) {
    (
        vec![Some(7), None, None, None],
        vec![vec![2, 1], vec![3], vec![3], vec![]],
    )
}
#[test]
fn exact_original_data_debits_and_fifo_successor_order() {
    let (values, edges) = fixture();
    let (expected, result, work, storage) = original(values.clone(), &edges);
    assert!(result.is_ok());
    let p = probe(values, &edges, LIMIT, LIMIT, 0);
    assert!(p.result.is_ok());
    assert_eq!(p.origins, expected);
    assert_eq!(p.queue, [0, 2, 1, 3]);
    assert_eq!(p.head, p.queue.len());
    assert_eq!(p.census, 4);
    assert_eq!(p.work, work + 32);
    assert_eq!(
        p.storage,
        FLOOR + storage + retained_origin_worklist_frame_v1::<u32>().unwrap()
    );
}
#[test]
fn cycles_duplicate_edges_and_equal_roots_match_original() {
    for (values, edges) in [
        (
            vec![Some(7), None, None],
            vec![vec![1, 1], vec![0, 2], vec![1]],
        ),
        (vec![Some(7), Some(7), None], vec![vec![2], vec![2], vec![]]),
        (vec![None, None], vec![vec![1], vec![0]]),
        (vec![], vec![]),
    ] {
        let (expected, result, work, storage) = original(values.clone(), &edges);
        assert!(result.is_ok());
        let p = probe(values, &edges, LIMIT, LIMIT, 0);
        assert!(p.result.is_ok());
        assert_eq!(p.origins, expected);
        assert_eq!(p.work, work + 32);
        assert_eq!(
            p.storage,
            FLOOR + storage + retained_origin_worklist_frame_v1::<u32>().unwrap()
        );
    }
}
#[test]
fn conflict_retains_partially_propagated_origins_and_fifo_queue() {
    let values = vec![Some(7), Some(8), None, None];
    let edges = vec![vec![2], vec![3], vec![3], vec![]];
    let (expected, result, work, storage) = original(values.clone(), &edges);
    assert!(matches!(
        result,
        Err(Error::Incomplete("original conflict"))
    ));
    let p = probe(values, &edges, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Incomplete("original conflict"))
    ));
    assert_eq!(p.phase, Phase::Terminal);
    assert_eq!(p.origins, expected);
    assert_eq!(p.queue, [0, 1, 2, 3]);
    assert_eq!(p.head, 3);
    assert_eq!(p.work, work + 32);
    assert_eq!(
        p.storage,
        FLOOR + storage + retained_origin_worklist_frame_v1::<u32>().unwrap()
    );
}
#[test]
fn length_error_preserves_original_header_and_precedes_queue_allocation() {
    let values = vec![Some(7)];
    let edges = vec![];
    let (expected, result, work, storage) = original(values.clone(), &edges);
    assert!(matches!(
        result,
        Err(Error::Unsupported(
            "local provenance tables have inconsistent lengths"
        ))
    ));
    let p = probe(values, &edges, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "local provenance tables have inconsistent lengths"
        ))
    ));
    assert_eq!(p.origins, expected);
    assert!(p.queue.is_empty());
    assert_eq!(p.work, work + 32);
    assert_eq!(
        p.storage,
        FLOOR + storage + retained_origin_worklist_frame_v1::<u32>().unwrap()
    );
}
#[test]
fn exact_and_one_short_resource_limits_keep_partial_queue() {
    let (values, edges) = fixture();
    let full = probe(values.clone(), &edges, LIMIT, LIMIT, 0);
    assert!(full.result.is_ok());
    assert!(
        probe(values.clone(), &edges, full.work, full.storage, 0)
            .result
            .is_ok()
    );
    let work = probe(values.clone(), &edges, full.work - 1, full.storage, 0);
    assert!(work.result.is_err() && work.failed_work.is_some());
    assert!(!work.queue.is_empty());
    let storage = probe(values, &edges, full.work, full.storage - 1, 0);
    assert!(storage.result.is_err() && storage.failed_storage.is_some());
    assert_eq!(storage.phase, Phase::Terminal);
}
#[test]
fn every_work_refusal_preserves_accepted_custody_and_partial_output() {
    let (values, edges) = fixture();
    let full = probe(values.clone(), &edges, LIMIT, LIMIT, 0);
    let mut partial = false;
    for limit in 0..full.work {
        let p = probe(values.clone(), &edges, limit, LIMIT, 0);
        assert!(p.result.is_err() && p.failed_work.is_some());
        assert_eq!(p.phase, Phase::Terminal);
        partial |= !p.queue.is_empty() && p.origins.iter().filter(|x| x.is_some()).count() > 1;
    }
    assert!(partial);
}
#[test]
fn completed_scratch_survives_callback_success_error_and_panic() {
    let (values, edges) = fixture();
    for mode in 0..3 {
        let p = probe(values.clone(), &edges, LIMIT, LIMIT, mode);
        assert_eq!(p.result.is_ok(), mode == 0);
        assert_eq!(p.phase, Phase::Complete);
        assert_eq!(p.queue, [0, 2, 1, 3]);
        assert!(p.failed_work.is_none() && p.failed_storage.is_none());
    }
}
#[test]
fn occupied_retry_wrong_ledger_and_sticky_denial_are_terminal() {
    let (mut values, edges) = fixture();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedExactOriginWorklistV1::new();
    pending
        .prepare_into(
            &mut values,
            &edges,
            "conflict",
            &mut Resources::new(&mut budget, &mut owned),
        )
        .unwrap();
    let mut other_work = Work::new(LIMIT);
    let mut other_budget = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(!pending.completed(&Resources::new(&mut other_budget, &mut other_owned)));
    assert_eq!(other_owned, 0);
    let before = (
        budget.work(),
        owned,
        pending.worklist.as_ptr(),
        pending.worklist.len(),
    );
    assert!(
        pending
            .prepare_into(
                &mut values,
                &edges,
                "conflict",
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            owned,
            pending.worklist.as_ptr(),
            pending.worklist.len()
        )
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(budget.charge_work(LIMIT).is_err());
    let before = (budget.work(), owned);
    let mut fresh = RetainedExactOriginWorklistV1::new();
    assert!(
        fresh
            .prepare_into(
                &mut values,
                &edges,
                "conflict",
                &mut Resources::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(before, (budget.work(), owned));
    drop(fresh);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn unmetered_refusal_and_generic_frame_preserve_non_static_copy_values() {
    let mut values = [Some(3u32), None];
    let edges = [vec![1], vec![]];
    let mut pending = RetainedExactOriginWorklistV1::new();
    assert!(
        pending
            .prepare_into(&mut values, &edges, "conflict", &mut Resources::unmetered())
            .is_err()
    );
    assert_eq!(values, [Some(3), None]);
    assert_eq!(pending.phase, Phase::Terminal);
    assert_eq!(
        retained_origin_worklist_frame_v1::<u32>().unwrap(),
        typed_rows::<u32>().unwrap().into_iter().sum::<usize>()
    );
    let local = 5u32;
    let mut borrowed = [Some(&local), None];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedExactOriginWorklistV1::new();
    pending
        .prepare_into(
            &mut borrowed,
            &edges,
            "conflict",
            &mut Resources::new(&mut budget, &mut owned),
        )
        .unwrap();
    assert_eq!(borrowed, [Some(&local), Some(&local)]);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn donor_fifo_and_admission_order_are_visible_without_sort_or_fallback() {
    let source = include_str!("bf16_nominal_retained_origin_worklist_v1.rs");
    let start = source.find("fn propagate_attached").unwrap();
    let end = start + source[start..].find("\n}\nconst FRAME_ROWS").unwrap();
    let body: String = source[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut position = 0;
    for needle in [
        "resources.reserve_storage(",
        "iforigins.len()!=edges.len()",
        "resources.reserve(&mutself.worklist,origins.len())?;",
        "for(local,origin)inorigins.iter().enumerate()",
        "whileletSome(&source)=self.worklist.get(self.head)",
        "self.head+=1;",
        "for&destinationin&edges[source]",
        "origins[destination]=Some(origin);",
        "resources.push(&mutself.worklist,destination)?;",
    ] {
        position += body[position..].find(needle).unwrap() + needle.len();
    }
    assert!(!body.contains("sort"));
    assert!(!body.contains("Budget::new("));
    assert!(!body.contains("unmetered("));
}

#[test]
fn unchanged_malformed_successor_panic_keeps_attached_queue() {
    let edges = [vec![1, 99], vec![]];
    let mut expected = vec![Some(7u32), None];
    let mut resources = Resources::unmetered();
    let original = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        propagate_exact_local_origins_with_resources_v1(
            &mut expected,
            &edges,
            "conflict",
            &mut resources,
        )
    }));
    assert!(original.is_err());
    drop(original);
    let p = probe(vec![Some(7), None], &edges, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Incomplete("component panic"))
    ));
    assert_eq!(p.phase, Phase::Terminal);
    assert_eq!(p.origins, expected);
    assert_eq!(p.queue, [0, 1]);
    assert_eq!(p.head, 1);
    assert!(p.failed_work.is_none() && p.failed_storage.is_none());
}
