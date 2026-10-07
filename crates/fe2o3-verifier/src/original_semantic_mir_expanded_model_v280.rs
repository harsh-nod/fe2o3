//! Scoped support text and static coordinates from the retained expanded owners.
//! Neither the text nor the observations are a refinement or execution receipt.
use super::super::invocations::{CallKind, InvocationPlan};
pub use super::forwarding_observation::ExpandedSupportForwardingV288;
use super::{
    Error, Resource, Result, Writer, expanded_generation::ExpandedGenerationV221,
    slots::SourceSlots, source_frame_plan::FramePlan, tile_target::TileMicroCutsV180, vector,
};
use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, CanonicalKirFunctionCoordinateV1 as Function,
    EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceCorrespondenceV18 as Original, ProductionSourceExecutionLayoutV1,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootV1,
    ProductionSourceOwnedViewV18 as Source, ProductionSourceTileExpansionV159 as Tile,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use std::{
    mem::{align_of, align_of_val, size_of, size_of_val},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[cfg(test)]
#[path = "original_semantic_mir_expanded_model_v280_tests.rs"]
mod tests;

/// Descriptive runtime inputs. The compiler retains the authenticated descriptor;
/// constructing this row does not authenticate a launch or grant proof authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedSupportRuntimeV280 {
    /// Semantic root in the complete original order.
    pub source_root: SemanticFunctionIdV1,
    /// Exact retained source contract, including dynamic-layout inputs.
    pub source_launch: ProductionSourceLaunchInputV1,
    /// Exact retained source interpretation; zero remains a dynamic sentinel.
    pub source_layout: ProductionSourceExecutionLayoutV1,
    /// Separately authenticated physical coordinate envelope.
    pub physical: ExplicitLaunchExtent,
}

impl ExpandedSupportRuntimeV280 {
    pub(super) fn checked_launch(
        &self,
        retained: &ProductionSourceLaunchRootV1,
        budget: &mut Budget<'_>,
    ) -> Result<(u8, [u64; 3])> {
        // Prepay every fixed raw/layout field and all three physical products.
        budget.charge_work(32)?;
        if self.source_root != retained.selected_root()
            || self.source_launch != retained.source_launch()
            || self.source_layout != retained.layout()
        {
            return Err(mismatch());
        }
        let ExplicitLaunchExtent::Exact { rank, extents } = self.physical else {
            return Err(mismatch());
        };
        if rank != retained.source_rank() {
            return Err(mismatch());
        }
        let workgroup = self.source_launch.exact_workgroup().ok_or_else(mismatch)?;
        for (axis, groups) in self.source_launch.max_grid().into_iter().enumerate() {
            let expected = u64::from(workgroup[axis])
                .checked_mul(u64::from(groups))
                .ok_or(Resource::Arithmetic)?;
            if extents[axis] != expected {
                return Err(mismatch());
            }
        }
        Ok((rank, extents))
    }
}

/// A complete original root and its actual expanded target entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSupportRootV280 {
    /// Semantic source function ordinal.
    pub source_function: u32,
    /// Original canonical function ordinal.
    pub original_function: usize,
    /// Actual expanded canonical function coordinate.
    pub target_function: Function,
    /// Complete range in the instance roster.
    pub instances: Range<usize>,
    /// Complete range in the cut roster, including empty cuts.
    pub cuts: Range<usize>,
}

/// A static retained call instance. Inactive is not a suspended-frame state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSupportInstanceV280 {
    /// Original root ordinal.
    pub root: usize,
    /// Root-relative instance ordinal.
    pub instance: usize,
    /// Semantic source function ordinal.
    pub source_function: u32,
    /// Exact incoming root-relative caller instance and semantic call block.
    pub incoming: Option<(usize, u32)>,
    /// Whether this instance is retained as active by source import.
    pub active: bool,
    /// Logical local range, not a physical allocation range.
    pub locals: Range<usize>,
    /// Complete original block/cut range.
    pub cuts: Range<usize>,
    /// Range in the complete call roster.
    pub calls: Range<usize>,
    /// Exact incoming global call row; not a dynamic suspended-frame witness.
    pub parent_call_v281: Option<usize>,
    /// Original call-frame depth, distinct from target execution frames.
    pub depth_v281: usize,
}

/// Original call class, independent of whether an expanded continuation exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpandedSupportCallKindV280 {
    /// Ordinary call with a continuation.
    Direct,
    /// Tail call.
    Tail,
    /// Drop call.
    Drop,
}

