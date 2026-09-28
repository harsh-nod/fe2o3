//! Authentic retirement-observation connector, not an ordered F2 producer.
//! Reuses ONE actual root-CFG/rich loan and the SAME pending/counter owner.
#![allow(dead_code)]
use super::*;
use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::lazy_fixed_proof_owner_v1::{
    LazyFixedProofOwnerV1,
    retirement::{RetiredLazyProofPayloadsV1, with_retired_lazy_proof_v1},
};
use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::{
    NominalRootCfgSourceV1, NominalRootSourceTablesV1,
};

type Slot = Option<RetiredLazyProofPayloadsV1>;
type Owner = LazyFixedProofOwnerV1<'static, 'static, 'static>;
type View = ActualRootArgumentInitializationV1<'static>;
type FrameFn = fn() -> Result<usize>;
const SLOT_ROWS: usize = 6;
const SEAM_ROWS: usize = 9;
const COMPAT_ROWS: usize = 6;
const OBSERVATION_ROWS: usize = 17;

/// DATA predicate only. No source/owner identity or reusable readiness result.
pub(in crate::production_ranked_projection_v1) fn slot_vacant_v1(slot: &Slot) -> bool {
    slot.is_none()
}
fn slot_rows() -> Result<[usize; SLOT_ROWS]> {
    Ok([
        // The new literal None and its constructor/return transfers. The whole
        // pending header is separately present in the original assembly row.
        call_frame::<Slot>(size_of::<Slot>())?,
        call_frame::<bool>(size_of::<(&Slot, bool)>())?,
        call_frame::<[usize; SLOT_ROWS]>(size_of::<([usize; SLOT_ROWS], Option<usize>)>())?,
        call_frame::<usize>(size_of::<(
            Result<[usize; SLOT_ROWS]>,
            [usize; SLOT_ROWS],
            &[usize],
        )>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn slot_construction_frame_v1() -> Result<usize> {
    sum(&slot_rows()?)
}

/// Explicit added vertices of the private disjoint continuation. The 25 old F1
/// rows remain byte-exact and admitted independently with its ACTUAL F/R.
fn continuation_rows<R, F>() -> Result<[usize; SEAM_ROWS]> {
    type P = PendingActualRootPrefixIndicesV1;
    Ok([
        call_frame::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut P,
            FrameFn,
            F,
            Result<R>,
            LedgerId,
            bool,
        )>())?,
        // New continuation parameters, including exclusive slot loan.
        call_frame::<R>(size_of::<(F, View, &mut Context, &mut Slot, Result<R>)>())?,
        // Additional live admission fields, including the actual frame function
        // pointer and all checked component/total/error transfers.
        call_frame::<()>(size_of::<(
            &mut Prep<'static, 'static>,
            &mut P,
            &FrameFn,
            usize,
            usize,
            usize,
            usize,
            usize,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            [usize; 4],
            &[usize],
        )>())?,
        // Disjoint borrow in the graph continuation, not a second pending.
        call_frame::<R>(size_of::<(
            &mut Slot,
            &mut PendingArgumentProducersV1,
            &mut RootEntryPrefixV1,
            &mut Context,
            F,
            View,
            Result<R>,
        )>())?,
        call_frame::<usize>(size_of::<(FrameFn, Result<usize>)>())?,
        call_frame::<[usize; SEAM_ROWS]>(size_of::<([usize; SEAM_ROWS], Option<usize>)>())?,
        call_frame::<usize>(size_of::<(
            Result<[usize; SEAM_ROWS]>,
            [usize; SEAM_ROWS],
            &[usize],
        )>())?,
        call_frame::<usize>(size_of::<(
            [usize; 4],
            &[usize],
            Result<usize>,
            Option<usize>,
            Error,
        )>())?,
        call_frame::<usize>(size_of::<(
            &mut Prep<'static, 'static>,
            [usize; 4],
            &[usize],
            usize,
            Result<usize>,
        )>())?,
    ])
}
#[cfg(not(test))]
pub(super) fn continuation_frame_v1<R, F>() -> Result<usize> {
    sum(&continuation_rows::<R, F>()?)
}
#[cfg(test)]
pub(super) fn continuation_frame_v1<R, F>() -> Result<usize> {
    let bytes = sum(&continuation_rows::<R, F>()?)?;
    // Every F1 call reaches the hook, even before this observer is active.
    // The companion names recorder/getter/caller rows without ledger recursion.
    bytes
        .checked_add(genuine::factory_recording_frame_for_test_v1()?)
        .ok_or_else(arithmetic)
}
/// DATA-only debit mechanics; source/freshness/owner checks remain in the
/// private factory before this call. No readiness or alternative ledger.
pub(super) fn admit_factory_frame_v1(
    resources: &mut Prep<'_, '_>,
    components: [usize; 4],
) -> Result<usize> {
    let bytes = sum(&components)?;
    resources.work(bytes)?;
    resources.reserve_storage(bytes)?;
    Ok(bytes)
}
fn compatibility_rows<R, F>() -> Result<[usize; COMPAT_ROWS]> {
    Ok([
        // Preserved public entry and its actual, non-Copy consumer.
        call_frame::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut PendingActualRootPrefixIndicesV1,
            F,
            FrameFn,
            Result<R>,
        )>())?,
        // Compatibility adapter captures exactly F; the ignored slot is borrowed.
        call_frame::<R>(size_of::<(F, View, &mut Context, &mut Slot, Result<R>)>())?,
        call_frame::<[usize; COMPAT_ROWS]>(size_of::<([usize; COMPAT_ROWS], Option<usize>)>())?,
        call_frame::<usize>(size_of::<(
            Result<[usize; COMPAT_ROWS]>,
            [usize; COMPAT_ROWS],
            &[usize],
        )>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}
pub(super) fn compatibility_entry_frame_v1<R, F>() -> Result<usize> {
    sum(&compatibility_rows::<R, F>()?)
}

/// Closed boolean/ledger comparison, NOT a token or an alternative constructor.
/// Production operands below come only from the existing authenticated views.
fn require_join_v1(
    same_cfg_function: bool,
    same_rich_function: bool,
    expected: LedgerId,
    observed: Option<LedgerId>,
    rich_matches: bool,
    denied: bool,
) -> Result<()> {
    if !same_cfg_function
        || !same_rich_function
        || observed != Some(expected)
        || !rich_matches
        || denied
    {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}
fn observation_rows<R, F>() -> Result<[usize; OBSERVATION_ROWS]> {
    type Cfg = NominalRootCfgSourceV1<'static>;
    type Rich = RichNominalSourceTablesV1<'static>;
    type Resources = Prep<'static, 'static>;
    Ok([
        call_frame::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &Cfg,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut PendingActualRootPrefixIndicesV1,
            F,
            &Rich,
            FrameFn,
            Result<R>,
        )>())?,
        // Actual F1 continuation closure and its captured cfg + F.
        call_frame::<R>(size_of::<(
            &Cfg,
            &Rich,
            F,
            View,
            &mut Context,
            &mut Slot,
            Result<R>,
        )>())?,
        // Actual original-resource callback: all captured source/view/slot/F
        // fields, source getters and boolean/ledger result transfers are named.
        call_frame::<R>(size_of::<(
            &mut Resources,
            &Cfg,
            F,
            View,
            &mut Slot,
            &Rich,
            &SemanticFunctionDeclV1,
            &SemanticFunctionDeclV1,
            LedgerId,
            Option<LedgerId>,
            bool,
            bool,
            bool,
            bool,
            Result<()>,
            Result<R>,
        )>())?,
        // Bridge's actual callback captures View + F, not context/resources.
        call_frame::<R>(size_of::<(View, F, &mut Owner, Result<R>)>())?,
        call_frame::<()>(size_of::<(
            bool,
            bool,
            LedgerId,
            Option<LedgerId>,
            bool,
            bool,
            Result<()>,
            Error,
        )>())?,
        call_frame::<&NominalRootSourceTablesV1<'static>>(size_of::<&Cfg>())?,
        call_frame::<&Rich>(size_of::<&NominalRootSourceTablesV1<'static>>())?,
        call_frame::<&SemanticFunctionDeclV1>(size_of::<&Cfg>())?,
        call_frame::<&SemanticFunctionDeclV1>(size_of::<&Rich>())?,
        call_frame::<bool>(size_of::<(&Rich, LedgerId, bool)>())?,
        call_frame::<Option<LedgerId>>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            &Budget<'static>,
            LedgerId,
        )>())?,
        call_frame::<bool>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            Option<usize>,
            Option<usize>,
        )>())?,
        call_frame::<()>(size_of::<(&mut Resources, &mut &mut Budget<'static>, usize)>())?,
        call_frame::<[usize; OBSERVATION_ROWS]>(size_of::<(
            [usize; OBSERVATION_ROWS],
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(
            Result<[usize; OBSERVATION_ROWS]>,
            [usize; OBSERVATION_ROWS],
            &[usize],
        )>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}
fn observation_entry_frame_v1<R, F>() -> Result<usize> {
    sum(&observation_rows::<R, F>()?)
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    /// One authentic observation only. No earlier F2 prelude or argument writer
    /// is represented as executed; no pending/resource/source borrow escapes.
    pub(in crate::production_ranked_projection_v1) fn with_actual_root_retired_fixed_proof_observation_v1<
        R: Copy + 'static,
        F,
    >(
        &mut self,
        checked: &CheckedBf16NominalCallV1<'_>,
        cfg: &NominalRootCfgSourceV1<'_>,
        actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
        pending: &mut PendingActualRootPrefixIndicesV1,
        inspect: F,
    ) -> Result<R>
    where
        F: for<'v, 'a, 'b, 'w> FnOnce(
            ActualRootArgumentInitializationV1<'v>,
            &mut LazyFixedProofOwnerV1<'a, 'b, 'w>,
        ) -> Result<R>,
    {
        let rich = cfg.source_tables().rich();
        self.with_actual_root_argument_initialization_parts_v1(
            checked,
            rich,
            actual_inputs,
            pending,
            observation_entry_frame_v1::<R, F>,
            move |view, context, retired| {
                context.with_resources(move |resources| {
                    resources.work(32)?;
                    require_join_v1(
                        std::ptr::eq(view.source.function, cfg.function()),
                        std::ptr::eq(view.source.function, rich.function()),
                        view.source.ledger,
                        resources.original_ledger_v1(),
                        rich.belongs_to_original_ledger_v1(view.source.ledger),
                        resources.has_denial(),
                    )?;
                    with_retired_lazy_proof_v1(cfg, resources, retired, move |owner| {
                        inspect(view, owner)
                    })
                })
            },
        )
    }
}

#[cfg(test)]
#[path = "bf16_nominal_root_retired_fixed_observation_genuine_v1_tests.rs"]
pub(super) mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_root_retired_fixed_observation_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use genuine::observe_actual_root_retired_fixed_proof_for_test_v1;

#[cfg(test)]
pub(crate) use genuine::observe_actual_root_fixed_prefix_comparison_for_test_v1;
