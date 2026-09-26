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
    SemanticDirectCallV1, SemanticFunctionAbiV1,
    SemanticGfx942InlineInstructionV30 as SourceInstruction, SemanticGfx942InlineU32V30,
};

type Error = ProductionMirPlironTranslationErrorV1;

/// Borrowed checked views of the original instructions, never a replacement graph.
pub(super) struct Gfx942InlineScalarCorrespondenceV30<'a> {
    values: InlineScalarValuesV18<'a>,
}

enum InlineScalarValuesV18<'a> {
    Legacy(BTreeMap<ValueId, (&'a Operation, ValidatedGfx942InlineAssemblyV1)>),
    Source(Vec<(ValueId, &'a Operation, ValidatedGfx942InlineAssemblyV1)>),
}

impl<'a> Gfx942InlineScalarCorrespondenceV30<'a> {
    pub(super) fn empty() -> Self {
        Self {
            values: InlineScalarValuesV18::Legacy(BTreeMap::new()),
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
        budget: &mut dyn CorrelationChargeV18,
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
        let mut values = BTreeMap::new();
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
            let source = checked_source_instruction(semantic, source_function, call, instruction, abi)
                .ok_or_else(fail)?;
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
                charge_index(budget, types.len(), instruction.input_count())?;
                let validated = checked_physical_instruction(operation, source, instruction, |value| {
                    types.get(&value).and_then(|ty| ty.as_scalar())
                })
                .ok_or_else(|| mismatch(current))?;
                matched = Some((operation, validated));
            }
            let (operation, validated) = matched.ok_or_else(fail)?;
            charge_index(budget, values.len(), 1)?;
            if values
                .insert(validated.result(), (operation, validated))
                .is_some()
            {
                return Err(fail());
            }
        }
        if let Some(location) = actual.keys().next() {
            return Err(mismatch(*location));
        }
        Ok(Self { values: InlineScalarValuesV18::Legacy(values) })
    }

    // Only source-occurrence metadata is indexed. Every operation and input
    // type is borrowed from the existing canonical inventory, never rebuilt.
    pub(super) fn build_source_v18(
        relation: &ProductionSourceCorrespondenceV18<'a>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        relation.retain_query((|| {
            relation.query(budget)?;
            let source = relation.source.source_semantic(budget)?;
            let physical = relation.source.root(root, budget)?.1;
            let function = relation.inventory.functions().get(physical)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar physical root"))?;
            let operations = relation.inventory.operations().get(function.operations.clone())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar operation interval"))?;
            let mut count = 0usize;
            for operation in operations {
                budget.charge_work(1)?;
                if matches!(operation.operation.kind, OperationKind::InlineAssembly(_)) {
                    count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Self>(),
                std::mem::size_of::<Vec<(usize, [u8; 32])>>(),
                std::mem::size_of::<[(ValueId, Option<ScalarType>); 2]>(),
            ])?)?;
            let mut values = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
            let mut statements = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
            for instance in 0..relation.source.instance_count(root, budget)? {
                let (original, _) = relation.source.instance(root, instance, budget)?;
                budget.charge_work(1)?;
                let declaration = source.functions().get(original.index() as usize)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar original function"))?;
                for (block, contents) in declaration.blocks().iter().enumerate() {
                    budget.charge_work(2)?;
                    let SemanticTerminatorKindV1::Call(call) = contents.terminator().kind() else { continue; };
                    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(instruction),
                        binding, ..
                    }) = source.callables().get(call.callee().index() as usize) else { continue; };
                    budget.charge_work(192)?;
                    let identity = checked_source_instruction(source, declaration, call, *instruction, binding.abi())
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar original instruction"))?;
                    if values.len() == values.capacity() || statements.len() == statements.capacity() {
                        return relation.source.missing("inline scalar source exceeds actual instruction census");
                    }
                    let block = SemanticBlockIdV1::from_index(u32::try_from(block)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?);
                    let rows = relation.source_operation_rows(root, instance, block, None, budget)?;
                    let mut matched = None;
                    for row in rows {
                        let ProductionSourceOperationV18::Operation(coordinate) =
                            relation.mapped_source_operation(row.location, budget)? else { continue; };
                        budget.charge_work(4)?;
                        if coordinate.block.function != function.coordinate {
                            return relation.source.missing("inline scalar source changed physical root");
                        }
                        let actual = function.function.body.as_ref()
                            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
                            .and_then(|block| block.operations.get(coordinate.operation as usize))
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar actual operation"))?;
                        if !matches!(actual.kind, OperationKind::InlineAssembly(_)) { continue; }
                        if matched.is_some() {
                            return relation.source.missing("inline scalar source has multiple actual instructions");
                        }
                        let mut inputs = [(ValueId(0), None); 2];
                        let mut used = 0usize;
                        actual.kind.try_visit_operands(|value| -> SourceOwnedResultV18<()> {
                            budget.charge_work(2)?;
                            let slot = inputs.get_mut(used).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "inline scalar operand census"))?;
                            let definition = relation.inventory.definition_for_value(function.coordinate, value, budget)
                                .map_err(source_pointer_inventory_error_v18)?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar input definition"))?;
                            *slot = (value, definition.ty.as_scalar());
                            used += 1;
                            Ok(())
                        })?;
                        if used != instruction.input_count() {
                            return relation.source.missing("inline scalar input count changed");
                        }
                        budget.charge_work(192)?;
                        let validated = checked_physical_instruction(actual, identity, *instruction, |value| {
                            inputs[..used].iter().find(|row| row.0 == value).and_then(|row| row.1)
                        }).ok_or(ProductionSourceOwnedViewErrorV18::Binding("inline scalar physical instruction"))?;
                        matched = Some((validated.result(), actual, validated));
                    }
                    let value = matched.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "inline scalar source has no actual instruction"))?;
                    budget.charge_work(2)?;
                    values.push(value);
                    statements.push((instance, identity.statement));
                }
            }
            if values.len() != count { return relation.source.missing("inline scalar instruction census is incomplete"); }
            private_array_heapsort_v1(&mut values, |row| [row.0.0 as usize],
                &mut SourceCorrespondenceWorkV18(budget),
                || ProductionSourceOwnedViewErrorV18::Binding("inline scalar value sort"))?;
            for pair in values.windows(2) {
                budget.charge_work(1)?;
                if pair[0].0 == pair[1].0 { return relation.source.missing("inline scalar has duplicate actual result"); }
            }
            let statement_key = |row: &(usize, [u8; 32])| -> [usize; 33] {
                std::array::from_fn(|index| if index == 0 { row.0 } else { row.1[index - 1] as usize })
            };
            private_array_heapsort_v1(&mut statements, statement_key,
                &mut SourceCorrespondenceWorkV18(budget),
                || ProductionSourceOwnedViewErrorV18::Binding("inline scalar statement sort"))?;
            for pair in statements.windows(2) {
                budget.charge_work(33)?;
                if pair[0] == pair[1] { return relation.source.missing("inline scalar repeated original statement"); }
            }
            // The containing checked scope retains this conservative credit
            // until both the temporary census and returned cache are dropped.
            drop(statements);
            Ok(Self { values: InlineScalarValuesV18::Source(values) })
        })())
    }

    pub(super) fn transport_optimized_source_v18<'out>(
        &self,
        relation: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &'out ProductionOptimizedSourceCorrespondenceV18<'out>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Gfx942InlineScalarCorrespondenceV30<'out>> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        relation.retain_query((|| {
            optimized_source_endpoints_v18(relation, optimized, budget)?;
            let InlineScalarValuesV18::Source(original) = &self.values else {
                return relation.source.missing("optimized inline source requires original source rows");
            };
            let input_function = relation.source.root(root, budget)?.1;
            let input_function = relation.inventory.functions().get(input_function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized inline input root"))?;
            let inventory = optimized.output_inventory(budget)?;
            let function = inventory.function_for_name(input_function.function.id.as_str(), budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized inline output root"))?;
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Gfx942InlineScalarCorrespondenceV30<'_>>(),
                std::mem::size_of::<[(ValueId, Option<ScalarType>); 2]>(),
            ])?)?;
            let mut values = emission_vec_v1(original.len(), budget).map_err(source_emission_error_v18)?;
            for (value, retained, validated) in original {
                budget.charge_work(4)?;
                let before = relation.inventory.definition_for_value(input_function.coordinate, *value, budget)
                    .map_err(source_pointer_inventory_error_v18)?
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized inline input definition"))?;
                let Definition::Result { operation: input, result: 0 } = before.coordinate else {
                    return relation.source.missing("optimized inline input is not its original result");
                };
                let input_row = optimized_source_operation_row_v18(relation.inventory, input, budget)?;
                if !std::ptr::eq(input_row.operation, *retained) || validated.result() != *value {
                    return relation.source.missing("optimized inline substituted original instruction");
                }
                let output = match optimized.operation(input, budget)? {
                    ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
                    ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
                    ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                        return relation.source.missing("optimized inline removed a reachable instruction");
                    }
                };
                if output.block.function != function.coordinate {
                    return relation.source.missing("optimized inline changed output root");
                }
                let actual = optimized_source_operation_row_v18(inventory, output, budget)?.operation;
                let (OperationKind::InlineAssembly(before), OperationKind::InlineAssembly(after)) =
                    (&retained.kind, &actual.kind) else {
                        return relation.source.missing("optimized inline operation class changed");
                    };
                budget.charge_work(192)?;
                if before.source != after.source {
                    return relation.source.missing("optimized inline original source identity changed");
                }
                let mut inputs = [(ValueId(0), None); 2];
                let mut used = 0usize;
                actual.kind.try_visit_operands(|value| -> SourceOwnedResultV18<()> {
                    budget.charge_work(2)?;
                    let slot = inputs.get_mut(used).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized inline operand census"))?;
                    let definition = inventory.definition_for_value(function.coordinate, value, budget)
                        .map_err(source_pointer_inventory_error_v18)?
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized inline input type"))?;
                    *slot = (value, definition.ty.as_scalar());
                    used += 1;
                    Ok(())
                })?;
                if used != validated.inputs().len() {
                    return relation.source.missing("optimized inline operand count changed");
                }
                for (operand, expected) in inputs[..used].iter().enumerate() {
                    let (_, value) = optimized_source_actual_operand_v18(relation, optimized,
                        fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                            operation: input,
                            operand: u32::try_from(operand).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        }, output, budget)?;
                    if value != expected.0 {
                        return relation.source.missing("optimized inline input is not its actual selected use");
                    }
                }
                let checked = validate_gfx942_inline_assembly_v1(actual, |value| {
                    inputs[..used].iter().find(|row| row.0 == value).and_then(|row| row.1)
                }).map_err(|_| ProductionSourceOwnedViewErrorV18::Binding("optimized inline instruction validation"))?;
                if checked.instruction() != validated.instruction() || checked.scalar_type() != validated.scalar_type() {
                    return relation.source.missing("optimized inline instruction contract changed");
                }
                let result = Definition::Result { operation: output, result: 0 };
                let mut matches = 0usize;
                for row in optimized.definition_descendants(Definition::Result { operation: input, result: 0 }, budget)? {
                    budget.charge_work(1)?;
                    if row.output == result { matches = matches.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?; }
                }
                if matches != 1 || optimized_source_definition_row_v18(inventory, result, budget)?.value != Some(checked.result()) {
                    return relation.source.missing("optimized inline result descendant changed");
                }
                if values.len() == values.capacity() { return relation.source.missing("optimized inline result census capacity"); }
                values.push((checked.result(), actual, checked));
            }
            let mut actual_count = 0usize;
            for row in inventory.operations().get(function.operations.clone())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("optimized inline operation range"))? {
                budget.charge_work(1)?;
                if matches!(row.operation.kind, OperationKind::InlineAssembly(_)) {
                    actual_count = actual_count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            if actual_count != values.len() { return relation.source.missing("optimized inline complete output census"); }
            private_array_heapsort_v1(&mut values, |row| [row.0.0 as usize],
                &mut SourceCorrespondenceWorkV18(budget),
                || ProductionSourceOwnedViewErrorV18::Binding("optimized inline value sort"))?;
            for pair in values.windows(2) {
                budget.charge_work(1)?;
                if pair[0].0 == pair[1].0 { return relation.source.missing("optimized inline duplicate output result"); }
            }
            Ok(Gfx942InlineScalarCorrespondenceV30 { values: InlineScalarValuesV18::Source(values) })
        })())
    }

    pub(super) fn normalize(
        &self,
        operation: &Operation,
        value: ValueId,
        budget: &mut dyn CorrelationChargeV18,
        mut recurse: impl FnMut(
            ValueId,
            &mut dyn CorrelationChargeV18,
        ) -> Option<NormalizedScalarExpressionV1>,
    ) -> Option<NormalizedScalarExpressionV1> {
        let (retained, validated) = match &self.values {
            InlineScalarValuesV18::Legacy(values) => {
                charge_index(budget, values.len(), 1).ok()?;
                let (retained, validated) = values.get(&value)?;
                (*retained, validated)
            }
            InlineScalarValuesV18::Source(values) => {
            let mut first = 0;
            let mut end = values.len();
            while first < end {
                charge(budget, 4).ok()?;
                let middle = first + (end - first) / 2;
                if values[middle].0 < value { first = middle + 1; }
                else { end = middle; }
            }
            charge(budget, 2).ok()?;
            let (found, retained, validated) = values.get(first)?;
            if *found != value { return None; }
            (*retained, validated)
            }
        };
        if !std::ptr::eq(retained, operation) {
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
            lhs: normalized_scalar_box_v18(lhs, budget)?,
            rhs: normalized_scalar_box_v18(rhs, budget)?,
        })
    }
}

