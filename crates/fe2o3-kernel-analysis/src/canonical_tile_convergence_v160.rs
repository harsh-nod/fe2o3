//! Conservative, bounded workgroup arrival analysis over an actual V18 owner.
//!
//! Acyclic control reconverges only at its exact postdominator. Cyclic control
//! remains conservative: divergent backedges taint all later loop visits.
//! This does not prove memory definedness, termination, or source refinement.

use crate::{
    CanonicalKirFunctionRefV1, CanonicalKirInventoryV18,
    canonical_kir_private_cell_pair_resources_v1::{Meter, ScopeError, scoped},
};
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Site,
    ExecutionOperationV15 as Execution, IndexKind, IntrinsicKind, OperationKind as Kind,
    Terminator, WorkgroupSize,
};
use std::{fmt, mem::size_of};

#[path = "canonical_tile_reconvergence_v165.rs"]
mod reconvergence;

/// A refusal from an analysis of the exact borrowed canonical owner. Success is
/// conditional on defined execution and does not grant source or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalTileConvergenceErrorV160 {
    /// Work, storage, allocation, arithmetic, or continuing-ledger refusal.
    Resource(Resource),
    /// The coordinate is not a declared kernel entry with a consistent 1-D launch.
    KernelEntry,
    /// An inventory relation did not refer to its exact containing function.
    Inventory,
    /// The reachable operation has no admitted inter-lane arrival semantics.
    UnsupportedArrival(Site),
    /// A collective may be reached under lane-varying control or iteration history.
    VaryingArrival(Site),
    /// A tile input descriptor or base may differ between lanes of a workgroup.
    VaryingTileInput(Site),
    /// The tile lane count differs from the actual declared launch.
    LaunchMismatch(Site),
    /// The bounded analysis unwound; all owned scratch is discarded first.
    Panicked,
}
type Error = CanonicalTileConvergenceErrorV160;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical tile convergence refused: {self:?}")
    }
}
impl std::error::Error for Error {}

struct Graph<'a, 'g> {
    inventory: &'a CanonicalKirInventoryV18<'g>,
    function: &'a CanonicalKirFunctionRefV1<'g>,
    lanes: u16,
    definitions: usize,
    blocks: usize,
    nodes: usize,
}
impl Graph<'_, '_> {
    fn definition(&self, index: usize) -> Result<usize> {
        if !self.function.definitions.contains(&index) {
            return Err(Error::Inventory);
        }
        Ok(index - self.function.definitions.start)
    }
    fn block(&self, index: usize) -> Result<usize> {
        if index >= self.blocks {
            return Err(Error::Inventory);
        }
        Ok(self.definitions + index)
    }
    fn operation(&self, index: usize) -> Result<usize> {
        if !self.function.operations.contains(&index) {
            return Err(Error::Inventory);
        }
        Ok(self.definitions + self.blocks + index - self.function.operations.start)
    }
    fn target(&self, edge: usize) -> Result<usize> {
        let edge = self.inventory.edges().get(edge).ok_or(Error::Inventory)?;
        let target = edge.target.block as usize;
        if edge.target.function != self.function.coordinate || target >= self.blocks {
            return Err(Error::Inventory);
        }
        Ok(target)
    }

