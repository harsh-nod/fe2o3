//! Constructed artifacts exercise the encoder core; source View exercises the
//! full-stage method. Neither fabricates collector bindings or signed owners.
use super::*;
use fe2o3_kernel_opt::{
    CanonicalPolicy8HistoryRoleV1 as Role, materialize_policy8_history_inputs_v1,
    read_inert_policy8_history_v1,
};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 2_000_000_000;
const ROLES: [Role; 7] = [
    Role::B,
    Role::C,
    Role::S,
    Role::O,
    Role::I,
    Role::J,
    Role::K,
];

#[derive(Debug, Eq, PartialEq)]
struct Summary {
    bytes: usize,
    digest: [u8; 32],
    pairs: usize,
}

fn equal_bytes(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> Result8<()> {
    budget
        .charge_work(
            a.len()
                .checked_add(b.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    if a != b {
        return Err(execution_error("decoded history exact actual bytes"));
    }
    Ok(())
}

fn inspect(
    artifacts: &PreparedPolicy8ArtifactsV1,
    wire: &InertCanonicalPolicy8HistoryV1,
    budget: &mut Budget<'_>,
) -> Result8<Summary> {
    budget.charge_work(128).map_err(resource)?;
    if wire.authenticates_execution() || wire.grants_authority() {
        return Err(execution_error("exported history remains inert"));
    }
    let (b, p6) = artifacts.admitted.history().portable_prefix();
    let p5 = p6.intermediate_policy5();
    let p4 = p5.intermediate_policy4();
    let actual = [
        b,
        p4.intermediate_policy3().owner(),
        p4.owner(),
        p5.owner(),
        p6.owner(),
        artifacts.admitted.historical_j(),
        artifacts.output(),
    ];
    let frame = read_inert_policy8_history_v1(wire.canonical_bytes(), artifacts.output(), budget)
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::History(e)))?;
    budget
        .reserve_storage(frame.storage().retained_storage())
        .map_err(resource)?;
    if !std::ptr::eq(frame.external_output(), artifacts.output()) {
        return Err(execution_error("history frame keeps actual K borrow"));
    }
    equal_bytes(
        frame.policy5_record(),
        p5.execution().canonical_bytes(),
        budget,
    )?;
    equal_bytes(
        frame.integer_record(),
        p6.continuation().execution().canonical_bytes(),
        budget,
    )?;
    equal_bytes(
        frame.unvalidated_policy6_record(),
        p6.execution().canonical_bytes(),
        budget,
    )?;
    equal_bytes(
        frame.policy7_record(),
        artifacts.prefix_execution().canonical_bytes(),
        budget,
    )?;
    let inputs = materialize_policy8_history_inputs_v1(&frame, budget)
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::DecodedInputs(Box::new(e))))?;
    budget
        .reserve_storage(inputs.storage().retained_storage())
        .map_err(resource)?;
    for (ordinal, role) in ROLES.into_iter().enumerate() {
        equal_bytes(
            inputs.graph(role).canonical().canonical_bytes(),
            actual[ordinal].canonical().canonical_bytes(),
            budget,
        )?;
        // Sharing is checked within the newly admitted pool, not confused with
        // allocation identity of equal-byte independently decoded historical IR.
        for previous in ROLES[..ordinal].iter().copied() {
            budget.charge_work(2).map_err(resource)?;
            if frame.role(role).pool_index() == frame.role(previous).pool_index()
                && !std::ptr::eq(inputs.graph(role), inputs.graph(previous))
            {
                return Err(execution_error(
                    "one actual admitted owner for each role alias",
                ));
            }
        }
    }
    let checked = inputs
        .check_semantics(budget)
        .map_err(|e| export_error(Policy8HistoryExportErrorV1::DecodedSemantics(Box::new(e))))?;
    budget
        .reserve_storage(checked.storage().retained_storage())
        .map_err(resource)?;
    budget.charge_work(12).map_err(resource)?;
    if !std::ptr::eq(checked.output(), artifacts.output())
        || !std::ptr::eq(checked.continuation().input(), inputs.graph(Role::J))
        || !std::ptr::eq(
            checked
                .policy7_relation()
                .continuation()
                .relation()
                .output(),
            inputs.graph(Role::J),
        )
        || checked.continuation().proved_pairs() != artifacts.test_deletion_count_v1()
        || checked.continuation().has_substitutions() != (artifacts.test_deletion_count_v1() != 0)
        || checked.authenticates_execution()
        || checked.grants_authority()
        || inputs.proves_semantic_preservation()
        || inputs.authenticates_execution()
        || inputs.grants_authority()
    {
        return Err(execution_error("same-J/K full decoded semantic relation"));
    }
    let rows = checked
        .policy7_relation()
        .policy6_relation()
        .policy5_relation()
        .load_forwarding_rows();
    let expected = p5.load_forwarding_rows();
    budget
        .charge_work(
            rows.len()
                .checked_add(expected.len())
                .and_then(|n| n.checked_mul(6))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    if rows != expected {
        return Err(execution_error("actual complete P5 load rows"));
    }
    budget
        .charge_work(wire.canonical_bytes().len())
        .map_err(resource)?;
    Ok(Summary {
        bytes: wire.canonical_bytes().len(),
        digest: Sha256::digest(wire.canonical_bytes()).into(),
        pairs: checked.continuation().proved_pairs(),
    })
}