pub(super) fn checked_source_instruction(
    semantic: &AdmittedInertSemanticMirV1,
    source_function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
    instruction: SemanticGfx942InlineU32V30,
    abi: &SemanticFunctionAbiV1,
) -> Option<AssemblySourceIdentity> {
    let source = call.inline_assembly_source_v30()?;
    let source = AssemblySourceIdentity::new(source.frontend_unit(), *source.function().as_bytes(),
        source.contract(), source.statement());
    let destination = call.destination()?;
    (source.function == *source_function.identity().as_bytes()
        && source.is_complete()
        && instruction.option_bits() == SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
        && !abi.can_unwind() && !abi.c_variadic()
        && abi.source_input_types().len() == instruction.input_count()
        && abi.source_input_types().iter().all(|ty| *ty == destination.place().ty())
        && abi.source_output_type() == destination.place().ty()
        && matches!(call.unwind(), SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable)
        && call.variadic_argument_abis().is_empty()
        && destination.place().projections().is_empty()
        && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
        && call.arguments().len() == instruction.input_count()
        && is_u32(semantic.types(), destination.place().ty())
        && call.arguments().iter().all(|argument| argument.ty() == destination.place().ty()))
        .then_some(source)
}

pub(super) fn checked_physical_instruction(
    operation: &Operation,
    source: AssemblySourceIdentity,
    instruction: SemanticGfx942InlineU32V30,
    value_type: impl Fn(ValueId) -> Option<ScalarType>,
) -> Option<ValidatedGfx942InlineAssemblyV1> {
    let OperationKind::InlineAssembly(assembly) = &operation.kind else { return None; };
    if assembly.source != source || assembly.mnemonic != instruction.instruction().mnemonic()
        || assembly.options.len() != 1 || !assembly.options.contains(&AssemblyOption::NoMemory)
        || !assembly.declared_effects.is_empty()
        || assembly.operands.len() != instruction.input_count() + 1
    { return None; }
    let validated = validate_gfx942_inline_assembly_v1(operation, value_type).ok()?;
    (validated.scalar_type() == ScalarType::U32
        && validated.instruction() == instruction_kind(instruction.instruction())).then_some(validated)
}

fn complete_value_types<'a>(
    function: &'a Function,
    budget: &mut dyn CorrelationChargeV18,
) -> Result<BTreeMap<ValueId, &'a Type>, Error> {
    let body = function.body.as_ref().ok_or(Error::KernelShape)?;
    if body.parameters.len() != function.signature.parameters.len() {
        return Err(Error::KernelShape);
    }
    let mut types = BTreeMap::new();
    let mut insert = |value, ty, budget: &mut dyn CorrelationChargeV18| {
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

fn charge(budget: &mut dyn CorrelationChargeV18, amount: usize) -> Result<(), Error> {
    budget.charge_many(amount).ok_or(Error::ResourceLimit)?;
    Ok(())
}

fn charge_index(
    budget: &mut dyn CorrelationChargeV18,
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
