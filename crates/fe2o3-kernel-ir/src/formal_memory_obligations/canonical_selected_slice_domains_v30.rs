//! Selected slice accesses retain conditional origins at actual incoming edges.
//! Neither a root number nor a structurally seeded cycle is an allocation proof.
use super::*;
use crate::{Axis, ExplicitLaunchExtent, FormalIndexWidth, ScalarType};
use runtime_slice_read_v1::ReadIndex;
use std::ops::Range;

#[path = "canonical_selected_slice_graph_v30.rs"]
mod graph;
use graph::PointerGraphV30;
pub use graph::{
    CanonicalSelectedPointerIncomingV30, CanonicalSelectedPointerNodeV30,
    CanonicalSelectedPointerStepV30,
};

/// Actual location where a concrete leaf enters the selected pointer graph.
/// This is distinct from the successful guard edge proving its bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSelectedSliceInjectionV30 {
    Access,
    Incoming(usize),
}

/// Descriptive local bounds, never a Store-to-read certificate conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSelectedSliceDomainV30 {
    Read(FormalRuntimeSliceReadDomainV1),
    Store(FormalRuntimeSliceReadDomainV1),
}

impl CanonicalSelectedSliceDomainV30 {
    pub const fn writing(self) -> bool {
        matches!(self, Self::Store(_))
    }
    pub const fn allocation(self) -> FormalAllocationIdentity {
        match self {
            Self::Read(row) | Self::Store(row) => row.allocation(),
        }
    }
    pub const fn slice(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.slice(),
        }
    }
    pub const fn pointer(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.pointer(),
        }
    }
    pub const fn index(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.index(),
        }
    }
    pub const fn guard_index(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.guard_index(),
        }
    }
    pub const fn length(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.length(),
        }
    }
    pub const fn predicate(self) -> ValueId {
        match self {
            Self::Read(row) | Self::Store(row) => row.predicate(),
        }
    }
    pub const fn path(self) -> FormalGuardedPathV1 {
        match self {
            Self::Read(row) | Self::Store(row) => row.path(),
        }
    }
    pub const fn element_bytes(self) -> u64 {
        match self {
            Self::Read(row) | Self::Store(row) => row.element_bytes(),
        }
    }
}

/// An inert ordered choice. Its conditional facts are valid only while borrowed
/// from the same checked batch, under its exact source/runtime premises.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSelectedSliceChoiceV30 {
    pub injection: CanonicalSelectedSliceInjectionV30,
    pub leaf: usize,
    pub domain: CanonicalSelectedSliceDomainV30,
    pub root_space: AddressSpace,
    pub root_access: AccessMode,
    pub index_origin: CanonicalGuardedReadIndexOriginV1,
    pub length_origin: ValueId,
    pub invocation_projection: Option<(Axis, ValueId)>,
    /// True means the guard dominates the memory access itself. Otherwise it
    /// dominates this exact incoming edge's source before the pointer transfer.
    pub at_access: bool,
}

pub struct CanonicalSelectedSliceAccessV30 {
    operation: Coordinate,
    pointer: ValueId,
    value: ValueId,
    scalar: ScalarType,
    memory: MemoryAccess,
    writing: bool,
    node: usize,
    choices: Range<usize>,
}

impl CanonicalSelectedSliceAccessV30 {
    pub const fn operation(&self) -> Coordinate {
        self.operation
    }
    pub const fn pointer(&self) -> ValueId {
        self.pointer
    }
    pub const fn value(&self) -> ValueId {
        self.value
    }
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    pub const fn memory(&self) -> MemoryAccess {
        self.memory
    }
    pub const fn writing(&self) -> bool {
        self.writing
    }
    pub const fn root_node(&self) -> usize {
        self.node
    }
}

/// One exact actual Slice parameter and its undischarged runtime requirements.
/// Distinct parameter ordinals never prove disjoint allocations.
pub struct CanonicalSelectedSliceParameterV30 {
    value: ValueId,
    scalar: ScalarType,
    space: AddressSpace,
    access: AccessMode,
    reads: usize,
    writes: usize,
    axis: Option<Axis>,
    different_projection: bool,
}