    // One intermediate node per operation keeps multi-result dependencies linear
    // in operands plus results, rather than their Cartesian product.
    fn dependencies(
        &self,
        reachable: &[bool],
        postdominance: Option<&reconvergence::Postdominance>,
        meter: &mut Meter<'_, '_, Error>,
        mut visit: impl FnMut(usize, usize) -> Result<()>,
    ) -> Result<()> {
        for (ordinal, block) in self.inventory.blocks()[self.function.blocks.clone()]
            .iter()
            .enumerate()
        {
            meter.work(1)?;
            if !reachable[ordinal] {
                continue;
            }
            let control = self.block(ordinal)?;
            for definition in block.parameters.clone() {
                meter.work(1)?;
                visit(control, self.definition(definition)?)?;
            }
            for index in block.operations.clone() {
                meter.work(1)?;
                let operation = &self.inventory.operations()[index];
                let node = self.operation(index)?;
                visit(control, node)?;
                for definition in operation.results.clone() {
                    meter.work(1)?;
                    visit(node, self.definition(definition)?)?;
                }
                for operand in &self.inventory.uses()[operation.operands.clone()] {
                    meter.work(1)?;
                    visit(self.definition(operand.definition)?, node)?;
                }
            }
            let discriminator = match block.terminator {
                Terminator::ConditionalBranch { condition, .. } => Some(*condition),
                Terminator::Switch { selector, .. }
                | Terminator::IntegerSwitch { selector, .. } => Some(*selector),
                Terminator::Branch { .. } | Terminator::Return { .. } | Terminator::Unreachable => {
                    None
                }
            };
            let mut selector = None;
            if let Some(discriminator) = discriminator {
                for operand in &self.inventory.uses()[block.terminator_uses.clone()] {
                    meter.work(1)?;
                    if operand.value == discriminator {
                        selector = Some(self.definition(operand.definition)?);
                        break;
                    }
                }
                if selector.is_none() {
                    return Err(Error::Inventory);
                }
            }
            for edge in block.edges.clone() {
                meter.work(1)?;
                let mut target = self.target(edge)?;
                if let Some(postdominance) = postdominance {
                    let stop = postdominance.parent(ordinal)?;
                    while target != stop {
                        meter.work(1)?;
                        visit(control, self.block(target)?)?;
                        if let Some(selector) = selector {
                            visit(selector, self.block(target)?)?;
                        }
                        target = postdominance.parent(target)?;
                    }
                } else {
                    visit(control, self.block(target)?)?;
                    if let Some(selector) = selector {
                        visit(selector, self.block(target)?)?;
                    }
                }
                for binding in
                    &self.inventory.edge_arguments()[self.inventory.edges()[edge].bindings.clone()]
                {
                    meter.work(1)?;
                    visit(
                        self.definition(binding.incoming_definition)?,
                        self.definition(binding.target_definition)?,
                    )?;
                    // Reconverged arrival does not make an edge-selected value
                    // uniform. Preserve both incoming-path and edge selection.
                    visit(control, self.definition(binding.target_definition)?)?;
                    if let Some(selector) = selector {
                        visit(selector, self.definition(binding.target_definition)?)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Scratch {
    reachable: Vec<bool>,
    reach_queue: Vec<usize>,
    degrees: Vec<usize>,
    starts: Vec<usize>,
    edges: Vec<usize>,
    varying: Vec<bool>,
    queue: Vec<usize>,
}

fn filled<T: Copy>(meter: &mut Meter<'_, '_, Error>, count: usize, value: T) -> Result<Vec<T>> {
    let (mut rows, _) = meter.table(count)?;
    for _ in 0..count {
        meter.push(&mut rows, value)?;
    }
    Ok(rows)
}

fn varying_seed(kind: &Kind, site: Site) -> Result<bool> {
    Ok(match kind {
        Kind::Constant(_)
        | Kind::Unary { .. }
        | Kind::Binary { .. }
        | Kind::Compare { .. }
        | Kind::Cast { .. }
        | Kind::Select { .. }
        | Kind::SliceLength { .. }
        | Kind::SliceData { .. }
        | Kind::GetElementPointer { .. }
        | Kind::VectorLayoutConvert(_)
        | Kind::Store { .. }
        | Kind::GuardedStore { .. }
        | Kind::Fence(_)
        | Kind::Barrier(_)
        | Kind::WorkgroupBarrier(_)
        | Kind::Execution(Execution::ContextIssue | Execution::WorkgroupDerive { .. }) => false,
        Kind::Intrinsic(intrinsic) => match intrinsic.kind {
            IntrinsicKind::LaunchExtent { .. } => false,
            IntrinsicKind::InvocationIndex { kind, .. } => {
                matches!(kind, IndexKind::Local | IndexKind::Global)
            }
        },
        Kind::Alloca { address_space, .. } => *address_space != AddressSpace::Workgroup,
        Kind::Load { .. }
        | Kind::GuardedLoad { .. }
        | Kind::VectorLoad(_)
        | Kind::Execution(
            Execution::MaskedTileLoadU32 { .. }
            | Execution::TileIntoFragmentU32 { .. }
            | Execution::FragmentIntoPartsU32 { .. }
            | Execution::ScopeEnd { .. },
        ) => true,
        // A missing result is not evidence that an opaque operation returns on
        // every lane. Calls, assembly, and physical collectives refuse here.
        _ => return Err(Error::UnsupportedArrival(site)),
    })
}

fn analyze(graph: &Graph<'_, '_>, meter: &mut Meter<'_, '_, Error>) -> Result<()> {
    meter.reserve(size_of::<Scratch>())?;
    let mut scratch = Scratch::default();
    scratch.reachable = filled(meter, graph.blocks, false)?;
    scratch.reach_queue = meter.table(graph.blocks)?.0;
    scratch.reachable[0] = true;
    meter.push(&mut scratch.reach_queue, 0)?;
    let mut head = 0;
    while head < scratch.reach_queue.len() {
        meter.work(1)?;
        let block = scratch.reach_queue[head];
        head += 1;
        for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
            .edges
            .clone()
        {
            meter.work(1)?;
            let target = graph.target(edge)?;
            if !scratch.reachable[target] {
                scratch.reachable[target] = true;
                meter.push(&mut scratch.reach_queue, target)?;
            }
        }
    }

    scratch.degrees = filled(meter, graph.nodes, 0_usize)?;
    let postdominance = reconvergence::derive(graph, &scratch.reachable, meter)?;
    graph.dependencies(
        &scratch.reachable,
        postdominance.as_ref(),
        meter,
        |from, _| {
            scratch.degrees[from] = scratch.degrees[from]
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
            Ok(())
        },
    )?;
    scratch.starts = meter
        .table(graph.nodes.checked_add(1).ok_or(Resource::Arithmetic)?)?
        .0;
    let mut total = 0_usize;
    meter.push(&mut scratch.starts, total)?;
    for degree in &scratch.degrees {
        total = total.checked_add(*degree).ok_or(Resource::Arithmetic)?;
        meter.push(&mut scratch.starts, total)?;
    }
    scratch.edges = filled(meter, total, 0_usize)?;
    for degree in &mut scratch.degrees {
        meter.work(1)?;
        *degree = 0;
    }
    graph.dependencies(
        &scratch.reachable,
        postdominance.as_ref(),
        meter,
        |from, to| {
            let index = scratch.starts[from]
                .checked_add(scratch.degrees[from])
                .ok_or(Resource::Arithmetic)?;
            if index >= scratch.starts[from + 1] {
                return Err(Error::Inventory);
            }
            scratch.edges[index] = to;
            scratch.degrees[from] += 1;
            Ok(())
        },
    )?;
    scratch.varying = filled(meter, graph.nodes, false)?;
    scratch.queue = meter.table(graph.nodes)?.0;
    for index in graph.function.operations.clone() {
        meter.work(1)?;
        let operation = &graph.inventory.operations()[index];
        if scratch.reachable[operation.coordinate.block.block as usize]
            && varying_seed(&operation.operation.kind, operation.coordinate)?
        {
            let node = graph.operation(index)?;
            scratch.varying[node] = true;
            meter.push(&mut scratch.queue, node)?;
        }
    }
    head = 0;
    while head < scratch.queue.len() {
        meter.work(1)?;
        let node = scratch.queue[head];
        head += 1;
        for edge in scratch.starts[node]..scratch.starts[node + 1] {
            meter.work(1)?;
            let target = scratch.edges[edge];
            if !scratch.varying[target] {
                scratch.varying[target] = true;
                meter.push(&mut scratch.queue, target)?;
            }
        }
    }
    for operation in &graph.inventory.operations()[graph.function.operations.clone()] {
        meter.work(1)?;
        let block = operation.coordinate.block.block as usize;
        if !scratch.reachable[block] {
            continue;
        }
        if matches!(
            operation.operation.kind,
            Kind::Execution(_) | Kind::Barrier(_) | Kind::WorkgroupBarrier(_)
        ) && scratch.varying[graph.block(block)?]
        {
            return Err(Error::VaryingArrival(operation.coordinate));
        }
        if let Kind::Execution(Execution::MaskedTileLoadU32 { lanes, .. }) =
            operation.operation.kind
        {
            if lanes != graph.lanes {
                return Err(Error::LaunchMismatch(operation.coordinate));
            }
            let operands = &graph.inventory.uses()[operation.operands.clone()];
            if operands.len() != 3 {
                return Err(Error::Inventory);
            }
            for operand in &operands[1..] {
                meter.work(1)?;
                if scratch.varying[graph.definition(operand.definition)?] {
                    return Err(Error::VaryingTileInput(operation.coordinate));
                }
            }
        }
    }
    Ok(())
}

/// Checks uniform tile descriptors, bases, and workgroup arrival in the actual
/// borrowed graph. ABI uniformity is assumed only for a declared kernel entry.
/// Reachable opaque operations refuse. Acyclic arrival reconverges only at exact
/// postdominators; edge-selected values remain tainted. Cyclic control retains
/// divergence through joins and backedges. Scratch and work are fully charged
/// to the continuing ledger, and no result storage or authority is retained.
///
/// Acyclic postdominance costs O((blocks + CFG edges) log blocks); propagation is
/// linear in definitions, operations, uses, edges, and derived control dependencies.
/// Enumerating control dependencies can require O(CFG edges * blocks) in the
/// worst case. Each postdominator-chain visit is charged to the work budget.
/// Memory validity, undefined arithmetic, and original-source semantics remain
/// independent obligations; this analysis is conditional on defined execution.
pub fn check_canonical_tile_convergence_v160(
    inventory: &CanonicalKirInventoryV18<'_>,
    coordinate: Function,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(budget, |meter| {
        meter.reserve(size_of::<Graph<'_, '_>>())?;
        meter.work(1)?;
        let function = inventory
            .functions()
            .get(coordinate.0 as usize)
            .filter(|row| row.coordinate == coordinate && !row.blocks.is_empty())
            .ok_or(Error::KernelEntry)?;
        let mut geometry = None;
        for kernel in inventory.kernels() {
            meter.work(1)?;
            if kernel.entry != coordinate {
                continue;
            }
            let Some(size) = kernel.kernel.workgroup_size else {
                return Err(Error::KernelEntry);
            };
            if size.y != 1
                || size.z != 1
                || !(1..=256).contains(&size.x)
                || geometry.is_some_and(|prior| prior != size)
            {
                return Err(Error::KernelEntry);
            }
            geometry = Some(size);
        }
        let Some(WorkgroupSize { x, .. }) = geometry else {
            return Err(Error::KernelEntry);
        };
        let definitions = function.definitions.len();
        let blocks = function.blocks.len();
        let nodes = definitions
            .checked_add(blocks)
            .and_then(|count| count.checked_add(function.operations.len()))
            .ok_or(Resource::Arithmetic)?;
        analyze(
            &Graph {
                inventory,
                function,
                lanes: x as u16,
                definitions,
                blocks,
                nodes,
            },
            meter,
        )
    })
}

#[cfg(test)]
#[path = "canonical_tile_convergence_v160_tests.rs"]
mod tests;
