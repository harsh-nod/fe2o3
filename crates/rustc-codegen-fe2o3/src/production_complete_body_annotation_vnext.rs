//! Move-only annotation transport between live import phases. It removes the
//! temporary preflight-plan borrow, not its provenance: the importer continues
//! to own that plan/transaction and the normal compiler retains its bindings.
//! No Clone/Deserialize/public constructor and no arbitrary record admission.
use crate::production_complete_body_call_vnext::ActualCompleteBodyCallVNext;
use crate::production_complete_body_semantic_source_vnext::{
    ProducedCompleteBodySemanticSourceVNext, produce_complete_body_semantic_source_vnext,
};
use crate::production_complete_body_source_occurrences_vnext::CompleteBodySourceVNext;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, Gfx942CompleteBodyPackedV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::Instance;
/// One named adapter allowance, prepaid on the existing semantic-construction
/// ledger before this helper. It covers the collector's <=1Mi logical replay,
/// source hash/ABI/model work and second raw observation. It is not rustc/RSS.
pub(crate) const PREPAID_CONSTRUCTION_WORK_VNEXT: usize = 2_200_000;
pub(crate) fn prepare_complete_body_annotation_vnext<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    budget: &mut Budget<'_>,
) -> Result<CompleteBodyCallAnnotationVNext<'tcx>, String> {
    use crate::production_complete_body_source_occurrences_vnext::{
        CompleteBodySourceOccurrencesVNext, CompleteBodySourceWorkVNext,
    };
    let [recipe] = plan.terminal_expansion_producers() else {
        return Err("complete-body exact source recipe absent".into());
    };
    let [root] = plan.function_producers() else {
        return Err("complete-body exact source root absent".into());
    };
    // Prepay the collector's independent existing work-only meter on the same
    // parent allowance. No successful traversal is retrospectively charged.
    const COLLECTOR_WORK: usize = 1_048_576;
    budget
        .charge_work(COLLECTOR_WORK)
        .map_err(|e| e.to_string())?;
    let mut work = CompleteBodySourceWorkVNext::new(COLLECTOR_WORK).map_err(str::to_owned)?;
    let mut occurrences = CompleteBodySourceOccurrencesVNext::from_plan(tcx, plan, &mut work)
        .map_err(str::to_owned)?
        .ok_or("complete-body source profile absent")?;
    let body = tcx.instance_mir(root.instance.def);
    let items = crate::production_complete_body_census_vnext::require_body_bounds(body)
        .map_err(str::to_owned)?;
    budget
        .charge_work(
            items
                .checked_mul(3)
                .and_then(|n| n.checked_add(8192))
                .ok_or("complete-body annotation work overflow")?,
        )
        .map_err(|e| e.to_string())?;
    let actual = crate::production_complete_body_census_vnext::observe_root(
        tcx,
        root.instance,
        body,
        recipe.block,
    )
    .map_err(str::to_owned)?;
    let callee = plan
        .function_producers()
        .len()
        .checked_add(recipe.terminal as usize)
        .and_then(|n| u32::try_from(n).ok())
        .map(SemanticCallableIdV1::from_index)
        .ok_or("complete-body current callable position overflow")?;
    let source = occurrences
        .take(tcx, recipe.caller, recipe.block, callee, actual, &mut work)
        .map_err(str::to_owned)?;
    occurrences.require_drained().map_err(str::to_owned)?;
    CompleteBodyCallAnnotationVNext::from_source(source, budget)
}

