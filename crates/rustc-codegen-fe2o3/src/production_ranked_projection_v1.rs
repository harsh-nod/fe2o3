//! Generic projection from admitted semantic MIR into bounds-verifiable ranked PLIRON.
//!
//! Static proof facts come from the indexed place and its semantic array type.
//! Rust bounds-assert terminators are retained in semantic MIR but do not
//! manufacture a static extent or authorize an access in this projection.

use std::fmt;

use dialect_kernel::{AccessKindAttr, SUPPORTED_ELEMENT_WIDTHS};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantValueV1, SemanticFunctionDeclV1, SemanticFunctionRoleV1, SemanticLocalIdV1,
    SemanticOperandV1, SemanticPlaceV1, SemanticProjectionKindV1, SemanticRvalueKindV1,
    SemanticSourceProvenanceV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
    SemanticTypeIdV1, SemanticTypeShapeV1,
};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelErrorV1, ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1,
    ProductionSessionErrorV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

const ROOT_NAME_V1: &str = "semantic_bounds_module";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedAccessSourceV1 {
    operation: usize,
    access: AccessKindAttr,
    source: SemanticSourceProvenanceV1,
}

/// Move-only result retaining both the exact admitted Rust semantics and the
/// owner-held PLIRON graph that passed generic ranked-bounds verification.
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

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum ProductionRankedProjectionErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
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
                            operation,
                            ..
                        }
                        | fe2o3_kernel_analysis::RankedBoundsFindingV1::UnprovedBound {
                            operation,
                            ..
                        } = finding
                            && let Some(access) = access_sources
                                .iter()
                                .find(|source| source.operation == *operation)
                        {
                            write!(formatter, "\n  --> {}", source_label(access.source))?;
                            write!(
                                formatter,
                                "\n  = Rust {:?} projected to kernel.access at block 0 op {}",
                                access.access, access.operation,
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
            Self::Unsupported(_) | Self::Construction(_) => None,
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
    let mut ranked_ir = format!("func @{} {{\n", function_name(function)?);
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            project_place_access(
                semantic.types(),
                function,
                assignment.destination(),
                AccessKindAttr::Write,
                statement.source(),
                &constants,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            )?;
            project_rvalue_reads(
                semantic.types(),
                function,
                assignment.value().kind(),
                statement.source(),
                &constants,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            )?;
        }
    }
    if sources.is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access",
        ));
    }
    ranked_ir.push_str("  kernel.return\n}\n");

    let kernel = ProductionRankedKernelV1::new(
        function_name(function)?,
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
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
    Ok(ProductionRankedSemanticProgramV1 {
        semantic: semantic_owner,
        lowering,
        ranked_ir,
    })
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
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match value {
        SemanticRvalueKindV1::Use(operand) => project_operand_read(
            types, function, operand, source, constants, operations, sources, next_value, ranked_ir,
        ),
        SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => project_operand_read(
            types, function, operand, source, constants, operations, sources, next_value, ranked_ir,
        ),
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            project_operand_read(
                types, function, left, source, constants, operations, sources, next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types, function, right, source, constants, operations, sources, next_value,
                ranked_ir,
            )
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                project_operand_read(
                    types, function, operand, source, constants, operations, sources, next_value,
                    ranked_ir,
                )?;
            }
            Ok(())
        }
        SemanticRvalueKindV1::Load(load) => project_place_access(
            types,
            function,
            load.source(),
            AccessKindAttr::Read,
            source,
            constants,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Borrow { .. }
        | SemanticRvalueKindV1::AddressOf { .. }
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_) => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_operand_read(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
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
            source,
            constants,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_place_access(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed place with an out-of-range local",
        ));
    };
    let mut current = local.ty();
    let mut shape = Vec::new();
    let mut indices = Vec::new();
    for projection in place.projections() {
        match projection.kind() {
            SemanticProjectionKindV1::Dereference => {
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
                current = pointer.pointee();
            }
            SemanticProjectionKindV1::Index(index) => {
                let extent = static_array_extent(types, current)?;
                let value = constants
                    .get(index.index() as usize)
                    .copied()
                    .flatten()
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a dynamic index before dynamic ranked-value projection is available",
                    ))?;
                shape.push(extent);
                indices.push(value);
                current = projection.result_type();
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset, from_end, ..
            } => {
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
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an indexed place containing a subslice projection",
                ));
            }
        }
    }
    if indices.is_empty() {
        return Ok(());
    }
    let element_width = type_width(types, place.ty())?;
    let view_id = next_value_id(next_value)?;
    operations.push(ProductionRankedOperationV1::View {
        result: view_id,
        element_width,
        writable: access == AccessKindAttr::Write,
        shape: shape.clone(),
        dynamic_extents: vec![],
    });
    ranked_ir.push_str(&format!(
        "  %{} = kernel.ranked_view <{}, {}, {:?}>\n",
        view_id.get(),
        element_width,
        access == AccessKindAttr::Write,
        shape,
    ));
    let mut ranked_indices = Vec::with_capacity(indices.len());
    for value in indices {
        let index_id = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::IndexConstant {
            result: index_id,
            value,
        });
        ranked_ir.push_str(&format!(
            "  %{} = kernel.index_constant {}\n",
            index_id.get(),
            value,
        ));
        ranked_indices.push(ProductionRankedValueV1::Local(index_id));
    }
    let operation = operations.len();
    operations.push(ProductionRankedOperationV1::Access {
        kind: access,
        view: ProductionRankedValueV1::Local(view_id),
        indices: ranked_indices.clone(),
    });
    ranked_ir.push_str(&format!(
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
    ));
    sources.push(ProjectedAccessSourceV1 {
        operation,
        access,
        source,
    });
    Ok(())
}

fn static_array_extent(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<u64, ProductionRankedProjectionErrorV1> {
    match types.get(ty.index() as usize).map(|ty| ty.shape()) {
        Some(SemanticTypeShapeV1::Array { length, .. }) => Ok(*length),
        Some(SemanticTypeShapeV1::Slice { .. }) => {
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a slice access before dynamic extent projection is available",
            ))
        }
        _ => Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index projection whose base is not an array or slice",
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