fn component(artifacts: &PreparedPolicy8ArtifactsV1, budget: &mut Budget<'_>) -> Result8<Summary> {
    export_scope(artifacts.retained_floor, budget, |budget| {
        let wire = export_scope(artifacts.retained_floor, budget, |budget| {
            encode_actual_history(artifacts, budget)
        })?;
        // Match the full-stage method's unreserved transfer before replay.
        budget
            .reserve_storage(wire.storage().retained_storage())
            .map_err(resource)?;
        inspect(artifacts, &wire, budget)
    })
}

/// Only called with the genuine invocation's full stage; no test constructor
/// for collector bindings or ranked verification is provided.
pub(crate) fn observe_stage(
    stage: &CheckedOutputTargetProductionCompilationPolicy8V1,
    budget: &mut Budget<'_>,
) -> Result8<()> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let work = budget.work();
    // Refusal is checked on the same live ledger and cannot count as success.
    let removed = floor
        .checked_sub(stage.retained_floor)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| resource(Resource::Accounting))?;
    budget.release_storage(removed).map_err(resource)?;
    let refused = stage.export_portable_history_v1(budget);
    let exact = matches!(
        &refused,
        Err(ProductionPipelineError::CheckedOutputPolicy8Stage(
            CheckedOutputPolicy8StageErrorV1::Resource(Resource::Accounting)
        ))
    );
    drop(refused);
    budget.reserve_storage(removed).map_err(resource)?;
    if !exact || budget.work() != work || budget.work_ledger_identity_v1() != ledger {
        return Err(execution_error(
            "full stage history floor refuses before work",
        ));
    }
    export_scope(stage.retained_floor, budget, |budget| {
        let wire = stage.export_portable_history_v1(budget)?;
        budget
            .reserve_storage(wire.storage().retained_storage())
            .map_err(resource)?;
        let checked = inspect(&stage.artifacts, &wire, budget)?;
        if checked.pairs == 0 {
            return Err(execution_error(
                "ordinary source history must preserve nonempty J/K mutation",
            ));
        }
        Ok(())
    })?;
    if budget.storage() != floor
        || budget.work() <= work
        || budget.work_ledger_identity_v1() != ledger
    {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}

/// Reused by two existing constructed fixture tests across both modes/profiles
/// and mutation/no-op. This is encoder-core coverage, NOT full-stage admission.
pub(crate) fn exercise(artifacts: &PreparedPolicy8ArtifactsV1, parent: &mut Budget<'_>) {
    let floor = parent.storage();
    let original = artifacts.original() as *const Graph;
    let history = artifacts.admitted.historical_j() as *const Graph;
    let expected = component(artifacts, parent).unwrap();
    assert_eq!(parent.storage(), floor);
    assert_eq!(artifacts.original() as *const Graph, original);
    assert_eq!(artifacts.admitted.historical_j() as *const Graph, history);
    let measure = |work_limit, storage_limit, prior_work, unrelated, prior_denial| {
        let mut work = Work::new(work_limit);
        let (result, used, peak, failed_storage) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor + unrelated).unwrap();
            budget.charge_work(prior_work).unwrap();
            if prior_denial {
                assert!(budget.charge_work(usize::MAX).is_err());
                assert!(budget.reserve_storage(usize::MAX).is_err());
            }
            let ledger = budget.work_ledger_identity_v1();
            let result = component(artifacts, &mut budget).map_err(|e| format!("{e:?}"));
            assert_eq!(budget.storage(), floor + unrelated);
            assert!(budget.work_ledger_identity_v1() == ledger);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            )
        };
        (result, used, peak, work.failed_work(), failed_storage)
    };
    let baseline = measure(WORK, STORAGE, 0, 0, false);
    assert_eq!(baseline.0.as_ref().unwrap(), &expected);
    assert_eq!(measure(baseline.1, baseline.2, 0, 0, false), baseline);
    let short_work = measure(baseline.1 - 1, baseline.2, 0, 0, false);
    assert!(short_work.0.is_err() && short_work.3.is_some());
    let short_storage = measure(baseline.1, baseline.2 - 1, 0, 0, false);
    assert!(short_storage.0.is_err() && short_storage.4.is_some());
    let cumulative = measure(baseline.1 + 17, baseline.2 + 31, 17, 31, false);
    assert_eq!(cumulative.0, baseline.0);
    assert_eq!(
        (cumulative.1, cumulative.2),
        (baseline.1 + 17, baseline.2 + 31)
    );
    assert!(
        measure(baseline.1 + 16, baseline.2 + 31, 17, 31, false)
            .0
            .is_err()
    );
    assert!(
        measure(baseline.1 + 17, baseline.2 + 30, 17, 31, false)
            .0
            .is_err()
    );
    let prior = measure(WORK, STORAGE, 0, 0, true);
    assert_eq!(prior.0, baseline.0);
    assert_eq!((prior.3, prior.4), (Some(usize::MAX), Some(usize::MAX)));
    hostile_wire(artifacts, parent);
    for panic in [false, true] {
        let result: Result8<()> = export_scope(floor, parent, |budget| {
            let _wire = encode_actual_history(artifacts, budget)?;
            if panic {
                panic!("actual encoded history failure cleanup");
            }
            Err(execution_error("actual encoded history error cleanup"))
        });
        assert!(result.is_err());
        assert_eq!(parent.storage(), floor);
    }
}

