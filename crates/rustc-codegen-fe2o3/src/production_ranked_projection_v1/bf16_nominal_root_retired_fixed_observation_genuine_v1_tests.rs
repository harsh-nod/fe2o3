//! Genuine retained-proof observation only; no F2 chronology or fixed-query oracle.
use super::*;
use crate::production_ranked_projection_v1::{
    assertion_analyzer_resources_v1::lazy_fixed_proof_owner_v1::{
        LazyFixedEventV1, LazyFixedProofOwnerV1, lazy_state_observer_frame_for_test_v1,
        retirement::{self, Snapshot},
    },
    bf16_nominal_source_preparation_v1::{
        NominalRootCfgSourceV1, with_nominal_root_cfg_preparation_v1,
    },
    canonical_assertion_facts_v1::with_nominal_canonical_facts_observation_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::{any::Any, cell::Cell, mem::size_of};
type Q<T> = std::result::Result<T, QueryError>;
type PanicPayload = Box<dyn Any + Send>;
type LazyState = (u8, usize, bool, usize, usize, usize);

#[path = "bf16_nominal_root_retired_fixed_observation_oracle_v1_tests.rs"]
mod oracle;

thread_local! {
    // None outside this observer; old F1 controls/calls never contribute.
    static FACTORY_FRAMES: Cell<Option<([usize; 5], usize)>> = const { Cell::new(None) };
}
pub(in crate::production_ranked_projection_v1) fn record_factory_frames_for_test_v1(
    rows: [usize; 5],
) {
    FACTORY_FRAMES.with(|cell| {
        if let Some((prior, phase)) = cell.get() {
            // Poison BEFORE any refusal can unwind and be caught by a test.
            cell.set(Some((prior, 2)));
            assert_eq!(phase, 0, "one authentic factory entry per run");
            assert_eq!(
                rows[..4].iter().try_fold(0usize, |a, b| a.checked_add(*b)),
                Some(rows[4])
            );
            cell.set(Some((rows, 1)));
        }
    });
}
fn start_frames() {
    FACTORY_FRAMES.with(|cell| {
        if let Some((prior, _)) = cell.get() {
            cell.set(Some((prior, 2)));
            panic!("genuine frame recording cannot replace an active scope");
        }
        cell.set(Some(([0; 5], 0)));
    });
}
fn finish_frames() -> [usize; 5] {
    FACTORY_FRAMES.with(|cell| {
        let (rows, phase) = cell.take().expect("active genuine factory frame recording");
        assert_eq!(phase, 1, "poisoned/incomplete recording cannot qualify");
        rows
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Empty,
    Prefix,
    Error,
    Panic,
    Reentry,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Namespace {
    function: usize,
    locals: usize,
    operations: usize,
    next_value: u32,
    next_argument: usize,
    index_address: usize,
    slice_address: usize,
    operations_address: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    namespace: Namespace,
    plan: oracle::Plan,
    visited: usize,
    lazy_state: LazyState,
    live: Snapshot,
}
#[derive(Clone, Copy, Debug, Default)]
struct Witness {
    reached: bool,
    same_panic: bool,
    retired_before_return: bool,
    protected_before_drop: bool,
    dropped_before_refund: bool,
    original_floor_restored: bool,
    frames: [usize; 5],
    before: [usize; 3],
    protected: [usize; 3],
    after: [usize; 3],
    owned: usize,
    observation: Option<Observation>,
}

fn projection(error: Error) -> QueryError {
    match error {
        Error::CanonicalAssertions(
            crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error)
        ) => QueryError::Resource(error),
        _ => QueryError::Unavailable("authentic retired-proof observation refused"),
    }
}
fn observe_namespace(
    view: &ActualRootArgumentInitializationV1<'_>,
    cfg: &NominalRootCfgSourceV1<'_>,
) -> Namespace {
    assert!(std::ptr::eq(view.function(), cfg.function()));
    let locals = view.function().locals().len();
    assert_eq!(view.index_arguments().len(), locals);
    assert_eq!(view.slice_arguments().len(), locals);
    // The caller prepays precisely both local-table comparisons before the loan.
    assert!(view.index_arguments().iter().all(Option::is_none));
    assert!(view.slice_arguments().iter().all(Option::is_none));
    assert_eq!(view.next_argument(), 1);
    assert!(view.arguments.initialized && view.arguments.later.vacant());
    Namespace {
        function: view.function() as *const _ as usize,
        locals,
        operations: view.entry_operations().len(),
        next_value: view.next_value(),
        next_argument: view.next_argument(),
        index_address: view.index_arguments().as_ptr() as usize,
        slice_address: view.slice_arguments().as_ptr() as usize,
        operations_address: view.entry_operations().as_ptr() as usize,
    }
}
fn check_pending(pending: &PendingActualRootPrefixIndicesV1, observed: Observation) {
    assert!(pending.started && !pending.completed);
    assert!(pending.indices.indices.is_empty() && pending.indices.index_fifo.is_empty());
    assert!(pending.arguments.initialized && pending.arguments.later.vacant());
    assert_eq!(
        pending.arguments.function,
        Some(observed.namespace.function)
    );
    assert_eq!(
        pending.arguments.next_argument,
        observed.namespace.next_argument
    );
    assert_eq!(
        pending.arguments.index_arguments.as_ptr() as usize,
        observed.namespace.index_address
    );
    assert_eq!(
        pending.arguments.slice_arguments.as_ptr() as usize,
        observed.namespace.slice_address
    );
    assert_eq!(
        pending.prefix.entry_operations.as_ptr() as usize,
        observed.namespace.operations_address
    );
    assert_eq!(
        pending.prefix.entry_operations.len(),
        observed.namespace.operations
    );
    assert_eq!(pending.prefix.next_value, observed.namespace.next_value);
    let retired = pending
        .retired_fixed_proof
        .as_ref()
        .expect("authentic pending owns retirement");
    assert_eq!(retired.snapshot(), observed.live);
    assert_eq!(
        retired.snapshot().side,
        [false; 6],
        "authentic source tables stay borrowed"
    );
}

fn inspect_prefix(
    view: &ActualRootArgumentInitializationV1<'_>,
    cfg: &NominalRootCfgSourceV1<'_>,
    lazy: &mut LazyFixedProofOwnerV1<'_, '_, '_>,
    plan: oracle::Plan,
    observed: &mut Option<Observation>,
) -> Result<Observation> {
    let namespace = observe_namespace(view, cfg);
    assert_eq!(lazy.state_for_test(), (0, 0, false, 0, 0, 0));
    let mut visited = 0;
    for row in &plan.rows[..plan.len] {
        assert_eq!(
            cfg.function()
                .blocks()
                .get(row.block)
                .map(|s| s as *const _ as usize),
            Some(row.source_address)
        );
        if row.event == oracle::Event::FixedOracleBoundary {
            *observed = Some(Observation {
                namespace,
                plan,
                visited,
                lazy_state: lazy.state_for_test(),
                live: retirement::live_snapshot(lazy),
            });
            return Err(Error::Incomplete(
                "genuine fixed-query oracle is not yet retained",
            ));
        }
        let result = lazy.visit(row.block);
        match row.event {
            oracle::Event::Other => assert_eq!(result?, LazyFixedEventV1::Other),
            oracle::Event::LiteralSkip => assert_eq!(result?, LazyFixedEventV1::LiteralSkip),
            oracle::Event::NonFixedBoundary => {
                assert_eq!(result?, LazyFixedEventV1::NonFixedBoundary)
            }
            oracle::Event::MalformedBounds => {
                let error = result.expect_err("original malformed-shape refusal");
                assert!(matches!(
                    &error,
                    Error::Incomplete(
                        "a Rust bounds check without the canonical success/unreachable shape"
                    )
                ));
                *observed = Some(Observation {
                    namespace,
                    plan,
                    visited,
                    lazy_state: lazy.state_for_test(),
                    live: retirement::live_snapshot(lazy),
                });
                return Err(error);
            }
            oracle::Event::FixedOracleBoundary => unreachable!(),
        }
        visited += 1;
        if row.event == oracle::Event::NonFixedBoundary {
            break;
        }
    }
    let result = Observation {
        namespace,
        plan,
        visited,
        lazy_state: lazy.state_for_test(),
        live: retirement::live_snapshot(lazy),
    };
    // This narrowed oracle never enters fixed activation. Do not invent coverage.
    assert_eq!(result.lazy_state.0, 0);
    assert_eq!(result.lazy_state.1, visited);
    assert!(!result.lazy_state.2);
    *observed = Some(result);
    Ok(result)
}

fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
    witness: &mut Witness,
) -> Q<Observation> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let identity = budget.work_ledger_identity_v1();
        let slot = budget as *const Budget<'_> as usize;
        let floor = budget.storage();
        let before = [budget.work(), budget.storage(), budget.peak_storage()];
        let header = observer_frame().map_err(projection)?;
        budget.charge_work(header)?;
        budget.reserve_storage(header)?;
        let mut owned = header;
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut observed = None;
        let mut same_panic = false;
        let mut frames = [0; 5];
        let mut retired_before_return = false;
        // This fixed payload's heap is explicitly in observer_frame. It is not
        // a source/kernel payload and never carries recipe or ledger authority.
        let mut panic_payload: Option<PanicPayload> = if mode == Mode::Panic {
            Some(Box::new([0x7265746972656431u64, 0x73616d6570616e69u64]))
        } else { None };
        let panic_address = panic_payload.as_ref().map(|p| p.as_ref() as *const (dyn Any + Send) as *const () as usize);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                budget, |checked, budget| {
                    with_nominal_root_cfg_preparation_v1(
                        owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                        budget, |cfg, budget| {
                            // Oracle is independent source DATA built before the
                            // exclusive proof loan, with work on the same Budget.
                            let plan = if mode == Mode::Prefix { oracle::plan(cfg, budget)? } else { oracle::Plan::empty() };
                            budget.charge_work(cfg.function().locals().len().checked_mul(2).ok_or(QueryError::Resource(Resource::Arithmetic))?)?;
                            budget.charge_work(plan.len)?;
                            with_nominal_canonical_facts_observation_v1(
                                owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                                budget, |facts| {
                                    with_nominal_recipe_resources_v1(facts, cfg.source_tables().rich(), &mut owned, |context| {
                                        start_frames();
                                        let immediate = catch_unwind(AssertUnwindSafe(|| {
                                            context.with_actual_root_retired_fixed_proof_observation_v1(
                                                checked, cfg, actual_inputs, &mut pending, |view, lazy| {
                                                    let result = inspect_prefix(&view, cfg, lazy, plan, &mut observed)?;
                                                    match mode {
                                                        Mode::Error => Err(Error::Incomplete("retirement observer callback refusal")),
                                                        Mode::Panic => resume_unwind(panic_payload.take().expect("one original panic payload")),
                                                        _ => Ok(result),
                                                    }
                                                },
                                            )
                                        }));
                                        frames = finish_frames();
                                        let current = observed.expect("authentic callback entered");
                                        check_pending(&pending, current);
                                        retired_before_return = true;
                                        match immediate {
                                            Ok(result) => {
                                                if mode == Mode::Reentry {
                                                    assert!(result.is_ok());
                                                    assert!(context.with_actual_root_retired_fixed_proof_observation_v1(
                                                        checked, cfg, actual_inputs, &mut pending,
                                                        |_, _| -> Result<()> { panic!("retired proof retry entered") },
                                                    ).is_err());
                                                    assert!(context.with_actual_root_argument_initialization_v1(
                                                        checked, cfg.source_tables().rich(), actual_inputs, &mut pending,
                                                        |_, _| -> Result<()> { panic!("F1 resumed retired owner") },
                                                    ).is_err());
                                                    assert!(context.with_actual_root_prefix_indices_v1(
                                                        checked, cfg.source_tables().rich(), actual_inputs, &mut pending,
                                                        |_, _| -> Result<()> { panic!("old assembly resumed retired owner") },
                                                    ).is_err());
                                                    check_pending(&pending, current);
                                                }
                                                result
                                            }
                                            Err(payload) => {
                                                assert_eq!(mode, Mode::Panic);
                                                assert_eq!(Some(payload.as_ref() as *const (dyn Any + Send) as *const () as usize), panic_address);
                                                assert_eq!(payload.downcast_ref::<[u64; 2]>(), Some(&[0x7265746972656431, 0x73616d6570616e69]));
                                                same_panic = true;
                                                // Preserve identical box and original outer catch priority.
                                                resume_unwind(payload)
                                            }
                                        }
                                    }).map_err(projection)
                                },
                            )
                        },
                    )
                },
            )
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) },
        };
        let current = observed.expect("genuine authentic retirement observation reached");
        check_pending(&pending, current);
        let expected_phase = if mode == Mode::Reentry || result.is_err() {
            Phase::Terminal
        } else {
            Phase::InitializedBeforeArgumentWriters
        };
        assert_eq!(pending.arguments.phase, expected_phase);
        let protected = floor.checked_add(owned).ok_or(QueryError::Resource(Resource::Arithmetic))?;
        witness.reached = true;
        witness.same_panic = same_panic;
        witness.retired_before_return = retired_before_return;
        witness.frames = frames;
        witness.before = before;
        witness.protected = [budget.work(), budget.storage(), budget.peak_storage()];
        witness.owned = owned;
        witness.observation = Some(current);
        if slot != budget as *const Budget<'_> as usize || identity != budget.work_ledger_identity_v1()
            || budget.storage() < protected || budget.work() < before[0] || budget.peak_storage() < before[2]
            || budget.failed_work().is_some() || budget.failed_storage().is_some()
        {
            drop(pending);
            return Err(QueryError::Resource(Resource::Accounting));
        }
        witness.protected_before_drop = true;
        drop(pending);
        drop(panic_payload);
        witness.dropped_before_refund = true;
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), floor);
        witness.original_floor_restored = true;
        witness.after = [budget.work(), budget.storage(), budget.peak_storage()];
        result
    })
}

