//! Generic projection from admitted semantic MIR into safety-verifiable ranked PLIRON.
//!
//! Static proof facts come from the indexed place and its semantic array type.
//! Rust bounds-assert terminators are retained in semantic MIR but do not
//! manufacture a static extent or authorize an access in this projection.

use std::{collections::VecDeque, fmt};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, DYNAMIC_EXTENT, IndexBinaryKindAttr,
    MAX_RANKED_MEMORY_RANK, MemorySpaceAttr, SUPPORTED_ELEMENT_WIDTHS,
};
use fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_OPERATIONS;
use fe2o3_lower_mir_kernel::{
    ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirErrorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAtomicAccessV1, SemanticAtomicOrderingV1, SemanticAtomicScopeV1, SemanticBlockIdV1,
    SemanticCallableDeclV1, SemanticCallableIdV1, SemanticCompilerIntrinsicOperationV1,
    SemanticConstantValueV1, SemanticDirectCallV1, SemanticDirectTailCallV1,
    SemanticDisjointIndexSpaceV1, SemanticFunctionDeclV1, SemanticFunctionRoleV1,
    SemanticLocalIdV1, SemanticLocalRoleV1, SemanticOperandV1, SemanticPlaceV1,
    SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticSourceProvenanceV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
    SemanticUnwindActionV1,
};
use fe2o3_mir_model::{
    SemanticOptionAvailabilityV1, SemanticOptionDominanceV1, semantic_option_producers_v1,
};

use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelErrorV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1, ProductionSessionErrorV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

const ROOT_NAME_V1: &str = "semantic_safety_module";
// Leave one operation for the ranked function terminator.
const MAX_PROJECTED_OPERATIONS_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS - 1;
// Diagnostics are retained only until this bounded projection is consumed.
const MAX_PROJECTED_RANKED_IR_BYTES_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS * 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedAccessSourceV1 {
    block: usize,
    operation: usize,
    access: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    source: SemanticSourceProvenanceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GuardedDisjointAccessV1 {
    view: ProductionRankedValueIdV1,
    allocation_origin: u32,
    index: ProductionRankedValueV1,
    precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    extent_argument: u32,
    access: AccessKindAttr,
    source: SemanticSourceProvenanceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuardPredicateV1 {
    comparisons: Vec<(ProductionRankedValueV1, ProductionRankedValueV1)>,
}

impl GuardPredicateV1 {
    fn for_access(access: &GuardedDisjointAccessV1) -> Self {
        let mut comparisons = Vec::with_capacity(2);
        if let Some(precondition) = access.precondition {
            comparisons.push(precondition);
        }
        comparisons.push((
            access.index,
            ProductionRankedValueV1::Argument(access.extent_argument),
        ));
        Self { comparisons }
    }

    fn from_precondition(
        precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    ) -> Self {
        Self {
            comparisons: precondition.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GuardedAccessSiteV1 {
    insertion_operation: usize,
    access: GuardedDisjointAccessV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedDisjointIndexV1 {
    value: ProductionRankedValueV1,
    mapping: SemanticDisjointIndexSpaceV1,
    precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    availability: Option<SemanticOptionAvailabilityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedGridLeaderV1 {
    grid_leader: SemanticTypeIdV1,
    precondition: (ProductionRankedValueV1, ProductionRankedValueV1),
    availability: SemanticOptionAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapabilityEdgeKindV1 {
    Alias,
    AuthenticatedOptionPayload,
    IntoDisjoint {
        mapping: SemanticDisjointIndexSpaceV1,
    },
    CheckedShift {
        mapping: SemanticDisjointIndexSpaceV1,
        offset: u64,
        availability: SemanticOptionAvailabilityV1,
    },
    CheckedBlock {
        mapping: SemanticDisjointIndexSpaceV1,
        elements_per_lane: u64,
        availability: SemanticOptionAvailabilityV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapabilityEdgeV1 {
    destination: usize,
    use_block: usize,
    kind: CapabilityEdgeKindV1,
}

struct IntrinsicProjectionV1 {
    checked_reference_origins: Vec<Option<usize>>,
    guarded_accesses: Vec<GuardedDisjointAccessV1>,
    option_predicates: Vec<Option<GuardPredicateV1>>,
    extent_argument_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedViewV1 {
    result: ProductionRankedValueIdV1,
    element_width: u32,
    shape: Vec<u64>,
    memory_space: MemorySpaceAttr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedEffectSourceV1 {
    access: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    source: SemanticSourceProvenanceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedBlockItemV1 {
    Effect {
        operation: ProductionRankedOperationV1,
        source: Option<ProjectedEffectSourceV1>,
    },
    Guarded(GuardedDisjointAccessV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedSemanticBlockV1 {
    items: Vec<ProjectedBlockItemV1>,
}

impl ProjectedSemanticBlockV1 {
    fn has_memory_access(&self) -> bool {
        self.items.iter().any(|item| match item {
            ProjectedBlockItemV1::Effect {
                operation:
                    ProductionRankedOperationV1::Access { .. }
                    | ProductionRankedOperationV1::AtomicAccess { .. },
                ..
            }
            | ProjectedBlockItemV1::Guarded(_) => true,
            ProjectedBlockItemV1::Effect { .. } => false,
        })
    }

    fn has_concurrent_memory_access(&self) -> bool {
        self.items.iter().any(|item| match item {
            ProjectedBlockItemV1::Effect {
                source: Some(source),
                ..
            } => source.memory_space != MemorySpaceAttr::Private,
            ProjectedBlockItemV1::Guarded(_) => true,
            ProjectedBlockItemV1::Effect { source: None, .. } => false,
        })
    }
}

/// Move-only result retaining both the exact admitted Rust semantics and the
/// owner-held PLIRON graph that passed every mandatory generic kernel check.
pub(crate) struct ProductionRankedSemanticProgramV1 {
    receipt: ProductionRankedSemanticProjectionReceiptV1,
}

impl ProductionRankedSemanticProgramV1 {
    pub(crate) fn ranked_ir(&self) -> &str {
        self.receipt.ranked_ir()
    }

    pub(crate) fn function_name(&self) -> &str {
        self.receipt.lowering().kernel().function_name()
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.receipt.semantic().semantic().functions().len()
    }

    pub(crate) fn semantic_callable_count(&self) -> usize {
        self.receipt.semantic().semantic().callables().len()
    }

    pub(crate) fn bounds_are_clean(&self) -> bool {
        self.receipt.lowering().bounds_report().is_clean()
    }

    pub(crate) fn all_kernel_checks_are_clean(&self) -> bool {
        self.receipt.lowering().all_mandatory_reports_are_clean()
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn into_ranked_receipt(self) -> ProductionRankedSemanticProjectionReceiptV1 {
        self.receipt
    }
}

#[derive(Debug)]
pub(crate) enum ProductionRankedProjectionErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    Incomplete(&'static str),
    Unsupported(&'static str),
    Recipe(ProductionRankedKernelErrorV1),
    Custody(ProductionSemanticKirErrorV1),
    Construction(fe2o3_pliron::NameError),
    Compile {
        error: ProductionRankedCompileErrorV1,
        ranked_ir: String,
        access_sources: Vec<ProjectedAccessSourceV1>,
    },
}

impl fmt::Display for ProductionRankedProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticOwner(error) => {
                write!(formatter, "exact semantic middle end failed: {error}")
            }
            Self::Custody(error) => write!(formatter, "ranked proof custody failed: {error}"),
            Self::Unsupported(detail) => {
                write!(formatter, "semantic-to-ranked projection rejected {detail}")
            }
            Self::Incomplete(detail) => {
                write!(
                    formatter,
                    "semantic-to-ranked projection incomplete: {detail}"
                )
            }
            Self::Recipe(error) => write!(formatter, "semantic-to-ranked recipe failed: {error}"),
            Self::Construction(error) => write!(
                formatter,
                "semantic-to-ranked construction name was rejected: {error:?}",
            ),
            Self::Compile {
                error,
                ranked_ir,
                access_sources,
            } => {
                error.fmt(formatter)?;
                if let ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::RankedBounds(bounds),
                ) = error
                {
                    for finding in bounds.report().findings() {
                        if let fe2o3_kernel_analysis::RankedBoundsFindingV1::StaticOutOfBounds {
                            block,
                            operation,
                            ..
                        }
                        | fe2o3_kernel_analysis::RankedBoundsFindingV1::UnprovedBound {
                            block,
                            operation,
                            ..
                        } = finding
                            && let Some(access) = access_sources.iter().find(|source| {
                                source.block == *block && source.operation == *operation
                            })
                        {
                            write!(formatter, "\n  --> {}", source_label(access.source))?;
                            write!(
                                formatter,
                                "\n  = Rust {:?} projected to kernel.access at block {} op {}",
                                access.access, access.block, access.operation,
                            )?;
                        }
                    }
                }
                write!(
                    formatter,
                    "\n  = ranked PLIRON before rejected lowering:\n{}\n  = lowering stopped before target IR or artifact emission",
                    indent_ir(ranked_ir),
                )
            }
        }
    }
}

impl std::error::Error for ProductionRankedProjectionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::Recipe(error) => Some(error),
            Self::Compile { error, .. } => Some(error),
            Self::Incomplete(_) | Self::Unsupported(_) | Self::Construction(_) => None,
            Self::Custody(error) => Some(error),
        }
    }
}

pub(crate) fn project_and_verify_ranked_semantic_mir_v1(
    semantic_owner: ProductionSemanticMirOwnerV1,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    semantic_owner
        .verify_equivalence()
        .map_err(ProductionRankedProjectionErrorV1::SemanticOwner)?;
    let semantic = semantic_owner.semantic();
    if semantic.roots().len() != 1 || semantic.functions().len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic closure that is not one kernel root without helpers",
        ));
    }
    let root = semantic.roots()[0];
    let function = semantic.functions().get(root.index() as usize).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("an out-of-range semantic kernel root"),
    )?;
    if function.role() != SemanticFunctionRoleV1::KernelRoot {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a root without the KernelRoot role",
        ));
    }

    let constants = constant_locals(function);
    let mut entry_operations = Vec::new();
    let mut next_value = 0_u32;
    let mut incomplete = None;
    let mut projected_views = vec![None; function.locals().len()];
    let mut discarded_ir = String::new();
    let intrinsic = project_intrinsic_contracts(
        semantic.callables(),
        semantic.types(),
        function,
        &constants,
        &mut entry_operations,
        &mut next_value,
        &mut discarded_ir,
    )?;
    let switch_predicates = switch_predicates(function, &intrinsic.option_predicates)?;
    let mut projected_blocks = Vec::new();
    let mut projected_effect_count = 0_usize;
    for block in function.blocks() {
        let mut operations = Vec::new();
        let mut guarded_sites = Vec::new();
        let mut local_sources = Vec::new();
        for statement in block.statements() {
            retain_incomplete(
                project_statement_accesses(
                    semantic.types(),
                    function,
                    statement,
                    &constants,
                    &intrinsic.checked_reference_origins,
                    &intrinsic.guarded_accesses,
                    &mut guarded_sites,
                    &mut projected_views,
                    &mut operations,
                    &mut local_sources,
                    &mut next_value,
                    &mut discarded_ir,
                ),
                &mut incomplete,
            )?;
        }
        retain_incomplete(
            project_terminator_accesses(
                semantic.callables(),
                semantic.types(),
                function,
                block.terminator().kind(),
                block.terminator().source(),
                &constants,
                &intrinsic.checked_reference_origins,
                &intrinsic.guarded_accesses,
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut local_sources,
                &mut next_value,
                &mut discarded_ir,
            ),
            &mut incomplete,
        )?;
        let projected = order_projected_block_effects(
            operations,
            guarded_sites,
            local_sources,
            &mut entry_operations,
        )?;
        projected_effect_count = projected_effect_count
            .checked_add(projected.items.len())
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection operation count overflow",
            ))?;
        if projected_effect_count
            .checked_add(entry_operations.len())
            .is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS)
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection exceeds the ranked operation limit",
            ));
        }
        projected_blocks.push(projected);
    }
    if !projected_blocks
        .iter()
        .any(ProjectedSemanticBlockV1::has_memory_access)
    {
        if let Some(detail) = incomplete {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
        }
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access",
        ));
    }
    if projected_blocks
        .iter()
        .any(ProjectedSemanticBlockV1::has_concurrent_memory_access)
        && !entry_operations.iter().any(|operation| {
            matches!(
                operation,
                ProductionRankedOperationV1::InvocationIndex { .. }
            )
        })
    {
        incomplete.get_or_insert(
            "a concurrent memory effect before exact invocation-index projection is available",
        );
    }
    let (blocks, sources) = build_ranked_cfg(
        function,
        &switch_predicates,
        entry_operations,
        projected_blocks,
    )?;
    let ranked_ir = format_ranked_cfg(function_name(function)?, &blocks)?;

    let kernel = ProductionRankedKernelV1::new(
        function_name(function)?,
        intrinsic.extent_argument_count,
        blocks,
    )
    .map_err(ProductionRankedProjectionErrorV1::Recipe)?;
    let construction = ProductionConstructionV1::ranked_kernel(ROOT_NAME_V1, kernel)
        .map_err(ProductionRankedProjectionErrorV1::Construction)?;
    let lowering =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .map_err(|error| ProductionRankedProjectionErrorV1::Compile {
            error,
            ranked_ir: ranked_ir.clone(),
            access_sources: sources,
        })?;
    if let Some(detail) = incomplete {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
    }
    let receipt = ProductionRankedSemanticProjectionReceiptV1::assert_compiler_internal_projection(
        semantic_owner,
        lowering,
        ranked_ir,
    )
    .map_err(ProductionRankedProjectionErrorV1::Custody)?;
    Ok(ProductionRankedSemanticProgramV1 { receipt })
}

