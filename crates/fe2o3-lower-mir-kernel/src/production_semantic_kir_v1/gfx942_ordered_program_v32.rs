// Preserve one closed authored program as one executable operation. This leaf
// repeats immutable-call/type/register checks; source authentication and target,
// launch and CFG qualification remain independent admission responsibilities.

impl SemanticFunctionLoweringV1<'_> {
    fn lower_gfx942_ordered_program_v32(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        program: fe2o3_mir_model::semantic_mir_v1::SemanticGfx942U32ProgramV32,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::{
            AssemblySourceIdentity, Gfx942OrderedProgramRegistersV1, Gfx942OrderedProgramV1,
            Gfx942U32ProgramV1, validate_gfx942_ordered_program_v1,
        };
        use fe2o3_mir_model::semantic_mir_v1::{SemanticConstantValueV1, SemanticFunctionRoleV1};

        let function_index = self.semantic_function.index();
        let fail = |detail| unsupported(function_index, Some(block.index()), None, detail);
        self.require_call_argument_count(block, call, 8)?;
        if self.function.role() != SemanticFunctionRoleV1::KernelRoot
            || self.required_workgroup != Some([64, 1, 1])
        {
            return Err(fail(
                "ordered program requires a direct root and required 64x1x1 workgroup",
            ));
        }
        let actual = self
            .function
            .blocks()
            .get(block.index() as usize)
            .map(|body| body.terminator().kind());
        if !matches!(actual, Some(SemanticTerminatorKindV1::Call(retained)) if retained == call)
            || call.inline_assembly_source_v30().is_some()
            || call.ordered_region_source_v31().is_some()
            || !matches!(
                self.callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(actual),
                    ..
                }) if *actual == program
            )
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
        {
            return Err(fail(
                "ordered program must be the exact retained nonunwinding call",
            ));
        }
        let source = call
            .ordered_program_source_v32()
            .filter(|source| source.function() == self.function.identity())
            .ok_or_else(|| fail("ordered program source is absent or belongs to another caller"))?;
        let source = AssemblySourceIdentity::new(
            source.frontend_unit(),
            *source.function().as_bytes(),
            source.contract(),
            source.statement(),
        );
        if !source.is_complete() {
            return Err(fail("ordered program source references are incomplete"));
        }
        let destination = call
            .destination()
            .ok_or_else(|| fail("ordered program has no result place"))?;
        if !destination.place().projections().is_empty() {
            return Err(fail("ordered program result must be a direct scalar local"));
        }
        let result_type = destination.place().ty();
        if lower_scalar_type(self.types, result_type)? != Type::Scalar(ScalarType::U32)
            || call.arguments()[..3]
                .iter()
                .any(|input| input.ty() != result_type)
        {
            return Err(fail(
                "ordered program data and result require one exact u32 type",
            ));
        }
        let mut physical = [0_u8; 5];
        for (position, argument) in call.arguments()[3..].iter().enumerate() {
            let SemanticOperandV1::Constant(constant) = argument else {
                return Err(fail(
                    "ordered program register bindings must be literal u8 constants",
                ));
            };
            if lower_scalar_type(self.types, constant.ty())? != Type::Scalar(ScalarType::U8) {
                return Err(fail("ordered program physical binding type is not u8"));
            }
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err(fail("ordered program physical binding is not scalar"));
            };
            if value.size_bytes() != 1 {
                return Err(fail("ordered program physical binding has non-u8 storage"));
            }
            physical[position] = u8::try_from(value.bits())
                .map_err(|_| fail("ordered program physical binding overflows u8"))?;
        }
        let registers = Gfx942OrderedProgramRegistersV1::new(
            physical[0],
            physical[1],
            [physical[2], physical[3], physical[4]],
        )
        .map_err(|_| fail("ordered program physical bindings overlap or exceed v63"))?;
        // The semantic and KIR descriptors have independent checked constructors.
        // Revalidate all slots and initialization rules; never transmute models or
        // regenerate a preferred equivalent sequence from the evaluated result.
        let program = Gfx942U32ProgramV1::from_descriptors(program.count(), *program.descriptors())
            .map_err(|_| fail("ordered program descriptors fail independent KIR validation"))?;
        let mut inputs = [ValueId(0); 3];
        for (position, operand) in call.arguments()[..3].iter().enumerate() {
            // A projected call operand can perform a hidden memory read while
            // being lowered. This initial unconditional profile admits only
            // already computed scalar locals or scalar constants as inputs.
            if !ordered_program_direct_scalar_operand_v32(self.types, operand) {
                return Err(fail(
                    "ordered program inputs must be direct scalar locals or constants",
                ));
            }
            let (value, ty) = self
                .lower_operand(block, None, operand, operations)?
                .value()
                .map_err(fail)?;
            if ty != Type::Scalar(ScalarType::U32) {
                return Err(fail("ordered program lowered input is not u32"));
            }
            inputs[position] = value;
        }
        let region = Gfx942OrderedProgramV1::new(source, registers, inputs, program)
            .map_err(|_| fail("ordered program contract differs from canonical profile"))?;
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(self.next_value), Type::Scalar(ScalarType::U32)),
            OperationKind::Gfx942OrderedProgram(region),
        );
        validate_gfx942_ordered_program_v1(&operation, |value| {
            inputs.contains(&value).then_some(ScalarType::U32)
        })
        .map_err(|_| fail("ordered program emission failed shared contract validation"))?;
        // emit meters this operation and leaves it opaque to ordinary constant
        // and bounds propagation. Physical u8 metadata never becomes SSA input.
        self.emit(operations, Type::Scalar(ScalarType::U32), operation.kind)
    }
}