/// Exact static callsite and authenticated child association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSupportCallV280 {
    /// Original root ordinal.
    pub root: usize,
    /// Root-relative caller instance.
    pub caller: usize,
    /// Original semantic block ordinal.
    pub block: u32,
    /// Original callable ordinal.
    pub callable: u32,
    /// Original call class.
    pub kind: ExpandedSupportCallKindV280,
    /// Source SSA reachability.
    pub reachable: bool,
    /// Root-relative authenticated child, when retained.
    pub child: Option<usize>,
    /// Original continuation block for ordinary calls.
    pub continuation_v281: Option<usize>,
    /// Shared caller demands while this exact child is executing.
    pub carries_v281: Range<usize>,
}

/// One original block entry, including entries with no target candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSupportCutV280 {
    /// Original root ordinal.
    pub root: usize,
    /// Root-relative retained instance.
    pub instance: usize,
    /// Original semantic block ordinal.
    pub block: u32,
    /// Range in the actual expanded cursor roster.
    pub candidates: Range<usize>,
    /// Range in the zero-step edge roster.
    pub zero_edges: Range<usize>,
    /// Existing bounded zero-step rank, not a whole-execution termination proof.
    pub zero_rank: usize,
    /// Current-frame original demands; inactive/unreachable cuts remain empty.
    pub current_v281: Range<usize>,
}

/// An original SSA obligation, not an expanded target definition or proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedSupportFrameDemandV281 {
    /// Original root ordinal.
    pub root: usize,
    /// Root-local original instance.
    pub instance: usize,
    /// Local within the original semantic function.
    pub local: usize,
    /// Global source-interpreter logical coordinate.
    pub logical_local: usize,
    /// Exact retained original SSA endpoint.
    pub value: fe2o3_mir_model::SsaValueV1,
    /// Original entry/continuation at which component liveness was queried.
    pub component_block: usize,
    /// Exact leaf range overwritten by a projected return, if any.
    pub overwritten: Option<(usize, usize)>,
}

/// Exact expanded block boundary, not a bounding interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedSupportCursorV280 {
    /// Actual expanded inventory block ordinal.
    pub block: usize,
    /// Actual expanded inventory operation ordinal; None is the terminator gap.
    pub operation: Option<usize>,
    /// Operation prefix in the actual block.
    pub prefix: usize,
}

/// Borrowed static rows. Incoming links describe ancestry, not suspended values.
pub struct ExpandedSupportCensusV280<'a> {
    /// Complete original-root/expanded-entry roster.
    pub roots: &'a [ExpandedSupportRootV280],
    /// Complete retained instances, including inactive instances.
    pub instances: &'a [ExpandedSupportInstanceV280],
    /// Complete original callsite roster.
    pub calls: &'a [ExpandedSupportCallV280],
    /// Every original instance/block, including zero-candidate cuts.
    pub cuts: &'a [ExpandedSupportCutV280],
    /// Actual expanded cursor candidates.
    pub candidates: &'a [ExpandedSupportCursorV280],
    /// Existing coincident-cursor edges, as global cut ordinals.
    pub zero_edges: &'a [(usize, usize)],
    /// Current and shared suspended-caller requirements, not temporal evidence.
    pub frame_demands_v281: &'a [ExpandedSupportFrameDemandV281],
    /// Whole-Slice paths observed during actual frame binding emission.
    pub forwarding_v288: &'a [ExpandedSupportForwardingV288],
}

struct Census {
    roots: Vec<ExpandedSupportRootV280>,
    instances: Vec<ExpandedSupportInstanceV280>,
    calls: Vec<ExpandedSupportCallV280>,
    cuts: Vec<ExpandedSupportCutV280>,
    candidates: Vec<ExpandedSupportCursorV280>,
    zero_edges: Vec<(usize, usize)>,
    frame_demands: Vec<ExpandedSupportFrameDemandV281>,
    forwarding: Vec<ExpandedSupportForwardingV288>,
}