fn project_intrinsic_contracts(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<IntrinsicProjectionV1, ProductionRankedProjectionErrorV1> {
    let local_count = function.locals().len();
    let mut index_values = vec![None; local_count];
    let mut grid_leaders = vec![None; local_count];
    let mut option_predicates = vec![None; local_count];
    let mut edges_by_source = vec![Vec::new(); local_count];
    let option_producers = semantic_option_producers_v1(function, callables)
        .map_err(|error| ProductionRankedProjectionErrorV1::Unsupported(error.detail()))?;
    let option_dominance = SemanticOptionDominanceV1::analyze(function, &option_producers)
        .map_err(|error| ProductionRankedProjectionErrorV1::Unsupported(error.detail()))?;
    let mut edge_count = 0_usize;
    let mut borrowed_locals = Vec::new();
    let launch_extent = required_launch_extent_x(function);

    for (block_index, block) in function.blocks().iter().enumerate() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let (source, borrowed) = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => (transparent_operand_place(operand), false),
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.projections().is_empty() =>
                {
                    (Some(place), true)
                }
                _ => (None, false),
            };
            let Some(source) = source else {
                continue;
            };
            let source = source.local().index() as usize;
            let destination = assignment.destination().local().index() as usize;
            if borrowed {
                if borrowed_locals.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "borrowed capability uses exceed the charged projection limit",
                    ));
                }
                borrowed_locals.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "borrowed capability use storage cannot be reserved",
                    )
                })?;
                borrowed_locals.push((source, block_index));
            }
            push_capability_edge(
                &mut edges_by_source,
                &mut edge_count,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: block_index,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )?;
        }

        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let kind = match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                index_space, ..
            } => CapabilityEdgeKindV1::IntoDisjoint {
                mapping: *index_space,
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                output_space,
                offset,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                output_space,
                offset,
                ..
            } => {
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked shift lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedShift {
                    mapping: *output_space,
                    offset: *offset,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                output_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                if *lanes_per_block != 1 {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a blocked mapping with more than one lane before quotient facts are available",
                    ));
                }
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || *elements_per_lane == 0
                    || lanes_per_block.checked_mul(*elements_per_lane).is_none()
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed blocked mapping reached ranked projection",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked block lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping: expected,
                    elements_per_lane: *elements_per_lane,
                    availability,
                }
            }
            _ => continue,
        };
        let destination = simple_call_destination(call)?.index() as usize;
        let source = call
            .arguments()
            .first()
            .and_then(simple_operand_local)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "an index capability transform without one exact input local",
            ))?
            .index() as usize;
        push_capability_edge(
            &mut edges_by_source,
            &mut edge_count,
            source,
            CapabilityEdgeV1 {
                destination,
                use_block: block_index,
                kind,
            },
        )?;
    }

    let mut index_worklist = VecDeque::new();
    let mut grid_worklist = VecDeque::new();
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if !matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
        ) {
            continue;
        }
        let destination = simple_call_destination(call)?;
        let destination = destination.index() as usize;
        if destination >= local_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an invocation-capability destination outside the semantic local table",
            ));
        }
        if index_values[destination].is_some() || grid_leaders[destination].is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple invocation capabilities for one semantic local",
            ));
        }
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension: 0,
            launch_extent,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "  %{} = kernel.invocation_index <0, dynamic>\n",
                result.get()
            ),
        )?;
        match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => {
                index_values[destination] = Some(ProjectedDisjointIndexV1 {
                    value: ProductionRankedValueV1::Local(result),
                    mapping: SemanticDisjointIndexSpaceV1::Index1d,
                    precondition: None,
                    availability: None,
                });
                index_worklist.push_back(destination);
            }
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader } => {
                let availability = option_dominance
                    .availability(SemanticLocalIdV1::from_index(destination as u32))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a grid-leader capability lacks authenticated Option Some availability",
                    ))?;
                reserve_operation(operations)?;
                let one = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: one,
                    value: 1,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!("  %{} = kernel.index_constant 1\n", one.get()),
                )?;
                option_predicates[destination] = Some(GuardPredicateV1 {
                    comparisons: vec![(
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    )],
                });
                grid_leaders[destination] = Some(ProjectedGridLeaderV1 {
                    grid_leader: *grid_leader,
                    precondition: (
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    ),
                    availability,
                });
                grid_worklist.push_back(destination);
            }
            _ => unreachable!(),
        }
    }

    // rustc may erase the move that binds an unforgeable zero-sized payload
    // from `Option::Some`. Recover that edge only when one exact authenticated
    // producer controls the borrow's block and its payload type matches.
    for (borrowed_local, use_block) in borrowed_locals {
        if grid_leaders[borrowed_local].is_some() {
            continue;
        }
        let borrowed_type = function.locals()[borrowed_local].ty();
        let use_block_id = SemanticBlockIdV1::from_index(use_block as u32);
        let mut producer = None;
        for (candidate_local, candidate) in grid_leaders.iter().copied().enumerate() {
            let Some(candidate) = candidate else {
                continue;
            };
            if candidate.grid_leader != borrowed_type
                || !option_dominance.allows(candidate.availability, use_block_id)
            {
                continue;
            }
            if producer.replace(candidate_local).is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a borrowed zero-sized capability has multiple active Option producers",
                ));
            }
        }
        if let Some(producer) = producer {
            push_capability_edge(
                &mut edges_by_source,
                &mut edge_count,
                producer,
                CapabilityEdgeV1 {
                    destination: borrowed_local,
                    use_block,
                    kind: CapabilityEdgeKindV1::AuthenticatedOptionPayload,
                },
            )?;
        }
    }

    let mut processed_edges = 0_usize;
    while let Some(source) = index_worklist.pop_front() {
        let input = index_values[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "the capability worklist lost an index value",
        ))?;
        for edge in &edges_by_source[source] {
            processed_edges = processed_edges.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "capability work accounting overflowed",
                ),
            )?;
            if !input.availability.is_none_or(|availability| {
                option_dominance.allows(
                    availability,
                    SemanticBlockIdV1::from_index(edge.use_block as u32),
                )
            }) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an index capability is used outside its authenticated Some edge",
                ));
            }
            let projected = match edge.kind {
                CapabilityEdgeKindV1::Alias | CapabilityEdgeKindV1::AuthenticatedOptionPayload => {
                    input
                }
                CapabilityEdgeKindV1::IntoDisjoint { mapping } => {
                    ProjectedDisjointIndexV1 { mapping, ..input }
                }
                CapabilityEdgeKindV1::CheckedShift {
                    mapping,
                    offset,
                    availability,
                } => {
                    reserve_operation(operations)?;
                    let offset_value = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: offset_value,
                        value: offset,
                    });
                    reserve_operation(operations)?;
                    let shifted = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexBinary {
                        result: shifted,
                        kind: IndexBinaryKindAttr::Add,
                        lhs: input.value,
                        rhs: ProductionRankedValueV1::Local(offset_value),
                    });
                    push_ranked_ir(
                        ranked_ir,
                        &format!(
                            "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add {}, %{}\n",
                            offset_value.get(),
                            offset,
                            shifted.get(),
                            ranked_value_text_v1(input.value),
                            offset_value.get(),
                        ),
                    )?;
                    let precondition = if offset == 0 {
                        input.precondition
                    } else {
                        reserve_operation(operations)?;
                        let upper = next_value_id(next_value)?;
                        operations.push(ProductionRankedOperationV1::IndexConstant {
                            result: upper,
                            value: u64::MAX - offset + 1,
                        });
                        push_ranked_ir(
                            ranked_ir,
                            &format!(
                                "  %{} = kernel.index_constant {}\n",
                                upper.get(),
                                u64::MAX - offset + 1,
                            ),
                        )?;
                        Some((input.value, ProductionRankedValueV1::Local(upper)))
                    };
                    ProjectedDisjointIndexV1 {
                        value: ProductionRankedValueV1::Local(shifted),
                        mapping,
                        precondition,
                        availability: Some(availability),
                    }
                }
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping,
                    elements_per_lane,
                    availability,
                } => {
                    let maximum_raw = (u64::MAX - (elements_per_lane - 1)) / elements_per_lane;
                    reserve_operation(operations)?;
                    let upper = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: upper,
                        value: maximum_raw + 1,
                    });
                    push_ranked_ir(
                        ranked_ir,
                        &format!(
                            "  %{} = kernel.index_constant {}\n",
                            upper.get(),
                            maximum_raw + 1,
                        ),
                    )?;
                    ProjectedDisjointIndexV1 {
                        mapping,
                        precondition: Some((input.value, ProductionRankedValueV1::Local(upper))),
                        availability: Some(availability),
                        ..input
                    }
                }
            };
            if matches!(
                edge.kind,
                CapabilityEdgeKindV1::CheckedShift { .. }
                    | CapabilityEdgeKindV1::CheckedBlock { .. }
            ) {
                let predicate = option_predicates.get_mut(edge.destination).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a checked capability destination outside the semantic local table",
                    ),
                )?;
                if predicate.is_some() {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "multiple checked predicates for one semantic local",
                    ));
                }
                *predicate = Some(GuardPredicateV1::from_precondition(projected.precondition));
            }
            assign_index_capability(
                edge.destination,
                projected,
                &mut index_values,
                &grid_leaders,
                &mut index_worklist,
            )?;
        }
    }

    while let Some(source) = grid_worklist.pop_front() {
        let input = grid_leaders[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "the capability worklist lost grid-leader authority",
        ))?;
        for edge in &edges_by_source[source] {
            if !matches!(
                edge.kind,
                CapabilityEdgeKindV1::Alias | CapabilityEdgeKindV1::AuthenticatedOptionPayload
            ) {
                continue;
            }
            if !option_dominance.allows(
                input.availability,
                SemanticBlockIdV1::from_index(edge.use_block as u32),
            ) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "grid-leader authority is aliased outside its authenticated Some edge",
                ));
            }
            processed_edges = processed_edges.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "capability work accounting overflowed",
                ),
            )?;
            let destination = edge.destination;
            if destination >= grid_leaders.len() || index_values[destination].is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a grid-leader alias escaped the semantic local table or changed capability kind",
                ));
            }
            match grid_leaders[destination] {
                None => {
                    grid_leaders[destination] = Some(input);
                    grid_worklist.push_back(destination);
                }
                Some(existing) if existing == input => {}
                Some(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "multiple grid-leader capabilities reach one semantic local",
                    ));
                }
            }
        }
    }
    if processed_edges > edge_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability worklist exceeded its charged def-use edges",
        ));
    }

    let allocation_origins = local_allocation_origins(function)?;
    let mut views_by_origin: Vec<Option<ProjectedViewV1>> = vec![None; function.locals().len()];
    let mut admitted_writable_origin = None;
    let mut guarded_accesses = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let (element, index, precondition) = match operation {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { element, .. } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    block_index,
                )?;
                if projected.mapping != SemanticDisjointIndexSpaceV1::Index1d {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "identity accessor received a non-identity mapping",
                    ));
                }
                (*element, projected.value, projected.precondition)
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                element,
                index_space,
                ..
            } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    block_index,
                )?;
                if projected.mapping != *index_space {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "disjoint accessor mapping identity changed",
                    ));
                }
                (*element, projected.value, projected.precondition)
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                element,
                grid_leader,
                ..
            } => {
                let leader_local = call
                    .arguments()
                    .get(1)
                    .and_then(simple_operand_local)
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "an exclusive access without one exact grid-leader local",
                    ))?
                    .index() as usize;
                let leader = grid_leaders.get(leader_local).copied().flatten().ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "an exclusive access without authenticated grid-leader authority",
                    ),
                )?;
                if leader.grid_leader != *grid_leader {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "grid-leader capability type identity changed",
                    ));
                }
                if !option_dominance.allows(
                    leader.availability,
                    SemanticBlockIdV1::from_index(block_index as u32),
                ) {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "grid-leader authority is used outside its authenticated Some edge",
                    ));
                }
                let value = call
                    .arguments()
                    .get(2)
                    .and_then(|operand| constant_operand_value(operand, constants))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic grid-exclusive index requires a deliberate ranked argument projection",
                    ))?;
                reserve_operation(operations)?;
                let constant_index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: constant_index,
                    value,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_constant {}\n",
                        constant_index.get(),
                        value,
                    ),
                )?;
                reserve_operation(operations)?;
                let index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: index,
                    kind: IndexBinaryKindAttr::Add,
                    lhs: leader.precondition.0,
                    rhs: ProductionRankedValueV1::Local(constant_index),
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_binary Add {}, %{}\n",
                        index.get(),
                        ranked_value_text_v1(leader.precondition.0),
                        constant_index.get(),
                    ),
                )?;
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    Some(leader.precondition),
                )
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                element,
                index_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    block_index,
                )?;
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if projected.mapping != expected || *index_space != expected {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "blocked accessor mapping identity changed",
                    ));
                }
                if *lanes_per_block != 1 {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a blocked mapping with more than one lane before quotient facts are available",
                    ));
                }
                let component = call
                    .arguments()
                    .get(2)
                    .and_then(|operand| constant_operand_value(operand, constants))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic blocked component before ranked-value projection is available",
                    ))?;
                if component >= *elements_per_lane {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a blocked component is outside the authenticated elements-per-lane bound",
                    ));
                }
                reserve_operation(operations)?;
                let elements = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: elements,
                    value: *elements_per_lane,
                });
                reserve_operation(operations)?;
                let block_base = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: block_base,
                    kind: IndexBinaryKindAttr::Multiply,
                    lhs: projected.value,
                    rhs: ProductionRankedValueV1::Local(elements),
                });
                reserve_operation(operations)?;
                let component_value = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: component_value,
                    value: component,
                });
                reserve_operation(operations)?;
                let index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: index,
                    kind: IndexBinaryKindAttr::Add,
                    lhs: ProductionRankedValueV1::Local(block_base),
                    rhs: ProductionRankedValueV1::Local(component_value),
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Multiply {}, %{}\n  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add %{}, %{}\n",
                        elements.get(),
                        elements_per_lane,
                        block_base.get(),
                        ranked_value_text_v1(projected.value),
                        elements.get(),
                        component_value.get(),
                        component,
                        index.get(),
                        block_base.get(),
                        component_value.get(),
                    ),
                )?;
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    projected.precondition,
                )
            }
            _ => continue,
        };

        let receiver = call
            .arguments()
            .first()
            .and_then(simple_operand_local)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint receiver without one exact local",
            ))?
            .index() as usize;
        let allocation_origin = allocation_origins.get(receiver).copied().flatten().ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint receiver without one authenticated kernel-argument origin",
            ),
        )?;
        admit_writable_origin(&mut admitted_writable_origin, allocation_origin)?;

        let origin_index = allocation_origin as usize;
        let element_width = type_width(types, element)?;
        let (view, extent_argument) = match views_by_origin
            .get(origin_index)
            .and_then(|view| view.as_ref())
        {
            Some(view) if view.element_width == element_width => (view.result, 0),
            Some(_) => {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "one allocation origin was projected with conflicting element widths",
                ));
            }
            None => {
                reserve_operation(operations)?;
                let view = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width,
                    writable: true,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                    memory_space: MemorySpaceAttr::Global,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.ranked_view <{}, true, [dynamic], Global>(%arg0)\n",
                        view.get(),
                        element_width,
                    ),
                )?;
                let slot = views_by_origin.get_mut(origin_index).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a kernel argument origin outside the semantic local table",
                    ),
                )?;
                *slot = Some(ProjectedViewV1 {
                    result: view,
                    element_width,
                    shape: vec![DYNAMIC_EXTENT],
                    memory_space: MemorySpaceAttr::Global,
                });
                (view, 0)
            }
        };
        let access = GuardedDisjointAccessV1 {
            view,
            allocation_origin,
            index,
            precondition,
            extent_argument,
            access: AccessKindAttr::Write,
            source: block.terminator().source(),
        };
        let destination = simple_call_destination(call)?.index() as usize;
        let predicate = option_predicates.get_mut(destination).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a checked disjoint destination outside the semantic local table",
            ),
        )?;
        if predicate.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple checked predicates for one semantic local",
            ));
        }
        *predicate = Some(GuardPredicateV1::for_access(&access));
        guarded_accesses.push(access);
    }

    let checked_reference_origins =
        checked_reference_origins(function, callables, guarded_accesses.len())?;
    Ok(IntrinsicProjectionV1 {
        checked_reference_origins,
        extent_argument_count: usize::from(!guarded_accesses.is_empty()),
        guarded_accesses,
        option_predicates,
    })
}