pub(crate) fn observe_actual_root_retired_fixed_proof_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    // Direct original Budget observation includes entry prework OUTSIDE each
    // run's inner owner callback, not a residual reconstructed from call counts.
    let before_work = budget.work();
    for mode in [
        Mode::Empty,
        Mode::Error,
        Mode::Panic,
        Mode::Reentry,
        Mode::Prefix,
    ] {
        let mut witness = Witness::default();
        let result = run(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            mode,
            &mut witness,
        );
        assert!(
            witness.reached
                && witness.retired_before_return
                && witness.protected_before_drop
                && witness.dropped_before_refund
                && witness.original_floor_restored
        );
        let observed = witness.observation.unwrap();
        match mode {
            Mode::Error => assert!(matches!(result, Err(QueryError::Unavailable(_)))),
            Mode::Panic => {
                assert!(witness.same_panic);
                assert!(matches!(result, Err(QueryError::CallbackPanicked)));
            }
            Mode::Prefix
                if matches!(
                    observed.plan.stop,
                    oracle::Stop::MalformedBounds | oracle::Stop::FixedOracleBoundary
                ) =>
            {
                assert!(matches!(result, Err(QueryError::Unavailable(_))));
            }
            _ => {
                result?;
            }
        }
        assert_eq!(observed.live.checked.1, 0);
        assert_eq!(observed.live.checked.2, 0);
        assert!(observed.live.dominance.is_none() && observed.live.zero.is_none());
        assert_eq!(observed.live.side, [false; 6]);
        let stop_block = observed.plan.rows[..observed.plan.len]
            .last()
            .map(|r| r.block);
        eprintln!(
            "fe2o3-retired-fixed-observation-v1 mode={mode:?} locals={} visited={} stop={:?} stop_block={stop_block:?} fixed_query_coverage=false same_panic={} custody=pass frames={:?} before={:?} protected={:?} after={:?} owned={}",
            observed.namespace.locals,
            observed.visited,
            observed.plan.stop,
            witness.same_panic,
            witness.frames,
            witness.before,
            witness.protected,
            witness.after,
            witness.owned
        );
        // Exact source ordinal/classification rows, not a counts-only oracle.
        for row in &observed.plan.rows[..observed.plan.len] {
            eprintln!(
                "fe2o3-retired-fixed-prefix-v1 block={} source_address={} expected={:?}",
                row.block, row.source_address, row.event
            );
        }
    }
    let after_work = budget.work();
    let total_work = after_work
        .checked_sub(before_work)
        .ok_or(QueryError::Resource(Resource::Accounting))?;
    eprintln!(
        "fe2o3-retired-fixed-observer-total-v1 before_work={before_work} after_work={after_work} total_work={total_work} modes=5 fixed_query_coverage=false"
    );
    Ok(())
}

