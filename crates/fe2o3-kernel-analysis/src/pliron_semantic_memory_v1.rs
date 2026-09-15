//! Source-only, bounded memory-version and noninterference proof for typed SSA
//! reads. This is not an output-refinement, device-provenance or KIR seal.
//!
//! The initial closed subset is an acyclic static invocation domain with
//! unordered nonvolatile Global accesses. A typed read must immediately follow
//! its exact RankedAccessOp read observation. They describe one memory event;
//! the former supplies its SSA result, the latter its existing ranked checks.

use std::{
    collections::HashSet,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{
    AccessKindAttr, MemorySpaceAttr, RankedAccessOp, SemanticReadOrderingAttr,
    SemanticReadVolatilityAttr, SemanticTypedReadOp, SemanticTypedScalarV1,
};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    op::Op,
    operation::Operation,
    value::Value,
};

use crate::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::pliron_invocation_trace::{
    MAX_PLIRON_TRACE_TOTAL_STEPS_V1, PlironTraceEventV1, pliron_execution_layout_with_inventory_v1,
    trace_pliron_invocations_with_inputs_v1,
};
use crate::pliron_provenance_alias::collect_pliron_provenance_alias_with_inventory_v1;
use crate::{
    PlironIrStructuralIdentityV1, analyze_pliron_sparse_indices_v1,
    derive_pliron_ir_structural_identity_v1,
};

#[path = "pliron_semantic_memory_v1/collect.rs"]
mod collect;
#[path = "pliron_semantic_memory_v1/control_flow.rs"]
mod control_flow;
#[path = "pliron_semantic_memory_v1/coverage.rs"]
mod coverage;
pub(crate) use collect::paired_read_access_v1;
#[path = "pliron_semantic_memory_v1/inputs.rs"]
mod inputs;
#[path = "pliron_semantic_memory_v1/state.rs"]
mod state;
pub(crate) use inputs::LivePlironInitialReadInputsV1;
pub use state::PlironSemanticMemoryVersionV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironSemanticMemorySiteV1 {
    block: usize,
    operation: usize,
}

impl PlironSemanticMemorySiteV1 {
    pub(crate) const fn new(block: usize, operation: usize) -> Self {
        Self { block, operation }
    }

    pub const fn block(self) -> usize {
        self.block
    }
    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironSemanticMemoryErrorV1 {
    MissingContextIdentity,
    ContextChanged,
    FunctionChanged,
    MutationEpochChanged,
    InvalidGraph,
    UnsupportedControlFlow,
    UnsupportedOperation {
        site: PlironSemanticMemorySiteV1,
    },
    NonDominatingOperand {
        site: PlironSemanticMemorySiteV1,
    },
    UnpairedRead {
        site: PlironSemanticMemorySiteV1,
    },
    UnsupportedRead {
        site: PlironSemanticMemorySiteV1,
    },
    NonInitialRead {
        site: PlironSemanticMemorySiteV1,
    },
    UnboundLoadSymbol {
        site: PlironSemanticMemorySiteV1,
    },
    DuplicateLoadIdentity,
    UnconsumedRead,
    EmptyReadRoster,
    IncompleteProvenance,
    IncompleteExecutionDomain,
    IncompleteTrace,
    UnprovedBounds {
        site: PlironSemanticMemorySiteV1,
    },
    Interference {
        first: PlironSemanticMemorySiteV1,
        first_invocation: usize,
        second: PlironSemanticMemorySiteV1,
        second_invocation: usize,
    },
    ResourceLimit,
}

impl fmt::Display for PlironSemanticMemoryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source semantic memory proof rejected: {self:?}")
    }
}
impl Error for PlironSemanticMemoryErrorV1 {}

/// One cell's observed version in the retained exact invocation trace. Initial
/// denotes function-entry memory, not zero or a caller-chosen symbolic value.
#[derive(Debug)]
pub struct PlironSemanticReadInstanceV1 {
    invocation: usize,
    coordinates: Vec<u64>,
    indices: Vec<u64>,
    version: PlironSemanticMemoryVersionV1,
}

impl PlironSemanticReadInstanceV1 {
    pub const fn invocation(&self) -> usize {
        self.invocation
    }
    pub fn coordinates(&self) -> &[u64] {
        &self.coordinates
    }
    pub fn indices(&self) -> &[u64] {
        &self.indices
    }
    pub const fn version(&self) -> PlironSemanticMemoryVersionV1 {
        self.version
    }
}

