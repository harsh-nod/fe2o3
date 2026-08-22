//! Generic projection from admitted semantic MIR into safety-verifiable ranked PLIRON.
//!
//! Static proof facts come from the indexed place and its semantic array type.
//! Rust bounds-assert terminators are retained in semantic MIR but do not
//! manufacture a static extent or authorize an access in this projection.

use std::{collections::VecDeque, fmt};

use dialect_kernel::{
    AccessKindAttr, DYNAMIC_EXTENT, IndexBinaryKindAttr, MAX_RANKED_MEMORY_RANK, MemorySpaceAttr,
    SUPPORTED_ELEMENT_WIDTHS,
};
use fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_OPERATIONS;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCallableIdV1, SemanticCompilerIntrinsicOperationV1,
    SemanticConstantValueV1, SemanticDirectCallV1, SemanticDirectTailCallV1,
    SemanticDisjointIndexSpaceV1, SemanticFunctionDeclV1, SemanticFunctionRoleV1,
    SemanticLocalIdV1, SemanticLocalRoleV1, SemanticOperandV1, SemanticPlaceV1,
    SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticSourceProvenanceV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelErrorV1, ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1,
    ProductionSessionErrorV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
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
    index: ProductionRankedValueV1,
    precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    extent_argument: u32,
    access: AccessKindAttr,
    source: SemanticSourceProvenanceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedDisjointIndexV1 {
    value: ProductionRankedValueV1,
    mapping: SemanticDisjointIndexSpaceV1,
    precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
}

struct IntrinsicProjectionV1 {
    checked_reference_origins: Vec<Option<usize>>,
    guarded_accesses: Vec<GuardedDisjointAccessV1>,
    extent_argument_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedViewV1 {
    result: ProductionRankedValueIdV1,
    element_width: u32,
    shape: Vec<u64>,
    memory_space: MemorySpaceAttr,
}

/// Move-only result retaining both the exact admitted Rust semantics and the
/// owner-held PLIRON graph that passed every mandatory generic kernel check.
pub(crate) struct ProductionRankedSemanticProgramV1 {
    semantic: ProductionSemanticMirOwnerV1,
    lowering: ProductionRankedKernelLoweringInputV1,
    ranked_ir: String,
}

impl ProductionRankedSemanticProgramV1 {
    pub(crate) fn ranked_ir(&self) -> &str {
        &self.ranked_ir
    }

    pub(crate) fn function_name(&self) -> &str {
        self.lowering.kernel().function_name()
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.semantic.semantic().functions().len()
    }

    pub(crate) fn semantic_callable_count(&self) -> usize {
        self.semantic.semantic().callables().len()
    }

    pub(crate) fn bounds_are_clean(&self) -> bool {
        self.lowering.bounds_report().is_clean()
    }