// Observer-only concrete source frames. No old lexical allowance is enlarged;
// these typed rows and fixed panic payload are admitted before construction.
// Standard allocator/formatting/panic runtime internals are not a machine-stack
// or RSS claim. The independent classification loop admits its dynamic work.

type OuterCatchCaptures = (
    &'static &'static ProductionPreRankedKirOwnerV1,
    &'static &'static CheckedBf16CallInstanceV1<'static>,
    &'static &'static CanonicalKirInventoryV1<'static>,
    &'static &'static crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
    &'static mut &'static mut Budget<'static>,
    &'static Mode,
    &'static mut usize,
    &'static mut PendingActualRootPrefixIndicesV1,
    &'static mut Option<Observation>,
    &'static mut bool,
    &'static mut [usize; 5],
    &'static mut bool,
    &'static mut Option<PanicPayload>,
    &'static Option<usize>,
);
type ImmediateCatchCaptures = (
    &'static mut &'static mut Context,
    &'static &'static CheckedBf16NominalCallV1<'static>,
    &'static &'static NominalRootCfgSourceV1<'static>,
    &'static &'static crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
    &'static mut PendingActualRootPrefixIndicesV1,
    &'static oracle::Plan,
    &'static mut Option<Observation>,
    &'static Mode,
    &'static mut Option<PanicPayload>,
);