fn push_capability_edge(
    edges_by_source: &mut [Vec<CapabilityEdgeV1>],
    edge_count: &mut usize,
    source: usize,
    edge: CapabilityEdgeV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if source >= edges_by_source.len() || edge.destination >= edges_by_source.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a capability def-use edge outside the semantic local table",
        ));
    }
    if *edge_count == MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability def-use edges exceed the charged projection limit",
        ));
    }
    edges_by_source[source].try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "capability def-use edge storage cannot be reserved",
        )
    })?;
    edges_by_source[source].push(edge);
    *edge_count += 1;
    Ok(())
}

fn assign_index_capability(
    destination: usize,
    projected: ProjectedDisjointIndexV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    worklist: &mut VecDeque<usize>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if destination >= index_values.len() || grid_leaders[destination].is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index capability escaped the semantic local table or changed capability kind",
        ));
    }
    match index_values[destination] {
        None => {
            index_values[destination] = Some(projected);
            worklist.push_back(destination);
        }
        Some(existing) if existing == projected => {}
        Some(_) => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local",
            ));
        }
    }
    Ok(())
}

fn required_launch_extent_x(function: &SemanticFunctionDeclV1) -> u64 {
    function
        .kernel_entry()
        .and_then(|entry| entry.source_contract().launch())
        .and_then(|launch| launch.required())
        .map(|dimensions| u64::from(dimensions.as_array()[0]))
        .unwrap_or(0)
}

fn admit_writable_origin(
    admitted: &mut Option<u32>,
    candidate: u32,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match *admitted {
        None => *admitted = Some(candidate),
        Some(existing) if existing == candidate => {}
        Some(_) => {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "multiple writable global allocation origins require an authenticated runtime no-alias obligation",
            ));
        }
    }
    Ok(())
}

fn local_allocation_origins(
    function: &SemanticFunctionDeclV1,
) -> Result<Vec<Option<u32>>, ProductionRankedProjectionErrorV1> {
    let definitions = local_definition_counts(function);
    let mut origins = vec![None; function.locals().len()];
    let mut aliases_by_source = vec![Vec::new(); function.locals().len()];
    for (local_index, local) in function.locals().iter().enumerate() {
        if let SemanticLocalRoleV1::Argument(argument) = local.role() {
            origins[local_index] = Some(argument);
        }
    }
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty()
                || definitions
                    .get(assignment.destination().local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
                    SemanticOperandV1::Constant(_) => None,
                },
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } => Some(place),
                SemanticRvalueKindV1::Load(load) if load.atomic().is_none() => Some(load.source()),
                _ => None,
            };
            let Some(source) = source else {
                continue;
            };
            let source = source.local().index() as usize;
            let destination = assignment.destination().local().index() as usize;
            if source >= aliases_by_source.len() || destination >= aliases_by_source.len() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an allocation-origin edge outside the semantic local table",
                ));
            }
            aliases_by_source[source].try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "allocation-origin edge storage cannot be reserved",
                )
            })?;
            aliases_by_source[source].push(destination);
        }
    }

    let mut worklist = origins
        .iter()
        .enumerate()
        .filter_map(|(local, origin)| origin.map(|_| local))
        .collect::<VecDeque<_>>();
    while let Some(source) = worklist.pop_front() {
        let Some(origin) = origins[source] else {
            continue;
        };
        for &destination in &aliases_by_source[source] {
            match origins[destination] {
                None => {
                    origins[destination] = Some(origin);
                    worklist.push_back(destination);
                }
                Some(existing) if existing == origin => {}
                Some(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a local may alias multiple kernel allocation origins",
                    ));
                }
            }
        }
    }
    Ok(origins)
}
fn projected_disjoint_operand_v1(
    call: &SemanticDirectCallV1,
    argument: usize,
    values: &[Option<ProjectedDisjointIndexV1>],
    option_dominance: &SemanticOptionDominanceV1,
    use_block: usize,
) -> Result<ProjectedDisjointIndexV1, ProductionRankedProjectionErrorV1> {
    let local = call
        .arguments()
        .get(argument)
        .and_then(simple_operand_local)
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access whose index witness is not one exact local",
        ))?;
    let projected = values
        .get(local.index() as usize)
        .copied()
        .flatten()
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access not bound to authenticated index authority",
        ))?;
    if !projected.availability.is_none_or(|availability| {
        option_dominance.allows(
            availability,
            SemanticBlockIdV1::from_index(use_block as u32),
        )
    }) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index capability is used outside its authenticated Some edge",
        ));
    }
    Ok(projected)
}

fn ranked_value_text_v1(value: ProductionRankedValueV1) -> String {
    match value {
        ProductionRankedValueV1::Local(identity) => format!("%{}", identity.get()),
        ProductionRankedValueV1::Argument(argument) => format!("%arg{argument}"),
    }
}

fn switch_predicates(
    function: &SemanticFunctionDeclV1,
    option_predicates: &[Option<GuardPredicateV1>],
) -> Result<Vec<Option<GuardPredicateV1>>, ProductionRankedProjectionErrorV1> {
    let mut predicates = vec![None; function.locals().len()];
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty() {
                continue;
            }
            let SemanticRvalueKindV1::Discriminant(source) = assignment.value().kind() else {
                continue;
            };
            if !source.projections().is_empty() {
                continue;
            }
            let Some(predicate) = option_predicates
                .get(source.local().index() as usize)
                .and_then(Clone::clone)
            else {
                continue;
            };
            let slot = predicates
                .get_mut(destination.local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "an Option discriminant outside the semantic local table",
                ))?;
            if slot.is_some() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an Option discriminant local with multiple definitions",
                ));
            }
            *slot = Some(predicate);
        }
    }
    Ok(predicates)
}

fn operation_defines_value(operation: &ProductionRankedOperationV1) -> bool {
    matches!(
        operation,
        ProductionRankedOperationV1::View { .. }
            | ProductionRankedOperationV1::ViewInSpace { .. }
            | ProductionRankedOperationV1::IndexConstant { .. }
            | ProductionRankedOperationV1::InvocationIndex { .. }
            | ProductionRankedOperationV1::IndexBinary { .. }
            | ProductionRankedOperationV1::Dimension { .. }
            | ProductionRankedOperationV1::SemanticSymbol { .. }
            | ProductionRankedOperationV1::SemanticConstant { .. }
            | ProductionRankedOperationV1::SemanticBinary { .. }
    )
}

fn order_projected_block_effects(
    operations: Vec<ProductionRankedOperationV1>,
    guarded_sites: Vec<GuardedAccessSiteV1>,
    sources: Vec<ProjectedAccessSourceV1>,
    entry_operations: &mut Vec<ProductionRankedOperationV1>,
) -> Result<ProjectedSemanticBlockV1, ProductionRankedProjectionErrorV1> {
    let mut source_at = vec![None; operations.len()];
    for source in sources {
        let slot = source_at.get_mut(source.operation).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a projected access source outside its semantic block",
            ),
        )?;
        if slot.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple projected sources for one ranked effect",
            ));
        }
        *slot = Some(ProjectedEffectSourceV1 {
            access: source.access,
            memory_space: source.memory_space,
            source: source.source,
        });
    }
    let mut sites = guarded_sites.into_iter().peekable();
    let mut items = Vec::new();
    for (index, operation) in operations.into_iter().enumerate() {
        while sites
            .peek()
            .is_some_and(|site| site.insertion_operation == index)
        {
            items.push(ProjectedBlockItemV1::Guarded(
                sites.next().expect("peeked guarded access site").access,
            ));
        }
        if sites
            .peek()
            .is_some_and(|site| site.insertion_operation < index)
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "checked access sites are not in semantic statement order",
            ));
        }
        let source = source_at[index];
        if operation_defines_value(&operation) {
            if source.is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a pure ranked definition carried a memory-effect source",
                ));
            }
            reserve_operation(entry_operations)?;
            entry_operations.push(operation);
        } else {
            items.push(ProjectedBlockItemV1::Effect { operation, source });
        }
    }
    for site in sites {
        if site.insertion_operation != source_at.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked access site is outside its semantic block",
            ));
        }
        items.push(ProjectedBlockItemV1::Guarded(site.access));
    }
    Ok(ProjectedSemanticBlockV1 { items })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedCfgTerminatorV1 {
    Branch(usize),
    Predicate {
        predicate: GuardPredicateV1,
        true_block: usize,
        false_block: usize,
    },
    Return,
}

