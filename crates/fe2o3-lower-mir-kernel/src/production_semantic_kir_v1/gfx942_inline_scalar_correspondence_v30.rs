//! Closed root-local ISA values inside the existing independently checked relation.
//!
//! The index is not source authentication or executable IR. Construction derives
//! KIR from retained replay-checked source/SSA custody, including immutable
//! pre-ranked materialization. Explicit subsequent owner verification compares
//! the complete rederived module, canonical bytes, and correspondence BEFORE
//! revalidating this relation. No entry point accepts an externally replaced
//! module. That exact custody/replay, not mathematical value equality, rejects
//! substitution by a distinct SSA value with coincidentally equal contents.

use super::*;
use fe2o3_kernel_ir::{
    AssemblyOption, AssemblySourceIdentity, Gfx942InlineAssemblyInstructionV1 as Instruction,
    ValidatedGfx942InlineAssemblyV1, validate_gfx942_inline_assembly_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx942InlineInstructionV30 as SourceInstruction, SemanticGfx942InlineU32V30,
};

type Error = ProductionMirPlironTranslationErrorV1;

/// Borrowed checked views of the original instructions, never a replacement graph.
pub(super) struct Gfx942InlineScalarCorrespondenceV30<'a> {
    values: BTreeMap<ValueId, (&'a Operation, ValidatedGfx942InlineAssemblyV1)>,
}

