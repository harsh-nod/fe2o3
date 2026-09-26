//! Integrated actual retained-input/root-prefix/index preparation. This is one
//! physical root-assembly prefix, not a separate index namespace or ready token.
//! Actual access appends/origins/sites/Final/full CFG assembly remain pending.
use super::*;
use crate::production_ranked_projection_v1::*;
use crate::production_ranked_projection_v1::{
    root_entry_prefix_preparation_v1::{RootEntryPrefixV1, prepare_root_entry_prefix_paid_v1},
    root_invocation_index_preparation_v1::{
        RootInvocationIndexStorageV1, prepare_root_namespace_indices_v1,
    },
};
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, AuthenticatedReferenceEffectBindingsV1,
};
use fe2o3_lower_mir_kernel::{CheckedBf16NominalCallV1, ProductionPreRankedKirOwnerV1};

/// Must exist physically outside rich/facts/context/graph callbacks. It owns
/// actual entry operations/SSA counter; future access/CFG preparation extends
/// this SAME assembly. No conversion from unjoined component data is provided.
pub(in crate::production_ranked_projection_v1) struct PendingActualRootPrefixIndicesV1 {
    graph: PendingNominalInitialGraphV1,
    prefix: RootEntryPrefixV1,
    indices: RootInvocationIndexStorageV1,
    ledger: Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>,
    started: bool,
    completed: bool,
}
impl PendingActualRootPrefixIndicesV1 {
    pub(in crate::production_ranked_projection_v1) const fn new() -> Self {
        Self {
            graph: PendingNominalInitialGraphV1::new(),
            prefix: RootEntryPrefixV1::empty(),
            indices: RootInvocationIndexStorageV1::empty(),
            ledger: None,
            started: false,
            completed: false,
        }
    }
}

/// Read-only lexical access to the actual CO-OWNED data. It establishes neither
/// guarded accesses nor checked origins, ranked recipe, verification or launch.
pub(in crate::production_ranked_projection_v1) struct ActualRootPrefixIndicesV1<'a> {
    graph: &'a NominalCompleteForProfileGraphV1<'a>,
    source_root: ProductionSourceLaunchRootV1,
    function: &'a SemanticFunctionDeclV1,
    input: &'a ProductionRankedRootInputV1,
    references: &'a [AuthenticatedReferenceEffectBindingV1],
    prefix: &'a RootEntryPrefixV1,
    indices: &'a RootInvocationIndexStorageV1,
}
impl ActualRootPrefixIndicesV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.function
    }
    pub(in crate::production_ranked_projection_v1) fn entry_operations(
        &self,
    ) -> &[ProductionRankedOperationV1] {
        &self.prefix.entry_operations
    }
    pub(in crate::production_ranked_projection_v1) fn next_value(&self) -> u32 {
        self.prefix.next_value
    }
    pub(in crate::production_ranked_projection_v1) fn indices(
        &self,
    ) -> &[Option<ProjectedDisjointIndexV1>] {
        &self.indices.indices
    }
}