fn projected_cfg_terminator(
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    switch_predicates: &[Option<GuardPredicateV1>],
) -> Result<ProjectedCfgTerminatorV1, ProductionRankedProjectionErrorV1> {
    let block = function.blocks().get(block_index).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("a semantic CFG block outside the function"),
    )?;
    let target = |target: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1| {
        let target = target.index() as usize;
        (target < function.blocks().len()).then_some(target).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic CFG edge outside the function",
            ),
        )
    };
    match block.terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge) => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            let discriminant = simple_operand_local(discriminant).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "a barrier-relevant switch whose discriminant is not one exact local",
                ),
            )?;
            let predicate = switch_predicates
                .get(discriminant.index() as usize)
                .and_then(Clone::clone)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a barrier-relevant switch without an authenticated checked-access predicate",
                ))?;
            if targets.values().len() != 2
                || targets.values()[0].value() != 0
                || targets.values()[1].value() != 1
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked Option switch whose exact 0/1 variants were not retained",
                ));
            }
            let otherwise = target(targets.otherwise().target())?;
            let otherwise_block = &function.blocks()[otherwise];
            if !otherwise_block.statements().is_empty()
                || !matches!(
                    otherwise_block.terminator().kind(),
                    SemanticTerminatorKindV1::Unreachable
                )
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked Option switch with a reachable non-variant successor",
                ));
            }
            Ok(ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block: target(targets.values()[1].edge().target())?,
                false_block: target(targets.values()[0].edge().target())?,
            })
        }
        SemanticTerminatorKindV1::Call(call) => {
            if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a call with cleanup control flow before exact unwind projection",
                ));
            }
            match call.destination() {
                Some(destination) => Ok(ProjectedCfgTerminatorV1::Branch(target(
                    destination.edge().target(),
                )?)),
                None => Ok(ProjectedCfgTerminatorV1::Return),
            }
        }
        SemanticTerminatorKindV1::Assert { target: edge, .. }
        | SemanticTerminatorKindV1::Drop { target: edge, .. } => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a false edge before exact semantic CFG normalization",
            ))
        }
        SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(ProjectedCfgTerminatorV1::Return),
    }
}

fn projected_block_expansion(
    block: &ProjectedSemanticBlockV1,
    terminator: &ProjectedCfgTerminatorV1,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    let mut count = 1_usize;
    for item in &block.items {
        if let ProjectedBlockItemV1::Guarded(access) = item {
            count = count
                .checked_add(GuardPredicateV1::for_access(access).comparisons.len() + 1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "checked-access CFG block count overflow",
                ))?;
        }
    }
    if let ProjectedCfgTerminatorV1::Predicate { predicate, .. } = terminator {
        count = count
            .checked_add(predicate.comparisons.len().saturating_sub(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG predicate block count overflow",
            ))?;
    }
    Ok(count)
}

fn build_ranked_cfg(
    function: &SemanticFunctionDeclV1,
    switch_predicates: &[Option<GuardPredicateV1>],
    entry_operations: Vec<ProductionRankedOperationV1>,
    projected_blocks: Vec<ProjectedSemanticBlockV1>,
) -> Result<
    (Vec<ProductionRankedBlockV1>, Vec<ProjectedAccessSourceV1>),
    ProductionRankedProjectionErrorV1,
> {
    if projected_blocks.len() != function.blocks().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection lost a basic block",
        ));
    }
    let terminators = (0..function.blocks().len())
        .map(|index| projected_cfg_terminator(function, index, switch_predicates))
        .collect::<Result<Vec<_>, _>>()?;
    let entry = function.entry().index() as usize;
    let reachable = reachable_projected_blocks(entry, &terminators)?;
    let mut base_blocks = vec![None; projected_blocks.len()];
    let mut block_count = 1_usize;
    for (index, (block, terminator)) in projected_blocks.iter().zip(&terminators).enumerate() {
        if !reachable[index] {
            continue;
        }
        base_blocks[index] = Some(block_count);
        block_count = block_count
            .checked_add(projected_block_expansion(block, terminator)?)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG block count overflow",
            ))?;
    }
    if block_count > fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection exceeds the ranked block limit",
        ));
    }
    let operation_count = projected_blocks
        .iter()
        .try_fold(entry_operations.len(), |count, block| {
            count.checked_add(block.items.len())
        })
        .and_then(|count| count.checked_add(block_count));
    if operation_count.is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection exceeds the ranked operation limit",
        ));
    }
    let entry_target = base_blocks.get(entry).copied().flatten().ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("semantic entry block outside the function"),
    )?;
    let mut blocks = Vec::with_capacity(block_count);
    blocks.push(ProductionRankedBlockV1::new(
        entry_operations,
        ProductionRankedTerminatorV1::Branch {
            target: ranked_block_id(entry_target)?,
        },
    ));
    let mut sources = Vec::new();
    for (semantic_index, (projected, terminator)) in
        projected_blocks.into_iter().zip(terminators).enumerate()
    {
        let Some(mut current) = base_blocks[semantic_index] else {
            continue;
        };
        let mut operations = Vec::new();
        for item in projected.items {
            match item {
                ProjectedBlockItemV1::Effect { operation, source } => {
                    if let Some(source) = source {
                        sources.push(ProjectedAccessSourceV1 {
                            block: current,
                            operation: operations.len(),
                            access: source.access,
                            memory_space: source.memory_space,
                            source: source.source,
                        });
                    }
                    operations.push(operation);
                }
                ProjectedBlockItemV1::Guarded(access) => {
                    let predicate = GuardPredicateV1::for_access(&access);
                    let continuation = current + predicate.comparisons.len() + 1;
                    append_predicate_blocks(
                        &mut blocks,
                        current,
                        operations,
                        &predicate,
                        current + predicate.comparisons.len(),
                        continuation,
                    )?;
                    let access_block = current + predicate.comparisons.len();
                    push_block_at(
                        &mut blocks,
                        access_block,
                        vec![ProductionRankedOperationV1::Access {
                            kind: access.access,
                            view: ProductionRankedValueV1::Local(access.view),
                            indices: vec![access.index],
                        }],
                        ProductionRankedTerminatorV1::Branch {
                            target: ranked_block_id(continuation)?,
                        },
                    )?;
                    sources.push(ProjectedAccessSourceV1 {
                        block: access_block,
                        operation: 0,
                        access: access.access,
                        memory_space: MemorySpaceAttr::Global,
                        source: access.source,
                    });
                    current = continuation;
                    operations = Vec::new();
                }
            }
        }
        match terminator {
            ProjectedCfgTerminatorV1::Branch(target) => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Branch {
                    target: ranked_block_id(projected_target(&base_blocks, target)?)?,
                },
            )?,
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block: _,
            } if predicate.comparisons.is_empty() => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Branch {
                    target: ranked_block_id(projected_target(&base_blocks, true_block)?)?,
                },
            )?,
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block,
            } => append_predicate_blocks(
                &mut blocks,
                current,
                operations,
                &predicate,
                projected_target(&base_blocks, true_block)?,
                projected_target(&base_blocks, false_block)?,
            )?,
            ProjectedCfgTerminatorV1::Return => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Return,
            )?,
        }
    }
    if blocks.len() != block_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection produced a non-canonical block inventory",
        ));
    }
    Ok((blocks, sources))
}

fn reachable_projected_blocks(
    entry: usize,
    terminators: &[ProjectedCfgTerminatorV1],
) -> Result<Vec<bool>, ProductionRankedProjectionErrorV1> {
    if entry >= terminators.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic entry block outside the function",
        ));
    }
    let mut reachable = vec![false; terminators.len()];
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        let slot =
            reachable
                .get_mut(block)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a semantic CFG edge outside the function",
                ))?;
        if *slot {
            continue;
        }
        *slot = true;
        match &terminators[block] {
            ProjectedCfgTerminatorV1::Branch(target) => pending.push(*target),
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block,
            } => {
                pending.push(*true_block);
                if !predicate.comparisons.is_empty() {
                    pending.push(*false_block);
                }
            }
            ProjectedCfgTerminatorV1::Return => {}
        }
    }
    Ok(reachable)
}

fn projected_target(
    base_blocks: &[Option<usize>],
    semantic_block: usize,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    base_blocks.get(semantic_block).copied().flatten().ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a projected CFG edge targets a pruned semantic block",
        ),
    )
}

fn append_predicate_blocks(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    predicate: &GuardPredicateV1,
    true_block: usize,
    false_block: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if predicate.comparisons.is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an empty predicate was materialized as conditional control flow",
        ));
    }
    for (index, &(lhs, rhs)) in predicate.comparisons.iter().enumerate() {
        let block = first_block + index;
        let operations = if index == 0 {
            first_operations.clone()
        } else {
            Vec::new()
        };
        let next = if index + 1 == predicate.comparisons.len() {
            true_block
        } else {
            block + 1
        };
        push_block_at(
            blocks,
            block,
            operations,
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs,
                rhs,
                true_block: ranked_block_id(next)?,
                false_block: ranked_block_id(false_block)?,
            },
        )?;
    }
    Ok(())
}

fn push_block_at(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    expected: usize,
    operations: Vec<ProductionRankedOperationV1>,
    terminator: ProductionRankedTerminatorV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if blocks.len() != expected {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection produced non-canonical block numbering",
        ));
    }
    blocks.push(ProductionRankedBlockV1::new(operations, terminator));
    Ok(())
}

fn ranked_block_id(block: usize) -> Result<u32, ProductionRankedProjectionErrorV1> {
    u32::try_from(block).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG block identity does not fit u32",
        )
    })
}

fn format_ranked_cfg(
    function_name: &str,
    blocks: &[ProductionRankedBlockV1],
) -> Result<String, ProductionRankedProjectionErrorV1> {
    let mut output = String::new();
    push_ranked_ir(&mut output, &format!("func @{function_name} {{\n"))?;
    for (block_index, block) in blocks.iter().enumerate() {
        push_ranked_ir(&mut output, &format!("^bb{block_index}:\n"))?;
        for operation in block.operations() {
            push_ranked_ir(&mut output, &format_ranked_operation(operation))?;
        }
        let terminator = match block.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            } => format!(
                "  kernel.cond_br {} < {} ^bb{}, ^bb{}\n",
                ranked_value_text_v1(*lhs),
                ranked_value_text_v1(*rhs),
                true_block,
                false_block,
            ),
            ProductionRankedTerminatorV1::Branch { target } => {
                format!("  kernel.br ^bb{target}\n")
            }
            ProductionRankedTerminatorV1::Return => "  kernel.return\n".to_owned(),
        };
        push_ranked_ir(&mut output, &terminator)?;
    }
    push_ranked_ir(&mut output, "}\n")?;
    Ok(output)
}