const OBSERVER_ROWS: usize = 32;
fn observer_rows() -> Result<[usize; OBSERVER_ROWS]> {
    Ok([
        call_frame::<Q<()>>(size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &CanonicalKirInventoryV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut Budget<'static>,
            [Mode; 5],
            std::array::IntoIter<Mode, 5>,
            Mode,
            Witness,
            Q<Observation>,
            Observation,
            Option<usize>,
        )>())?,
        call_frame::<Q<Observation>>(size_of::<(
            PendingActualRootPrefixIndicesV1,
            Namespace,
            Observation,
            Witness,
            Mode,
            LedgerId,
            usize,
            usize,
            usize,
            usize,
            [usize; 3],
            [usize; 5],
            Option<Observation>,
            bool,
            bool,
            Option<PanicPayload>,
            Option<usize>,
            std::thread::Result<Q<Observation>>,
            Q<Observation>,
            PanicPayload,
            [u64; 2],
        )>())?,
        call_frame::<Q<Observation>>(size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &CanonicalKirInventoryV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut Budget<'static>,
            Mode,
            &mut Witness,
        )>())?,
        call_frame::<Q<Observation>>(size_of::<(
            OuterCatchCaptures,
            AssertUnwindSafe<OuterCatchCaptures>,
            std::thread::Result<Q<Observation>>,
            Q<Observation>,
        )>())?,
        call_frame::<Q<Observation>>(size_of::<(
            &CheckedBf16NominalCallV1<'static>,
            &mut Budget<'static>,
            &NominalRootCfgSourceV1<'static>,
            oracle::Plan,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut Context,
        )>())?,
        call_frame::<Observation>(size_of::<(
            ActualRootArgumentInitializationV1<'static>,
            &mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &NominalRootCfgSourceV1<'static>,
            oracle::Plan,
            &mut Option<Observation>,
            Mode,
            Observation,
            &mut Option<PanicPayload>,
        )>())?,
        call_frame::<Observation>(size_of::<(
            ImmediateCatchCaptures,
            AssertUnwindSafe<ImmediateCatchCaptures>,
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &NominalRootCfgSourceV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut PendingActualRootPrefixIndicesV1,
            std::thread::Result<Result<Observation>>,
            Result<Observation>,
            Observation,
            PanicPayload,
            Option<usize>,
            Option<&[u64; 2]>,
            &[u64; 2],
        )>())?,
        call_frame::<Namespace>(size_of::<(
            &ActualRootArgumentInitializationV1<'static>,
            &NominalRootCfgSourceV1<'static>,
            usize,
            Namespace,
            &[Option<u32>],
            &[Option<u32>],
            &[ProductionRankedOperationV1],
        )>())?,
        call_frame::<bool>(size_of::<(
            std::slice::Iter<'static, Option<u32>>,
            Option<&Option<u32>>,
            &Option<u32>,
            bool,
        )>())?,
        call_frame::<bool>(size_of::<(
            std::slice::Iter<'static, Option<u32>>,
            Option<&Option<u32>>,
            &Option<u32>,
            bool,
        )>())?,
        call_frame::<()>(size_of::<(
            &PendingActualRootPrefixIndicesV1,
            Observation,
            &RetiredLazyProofPayloadsV1,
            Snapshot,
            Snapshot,
            [bool; 6],
            Option<usize>,
        )>())?,
        call_frame::<Observation>(size_of::<(
            &ActualRootArgumentInitializationV1<'static>,
            &NominalRootCfgSourceV1<'static>,
            &mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
            oracle::Plan,
            &mut Option<Observation>,
            Namespace,
            usize,
            Result<LazyFixedEventV1>,
            Error,
            Observation,
            Snapshot,
            LazyState,
        )>())?,
        call_frame::<Option<&oracle::Step>>(size_of::<(
            &[oracle::Step],
            std::slice::Iter<'static, oracle::Step>,
            &oracle::Step,
            Option<&SemanticBasicBlockV1>,
            &SemanticBasicBlockV1,
            Option<usize>,
            Option<usize>,
            bool,
        )>())?,
        call_frame::<QueryError>(size_of::<(Error, Resource)>())?,
        call_frame::<[usize; 5]>(size_of::<(
            &Cell<Option<([usize; 5], usize)>>,
            [usize; 5],
            usize,
            Option<([usize; 5], usize)>,
            ([usize; 5], usize),
        )>())?,
        call_frame::<()>(size_of::<(
            &Cell<Option<([usize; 5], usize)>>,
            Option<([usize; 5], usize)>,
            [usize; 5],
            usize,
            bool,
        )>())?,
        call_frame::<oracle::Plan>(size_of::<(
            oracle::Plan,
            [oracle::Step; 32],
            oracle::Step,
            usize,
            oracle::Stop,
        )>())?,
        call_frame::<oracle::Plan>(size_of::<(
            &NominalRootCfgSourceV1<'static>,
            &mut Budget<'static>,
            &SemanticFunctionDeclV1,
            oracle::Plan,
            usize,
            &SemanticBasicBlockV1,
            oracle::Event,
            oracle::Step,
            oracle::Stop,
        )>())?,
        call_frame::<Option<(usize, &SemanticBasicBlockV1)>>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, SemanticBasicBlockV1>>,
            Option<(usize, &SemanticBasicBlockV1)>,
            usize,
            &SemanticBasicBlockV1,
            Q<oracle::Event>,
        )>())?,
        call_frame::<Q<oracle::Event>>(size_of::<(
            &SemanticTerminatorKindV1,
            &[Option<u64>],
            &mut Budget<'static>,
            &bool,
            &SemanticUnwindActionV1,
            &SemanticOperandV1,
            &SemanticOperandV1,
            Option<u64>,
            Option<u64>,
            bool,
        )>())?,
        call_frame::<Option<u64>>(size_of::<(
            &SemanticOperandV1,
            &[Option<u64>],
            oracle::FrozenConstantDefinition,
            SemanticLocalIdV1,
            usize,
            Option<&Option<u64>>,
            Option<Option<u64>>,
            Option<u64>,
            u64,
        )>())?,
        call_frame::<oracle::FrozenConstantDefinition>(size_of::<(
            &SemanticOperandV1,
            &SemanticConstantV1,
            &SemanticConstantValueV1,
            &SemanticScalarValueV1,
            &SemanticPlaceV1,
            u128,
            std::result::Result<u64, std::num::TryFromIntError>,
            std::result::Result<oracle::FrozenConstantDefinition, std::num::TryFromIntError>,
            u64,
            SemanticLocalIdV1,
        )>())?,
        call_frame::<Snapshot>(size_of::<(
            &LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &RetiredLazyProofPayloadsV1,
            Snapshot,
            Option<Snapshot>,
            LazyState,
        )>())?,
        call_frame::<()>(size_of::<(
            PanicPayload,
            PanicPayload,
            Option<PanicPayload>,
            &(dyn Any + Send),
            *const (),
            Option<usize>,
            &mut bool,
            AssertUnwindSafe<(&mut Context, &mut PendingActualRootPrefixIndicesV1)>,
        )>())?,
        call_frame::<bool>(size_of::<(
            &Budget<'static>,
            &usize,
            usize,
            usize,
            [usize; 3],
            Option<usize>,
            CanonicalKernelIrWorkLedgerIdentityV1,
            Phase,
            Phase,
            &Option<Observation>,
        )>())?,
        call_frame::<()>(size_of::<(
            &mut Budget<'static>,
            usize,
            usize,
            PendingActualRootPrefixIndicesV1,
            Option<PanicPayload>,
            Q<Observation>,
            &mut Witness,
        )>())?,
        call_frame::<()>(size_of::<(
            Observation,
            Witness,
            Mode,
            Option<usize>,
            &oracle::Step,
            std::slice::Iter<'static, oracle::Step>,
            std::fmt::Arguments<'static>,
        )>())?,
        call_frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            Option<usize>,
            Option<usize>,
        )>())?,
        call_frame::<Q<()>>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Q<usize>,
            &Budget<'static>,
            std::fmt::Arguments<'static>,
        )>())?,
        call_frame::<[usize; OBSERVER_ROWS]>(size_of::<([usize; OBSERVER_ROWS], Option<usize>)>())?,
        call_frame::<usize>(size_of::<(
            Result<[usize; OBSERVER_ROWS]>,
            [usize; OBSERVER_ROWS],
            &[usize],
        )>())?,
        call_frame::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>())?,
    ])
}
fn observer_frame() -> Result<usize> {
    let own = sum(&observer_rows()?)?;
    let state = lazy_state_observer_frame_for_test_v1()?;
    let snapshot = retirement::empty_observation_snapshot_frame_for_test_v1()?;
    own.checked_add(state)
        .and_then(|n| n.checked_add(snapshot))
        .ok_or_else(arithmetic)
}

