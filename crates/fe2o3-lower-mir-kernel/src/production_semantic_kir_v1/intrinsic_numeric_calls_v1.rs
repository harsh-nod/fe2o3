// Non-inlined numeric call bodies keep unrelated intrinsic temporaries off the dispatcher stack.
impl SemanticFunctionLoweringV1<'_> {
    #[inline(never)]
    fn lower_intrinsic_math_f32_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        function: &SemanticF32MathFunctionV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, function.arity() + 1)?;
            let context = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            if !matches!(context, SemanticValueBindingV1::MathContext) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "device math operation lacks compiler-issued math authority",
                ));
            }
            let function = lower_f32_math_function(*function);
            let mut arguments = Vec::with_capacity(function.arity());
            for argument in &call.arguments()[1..] {
                let (id, ty) = self
                    .lower_operand(block, None, argument, operations)?
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                if ty != Type::Scalar(ScalarType::F32) {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "device math argument is not f32",
                    ));
                }
                arguments.push(id);
            }
            self.emit_float_operation(
                operations,
                FloatOperation::F32Math {
                    function,
                    implementation: function.required_implementation(),
                    arguments,
                },
            )?
        })
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn lower_intrinsic_bf16_conversion_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
        kind: &SemanticBf16ConversionKindV1,
        input: &SemanticTypeIdV1,
        output: &SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            if semantic_operand_type(&call.arguments()[0]) != *input
                || destination.place().ty() != *output
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "BF16 conversion semantic input or output type changed",
                ));
            }
            let argument = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            match kind {
                SemanticBf16ConversionKindV1::FromBits => {
                    let (bits, ty) = argument
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                    if ty != Type::Scalar(ScalarType::U16) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 from_bits input is not u16",
                        ));
                    }
                    binding_from_value_defs(self.types, *output, &[ValueDef::new(bits, ty)])?
                }
                SemanticBf16ConversionKindV1::ToBits => {
                    let values = argument
                        .values()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                    let [(bits, ty)] = values.as_slice() else {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 to_bits storage is not one scalar",
                        ));
                    };
                    if *ty != Type::Scalar(ScalarType::U16) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 to_bits storage is not u16",
                        ));
                    }
                    binding_from_value_defs(
                        self.types,
                        *output,
                        &[ValueDef::new(*bits, ty.clone())],
                    )?
                }
                SemanticBf16ConversionKindV1::FromF32RoundTiesEven => {
                    let (value, ty) = argument
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                    if ty != Type::Scalar(ScalarType::F32) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 from_f32 input is not f32",
                        ));
                    }
                    let (narrowed, narrowed_ty) = self
                        .emit_float_operation(
                            operations,
                            FloatOperation::Convert {
                                kind: FloatConversionKind::F32ToBf16RoundTiesEven,
                                value,
                            },
                        )?
                        .value()
                        .expect("BF16 conversion emits one value");
                    let bits_ty = Type::Scalar(ScalarType::U16);
                    let bits = self.emit_id(
                        operations,
                        bits_ty.clone(),
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value: narrowed,
                            to: bits_ty.clone(),
                        },
                    )?;
                    debug_assert_eq!(narrowed_ty, Type::Scalar(ScalarType::Bf16));
                    binding_from_value_defs(self.types, *output, &[ValueDef::new(bits, bits_ty)])?
                }
                SemanticBf16ConversionKindV1::ToF32 => {
                    let values = argument
                        .values()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                    let [(bits, ty)] = values.as_slice() else {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 to_f32 storage is not one scalar",
                        ));
                    };
                    if *ty != Type::Scalar(ScalarType::U16) {
                        return Err(unsupported(
                            0,
                            Some(block.index()),
                            None,
                            "BF16 to_f32 storage is not u16",
                        ));
                    }
                    let bf16 = self.emit_id(
                        operations,
                        Type::Scalar(ScalarType::Bf16),
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value: *bits,
                            to: Type::Scalar(ScalarType::Bf16),
                        },
                    )?;
                    self.emit_float_operation(
                        operations,
                        FloatOperation::Convert {
                            kind: FloatConversionKind::Bf16ToF32,
                            value: bf16,
                        },
                    )?
                }
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_fabs_f32_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            if semantic_operand_type(&call.arguments()[0]) != destination.place().ty() {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "fabs input and destination types changed",
                ));
            }
            let (argument, ty) = self
                .lower_operand(block, None, &call.arguments()[0], operations)?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            if ty != Type::Scalar(ScalarType::F32) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "fabs input is not f32",
                ));
            }
            self.emit_float_operation(
                operations,
                FloatOperation::F32Math {
                    function: F32MathFunction::Abs,
                    implementation: F32MathFunction::Abs.required_implementation(),
                    arguments: vec![argument],
                },
            )?
        })
    }
}