fn format_ranked_operation(operation: &ProductionRankedOperationV1) -> String {
    match operation {
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
        } => format!(
            "  %{} = kernel.ranked_view <{}, {}, {:?}>({})\n",
            result.get(),
            element_width,
            writable,
            shape,
            format_ranked_values(dynamic_extents),
        ),
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
        } => format!(
            "  %{} = kernel.ranked_view <{}, {}, {:?}, {:?}>({})\n",
            result.get(),
            element_width,
            writable,
            shape,
            memory_space,
            format_ranked_values(dynamic_extents),
        ),
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            format!("  %{} = kernel.index_constant {}\n", result.get(), value)
        }
        ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => format!(
            "  %{} = kernel.invocation_index <{}, {}>\n",
            result.get(),
            dimension,
            if *launch_extent == 0 {
                "dynamic".to_owned()
            } else {
                launch_extent.to_string()
            },
        ),
        ProductionRankedOperationV1::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        } => format!(
            "  %{} = kernel.index_binary {:?} {}, {}\n",
            result.get(),
            kind,
            ranked_value_text_v1(*lhs),
            ranked_value_text_v1(*rhs),
        ),
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => format!(
            "  %{} = kernel.dimension {} {}\n",
            result.get(),
            ranked_value_text_v1(*view),
            dimension,
        ),
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => format!(
            "  kernel.access {:?} {}[{}]\n",
            kind,
            ranked_value_text_v1(*view),
            format_ranked_values(indices),
        ),
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
        } => format!(
            "  kernel.atomic_access {:?} <{:?}, {:?}> {}[{}]\n",
            kind,
            ordering,
            scope,
            ranked_value_text_v1(*view),
            format_ranked_values(indices),
        ),
        ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => format!(
            "  gpu.barrier <{:?}, {:?}, {:?}, {:?}>\n",
            execution_scope, memory_scope, address_space, order,
        ),
        ProductionRankedOperationV1::SemanticSymbol { result, symbol } => {
            format!("  %{} = kernel.semantic_symbol {}\n", result.get(), symbol)
        }
        ProductionRankedOperationV1::SemanticConstant { result, value } => {
            format!("  %{} = kernel.semantic_constant {}\n", result.get(), value)
        }
        ProductionRankedOperationV1::SemanticBinary {
            result,
            kind,
            lhs,
            rhs,
        } => format!(
            "  %{} = kernel.semantic_binary {:?} {}, {}\n",
            result.get(),
            kind,
            ranked_value_text_v1(*lhs),
            ranked_value_text_v1(*rhs),
        ),
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => format!(
            "  kernel.require_equivalent {}, {}\n",
            ranked_value_text_v1(*actual),
            ranked_value_text_v1(*expected),
        ),
    }
}

fn format_ranked_values(values: &[ProductionRankedValueV1]) -> String {
    values
        .iter()
        .map(|value| ranked_value_text_v1(*value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn checked_reference_origins(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded_access_count: usize,
) -> Result<Vec<Option<usize>>, ProductionRankedProjectionErrorV1> {
    let definitions = local_definition_counts(function);
    let mut origins = vec![None; function.locals().len()];
    let mut aliases_by_source = vec![Vec::new(); function.locals().len()];
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty()
                || definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => transparent_operand_place(operand),
                SemanticRvalueKindV1::Load(load) if load.atomic().is_none() => {
                    transparent_place(load.source())
                }
                _ => None,
            };
            let Some(source) = source else {
                continue;
            };
            let edges = aliases_by_source
                .get_mut(source.local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked reference alias outside the semantic local table",
                ))?;
            edges.push(destination.local().index() as usize);
        }
    }

    let mut worklist = VecDeque::new();
    let mut access = 0_usize;
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        if !matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. },
                ..
            })
        ) {
            continue;
        }
        let destination = simple_call_destination(call)?;
        if definitions.get(destination.index() as usize).copied() != Some(1) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint result without one exact definition",
            ));
        }
        origins[destination.index() as usize] = Some(access);
        worklist.push_back(destination.index() as usize);
        access += 1;
    }
    if access != guarded_access_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "checked disjoint access inventory changed during projection",
        ));
    }
    while let Some(source) = worklist.pop_front() {
        let Some(origin) = origins[source] else {
            continue;
        };
        for &destination in &aliases_by_source[source] {
            let slot = &mut origins[destination];
            if slot.is_none() {
                *slot = Some(origin);
                worklist.push_back(destination);
            } else if *slot != Some(origin) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked disjoint reference with conflicting origins",
                ));
            }
        }
    }
    Ok(origins)
}

fn local_definition_counts(function: &SemanticFunctionDeclV1) -> Vec<u8> {
    let mut definitions = vec![0_u8; function.locals().len()];
    let mut record = |place: &SemanticPlaceV1| {
        if matches!(
            place
                .projections()
                .first()
                .map(|projection| projection.kind()),
            Some(SemanticProjectionKindV1::Dereference)
        ) {
            return;
        }
        if let Some(slot) = definitions.get_mut(place.local().index() as usize) {
            *slot = slot.saturating_add(1);
        }
    };
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => record(assignment.destination()),
                SemanticStatementKindV1::Store(store) => record(store.destination()),
                SemanticStatementKindV1::AtomicRmw(atomic) => record(atomic.destination()),
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record(atomic.destination())
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record(place),
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            record(destination.place());
        }
    }
    definitions
}

fn checked_reference_origin(place: &SemanticPlaceV1, origins: &[Option<usize>]) -> Option<usize> {
    let origin = origins
        .get(place.local().index() as usize)
        .copied()
        .flatten()?;
    let mut projections = place.projections().iter();
    if !matches!(
        projections.next().map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ) || !projections.all(|projection| {
        matches!(
            projection.kind(),
            SemanticProjectionKindV1::Field(_)
                | SemanticProjectionKindV1::Downcast(_)
                | SemanticProjectionKindV1::OpaqueCast
                | SemanticProjectionKindV1::Subtype
        )
    }) {
        return None;
    }
    Some(origin)
}

fn transparent_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    let place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        SemanticOperandV1::Constant(_) => return None,
    };
    transparent_place(place)
}

fn transparent_place(place: &SemanticPlaceV1) -> Option<&SemanticPlaceV1> {
    place
        .projections()
        .iter()
        .all(|projection| {
            matches!(
                projection.kind(),
                SemanticProjectionKindV1::Field(_)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype
            )
        })
        .then_some(place)
}

fn simple_operand_local(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    transparent_operand_place(operand)
        .filter(|place| place.projections().is_empty())
        .map(SemanticPlaceV1::local)
}

fn simple_call_destination(
    call: &SemanticDirectCallV1,
) -> Result<SemanticLocalIdV1, ProductionRankedProjectionErrorV1> {
    call.destination()
        .map(|destination| destination.place())
        .filter(|place| place.projections().is_empty())
        .map(SemanticPlaceV1::local)
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a compiler intrinsic without one exact local destination",
        ))
}

fn reserve_operation(
    operations: &mut Vec<ProductionRankedOperationV1>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if operations.len() == MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic intrinsic projection exceeding the ranked operation limit",
        ));
    }
    operations.try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "semantic intrinsic projection storage cannot be reserved",
        )
    })
}