pub(in crate::production_ranked_projection_v1) fn factory_recording_frame_for_test_v1()
-> Result<usize> {
    type State = Option<([usize; 5], usize)>;
    const N: usize = 9;
    let rows: [usize; N] = [
        // Caller input, captured shared row loan, TLS callback argument/state,
        // prior and current rows, phase and poison/install transfers.
        call_frame::<()>(size_of::<(
            [usize; 5],
            &[usize; 5],
            &Cell<State>,
            State,
            [usize; 5],
            usize,
            State,
            State,
        )>())?,
        call_frame::<()>(size_of::<(
            &std::thread::LocalKey<Cell<State>>,
            &[usize; 5],
            &Cell<State>,
            State,
            (),
        )>())?,
        call_frame::<Option<usize>>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
            Option<usize>,
            bool,
        )>())?,
        call_frame::<State>(size_of::<(&Cell<State>, State, State)>())?,
        call_frame::<[usize; N]>(size_of::<([usize; N], Option<usize>)>())?,
        call_frame::<usize>(size_of::<([usize; N], &[usize], Result<usize>)>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(usize, usize, Option<usize>, Result<usize>, Error)>())?,
        // cfg(test) continuation caller's base/getter/checked-add/error and
        // result transfers; no call back into that continuation formula.
        call_frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Result<usize>,
            Result<usize>,
            Option<usize>,
            Error,
        )>())?,
    ];
    sum(&rows)
}

