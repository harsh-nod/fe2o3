//! Conditional rank-one coverage, never unconditional ownership or launch authority.
//!
//! The live adapter accepts only entry-defined extents/views and an acyclic CFG
//! of exact index comparisons, non-atomic indexed accesses, branches and returns,
//! plus a verified nontrapping value DAG and paired original read results.
//! A terminal trap must be unreachable under the exact recorded conditions.
//! The pure derivation does not enumerate runtime extents or invocation counts.

use std::collections::{BTreeSet, HashMap, VecDeque};

use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{
    AccessKindAttr, BranchOp, DYNAMIC_EXTENT, DimensionOp, IndexConstantOp, IndexLessThanBranchOp,
    InvocationIndexOp, MemorySpaceAttr, OwnershipContractOp, OwnershipCoverageAttr,
    OwnershipPartitionAttr, RankedAccessOp, RankedViewOp, ReturnOp, TrapOp,
};
use pliron::{
    attribute::AttributeDict,
    builtin::{
        attributes::{IdentifierAttr, StringAttr},
        op_interfaces::OneRegionInterface,
        ops::FuncOp,
    },
    common_traits::Verify,
    context::Context,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    value::Value,
};

use crate::PlironIrStructuralIdentityV1;
use crate::production_analysis::pliron_pass_contract::{
    PlironPassPreservationErrorV1, ScopedVerifiedOwnershipInputV1,
};

#[path = "pliron_conditional_prefix_ownership_v1/value_graph.rs"]
mod value_graph;

#[path = "pliron_conditional_prefix_ownership_v1/resources.rs"]
mod resources;
pub(super) use resources::resource_upper_bound_v1;

#[cfg(test)]
#[path = "pliron_conditional_prefix_ownership_v1/tests.rs"]
mod tests;

pub const MAX_CONDITIONAL_PREFIX_BLOCKS_V1: usize = 64;
pub const MAX_CONDITIONAL_PREFIX_OPERATIONS_V1: usize = 256;
pub const MAX_CONDITIONAL_PREFIX_ARGUMENTS_V1: usize = 32;
pub const MAX_CONDITIONAL_PREFIX_INPUTS_V1: usize = 8;
pub const MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1: usize = 32;
pub const MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1: usize = 9;
pub const MAX_CONDITIONAL_PREFIX_WORK_V1: usize = 16_384;
pub const MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1: usize = 16;
pub const MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalPrefixSiteV1 {
    block: u32,
    operation: u32,
}

impl ConditionalPrefixSiteV1 {
    pub const fn block(self) -> u32 {
        self.block
    }
    pub const fn operation(self) -> u32 {
        self.operation
    }
}

/// An extent's exact source, not a caller-supplied observed length.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConditionalPrefixExtentSourceV1 {
    /// Ranked-analysis entry ordinal, NOT a physical kernel ABI argument index.
    /// Requires an exact compiler-owned mapping before any host consumption.
    RankedEntryArgument(u32),
    Constant(u64),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalPrefixExtentV1 {
    view: ConditionalPrefixSiteV1,
    allocation_origin: u64,
    noalias_class: u64,
    source: ConditionalPrefixExtentSourceV1,
}

impl ConditionalPrefixExtentV1 {
    pub const fn view(self) -> ConditionalPrefixSiteV1 {
        self.view
    }
    pub const fn allocation_origin(self) -> u64 {
        self.allocation_origin
    }
    pub const fn noalias_class(self) -> u64 {
        self.noalias_class
    }
    pub const fn dimension(self) -> u32 {
        0
    }
    pub const fn source(self) -> ConditionalPrefixExtentSourceV1 {
        self.source
    }
}

/// Axis zero of the actual global workitem geometry; never a workgroup count.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalPrefixLaunchV1 {
    layout: ConditionalPrefixSiteV1,
    grid_identity: u64,
    declared_workitems: u64,
}

impl ConditionalPrefixLaunchV1 {
    pub const fn layout(self) -> ConditionalPrefixSiteV1 {
        self.layout
    }
    pub const fn grid_identity(self) -> u64 {
        self.grid_identity
    }
    pub const fn axis(self) -> u32 {
        0
    }
    /// Zero is the existing dynamic-launch sentinel, not a concrete empty launch.
    pub const fn declared_workitems(self) -> u64 {
        self.declared_workitems
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConditionalPrefixConditionV1 {
    OutputExtentAtMostInput {
        output: ConditionalPrefixExtentV1,
        input: ConditionalPrefixExtentV1,
    },
    OutputExtentAtMostActualWorkitems {
        output: ConditionalPrefixExtentV1,
        launch: ConditionalPrefixLaunchV1,
    },
}

/// Outstanding compiler-owned joins, separate from the numerical inequalities.
/// No ranked ordinal is a host index, and declared launch geometry is not an
/// observed dispatch. This module neither constructs nor discharges these joins.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConditionalPrefixHostBindingObligationV1 {
    RankedViewToPhysicalAllocationExtent { extent: ConditionalPrefixExtentV1 },
    RankedLaunchToActualDispatch { launch: ConditionalPrefixLaunchV1 },
}

/// One literal in the exact source path DNF: `i < extent` or its negation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalPrefixGuardAtomV1 {
    branch: ConditionalPrefixSiteV1,
    extent: ConditionalPrefixExtentV1,
    less_than: bool,
}