pub(crate) struct CompleteBodyCallAnnotationVNext<'tcx> {
    root: Instance<'tcx>,
    terminal: Instance<'tcx>,
    key: (SemanticFunctionIdV1, u32, SemanticCallableIdV1),
    packed: Gfx942CompleteBodyPackedV1,
    locals: [u32; 5],
    semantic_locals: [SemanticLocalIdV1; 5],
    semantic_block: SemanticBlockIdV1,
    moved: u8,
    pending: Option<ProducedCompleteBodySemanticSourceVNext>,
}
impl<'tcx> CompleteBodyCallAnnotationVNext<'tcx> {
    /// Source token originates only in the exact authenticated occurrence table.
    /// Drop on error discards the consumed token; no retry or fallback.
    pub(crate) fn from_source(
        source: CompleteBodySourceVNext<'_, 'tcx>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, String> {
        let [root] = source.plan().function_producers() else {
            return Err("complete-body annotation root roster differs".into());
        };
        let root = root.instance;
        let terminal = source.terminal_instance();
        let key = source.occurrence_key_vnext();
        let packed = source.packed();
        let locals = source.argument_locals();
        let moved = source.moved_arguments_vnext();
        let [retained] = source.plan().body_producers() else {
            return Err("complete-body actual source body mapping absent".into());
        };
        if retained.function != key.0 {
            return Err("complete-body source body mapping names another function".into());
        }
        let mut semantic_locals = [SemanticLocalIdV1::from_index(0); 5];
        for (ordinal, raw) in locals.iter().copied().enumerate() {
            semantic_locals[ordinal] =
                mapped_coordinate_vnext(raw, &retained.raw_to_semantic_locals, |mapped| {
                    retained
                        .locals
                        .get(mapped.index() as usize)
                        .map(|row| row.rustc_local)
                })?;
        }
        let semantic_block =
            mapped_coordinate_vnext(key.1, &retained.raw_to_semantic_blocks, |mapped| {
                retained
                    .blocks
                    .get(mapped.index() as usize)
                    .map(|row| row.rustc_block)
            })?;
        if retained
            .blocks
            .get(semantic_block.index() as usize)
            .is_none_or(|row| {
                row.rustc_block != key.1 || row.identity != source.semantic_block_identity()
            })
        {
            return Err("complete-body source block mapping differs".into());
        }
        let pending = produce_complete_body_semantic_source_vnext(source, budget)?;
        Ok(Self {
            root,
            terminal,
            key,
            packed,
            locals,
            semantic_locals,
            semantic_block,
            moved,
            pending: Some(pending),
        })
    }

    /// ActualCompleteBodyCallVNext is freshly observed by the normal body
    /// producer from its own root MIR/func/ten operands, not decoded from JSON.
    /// All checks precede slot consumption. A failed importer is abandoned even
    /// though this method leaves its slot unconsumed for deterministic diagnostics.
    pub(crate) fn attach(
        &mut self,
        current_root: Instance<'tcx>,
        key: (SemanticFunctionIdV1, u32, SemanticCallableIdV1),
        semantic_block: SemanticBlockIdV1,
        actual: &ActualCompleteBodyCallVNext<'tcx>,
        mut call: SemanticDirectCallV1,
    ) -> Result<SemanticDirectCallV1, &'static str> {
        let pending = self
            .pending
            .as_ref()
            .ok_or("complete-body annotation already consumed")?;
        if current_root != self.root
            || key != self.key
            || semantic_block != self.semantic_block
            || call.callee() != key.2
            || actual.instance() != self.terminal
            || actual.packed() != self.packed
            || actual.registers() != pending.registers
            || actual.argument_locals() != self.locals
            || actual.moved_arguments() != self.moved
            || pending.packing.block_count != self.packed.block_count()
            || pending.packing.instruction_count != self.packed.instruction_count()
            || pending.packing.block_words != self.packed.block_words()
            || pending.packing.instruction_words != self.packed.instruction_words()
            || call.arguments().len() != 10
            || call.complete_body_source_vnext().is_some()
            || call.inline_assembly_source_v30().is_some()
            || call.ordered_region_source_v31().is_some()
            || call.ordered_program_source_v32().is_some()
        {
            return Err("complete-body current raw/semantic call binding differs");
        }
        for (ordinal, operand) in call.arguments()[..5].iter().enumerate() {
            let (place, moved) = match operand {
                SemanticOperandV1::Copy(place) => (place, false),
                SemanticOperandV1::Move(place) => (place, true),
                _ => return Err("complete-body emitted source input is not direct transport"),
            };
            if !place.projections().is_empty()
                || place.local() != self.semantic_locals[ordinal]
                || moved != (self.moved & (1 << ordinal) != 0)
            {
                return Err("complete-body emitted source operand changed");
            }
        }
        for (ordinal, operand) in call.arguments()[5..].iter().enumerate() {
            let SemanticOperandV1::Constant(constant) = operand else {
                return Err("complete-body emitted register is not a literal");
            };
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err("complete-body emitted register is not scalar");
            };
            if value.size_bytes() != 1 || value.bits() != u128::from(pending.registers[ordinal]) {
                return Err("complete-body emitted register differs");
            }
        }
        let pending = self
            .pending
            .take()
            .ok_or("complete-body annotation disappeared")?;
        call = call.with_complete_body_source_vnext(pending.source);
        Ok(call)
    }
    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        if self.pending.is_none() {
            Ok(())
        } else {
            Err("complete-body source annotation unconsumed")
        }
    }
}

/// Reconciles two coordinate spaces against the retained reverse row.
/// Pure position validation only; this helper creates no source/owner custody.
fn mapped_coordinate_vnext<T: Copy>(
    raw: u32,
    raw_to_semantic: &[T],
    row_raw: impl FnOnce(T) -> Option<u32>,
) -> Result<T, &'static str> {
    let mapped = *raw_to_semantic
        .get(raw as usize)
        .ok_or("complete-body raw-to-semantic coordinate absent")?;
    if row_raw(mapped) != Some(raw) {
        return Err("complete-body raw-to-semantic coordinate differs");
    }
    Ok(mapped)
}

#[cfg(test)]
mod coordinate_tests {
    use super::mapped_coordinate_vnext;
    #[test]
    fn permuted_raw_and_semantic_positions_are_not_equal() {
        let mapping = [3usize, 1, 0, 2];
        let rows = [2u32, 1, 3, 0];
        for raw in 0..4 {
            let mapped =
                mapped_coordinate_vnext(raw, &mapping, |semantic| rows.get(semantic).copied())
                    .unwrap();
            assert_eq!(mapped, mapping[raw as usize]);
        }
        assert_eq!(
            mapped_coordinate_vnext(0, &mapping, |semantic| rows.get(semantic).copied()).unwrap(),
            3
        );
    }
    #[test]
    fn stale_missing_and_raw_substituted_mappings_refuse() {
        let mapping = [3usize, 1, 0, 2];
        let rows = [2u32, 1, 3, 0];
        assert!(
            mapped_coordinate_vnext(4, &mapping, |semantic| rows.get(semantic).copied()).is_err()
        );
        for bad in [[0, 1, 2, 3], [1, 3, 0, 2], [9, 1, 0, 2]] {
            assert!(
                mapped_coordinate_vnext(0, &bad, |semantic| rows.get(semantic).copied()).is_err()
            );
        }
    }
}
