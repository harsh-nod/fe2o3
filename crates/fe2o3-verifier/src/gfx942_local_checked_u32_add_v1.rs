//! Conditional, single-operation simulation coordinates for checked `u32` addition.
//!
//! The retained V4 owner authenticates its embedded source-side receipt, not compiler origin.
//! Its operation spans do not retain loop-local-to-SSA bindings. This checker therefore leaves
//! both MIR-local/KIR-SSA and KIR-SSA/physical-register equality explicitly unresolved. Under
//! those entry equalities, the extracted additions have the same wrapping value and overflow
//! bit according to the closed encoded S_ADD_U32 model. CFG, ABI, payload lineage, hardware ISA
//! correspondence and result continuation remain separate obligations. This is not authority.

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, Gfx942IntegerSemanticsErrorV1,
    Gfx942MachineDataflowErrorV1, Gfx942MachineDataflowV1, Gfx942ReachingDefinitionV1,
    Gfx942RegisterUnitV1, Gfx942SAddU32V1, Gfx942U32SourceV1, PhysicalMachineAnalysisEvidenceV1,
    PhysicalMachineEffectRequestV1, analyze_control_flow,
};
use fe2o3_kernel_ir::{
    BinaryOp, CheckedBinaryOperator, Constant, Function, KernelIrDecodeError, Module,
    OperationKind, ScalarType, Type, ValueId, decode_module_v8,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCheckedBinaryOpV1, SemanticConstantValueV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};
use std::{error::Error, fmt};

use crate::{ValidatedCompilerProofInputsV4, VerifiedSemanticU32InductionKirAnchorV1};

mod mov_prefix;
pub use mov_prefix::{
    Gfx942LocalMovPrefixCheckedU32AddErrorV1, Gfx942LocalMovPrefixCheckedU32AddObligationV1,
    check_gfx942_local_mov_prefix_checked_u32_add_v1,
};

/// An equality the checker does NOT discharge. The coordinates are extracted, not caller values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnresolvedCheckedU32AddEntryRelationV1 {
    pub semantic_function: u32,
    pub semantic_local: u32,
    pub kernel_ir_value: u32,
    pub machine_source_sgpr: u8,
    pub machine_instruction_offset: u64,
}

/// Conditional result correspondence; tuple field zero is value and field one is overflow.
/// The machine result is the destination SGPR and SCC. No continuation binding is discharged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalCheckedU32AddResultsV1 {
    pub semantic_result_local: u32,
    pub kernel_ir_value: u32,
    pub kernel_ir_overflow: u32,
    pub machine_destination_sgpr: u8,
}

/// A borrowed obligation, inseparable from the exact input owners during its lifetime.
///
/// There is intentionally no accepted/refined bit, serialization, runtime-value witness or
/// authority conversion. Physical reaching definitions are provenance, not source bindings.
#[derive(Debug)]
pub struct Gfx942LocalCheckedU32AddObligationV1<'a> {
    inputs: &'a ValidatedCompilerProofInputsV4,
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    coordinates: LocalCoordinates,
}

impl Gfx942LocalCheckedU32AddObligationV1<'_> {
    pub const fn inputs(&self) -> &ValidatedCompilerProofInputsV4 {
        self.inputs
    }

    pub const fn execution(&self) -> &AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }

    pub const fn anchor(&self) -> VerifiedSemanticU32InductionKirAnchorV1 {
        self.coordinates.source.anchor
    }

    pub fn function_symbol(&self) -> &str {
        &self.coordinates.source.function
    }

    pub const fn literal(&self) -> u32 {
        self.coordinates.source.literal
    }

    /// Both semantic-local == KIR-value and KIR-value == SGPR remain hypotheses.
    pub const fn unresolved_entry_relation(&self) -> UnresolvedCheckedU32AddEntryRelationV1 {
        self.coordinates.entry
    }

    pub const fn conditional_results(&self) -> ConditionalCheckedU32AddResultsV1 {
        self.coordinates.results
    }

    pub const fn machine_input_definition(&self) -> Gfx942ReachingDefinitionV1 {
        self.coordinates.definition
    }

    pub const fn machine_operation(&self) -> &Gfx942SAddU32V1 {
        &self.coordinates.machine
    }
}