    pub(crate) fn all_kernel_checks_are_clean(&self) -> bool {
        self.lowering.bounds_report().is_clean()
            && self.lowering.race_report().is_clean()
            && self.lowering.barrier_report().is_clean()
            && self.lowering.workgroup_report().is_clean()
            && self.lowering.semantic_report().is_clean()
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn into_verified_semantic_owner(self) -> ProductionSemanticMirOwnerV1 {
        let Self {
            semantic,
            lowering,
            ranked_ir,
        } = self;
        drop((lowering, ranked_ir));
        semantic
    }
}

#[derive(Debug)]
pub(crate) enum ProductionRankedProjectionErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    Incomplete(&'static str),
    Unsupported(&'static str),
    Recipe(ProductionRankedKernelErrorV1),
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
    let mut operations = Vec::new();
    let mut sources = Vec::new();
    let mut next_value = 0_u32;
    let mut ranked_ir = String::new();
    let mut incomplete = None;
    let mut projected_views = vec![None; function.locals().len()];
    push_ranked_ir(
        &mut ranked_ir,
        &format!("func @{} {{\n", function_name(function)?),
    )?;
    let intrinsic = project_intrinsic_contracts(
        semantic.callables(),
        semantic.types(),
        function,
        &constants,
        &mut operations,
        &mut next_value,
        &mut ranked_ir,
    )?;
    for block in function.blocks() {
        for statement in block.statements() {
            retain_incomplete(
                project_statement_accesses(
                    semantic.types(),
                    function,
                    statement,
                    &constants,
                    &intrinsic.checked_reference_origins,
                    &mut projected_views,
                    &mut operations,
                    &mut sources,
                    &mut next_value,
                    &mut ranked_ir,
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
                &mut projected_views,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            ),
            &mut incomplete,
        )?;
    }
    if sources.is_empty() && intrinsic.guarded_accesses.is_empty() {
        if let Some(detail) = incomplete {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
        }
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access",
        ));
    }
    if sources
        .iter()
        .any(|source| source.memory_space != MemorySpaceAttr::Private)
        && !operations.iter().any(|operation| {
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
    let blocks = finish_guarded_access_graph(
        operations,
        &intrinsic.guarded_accesses,
        &mut sources,
        &mut ranked_ir,
    )?;
    push_ranked_ir(&mut ranked_ir, "}\n")?;

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
    Ok(ProductionRankedSemanticProgramV1 {
        semantic: semantic_owner,
        lowering,
        ranked_ir,
    })
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
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        if matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier
                    | SemanticCompilerIntrinsicOperationV1::WaveBarrier,
                ..
            })
        ) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a barrier before exact semantic CFG projection is available",
            ));
        }
    }
    let mut index_values = vec![None; function.locals().len()];
    let mut grid_leader_destinations = vec![false; function.locals().len()];
    let mut grid_leader_precondition = None;
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
        if destination >= index_values.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an invocation-capability destination outside the semantic local table",
            ));
        }
        if index_values[destination].is_some() || grid_leader_destinations[destination] {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple invocation capabilities for one semantic local",
            ));
        }
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension: 0,
            launch_extent: 0,
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
                });
            }
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader } => {
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
                let precondition = (
                    ProductionRankedValueV1::Local(result),
                    ProductionRankedValueV1::Local(one),
                );
                grid_leader_destinations[destination] = true;
                match grid_leader_precondition {
                    None => grid_leader_precondition = Some((*grid_leader, precondition)),
                    Some((existing, _)) if existing == *grid_leader => {}
                    Some(_) => {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "multiple grid-leader semantic type identities",
                        ));
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for block in function.blocks() {
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let source = match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand) => transparent_operand_place(operand),
                    _ => None,
                };
                let Some(source) = source else {
                    continue;
                };
                let source = source.local().index() as usize;
                let destination = assignment.destination().local().index() as usize;
                if source >= index_values.len() || destination >= index_values.len() {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an invocation-capability alias outside the semantic local table",
                    ));
                }
                if index_values[destination].is_none() && index_values[source].is_some() {
                    index_values[destination] = index_values[source];
                    changed = true;
                }
            }
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            let (mapping, offset, passthrough) = match operation {
                SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                    index_space,
                    ..
                } => (*index_space, 0, true),
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                    output_space,
                    offset,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                    output_space,
                    offset,
                    ..
                } => (*output_space, *offset, false),
                _ => continue,
            };
            let destination = simple_call_destination(call)?.index() as usize;
            if index_values[destination].is_some() {
                continue;
            }
            let source = call
                .arguments()
                .first()
                .and_then(simple_operand_local)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "an index capability transform without one exact input local",
                ))?
                .index() as usize;
            let Some(input) = index_values[source] else {
                continue;
            };
            if passthrough {
                index_values[destination] = Some(ProjectedDisjointIndexV1 { mapping, ..input });
                changed = true;
                continue;
            }
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
            index_values[destination] = Some(ProjectedDisjointIndexV1 {
                value: ProductionRankedValueV1::Local(shifted),
                mapping,
                precondition,
            });
            changed = true;
        }
    }

    let mut guarded_accesses = Vec::new();
    for block in function.blocks() {
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
                let projected = projected_disjoint_operand_v1(call, 1, &index_values)?;
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
                let projected = projected_disjoint_operand_v1(call, 1, &index_values)?;
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
                let Some((producer_type, precondition)) = grid_leader_precondition else {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "an exclusive access without a grid-leader producer",
                    ));
                };
                if producer_type != *grid_leader {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "grid-leader capability type identity changed",
                    ));
                }
                let value = call
                    .arguments()
                    .get(2)
                    .and_then(|operand| constant_operand_value(operand, constants))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic grid-exclusive index before ranked-value projection is available",
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
                    lhs: precondition.0,
                    rhs: ProductionRankedValueV1::Local(constant_index),
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_binary Add {}, %{}\n",
                        index.get(),
                        ranked_value_text_v1(precondition.0),
                        constant_index.get(),
                    ),
                )?;
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    Some(precondition),
                )
            }
            _ => continue,
        };
        let extent_argument = u32::try_from(guarded_accesses.len()).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "too many checked disjoint extents for the ranked recipe",
            )
        })?;
        reserve_operation(operations)?;
        let view = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: view,
            element_width: type_width(types, element)?,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(extent_argument)],
            memory_space: MemorySpaceAttr::Global,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "  %{} = kernel.ranked_view <{}, true, [dynamic], Global>(%arg{})\n",
                view.get(),
                type_width(types, element)?,
                extent_argument,
            ),
        )?;
        guarded_accesses.push(GuardedDisjointAccessV1 {
            view,
            index,
            precondition,
            extent_argument,
            access: AccessKindAttr::Write,
            source: block.terminator().source(),
        });
    }

    let checked_reference_origins =
        checked_reference_origins(function, callables, guarded_accesses.len())?;
    Ok(IntrinsicProjectionV1 {
        checked_reference_origins,
        extent_argument_count: guarded_accesses.len(),
        guarded_accesses,
    })
}