impl ConditionalPrefixGuardAtomV1 {
    pub const fn branch(self) -> ConditionalPrefixSiteV1 {
        self.branch
    }
    pub const fn extent(self) -> ConditionalPrefixExtentV1 {
        self.extent
    }
    pub const fn less_than(self) -> bool {
        self.less_than
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionalPrefixDerivationErrorV1 {
    Limit(&'static str),
    Unsupported(&'static str),
    UnsupportedOperation {
        site: ConditionalPrefixSiteV1,
        operation: &'static str,
    },
    Malformed(&'static str),
    CyclicControlFlow,
    UnreachableBlock,
    DuplicateWrite,
    InexactWriteGuard,
    UnguardedRead,
    NotDynamic,
}

impl std::fmt::Display for ConditionalPrefixDerivationErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "conditional prefix coverage unavailable: {self:?}")
    }
}

impl std::error::Error for ConditionalPrefixDerivationErrorV1 {}

/// A conditional theorem for one exact live graph snapshot. All runtime
/// conditions remain outstanding. This record does not replace ordinary
/// bounds, race, source, value-refinement, target, or host admission checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalPrefixCoverageV1 {
    graph_identity: PlironIrStructuralIdentityV1,
    mutation_epoch: u64,
    contract: ConditionalPrefixSiteV1,
    invocation: ConditionalPrefixSiteV1,
    write: ConditionalPrefixSiteV1,
    output: ConditionalPrefixExtentV1,
    launch: ConditionalPrefixLaunchV1,
    source_guard_dnf: Vec<Vec<ConditionalPrefixGuardAtomV1>>,
    conditions: Vec<ConditionalPrefixConditionV1>,
    host_binding_obligations: Vec<ConditionalPrefixHostBindingObligationV1>,
}

impl ConditionalPrefixCoverageV1 {
    pub fn graph_identity(&self) -> &PlironIrStructuralIdentityV1 {
        &self.graph_identity
    }
    pub const fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }
    pub const fn ownership_contract(&self) -> ConditionalPrefixSiteV1 {
        self.contract
    }
    pub const fn invocation(&self) -> ConditionalPrefixSiteV1 {
        self.invocation
    }
    pub const fn source_write(&self) -> ConditionalPrefixSiteV1 {
        self.write
    }
    pub const fn output(&self) -> ConditionalPrefixExtentV1 {
        self.output
    }
    pub const fn launch(&self) -> ConditionalPrefixLaunchV1 {
        self.launch
    }
    pub fn source_guard_dnf(&self) -> &[Vec<ConditionalPrefixGuardAtomV1>] {
        &self.source_guard_dnf
    }
    pub fn conditions(&self) -> &[ConditionalPrefixConditionV1] {
        &self.conditions
    }
    pub fn host_binding_obligations(&self) -> &[ConditionalPrefixHostBindingObligationV1] {
        &self.host_binding_obligations
    }
    pub const fn proves_unconditional_total_view(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Deterministic, versioned record bytes for a future condition-roster join.
    /// The full live graph is retained separately for byte-exact revalidation.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = b"fe2o3.conditional-prefix.v1\0".to_vec();
        out.extend_from_slice(self.graph_identity.sha256());
        put_u64(&mut out, self.mutation_epoch);
        for site in [self.contract, self.invocation, self.write] {
            put_site(&mut out, site);
        }
        put_extent(&mut out, self.output);
        put_launch(&mut out, self.launch);
        put_u64(&mut out, self.source_guard_dnf.len() as u64);
        for term in &self.source_guard_dnf {
            put_u64(&mut out, term.len() as u64);
            for atom in term {
                put_site(&mut out, atom.branch);
                put_extent(&mut out, atom.extent);
                out.push(u8::from(atom.less_than));
            }
        }
        put_u64(&mut out, self.conditions.len() as u64);
        for condition in &self.conditions {
            match *condition {
                ConditionalPrefixConditionV1::OutputExtentAtMostInput { output, input } => {
                    out.push(1);
                    put_extent(&mut out, output);
                    put_extent(&mut out, input);
                }
                ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems {
                    output,
                    launch,
                } => {
                    out.push(2);
                    put_extent(&mut out, output);
                    put_launch(&mut out, launch);
                }
            }
        }
        put_u64(&mut out, self.host_binding_obligations.len() as u64);
        for obligation in &self.host_binding_obligations {
            match *obligation {
                ConditionalPrefixHostBindingObligationV1::RankedViewToPhysicalAllocationExtent {
                    extent,
                } => {
                    out.push(1);
                    put_extent(&mut out, extent);
                }
                ConditionalPrefixHostBindingObligationV1::RankedLaunchToActualDispatch {
                    launch,
                } => {
                    out.push(2);
                    put_launch(&mut out, launch);
                }
            }
        }
        out
    }
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_site(out: &mut Vec<u8>, site: ConditionalPrefixSiteV1) {
    out.extend_from_slice(&site.block.to_le_bytes());
    out.extend_from_slice(&site.operation.to_le_bytes());
}
fn put_extent(out: &mut Vec<u8>, extent: ConditionalPrefixExtentV1) {
    put_site(out, extent.view);
    put_u64(out, extent.allocation_origin);
    put_u64(out, extent.noalias_class);
    match extent.source {
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(index) => {
            out.push(1);
            put_u64(out, u64::from(index));
        }
        ConditionalPrefixExtentSourceV1::Constant(value) => {
            out.push(2);
            put_u64(out, value);
        }
    }
}
fn put_launch(out: &mut Vec<u8>, launch: ConditionalPrefixLaunchV1) {
    put_site(out, launch.layout);
    put_u64(out, launch.grid_identity);
    put_u64(out, launch.declared_workitems);
}

#[derive(Clone)]
struct PrefixBlock {
    accesses: Vec<(ConditionalPrefixSiteV1, usize, bool)>,
    terminator: PrefixTerminator,
}

#[derive(Clone, Copy)]
enum PrefixTerminator {
    Return,
    Trap(ConditionalPrefixSiteV1),
    Goto(usize),
    LessThan {
        site: ConditionalPrefixSiteV1,
        extent: usize,
        yes: usize,
        no: usize,
    },
}

impl PrefixTerminator {
    fn targets(&self) -> Vec<usize> {
        match *self {
            Self::Return | Self::Trap(_) => vec![],
            Self::Goto(target) => vec![target],
            Self::LessThan { yes, no, .. } => vec![yes, no],
        }
    }
}

#[derive(Clone)]
struct PrefixModel {
    views: Vec<ConditionalPrefixExtentV1>,
    writable: Vec<bool>,
    blocks: Vec<PrefixBlock>,
    output: usize,
    contract: ConditionalPrefixSiteV1,
    invocation: ConditionalPrefixSiteV1,
    launch: ConditionalPrefixLaunchV1,
}

struct PrefixProof {
    write: ConditionalPrefixSiteV1,
    source_guard_dnf: Vec<Vec<ConditionalPrefixGuardAtomV1>>,
    conditions: Vec<ConditionalPrefixConditionV1>,
}

fn charge(work: &mut usize, amount: usize) -> Result<(), ConditionalPrefixDerivationErrorV1> {
    *work = work
        .checked_add(amount)
        .ok_or(ConditionalPrefixDerivationErrorV1::Limit("work"))?;
    if *work > MAX_CONDITIONAL_PREFIX_WORK_V1 {
        return Err(ConditionalPrefixDerivationErrorV1::Limit("work"));
    }
    Ok(())
}

// O(B + E + bounded DNF work). There is no work proportional to N or W.
#[cfg(test)]
fn derive_model(model: &PrefixModel) -> Result<PrefixProof, ConditionalPrefixDerivationErrorV1> {
    derive_model_with_work(model, &mut 0)
}

fn derive_model_with_work(
    model: &PrefixModel,
    work: &mut usize,
) -> Result<PrefixProof, ConditionalPrefixDerivationErrorV1> {
    use ConditionalPrefixDerivationErrorV1 as Error;
    if model.blocks.is_empty()
        || model.blocks.len() > MAX_CONDITIONAL_PREFIX_BLOCKS_V1
        || model.views.len() > MAX_CONDITIONAL_PREFIX_INPUTS_V1 + 1
    {
        return Err(Error::Limit("blocks/views"));
    }
    let operations = model
        .blocks
        .iter()
        .try_fold(0_usize, |sum, block| {
            sum.checked_add(block.accesses.len())?.checked_add(1)
        })
        .ok_or(Error::Limit("operations"))?;
    if operations > MAX_CONDITIONAL_PREFIX_OPERATIONS_V1 {
        return Err(Error::Limit("operations"));
    }
    let output = *model
        .views
        .get(model.output)
        .ok_or(Error::Malformed("output view"))?;
    if model.writable.len() != model.views.len() || !model.writable[model.output] {
        return Err(Error::Malformed("view permissions"));
    }
    if model
        .views
        .iter()
        .any(|view| view.allocation_origin == 0 || view.noalias_class == 0)
    {
        return Err(Error::Malformed("allocation identity"));
    }
    let mut branches = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    let mut write = None;
    for block in &model.blocks {
        if let PrefixTerminator::LessThan {
            site,
            extent,
            yes,
            no,
        } = block.terminator
        {
            if extent >= model.views.len() || yes == no {
                return Err(Error::Malformed("guard"));
            }
            branches.insert(site);
        }
        for &(site, view, writes) in &block.accesses {
            if view >= model.views.len() {
                return Err(Error::Malformed("access view"));
            }
            if writes {
                if view != model.output || write.replace(site).is_some() {
                    return Err(Error::DuplicateWrite);
                }
            } else {
                if model.writable[view] {
                    return Err(Error::Unsupported("read from writable view"));
                }
                inputs.insert(view);
            }
        }
    }
    let write = write.ok_or(Error::Malformed("missing write"))?;
    if inputs.len() + 1 != model.views.len() {
        return Err(Error::Unsupported("unaccessed view"));
    }
    for &input in &inputs {
        let input = model.views[input];
        if input.allocation_origin == output.allocation_origin
            || input.noalias_class == output.noalias_class
        {
            return Err(Error::Unsupported("output/input alias relation"));
        }
    }
    let order = topological_blocks(&model.blocks, work)?;
    let expected = inputs
        .iter()
        .copied()
        .chain([model.output])
        .map(|i| model.views[i])
        .collect::<BTreeSet<_>>();
    let mut conditions = inputs
        .iter()
        .map(
            |&input| ConditionalPrefixConditionV1::OutputExtentAtMostInput {
                output,
                input: model.views[input],
            },
        )
        .collect::<Vec<_>>();
    conditions.push(
        ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems {
            output,
            launch: model.launch,
        },
    );
    conditions.sort_unstable();
    let mut paths = vec![Vec::<Vec<ConditionalPrefixGuardAtomV1>>::new(); model.blocks.len()];
    paths[0].push(vec![]);
    let mut source_guard_dnf = Vec::new();
    for index in order {
        if paths[index].is_empty() {
            return Err(Error::UnreachableBlock);
        }
        let terms = paths[index].clone();
        charge(work, terms.iter().map(|t| t.len() + 1).sum())?;
        if let PrefixTerminator::Trap(site) = model.blocks[index].terminator {
            for term in &terms {
                charge(
                    work,
                    term.len().saturating_mul(term.len() + conditions.len() + 2),
                )?;
                let inside_output = term.iter().any(|a| a.less_than && a.extent == output);
                // i < output <= input contradicts i >= input only under the
                // exact condition returned below. Its host check is outstanding.
                if !term.iter().filter(|a| !a.less_than).any(|negative| {
                    term.iter().any(|a| a.less_than && a.extent == negative.extent)
                        || (inside_output && conditions.iter().any(|condition| {
                            matches!(condition, ConditionalPrefixConditionV1::OutputExtentAtMostInput {
                                output: bound_output, input,
                            } if *bound_output == output && *input == negative.extent)
                        }))
                }) {
                    return Err(Error::UnsupportedOperation {
                        site,
                        operation: "kernel.trap",
                    });
                }
            }
        }
        for &(site, view, writes) in &model.blocks[index].accesses {
            for term in &terms {
                charge(work, term.len() + 1)?;
                if writes {
                    let actual = term
                        .iter()
                        .filter(|atom| atom.less_than)
                        .map(|atom| atom.extent)
                        .collect::<BTreeSet<_>>();
                    if site != write || actual != expected || term.iter().any(|a| !a.less_than) {
                        return Err(Error::InexactWriteGuard);
                    }
                } else if !term
                    .iter()
                    .any(|atom| atom.less_than && atom.extent == model.views[view])
                {
                    return Err(Error::UnguardedRead);
                }
            }
            if writes {
                source_guard_dnf = terms.clone();
            }
        }
        let successors = match model.blocks[index].terminator {
            PrefixTerminator::Return | PrefixTerminator::Trap(_) => Vec::new(),
            PrefixTerminator::Goto(target) => vec![(target, None)],
            PrefixTerminator::LessThan {
                site,
                extent,
                yes,
                no,
            } => vec![
                (
                    yes,
                    Some(ConditionalPrefixGuardAtomV1 {
                        branch: site,
                        extent: model.views[extent],
                        less_than: true,
                    }),
                ),
                (
                    no,
                    Some(ConditionalPrefixGuardAtomV1 {
                        branch: site,
                        extent: model.views[extent],
                        less_than: false,
                    }),
                ),
            ],
        };
        for (target, atom) in successors {
            for term in &terms {
                charge(work, term.len() + 1)?;
                let mut term = term.clone();
                if let Some(atom) = atom {
                    term.push(atom);
                }
                if term.len() > MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 {
                    return Err(Error::Limit("guard atoms"));
                }
                term.sort_unstable();
                charge(work, paths[target].len() * (term.len() + 1))?;
                if !paths[target].contains(&term) {
                    if paths[target].len() == MAX_CONDITIONAL_PREFIX_DNF_TERMS_V1 {
                        return Err(Error::Limit("DNF terms"));
                    }
                    paths[target].push(term);
                }
            }
        }
    }
    if source_guard_dnf.is_empty()
        || source_guard_dnf
            .iter()
            .flatten()
            .map(|a| a.branch)
            .collect::<BTreeSet<_>>()
            != branches
    {
        return Err(Error::InexactWriteGuard);
    }
    source_guard_dnf.sort_unstable();
    source_guard_dnf.dedup();
    Ok(PrefixProof {
        write,
        source_guard_dnf,
        conditions,
    })
}

#[derive(Clone, Copy)]
enum IndexSymbol {
    Invocation,
    Extent(usize),
    Constant(u64),
}

// Bound dictionary scans and key comparisons before any dialect verifier. The
// shared inventory's configurable limits are private and its default limits
// are wider than this adapter's, so retain the tight inventory below. Opaque
// debug metadata and canonical serialization still belong to the identity layer.
fn preflight_attributes(
    attributes: &AttributeDict,
    work: &mut usize,
) -> Result<(), ConditionalPrefixDerivationErrorV1> {
    use ConditionalPrefixDerivationErrorV1 as Error;
    if attributes.0.len() > MAX_CONDITIONAL_PREFIX_ATTRIBUTES_PER_ENTITY_V1 {
        return Err(Error::Limit("attributes per entity"));
    }
    charge(work, attributes.0.len())?;
    for (key, value) in attributes.0.iter() {
        let key: &str = key.as_ref();
        if key.len() > MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1 {
            return Err(Error::Limit("attribute key bytes"));
        }
        charge(work, key.len())?;
        // Inspect strings without invoking a printer or cloning their payload.
        let text = if let Some(string) = value.downcast_ref::<StringAttr>() {
            Some(string.as_str())
        } else {
            value.downcast_ref::<IdentifierAttr>().map(|identifier| {
                let identifier: &pliron::identifier::Identifier = identifier.as_ref();
                identifier.as_ref()
            })
        };
        if let Some(text) = text {
            if text.len() > MAX_CONDITIONAL_PREFIX_ATTRIBUTE_TEXT_BYTES_V1 {
                return Err(Error::Limit("attribute text bytes"));
            }
            charge(work, text.len())?;
        }
    }
    Ok(())
}

fn topological_blocks(
    blocks: &[PrefixBlock],
    work: &mut usize,
) -> Result<Vec<usize>, ConditionalPrefixDerivationErrorV1> {
    use ConditionalPrefixDerivationErrorV1 as Error;
    charge(work, blocks.len().saturating_mul(4))?;
    let mut indegree = vec![0; blocks.len()];
    for block in blocks {
        for target in block.terminator.targets() {
            charge(work, 2)?;
            *indegree.get_mut(target).ok_or(Error::Malformed("target"))? += 1;
        }
    }
    let mut order = Vec::with_capacity(blocks.len());
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(block, &count)| (count == 0).then_some(block))
        .collect::<VecDeque<_>>();
    while let Some(next) = ready.pop_front() {
        order.push(next);
        for target in blocks[next].terminator.targets() {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                ready.push_back(target);
            }
        }
    }
    if order.len() != blocks.len() {
        return Err(Error::CyclicControlFlow);
    }
    Ok(order)
}