impl Census {
    fn derive(
        plan: &InvocationPlan<'_, '_>,
        model: &ExpandedGenerationV221<'_, '_, '_, '_>,
        cuts: &TileMicroCutsV180<'_, '_, '_, '_>,
        frames: &FramePlan<'_, '_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        frames.check(plan, model.target(out)?.source_slots(out)?, out)?;
        let source = plan.source(out)?;
        let count = source.root_count(out.budget)?;
        let rows = cuts.static_rows_v280(out)?;
        out.budget.charge_work(2)?;
        if rows.roots.len() != count
            || rows.rank.len() != rows.cuts.len()
            || frames.roots.len() != count
            || frames.cuts.len() != rows.cuts.len()
        {
            return Err(mismatch());
        }
        let (mut instances, mut calls) = (0usize, 0usize);
        for root in 0..count {
            let scope = plan.root(root, out)?;
            instances = add(instances, scope.instances.len())?;
            for instance in 0..scope.instances.len() {
                out.budget.charge_work(1)?;
                calls = add(calls, plan.calls(root, instance, out)?.len())?;
            }
        }
        let mut census = Self {
            roots: vector(count, out)?,
            instances: vector(instances, out)?,
            calls: vector(calls, out)?,
            cuts: vector(rows.cuts.len(), out)?,
            candidates: vector(rows.candidates.len(), out)?,
            zero_edges: vector(rows.zero_edges.len(), out)?,
            frame_demands: vector(frames.demands.len(), out)?,
            forwarding: Vec::new(),
        };
        for (root, range) in rows.roots.iter().enumerate() {
            let scope = plan.root(root, out)?;
            if scope.instances.start != census.instances.len() {
                return Err(mismatch());
            }
            census.roots.push(ExpandedSupportRootV280 {
                source_function: scope.function.index(),
                original_function: scope.physical,
                target_function: model.target(out)?.root_function(root, out)?,
                instances: scope.instances.clone(),
                cuts: range.clone(),
            });
            for instance in 0..scope.instances.len() {
                out.budget.charge_work(1)?;
                let row = plan.instance(root, instance, out)?;
                let frame = frames
                    .frames
                    .get(census.instances.len())
                    .ok_or_else(mismatch)?;
                if (frame.root, frame.instance, frame.function, frame.active)
                    != (root, instance, row.function, row.active)
                    || frame.locals != row.locals
                {
                    return Err(mismatch());
                }
                let first = census.calls.len();
                for call in plan.calls(root, instance, out)? {
                    out.budget.charge_work(1)?;
                    let carry = frames.calls.get(census.calls.len()).ok_or_else(mismatch)?;
                    if (
                        carry.root,
                        carry.caller,
                        carry.block,
                        carry.child,
                        carry.kind,
                        carry.reachable,
                    ) != (
                        root,
                        call.caller,
                        call.block.index() as usize,
                        call.child,
                        call.kind,
                        call.ssa_reachable,
                    ) {
                        return Err(mismatch());
                    }
                    census.calls.push(ExpandedSupportCallV280 {
                        root,
                        caller: call.caller,
                        block: call.block.index(),
                        callable: call.callable.index(),
                        kind: match call.kind {
                            CallKind::Direct => ExpandedSupportCallKindV280::Direct,
                            CallKind::Tail => ExpandedSupportCallKindV280::Tail,
                            CallKind::Drop => ExpandedSupportCallKindV280::Drop,
                        },
                        reachable: call.ssa_reachable,
                        child: call.child,
                        continuation_v281: carry.continuation,
                        carries_v281: carry.demands.clone(),
                    });
                }
                census.instances.push(ExpandedSupportInstanceV280 {
                    root,
                    instance,
                    source_function: row.function.index(),
                    incoming: row.incoming.map(|(caller, block)| (caller, block.index())),
                    active: row.active,
                    locals: row.locals.clone(),
                    cuts: row.blocks.clone(),
                    calls: first..census.calls.len(),
                    parent_call_v281: frame.parent_call,
                    depth_v281: frame.depth,
                });
            }
            if range.start != census.cuts.len() {
                return Err(mismatch());
            }
            for pc in range.clone() {
                out.budget.charge_work(1)?;
                let cut = rows.cuts.get(pc).ok_or_else(mismatch)?;
                let current = frames.cuts.get(pc).ok_or_else(mismatch)?;
                if (current.root, current.instance, current.block, current.pc)
                    != (root, cut.instance, cut.block.index() as usize, pc)
                {
                    return Err(mismatch());
                }
                census.cuts.push(ExpandedSupportCutV280 {
                    root,
                    instance: cut.instance,
                    block: cut.block.index(),
                    candidates: cut.candidates.clone(),
                    zero_edges: cut.edges.clone(),
                    zero_rank: *rows.rank.get(pc).ok_or_else(mismatch)?,
                    current_v281: current.demands.clone(),
                });
            }
        }
        for row in rows.candidates {
            out.budget.charge_work(1)?;
            census.candidates.push(ExpandedSupportCursorV280 {
                block: row.block,
                operation: row.operation,
                prefix: row.prefix,
            });
        }
        for &edge in rows.zero_edges {
            out.budget.charge_work(1)?;
            census.zero_edges.push(edge);
        }
        for demand in &frames.demands {
            out.budget.charge_work(2)?;
            let frame = frames.frames.get(demand.frame).ok_or_else(mismatch)?;
            if demand.source.local >= frame.locals.len() {
                return Err(mismatch());
            }
            census.frame_demands.push(ExpandedSupportFrameDemandV281 {
                root: frame.root,
                instance: frame.instance,
                local: demand.source.local,
                logical_local: add(frame.locals.start, demand.source.local)?,
                value: demand.source.value,
                component_block: demand.source.components.block,
                overwritten: demand.source.components.overwritten,
            });
        }
        if census.instances.len() != instances
            || census.calls.len() != calls
            || census.cuts.len() != rows.cuts.len()
        {
            return Err(mismatch());
        }
        Ok(census)
    }

