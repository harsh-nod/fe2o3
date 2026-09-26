//! Closed original-source initial graph preparation. Not a checked-reference
//! constructor, final graph, root recipe, effect projection or admission.
use super::*;
use crate::production_ranked_projection_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticDirectCallV1,
    SemanticTerminatorKindV1,
    root_initial_capability_graph_v1::{InitialGraphStorageV1, populate_initial_graph_v1},
};
use fe2o3_lower_mir_kernel::CheckedBf16NominalCallV1;

/// Must be created in the physical OUTER recipe owner before rich/facts scopes.
/// Every partial vector remains here on error or panic. Do not drop this owner
/// until those scopes' postflights finish; drop it BEFORE refunding its counter.
/// This type never refunds, replaces a graph, or grants a ready-access identity.
pub(in crate::production_ranked_projection_v1) struct PendingNominalInitialGraphV1 {
    graph: Option<InitialGraphStorageV1>,
}
impl PendingNominalInitialGraphV1 {
    pub(in crate::production_ranked_projection_v1) const fn new() -> Self {
        Self { graph: None }
    }
}

/// Source-local row numbers only. No GuardedAccess or ranked block indices exist
/// here. The private constructor is lent only after actual owner/inventory/call,
/// source-function and real entry/call materialization joins succeed.
pub(in crate::production_ranked_projection_v1) struct NominalInitialGraphV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    graph: &'a InitialGraphStorageV1,
}
impl NominalInitialGraphV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.function
    }
    pub(in crate::production_ranked_projection_v1) fn edges(
        &self,
    ) -> &[Vec<crate::production_ranked_projection_v1::CapabilityEdgeV1>] {
        &self.graph.edges
    }
    pub(in crate::production_ranked_projection_v1) fn edge_count(&self) -> usize {
        self.graph.edge_count
    }
}

// Deliberately narrower than ordinary projection. GridLeader recovery is a
// LATER edge family and has no metered counterpart in this slice. Other unknown
// effects/transforms are refused, not treated as transparent or zero-effect.
fn allowed_intrinsic(operation: &SemanticCompilerIntrinsicOperationV1) -> bool {
    matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Trap
            | SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { .. }
            | SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewColumnMajor { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. }
            | SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
    )
}

