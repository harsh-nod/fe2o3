/// One exact conversion between the integer-backed narrow-float values and `f32`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatConversionKind {
    F16ToF32,
    F32ToF16RoundTiesEven,
    Bf16ToF32,
    F32ToBf16RoundTiesEven,
}

/// The integer-backed narrow format used by widened arithmetic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NarrowFloatFormat {
    F16,
    Bf16,
}

impl NarrowFloatFormat {
    pub const fn ty(self) -> Type {
        match self {
            Self::F16 => Type::Scalar(ScalarType::F16),
            Self::Bf16 => Type::Scalar(ScalarType::Bf16),
        }
    }

    pub const fn capability(self) -> TargetCapability {
        match self {
            Self::F16 => TargetCapability::Float16,
            Self::Bf16 => TargetCapability::BFloat16,
        }
    }
}

/// Binary arithmetic performed after exact widening to `f32`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WidenedFloatBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Scalar functions exposed by `fe2o3-device::DeviceMath`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32MathFunction {
    Sqrt,
    FusedMultiplyAdd,
    Floor,
    Ceil,
    Truncate,
    RoundTiesEven,
    Sin,
    Cos,
    Exp,
    Exp2,
    Ln,
    Log2,
    Log10,
    /// IEEE-754 absolute value with no arithmetic rounding.
    Abs,
}

impl F32MathFunction {
    pub const fn arity(self) -> usize {
        match self {
            Self::FusedMultiplyAdd => 3,
            _ => 1,
        }
    }

    pub const fn required_implementation(self) -> F32MathImplementation {
        match self {
            Self::Sqrt => F32MathImplementation::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
            Self::FusedMultiplyAdd
            | Self::Floor
            | Self::Ceil
            | Self::Truncate
            | Self::RoundTiesEven => F32MathImplementation::ConstrainedLlvm,
            Self::Sin
            | Self::Cos
            | Self::Exp
            | Self::Exp2
            | Self::Ln
            | Self::Log2
            | Self::Log10 => F32MathImplementation::OcmlAbiV1,
            Self::Abs => F32MathImplementation::IeeeFabsV1,
        }
    }
}

/// The implementation contract that gives an `f32` math operation meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32MathImplementation {
    /// LLVM constrained intrinsics, round-to-nearest-even, ignored exceptions.
    ConstrainedLlvm,
    /// The strict `__ocml_*_f32` ABI, with fast/finite/unsafe modes disabled.
    OcmlAbiV1,
    /// IEEE `f32` square root, round-to-nearest-even, with exceptions ignored.
    ///
    /// A target may realize this with constrained sqrt or with native
    /// `llvm.sqrt.f32` when its strict default floating-point environment has
    /// exactly these semantics.
    IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
    /// LLVM `fabs` semantics for one `f32` value.
    IeeeFabsV1,
}

