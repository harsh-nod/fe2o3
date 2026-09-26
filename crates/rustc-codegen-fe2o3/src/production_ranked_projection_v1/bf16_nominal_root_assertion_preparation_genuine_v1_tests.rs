//! Genuine-owner assertion-scope observations in the real source qualification hook.
//! Actual Assert count is returned as scalar observation, never inferred coverage.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const HEADERS: usize = 8192;
const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct RootAssertionObservationV1 {
    pub(in crate::production_ranked_projection_v1) blocks: usize,
    pub(in crate::production_ranked_projection_v1) assertions: usize,
    pub(in crate::production_ranked_projection_v1) true_decisions: usize,
    pub(in crate::production_ranked_projection_v1) logical_work: usize,
}
fn observe_view(
    view: &NominalRootAssertionSourceV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    caller: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> Result<RootAssertionObservationV1> {
    budget.charge_work(256)?;
    let source = owner.semantic_ssa().source_semantic();
    let function =
        source
            .functions()
            .get(caller.index() as usize)
            .ok_or(QueryError::Unavailable(
                "genuine root assertion function absent",
            ))?;
    let cfg = view.cfg();
    assert!(std::ptr::eq(function, cfg.function()));
    assert_eq!(cfg.types().as_ptr(), source.types().as_ptr());
    assert_eq!(cfg.types().len(), source.types().len());
    let report = cfg.source_tables().induction_report();
    assert_eq!(report.semantic_mir_sha256(), source.semantic_sha256());
    assert_eq!(report.function(), caller);
    assert_eq!(report.function_identity(), function.identity());
    assert!(!report.grants_authority());
    assert!(!cfg.source_tables().rich().option_producers().is_empty());
    assert_eq!(view.decisions().len(), function.blocks().len());
    assert_eq!(cfg.graph().successors.len(), function.blocks().len());
    assert_eq!(cfg.graph().predecessors.len(), function.blocks().len());
    assert_eq!(cfg.graph().reachable.len(), function.blocks().len());
    let scan = function
        .blocks()
        .len()
        .checked_mul(32)
        .and_then(|n| n.checked_add(256))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(scan)?;
    let mut observation = RootAssertionObservationV1 {
        blocks: function.blocks().len(),
        assertions: 0,
        true_decisions: 0,
        logical_work: view.logical_work(),
    };
    for (block, &decision) in function.blocks().iter().zip(view.decisions()) {
        let is_assert = matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Assert { .. }
        );
        if is_assert {
            observation.assertions = observation
                .assertions
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        } else {
            assert!(!decision);
        }
        if decision {
            observation.true_decisions = observation
                .true_decisions
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    assert!(observation.blocks > 0);
    assert!(observation.true_decisions <= observation.assertions);
    // Zero actual asserts is legitimate ownership preparation, not positive
    // evaluator coverage. B's raw-component parity controls cover those paths.
    Ok(observation)
}
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn observe_root_assertion_preparation_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'_>,
) -> Result<RootAssertionObservationV1> {
    let floor = budget.storage();
    let original = budget.work_ledger_identity_v1();
    let entered = Cell::new(0usize);
    let observation = with_nominal_root_assertion_preparation_v1(
        owner,
        inventory,
        root,
        caller,
        block,
        call,
        budget,
        |view, budget| {
            entered.set(entered.get() + 1);
            assert!(budget.work_ledger_identity_v1() == original);
            observe_view(view, owner, caller, budget)
        },
    )?;
    assert_eq!(entered.get(), 1);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == original);
    Ok(observation)
}

fn with_headers<'w>(
    budget: &mut Budget<'w>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'w>) -> Result<()>,
) -> Result<()> {
    let before = Custody::new(budget)?;
    budget.reserve_storage(bytes)?;
    let result = catch_unwind(AssertUnwindSafe(|| body(budget)));
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    before.check(budget, bytes)?;
    budget.release_storage(bytes)?;
    result
}

