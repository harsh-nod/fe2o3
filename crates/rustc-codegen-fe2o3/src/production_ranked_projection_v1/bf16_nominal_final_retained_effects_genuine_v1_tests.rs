//! Actual-source retained-table observations, not synthetic positive admission.
use super::*;
use crate::production_ranked_projection_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticTerminatorKindV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
use std::cell::Cell;

const HEADERS: usize = 8192;
const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;

fn observe_view(
    view: &NominalFinalRetainedEffectsV1<'_, '_>,
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<[usize; 4]> {
    budget.charge_work(add(256, times(view.effects().len(), 32)?)?)?;
    let candidate = view.candidate();
    assert!(std::ptr::eq(candidate.owner(), owner));
    assert!(std::ptr::eq(candidate.inventory(), inventory));
    assert!(std::ptr::eq(candidate.source_call(), source.source_call()));
    assert!(inventory.belongs_to(owner.executable()));
    let semantic = owner.semantic_ssa().source_semantic();
    let function = &semantic.functions()[source.root().index() as usize];
    assert!(std::ptr::eq(view.function(), function));
    assert_eq!(view.effects().len(), function.blocks().len());
    assert_eq!(view.run().query_visits, [1, 1, 1]);
    assert_eq!(view.run().authenticated_visits[2], 1);
    assert!(!view.run().array_destination_has_origin);
    assert_ne!(
        candidate.call().coordinate.block.function,
        candidate.matrix().coordinate.block.function
    );
    assert_eq!(
        candidate.returned().coordinate.function,
        candidate.matrix().coordinate.block.function
    );
    assert_eq!(candidate.permutation(), source.return_permutation());
    let Terminator::Return { values } = candidate.returned().terminator else {
        panic!("actual retained candidate Return");
    };
    for (index, row) in candidate.rows().iter().enumerate() {
        assert_eq!(
            row.caller_value(),
            candidate.call().operation.results[index].id
        );
        assert_eq!(row.return_value(), values[index]);
        assert_eq!(
            row.matrix_value(),
            candidate.matrix().operation.results[usize::from(candidate.permutation()[index])].id
        );
    }
    let mut counts = [0usize; 4];
    let mut actual_fragment_reads = 0usize;
    for (block, effect) in function.blocks().iter().zip(view.effects()) {
        let fragment_read = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => matches!(
                semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. },
                    ..
                })
            ),
            _ => false,
        };
        actual_fragment_reads += usize::from(fragment_read);
        // These are the existing Identity/Swap01 fixture's two source loads,
        // not inferred from the tensor-only candidate or an expected block ID.
        assert_eq!(effect.global_read.is_some(), fragment_read);
        // The current root delegates its sole MFMA to the nominal helper.
        // Other retained fields are checked rather than silently discarded.
        assert!(effect.layout.is_none());
        assert!(effect.transpose_workgroup.is_none());
        assert!(effect.read_view.is_none());
        for (slot, present) in counts.iter_mut().zip([
            effect.layout.is_some(),
            effect.global_read.is_some(),
            effect.transpose_workgroup.is_some(),
            effect.read_view.is_some(),
        ]) {
            *slot = slot
                .checked_add(usize::from(present))
                .ok_or(Resource::Arithmetic)?;
        }
    }
    assert_eq!(actual_fragment_reads, 2);
    assert_eq!(counts, [0, 2, 0, 0]);
    // Source-indexed effects are NOT ranked positions, bounds certificates or
    // the conditional output-write recipe. No access authority is constructed.
    Ok(counts)
}