fn closed_profile(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    source_call: &SemanticDirectCallV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(64)?;
    if function.blocks().is_empty()
        || function.blocks().len() > 32
        || function.locals().len() > 4096
        || callables.len() > 4096
    {
        return Err(Error::Incomplete(
            "nominal initial graph exceeds closed source profile",
        ));
    }
    let mut defined = 0usize;
    for block in function.blocks() {
        resources.work(32)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        resources.work(32)?;
        match callables.get(call.callee().index() as usize) {
            Some(SemanticCallableDeclV1::Defined { .. }) if std::ptr::eq(call, source_call) => {
                defined += 1;
            }
            Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. })
                if allowed_intrinsic(operation) => {}
            _ => {
                return Err(Error::Incomplete(
                    "nominal initial graph has an unconnected later edge or callable family",
                ));
            }
        }
    }
    if defined != 1 {
        return Err(Error::Incomplete(
            "nominal initial graph requires the exact sole source call",
        ));
    }
    Ok(())
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    /// The pending physical owner and its accepted credits stay OUTSIDE rich and
    /// facts factories. This scope borrows them; it performs NO refund or cleanup.
    /// Failed preparation is never loaned; an occupied partial owner cannot be
    /// replaced or retried. An empty preflight refusal grants no ready view.
    pub(in crate::production_ranked_projection_v1) fn with_initial_graph_v1<R, F>(
        &mut self,
        checked: &CheckedBf16NominalCallV1<'_>,
        rich: &RichNominalSourceTablesV1<'_>,
        pending: &mut PendingNominalInitialGraphV1,
        inspect: F,
    ) -> Result<R>
    where
        F: for<'a> FnOnce(NominalInitialGraphV1<'a>, &mut Self) -> Result<R>,
    {
        self.with_resources(|resources| resources.work(64))?;
        if pending.graph.is_some() {
            return Err(Error::Incomplete(
                "nominal initial graph pending owner is already occupied",
            ));
        }
        if self.facts.masked.is_some()
            || !std::ptr::eq(checked.emission().owner(), self.facts.owner)
            || !checked.belongs_to(self.facts.report.inventory())
            || checked.emission().root() != self.facts.correspondence_owner
            || checked.emission().root() != self.facts.semantic_function
        {
            return Err(Error::Incomplete(
                "nominal initial graph owner inventory or root differs",
            ));
        }
        require_same_source_v1(self.function, rich.function())?;
        let function = self.function;
        let owner = self.facts.owner;
        let source = owner.semantic_ssa().source_semantic();
        let root = checked.emission().root().index() as usize;
        let actual = source.functions().get(root).ok_or(Error::Incomplete(
            "nominal initial graph source function absent",
        ))?;
        require_same_source_v1(function, actual)?;
        let block_index = checked.emission().source_call_block().index() as usize;
        let call = function
            .blocks()
            .get(block_index)
            .and_then(|block| {
                if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                    Some(call)
                } else {
                    None
                }
            })
            .ok_or(Error::Incomplete(
                "nominal initial graph source call absent",
            ))?;
        if !std::ptr::eq(call, checked.source_call())
            || source.functions().len() != 2
            || source.types().len() > 4096
            || rich.scalar_counts().len() != function.locals().len()
        {
            return Err(Error::Incomplete(
                "nominal initial graph source call or tables differ",
            ));
        }
        let mut frame = 8192usize;
        for amount in [
            size_of::<InitialGraphStorageV1>(),
            size_of::<PendingNominalInitialGraphV1>(),
            size_of::<NominalInitialGraphV1<'static>>(),
            size_of::<F>()
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
            size_of::<Result<R>>()
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        ] {
            frame = frame
                .checked_add(amount)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
        }
        // Admit the generic callback/result frame before any real-facts query.
        // Callers separately admit capture initialization at its construction site.
        self.with_resources(|resources| {
            resources.work(frame)?;
            resources.reserve_storage(frame)
        })?;
        if !self
            .with_facts(|facts| facts.is_materialized_block(function.entry().index() as usize))?
            || !self.with_facts(|facts| facts.is_materialized_block(block_index))?
        {
            return Err(Error::Incomplete(
                "nominal initial graph source boundary is not materialized",
            ));
        }
        self.with_resources(|resources| {
            closed_profile(function, source.callables(), call, resources)?;
            pending.graph = Some(InitialGraphStorageV1::empty());
            let graph = pending
                .graph
                .as_mut()
                .expect("empty pending graph just installed");
            // Reserve directly into the OUTER payload, including allocation failures.
            resources.work(function.locals().len())?;
            resources.reserve(&mut graph.edges, function.locals().len())?;
            graph.edges.resize_with(function.locals().len(), Vec::new);
            populate_initial_graph_v1(
                source.callables(),
                function,
                None,
                rich.scalar_counts(),
                rich.option_dominance(),
                rich.enum_payload_dominance(),
                &mut graph.edges,
                &mut graph.edge_count,
                &mut graph.stores,
                &mut graph.loads,
                &mut graph.borrowed,
                resources,
            )
        })?;
        let graph = pending
            .graph
            .as_ref()
            .expect("successful pending graph exists");
        let result = inspect(NominalInitialGraphV1 { function, graph }, self);
        self.state.check(self.facts.budget, *self.owned)?;
        result
    }
}

#[cfg(test)]
#[path = "bf16_nominal_initial_graph_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bf16_nominal_initial_graph_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::{
    nominal_initial_graph_controls_for_test_v1, observe_nominal_initial_graph_for_test_v1,
};
