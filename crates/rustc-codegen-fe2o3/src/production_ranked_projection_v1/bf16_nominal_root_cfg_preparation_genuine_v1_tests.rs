//! Proposed genuine-source hooks. Root wires/runs these under its owned gate.
//! No manufactured source owner, ranked recipe, normal admission or native claim.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const HEADERS: usize = 8192;
const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;

fn observe_view(
    view: &NominalRootCfgSourceV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    caller: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(256)?;
    let source = owner.semantic_ssa().source_semantic();
    let function =
        source
            .functions()
            .get(caller.index() as usize)
            .ok_or(QueryError::Unavailable(
                "genuine source CFG function absent",
            ))?;
    assert!(std::ptr::eq(function, view.function()));
    assert_eq!(
        view.source_tables()
            .induction_report()
            .semantic_mir_sha256(),
        source.semantic_sha256()
    );
    assert_eq!(view.source_tables().induction_report().function(), caller);
    assert_eq!(
        view.source_tables().induction_report().function_identity(),
        function.identity()
    );
    assert!(!view.source_tables().induction_report().grants_authority());
    assert!(!view.source_tables().rich().option_producers().is_empty());
    let graph = view.graph();
    assert_eq!(graph.successors.len(), function.blocks().len());
    assert_eq!(graph.predecessors.len(), function.blocks().len());
    assert_eq!(graph.reachable.len(), function.blocks().len());
    assert_eq!(graph.entry, function.entry().index() as usize);
    assert!(graph.reachable[graph.entry]);
    let mut calls = 0usize;
    let mut edges = 0usize;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge_work(32)?;
        let row = &graph.successors[block_index];
        let raw_count = match block.terminator().kind() {
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => targets
                .values()
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
            _ => 1,
        };
        // Before every source scan, comparison, binary search and row check.
        let work = raw_count
            .checked_add(row.len())
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(row.len().checked_add(1)?))
            .and_then(|n| n.checked_mul(16))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        assert!(row.windows(2).all(|pair| pair[0] < pair[1]));
        let has = |target: usize| row.binary_search(&target).is_ok();
        for &target in row {
            assert!(target < function.blocks().len());
            assert!(source_contains_target(function, block_index, target));
            budget.charge_work(
                graph.predecessors[target]
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            assert!(
                graph.predecessors[target]
                    .binary_search(&block_index)
                    .is_ok()
            );
            if graph.reachable[block_index] {
                assert!(graph.reachable[target]);
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => assert!(has(edge.target().index() as usize)),
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                for target in targets.values() {
                    assert!(has(target.edge().target().index() as usize));
                }
                let otherwise = targets.otherwise().target().index() as usize;
                if source_contains_target(function, block_index, otherwise) {
                    assert!(has(otherwise));
                }
            }
            SemanticTerminatorKindV1::Call(call) => {
                calls = calls.checked_add(1).ok_or(Resource::Arithmetic)?;
                if let Some(destination) = call.destination() {
                    assert!(has(destination.edge().target().index() as usize));
                }
            }
            SemanticTerminatorKindV1::Assert { target, .. }
            | SemanticTerminatorKindV1::Drop { target, .. } => {
                assert!(has(target.target().index() as usize))
            }
            SemanticTerminatorKindV1::FalseEdge { .. } => panic!("unnormalized source CFG"),
            _ => assert!(row.is_empty()),
        }
        let predecessors = &graph.predecessors[block_index];
        budget.charge_work(
            predecessors
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
        )?;
        assert!(predecessors.windows(2).all(|pair| pair[0] < pair[1]));
        for &predecessor in predecessors {
            budget.charge_work(
                graph.successors[predecessor]
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            assert!(
                graph.successors[predecessor]
                    .binary_search(&block_index)
                    .is_ok()
            );
        }
        edges = edges.checked_add(row.len()).ok_or(Resource::Arithmetic)?;
    }
    assert!(calls > 0 && edges > 0);
    Ok(())
}
fn source_contains_target(function: &SemanticFunctionDeclV1, block: usize, wanted: usize) -> bool {
    match function.blocks()[block].terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge) => edge.target().index() as usize == wanted,
        SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
            let otherwise = targets.otherwise().target().index() as usize;
            targets
                .values()
                .iter()
                .any(|edge| edge.edge().target().index() as usize == wanted)
                || (otherwise == wanted
                    && !(targets.values().len() == 2
                        && targets.values().iter().any(|target| target.value() == 0)
                        && targets.values().iter().any(|target| target.value() == 1)
                        && switch_fallback_is_empty_unreachable_v1(function, otherwise)))
        }
        SemanticTerminatorKindV1::Call(call) => call
            .destination()
            .is_some_and(|destination| destination.edge().target().index() as usize == wanted),
        SemanticTerminatorKindV1::Assert { target, .. }
        | SemanticTerminatorKindV1::Drop { target, .. } => {
            target.target().index() as usize == wanted
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn observe_root_cfg_preparation_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let floor = budget.storage();
    let original = budget.work_ledger_identity_v1();
    let entered = Cell::new(0usize);
    with_nominal_root_cfg_preparation_v1(
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
    Ok(())
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
pub(in crate::production_ranked_projection_v1) fn root_cfg_preparation_controls_for_test_v1(
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
        let result = with_nominal_root_cfg_preparation_v1(
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
            let result = with_nominal_root_cfg_preparation_v1(
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
                        1 => Err(QueryError::Unavailable("genuine source CFG root refusal")),
                        _ => panic!("genuine source CFG root unwind"),
                    }
                },
            );
            assert!(entered.get());
            assert_eq!(
                result,
                match mode {
                    0 => Ok(()),
                    1 => Err(QueryError::Unavailable("genuine source CFG root refusal")),
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
                "genuine source CFG source occurrence absent",
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
            let result = with_nominal_root_cfg_preparation_v1(
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
fn proposed_source_cfg_genuine_control_frames_fit_prepaid_fixed_envelope() {
    assert!(
        2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 16 * size_of::<Cell<usize>>()
            + 8 * size_of::<Result<()>>()
            + 2048
            <= HEADERS
    );
}