struct ActualSelectedInputsV1<'a> {
    input: &'a ProductionRankedRootInputV1,
    source_root: ProductionSourceLaunchRootV1,
    references: &'a [AuthenticatedReferenceEffectBindingV1],
}
fn names_equal_v1(
    left: &str,
    right: &str,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<bool> {
    let work = left
        .len()
        .checked_add(right.len())
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    resources.work(work)?;
    Ok(left == right)
}

/// Content validation helper only. It accepts raw borrows for inert negative
/// controls, but cannot issue a lexical input view or enter assembly. The joined
/// factory obtains these borrows only from the pipeline-owned lexical view.
/// Complete roster selection runs before prefix emission. No cloned bindings,
/// inferred empty default, caller ranks/counts, or caller namespace are accepted.
fn actual_selected_inputs_v1<'a>(
    owner: &ProductionPreRankedKirOwnerV1,
    checked: &CheckedBf16NominalCallV1<'_>,
    function: &SemanticFunctionDeclV1,
    inputs: &'a [ProductionRankedRootInputV1],
    bindings: &'a AuthenticatedReferenceEffectBindingsV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<ActualSelectedInputsV1<'a>> {
    resources.work(128)?;
    let source = owner.semantic_ssa().source_semantic();
    let roster = owner.source_launch();
    // The already-joined S1 profile has exactly two source functions. Use a
    // bounded stack selection table, not an unmetered partition/clone builder.
    if source.functions().len() != 2
        || inputs.is_empty()
        || inputs.len() > 2
        || inputs.len() != roster.roots().len()
        || roster.semantic_sha256() != source.semantic_sha256().as_bytes()
    {
        return Err(Error::Incomplete(
            "actual root prefix input/source roster differs",
        ));
    }
    for (index, input) in inputs.iter().enumerate() {
        resources.work(16)?;
        for earlier in &inputs[..index] {
            if names_equal_v1(&input.logical_name, &earlier.logical_name, resources)? {
                return Err(Error::Unsupported(
                    "duplicate typed logical roots in the ranked roster",
                ));
            }
        }
    }
    let mut bound = [false; 2];
    let mut selected_reference = None;
    let mut selected = None;
    for (index, (input, source_root)) in inputs.iter().zip(roster.roots()).enumerate() {
        resources.work(128)?;
        let actual = source
            .functions()
            .get(source_root.selected_root().index() as usize)
            .ok_or(Error::Incomplete("actual root prefix roster source absent"))?;
        let binding = *actual
            .kernel_entry()
            .ok_or(Error::Unsupported(
                "a semantic KernelRoot without an authenticated kernel entry",
            ))?
            .kernel_binding_identity()
            .as_bytes();
        if actual.role() != SemanticFunctionRoleV1::KernelRoot
            || source_root.semantic_root_identity() != actual.identity()
            || source_root.kernel_binding() != binding
            || input.kernel_binding != binding
            || source_root.source_launch() != source_launch_input_v1(&input.source_launch)
        {
            return Err(Error::Unsupported(
                "source launch roster root changed before ranked projection",
            ));
        }
        if source_root.selected_root() == checked.emission().root() {
            if selected.is_some() || !std::ptr::eq(actual, function) {
                return Err(Error::Incomplete(
                    "actual root prefix selected source differs",
                ));
            }
            selected = Some((index, *source_root));
        }
    }
    let (root_index, source_root) = selected.ok_or(Error::Incomplete(
        "actual root prefix checked root is absent from retained inputs",
    ))?;
    for (binding_index, binding) in bindings.as_slice().iter().enumerate() {
        resources.work(32)?;
        let mut matched = None;
        for (index, input) in inputs.iter().enumerate() {
            if names_equal_v1(&binding.logical_kernel_name, &input.logical_name, resources)? {
                matched = Some(index);
                break;
            }
        }
        let index = matched.ok_or(Error::Unsupported(
            "a reference-effect binding outside the exact typed root roster",
        ))?;
        if bound[index] {
            return Err(Error::Unsupported(
                "duplicate reference-effect bindings for one ranked root",
            ));
        }
        bound[index] = true;
        if index == root_index {
            selected_reference = Some(binding_index);
        }
    }
    let references = match selected_reference {
        Some(index) => &bindings.as_slice()[index..index + 1],
        // Borrowed empty subslice is justified by the COMPLETE actual binding
        // scan, not a manufactured AuthenticatedReferenceEffectBindings default.
        None => &bindings.as_slice()[..0],
    };
    Ok(ActualSelectedInputsV1 {
        input: &inputs[root_index],
        source_root,
        references,
    })
}

/// S1 currently excludes these producers. Recheck their actual discriminants in
/// the SAME source loan so a later whitelist edit cannot silently invalidate the
/// no-emission prefix fact. Non-Call transfer paths return default read_view.
fn require_excluded_prefix_producers_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    for block in function.blocks() {
        resources.work(32)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        resources.work(32)?;
        let callable = callables
            .get(call.callee().index() as usize)
            .ok_or(Error::Incomplete("actual prefix source callable absent"))?;
        if matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice { .. }
                    | SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointBlockComponentIndex { .. },
                ..
            }
        ) {
            return Err(Error::Incomplete(
                "actual root prefix has an unconnected emitting producer",
            ));
        }
    }
    Ok(())
}