fn unsupported_operation(
    op: &dyn Op,
    site: ConditionalPrefixSiteV1,
) -> ConditionalPrefixDerivationErrorV1 {
    use dialect_kernel::*;
    let operation = if op.downcast_ref::<IndexUnknownOp>().is_some() {
        "kernel.index_unknown"
    } else if op.downcast_ref::<IndexBinaryOp>().is_some() {
        "kernel.index_binary"
    } else if op.downcast_ref::<IndexUnsignedCastOp>().is_some() {
        "kernel.index_unsigned_cast"
    } else if op.downcast_ref::<IndexEqualBranchOp>().is_some() {
        "kernel.index_eq_br"
    } else if op.downcast_ref::<BranchArgsOp>().is_some() {
        "kernel.br_args"
    } else if op.downcast_ref::<IndexLessThanBranchArgsOp>().is_some() {
        "kernel.index_lt_br_args"
    } else if op.downcast_ref::<SemanticTypedCompareOp>().is_some() {
        "kernel.semantic_typed_compare"
    } else if op.downcast_ref::<SemanticTypedCastOp>().is_some() {
        "kernel.semantic_typed_cast"
    } else if op.downcast_ref::<TrapOp>().is_some() {
        "kernel.trap"
    } else {
        "other operation"
    };
    ConditionalPrefixDerivationErrorV1::UnsupportedOperation { site, operation }
}

