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

pub(super) fn check_runtime_domain_v280(
    width: FormalIndexWidth,
    rank: u8,
    [x, y, z]: [u64; 3],
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<()> {
    budget.charge_work(8)?;
    if width == FormalIndexWidth::Unknown
        || !(1..=3).contains(&rank)
        || x == 0
        || y == 0
        || z == 0
        || (rank < 2 && y != 1)
        || (rank < 3 && z != 1)
        || (width == FormalIndexWidth::Bits32
            && [x, y, z].iter().any(|extent| *extent > u64::from(u32::MAX)))
    {
        return Err(mismatch());
    }
    Ok(())
}

impl<'plan, 'slots, 'view, 'source> ExpandedGenerationV221<'plan, 'slots, 'view, 'source> {
    fn headers() -> usize {
        size_of::<Self>()
            + 2 * size_of::<Result<Self>>()
            + 2 * size_of::<FormalIndexWidth>()
            + size_of::<EndiannessV2>()
            + size_of::<Option<&[super::expanded_model_v280::ExpandedSupportRuntimeV280]>>()
            + size_of::<Result<(u8, [u64; 3])>>()
            + size_of::<u8>()
            + 2 * size_of::<(
                &Self,
                &TileMicroCutsV180<'_, 'slots, 'view, 'source>,
                Option<&[super::expanded_model_v280::ExpandedSupportRuntimeV280]>,
            )>()
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
        let cuts = TileMicroCutsV180::derive(&self.target, self.plan, out)?;
        self.emit_support_with_cuts_v280(&cuts, None, out)
    }

    pub(super) fn emit_support_with_cuts_v280(
        &self,
        cuts: &TileMicroCutsV180<'_, 'slots, 'view, 'source>,
        runtime: Option<&[super::expanded_model_v280::ExpandedSupportRuntimeV280]>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            cuts.check_target_v280(&self.target, out)?;
            let relation = self.slots.correspondence(out)?;
            let original = TargetContracts::derive(relation.inventory(out.budget)?, self.width, out)?;
            let tags = SourceTagPairsV40::derive(self.slots, &original, out)?;
            let contracts = TargetContracts::derive(self.target.inventory(out)?, self.width, out)?;
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
            let count = source.root_count(out.budget)?;
            out.budget.charge_work(2)?;
            if launches.roots().len() != count
                || runtime.is_some_and(|rows| rows.len() != count)
            {
                return Err(mismatch());
            }
            for (root, launch) in launches.roots().iter().enumerate() {
                out.budget.charge_work(1)?;
                let (function, _) = source.root(root, out.budget)?;
                if launch.selected_root() != function {
                    return Err(mismatch());
                }
                let (rank, [x, y, z]) = match runtime {
                    Some(rows) => rows[root].checked_launch(launch, out.budget)?,
                    None => (launch.source_rank(), launch.layout().global_extents()),
                };
                check_runtime_domain_v280(self.width, rank, [x, y, z], out.budget)?;
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

    pub(super) fn emit_frame_contracts_v281(
        &self,
        frames: &super::source_frame_plan::FramePlan<'_, '_, '_, '_>,
        cuts: &TileMicroCutsV180<'_, 'slots, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        super::expanded_frame_contract::emit(
            frames,
            self.plan,
            self.slots,
            &self.target,
            cuts,
            self.width,
            out,
        )?;
        self.check(out)
    }
}