fn retain_incomplete(
    result: Result<(), ProductionRankedProjectionErrorV1>,
    incomplete: &mut Option<&'static str>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match result {
        Err(ProductionRankedProjectionErrorV1::Incomplete(detail)) => {
            incomplete.get_or_insert(detail);
            Ok(())
        }
        result => result,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_statement_accesses(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let source = statement.source();
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => {
            project_place_access(
                types,
                function,
                assignment.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_rvalue_reads(
                types,
                function,
                assignment.value().kind(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::Store(store) => {
            project_place_access_with_atomic(
                types,
                function,
                store.destination(),
                if store.atomic().is_some() {
                    AccessKindAttr::AtomicWrite
                } else {
                    AccessKindAttr::Write
                },
                store.atomic(),
                PlaceAccessRequirementV1::ExplicitMemory,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                store.value(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            project_place_access(
                types,
                function,
                atomic.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_atomic_address(
                types,
                function,
                atomic.address(),
                atomic.access(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                atomic.value(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            project_place_access(
                types,
                function,
                atomic.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_atomic_address(
                types,
                function,
                atomic.address(),
                atomic.success(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                atomic.expected(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                atomic.replacement(),
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => project_place_access(
            types,
            function,
            place,
            AccessKindAttr::Write,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            if function.locals().get(local.index() as usize).is_none() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a storage statement with an out-of-range local",
                ));
            }
            // Storage lifetime markers do not read or write the local's value.
            Ok(())
        }
        SemanticStatementKindV1::Nop => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_atomic_address(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    address: &SemanticPlaceV1,
    atomic: SemanticAtomicAccessV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_place_access_with_atomic(
        types,
        function,
        address,
        AccessKindAttr::AtomicReadModifyWrite,
        Some(atomic),
        PlaceAccessRequirementV1::ExplicitMemory,
        source,
        constants,
        checked_reference_origins,
        guarded_accesses,
        guarded_sites,
        projected_views,
        operations,
        sources,
        next_value,
        ranked_ir,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_terminator_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    terminator: &SemanticTerminatorKindV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => project_operand_read(
            types,
            function,
            discriminant,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Call(call) => project_direct_call_accesses(
            callables,
            types,
            function,
            call,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::TailCall(call) => project_tail_call_accesses(
            callables,
            types,
            function,
            call,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Drop { .. } => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a drop terminator before exact drop-glue memory-effect summaries are available",
            ))
        }
        SemanticTerminatorKindV1::Assert { condition, .. } => project_operand_read(
            types,
            function,
            condition,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_direct_call_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if matches!(
        callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
            ..
        })
    ) {
        reserve_operation(operations)?;
        operations.push(ProductionRankedOperationV1::Barrier {
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
        });
        push_ranked_ir(
            ranked_ir,
            "  gpu.barrier <workgroup, workgroup, workgroup, acquire_release>\n",
        )?;
        return Ok(());
    }
    if matches!(
        callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. },
            ..
        })
    ) {
        return Ok(());
    }
    require_bounds_neutral_callable(callables, call.callee())?;
    for argument in call.arguments() {
        project_operand_read(
            types,
            function,
            argument,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    if let Some(destination) = call.destination() {
        project_place_access(
            types,
            function,
            destination.place(),
            AccessKindAttr::Write,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn project_tail_call_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectTailCallV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    require_bounds_neutral_callable(callables, call.callee())?;
    for argument in call.arguments() {
        project_operand_read(
            types,
            function,
            argument,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    Ok(())
}

fn require_bounds_neutral_callable(
    callables: &[SemanticCallableDeclV1],
    callable: SemanticCallableIdV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match callables.get(callable.index() as usize) {
        Some(SemanticCallableDeclV1::CompilerIntrinsic { .. }) => Ok(()),
        Some(
            SemanticCallableDeclV1::Defined { .. } | SemanticCallableDeclV1::DeviceFfiImport { .. },
        )
        | None => Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a call terminator before exact callable memory-effect summaries are available",
        )),
    }
}

#[derive(Clone, Copy)]
enum ConstantDefinitionV1 {
    Missing,
    Direct(u64),
    Alias(SemanticLocalIdV1),
    Invalid,
}

fn constant_locals(function: &SemanticFunctionDeclV1) -> Vec<Option<u64>> {
    let mut definitions = vec![ConstantDefinitionV1::Missing; function.locals().len()];
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        assignment.destination().local(),
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand) => constant_definition(operand),
                            _ => ConstantDefinitionV1::Invalid,
                        },
                    );
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place)
                    if place.projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        place.local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::Store(store)
                    if store.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        store.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::AtomicRmw(atomic)
                    if atomic.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        atomic.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic)
                    if atomic.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        atomic.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::Assign(_)
                | SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_)
                | SemanticStatementKindV1::SetDiscriminant { .. }
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && destination.place().projections().is_empty()
        {
            record_constant_definition(
                &mut definitions,
                destination.place().local(),
                ConstantDefinitionV1::Invalid,
            );
        }
    }
    let mut states = vec![0_u8; definitions.len()];
    let mut values = vec![None; definitions.len()];
    for index in 0..definitions.len() {
        resolve_constant(index, &definitions, &mut states, &mut values);
    }
    values
}

fn constant_definition(operand: &SemanticOperandV1) -> ConstantDefinitionV1 {
    match operand {
        SemanticOperandV1::Constant(constant) => match constant.value() {
            SemanticConstantValueV1::Scalar(value) => u64::try_from(value.bits())
                .map(ConstantDefinitionV1::Direct)
                .unwrap_or(ConstantDefinitionV1::Invalid),
            _ => ConstantDefinitionV1::Invalid,
        },
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            ConstantDefinitionV1::Alias(place.local())
        }
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => ConstantDefinitionV1::Invalid,
    }
}

fn constant_operand_value(operand: &SemanticOperandV1, constants: &[Option<u64>]) -> Option<u64> {
    match constant_definition(operand) {
        ConstantDefinitionV1::Direct(value) => Some(value),
        ConstantDefinitionV1::Alias(local) => {
            constants.get(local.index() as usize).copied().flatten()
        }
        ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => None,
    }
}

fn record_constant_definition(
    definitions: &mut [ConstantDefinitionV1],
    local: SemanticLocalIdV1,
    definition: ConstantDefinitionV1,
) {
    if let Some(slot) = definitions.get_mut(local.index() as usize) {
        *slot = if matches!(slot, ConstantDefinitionV1::Missing) {
            definition
        } else {
            ConstantDefinitionV1::Invalid
        };
    }
}

fn resolve_constant(
    index: usize,
    definitions: &[ConstantDefinitionV1],
    states: &mut [u8],
    values: &mut [Option<u64>],
) -> Option<u64> {
    match states.get(index).copied() {
        Some(2) => return values[index],
        Some(1) | None => return None,
        Some(_) => {}
    }
    states[index] = 1;
    let value = match definitions[index] {
        ConstantDefinitionV1::Direct(value) => Some(value),
        ConstantDefinitionV1::Alias(local) => {
            resolve_constant(local.index() as usize, definitions, states, values)
        }
        ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => None,
    };
    states[index] = 2;
    values[index] = value;
    value
}

#[allow(clippy::too_many_arguments)]
fn project_rvalue_reads(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    value: &SemanticRvalueKindV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match value {
        SemanticRvalueKindV1::Use(operand) => project_operand_read(
            types,
            function,
            operand,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => project_operand_read(
            types,
            function,
            operand,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            project_operand_read(
                types,
                function,
                left,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                right,
                source,
                constants,
                checked_reference_origins,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                project_operand_read(
                    types,
                    function,
                    operand,
                    source,
                    constants,
                    checked_reference_origins,
                    guarded_accesses,
                    guarded_sites,
                    projected_views,
                    operations,
                    sources,
                    next_value,
                    ranked_ir,
                )?;
            }
            Ok(())
        }
        SemanticRvalueKindV1::Load(load) => project_place_access_with_atomic(
            types,
            function,
            load.source(),
            if load.atomic().is_some() {
                AccessKindAttr::AtomicRead
            } else {
                AccessKindAttr::Read
            },
            load.atomic(),
            PlaceAccessRequirementV1::ExplicitMemory,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => project_place_access(
            types,
            function,
            place,
            AccessKindAttr::Read,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_operand_read(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => project_place_access(
            types,
            function,
            place,
            AccessKindAttr::Read,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            checked_reference_origins,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaceAccessRequirementV1 {
    IfMemory,
    ExplicitMemory,
}

#[allow(clippy::too_many_arguments)]
fn project_place_access(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    requirement: PlaceAccessRequirementV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_place_access_with_atomic(
        types,
        function,
        place,
        access,
        None,
        requirement,
        source,
        constants,
        checked_reference_origins,
        guarded_accesses,
        guarded_sites,
        projected_views,
        operations,
        sources,
        next_value,
        ranked_ir,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_place_access_with_atomic(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    atomic: Option<SemanticAtomicAccessV1>,
    requirement: PlaceAccessRequirementV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    guarded_accesses: &[GuardedDisjointAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if access.is_atomic() != atomic.is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an atomic access whose ordering/scope contract is missing or attached to a non-atomic access",
        ));
    }
    if let Some(origin) = checked_reference_origin(place, checked_reference_origins) {
        if atomic.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an atomic access through a checked disjoint reference before exact atomic capability projection",
            ));
        }
        let mut guarded =
            *guarded_accesses
                .get(origin)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked disjoint reference whose access origin is out of range",
                ))?;
        guarded.access = access;
        guarded.source = source;
        guarded_sites.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "checked disjoint access-site storage cannot be reserved",
            )
        })?;
        guarded_sites.push(GuardedAccessSiteV1 {
            insertion_operation: operations.len(),
            access: guarded,
        });
        return Ok(());
    }
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed place with an out-of-range local",
        ));
    };
    let mut current = local.ty();
    let mut shape = Vec::new();
    let mut indices = Vec::new();
    let mut crosses_memory_boundary = false;
    let mut dereferenced_memory_space = None;
    for projection in place.projections() {
        match projection.kind() {
            SemanticProjectionKindV1::Dereference => {
                crosses_memory_boundary = true;
                let Some(ty) = types.get(current.index() as usize) else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place with an out-of-range type",
                    ));
                };
                let SemanticTypeShapeV1::Pointer(pointer) = ty.shape() else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a dereference whose semantic type is not a pointer",
                    ));
                };
                dereferenced_memory_space = Some(memory_space(pointer.address_space())?);
                current = pointer.pointee();
            }
            SemanticProjectionKindV1::Index(index) => {
                crosses_memory_boundary = true;
                if shape.len() == MAX_RANKED_MEMORY_RANK {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place exceeding the ranked-memory rank limit",
                    ));
                }
                let extent = static_array_extent(types, current)?;
                let value = constants
                    .get(index.index() as usize)
                    .copied()
                    .flatten()
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic index before dynamic ranked-value projection is available",
                    ))?;
                shape.push(extent);
                indices.push(value);
                current = projection.result_type();
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset, from_end, ..
            } => {
                crosses_memory_boundary = true;
                if shape.len() == MAX_RANKED_MEMORY_RANK {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place exceeding the ranked-memory rank limit",
                    ));
                }
                let extent = static_array_extent(types, current)?;
                let value = if from_end {
                    extent.checked_sub(offset).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "a from-end constant index larger than its static extent",
                        ),
                    )?
                } else {
                    offset
                };
                shape.push(extent);
                indices.push(value);
                current = projection.result_type();
            }
            SemanticProjectionKindV1::Field(_)
            | SemanticProjectionKindV1::Downcast(_)
            | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype => current = projection.result_type(),
            SemanticProjectionKindV1::Subslice { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an indexed place containing a subslice projection",
                ));
            }
        }
    }
    if indices.is_empty() {
        if crosses_memory_boundary {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a dereferenced memory access without a ranked index projection",
            ));
        }
        if requirement == PlaceAccessRequirementV1::ExplicitMemory {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an explicit memory operation without a ranked index projection",
            ));
        }
        return Ok(());
    }
    reserve_projected_access(operations, sources, indices.len() + 2)?;
    let element_width = type_width(types, place.ty())?;
    let memory_space = if let Some(memory_space) = dereferenced_memory_space {
        memory_space
    } else if matches!(local.role(), SemanticLocalRoleV1::Argument(_)) {
        MemorySpaceAttr::Global
    } else {
        MemorySpaceAttr::Private
    };
    if memory_space == MemorySpaceAttr::Workgroup {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "workgroup memory before exact semantic CFG projection is available",
        ));
    }
    let view_slot = projected_views
        .get_mut(place.local().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed place outside the ranked view table",
        ))?;
    let view_id = if let Some(view) = view_slot {
        if view.element_width != element_width
            || view.shape != shape
            || view.memory_space != memory_space
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one semantic allocation used through inconsistent ranked views",
            ));
        }
        view.result
    } else {
        let view_id = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: view_id,
            element_width,
            writable: true,
            shape: shape.clone(),
            dynamic_extents: vec![],
            memory_space,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "  %{} = kernel.ranked_view <{}, true, {:?}, {:?}>\n",
                view_id.get(),
                element_width,
                shape,
                memory_space,
            ),
        )?;
        *view_slot = Some(ProjectedViewV1 {
            result: view_id,
            element_width,
            shape: shape.clone(),
            memory_space,
        });
        view_id
    };
    let mut ranked_indices = Vec::with_capacity(indices.len());
    for value in indices {
        let index_id = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::IndexConstant {
            result: index_id,
            value,
        });
        push_ranked_ir(
            ranked_ir,
            &format!("  %{} = kernel.index_constant {}\n", index_id.get(), value,),
        )?;
        ranked_indices.push(ProductionRankedValueV1::Local(index_id));
    }
    let operation = operations.len();
    if let Some(atomic) = atomic {
        operations.push(ProductionRankedOperationV1::AtomicAccess {
            kind: access,
            ordering: atomic_ordering_v1(atomic.ordering()),
            scope: atomic_scope_v1(atomic.scope()),
            view: ProductionRankedValueV1::Local(view_id),
            indices: ranked_indices.clone(),
        });
    } else {
        operations.push(ProductionRankedOperationV1::Access {
            kind: access,
            view: ProductionRankedValueV1::Local(view_id),
            indices: ranked_indices.clone(),
        });
    }
    push_ranked_ir(
        ranked_ir,
        &format!(
            "  kernel.{} {:?} %{}[{}]\n",
            if atomic.is_some() {
                "atomic_access"
            } else {
                "access"
            },
            access,
            view_id.get(),
            ranked_indices
                .iter()
                .map(|value| match value {
                    ProductionRankedValueV1::Local(identity) => format!("%{}", identity.get()),
                    ProductionRankedValueV1::Argument(argument) => format!("%arg{argument}"),
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
    )?;
    sources.push(ProjectedAccessSourceV1 {
        block: 0,
        operation,
        access,
        memory_space,
        source,
    });
    Ok(())
}

const fn atomic_ordering_v1(ordering: SemanticAtomicOrderingV1) -> AtomicOrderingAttr {
    match ordering {
        SemanticAtomicOrderingV1::Relaxed => AtomicOrderingAttr::Relaxed,
        SemanticAtomicOrderingV1::Release => AtomicOrderingAttr::Release,
        SemanticAtomicOrderingV1::Acquire => AtomicOrderingAttr::Acquire,
        SemanticAtomicOrderingV1::AcquireRelease => AtomicOrderingAttr::AcquireRelease,
        SemanticAtomicOrderingV1::SequentiallyConsistent => {
            AtomicOrderingAttr::SequentiallyConsistent
        }
    }
}

const fn atomic_scope_v1(scope: SemanticAtomicScopeV1) -> AtomicScopeAttr {
    match scope {
        SemanticAtomicScopeV1::SingleThread => AtomicScopeAttr::SingleThread,
        SemanticAtomicScopeV1::Workgroup => AtomicScopeAttr::Workgroup,
        SemanticAtomicScopeV1::Agent => AtomicScopeAttr::Agent,
        SemanticAtomicScopeV1::Device => AtomicScopeAttr::Device,
        SemanticAtomicScopeV1::System => AtomicScopeAttr::System,
    }
}

fn reserve_projected_access(
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    additional_operations: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let actual = operations.len().checked_add(additional_operations).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection with overflowing operation work",
        ),
    )?;
    if actual > MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection exceeding the ranked operation limit",
        ));
    }
    operations.try_reserve(additional_operations).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose operation storage cannot be reserved",
        )
    })?;
    sources.try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose source storage cannot be reserved",
        )
    })
}

fn push_ranked_ir(
    ranked_ir: &mut String,
    fragment: &str,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let actual = ranked_ir.len().checked_add(fragment.len()).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection with overflowing diagnostic text",
        ),
    )?;
    if actual > MAX_PROJECTED_RANKED_IR_BYTES_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection exceeding the diagnostic text limit",
        ));
    }
    ranked_ir.try_reserve(fragment.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose diagnostic storage cannot be reserved",
        )
    })?;
    ranked_ir.push_str(fragment);
    Ok(())
}

fn static_array_extent(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<u64, ProductionRankedProjectionErrorV1> {
    match types.get(ty.index() as usize).map(|ty| ty.shape()) {
        Some(SemanticTypeShapeV1::Array { length, .. }) => Ok(*length),
        Some(SemanticTypeShapeV1::Slice { .. }) => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a slice access before dynamic extent projection is available",
            ))
        }
        _ => Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index projection whose base is not an array or slice",
        )),
    }
}

fn memory_space(address_space: u32) -> Result<MemorySpaceAttr, ProductionRankedProjectionErrorV1> {
    match address_space {
        0 | 1 | 4 => Ok(MemorySpaceAttr::Global),
        3 => Ok(MemorySpaceAttr::Workgroup),
        5 => Ok(MemorySpaceAttr::Private),
        _ => Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a pointer address space outside the generic memory model",
        )),
    }
}

fn type_width(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<u32, ProductionRankedProjectionErrorV1> {
    let bytes = types
        .get(ty.index() as usize)
        .and_then(|ty| ty.layout().size_bytes())
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed element without a static layout size",
        ))?;
    let bits = u32::try_from(bytes.checked_mul(8).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("an overflowing element width"),
    )?)
    .map_err(|_| ProductionRankedProjectionErrorV1::Unsupported("an overflowing element width"))?;
    if !SUPPORTED_ELEMENT_WIDTHS.contains(&bits) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an element width outside the ranked-memory dialect",
        ));
    }
    Ok(bits)
}

fn next_value_id(
    next: &mut u32,
) -> Result<ProductionRankedValueIdV1, ProductionRankedProjectionErrorV1> {
    let value = *next;
    *next = next
        .checked_add(1)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "too many ranked SSA values",
        ))?;
    Ok(ProductionRankedValueIdV1::new(value))
}

fn function_name(
    function: &SemanticFunctionDeclV1,
) -> Result<&str, ProductionRankedProjectionErrorV1> {
    let symbol = function
        .kernel_entry()
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel root without a kernel export",
        ))?
        .export_symbol()
        .as_bytes();
    std::str::from_utf8(symbol).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported("a non-UTF-8 kernel export symbol")
    })
}

fn source_label(source: SemanticSourceProvenanceV1) -> String {
    let Some(origin) = source.call_site().or_else(|| source.expansion()) else {
        return "Rust source location unavailable".to_owned();
    };
    let (line, column) = origin.start_coordinate();
    let digest = origin.file();
    format!(
        "Rust source {}:{}:{}",
        &crate::encode_hex(digest.as_bytes())[..12],
        line,
        column,
    )
}