fn assembly_frame<R, F>() -> Result<usize> {
    let mut bytes = 8192usize;
    for amount in [
        size_of::<PendingActualRootPrefixIndicesV1>(),
        size_of::<ActualRootPrefixIndicesV1<'static>>(),
        size_of::<ActualSelectedInputsV1<'static>>(),
        size_of::<F>()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        size_of::<Result<R>>()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    ] {
        bytes = bytes
            .checked_add(amount)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    Ok(bytes)
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    /// The non-Clone lexical input view is constructed only by the owning
    /// pipeline after materialization. Equal raw slices cannot replace it.
    /// No normal production route constructs this view at this checkpoint.
    /// No root/access ready token is constructed: one source visit populates
    /// the actual physical namespace, lends it, then performs original postflights.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::production_ranked_projection_v1) fn with_actual_root_prefix_indices_v1<R, F>(
        &mut self,
        checked: &CheckedBf16NominalCallV1<'_>,
        rich: &RichNominalSourceTablesV1<'_>,
        actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
        pending: &mut PendingActualRootPrefixIndicesV1,
        inspect: F,
    ) -> Result<R>
    where
        F: for<'a> FnOnce(ActualRootPrefixIndicesV1<'a>, &mut Self) -> Result<R>,
    {
        let facts_owner = self.facts.owner;
        self.with_resources(|resources| {
            resources.work(64)?;
            if !actual_inputs.belongs_to(facts_owner)
                || !actual_inputs.belongs_to(checked.emission().owner())
            {
                return Err(Error::Incomplete(
                    "actual input loan belongs to another retained owner",
                ));
            }
            let ledger = resources
                .original_ledger_v1()
                .ok_or_else(|| resource(Resource::Accounting))?;
            if pending.ledger.is_some_and(|saved| saved != ledger) {
                return Err(resource(Resource::Accounting));
            }
            if pending.started || pending.ledger.is_some() || pending.completed {
                return Err(Error::Incomplete(
                    "actual root assembly cannot be replaced or retried",
                ));
            }
            let frame = assembly_frame::<R, F>()?;
            resources.work(frame)?;
            resources.reserve_storage(frame)?;
            pending.ledger = Some(ledger);
            pending.started = true;
            Ok(())
        })?;
        let PendingActualRootPrefixIndicesV1 {
            graph,
            prefix,
            indices,
            completed,
            ..
        } = pending;
        self.with_complete_for_profile_graph_v1(checked, rich, graph, |graph, context| {
            let owner = context.facts.owner;
            let source = owner.semantic_ssa().source_semantic();
            let function = graph.function();
            require_same_source_v1(function, rich.function())?;
            let selected = context.with_resources(|resources| {
                actual_selected_inputs_v1(
                    owner,
                    checked,
                    function,
                    actual_inputs.inputs(),
                    actual_inputs.bindings(),
                    resources,
                )
            })?;
            context.with_resources(|resources| {
                prepare_root_entry_prefix_paid_v1(
                    selected.source_root,
                    selected.references,
                    prefix,
                    resources,
                )?;
                // Preserve the ordinary whole-callable retired-family refusal;
                // S1 intentionally is not a substitute for this distinct check.
                let retired_work = source
                    .callables()
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(64))
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                resources.work(retired_work)?;
                reject_retired_production_intrinsics_v1(source.callables())?;
                require_excluded_prefix_producers_v1(function, source.callables(), resources)?;
                prepare_root_namespace_indices_v1(
                    source.callables(),
                    function,
                    rich.scalar_counts(),
                    rich.address_escaped(),
                    rich.option_dominance(),
                    rich.enum_payload_dominance(),
                    graph.edges(),
                    graph.edge_count(),
                    indices,
                    &mut prefix.entry_operations,
                    &mut prefix.next_value,
                    resources,
                )
            })?;
            *completed = true; // Prefix/index DATA only; no actual access rows yet.
            inspect(
                ActualRootPrefixIndicesV1 {
                    graph: &graph,
                    source_root: selected.source_root,
                    function,
                    input: selected.input,
                    references: selected.references,
                    prefix,
                    indices,
                },
                context,
            )
        })
    }
}

#[cfg(test)]
#[path = "bf16_nominal_root_prefix_indices_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bf16_nominal_root_prefix_indices_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
pub(crate) use genuine::observe_actual_root_prefix_indices_for_test_v1;
