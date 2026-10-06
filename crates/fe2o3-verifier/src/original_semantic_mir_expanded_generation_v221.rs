//! Production-owned original-source and actual-expanded-target generation.
//! This context emits necessary models and cut predicates, never a proof receipt.
use super::super::invocations::InvocationPlan;
use super::{
    Error, Resource, Result, TargetContracts, Writer,
    byte_bindings::SourceByteBindings,
    expanded_execution::ExpandedExecutionBindingsV199,
    slots::{SourceSlots, SourceTagPairsV40},
    source_function::SourceByteProgram,
    tile_target::{TileMicroCutsV180, TileTargetV176},
};
use fe2o3_kernel_ir::{EndiannessV2, FormalIndexWidth};
use std::{fmt::Write as _, mem::size_of};

#[cfg(test)]
#[path = "original_semantic_mir_expanded_generation_v221_tests.rs"]
mod tests;

pub(super) struct ExpandedGenerationV221<'plan, 'slots, 'view, 'source> {
    plan: &'plan InvocationPlan<'view, 'source>,
    slots: &'slots SourceSlots<'view, 'source>,
    target: TileTargetV176<'slots, 'view, 'source>,
    width: FormalIndexWidth,
    endianness: EndiannessV2,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("expanded generation differs from its retained source or runtime")
}

impl<'plan, 'slots, 'view, 'source> ExpandedGenerationV221<'plan, 'slots, 'view, 'source> {
    fn headers() -> usize {
        size_of::<Self>()
            + 2 * size_of::<Result<Self>>()
            + 2 * size_of::<FormalIndexWidth>()
            + size_of::<EndiannessV2>()
            + size_of::<Option<usize>>()
            + 2 * size_of::<Result<()>>()
            + size_of::<[u64; 3]>()
            + size_of::<
                std::iter::Enumerate<
                    std::slice::Iter<'_, fe2o3_lower_mir_kernel::ProductionSourceLaunchRootV1>,
                >,
            >()
            + 12 * size_of::<usize>()
            + 16 * size_of::<&()>()
    }

    pub(super) fn derive(
        plan: &'plan InvocationPlan<'view, 'source>,
        slots: &'slots SourceSlots<'view, 'source>,
        width: FormalIndexWidth,
        endianness: EndiannessV2,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(Self::headers())?;
            out.budget.charge_work(3)?;
            if width == FormalIndexWidth::Unknown
                || !std::ptr::eq(
                    plan.source(out)?,
                    slots.correspondence(out)?.source(out.budget)?,
                )
            {
                return Err(mismatch());
            }
            let target = TileTargetV176::derive(slots, out)?;
            Ok(Self {
                plan,
                slots,
                target,
                width,
                endianness,
                required: out.budget.storage(),
            })
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.charge_work(2)?;
        if !std::ptr::eq(
            self.plan.source(out)?,
            self.slots.correspondence(out)?.source(out.budget)?,
        ) || !std::ptr::eq(self.slots, self.target.source_slots(out)?)
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(super) fn target(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<&TileTargetV176<'slots, 'view, 'source>> {
        self.check(out)?;
        Ok(&self.target)
    }

    /// Emit real source-generated support before the target and coupled laws.
    /// No synthetic source declarations or original-target inventory substitutes.
    pub(super) fn emit_support(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let relation = self.slots.correspondence(out)?;
            let original = TargetContracts::derive(relation.inventory(out.budget)?, self.width, out)?;
            let tags = SourceTagPairsV40::derive(self.slots, &original, out)?;
            let contracts = TargetContracts::derive(self.target.inventory(out)?, self.width, out)?;
            let cuts = TileMicroCutsV180::derive(&self.target, self.plan, out)?;
            let bytes = SourceByteBindings::derive_expanded_v188(&self.target, out)?;
            let mut program = SourceByteProgram::derive(self.plan, self.slots, out)?;
            super::emit_model_prelude_v187(out)?;
            self.slots.emit_source_tag_contracts(0, out)?;
            tags.emit_expanded_v190(&self.target, &contracts, self.width, 0, 1, out)?;
            self.slots.emit(out)?;
            program.emit(out)?;
            bytes.emit(out)?;
            out.budget.charge_work(2)?;
            let index_bytes = match self.width {
                FormalIndexWidth::Bits32 => 4,
                FormalIndexWidth::Bits64 => 8,
                FormalIndexWidth::Unknown => return Err(mismatch()),
            };
            writeln!(out, "spec fn invocation_runtime_index_bytes_v36() -> int {{ {index_bytes} }}\nspec fn invocation_runtime_little_endian_v36() -> bool {{ {} }}", self.endianness == EndiannessV2::Little)
                .map_err(|_| out.error())?;
            self.target.emit(self.width, out)?;
            cuts.emit(out)?;
            let source = relation.source(out.budget)?;
            let launches = source.source_launch(out.budget)?;
            for root in 0..source.root_count(out.budget)? {
                out.budget.charge_work(1)?;
                let (function, _) = source.root(root, out.budget)?;
                let mut selected = None;
                for (index, launch) in launches.roots().iter().enumerate() {
                    out.budget.charge_work(2)?;
                    if launch.selected_root() == function {
                        if selected.is_some() { return Err(mismatch()); }
                        selected = Some(index);
                    }
                }
                let launch = &launches.roots()[selected.ok_or_else(mismatch)?];
                let [x, y, z] = launch.layout().global_extents();
                let rank = launch.source_rank();
                out.budget.charge_work(8)?;
                if !(1..=3).contains(&rank) || x == 0 || y == 0 || z == 0
                    || (rank < 2 && y != 1) || (rank < 3 && z != 1)
                    || (self.width == FormalIndexWidth::Bits32
                        && [x, y, z].iter().any(|extent| *extent > u64::from(u32::MAX)))
                {
                    return Err(mismatch());
                }
                writeln!(out, "spec fn invocation_runtime_launch_{root}_v36() -> (int, Seq<int>) {{ ({rank}, seq![{x}int, {y}int, {z}int]) }}")
                    .map_err(|_| out.error())?;
                super::emit_execution_v37(relation, root, out)?;
            }
            let execution = ExpandedExecutionBindingsV199::derive(self.plan, self.slots, &self.target, out)?;
            program.emit_context_issue_segments_v222(self.plan, &execution, out)?;
            self.check(out)
        })
    }

    /// Current-frame values only; suspended frames, effects and lifetime history
    /// remain separate obligations and cannot be inferred from this predicate.
    pub(super) fn emit_live_values(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        super::paired::emit_source_cut_values_v213(
            self.plan,
            self.slots,
            &self.target,
            self.width,
            out,
        )?;
        self.check(out)
    }

    pub(super) fn finish(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        super::support_closure::retain_referenced(out)?;
        writeln!(out, "}}").map_err(|_| out.error())?;
        self.check(out)
    }
}