/// Checks actual operands and coordinates without asserting a missing inter-stage state relation.
///
/// This first profile accepts an unprojected MIR local plus a u32 literal, a dominating-block
/// KIR parameter plus that exact literal, and a regular SGPR plus that encoded literal. KIR
/// operand normalization, multiple reaching definitions and unsupported physical definitions reject.
pub fn check_gfx942_local_checked_u32_add_v1<'a>(
    inputs: &'a ValidatedCompilerProofInputsV4,
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    anchor_index: usize,
    instruction_offset: u64,
) -> Result<Gfx942LocalCheckedU32AddObligationV1<'a>, Gfx942LocalCheckedU32AddErrorV1> {
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes())
        .map_err(Gfx942LocalCheckedU32AddErrorV1::KernelIr)?;
    let source = source_coordinates(inputs, &module, anchor_index)?;
    let coordinates = machine_coordinates(
        source,
        execution.request(),
        execution.analysis(),
        instruction_offset,
    )?;
    Ok(Gfx942LocalCheckedU32AddObligationV1 {
        inputs,
        execution,
        coordinates,
    })
}

#[derive(Debug)]
struct SourceCoordinates {
    anchor: VerifiedSemanticU32InductionKirAnchorV1,
    function: String,
    local: u32,
    result_local: u32,
    lhs: u32,
    literal: u32,
}

#[derive(Debug)]
struct LocalCoordinates {
    source: SourceCoordinates,
    machine: Gfx942SAddU32V1,
    entry: UnresolvedCheckedU32AddEntryRelationV1,
    results: ConditionalCheckedU32AddResultsV1,
    definition: Gfx942ReachingDefinitionV1,
}