/// Privately derived facts for an actual typed read. No conversion to a free
/// semantic symbol is provided. Retaining a copied label does not retain proof.
#[derive(Debug)]
pub struct PlironProvedSemanticReadV1 {
    site: PlironSemanticMemorySiteV1,
    access: Ptr<Operation>,
    producer: Ptr<Operation>,
    result: Value,
    view: Value,
    indices: Vec<Value>,
    allocation: u64,
    scalar: SemanticTypedScalarV1,
    instances: Vec<PlironSemanticReadInstanceV1>,
}

impl PlironProvedSemanticReadV1 {
    pub const fn site(&self) -> PlironSemanticMemorySiteV1 {
        self.site
    }
    pub const fn access(&self) -> Ptr<Operation> {
        self.access
    }
    pub const fn producer(&self) -> Ptr<Operation> {
        self.producer
    }
    pub const fn result(&self) -> Value {
        self.result
    }
    pub const fn view(&self) -> Value {
        self.view
    }
    pub fn indices(&self) -> &[Value] {
        &self.indices
    }
    pub const fn allocation_origin(&self) -> u64 {
        self.allocation
    }
    pub const fn scalar(&self) -> SemanticTypedScalarV1 {
        self.scalar
    }
    pub fn instances(&self) -> &[PlironSemanticReadInstanceV1] {
        &self.instances
    }
    pub fn reads_initial_memory(&self) -> bool {
        !self.instances.is_empty()
            && self
                .instances
                .iter()
                .all(|instance| instance.version == PlironSemanticMemoryVersionV1::Initial)
    }
}

pub struct LivePlironSemanticMemoryProofV1 {
    context: ContextIdentity,
    function: Ptr<Operation>,
    epoch: u64,
    structural: PlironIrStructuralIdentityV1,
    blocks: Vec<Ptr<BasicBlock>>,
    operations: Vec<Ptr<Operation>>,
    reads: Vec<PlironProvedSemanticReadV1>,
}

impl LivePlironSemanticMemoryProofV1 {
    pub fn with_live_reads<R>(
        &self,
        context: &Context,
        function: &FuncOp,
        inspect: impl FnOnce(&[PlironProvedSemanticReadV1]) -> R,
    ) -> Result<R, PlironSemanticMemoryErrorV1> {
        self.revalidate(context, function)?;
        let result = inspect(&self.reads);
        self.revalidate(context, function)?;
        Ok(result)
    }