fn hostile_wire(artifacts: &PreparedPolicy8ArtifactsV1, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    export_scope(floor, budget, |budget| {
        let wire = encode_actual_history(artifacts, budget)?;
        let n = wire.canonical_bytes().len();
        budget.reserve_storage(n).map_err(resource)?;
        let mut bad = Vec::new();
        bad.try_reserve_exact(n)
            .map_err(|_| resource(Resource::Allocation))?;
        budget
            .reserve_storage(
                bad.capacity()
                    .checked_sub(n)
                    .ok_or_else(|| resource(Resource::Accounting))?,
            )
            .map_err(resource)?;
        budget.charge_work(n).map_err(resource)?;
        bad.extend_from_slice(wire.canonical_bytes());
        // Mutate the actual closed container header, then require frame refusal.
        bad[8] ^= 1;
        assert!(read_inert_policy8_history_v1(&bad, artifacts.output(), budget).is_err());
        if artifacts.test_deletion_count_v1() != 0 {
            assert!(
                read_inert_policy8_history_v1(
                    wire.canonical_bytes(),
                    artifacts.admitted.historical_j(),
                    budget
                )
                .is_err()
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

struct Mark(Arc<AtomicBool>);
impl Drop for Mark {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
struct BadDrop(Arc<AtomicBool>);
impl Drop for BadDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
        panic!("rejected history result");
    }
}

#[test]
fn history_scope_preserves_floor_work_and_partial_drop_on_all_exits() {
    for exit in 0..3 {
        let mut work = Work::new(9);
        let mut budget = Budget::new(&mut work, 53);
        budget.reserve_storage(13).unwrap();
        budget.charge_work(2).unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let result = export_scope(13, &mut budget, |budget| {
            let _owned = Mark(dropped.clone());
            budget.reserve_storage(40).map_err(resource)?;
            budget.charge_work(7).map_err(resource)?;
            match exit {
                0 => Ok(11),
                1 => Err(execution_error("closed test error")),
                _ => panic!("closed test panic"),
            }
        });
        assert_eq!(result.is_ok(), exit == 0);
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(
            (budget.storage(), budget.work(), budget.peak_storage()),
            (13, 9, 53)
        );
    }
}

#[test]
fn history_scope_rejects_foreign_work_and_undercut_without_refund() {
    let mut one = Work::new(100);
    let mut two = Work::new(100);
    let mut budget = Budget::new(&mut one, 100);
    let mut foreign = Budget::new(&mut two, 100);
    budget.reserve_storage(13).unwrap();
    foreign.reserve_storage(29).unwrap();
    let first = budget.work_ledger_identity_v1();
    let second = foreign.work_ledger_identity_v1();
    assert!(
        export_scope(13, &mut budget, |budget| {
            budget.reserve_storage(17).map_err(resource)?;
            std::mem::swap(budget, &mut foreign);
            Ok(())
        })
        .is_err()
    );
    assert!(
        budget.work_ledger_identity_v1() == second && foreign.work_ledger_identity_v1() == first
    );
    assert_eq!((budget.storage(), foreign.storage()), (29, 30));
    std::mem::swap(&mut budget, &mut foreign);
    budget.release_storage(17).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    assert!(
        export_scope(13, &mut budget, |budget| {
            budget.release_storage(1).map_err(resource)?;
            Ok(BadDrop(dropped.clone()))
        })
        .is_err()
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 12);
}

#[test]
fn history_scope_preserves_first_denials_and_rejects_short_entry_before_work() {
    let mut work = Work::new(100);
    {
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(13).unwrap();
        assert!(export_scope(14, &mut budget, |_| Ok(())).is_err());
        assert_eq!((budget.storage(), budget.work()), (13, 0));
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        export_scope(13, &mut budget, |budget| {
            budget.charge_work(7).map_err(resource)?;
            budget.reserve_storage(17).map_err(resource)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            (budget.storage(), budget.work(), budget.failed_storage()),
            (13, 7, Some(usize::MAX))
        );
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

struct HostilePayload(Arc<AtomicBool>);
impl Drop for HostilePayload {
    fn drop(&mut self) {
        assert!(self.0.load(Ordering::SeqCst));
        panic!("deferred history payload drop");
    }
}
#[test]
fn history_scope_defers_hostile_payload_until_partial_backing_and_floor_cleanup() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(13).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result8<()> = export_scope(13, &mut budget, |budget| {
            budget.reserve_storage(17).map_err(resource)?;
            let _owned = Mark(dropped.clone());
            std::panic::panic_any(HostilePayload(dropped.clone()))
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 13);
    assert!(dropped.load(Ordering::SeqCst));
}
