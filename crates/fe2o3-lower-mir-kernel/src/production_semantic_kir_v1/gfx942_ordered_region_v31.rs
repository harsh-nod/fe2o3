// Preserve one closed authored region as one executable operation. This leaf
// repeats immutable-call/type/register checks; source authentication and target,
// launch and CFG qualification remain independent admission responsibilities.

impl SemanticFunctionLoweringV1<'_> {
    fn lower_gfx942_ordered_region_v31(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        profile: fe2o3_mir_model::semantic_mir_v1::SemanticGfx942OrderedRegionProfileV31,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::{
            AssemblySourceIdentity, Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1,
            validate_gfx942_ordered_region_v1,
        };
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticConstantValueV1, SemanticFunctionRoleV1, SemanticGfx942OrderedRegionProfileV31,
        };

        let function_index = self.semantic_function.index();
        let fail = |detail| unsupported(function_index, Some(block.index()), None, detail);
        self.require_call_argument_count(block, call, 8)?;
        if self.function.role() != SemanticFunctionRoleV1::KernelRoot
            || self.required_workgroup != Some([64, 1, 1])
        {
            return Err(fail(
                "ordered region requires a direct root and required 64x1x1 workgroup",
            ));
        }
        let actual = self
            .function
            .blocks()
            .get(block.index() as usize)
            .map(|body| body.terminator().kind());
        if !matches!(actual, Some(SemanticTerminatorKindV1::Call(retained)) if retained == call)
            || call.inline_assembly_source_v30().is_some()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
        {
            return Err(fail(
                "ordered region must be the exact retained nonunwinding call",
            ));
        }
        let source = call
            .ordered_region_source_v31()
            .filter(|source| source.function() == self.function.identity())
            .ok_or_else(|| fail("ordered region source is absent or belongs to another caller"))?;
        let source = AssemblySourceIdentity::new(
            source.frontend_unit(),
            *source.function().as_bytes(),
            source.contract(),
            source.statement(),
        );
        if !source.is_complete() {
            return Err(fail("ordered region source references are incomplete"));
        }
        let destination = call
            .destination()
            .ok_or_else(|| fail("ordered region has no result place"))?;
        if !destination.place().projections().is_empty() {
            return Err(fail("ordered region result must be a direct scalar local"));
        }
        let result_type = destination.place().ty();
        if lower_scalar_type(self.types, result_type)? != Type::Scalar(ScalarType::U32)
            || call.arguments()[..3]
                .iter()
                .any(|input| input.ty() != result_type)
        {
            return Err(fail(
                "ordered region data and result require one exact u32 type",
            ));
        }
        let mut physical = [0_u8; 5];
        for (position, argument) in call.arguments()[3..].iter().enumerate() {
            let SemanticOperandV1::Constant(constant) = argument else {
                return Err(fail(
                    "ordered region register bindings must be literal u8 constants",
                ));
            };
            if lower_scalar_type(self.types, constant.ty())? != Type::Scalar(ScalarType::U8) {
                return Err(fail("ordered region physical binding type is not u8"));
            }
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err(fail("ordered region physical binding is not scalar"));
            };
            if value.size_bytes() != 1 {
                return Err(fail("ordered region physical binding has non-u8 storage"));
            }
            physical[position] = u8::try_from(value.bits())
                .map_err(|_| fail("ordered region physical binding overflows u8"))?;
        }
        let registers = Gfx942OrderedRegionRegistersV1::new(
            physical[0],
            physical[1],
            [physical[2], physical[3], physical[4]],
        )
        .map_err(|_| fail("ordered region physical bindings overlap or exceed v63"))?;
        match profile {
            SemanticGfx942OrderedRegionProfileV31::XorAddU32E32 => {}
        }
        let mut inputs = [ValueId(0); 3];
        for (position, operand) in call.arguments()[..3].iter().enumerate() {
            // A projected call operand can perform a hidden memory read while
            // being lowered. This initial unconditional profile admits only
            // already computed scalar locals or scalar constants as inputs.
            if !ordered_region_direct_scalar_operand_v31(self.types, operand) {
                return Err(fail(
                    "ordered region inputs must be direct scalar locals or constants",
                ));
            }
            let (value, ty) = self
                .lower_operand(block, None, operand, operations)?
                .value()
                .map_err(fail)?;
            if ty != Type::Scalar(ScalarType::U32) {
                return Err(fail("ordered region lowered input is not u32"));
            }
            inputs[position] = value;
        }
        let region = Gfx942OrderedRegionV1::new(source, registers, inputs)
            .map_err(|_| fail("ordered region contract differs from canonical profile"))?;
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(self.next_value), Type::Scalar(ScalarType::U32)),
            OperationKind::Gfx942OrderedRegion(region),
        );
        validate_gfx942_ordered_region_v1(&operation, |value| {
            inputs.contains(&value).then_some(ScalarType::U32)
        })
        .map_err(|_| fail("ordered region emission failed shared contract validation"))?;
        // emit meters this operation and leaves it opaque to ordinary constant
        // and bounds propagation. Physical u8 metadata never becomes SSA input.
        self.emit(operations, Type::Scalar(ScalarType::U32), operation.kind)
    }
}