fn projected_disjoint_operand_v1(
    call: &SemanticDirectCallV1,
    argument: usize,
    values: &[Option<ProjectedDisjointIndexV1>],
) -> Result<ProjectedDisjointIndexV1, ProductionRankedProjectionErrorV1> {
    let local = call
        .arguments()
        .get(argument)
        .and_then(simple_operand_local)
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access whose index witness is not one exact local",
        ))?;
    values.get(local.index() as usize).copied().flatten().ok_or(
        ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access not bound to authenticated index authority",
        ),
    )
}

fn ranked_value_text_v1(value: ProductionRankedValueV1) -> String {
    match value {
        ProductionRankedValueV1::Local(identity) => format!("%{}", identity.get()),
        ProductionRankedValueV1::Argument(argument) => format!("%arg{argument}"),
    }
}

fn finish_guarded_access_graph(
    entry_operations: Vec<ProductionRankedOperationV1>,
    guarded: &[GuardedDisjointAccessV1],
    sources: &mut Vec<ProjectedAccessSourceV1>,
    ranked_ir: &mut String,
) -> Result<Vec<ProductionRankedBlockV1>, ProductionRankedProjectionErrorV1> {
    if guarded.is_empty() {
        push_ranked_ir(ranked_ir, "  kernel.return\n")?;
        return Ok(vec![ProductionRankedBlockV1::new(
            entry_operations,
            ProductionRankedTerminatorV1::Return,
        )]);
    }
    let mut starts = Vec::with_capacity(guarded.len());
    let mut cursor = 1_usize;
    for access in guarded {
        starts.push(cursor);
        cursor = cursor
            .checked_add(2 + usize::from(access.precondition.is_some()))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "checked-access CFG block count overflow",
            ))?;
    }
    let block_count =
        cursor
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "checked-access CFG block count overflow",
            ))?;
    if entry_operations
        .len()
        .checked_add(guarded.len())
        .and_then(|count| count.checked_add(block_count))
        .is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS)
    {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "checked-access CFG exceeds the ranked operation limit",
        ));
    }
    let final_block = u32::try_from(cursor).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "checked-access CFG block identity does not fit u32",
        )
    })?;
    let mut blocks = Vec::new();
    blocks.try_reserve_exact(block_count).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "checked-access CFG storage cannot be reserved",
        )
    })?;
    blocks.push(ProductionRankedBlockV1::new(
        entry_operations,
        ProductionRankedTerminatorV1::Branch { target: 1 },
    ));
    push_ranked_ir(
        ranked_ir,
        &format!("  kernel.br ^{}\n", guarded_block_label(0, &guarded[0])),
    )?;
    for (access_index, access) in guarded.iter().enumerate() {
        let guard_block = u32::try_from(starts[access_index]).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "checked-access guard block identity does not fit u32",
            )
        })?;
        let bounds_block = guard_block + u32::from(access.precondition.is_some());
        let access_block = bounds_block + 1;
        let next_guard = if access_index + 1 == guarded.len() {
            final_block
        } else {
            u32::try_from(starts[access_index + 1]).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "checked-access guard block identity does not fit u32",
                )
            })?
        };
        let next_label = if access_index + 1 == guarded.len() {
            "exit".to_owned()
        } else {
            guarded_block_label(access_index + 1, &guarded[access_index + 1])
        };
        if let Some((lhs, rhs)) = access.precondition {
            blocks.push(ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs,
                    rhs,
                    true_block: bounds_block,
                    false_block: next_guard,
                },
            ));
            push_ranked_ir(
                ranked_ir,
                &format!(
                    "^guard{access_index}:\n  kernel.cond_br {} < {} ^bounds{access_index}, ^{next_label}\n",
                    ranked_value_text_v1(lhs),
                    ranked_value_text_v1(rhs),
                ),
            )?;
        }
        blocks.push(ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: access.index,
                rhs: ProductionRankedValueV1::Argument(access.extent_argument),
                true_block: access_block,
                false_block: next_guard,
            },
        ));
        blocks.push(ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::Access {
                kind: access.access,
                view: ProductionRankedValueV1::Local(access.view),
                indices: vec![access.index],
            }],
            ProductionRankedTerminatorV1::Branch { target: next_guard },
        ));
        sources.push(ProjectedAccessSourceV1 {
            block: access_block as usize,
            operation: 0,
            access: access.access,
            memory_space: MemorySpaceAttr::Global,
            source: access.source,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "^bounds{access_index}:\n  kernel.cond_br {} < %arg{} ^access{access_index}, ^{next_label}\n^access{access_index}:\n  kernel.access {:?} %{}[{}]\n  kernel.br ^{next_label}\n",
                ranked_value_text_v1(access.index),
                access.extent_argument,
                access.access,
                access.view.get(),
                ranked_value_text_v1(access.index),
            ),
        )?;
    }
    blocks.push(ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    ));
    push_ranked_ir(ranked_ir, "^exit:\n  kernel.return\n")?;
    Ok(blocks)
}