#[test]
fn retired_observer_frame_policy_has_typed_fixed_roster_and_no_residual() {
    let rows = observer_rows().unwrap();
    assert_eq!(rows.len(), OBSERVER_ROWS);
    assert_eq!(
        observer_frame().unwrap(),
        rows.iter().sum::<usize>()
            + lazy_state_observer_frame_for_test_v1().unwrap()
            + retirement::empty_observation_snapshot_frame_for_test_v1().unwrap()
    );
    assert!(
        observer_frame().unwrap()
            > size_of::<PendingActualRootPrefixIndicesV1>() + size_of::<oracle::Plan>()
    );
}
#[test]
fn factory_recording_is_inert_outside_new_genuine_scope() {
    FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
    record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
    FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
    start_frames();
    record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
    assert_eq!(finish_frames(), [1, 2, 3, 4, 10]);
    FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
}

#[test]
fn caught_invalid_recording_stays_poisoned_until_scope_is_discarded() {
    for kind in 0..3 {
        start_frames();
        match kind {
            0 => {
                record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
                assert!(
                    catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 10])).is_err()
                );
            }
            1 => {
                assert!(
                    catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 99])).is_err()
                );
                assert!(
                    catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 10])).is_err()
                );
            }
            _ => {
                assert!(catch_unwind(start_frames).is_err());
            }
        }
        assert!(catch_unwind(finish_frames).is_err());
        FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
    }
}