pub(in crate::production_ranked_projection_v1) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let before = Checkpoint::take(budget);
    let entered = Cell::new(0usize);
    let counts = with_nominal_final_retained_effects_v1(
        owner,
        inventory,
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |view, budget| {
            entered.set(entered.get() + 1);
            assert!(budget.work_ledger_identity_v1() == before.ledger);
            observe_view(view, owner, source, inventory, budget)
        },
    )?;
    assert_eq!(entered.get(), 1);
    before.require_completed(budget)?;
    eprintln!(
        "fe2o3-retained-final-effects-observation-v1 root={} call_block={} permutation={:?} layouts={} global_reads={} transpose_workgroups={} read_views={}",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        counts[0],
        counts[1],
        counts[2],
        counts[3],
    );
    Ok(())
}
fn with_headers<'w>(
    budget: &mut Budget<'w>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'w>) -> Result<()>,
) -> Result<()> {
    budget.reserve_storage(bytes)?;
    let protected = Checkpoint::take(budget);
    let outcome = catch_unwind(AssertUnwindSafe(|| body(budget)));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    protected.require_custody(budget)?;
    budget.release_storage(bytes)?;
    result
}
pub(in crate::production_ranked_projection_v1) fn controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    with_headers(original, HEADERS + 4 * 8192, |original| {
        for mode in 0..3 {
            original.charge_work(4 * 8192 + 128)?;
            let before = Checkpoint::take(original);
            let entered = Cell::new(0usize);
            let entered_ref = &entered;
            let payload = [7u8; 8192];
            let result = with_nominal_final_retained_effects_v1(
                owner,
                inventory,
                source.root(),
                source.call_block(),
                source.source_call(),
                original,
                move |view, budget| -> Result<[u8; 8192]> {
                    entered_ref.set(entered_ref.get() + 1);
                    assert!(budget.work_ledger_identity_v1() == before.ledger);
                    observe_view(view, owner, source, inventory, budget)?;
                    assert_eq!(payload[8191], 7);
                    budget.reserve_storage(23)?;
                    budget.charge_work(17)?;
                    match mode {
                        0 => Ok(payload),
                        1 => Err(Error::Unavailable("genuine retained Final refusal")),
                        _ => panic!("genuine retained Final unwind"),
                    }
                },
            );
            assert_eq!(entered.get(), 1);
            match mode {
                0 => assert_eq!(result, Ok([7; 8192])),
                1 => assert_eq!(
                    result,
                    Err(Error::Unavailable("genuine retained Final refusal"))
                ),
                _ => assert_eq!(result, Err(Error::CallbackPanicked)),
            }
            before.require_custody(original)?;
            assert_eq!(original.storage(), before.storage + 23);
            assert_eq!(
                (original.failed_work(), original.failed_storage()),
                (None, None)
            );
            original.release_storage(23)?;
        }
        let occurrence = owner
            .semantic_ssa()
            .occurrence_storage()
            .ok_or(Error::Unavailable(
                "genuine retained occurrence storage absent",
            ))?
            .retained_storage();
        let floor = add(
            add(owner.retained_analysis_storage_v1(), occurrence)?,
            inventory_storage,
        )?;
        let short = floor.checked_sub(1).ok_or(Resource::Arithmetic)?;
        // Independent ledgers below are negative-only probes, never positive
        // views. Each complete bounded probe is prepaid on ORIGINAL; no phase
        // or storage limit changes and no refund of original cumulative work.
        with_headers(original, PROBE_SCRATCH, |original| {
            for mode in 0..3 {
                original.charge_work(PROBE_WORK + 4 * 8192 + 256)?;
                let mut work = Work::new(if mode == 1 { 0 } else { PROBE_WORK });
                let mut probe = Budget::new(
                    &mut work,
                    if mode == 2 {
                        floor
                    } else {
                        add(floor, PROBE_SCRATCH)?
                    },
                );
                let incoming = if mode == 0 { short } else { floor };
                probe.reserve_storage(incoming)?;
                let entered = Cell::new(false);
                let entered_ref = &entered;
                let payload = [9u8; 8192];
                let result = with_nominal_final_retained_effects_v1(
                    owner,
                    inventory,
                    source.root(),
                    source.call_block(),
                    source.source_call(),
                    &mut probe,
                    move |_, _| {
                        entered_ref.set(true);
                        Ok(payload)
                    },
                );
                assert!(!entered.get());
                assert_eq!(probe.storage(), incoming);
                match mode {
                    0 => {
                        assert_eq!(result, Err(Resource::Accounting.into()));
                        assert!(probe.peak_storage() > floor);
                        assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
                    }
                    1 => {
                        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                        assert!(probe.failed_work().is_some());
                    }
                    _ => {
                        assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                        assert!(probe.failed_storage().is_some());
                    }
                }
            }
            Ok(())
        })
    })
}
#[test]
fn retained_genuine_control_frames_fit_the_prepaid_header() {
    assert!(
        2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 32 * size_of::<Cell<usize>>()
            + 12 * size_of::<Result<()>>()
            + 4 * size_of::<Checkpoint>()
            + 2048
            <= HEADERS
    );
}