/// Bounded resource diagnostics only. The sole independent ledger is a negative
/// F-1 accounting probe, wholly prepaid on ORIGINAL; no positive authority result
/// is constructed on it. No phase limits or external admission are widened.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn root_assertion_preparation_controls_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    original: &mut Budget<'_>,
) -> Result<()> {
    with_headers(original, 4 * 8192 + HEADERS, |original| {
        original.charge_work(4 * 8192 + 256)?;
        let entry = original.storage();
        let identity = original.work_ledger_identity_v1();
        let payload = [7u8; 8192];
        let result = with_nominal_root_assertion_preparation_v1(
            owner,
            inventory,
            root,
            caller,
            block,
            call,
            original,
            move |view, budget| {
                observe_view(view, owner, caller, budget)?;
                assert_eq!(payload[8191], 7);
                Ok(payload)
            },
        )?;
        assert_eq!(result, [7; 8192]);
        assert_eq!(original.storage(), entry);
        assert!(original.work_ledger_identity_v1() == identity);

        for mode in 0..3 {
            let before = original.storage();
            let entered = Cell::new(false);
            let result = with_nominal_root_assertion_preparation_v1(
                owner,
                inventory,
                root,
                caller,
                block,
                call,
                original,
                |view, budget| -> Result<()> {
                    entered.set(true);
                    observe_view(view, owner, caller, budget)?;
                    budget.reserve_storage(23)?;
                    budget.charge_work(17)?;
                    match mode {
                        0 => Ok(()),
                        1 => Err(QueryError::Unavailable(
                            "genuine source assertion root refusal",
                        )),
                        _ => panic!("genuine source assertion root unwind"),
                    }
                },
            );
            assert!(entered.get());
            assert_eq!(
                result,
                match mode {
                    0 => Ok(()),
                    1 => Err(QueryError::Unavailable(
                        "genuine source assertion root refusal"
                    )),
                    _ => Err(QueryError::CallbackPanicked),
                }
            );
            assert_eq!(original.storage(), before + 23);
            assert!(original.work_ledger_identity_v1() == identity);
            original.release_storage(23)?;
        }
        let occurrence = owner
            .semantic_ssa()
            .occurrence_storage()
            .ok_or(QueryError::Unavailable(
                "genuine source assertion source occurrence absent",
            ))?
            .retained_storage();
        let floor = owner
            .retained_analysis_storage_v1()
            .checked_add(occurrence)
            .and_then(|n| n.checked_add(inventory_storage))
            .ok_or(Resource::Arithmetic)?;
        let short = floor.checked_sub(1).ok_or(Resource::Arithmetic)?;
        let limit = floor
            .checked_add(PROBE_SCRATCH)
            .ok_or(Resource::Arithmetic)?;
        assert!(original.storage() >= floor);
        with_headers(original, PROBE_SCRATCH, |original| {
            original.charge_work(PROBE_WORK + 4 * 8192 + 256)?;
            let mut work = Work::new(PROBE_WORK);
            let mut probe = Budget::new(&mut work, limit);
            probe.reserve_storage(short)?;
            let entered = Cell::new(false);
            let entered_ref = &entered;
            let payload = [9u8; 8192];
            let result = with_nominal_root_assertion_preparation_v1(
                owner,
                inventory,
                root,
                caller,
                block,
                call,
                &mut probe,
                move |_, _| {
                    entered_ref.set(true);
                    Ok(payload)
                },
            );
            assert_eq!(result, Err(Resource::Accounting.into()));
            assert!(!entered.get());
            assert_eq!(probe.storage(), short);
            // Real generic entry frame acceptance must not mask the true F-1.
            assert!(probe.peak_storage() > floor);
            assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
            Ok(())
        })
    })
}

#[test]
fn proposed_source_assertion_genuine_control_frames_fit_prepaid_fixed_envelope() {
    assert!(
        2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 16 * size_of::<Cell<usize>>()
            + 8 * size_of::<Result<()>>()
            + 2048
            <= HEADERS
    );
}