fn indent_ir(ir: &str) -> String {
    ir.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::SemanticOptionProducerV1;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

    fn bytes(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn projection_types() -> Vec<SemanticTypeDeclV1> {
        vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(1)),
                SemanticLayoutIdentityV1::from_sha256(bytes(1)),
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(2)),
                SemanticLayoutIdentityV1::from_sha256(bytes(2)),
                SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: SCALAR_TYPE,
                    length: 4,
                },
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(3)),
                SemanticLayoutIdentityV1::from_sha256(bytes(3)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new(
                        SCALAR_TYPE,
                        SemanticMutabilityV1::Mutable,
                        1,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ]
    }

    fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }

    fn block(
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            statements,
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap()
    }

    fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
    }
    fn projection_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            blocks,
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn projection_function_with_locals(
        blocks: Vec<SemanticBasicBlockV1>,
        locals: Vec<SemanticLocalDeclV1>,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(10)),
            SemanticLayoutIdentityV1::from_sha256(bytes(10)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(11)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn barrier_call(target: Option<u32>) -> SemanticTerminatorKindV1 {
        let destination = target.map(|target| {
            SemanticCallDestinationV1::new(
                scalar_place(),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )
        });
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                destination,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
    }

    fn scalar_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE).unwrap()
    }

    fn ranked_place(offset: u64) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn dereferenced_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn constant(value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    }

    fn option_dominance_chain(
        producer_count: usize,
    ) -> (SemanticFunctionDeclV1, Vec<SemanticOptionProducerV1>) {
        assert!(producer_count > 0 && producer_count <= 64);
        let mut locals = Vec::with_capacity(1 + 2 * producer_count);
        locals.push(local(0, SCALAR_TYPE, SemanticLocalRoleV1::Return));
        let mut producers = Vec::with_capacity(producer_count);
        let mut blocks = Vec::with_capacity(3 * producer_count + 1);
        let final_some = 2 * producer_count;
        for index in 0..producer_count {
            let option_local = SemanticLocalIdV1::from_index((1 + 2 * index) as u32);
            let discriminator_local = SemanticLocalIdV1::from_index((2 + 2 * index) as u32);
            locals.push(local(
                (1 + 2 * index) as u8,
                POINTER_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            locals.push(local(
                (2 + 2 * index) as u8,
                SCALAR_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            let producer_block = 2 * index;
            let switch_block = producer_block + 1;
            let some_target = if index + 1 == producer_count {
                final_some
            } else {
                producer_block + 2
            };
            let none_target = final_some + 1 + index;
            let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
            let discriminator_place =
                SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    option_place.clone(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, switch_block as u32),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            blocks.push(block(
                producer_block as u8,
                vec![],
                SemanticTerminatorKindV1::Call(call),
            ));
            blocks.push(block(
                switch_block as u8,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, none_target as u32),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, some_target as u32),
                    )
                    .unwrap(),
                },
            ));
            producers.push(SemanticOptionProducerV1::new(
                option_local,
                SemanticBlockIdV1::from_index(switch_block as u32),
            ));
        }
        blocks.push(block(
            final_some as u8,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        for index in 0..producer_count {
            blocks.push(block(
                (final_some + 1 + index) as u8,
                vec![],
                SemanticTerminatorKindV1::Return,
            ));
        }
        (projection_function_with_locals(blocks, locals), producers)
    }

    fn atomic_access() -> SemanticAtomicAccessV1 {
        SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicScopeV1::Agent,
        )
    }

    type AuditOutput = (
        Vec<ProductionRankedOperationV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    );

    fn audit_function(
        function: &SemanticFunctionDeclV1,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        let types = projection_types();
        let constants = constant_locals(function);
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut guarded_sites = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        for basic_block in function.blocks() {
            for semantic_statement in basic_block.statements() {
                project_statement_accesses(
                    &types,
                    function,
                    semantic_statement,
                    &constants,
                    &[],
                    &[],
                    &mut guarded_sites,
                    &mut projected_views,
                    &mut operations,
                    &mut sources,
                    &mut next_value,
                    &mut ranked_ir,
                )?;
            }
            project_terminator_accesses(
                &[],
                &types,
                function,
                basic_block.terminator().kind(),
                basic_block.terminator().source(),
                &constants,
                &[],
                &[],
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            )?;
        }
        Ok((operations, sources, ranked_ir))
    }

    fn audit_statements(
        statements: Vec<SemanticStatementV1>,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        audit_function(&projection_function(vec![block(
            30,
            statements,
            SemanticTerminatorKindV1::Return,
        )]))
    }

    fn access_kinds(operations: &[ProductionRankedOperationV1]) -> Vec<AccessKindAttr> {
        operations
            .iter()
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::Access { kind, .. }
                | ProductionRankedOperationV1::AtomicAccess { kind, .. } => Some(*kind),
                ProductionRankedOperationV1::View { .. }
                | ProductionRankedOperationV1::ViewInSpace { .. }
                | ProductionRankedOperationV1::IndexConstant { .. }
                | ProductionRankedOperationV1::InvocationIndex { .. }
                | ProductionRankedOperationV1::IndexBinary { .. }
                | ProductionRankedOperationV1::Dimension { .. }
                | ProductionRankedOperationV1::Barrier { .. }
                | ProductionRankedOperationV1::SemanticSymbol { .. }
                | ProductionRankedOperationV1::SemanticConstant { .. }
                | ProductionRankedOperationV1::SemanticBinary { .. }
                | ProductionRankedOperationV1::RequireEquivalent { .. } => None,
            })
            .collect()
    }

    fn single_guarded_cfg(
        entry: Vec<ProductionRankedOperationV1>,
        access: GuardedDisjointAccessV1,
    ) -> (
        Vec<ProductionRankedBlockV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    ) {
        let function =
            projection_function(vec![block(29, vec![], SemanticTerminatorKindV1::Return)]);
        let (blocks, sources) = build_ranked_cfg(
            &function,
            &vec![None; function.locals().len()],
            entry,
            vec![ProjectedSemanticBlockV1 {
                items: vec![ProjectedBlockItemV1::Guarded(access)],
            }],
        )
        .unwrap();
        let ranked_ir = format_ranked_cfg("guarded_test", &blocks).unwrap();
        (blocks, sources, ranked_ir)
    }

    fn assert_unsupported(
        result: Result<AuditOutput, ProductionRankedProjectionErrorV1>,
        expected: &'static str,
    ) {
        match result {
            Err(
                ProductionRankedProjectionErrorV1::Incomplete(detail)
                | ProductionRankedProjectionErrorV1::Unsupported(detail),
            ) => {
                assert_eq!(detail, expected)
            }
            Err(other) => panic!("expected unsupported projection, got {other}"),
            Ok(_) => panic!("hostile projection unexpectedly passed"),
        }
    }

    #[test]
    fn regular_and_atomic_stores_project_destination_and_value_footprints() {
        for atomic in [None, Some(atomic_access())] {
            let (operations, sources, _) = audit_statements(vec![statement(
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    ranked_place(0),
                    SemanticOperandV1::Copy(ranked_place(1)),
                    SemanticVolatilityV1::NonVolatile,
                    atomic,
                )),
            )])
            .unwrap();
            assert_eq!(
                access_kinds(&operations),
                vec![
                    if atomic.is_some() {
                        AccessKindAttr::AtomicWrite
                    } else {
                        AccessKindAttr::Write
                    },
                    AccessKindAttr::Read,
                ]
            );
            assert_eq!(sources.len(), 2);
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::ViewInSpace { .. }
                    ))
                    .count(),
                1,
                "two effects on one semantic allocation created different PLIRON views",
            );
        }
    }

    #[test]
    fn guarded_disjoint_access_is_ordinary_clean_pliron_cfg() {
        let invocation = ProductionRankedValueIdV1::new(0);
        let view = ProductionRankedValueIdV1::new(1);
        let entry = vec![
            ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 0,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
            },
        ];
        let guarded = GuardedDisjointAccessV1 {
            view,
            index: ProductionRankedValueV1::Local(invocation),
            precondition: None,
            allocation_origin: 0,
            extent_argument: 0,
            access: AccessKindAttr::Write,
            source: SemanticSourceProvenanceV1::unavailable(),
        };
        let (blocks, sources, ranked_ir) = single_guarded_cfg(entry, guarded);
        assert_eq!(blocks.len(), 4);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].block, 2);
        let kernel = ProductionRankedKernelV1::new("generic_checked_access", 1, blocks).unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("checked_access_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.bounds_report().is_clean());
        assert!(lowering.race_report().is_clean());
        assert!(ranked_ir.contains("kernel.cond_br") && ranked_ir.contains("kernel.access"));
        assert!(ranked_ir.contains("kernel.br ^bb1"));
    }

    #[test]
    fn safe_syncthreads_reaches_mandatory_barrier_and_workgroup_checks() {
        let kernel = ProductionRankedKernelV1::new(
            "safe_syncthreads",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                }],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("safe_syncthreads_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.barrier_report().is_clean());
        assert!(lowering.workgroup_report().is_clean());
    }

    #[test]
    fn guarded_access_and_barrier_retain_semantic_source_order() {
        let guarded = || {
            ProjectedBlockItemV1::Guarded(GuardedDisjointAccessV1 {
                view: ProductionRankedValueIdV1::new(0),
                allocation_origin: 0,
                index: ProductionRankedValueV1::Argument(0),
                precondition: None,
                extent_argument: 1,
                access: AccessKindAttr::Write,
                source: SemanticSourceProvenanceV1::unavailable(),
            })
        };
        let barrier = || ProjectedBlockItemV1::Effect {
            operation: ProductionRankedOperationV1::Barrier {
                execution_scope: HierarchyAttr::Workgroup,
                memory_scope: MemoryScopeAttr::Workgroup,
                address_space: AddressSpaceAttr::Workgroup,
                order: MemoryOrderAttr::AcquireRelease,
            },
            source: None,
        };
        let function = projection_function(vec![
            block(70, vec![], barrier_call(Some(1))),
            block(71, vec![], SemanticTerminatorKindV1::Return),
        ]);

        let (after_blocks, _) = build_ranked_cfg(
            &function,
            &vec![None; function.locals().len()],
            vec![],
            vec![
                ProjectedSemanticBlockV1 {
                    items: vec![guarded(), barrier()],
                },
                ProjectedSemanticBlockV1 { items: vec![] },
            ],
        )
        .unwrap();
        assert!(matches!(
            after_blocks[2].operations(),
            [ProductionRankedOperationV1::Access { .. }]
        ));
        assert!(matches!(
            after_blocks[3].operations(),
            [ProductionRankedOperationV1::Barrier { .. }]
        ));

        let (before_blocks, _) = build_ranked_cfg(
            &function,
            &vec![None; function.locals().len()],
            vec![],
            vec![
                ProjectedSemanticBlockV1 {
                    items: vec![barrier()],
                },
                ProjectedSemanticBlockV1 {
                    items: vec![guarded()],
                },
            ],
        )
        .unwrap();
        assert!(matches!(
            before_blocks[1].operations(),
            [ProductionRankedOperationV1::Barrier { .. }]
        ));
        assert!(matches!(
            before_blocks[3].operations(),
            [ProductionRankedOperationV1::Access { .. }]
        ));
    }

    #[test]
    fn shifted_disjoint_access_retains_overflow_and_extent_guards() {
        let invocation = ProductionRankedValueIdV1::new(0);
        let offset = ProductionRankedValueIdV1::new(1);
        let shifted = ProductionRankedValueIdV1::new(2);
        let upper = ProductionRankedValueIdV1::new(3);
        let view = ProductionRankedValueIdV1::new(4);
        let entry = vec![
            ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 0,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: offset,
                value: 4,
            },
            ProductionRankedOperationV1::IndexBinary {
                result: shifted,
                kind: IndexBinaryKindAttr::Add,
                lhs: ProductionRankedValueV1::Local(invocation),
                rhs: ProductionRankedValueV1::Local(offset),
            },
            ProductionRankedOperationV1::IndexConstant {
                result: upper,
                value: u64::MAX - 3,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
            },
        ];
        let guarded = GuardedDisjointAccessV1 {
            view,
            allocation_origin: 0,
            index: ProductionRankedValueV1::Local(shifted),
            precondition: Some((
                ProductionRankedValueV1::Local(invocation),
                ProductionRankedValueV1::Local(upper),
            )),
            extent_argument: 0,
            access: AccessKindAttr::Write,
            source: SemanticSourceProvenanceV1::unavailable(),
        };
        let (blocks, sources, ranked_ir) = single_guarded_cfg(entry, guarded);
        assert_eq!(blocks.len(), 5);
        assert_eq!(sources[0].block, 3);
        assert!(ranked_ir.contains("kernel.br ^bb1"));
        assert!(ranked_ir.contains("^bb3:"));

        let kernel = ProductionRankedKernelV1::new("shifted_checked_access", 1, blocks).unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("shifted_access_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.bounds_report().is_clean());
        assert!(lowering.race_report().is_clean());
    }

    #[test]
    fn shared_option_dominance_scales_with_cfg_and_producer_count() {
        let (small_function, small_producers) = option_dominance_chain(16);
        let small = SemanticOptionDominanceV1::analyze(&small_function, &small_producers).unwrap();
        let (large_function, large_producers) = option_dominance_chain(64);
        let large = SemanticOptionDominanceV1::analyze(&large_function, &large_producers).unwrap();

        assert!(large.work_units() <= small.work_units() * 5);
        for producer in large_producers {
            assert!(large.availability(producer.option_local()).is_some());
        }
    }

    #[test]
    fn private_address_space_five_remains_in_the_generic_memory_model() {
        assert_eq!(memory_space(5).unwrap(), MemorySpaceAttr::Private);
    }

    #[test]
    fn reassigned_option_discriminator_cannot_mint_payload_authority() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let discriminant = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            discriminator_place.clone(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(option_place.clone()),
            ),
        )));
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place,
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(50, vec![], SemanticTerminatorKindV1::Call(call.clone())),
            block(
                51,
                vec![
                    discriminant,
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
                    ))),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(52, vec![], SemanticTerminatorKindV1::Return),
            block(53, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability discriminator does not have one exact definition"
        );
    }
    #[test]
    fn unrelated_switch_cannot_authenticate_option_payload() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(60, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                61,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place,
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: constant(1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(62, vec![], SemanticTerminatorKindV1::Return),
            block(63, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability switch is not bound to its unique discriminator"
        );
    }

    #[test]
    fn alternate_predecessor_cannot_enter_an_authenticated_some_target() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(70, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                71,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(72, vec![], SemanticTerminatorKindV1::Return),
            block(
                73,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability Some target is not uniquely controlled by its exact branch"
        );
    }

    #[test]
    fn option_payload_availability_excludes_the_none_merge() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(40, vec![], SemanticTerminatorKindV1::Call(call.clone())),
            block(
                41,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            ),
                        ],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(
                42,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                43,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(44, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let dominance = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap();
        let authority = dominance.availability(option_local).unwrap();

        assert!(dominance.allows(authority, SemanticBlockIdV1::from_index(2)));
        assert!(!dominance.allows(authority, SemanticBlockIdV1::from_index(1)));
        assert!(!dominance.allows(authority, SemanticBlockIdV1::from_index(3)));
        assert!(
            !dominance.allows(authority, SemanticBlockIdV1::from_index(4)),
            "the merge is reachable from None and must not inherit payload authority",
        );
    }

    #[test]
    fn capability_alias_worklist_processes_each_charged_edge_once() {
        let mut edges = vec![Vec::new(); 4];
        let mut charged = 0;
        for source in 0..3 {
            push_capability_edge(
                &mut edges,
                &mut charged,
                source,
                CapabilityEdgeV1 {
                    destination: source + 1,
                    use_block: 0,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )
            .unwrap();
        }
        let seed = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let mut values = vec![None; 4];
        let grid = vec![None; 4];
        let mut worklist = VecDeque::from([0]);
        values[0] = Some(seed);
        let mut processed = 0;
        while let Some(source) = worklist.pop_front() {
            for edge in &edges[source] {
                processed += 1;
                assert_eq!(edge.kind, CapabilityEdgeKindV1::Alias);
                assign_index_capability(
                    edge.destination,
                    values[source].unwrap(),
                    &mut values,
                    &grid,
                    &mut worklist,
                )
                .unwrap();
            }
        }

        assert_eq!(charged, 3);
        assert_eq!(processed, charged);
        assert!(values.iter().all(|value| *value == Some(seed)));
    }

    #[test]
    fn capability_alias_cycle_terminates_with_exact_edge_charge() {
        let mut edges = vec![Vec::new(); 2];
        let mut charged = 0;
        for (source, destination) in [(0, 1), (1, 0)] {
            push_capability_edge(
                &mut edges,
                &mut charged,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: 0,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )
            .unwrap();
        }
        let seed = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let mut values = vec![Some(seed), None];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::from([0]);
        let mut processed = 0;
        while let Some(source) = worklist.pop_front() {
            for edge in &edges[source] {
                processed += 1;
                assign_index_capability(
                    edge.destination,
                    values[source].unwrap(),
                    &mut values,
                    &grid,
                    &mut worklist,
                )
                .unwrap();
            }
        }

        assert_eq!(processed, charged);
        assert_eq!(values, vec![Some(seed), Some(seed)]);
    }
    #[test]
    fn conflicting_capability_def_use_paths_fail_closed() {
        let first = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let second = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(1),
            ..first
        };
        let mut values = vec![None, Some(first)];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::new();

        assert!(matches!(
            assign_index_capability(1, second, &mut values, &grid, &mut worklist),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local"
            ))
        ));
    }

    #[test]
    fn distinct_writable_origins_cannot_share_an_unqualified_ranked_graph() {
        let mut admitted = None;
        admit_writable_origin(&mut admitted, 0).unwrap();
        admit_writable_origin(&mut admitted, 0).unwrap();
        let error = admit_writable_origin(&mut admitted, 1).unwrap_err();

        assert!(matches!(
            error,
            ProductionRankedProjectionErrorV1::Incomplete(
                "multiple writable global allocation origins require an authenticated runtime no-alias obligation"
            )
        ));
    }
    #[test]
    fn checked_reference_provenance_covers_only_the_exact_pointee() {
        let origins = [None, None, None, Some(7)];
        assert_eq!(
            checked_reference_origin(&dereferenced_place(), &origins),
            Some(7)
        );
        let nested_index = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ARRAY_TYPE)
                    .unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_eq!(checked_reference_origin(&nested_index, &origins), None);

        let function = projection_function(vec![block(
            31,
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], POINTER_TYPE)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(3),
                                vec![],
                                POINTER_TYPE,
                            )
                            .unwrap(),
                        )),
                    ),
                ))),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    dereferenced_place(),
                    SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
                ))),
            ],
            SemanticTerminatorKindV1::Return,
        )]);
        assert_eq!(local_definition_counts(&function)[3], 1);
    }

    #[test]
    fn atomic_rmw_projects_result_one_atomic_address_effect_and_value() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticAtomicRmwOpV1::Add,
                atomic_access(),
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![AccessKindAttr::AtomicReadModifyWrite, AccessKindAttr::Read,]
        );
        assert_eq!(sources.len(), 2);
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::Relaxed,
                scope: AtomicScopeAttr::Agent,
                ..
            }
        )));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::AtomicRead
                    | AccessKindAttr::AtomicWrite
                    | AccessKindAttr::AtomicReadModifyWrite,
                ..
            }
        )));
    }

    #[test]
    fn atomic_compare_exchange_projects_both_candidates_and_address_effects() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticOperandV1::Move(ranked_place(2)),
                atomic_access(),
                SemanticAtomicOrderingV1::Relaxed,
                false,
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::AtomicReadModifyWrite,
                AccessKindAttr::Read,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn discriminant_and_deinitialize_places_are_not_silently_skipped() {
        let discriminant_read = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            scalar_place(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(ranked_place(0)),
            ),
        ));
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: ranked_place(1),
                variant_index: 0,
            }),
            statement(SemanticStatementKindV1::Deinitialize(ranked_place(2))),
            statement(discriminant_read),
        ])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::Write,
                AccessKindAttr::Write,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn storage_markers_and_nop_are_explicit_zero_effect_statements() {
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::Nop),
        ])
        .unwrap();
        assert!(operations.is_empty());
        assert!(sources.is_empty());

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(99),
            ))]),
            "a storage statement with an out-of-range local",
        );
    }

    #[test]
    fn explicit_or_dereferenced_unranked_memory_fails_closed() {
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(
                    scalar_place(),
                    constant(7),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
            ))]),
            "an explicit memory operation without a ranked index projection",
        );

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    scalar_place(),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            dereferenced_place(),
                            SemanticVolatilityV1::Volatile,
                            None,
                        )),
                    ),
                ),
            ))]),
            "a dereferenced memory access without a ranked index projection",
        );
    }

    #[test]
    fn unsupported_place_forms_fail_before_a_clean_result() {
        let hostile = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Subslice {
                        from: 0,
                        to: 1,
                        from_end: false,
                    },
                    ARRAY_TYPE,
                )
                .unwrap(),
            ],
            ARRAY_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                hostile,
            ))]),
            "an indexed place containing a subslice projection",
        );

        let dynamic = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                dynamic,
            ))]),
            "a dynamic index before dynamic ranked-value projection is available",
        );
    }

    #[test]
    fn unreachable_blocks_are_still_audited_for_memory_effects() {
        let function = projection_function(vec![
            block(40, vec![], SemanticTerminatorKindV1::Return),
            block(
                41,
                vec![statement(SemanticStatementKindV1::Store(
                    SemanticMemoryStoreV1::new(
                        scalar_place(),
                        constant(1),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    ),
                ))],
                SemanticTerminatorKindV1::Unreachable,
            ),
        ]);
        assert_unsupported(
            audit_function(&function),
            "an explicit memory operation without a ranked index projection",
        );
    }

    #[test]
    fn unresolved_call_and_drop_effects_fail_before_a_clean_result() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        assert_unsupported(
            audit_function(&projection_function(vec![block(
                42,
                vec![],
                SemanticTerminatorKindV1::Call(call),
            )])),
            "a call terminator before exact callable memory-effect summaries are available",
        );

        let edge = SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::DropReturn,
            SemanticBlockIdV1::from_index(0),
        );
        assert_unsupported(
            audit_function(&projection_function(vec![block(
                43,
                vec![],
                SemanticTerminatorKindV1::Drop {
                    place: scalar_place(),
                    drop_glue: SemanticFunctionIdV1::from_index(0),
                    target: edge,
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            )])),
            "a drop terminator before exact drop-glue memory-effect summaries are available",
        );
    }

    #[test]
    fn statement_projection_stops_at_the_ranked_operation_bound() {
        let function =
            projection_function(vec![block(50, vec![], SemanticTerminatorKindV1::Return)]);
        let types = projection_types();
        let semantic_statement =
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                ranked_place(0),
                constant(1),
                SemanticVolatilityV1::NonVolatile,
                None,
            )));
        let mut operations = vec![
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 0,
            };
            MAX_PROJECTED_OPERATIONS_V1 - 2
        ];
        let original = operations.len();
        let mut sources = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut guarded_sites = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let error = project_statement_accesses(
            &types,
            &function,
            &semantic_statement,
            &[None; 4],
            &[],
            &[],
            &mut guarded_sites,
            &mut projected_views,
            &mut operations,
            &mut sources,
            &mut next_value,
            &mut ranked_ir,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic statement projection exceeding the ranked operation limit"
            )
        ));
        assert_eq!(operations.len(), original);
        assert!(sources.is_empty());
        assert!(ranked_ir.is_empty());
    }

    #[test]
    fn constant_aliases_resolve_once_in_linear_time() {
        let definitions = [
            ConstantDefinitionV1::Direct(64),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(1)),
        ];
        let mut states = [0; 3];
        let mut values = [None; 3];
        assert_eq!(
            resolve_constant(2, &definitions, &mut states, &mut values),
            Some(64),
        );
        assert_eq!(values, [Some(64); 3]);
        assert_eq!(states, [2; 3]);
    }

    #[test]
    fn cyclic_or_multiply_defined_indices_are_not_constants() {
        let cycle = [
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(1)),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
        ];
        let mut states = [0; 2];
        let mut values = [None; 2];
        assert_eq!(resolve_constant(0, &cycle, &mut states, &mut values), None,);

        let mut definitions = [ConstantDefinitionV1::Missing];
        record_constant_definition(
            &mut definitions,
            SemanticLocalIdV1::from_index(0),
            ConstantDefinitionV1::Direct(63),
        );
        record_constant_definition(
            &mut definitions,
            SemanticLocalIdV1::from_index(0),
            ConstantDefinitionV1::Direct(64),
        );
        assert!(matches!(definitions[0], ConstantDefinitionV1::Invalid));
    }

    #[test]
    fn source_label_is_explicit_when_unavailable() {
        assert_eq!(
            source_label(SemanticSourceProvenanceV1::unavailable()),
            "Rust source location unavailable",
        );
    }
}