    fn view(&self) -> ExpandedSupportCensusV280<'_> {
        ExpandedSupportCensusV280 {
            roots: &self.roots,
            instances: &self.instances,
            calls: &self.calls,
            cuts: &self.cuts,
            candidates: &self.candidates,
            zero_edges: &self.zero_edges,
            frame_demands_v281: &self.frame_demands,
            forwarding_v288: &self.forwarding,
        }
    }
}

/// A scoped, non-authoritative support model. It cannot outlive the owner scope.
///
/// Borrowed text and census cannot become static output:
/// ```compile_fail
/// use fe2o3_verifier::ExpandedSupportModelV280;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(model: &ExpandedSupportModelV280<'_, '_>, budget: &mut Budget<'_>) -> &'static [u8] {
///     model.generated_source(budget).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::{ExpandedSupportModelV280, ExpandedSupportCensusV280};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(model: &ExpandedSupportModelV280<'_, '_>, budget: &mut Budget<'_>) -> ExpandedSupportCensusV280<'static> {
///     model.census(budget).unwrap()
/// }
/// ```
/// The support model is not an existing nominal prepared-proof request:
/// ```compile_fail
/// use fe2o3_verifier::{ExpandedSupportModelV280, PreparedOriginalSemanticMirRefinementV36};
/// fn proof(model: &ExpandedSupportModelV280<'_, '_>) {
///     let _: &PreparedOriginalSemanticMirRefinementV36<'_, '_> = model;
/// }
/// ```
/// Nor is it an executed refinement:
/// ```compile_fail
/// use fe2o3_verifier::{ExpandedSupportModelV280, ExecutedMixedComposedRefinementV29};
/// fn executed(model: &ExpandedSupportModelV280<'_, '_>) {
///     let _: &ExecutedMixedComposedRefinementV29<'_, '_, '_, '_, '_, '_> = model;
/// }
/// ```
pub struct ExpandedSupportModelV280<'model, 'source> {
    source: &'model Source<'source>,
    bytes: &'model [u8],
    census: &'model Census,
    ledger: Ledger,
    slot: usize,
    required: usize,
}

impl ExpandedSupportModelV280<'_, '_> {
    fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.source.check_query_v18(budget)?;
        budget.charge_work(1)?;
        if self.slot != std::ptr::from_ref(&*budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            return Err(self
                .source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    /// Exact emitted support bytes, not a complete paired theorem or receipt.
    pub fn generated_source(&self, budget: &mut Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(self.bytes)
    }

    /// Exact static rows. Reading them does not establish dynamic frame laws.
    pub fn census(&self, budget: &mut Budget<'_>) -> Result<ExpandedSupportCensusV280<'_>> {
        self.check(budget)?;
        Ok(self.census.view())
    }
}

fn mismatch() -> Error {
    Error::Statement("expanded support differs from its retained owners or runtime")
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}

// A trusted callback's destructor must not replace the selected refusal/unwind.
struct Pending<T>(Option<T>);
impl<T> Pending<T> {
    fn take(&mut self) -> T {
        self.0.take().expect("single expanded support consumer")
    }
}
impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if let Err(mut payload) = catch_unwind(AssertUnwindSafe(|| drop(self.0.take()))) {
            while let Err(next) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                payload = next;
            }
        }
    }
}

fn check_owners(
    source: &Source<'_>,
    original: &Original<'_>,
    tile: &Tile<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    source.check_query_v18(budget)?;
    budget.charge_work(2)?;
    if !std::ptr::eq(original.source(budget)?, source)
        || !std::ptr::eq(tile.original_source_v162(budget)?, original)
    {
        return Err(mismatch());
    }
    Ok(())
}