/// A pure floating-point operation with no implicit contraction or target fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FloatOperation {
    Convert {
        kind: FloatConversionKind,
        value: ValueId,
    },
    /// Widen both operands exactly, perform one constrained `f32` operation,
    /// then narrow once using round-to-nearest, ties-to-even.
    WidenedBinary {
        format: NarrowFloatFormat,
        op: WidenedFloatBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    F32Math {
        function: F32MathFunction,
        implementation: F32MathImplementation,
        arguments: Vec<ValueId>,
    },
    /// Two independent constrained `f32` FMAs followed by exact BF16 RNE packing.
    /// Each packed operand and the result use lane zero in bits 0..16.
    Bf16x2FusedMultiplyAdd {
        value: ValueId,
        multiplier: ValueId,
        addend: ValueId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FloatIntrinsicCapabilityV1 {
    None,
    Float16,
    BFloat16,
}

#[derive(Clone, Copy)]
pub(crate) enum FloatIntrinsicDescriptorV1 {
    Convert(FloatConversionKind),
    WidenedBinary(NarrowFloatFormat, WidenedFloatBinaryOp),
    F32Math(F32MathFunction),
    Bf16x2FusedMultiplyAdd,
}

const FLOAT_INTRINSICS_V1: [(&str, FloatIntrinsicDescriptorV1); 27] = [
    (
        "__fe2o3_ir_float_v1_f16_to_f32",
        FloatIntrinsicDescriptorV1::Convert(FloatConversionKind::F16ToF32),
    ),
    (
        "__fe2o3_ir_float_v1_f32_to_f16_rne",
        FloatIntrinsicDescriptorV1::Convert(FloatConversionKind::F32ToF16RoundTiesEven),
    ),
    (
        "__fe2o3_ir_float_v1_bf16_to_f32",
        FloatIntrinsicDescriptorV1::Convert(FloatConversionKind::Bf16ToF32),
    ),
    (
        "__fe2o3_ir_float_v1_f32_to_bf16_rne",
        FloatIntrinsicDescriptorV1::Convert(FloatConversionKind::F32ToBf16RoundTiesEven),
    ),
    (
        "__fe2o3_ir_float_v1_f16_add_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::F16,
            WidenedFloatBinaryOp::Add,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_f16_sub_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::F16,
            WidenedFloatBinaryOp::Subtract,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_f16_mul_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::F16,
            WidenedFloatBinaryOp::Multiply,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_f16_div_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::F16,
            WidenedFloatBinaryOp::Divide,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_bf16_add_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::Bf16,
            WidenedFloatBinaryOp::Add,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_bf16_sub_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::Bf16,
            WidenedFloatBinaryOp::Subtract,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_bf16_mul_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::Bf16,
            WidenedFloatBinaryOp::Multiply,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_bf16_div_widened_rne",
        FloatIntrinsicDescriptorV1::WidenedBinary(
            NarrowFloatFormat::Bf16,
            WidenedFloatBinaryOp::Divide,
        ),
    ),
    (
        "__fe2o3_ir_float_v1_sqrt_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Sqrt),
    ),
    (
        "__fe2o3_ir_float_v1_fma_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::FusedMultiplyAdd),
    ),
    (
        "__fe2o3_ir_float_v1_floor_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Floor),
    ),
    (
        "__fe2o3_ir_float_v1_ceil_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Ceil),
    ),
    (
        "__fe2o3_ir_float_v1_trunc_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Truncate),
    ),
    (
        "__fe2o3_ir_float_v1_roundeven_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::RoundTiesEven),
    ),
    (
        "__fe2o3_ir_float_v1_sin_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Sin),
    ),
    (
        "__fe2o3_ir_float_v1_cos_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Cos),
    ),
    (
        "__fe2o3_ir_float_v1_exp_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Exp),
    ),
    (
        "__fe2o3_ir_float_v1_exp2_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Exp2),
    ),
    (
        "__fe2o3_ir_float_v1_log_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Ln),
    ),
    (
        "__fe2o3_ir_float_v1_log2_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Log2),
    ),
    (
        "__fe2o3_ir_float_v1_log10_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Log10),
    ),
    (
        "__fe2o3_ir_float_v1_fabs_f32",
        FloatIntrinsicDescriptorV1::F32Math(F32MathFunction::Abs),
    ),
    (
        "__fe2o3_ir_float_v1_fma_bf16x2",
        FloatIntrinsicDescriptorV1::Bf16x2FusedMultiplyAdd,
    ),
];

impl FloatIntrinsicDescriptorV1 {
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Convert(_) => 1,
            Self::WidenedBinary(_, _) => 2,
            Self::F32Math(function) => function.arity(),
            Self::Bf16x2FusedMultiplyAdd => 3,
        }
    }

    pub(crate) const fn capability(self) -> FloatIntrinsicCapabilityV1 {
        match self {
            Self::Convert(
                FloatConversionKind::F16ToF32 | FloatConversionKind::F32ToF16RoundTiesEven,
            )
            | Self::WidenedBinary(NarrowFloatFormat::F16, _) => FloatIntrinsicCapabilityV1::Float16,
            Self::Convert(
                FloatConversionKind::Bf16ToF32 | FloatConversionKind::F32ToBf16RoundTiesEven,
            )
            | Self::WidenedBinary(NarrowFloatFormat::Bf16, _)
            | Self::Bf16x2FusedMultiplyAdd => FloatIntrinsicCapabilityV1::BFloat16,
            Self::F32Math(_) => FloatIntrinsicCapabilityV1::None,
        }
    }

    pub(crate) const fn parameter_type(self, index: usize) -> Option<Type> {
        if index >= self.arity() {
            return None;
        }
        Some(match self {
            Self::Convert(FloatConversionKind::F16ToF32) => Type::Scalar(ScalarType::F16),
            Self::Convert(FloatConversionKind::F32ToF16RoundTiesEven)
            | Self::Convert(FloatConversionKind::F32ToBf16RoundTiesEven)
            | Self::F32Math(_) => Type::F32,
            Self::Convert(FloatConversionKind::Bf16ToF32) => Type::Scalar(ScalarType::Bf16),
            Self::WidenedBinary(format, _) => format.ty(),
            Self::Bf16x2FusedMultiplyAdd => Type::Scalar(ScalarType::U32),
        })
    }

    pub(crate) const fn result_type(self) -> Type {
        match self {
            Self::Convert(FloatConversionKind::F16ToF32 | FloatConversionKind::Bf16ToF32)
            | Self::F32Math(_) => Type::F32,
            Self::Convert(FloatConversionKind::F32ToF16RoundTiesEven) => {
                Type::Scalar(ScalarType::F16)
            }
            Self::Convert(FloatConversionKind::F32ToBf16RoundTiesEven) => {
                Type::Scalar(ScalarType::Bf16)
            }
            Self::WidenedBinary(format, _) => format.ty(),
            Self::Bf16x2FusedMultiplyAdd => Type::Scalar(ScalarType::U32),
        }
    }

    fn operation(self) -> FloatOperation {
        let values = [ValueId(0), ValueId(1), ValueId(2)];
        match self {
            Self::Convert(kind) => FloatOperation::Convert {
                kind,
                value: values[0],
            },
            Self::WidenedBinary(format, op) => FloatOperation::WidenedBinary {
                format,
                op,
                lhs: values[0],
                rhs: values[1],
            },
            Self::F32Math(function) => FloatOperation::F32Math {
                function,
                implementation: function.required_implementation(),
                arguments: values[..function.arity()].to_vec(),
            },
            Self::Bf16x2FusedMultiplyAdd => FloatOperation::Bf16x2FusedMultiplyAdd {
                value: values[0],
                multiplier: values[1],
                addend: values[2],
            },
        }
    }
}

