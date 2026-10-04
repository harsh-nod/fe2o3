// Only the existing authenticated typed scalar ISA declaration is modeled here.
// No textual inline assembly, physical helper scratch, or caller-declared effects.
impl Frame<'_, '_, '_> {
    fn inline_u32_call(
        &mut self,
        call: &SemanticDirectCallV1,
        binding: &fe2o3_mir_model::semantic_mir_v1::SemanticNonBodyCallableBindingV1,
        assembly: fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineU32V30,
    ) -> Result<SemanticBlockIdV1, Error> {
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticGfx942InlineInstructionV30 as I, SemanticGfx942InlineU32V30,
        };
        self.meter.work(32)?;
        let source = call
            .inline_assembly_source_v30()
            .ok_or("helper typed ISA lacks exact source occurrence")?;
        if source.function() != self.function.identity()
            || [
                source.frontend_unit(),
                *source.function().as_bytes(),
                source.contract(),
                source.statement(),
            ]
            .contains(&[0; 32])
            || call.ordered_region_source_v31().is_some()
            || call.ordered_program_source_v32().is_some()
            || assembly.option_bits() != SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
        {
            return Err("helper typed ISA source or effect contract differs");
        }
        let signature = binding.abi();
        abi(self.types, signature, self.meter)?;
        let destination = call.destination().ok_or("helper typed ISA has no result")?;
        let ty = signature.source_output_type();
        let word = Scalar::Integer {
            signed: false,
            bits: 32,
        };
        if self
            .types
            .get(ty.index() as usize)
            .is_none_or(|declaration| {
                declaration.rust_type_kind()
                    != fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Ordinary
            })
            || scalar(self.types, ty)? != word
            || call.arguments().len() != assembly.input_count()
            || signature.source_input_types().len() != assembly.input_count()
            || signature
                .source_input_types()
                .iter()
                .any(|input| *input != ty)
            || call.arguments().iter().any(|argument| argument.ty() != ty)
            || destination.place().ty() != ty
            || !destination.place().projections().is_empty()
            || !call.variadic_argument_abis().is_empty()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Unreachable | SemanticUnwindActionV1::Continue
            )
        {
            return Err("helper typed ISA requires exact ordered u32 operands and direct return");
        }
        // Evaluate in source order; self.operand preserves moves and initialization.
        let first = self.operand(&call.arguments()[0])?;
        let value = match assembly.instruction() {
            I::VMovB32 => {
                if assembly.input_count() != 1 {
                    return Err("helper typed ISA move arity differs");
                }
                first
            }
            instruction => {
                if assembly.input_count() != 2 {
                    return Err("helper typed ISA binary arity differs");
                }
                let second = self.operand(&call.arguments()[1])?;
                let operation = match instruction {
                    I::VAddU32 => ProductionSemanticBinaryOpV2::Add,
                    I::VSubU32 => ProductionSemanticBinaryOpV2::Subtract,
                    I::VAndB32 => ProductionSemanticBinaryOpV2::BitAnd,
                    I::VOrB32 => ProductionSemanticBinaryOpV2::BitOr,
                    I::VXorB32 => ProductionSemanticBinaryOpV2::BitXor,
                    I::VMovB32 => unreachable!(),
                };
                self.output.push(
                    word,
                    Kind::Binary(
                        operation,
                        ProductionOverflowContractV2::Wrapping,
                        first,
                        second,
                    ),
                    self.meter,
                )?
            }
        };
        self.write(destination.place(), Slot::Scalar(value))?;
        Ok(destination.edge().target())
    }
}