fn check_runtime(
    source: &Source<'_>,
    runtime: &[ExpandedSupportRuntimeV280],
    width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let launches = source.source_launch(budget)?;
    budget.charge_work(2)?;
    if runtime.len() != source.root_count(budget)? || runtime.len() != launches.roots().len() {
        return Err(mismatch());
    }
    for (root, (input, retained)) in runtime.iter().zip(launches.roots()).enumerate() {
        budget.charge_work(1)?;
        if retained.selected_root() != source.root(root, budget)?.0 {
            return Err(mismatch());
        }
        let (rank, extents) = input.checked_launch(retained, budget)?;
        super::expanded_generation::check_runtime_domain_v280(width, rank, extents, budget)?;
    }
    Ok(())
}

/// Emits support from exact retained owners and calls a consumer while all owners
/// and text remain live. Runtime rows are descriptive, not caller authority; the
/// compiler must retain and recheck its original runtime/reference binding owner.
///
/// The consumer returns its exact additional backing storage, excluding R's
/// prepaid inline carrier. Scratch is dropped and refunded before that backing
/// plus `size_of::<R>()` is re-reserved; the returned byte count is this complete
/// payload credit. No borrowed view can escape
/// through the static result/error. Consumer errors and panic payloads are retained.
///
/// The scoped callback cannot return borrowed text, even when every input owner
/// outlives the call:
/// ```compile_fail
/// use fe2o3_verifier::{with_expanded_support_model_v280, ExpandedSupportRuntimeV280};
/// use fe2o3_lower_mir_kernel::{ProductionSourceOwnedViewV18 as Source,
///     ProductionSourceCorrespondenceV18 as Original, ProductionSourceTileExpansionV159 as Tile};
/// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
///     FormalIndexWidth, EndiannessV2};
/// fn escape(source: &Source<'_>, original: &Original<'_>, tile: &Tile<'_, '_>,
///     runtime: &[ExpandedSupportRuntimeV280], budget: &mut Budget<'_>) {
///     let _ = with_expanded_support_model_v280::<&'static [u8], (), _>(source,
///         original, tile, runtime, FormalIndexWidth::Bits32, EndiannessV2::Little,
///         budget, |model, budget| Ok((model.generated_source(budget).unwrap(), 0)));
/// }
/// ```
/// The complete typed census has the same scoped lifetime:
/// ```compile_fail
/// use fe2o3_verifier::{with_expanded_support_model_v280, ExpandedSupportRuntimeV280,
///     ExpandedSupportCensusV280};
/// use fe2o3_lower_mir_kernel::{ProductionSourceOwnedViewV18 as Source,
///     ProductionSourceCorrespondenceV18 as Original, ProductionSourceTileExpansionV159 as Tile};
/// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
///     FormalIndexWidth, EndiannessV2};
/// fn escape(source: &Source<'_>, original: &Original<'_>, tile: &Tile<'_, '_>,
///     runtime: &[ExpandedSupportRuntimeV280], budget: &mut Budget<'_>) {
///     let _ = with_expanded_support_model_v280::<ExpandedSupportCensusV280<'static>, (), _>(
///         source, original, tile, runtime, FormalIndexWidth::Bits32, EndiannessV2::Little,
///         budget, |model, budget| Ok((model.census(budget).unwrap(), 0)));
/// }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn with_expanded_support_model_v280<R: 'static, E: 'static, F>(
    source: &Source<'_>,
    original: &Original<'_>,
    tile: &Tile<'_, '_>,
    runtime: &[ExpandedSupportRuntimeV280],
    width: FormalIndexWidth,
    endian: EndiannessV2,
    budget: &mut Budget<'_>,
    consume: F,
) -> Result<std::result::Result<(R, usize), E>>
where
    F: for<'model, 'source, 'work> FnOnce(
        &ExpandedSupportModelV280<'model, 'source>,
        &mut Budget<'work>,
    ) -> std::result::Result<(R, usize), E>,
{
    let mut pending = Pending(Some(consume));
    check_owners(source, original, tile, budget)?;
    let floor = budget.storage();
    let mut consumer_error = Pending(None);
    let produce = |budget: &mut Budget<'_>| {
        check_runtime(source, runtime, width, budget)?;
        let mut out = Writer::new(budget)?;
        let actual_source = tile.original_source_v162(out.budget)?.source(out.budget)?;
        let plan = InvocationPlan::derive(actual_source, &mut out)?;
        let slots = SourceSlots::derive_tile_v162(&plan, tile, &mut out)?;
        let model = ExpandedGenerationV221::derive(&plan, &slots, width, endian, &mut out)?;
        let cuts = TileMicroCutsV180::derive(model.target(&mut out)?, &plan, &mut out)?;
        let frames = FramePlan::derive(&plan, &slots, &mut out)?;
        let mut census = Census::derive(&plan, &model, &cuts, &frames, &mut out)?;
        model.emit_support_with_cuts_v280(&cuts, Some(runtime), &mut out)?;
        let mut observe = |event, out: &mut Writer<'_, '_>| {
            super::forwarding_observation::append(&mut census.forwarding, event, out)
        };
        let observer_headers = (2 * size_of_val(&observe))
            .checked_add(align_of_val(&observe))
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(observer_headers)?;
        model.emit_frame_contracts_observed_v288(&frames, &cuts, &mut out, &mut observe)?;
        model.finish(&mut out)?;
        check_owners(source, original, tile, out.budget)?;
        let view = ExpandedSupportModelV280 {
            source,
            bytes: out.text.as_bytes(),
            census: &census,
            ledger: out.budget.work_ledger_identity_v1(),
            slot: std::ptr::from_ref(&*out.budget) as usize,
            required: out.budget.storage(),
        };
        let callback_floor = out.budget.storage();
        let result = match pending.take()(&view, out.budget) {
            Ok(value) => value,
            Err(error) => {
                consumer_error.0 = Some(error);
                return Err(Error::Statement("expanded support consumer refused"));
            }
        };
        let result = Pending(Some(result));
        // Preserve the original consumer error rather than replace it with a
        // later query/account error. The enclosing scope still checks custody.
        if let Some((_, bytes)) = &result.0 {
            view.check(out.budget)?;
            check_owners(source, original, tile, out.budget)?;
            if out.budget.storage().checked_sub(callback_floor) != Some(*bytes) {
                return Err(source
                    .retain_query_resource_error_v18(Resource::Accounting)
                    .into());
            }
        }
        Ok(result)
    };
    let headers = [
        SOURCE_LIMIT,
        2 * size_of_val(&produce),
        align_of_val(&produce),
        size_of::<Writer<'_, '_>>(),
        size_of::<Census>(),
        super::forwarding_observation::headers(),
        size_of::<ExpandedSupportModelV280<'_, '_>>(),
        size_of::<ExpandedSupportCensusV280<'_>>(),
        size_of::<Pending<F>>(),
        2 * size_of::<Pending<(R, usize)>>(),
        size_of::<Pending<E>>(),
        2 * size_of::<Result<std::result::Result<(R, usize), E>>>(),
        2 * size_of::<Result<Pending<(R, usize)>>>(),
        2 * size_of::<Result<()>>(),
        24 * size_of::<usize>(),
        2 * size_of::<ExpandedSupportRootV280>(),
        2 * size_of::<ExpandedSupportInstanceV280>(),
        2 * size_of::<ExpandedSupportCallV280>(),
        2 * size_of::<ExpandedSupportCutV280>(),
        2 * size_of::<ExpandedSupportCursorV280>(),
        2 * size_of::<ExpandedSupportFrameDemandV281>(),
        size_of::<FramePlan<'_, '_, '_, '_>>(),
        size_of::<ExpandedSupportRuntimeV280>(),
        2 * size_of::<Range<usize>>(),
        size_of::<Result<Census>>(),
        size_of::<super::tile_target::StaticCutsV280<'_>>(),
        align_of::<R>(),
        size_of::<R>(),
        size_of::<E>(),
    ]
    .into_iter()
    .try_fold(0usize, add)?;
    let result = budget.with_prepaid_scope(floor, 1, 1, headers, produce);
    if consumer_error.0.is_some() {
        return Ok(Err(consumer_error.take()));
    }
    let mut result = result.map_err(|error| match error {
        Error::Resource(resource) => source.retain_query_resource_error_v18(resource).into(),
        error => error,
    })?;
    if let Some((_, bytes)) = &mut result.0 {
        *bytes = bytes
            .checked_add(size_of::<R>())
            .ok_or(Resource::Arithmetic)?;
        budget
            .reserve_storage(*bytes)
            .map_err(|error| source.retain_query_resource_error_v18(error))?;
    }
    Ok(Ok(result.take()))
}