fn collect_model(
    context: &Context,
    function: &FuncOp,
    work: &mut usize,
) -> Result<PrefixModel, ConditionalPrefixDerivationErrorV1> {
    use ConditionalPrefixDerivationErrorV1 as Error;
    if function.get_operation().deref(context).num_regions() != 1 {
        return Err(Error::Malformed("function region"));
    }
    preflight_attributes(&function.get_operation().deref(context).attributes, work)?;
    let mut live_blocks = Vec::new();
    let mut live_operations = Vec::new();
    let mut operation_count = 0;
    for block in function.get_region(context).deref(context).iter(context) {
        if live_blocks.len() == MAX_CONDITIONAL_PREFIX_BLOCKS_V1 {
            return Err(Error::Limit("blocks"));
        }
        preflight_attributes(&block.deref(context).attributes, work)?;
        let mut operations = Vec::new();
        for operation in block.deref(context).iter(context) {
            if operation_count == MAX_CONDITIONAL_PREFIX_OPERATIONS_V1 {
                return Err(Error::Limit("operations"));
            }
            operation_count += 1;
            preflight_attributes(&operation.deref(context).attributes, work)?;
            operations.push(operation);
        }
        live_blocks.push(block);
        live_operations.push(operations);
    }
    let entry = *live_blocks.first().ok_or(Error::Malformed("entry block"))?;
    if entry.deref(context).get_num_arguments() > MAX_CONDITIONAL_PREFIX_ARGUMENTS_V1 {
        return Err(Error::Limit("entry arguments"));
    }
    // The owner has verified closed SSA and native dominance. Bound the local
    // adapter's supported operand, region, and successor shapes before walking.
    for (b, block) in live_blocks.iter().enumerate() {
        if b != 0 && block.deref(context).get_num_arguments() != 0 {
            return Err(Error::Unsupported("block arguments"));
        }
        for pointer in &live_operations[b] {
            let op = Operation::get_op_dyn(*pointer, context);
            let raw = pointer.deref(context);
            let effect = op.downcast_ref::<dialect_proof::RequireEffectRefinementOp>();
            if raw.get_num_operands() > if effect.is_some() { 10 } else { 9 }
                || raw.get_num_results() > 1
                || raw.num_regions() != 0
                || raw.get_num_successors() > 2
            {
                return Err(Error::Malformed("operation shape"));
            }
        }
    }
    // Closure and native dominance are already verified by the scoped owner input.
    let arguments = entry
        .deref(context)
        .arguments()
        .enumerate()
        .map(|(i, v)| (v, i as u32))
        .collect::<HashMap<_, _>>();
    let block_ids = live_blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (*b, i))
        .collect::<HashMap<_, _>>();
    let mut views = Vec::new();
    let mut writable = Vec::new();
    let mut view_ids = HashMap::new();
    let mut symbols = HashMap::<Value, IndexSymbol>::new();
    let mut blocks = Vec::new();
    let mut contract = None;
    let mut invocation = None;
    let mut invocation_extent = None;
    let mut launch = None;
    for (block_index, sites) in live_operations.iter().enumerate() {
        let mut accesses = Vec::new();
        let mut terminator = None;
        for (position, pointer) in sites.iter().enumerate() {
            let site = ConditionalPrefixSiteV1 {
                block: block_index as u32,
                operation: position as u32,
            };
            let op = Operation::get_op_dyn(*pointer, context);
            let raw = pointer.deref(context);
            if value_graph::is_value(&*op) {
                // Validate the completed value DAG in CFG order, independent
                // of the region's block storage order.
                continue;
            } else if value_graph::verify_metadata(context, &*op, &view_ids)? {
                continue;
            } else if let Some(view) = op.downcast_ref::<RankedViewOp>() {
                if block_index != 0 || views.len() == MAX_CONDITIONAL_PREFIX_INPUTS_V1 + 1 {
                    return Err(Error::Limit("entry views"));
                }
                if raw.get_num_results() != 1 {
                    return Err(Error::Malformed("view result"));
                }
                let ty = view
                    .view_type(context)
                    .ok_or(Error::Malformed("view type"))?;
                let ty = ty.deref(context);
                if ty.shape().len() != 1
                    || view.memory_space(context) != Some(MemorySpaceAttr::Global)
                {
                    return Err(Error::Unsupported("non-global/rank-one view"));
                }
                // RankedViewOp::verify scans the type's dynamic extents.
                view.verify(context).map_err(|_| Error::Malformed("view"))?;
                let origin = view
                    .allocation_origin(context)
                    .filter(|id| *id != 0)
                    .ok_or(Error::Malformed("allocation origin"))?;
                let class = view
                    .noalias_class(context)
                    .filter(|id| *id != 0)
                    .ok_or(Error::Malformed("noalias class"))?;
                let source = if ty.shape()[0] == DYNAMIC_EXTENT {
                    let extent = view
                        .dynamic_extent(context, 0)
                        .ok_or(Error::Malformed("dynamic extent"))?;
                    ConditionalPrefixExtentSourceV1::RankedEntryArgument(
                        *arguments
                            .get(&extent)
                            .ok_or(Error::Unsupported("non-argument extent"))?,
                    )
                } else {
                    ConditionalPrefixExtentSourceV1::Constant(ty.shape()[0])
                };
                view_ids.insert(view.result(context), views.len());
                views.push(ConditionalPrefixExtentV1 {
                    view: site,
                    allocation_origin: origin,
                    noalias_class: class,
                    source,
                });
                writable.push(ty.writable());
            } else if let Some(layout) = op.downcast_ref::<ExecutionLayoutOp>() {
                if block_index != 0 || launch.is_some() {
                    return Err(Error::Malformed("execution layout"));
                }
                layout
                    .verify(context)
                    .map_err(|_| Error::Malformed("execution layout"))?;
                let global = layout
                    .global_extents(context)
                    .ok_or(Error::Malformed("global extents"))?;
                let local = layout
                    .workgroup_extents(context)
                    .ok_or(Error::Malformed("local extents"))?;
                if global[1..] != [1, 1]
                    || local[1..] != [1, 1]
                    || local[0] == 0
                    || global[0] == u64::MAX
                {
                    return Err(Error::Unsupported("non-one-dimensional launch"));
                }
                launch = Some(ConditionalPrefixLaunchV1 {
                    layout: site,
                    grid_identity: layout
                        .grid_identity(context)
                        .ok_or(Error::Malformed("grid identity"))?,
                    declared_workitems: global[0],
                });
            } else if let Some(index) = op.downcast_ref::<InvocationIndexOp>() {
                if block_index != 0 || invocation.is_some() || index.dimension(context) != Some(0) {
                    return Err(Error::Unsupported("invocation identity"));
                }
                index
                    .verify(context)
                    .map_err(|_| Error::Malformed("invocation"))?;
                invocation_extent = index.launch_extent(context);
                invocation = Some(site);
                symbols.insert(index.result(context), IndexSymbol::Invocation);
            } else if let Some(dimension) = op.downcast_ref::<DimensionOp>() {
                if block_index != 0 || dimension.dimension(context) != Some(0) {
                    return Err(Error::Unsupported("dimension definition"));
                }
                dimension
                    .verify(context)
                    .map_err(|_| Error::Malformed("dimension"))?;
                let view = *view_ids
                    .get(&dimension.view(context))
                    .ok_or(Error::Malformed("dimension view"))?;
                symbols.insert(dimension.result(context), IndexSymbol::Extent(view));
            } else if let Some(constant) = op.downcast_ref::<IndexConstantOp>() {
                if block_index != 0 {
                    return Err(Error::Unsupported("non-entry constant"));
                }
                constant
                    .verify(context)
                    .map_err(|_| Error::Malformed("constant"))?;
                symbols.insert(
                    constant.result(context),
                    IndexSymbol::Constant(
                        constant
                            .value(context)
                            .ok_or(Error::Malformed("constant"))?,
                    ),
                );
            } else if let Some(owner) = op.downcast_ref::<OwnershipContractOp>() {
                if block_index != 0
                    || contract.is_some()
                    || owner.coverage(context) != Some(OwnershipCoverageAttr::TotalView)
                    || owner.partition(context) != Some(OwnershipPartitionAttr::ExactSets)
                {
                    return Err(Error::Unsupported("ownership contract"));
                }
                owner
                    .verify(context)
                    .map_err(|_| Error::Malformed("ownership contract"))?;
                contract = Some((
                    site,
                    *view_ids
                        .get(&owner.view(context))
                        .ok_or(Error::Malformed("owned view"))?,
                ));
            } else if let Some(access) = op.downcast_ref::<RankedAccessOp>() {
                access
                    .verify(context)
                    .map_err(|_| Error::Malformed("access"))?;
                if access.indices(context).len() != 1 || access.checked_success(context).is_some() {
                    return Err(Error::Unsupported("predicated or non-rank-one access"));
                }
                if !matches!(
                    symbols.get(&raw.get_operand(1)),
                    Some(IndexSymbol::Invocation)
                ) {
                    return Err(Error::Unsupported("non-identity index"));
                }
                let view = *view_ids
                    .get(&access.view(context))
                    .ok_or(Error::Malformed("access view"))?;
                let writes = match access.kind(context) {
                    Some(AccessKindAttr::Read) => false,
                    Some(AccessKindAttr::Write) => true,
                    _ => return Err(Error::Unsupported("atomic/effect access")),
                };
                accesses.push((site, view, writes));
            } else {
                if position + 1 != sites.len() {
                    return Err(unsupported_operation(&*op, site));
                }
                let targets = raw
                    .successors()
                    .map(|target| {
                        block_ids
                            .get(&target)
                            .copied()
                            .ok_or(Error::Malformed("foreign target"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let term = if let Some(branch) = op.downcast_ref::<IndexLessThanBranchOp>() {
                    branch
                        .verify(context)
                        .map_err(|_| Error::Malformed("branch"))?;
                    if !matches!(
                        symbols.get(&branch.lhs(context)),
                        Some(IndexSymbol::Invocation)
                    ) {
                        return Err(Error::Unsupported("non-prefix predicate"));
                    }
                    let rhs = branch.rhs(context);
                    let extent = match symbols.get(&rhs) {
                        Some(IndexSymbol::Extent(view)) => *view,
                        symbol => {
                            let source = match symbol {
                                Some(IndexSymbol::Constant(value)) => {
                                    ConditionalPrefixExtentSourceV1::Constant(*value)
                                }
                                None => ConditionalPrefixExtentSourceV1::RankedEntryArgument(
                                    *arguments
                                        .get(&rhs)
                                        .ok_or(Error::Unsupported("unknown bound"))?,
                                ),
                                _ => return Err(Error::Unsupported("non-extent bound")),
                            };
                            let matches = views
                                .iter()
                                .enumerate()
                                .filter(|(_, v)| v.source == source)
                                .map(|(i, _)| i)
                                .collect::<Vec<_>>();
                            let [view] = matches.as_slice() else {
                                return Err(Error::Unsupported("ambiguous bound identity"));
                            };
                            *view
                        }
                    };
                    let [yes, no] = targets.as_slice() else {
                        return Err(Error::Malformed("branch targets"));
                    };
                    PrefixTerminator::LessThan {
                        site,
                        extent,
                        yes: *yes,
                        no: *no,
                    }
                } else if let Some(branch) = op.downcast_ref::<BranchOp>() {
                    branch
                        .verify(context)
                        .map_err(|_| Error::Malformed("goto"))?;
                    let [target] = targets.as_slice() else {
                        return Err(Error::Malformed("goto target"));
                    };
                    PrefixTerminator::Goto(*target)
                } else if let Some(ret) = op.downcast_ref::<ReturnOp>() {
                    ret.verify(context)
                        .map_err(|_| Error::Malformed("return"))?;
                    PrefixTerminator::Return
                } else if let Some(trap) = op.downcast_ref::<TrapOp>() {
                    trap.verify(context).map_err(|_| Error::Malformed("trap"))?;
                    PrefixTerminator::Trap(site)
                } else {
                    return Err(unsupported_operation(&*op, site));
                };
                terminator = Some(term);
            }
        }
        blocks.push(PrefixBlock {
            accesses,
            terminator: terminator.ok_or(Error::Malformed("missing terminator"))?,
        });
    }
    let launch = launch.ok_or(Error::Malformed("missing layout"))?;
    if invocation_extent != Some(launch.declared_workitems) {
        return Err(Error::Malformed("invocation/layout mismatch"));
    }
    let (contract, output) = contract.ok_or(Error::Unsupported("missing TotalView"))?;
    if launch.declared_workitems != 0
        && views
            .iter()
            .all(|v| matches!(v.source, ConditionalPrefixExtentSourceV1::Constant(_)))
    {
        return Err(Error::NotDynamic);
    }
    let model = PrefixModel {
        views,
        writable,
        blocks,
        output,
        contract,
        invocation: invocation.ok_or(Error::Malformed("missing invocation"))?,
        launch,
    };
    value_graph::validate(context, &model, &live_operations, work)?;
    Ok(model)
}

/// Derives only from the active ownership pass's verified endpoints and snapshot.
pub(super) fn derive_with_scoped_input_v1(
    input: &ScopedVerifiedOwnershipInputV1<'_>,
) -> Result<
    Result<ConditionalPrefixCoverageV1, ConditionalPrefixDerivationErrorV1>,
    PlironPassPreservationErrorV1,
> {
    let (context, function) = input.endpoints()?;
    let (graph_identity, mutation_epoch) = input.snapshot()?;
    let result = (|| {
        let mut work = 0;
        let model = collect_model(context, function, &mut work)?;
        let proof = derive_model_with_work(&model, &mut work)?;
        let mut host_binding_obligations = model
            .views
            .iter()
            .map(|extent| {
                ConditionalPrefixHostBindingObligationV1::RankedViewToPhysicalAllocationExtent {
                    extent: *extent,
                }
            })
            .collect::<Vec<_>>();
        host_binding_obligations.push(
            ConditionalPrefixHostBindingObligationV1::RankedLaunchToActualDispatch {
                launch: model.launch,
            },
        );
        host_binding_obligations.sort();
        Ok(ConditionalPrefixCoverageV1 {
            graph_identity: graph_identity.clone(),
            mutation_epoch,
            contract: model.contract,
            invocation: model.invocation,
            write: proof.write,
            output: model.views[model.output],
            launch: model.launch,
            source_guard_dnf: proof.source_guard_dnf,
            conditions: proof.conditions,
            host_binding_obligations,
        })
    })();
    input.endpoints()?;
    Ok(result)
}