#[test]
fn common_recorder_frame_amount_is_positive_and_independent_of_active_state() {
    let inactive = factory_recording_frame_for_test_v1().unwrap();
    assert!(inactive > size_of::<[usize; 5]>());
    start_frames();
    assert_eq!(factory_recording_frame_for_test_v1().unwrap(), inactive);
    record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
    assert_eq!(factory_recording_frame_for_test_v1().unwrap(), inactive);
    assert_eq!(finish_frames(), [1, 2, 3, 4, 10]);
    assert_eq!(factory_recording_frame_for_test_v1().unwrap(), inactive);
    start_frames();
    assert!(catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 99])).is_err());
    assert_eq!(factory_recording_frame_for_test_v1().unwrap(), inactive);
    assert!(catch_unwind(finish_frames).is_err());
    assert_eq!(
        continuation_frame_v1::<(), ()>().unwrap(),
        continuation_rows::<(), ()>().unwrap().iter().sum::<usize>() + inactive
    );
}

#[test]
fn common_recorder_actual_data_admission_precedes_active_and_inactive_hook() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    // DATA-only exact/one-short debit controls, not fabricated source authority.
    let continuation = continuation_frame_v1::<(), ()>().unwrap();
    let components = [3, 5, continuation, 7];
    let bytes = components.iter().sum::<usize>();
    for mode in 0..4 {
        let active = mode != 0;
        if active {
            start_frames();
        }
        let mut work = Work::new(if mode == 2 { bytes - 1 } else { bytes });
        let mut budget = Budget::new(&mut work, if mode == 3 { bytes - 1 } else { bytes });
        let mut owned = 0;
        let admitted = admit_factory_frame_v1(&mut Prep::new(&mut budget, &mut owned), components);
        if let Ok(total) = admitted {
            assert!(mode < 2);
            assert_eq!(total, bytes);
            assert_eq!(
                (budget.work(), budget.storage(), owned),
                (bytes, bytes, bytes)
            );
            record_factory_frames_for_test_v1([3, 5, continuation, 7, total]);
            if active {
                assert_eq!(finish_frames(), [3, 5, continuation, 7, total]);
            } else {
                FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
            }
            budget.release_storage(owned).unwrap();
        } else {
            assert!(mode >= 2);
            assert_eq!((budget.storage(), owned), (0, 0));
            assert_eq!(budget.failed_work().is_some(), mode == 2);
            assert_eq!(budget.failed_storage().is_some(), mode == 3);
            if mode == 3 {
                assert_eq!(budget.work(), bytes);
            }
            FACTORY_FRAMES.with(|cell| assert_eq!(cell.get(), Some(([0; 5], 0))));
            assert!(catch_unwind(finish_frames).is_err());
        }
        FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
    }
}