impl<'a> Gfx942InlineScalarCorrespondenceV30<'a> {
    pub(super) fn empty() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Build once outside expression recursion. Scans and index storage are
    /// charged before work; all instruction source occurrences must match,
    /// including instructions whose results are not scalar write roots.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        semantic: Option<&AdmittedInertSemanticMirV1>,
        correspondence: &SemanticKirCorrespondenceV1,
        owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        function: &'a Function,
        kir: &KirCorrelationIndexV1<'a>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<Self, Error> {
        let mut actual = BTreeMap::new();
        for (location, operation) in &kir.operations {
            charge(budget, 1)?;
            if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                charge_index(budget, actual.len(), 1)?;
                actual.insert(*location, *operation);
            }
        }
        let Some(semantic) = semantic else {
            return match actual.keys().next() {
                Some(location) => Err(mismatch(*location)),
                None => Ok(Self::empty()),
            };
        };
        charge(budget, 4)?;
        let source_function = semantic
            .functions()
            .get(semantic_function.index() as usize)
            .ok_or(Error::KernelShape)?;
        let mut source_calls = BTreeMap::new();
        for (block, contents) in source_function.blocks().iter().enumerate() {
            charge(budget, 2)?;
            let SemanticTerminatorKindV1::Call(call) = contents.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(instruction),
                binding,
                ..
            }) = semantic.callables().get(call.callee().index() as usize)
            else {
                continue;
            };
            charge_index(budget, source_calls.len(), 1)?;
            source_calls.insert(
                u32::try_from(block).map_err(|_| Error::ResourceLimit)?,
                (call, *instruction, binding.abi()),
            );
        }
        if actual.is_empty() && source_calls.is_empty() {
            return Ok(Self::empty());
        }
        // This milestone deliberately excludes interprocedural scalar values.
        if source_function.role() != SemanticFunctionRoleV1::KernelRoot
            || owner != semantic_function
            || source_calls.len() != actual.len()
        {
            return Err(Error::KernelShape);
        }
        charge(budget, semantic.roots().len())?;
        if !semantic.roots().contains(&semantic_function) {
            return Err(Error::KernelShape);
        }
        let mut spans = BTreeMap::new();
        for span in correspondence.terminator_operation_spans() {
            charge(budget, 1)?;
            if span.correspondence_owner() == owner && span.semantic_function() == semantic_function
            {
                charge_index(budget, spans.len(), 1)?;
                if spans.insert(span.semantic_block().index(), span).is_some() {
                    return Err(Error::KernelShape);
                }
            }
        }
        let types = complete_value_types(function, budget)?;
        let mut result = Self::empty();
        let mut statements = BTreeSet::new();
        for (block, (call, instruction, abi)) in source_calls {
            charge_index(budget, spans.len(), 1)?;
            let span = spans.get(&block).ok_or(Error::KernelShape)?;
            let location = FunctionOperationLocation::new(
                span.kernel_ir_block(),
                span.first_operation_ordinal() as usize,
            );
            let fail = || mismatch(location);
            // Fixed-width source IDs + bounded instruction/operand validation.
            charge(budget, 192)?;
            let source = call.inline_assembly_source_v30().ok_or_else(fail)?;
            let source = AssemblySourceIdentity::new(
                source.frontend_unit(),
                *source.function().as_bytes(),
                source.contract(),
                source.statement(),
            );
            let destination = call.destination().ok_or_else(fail)?;
            if source.function != *source_function.identity().as_bytes()
                || !source.is_complete()
                || instruction.option_bits() != SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
                || abi.can_unwind()
                || abi.c_variadic()
                || abi.source_input_types().len() != instruction.input_count()
                || abi
                    .source_input_types()
                    .iter()
                    .any(|ty| *ty != destination.place().ty())
                || abi.source_output_type() != destination.place().ty()
                || !matches!(
                    call.unwind(),
                    SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
                )
                || !call.variadic_argument_abis().is_empty()
                || !destination.place().projections().is_empty()
                || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                || call.arguments().len() != instruction.input_count()
                || !is_u32(semantic.types(), destination.place().ty())
                || call
                    .arguments()
                    .iter()
                    .any(|argument| argument.ty() != destination.place().ty())
            {
                return Err(fail());
            }
            charge_index(budget, statements.len(), 32)?;
            if !statements.insert(source.statement) {
                return Err(fail());
            }
            charge_index(budget, kir.blocks.len(), 1)?;
            let operations = kir.blocks.get(&span.kernel_ir_block()).ok_or_else(fail)?;
            let start = usize::try_from(span.first_operation_ordinal())
                .map_err(|_| Error::ResourceLimit)?;
            let end = start
                .checked_add(span.operation_count() as usize)
                .ok_or(Error::ResourceLimit)?;
            let operations = operations.get(start..end).ok_or_else(fail)?;
            let mut matched = None;
            for (offset, operation) in operations.iter().enumerate() {
                charge(budget, 1)?;
                if !matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                    continue;
                }
                let current =
                    FunctionOperationLocation::new(span.kernel_ir_block(), start + offset);
                if matched.is_some() {
                    return Err(mismatch(current));
                }
                charge_index(budget, actual.len(), 1)?;
                if actual
                    .remove(&current)
                    .is_none_or(|retained| !std::ptr::eq(retained, operation))
                {
                    return Err(mismatch(current));
                }
                let OperationKind::InlineAssembly(assembly) = &operation.kind else {
                    unreachable!()
                };
                if assembly.source != source
                    || assembly.mnemonic != instruction.instruction().mnemonic()
                    || assembly.options.len() != 1
                    || !assembly.options.contains(&AssemblyOption::NoMemory)
                    || !assembly.declared_effects.is_empty()
                    || assembly.operands.len() != instruction.input_count() + 1
                {
                    return Err(mismatch(current));
                }
                charge_index(budget, types.len(), instruction.input_count())?;
                let validated = validate_gfx942_inline_assembly_v1(operation, |value| {
                    types.get(&value).and_then(|ty| ty.as_scalar())
                })
                .map_err(|_| mismatch(current))?;
                if validated.scalar_type() != ScalarType::U32
                    || validated.instruction() != instruction_kind(instruction.instruction())
                {
                    return Err(mismatch(current));
                }
                matched = Some((operation, validated));
            }
            let (operation, validated) = matched.ok_or_else(fail)?;
            charge_index(budget, result.values.len(), 1)?;
            if result
                .values
                .insert(validated.result(), (operation, validated))
                .is_some()
            {
                return Err(fail());
            }
        }
        if let Some(location) = actual.keys().next() {
            return Err(mismatch(*location));
        }
        Ok(result)
    }

    pub(super) fn normalize(
        &self,
        operation: &Operation,
        value: ValueId,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
        mut recurse: impl FnMut(
            ValueId,
            &mut UnsupportedIndexCorrelationBudgetV1,
        ) -> Option<NormalizedScalarExpressionV1>,
    ) -> Option<NormalizedScalarExpressionV1> {
        charge_index(budget, self.values.len(), 1).ok()?;
        let (retained, validated) = self.values.get(&value)?;
        if !std::ptr::eq(*retained, operation) {
            return None;
        }
        let lhs = recurse(validated.inputs()[0], budget)?;
        let operation = match validated.instruction() {
            Instruction::VMovB32 => return Some(lhs),
            Instruction::VAddU32 => ProductionSemanticBinaryOpV2::Add,
            Instruction::VSubU32 => ProductionSemanticBinaryOpV2::Subtract,
            Instruction::VAndB32 => ProductionSemanticBinaryOpV2::BitAnd,
            Instruction::VOrB32 => ProductionSemanticBinaryOpV2::BitOr,
            Instruction::VXorB32 => ProductionSemanticBinaryOpV2::BitXor,
            Instruction::SMovB32 => return None,
        };
        let rhs = recurse(validated.inputs()[1], budget)?;
        Some(NormalizedScalarExpressionV1::Binary {
            operation,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            // This is the ISA's modular value, NOT plain partial KIR arithmetic.
            // Nested operand expressions keep their original overflow/domain contracts.
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }
}

fn complete_value_types<'a>(
    function: &'a Function,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Result<BTreeMap<ValueId, &'a Type>, Error> {
    let body = function.body.as_ref().ok_or(Error::KernelShape)?;
    if body.parameters.len() != function.signature.parameters.len() {
        return Err(Error::KernelShape);
    }
    let mut types = BTreeMap::new();
    let mut insert = |value, ty, budget: &mut UnsupportedIndexCorrelationBudgetV1| {
        charge_index(budget, types.len(), 1)?;
        if types.insert(value, ty).is_some() {
            return Err(Error::KernelShape);
        }
        Ok(())
    };
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        insert(*value, ty, budget)?;
    }
    for block in &body.blocks {
        // Charge even empty blocks/operations before their traversal.
        // The existing KIR inventory already enforces its module operation limit.
        charge(budget, 1)?;
        for value in &block.parameters {
            insert(value.id, &value.ty, budget)?;
        }
        for operation in &block.operations {
            charge(budget, 1)?;
            for value in &operation.results {
                insert(value.id, &value.ty, budget)?;
            }
        }
    }
    Ok(types)
}

fn is_u32(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    )
}

const fn instruction_kind(instruction: SourceInstruction) -> Instruction {
    match instruction {
        SourceInstruction::VMovB32 => Instruction::VMovB32,
        SourceInstruction::VAddU32 => Instruction::VAddU32,
        SourceInstruction::VSubU32 => Instruction::VSubU32,
        SourceInstruction::VAndB32 => Instruction::VAndB32,
        SourceInstruction::VOrB32 => Instruction::VOrB32,
        SourceInstruction::VXorB32 => Instruction::VXorB32,
    }
}

fn mismatch(location: FunctionOperationLocation) -> Error {
    Error::ValueExpressionMismatch { location }
}

fn charge(budget: &mut UnsupportedIndexCorrelationBudgetV1, amount: usize) -> Result<(), Error> {
    budget.remaining = budget
        .remaining
        .checked_sub(amount)
        .ok_or(Error::ResourceLimit)?;
    Ok(())
}

fn charge_index(
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
    count: usize,
    width: usize,
) -> Result<(), Error> {
    let levels = usize::BITS as usize - count.leading_zeros() as usize + 1;
    charge(
        budget,
        levels
            .checked_mul(width)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::ResourceLimit)?,
    )
}