impl CanonicalSelectedSliceParameterV30 {
    pub const fn value(&self) -> ValueId {
        self.value
    }
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    pub const fn space(&self) -> AddressSpace {
        self.space
    }
    pub const fn access(&self) -> AccessMode {
        self.access
    }
    pub const fn reads(&self) -> usize {
        self.reads
    }
    pub const fn writes(&self) -> usize {
        self.writes
    }
    pub const fn invocation_axis(&self) -> Option<Axis> {
        self.axis
    }
    pub const fn requires_valid_aligned_extent(&self) -> bool {
        self.reads != 0 || self.writes != 0
    }
    pub const fn requires_initialized_extent(&self) -> bool {
        self.reads != 0
    }
    pub const fn requires_exclusive_runtime_binding(&self) -> bool {
        self.writes != 0
    }
    pub const fn requires_exact_launch_binding(&self) -> bool {
        self.writes != 0
    }
}

struct SelectedFunctionV30 {
    graph: PointerGraphV30,
    parameters: Vec<Option<CanonicalSelectedSliceParameterV30>>,
    accesses: Vec<CanonicalSelectedSliceAccessV30>,
    choices: Vec<CanonicalSelectedSliceChoiceV30>,
    launch: ExplicitLaunchExtent,
    counts: [usize; 2],
}

/// Conditional whole-module family for actual selected pointers. All selected
/// casts and incoming edges are interpreted, every reachable node must be
/// supported and seeded, and every leaf injection has an actual checked bound.
/// Source ownership, initialization, actual extents, cross-root nonaliasing and
/// the exact launch binding remain mandatory separate premises.
pub struct CheckedCanonicalSelectedSliceDomainsV30<'scope, 'graph> {
    owner: &'graph VerifiedCanonicalKernelIrModuleV18,
    functions: &'scope [SelectedFunctionV30],
    width: FormalIndexWidth,
    accounting: &'scope Accounting,
}

impl<'g> CheckedCanonicalSelectedSliceDomainsV30<'_, 'g> {
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize> {
        self.accounting.enter(budget)?;
        Ok(self.functions.len())
    }
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g VerifiedCanonicalKernelIrModuleV18> {
        self.accounting.enter(budget)?;
        Ok(self.owner)
    }
    pub fn refuse_retained_custody(&self) -> Failure {
        self.accounting.refuse_retained_custody()
    }
    fn function(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<&SelectedFunctionV30> {
        self.accounting.enter(budget)?;
        let result = match self.functions.get(function.0 as usize) {
            Some(row) => Ok(row),
            None => Err(Failure::Coordinate(Coordinate {
                block: BlockCoordinate { function, block: 0 },
                operation: 0,
            })),
        };
        self.accounting.save(result)
    }
    pub fn function_conditions(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)> {
        let row = self.function(function, budget)?;
        Ok((row.launch, self.width, row.counts[0], row.counts[1]))
    }
    pub fn pointer_nodes(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalSelectedPointerNodeV30]> {
        Ok(&self.function(function, budget)?.graph.nodes)
    }
    pub fn incoming_edges(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalSelectedPointerIncomingV30]> {
        Ok(&self.function(function, budget)?.graph.incoming)
    }
    pub fn parameters(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<&[Option<CanonicalSelectedSliceParameterV30>]> {
        Ok(&self.function(function, budget)?.parameters)
    }
    pub fn accesses(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalSelectedSliceAccessV30]> {
        Ok(&self.function(function, budget)?.accesses)
    }
    pub fn access_at(
        &self,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalSelectedSliceAccessV30>> {
        let function = self.function(coordinate.block.function, budget)?;
        let found = verification_find_last_by_v1(&function.accesses, 2, budget, |row| {
            row.operation.cmp(&coordinate)
        });
        let result = match found {
            Ok(Some(index)) => Ok(Some(&function.accesses[index])),
            Ok(None) => Ok(None),
            Err(error) => Err(error.into()),
        };
        self.accounting.save(result)
    }
    pub fn choices_at(
        &self,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&[CanonicalSelectedSliceChoiceV30]>> {
        let Some(access) = self.access_at(coordinate, budget)? else {
            return Ok(None);
        };
        let function = &self.functions[coordinate.block.function.0 as usize];
        let result = match function.choices.get(access.choices.clone()) {
            Some(rows) => Ok(Some(rows)),
            None => Err(ResourceError::Accounting.into()),
        };
        self.accounting.save(result)
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub const fn source_and_runtime_requirements_are_discharged(&self) -> bool {
        false
    }
}

#[path = "canonical_selected_slice_build_v30.rs"]
mod build;

#[path = "canonical_selected_slice_scope_v30.rs"]
mod scope;
pub use scope::with_canonical_selected_slice_domains_v30;

#[cfg(test)]
#[path = "canonical_selected_slice_domains_v30_tests.rs"]
mod tests;