#[test]
fn common_recorder_actual_factory_source_hook_follows_both_admissions() {
    let child: String = include_str!("bf16_nominal_root_retired_fixed_observation_v1.rs")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let test_start = child
        .find("#[cfg(test)]pub(super)fncontinuation_frame_v1")
        .unwrap();
    let test_end = child[test_start..]
        .find("pub(super)fnadmit_factory_frame_v1")
        .unwrap()
        + test_start;
    let caller = &child[test_start..test_end];
    assert!(caller.contains("factory_recording_frame_for_test_v1()"));
    assert!(caller.contains("checked_add"));
    assert!(!caller.contains("FACTORY_FRAMES"));
    let admit =
        &child[test_end..child[test_end..].find("fncompatibility_rows").unwrap() + test_end];
    let work = admit.find("resources.work(bytes)?;").unwrap();
    let storage = admit.find("resources.reserve_storage(bytes)?;").unwrap();
    assert!(work < storage);
    let f1: String = include_str!("bf16_nominal_root_argument_initialization_v1.rs")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let debit = f1.find("admit_factory_frame_v1(").unwrap();
    let hook = f1.find("record_factory_frames_for_test_v1(").unwrap();
    assert!(debit < hook);
    assert!(f1[debit..hook].contains("pending.frame_credits=bytes;"));
    assert!(f1[debit..hook].contains("pending.started=true;"));
}