fn guarded_block_label(index: usize, access: &GuardedDisjointAccessV1) -> String {
    if access.precondition.is_some() {
        format!("guard{index}")
    } else {
        format!("bounds{index}")
    }
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
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. },
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
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::Store(store) => {
            project_place_access(
                types,
                function,
                store.destination(),
                if store.atomic().is_some() {
                    AccessKindAttr::AtomicWrite
                } else {
                    AccessKindAttr::Write
                },
                PlaceAccessRequirementV1::ExplicitMemory,
                source,
                constants,
                checked_reference_origins,
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
                source,
                constants,
                checked_reference_origins,
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
                source,
                constants,
                checked_reference_origins,
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
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    checked_reference_origins: &[Option<usize>],
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_place_access(
        types,
        function,
        address,
        AccessKindAttr::AtomicReadModifyWrite,
        PlaceAccessRequirementV1::ExplicitMemory,
        source,
        constants,
        checked_reference_origins,
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
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if matches!(
        callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. },
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
                    projected_views,
                    operations,
                    sources,
                    next_value,
                    ranked_ir,
                )?;
            }
            Ok(())
        }
        SemanticRvalueKindV1::Load(load) => project_place_access(
            types,
            function,
            load.source(),
            if load.atomic().is_some() {
                AccessKindAttr::AtomicRead
            } else {
                AccessKindAttr::Read
            },
            PlaceAccessRequirementV1::ExplicitMemory,
            source,
            constants,
            checked_reference_origins,
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
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if checked_reference_origin(place, checked_reference_origins).is_some() {
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
    operations.push(ProductionRankedOperationV1::Access {
        kind: access,
        view: ProductionRankedValueV1::Local(view_id),
        indices: ranked_indices.clone(),
    });
    push_ranked_ir(
        ranked_ir,
        &format!(
            "  kernel.access {:?} %{}[{}]\n",
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

    fn projection_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
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
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
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
                | ProductionRankedOperationV1::AtomicAccess { .. }
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
        let guarded = [GuardedDisjointAccessV1 {
            view,
            index: ProductionRankedValueV1::Local(invocation),
            precondition: None,
            extent_argument: 0,
            access: AccessKindAttr::Write,
            source: SemanticSourceProvenanceV1::unavailable(),
        }];
        let mut sources = Vec::new();
        let mut ranked_ir = String::new();
        let blocks =
            finish_guarded_access_graph(entry, &guarded, &mut sources, &mut ranked_ir).unwrap();
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
        assert!(ranked_ir.contains("kernel.br ^bounds0"));
        assert!(!ranked_ir.contains("^guard0"));
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
        let guarded = [GuardedDisjointAccessV1 {
            view,
            index: ProductionRankedValueV1::Local(shifted),
            precondition: Some((
                ProductionRankedValueV1::Local(invocation),
                ProductionRankedValueV1::Local(upper),
            )),
            extent_argument: 0,
            access: AccessKindAttr::Write,
            source: SemanticSourceProvenanceV1::unavailable(),
        }];
        let mut sources = Vec::new();
        let mut ranked_ir = String::new();
        let blocks =
            finish_guarded_access_graph(entry, &guarded, &mut sources, &mut ranked_ir).unwrap();
        assert_eq!(blocks.len(), 5);
        assert_eq!(sources[0].block, 3);
        assert!(ranked_ir.contains("kernel.br ^guard0"));
        assert!(ranked_ir.contains("^guard0:"));
        assert!(ranked_ir.contains("^bounds0:"));

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
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let error = project_statement_accesses(
            &types,
            &function,
            &semantic_statement,
            &[None; 4],
            &[],
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