impl FloatOperation {
    pub(crate) const INTRINSIC_DESCRIPTOR_COUNT_V1: usize = FLOAT_INTRINSICS_V1.len();

    pub(crate) fn intrinsic_descriptor_v1(
        callee: &FunctionId,
    ) -> Option<FloatIntrinsicDescriptorV1> {
        FLOAT_INTRINSICS_V1
            .iter()
            .find_map(|(name, descriptor)| (callee.as_str() == *name).then_some(*descriptor))
    }

    pub(crate) fn intrinsic_descriptor_lookup_work_v1(callee: &FunctionId) -> Option<usize> {
        callee
            .as_str()
            .len()
            // One descriptor-row visit plus a terminal-inclusive comparison.
            .checked_add(2)?
            .checked_mul(Self::INTRINSIC_DESCRIPTOR_COUNT_V1)
    }

    #[cfg(test)]
    pub(crate) fn intrinsic_descriptor_roster_v1()
    -> impl Iterator<Item = (&'static str, FloatIntrinsicDescriptorV1)> {
        FLOAT_INTRINSICS_V1.into_iter()
    }

    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::Convert { value, .. } => vec![*value],
            Self::WidenedBinary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Self::F32Math { arguments, .. } => arguments.clone(),
            Self::Bf16x2FusedMultiplyAdd {
                value,
                multiplier,
                addend,
            } => vec![*value, *multiplier, *addend],
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        match self {
            Self::Convert { kind, .. } => BTreeSet::from([match kind {
                FloatConversionKind::F16ToF32 | FloatConversionKind::F32ToF16RoundTiesEven => {
                    TargetCapability::Float16
                }
                FloatConversionKind::Bf16ToF32 | FloatConversionKind::F32ToBf16RoundTiesEven => {
                    TargetCapability::BFloat16
                }
            }]),
            Self::WidenedBinary { format, .. } => BTreeSet::from([format.capability()]),
            Self::F32Math { .. } => BTreeSet::new(),
            Self::Bf16x2FusedMultiplyAdd { .. } => BTreeSet::from([TargetCapability::BFloat16]),
        }
    }

    /// Closed semantic identity used to carry this operation through the existing call node.
    pub fn intrinsic_function_id(&self) -> FunctionId {
        FunctionId::new(match self {
            Self::Convert { kind, .. } => match kind {
                FloatConversionKind::F16ToF32 => "__fe2o3_ir_float_v1_f16_to_f32",
                FloatConversionKind::F32ToF16RoundTiesEven => "__fe2o3_ir_float_v1_f32_to_f16_rne",
                FloatConversionKind::Bf16ToF32 => "__fe2o3_ir_float_v1_bf16_to_f32",
                FloatConversionKind::F32ToBf16RoundTiesEven => {
                    "__fe2o3_ir_float_v1_f32_to_bf16_rne"
                }
            },
            Self::WidenedBinary { format, op, .. } => match (format, op) {
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Add) => {
                    "__fe2o3_ir_float_v1_f16_add_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Subtract) => {
                    "__fe2o3_ir_float_v1_f16_sub_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Multiply) => {
                    "__fe2o3_ir_float_v1_f16_mul_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Divide) => {
                    "__fe2o3_ir_float_v1_f16_div_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Add) => {
                    "__fe2o3_ir_float_v1_bf16_add_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Subtract) => {
                    "__fe2o3_ir_float_v1_bf16_sub_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Multiply) => {
                    "__fe2o3_ir_float_v1_bf16_mul_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Divide) => {
                    "__fe2o3_ir_float_v1_bf16_div_widened_rne"
                }
            },
            Self::F32Math {
                function,
                implementation,
                ..
            } if function.required_implementation() != *implementation => {
                "__fe2o3_ir_float_v1_invalid_contract"
            }
            Self::F32Math { function, .. } => match function {
                F32MathFunction::Sqrt => "__fe2o3_ir_float_v1_sqrt_f32",
                F32MathFunction::FusedMultiplyAdd => "__fe2o3_ir_float_v1_fma_f32",
                F32MathFunction::Floor => "__fe2o3_ir_float_v1_floor_f32",
                F32MathFunction::Ceil => "__fe2o3_ir_float_v1_ceil_f32",
                F32MathFunction::Truncate => "__fe2o3_ir_float_v1_trunc_f32",
                F32MathFunction::RoundTiesEven => "__fe2o3_ir_float_v1_roundeven_f32",
                F32MathFunction::Sin => "__fe2o3_ir_float_v1_sin_f32",
                F32MathFunction::Cos => "__fe2o3_ir_float_v1_cos_f32",
                F32MathFunction::Exp => "__fe2o3_ir_float_v1_exp_f32",
                F32MathFunction::Exp2 => "__fe2o3_ir_float_v1_exp2_f32",
                F32MathFunction::Ln => "__fe2o3_ir_float_v1_log_f32",
                F32MathFunction::Log2 => "__fe2o3_ir_float_v1_log2_f32",
                F32MathFunction::Log10 => "__fe2o3_ir_float_v1_log10_f32",
                F32MathFunction::Abs => "__fe2o3_ir_float_v1_fabs_f32",
            },
            Self::Bf16x2FusedMultiplyAdd { .. } => "__fe2o3_ir_float_v1_fma_bf16x2",
        })
    }

    pub fn result_type(&self) -> Type {
        match self {
            Self::Convert { kind, .. } => match kind {
                FloatConversionKind::F16ToF32 | FloatConversionKind::Bf16ToF32 => Type::F32,
                FloatConversionKind::F32ToF16RoundTiesEven => Type::Scalar(ScalarType::F16),
                FloatConversionKind::F32ToBf16RoundTiesEven => Type::Scalar(ScalarType::Bf16),
            },
            Self::WidenedBinary { format, .. } => format.ty(),
            Self::F32Math { .. } => Type::F32,
            Self::Bf16x2FusedMultiplyAdd { .. } => Type::Scalar(ScalarType::U32),
        }
    }

    pub fn parameter_types(&self) -> Vec<Type> {
        match self {
            Self::Convert { kind, .. } => vec![match kind {
                FloatConversionKind::F16ToF32 => Type::Scalar(ScalarType::F16),
                FloatConversionKind::F32ToF16RoundTiesEven => Type::F32,
                FloatConversionKind::Bf16ToF32 => Type::Scalar(ScalarType::Bf16),
                FloatConversionKind::F32ToBf16RoundTiesEven => Type::F32,
            }],
            Self::WidenedBinary { format, .. } => vec![format.ty(), format.ty()],
            Self::F32Math { function, .. } => vec![Type::F32; function.arity()],
            Self::Bf16x2FusedMultiplyAdd { .. } => vec![Type::Scalar(ScalarType::U32); 3],
        }
    }

    pub fn declaration(&self) -> Function {
        let mut function = Function::external_import(
            self.intrinsic_function_id(),
            Signature::new(self.parameter_types(), vec![self.result_type()]),
        );
        function.required_capabilities = self.required_capabilities();
        function
    }

    pub fn operation(&self, result: ValueId) -> Operation {
        Operation::effect_free(
            ValueDef::new(result, self.result_type()),
            OperationKind::Call {
                callee: self.intrinsic_function_id(),
                arguments: self.operands(),
            },
        )
    }

    pub fn from_intrinsic_call(callee: &FunctionId, arguments: &[ValueId]) -> Option<Self> {
        let descriptor = Self::intrinsic_descriptor_v1(callee)?;
        if descriptor.arity() != arguments.len() {
            return None;
        }
        let mut float = descriptor.operation();
        match &mut float {
            Self::Convert { value, .. } => *value = arguments[0],
            Self::WidenedBinary { lhs, rhs, .. } => {
                *lhs = arguments[0];
                *rhs = arguments[1];
            }
            Self::F32Math {
                arguments: values, ..
            } => values.clone_from_slice(arguments),
            Self::Bf16x2FusedMultiplyAdd {
                value,
                multiplier,
                addend,
            } => {
                *value = arguments[0];
                *multiplier = arguments[1];
                *addend = arguments[2];
            }
        }
        Some(float)
    }

    pub fn from_intrinsic_id(callee: &FunctionId) -> Option<Self> {
        Self::intrinsic_descriptor_v1(callee).map(FloatIntrinsicDescriptorV1::operation)
    }
}