    pub fn revalidate(
        &self,
        context: &Context,
        function: &FuncOp,
    ) -> Result<(), PlironSemanticMemoryErrorV1> {
        use PlironSemanticMemoryErrorV1 as E;
        if require_context_identity(context).map_err(|_| E::MissingContextIdentity)? != self.context
        {
            return Err(E::ContextChanged);
        }
        if function.get_operation() != self.function {
            return Err(E::FunctionChanged);
        }
        if epoch(context)? != self.epoch {
            return Err(E::MutationEpochChanged);
        }
        guarded(|| {
            let inventory = BoundedPlironFunctionInventoryV1::collect(context, function)
                .map_err(|_| E::ResourceLimit)?;
            let mut work = control_flow::Work::new(MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
            control_flow::validate(context, function, &inventory, &mut work)?;
            let structural = derive_pliron_ir_structural_identity_v1(context, function)
                .map_err(|_| E::InvalidGraph)?;
            if !self.structural.exactly_matches(&structural)
                || inventory.blocks() != self.blocks
                || !inventory
                    .operations()
                    .iter()
                    .map(|site| site.pointer())
                    .eq(self.operations.iter().copied())
            {
                return Err(E::InvalidGraph);
            }
            if epoch(context)? != self.epoch {
                return Err(E::MutationEpochChanged);
            }
            Ok(())
        })
    }
}

/// Replay all memory effects from one live source graph. The caller supplies
/// neither an effect roster nor a downstream canonical seal. Unknown effects,
/// aliases, unresolved traces, and cross-invocation interference fail closed.
pub fn prove_live_pliron_semantic_memory_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<LivePlironSemanticMemoryProofV1, PlironSemanticMemoryErrorV1> {
    use PlironSemanticMemoryErrorV1 as E;
    let owner = require_context_identity(context).map_err(|_| E::MissingContextIdentity)?;
    let initial_epoch = epoch(context)?;
    guarded(|| {
        let raw = function.get_operation().deref(context);
        if raw.num_regions() != 1
            || raw.get_num_operands() != 0
            || raw.get_num_results() != 0
            || raw.get_num_successors() != 0
        {
            return Err(E::InvalidGraph);
        }
        drop(raw);
        let inventory = BoundedPlironFunctionInventoryV1::collect(context, function)
            .map_err(|_| E::ResourceLimit)?;
        let mut work = control_flow::Work::new(MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
        control_flow::validate(context, function, &inventory, &mut work)?;
        let mut reads = collect::reads(context, &inventory)?;
        let provenance = collect_pliron_provenance_alias_with_inventory_v1(context, &inventory)
            .map_err(|_| E::IncompleteProvenance)?;
        provenance
            .validate_space(MemorySpaceAttr::Global)
            .map_err(|_| E::IncompleteProvenance)?;
        let sparse =
            analyze_pliron_sparse_indices_v1(context, function).map_err(|_| E::IncompleteTrace)?;
        let layout = pliron_execution_layout_with_inventory_v1(context, &inventory)
            .map_err(|_| E::IncompleteExecutionDomain)?;
        if layout.is_none() && !sparse.has_declared_launch_extent() {
            return Err(E::IncompleteExecutionDomain);
        }
        let traces = trace_pliron_invocations_with_inputs_v1(context, &inventory, &sparse, layout)
            .map_err(|error| match error {
                crate::pliron_invocation_trace::PlironTraceFailureV1::ResourceLimit => {
                    E::ResourceLimit
                }
                _ => E::IncompleteTrace,
            })?;
        if traces.is_empty() {
            return Err(E::IncompleteExecutionDomain);
        }
        let coverage = coverage::check(context, &inventory, &reads, &traces, &mut work)?;
        let events = collect::events(&traces)?;
        let versions = state::versions(&events, MAX_PLIRON_TRACE_TOTAL_STEPS_V1).map_err(
            |error| match error {
                state::MemoryFailure::ResourceLimit => E::ResourceLimit,
                state::MemoryFailure::DuplicateEvent => E::InvalidGraph,
                state::MemoryFailure::Interference { first, second } => E::Interference {
                    first: PlironSemanticMemorySiteV1 {
                        block: events[first].block,
                        operation: events[first].operation,
                    },
                    first_invocation: events[first].invocation,
                    second: PlironSemanticMemorySiteV1 {
                        block: events[second].block,
                        operation: events[second].operation,
                    },
                    second_invocation: events[second].invocation,
                },
            },
        )?;
        let sites = reads
            .iter()
            .enumerate()
            .map(|(index, read)| ((read.site.block, read.site.operation - 1), index))
            .collect::<std::collections::HashMap<_, _>>();
        for (event, version) in events.into_iter().zip(versions) {
            let Some(&index) = sites.get(&(event.block, event.operation)) else {
                continue;
            };
            if event.write {
                return Err(E::InvalidGraph);
            }
            reads[index].instances.push(PlironSemanticReadInstanceV1 {
                invocation: event.invocation,
                coordinates: traces[event.invocation].invocation.clone(),
                indices: event.indices,
                version: version.ok_or(E::InvalidGraph)?,
            });
        }
        if reads
            .iter()
            .zip(coverage)
            .any(|(read, count)| read.instances.is_empty() || read.instances.len() != count)
        {
            return Err(E::IncompleteTrace);
        }
        let structural = derive_pliron_ir_structural_identity_v1(context, function)
            .map_err(|_| E::InvalidGraph)?;
        if epoch(context)? != initial_epoch {
            return Err(E::MutationEpochChanged);
        }
        Ok(LivePlironSemanticMemoryProofV1 {
            context: owner,
            function: function.get_operation(),
            epoch: initial_epoch,
            structural,
            blocks: inventory.blocks().to_vec(),
            operations: inventory
                .operations()
                .iter()
                .map(|site| site.pointer())
                .collect(),
            reads,
        })
    })
}

fn epoch(context: &Context) -> Result<u64, PlironSemanticMemoryErrorV1> {
    context
        .ir_mutation_attempt_epoch()
        .map(|epoch| epoch.value())
        .map_err(|_| PlironSemanticMemoryErrorV1::MutationEpochChanged)
}

fn guarded<T>(
    f: impl FnOnce() -> Result<T, PlironSemanticMemoryErrorV1>,
) -> Result<T, PlironSemanticMemoryErrorV1> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| PlironSemanticMemoryErrorV1::InvalidGraph)?
}

#[cfg(test)]
#[path = "pliron_semantic_memory_v1/tests.rs"]
mod tests;