fn source_coordinates(
    inputs: &ValidatedCompilerProofInputsV4,
    module: &Module,
    anchor_index: usize,
) -> Result<SourceCoordinates, Gfx942LocalCheckedU32AddErrorV1> {
    use Gfx942LocalCheckedU32AddErrorV1 as E;
    let anchor = *inputs
        .semantic_u32_induction_kir_anchors()
        .get(anchor_index)
        .ok_or(E::Anchor)?;
    if inputs
        .semantic_u32_induction_kir_anchors()
        .iter()
        .filter(|other| **other == anchor)
        .count()
        != 1
    {
        return Err(E::Anchor);
    }
    let semantic = inputs
        .semantic_mir()
        .functions()
        .get(anchor.semantic_function() as usize)
        .ok_or(E::Anchor)?;
    let statement = semantic
        .blocks()
        .get(anchor.semantic_block() as usize)
        .and_then(|block| block.statements().get(anchor.semantic_statement() as usize))
        .ok_or(E::Anchor)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(E::SourceShape);
    };
    let SemanticRvalueKindV1::CheckedBinary(add) = assignment.value().kind() else {
        return Err(E::SourceShape);
    };
    let SemanticOperandV1::Copy(local) = add.left() else {
        return Err(E::SourceShape);
    };
    let SemanticOperandV1::Constant(constant) = add.right() else {
        return Err(E::SourceShape);
    };
    if add.operation() != SemanticCheckedBinaryOpV1::Add
        || !local.projections().is_empty()
        || !assignment.destination().projections().is_empty()
        || !semantic_u32(inputs, local.ty())
        || !semantic_u32(inputs, constant.ty())
        || semantic
            .locals()
            .get(local.local().index() as usize)
            .is_none_or(|decl| decl.ty() != local.ty())
    {
        return Err(E::SourceShape);
    }
    let SemanticConstantValueV1::Scalar(literal) = constant.value() else {
        return Err(E::SourceShape);
    };
    let literal = u32::try_from(literal.bits())
        .ok()
        .filter(|_| literal.size_bytes() == 4)
        .ok_or(E::SourceShape)?;
    let result_ty = assignment.destination().ty();
    if semantic
        .locals()
        .get(assignment.destination().local().index() as usize)
        .is_none_or(|decl| decl.ty() != result_ty)
    {
        return Err(E::SourceShape);
    }
    let Some(SemanticTypeShapeV1::Tuple(tuple)) = inputs
        .semantic_mir()
        .types()
        .get(result_ty.index() as usize)
        .map(|ty| ty.shape())
    else {
        return Err(E::SourceShape);
    };
    let [value_ty, overflow_ty] = tuple.fields() else {
        return Err(E::SourceShape);
    };
    if !semantic_u32(inputs, *value_ty)
        || !matches!(
            inputs
                .semantic_mir()
                .types()
                .get(overflow_ty.index() as usize)
                .map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
    {
        return Err(E::SourceShape);
    }

    // V4 associates groups of semantic block records with defined KIR functions in order.
    // It does not identify a local with a numeric SSA ID; that equality stays unresolved.
    let functions = inputs
        .correspondence()
        .blocks()
        .iter()
        .map(|block| block.semantic_function())
        .collect::<std::collections::BTreeSet<_>>();
    let ordinal = functions
        .iter()
        .position(|function| *function == anchor.semantic_function())
        .ok_or(E::Anchor)?;
    let function = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .nth(ordinal)
        .ok_or(E::Anchor)?;
    let body = function.body.as_ref().ok_or(E::Anchor)?;
    let block = body
        .blocks
        .iter()
        .find(|block| block.id.0 == anchor.kernel_ir_block())
        .ok_or(E::Anchor)?;
    let operation = block
        .operations
        .get(anchor.kernel_ir_operation() as usize)
        .ok_or(E::Anchor)?;
    let OperationKind::Binary {
        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
        lhs,
        rhs,
    } = operation.kind
    else {
        return Err(E::KernelShape);
    };
    let [value, overflow] = operation.results.as_slice() else {
        return Err(E::KernelShape);
    };
    if value.id.0 != anchor.value_result()
        || overflow.id.0 != anchor.overflow_result()
        || value.ty != Type::Scalar(ScalarType::U32)
        || overflow.ty != Type::Scalar(ScalarType::Bool)
    {
        return Err(E::KernelShape);
    }
    if definition_count(function, lhs) != 1
        || definition_count(function, rhs) != 1
        || definition_count(function, value.id) != 1
        || definition_count(function, overflow.id) != 1
    {
        return Err(E::AmbiguousKernelDefinition);
    }
    let definition_block = body
        .blocks
        .iter()
        .find(|candidate| {
            candidate.parameters.iter().any(|parameter| {
                parameter.id == lhs && parameter.ty == Type::Scalar(ScalarType::U32)
            })
        })
        .ok_or(E::UnsupportedKernelOperand)?;
    // Pruned SSA can reuse a loop-header parameter in a dominated body block. This proves
    // its KIR availability, not the unresolved equality with the semantic MIR local.
    if !analyze_control_flow(function)
        .map_err(|_| E::UnsupportedKernelOperand)?
        .dominates(definition_block.id, block.id)
    {
        return Err(E::UnsupportedKernelOperand);
    }
    let spans = inputs
        .correspondence()
        .statement_spans()
        .iter()
        .filter(|span| {
            span.semantic_function() == anchor.semantic_function()
                && span.semantic_block() == anchor.semantic_block()
                && span.statement() == anchor.semantic_statement()
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        return Err(E::Anchor);
    };
    // No unmodeled normalization, extra effects or hidden constant producer in this profile.
    if span.kernel_ir_block() != block.id.0
        || span.operation_count() != 2
        || span.first_operation().checked_add(1) != Some(anchor.kernel_ir_operation())
    {
        return Err(E::UnsupportedKernelOperand);
    }
    let constant = block
        .operations
        .get(span.first_operation() as usize)
        .ok_or(E::Anchor)?;
    if constant.results.len() != 1
        || constant.results[0].id != rhs
        || constant.results[0].ty != Type::Scalar(ScalarType::U32)
        || constant.kind != OperationKind::Constant(Constant::U32(literal))
    {
        return Err(E::LiteralMismatch);
    }
    Ok(SourceCoordinates {
        anchor,
        function: function.id.as_str().to_owned(),
        local: local.local().index(),
        result_local: assignment.destination().local().index(),
        lhs: lhs.0,
        literal,
    })
}

fn semantic_u32(inputs: &ValidatedCompilerProofInputsV4, ty: SemanticTypeIdV1) -> bool {
    matches!(
        inputs
            .semantic_mir()
            .types()
            .get(ty.index() as usize)
            .map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    )
}

fn definition_count(function: &Function, value: ValueId) -> usize {
    let Some(body) = &function.body else { return 0 };
    body.parameters
        .iter()
        .filter(|parameter| **parameter == value)
        .count()
        + body
            .blocks
            .iter()
            .flat_map(|block| &block.parameters)
            .filter(|parameter| parameter.id == value)
            .count()
        + body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(|operation| &operation.results)
            .filter(|result| result.id == value)
            .count()
}

fn machine_coordinates(
    source: SourceCoordinates,
    request: &PhysicalMachineEffectRequestV1,
    analysis: &PhysicalMachineAnalysisEvidenceV1,
    offset: u64,
) -> Result<LocalCoordinates, Gfx942LocalCheckedU32AddErrorV1> {
    use Gfx942LocalCheckedU32AddErrorV1 as E;
    if analysis.effects().request_identity() != request.identity()
        || analysis.effects().payload_identity() != request.payload_identity()
        || request
            .entries()
            .iter()
            .filter(|entry| entry.symbol() == source.function)
            .count()
            != 1
    {
        return Err(E::MachineBinding);
    }
    let trace = analysis.trace();
    let instructions = trace
        .instructions()
        .iter()
        .filter(|instruction| {
            instruction.function_symbol() == source.function
                && instruction.instruction_offset() == offset
        })
        .collect::<Vec<_>>();
    let [instruction] = instructions.as_slice() else {
        return Err(E::MachineBinding);
    };
    let machine = Gfx942SAddU32V1::decode(instruction).map_err(E::MachineInstruction)?;
    let register = match machine.sources() {
        [
            Gfx942U32SourceV1::Sgpr(register),
            Gfx942U32SourceV1::Constant(literal),
        ]
        | [
            Gfx942U32SourceV1::Constant(literal),
            Gfx942U32SourceV1::Sgpr(register),
        ] if literal == source.literal => register,
        _ => return Err(E::LiteralMismatch),
    };
    let dataflow = Gfx942MachineDataflowV1::derive(trace).map_err(E::Dataflow)?;
    let definitions = dataflow
        .reaching_definitions_before(
            &source.function,
            offset,
            Gfx942RegisterUnitV1::Sgpr(u16::from(register)),
        )
        .map_err(E::Dataflow)?;
    let [definition] = definitions.as_slice() else {
        return Err(E::AmbiguousMachineDefinition);
    };
    if let Gfx942ReachingDefinitionV1::Instruction {
        offset: definition_offset,
    } = *definition
    {
        let definition = trace
            .instructions()
            .iter()
            .find(|instruction| {
                instruction.function_symbol() == source.function
                    && instruction.instruction_offset() == definition_offset
            })
            .ok_or(E::MachineBinding)?;
        let decoded =
            Gfx942SAddU32V1::decode(definition).map_err(|_| E::UnsupportedMachineDefinition)?;
        if decoded.destination() != register
            || !dataflow
                .instruction_dominates(&source.function, definition_offset, offset)
                .map_err(E::Dataflow)?
        {
            return Err(E::UnsupportedMachineDefinition);
        }
    }
    let entry = UnresolvedCheckedU32AddEntryRelationV1 {
        semantic_function: source.anchor.semantic_function(),
        semantic_local: source.local,
        kernel_ir_value: source.lhs,
        machine_source_sgpr: register,
        machine_instruction_offset: offset,
    };
    let results = ConditionalCheckedU32AddResultsV1 {
        semantic_result_local: source.result_local,
        kernel_ir_value: source.anchor.value_result(),
        kernel_ir_overflow: source.anchor.overflow_result(),
        machine_destination_sgpr: machine.destination(),
    };
    Ok(LocalCoordinates {
        source,
        machine,
        entry,
        results,
        definition: *definition,
    })
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942LocalCheckedU32AddErrorV1 {
    Anchor,
    SourceShape,
    KernelIr(KernelIrDecodeError),
    KernelShape,
    AmbiguousKernelDefinition,
    UnsupportedKernelOperand,
    LiteralMismatch,
    MachineBinding,
    MachineInstruction(Gfx942IntegerSemanticsErrorV1),
    Dataflow(Gfx942MachineDataflowErrorV1),
    AmbiguousMachineDefinition,
    UnsupportedMachineDefinition,
}

impl fmt::Display for Gfx942LocalCheckedU32AddErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported local checked-u32-add obligation: {self:?}"
        )
    }
}

impl Error for Gfx942LocalCheckedU32AddErrorV1 {}

#[cfg(test)]
mod tests;
