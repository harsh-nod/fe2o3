// Pre-ranked materialization preserves the existing assembly operation. This
// leaf checks inert record consistency; authenticated source replay is upstream.
// It does not change generic assembly effects or final functional refinement.

impl SemanticFunctionLoweringV1<'_> {
    fn lower_gfx942_inline_u32_v30(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        assembly: fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineU32V30,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::{
            AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity,
            InlineAssembly, InlineAssemblyTarget, validate_gfx942_inline_assembly_v1,
        };
        use fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineU32V30;

        self.require_call_argument_count(block, call, assembly.input_count())?;
        let fail = |detail| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                detail,
            )
        };
        let source = call
            .inline_assembly_source_v30()
            .filter(|source| source.function() == self.function.identity())
            .ok_or_else(|| fail("typed ISA occurrence is absent or belongs to another caller"))?;
        let source = AssemblySourceIdentity::new(
            source.frontend_unit(),
            *source.function().as_bytes(),
            source.contract(),
            source.statement(),
        );
        if !source.is_complete()
            || assembly.option_bits() != SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
        {
            return Err(fail("typed ISA source identity or option contract changed"));
        }
        let destination = call
            .destination()
            .ok_or_else(|| fail("typed ISA call has no destination"))?;
        let semantic_ty = destination.place().ty();
        if lower_scalar_type(self.types, semantic_ty)? != Type::Scalar(ScalarType::U32)
            || call
                .arguments()
                .iter()
                .any(|operand| semantic_operand_type(operand) != semantic_ty)
        {
            return Err(fail(
                "typed ISA operands and result require one exact u32 type",
            ));
        }
        let mut inputs = Vec::with_capacity(assembly.input_count());
        for operand in call.arguments() {
            let (input, ty) = self
                .lower_operand(block, None, operand, operations)?
                .value()
                .map_err(|detail| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        detail,
                    )
                })?;
            if ty != Type::Scalar(ScalarType::U32) {
                return Err(unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "typed ISA lowered input is not u32",
                ));
            }
            inputs.push(input);
        }
        let mut operands = Vec::with_capacity(inputs.len() + 1);
        operands.push(AssemblyOperand::output(0, AssemblyConstraint::Vgpr32));
        operands.extend(
            inputs
                .iter()
                .map(|value| AssemblyOperand::input(*value, AssemblyConstraint::Vgpr32)),
        );
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(self.next_value), Type::Scalar(ScalarType::U32)),
            OperationKind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                source,
                mnemonic: assembly.instruction().mnemonic().to_owned(),
                operands,
                options: BTreeSet::from([AssemblyOption::NoMemory]),
                declared_effects: BTreeSet::new(),
            }),
        );
        validate_gfx942_inline_assembly_v1(&operation, |value| {
            inputs.contains(&value).then_some(ScalarType::U32)
        })
        .map_err(|_| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "typed ISA emission disagrees with the shared gfx942 contract",
            )
        })?;
        // The normal emitter retains operation budgets and SSA identity checks.
        // Assembly remains opaque to integer constant/bounds propagation.
        self.emit(operations, Type::Scalar(ScalarType::U32), operation.kind)
    }
}

#[cfg(test)]
mod gfx942_inline_v30_tests {
    use super::*;
    include!("gfx942_inline_v30_tests.rs");
}
