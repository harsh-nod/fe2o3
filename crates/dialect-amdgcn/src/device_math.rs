//! Stable recognition of the `fe2o3-device` half and math diagnostic items.

use fe2o3_kernel_ir::{
    F32MathFunction, F32MathImplementation, FloatOperation, ScalarType, Type, ValueId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceValueDiagnosticItem {
    F16,
    Bf16,
    Bf16x2,
}

impl DeviceValueDiagnosticItem {
    pub const fn storage_type(self) -> Type {
        match self {
            Self::F16 => Type::Scalar(ScalarType::F16),
            Self::Bf16 => Type::Scalar(ScalarType::Bf16),
            Self::Bf16x2 => Type::Scalar(ScalarType::U32),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceMathDiagnosticItem {
    Context,
    ContextFromCompiler,
    F32(F32MathFunction),
    Bf16x2FusedMultiplyAdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fe2o3DeviceDiagnosticItem {
    Value(DeviceValueDiagnosticItem),
    Math(DeviceMathDiagnosticItem),
}

/// Recognizes only the versioned identities declared by `fe2o3-device`.
pub fn recognize_fe2o3_device_diagnostic_item(name: &str) -> Option<Fe2o3DeviceDiagnosticItem> {
    use DeviceMathDiagnosticItem as Math;
    use DeviceValueDiagnosticItem as Value;
    use F32MathFunction as F32;
    Some(match name {
        "fe2o3_device_f16_v1" => Fe2o3DeviceDiagnosticItem::Value(Value::F16),
        "fe2o3_device_bf16_v1" => Fe2o3DeviceDiagnosticItem::Value(Value::Bf16),
        "fe2o3_device_bf16x2_v1" => Fe2o3DeviceDiagnosticItem::Value(Value::Bf16x2),
        "fe2o3_device_math_context_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::Context),
        "fe2o3_device_math_context_from_compiler_v1" => {
            Fe2o3DeviceDiagnosticItem::Math(Math::ContextFromCompiler)
        }
        "fe2o3_device_math_sqrt_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Sqrt)),
        "fe2o3_device_math_fma_f32_v1" => {
            Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::FusedMultiplyAdd))
        }
        "fe2o3_device_math_floor_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Floor)),
        "fe2o3_device_math_ceil_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Ceil)),
        "fe2o3_device_math_trunc_f32_v1" => {
            Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Truncate))
        }
        "fe2o3_device_math_roundeven_f32_v1" => {
            Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::RoundTiesEven))
        }
        "fe2o3_device_math_sin_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Sin)),
        "fe2o3_device_math_cos_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Cos)),
        "fe2o3_device_math_exp_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Exp)),
        "fe2o3_device_math_exp2_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Exp2)),
        "fe2o3_device_math_log_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Ln)),
        "fe2o3_device_math_log2_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Log2)),
        "fe2o3_device_math_log10_f32_v1" => Fe2o3DeviceDiagnosticItem::Math(Math::F32(F32::Log10)),
        "fe2o3_device_math_fma_bf16x2_v1" => {
            Fe2o3DeviceDiagnosticItem::Math(Math::Bf16x2FusedMultiplyAdd)
        }
        _ => return None,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceMathRecognitionError {
    NotAnOperation(DeviceMathDiagnosticItem),
    WrongArity {
        item: DeviceMathDiagnosticItem,
        expected: usize,
        actual: usize,
    },
}

/// Converts an authenticated math diagnostic item and its numerical arguments into semantic IR.
///
/// The `DeviceMath` receiver is intentionally absent. The rustc producer must authenticate and
/// remove that capability argument before calling this function.
pub fn recognized_device_math_operation(
    item: DeviceMathDiagnosticItem,
    arguments: &[ValueId],
) -> Result<FloatOperation, DeviceMathRecognitionError> {
    match item {
        DeviceMathDiagnosticItem::Context | DeviceMathDiagnosticItem::ContextFromCompiler => {
            Err(DeviceMathRecognitionError::NotAnOperation(item))
        }
        DeviceMathDiagnosticItem::F32(function) => {
            let expected = function.arity();
            if arguments.len() != expected {
                return Err(DeviceMathRecognitionError::WrongArity {
                    item,
                    expected,
                    actual: arguments.len(),
                });
            }
            Ok(FloatOperation::F32Math {
                function,
                implementation: function.required_implementation(),
                arguments: arguments.to_vec(),
            })
        }
        DeviceMathDiagnosticItem::Bf16x2FusedMultiplyAdd => {
            let [value, multiplier, addend] = arguments else {
                return Err(DeviceMathRecognitionError::WrongArity {
                    item,
                    expected: 3,
                    actual: arguments.len(),
                });
            };
            Ok(FloatOperation::Bf16x2FusedMultiplyAdd {
                value: *value,
                multiplier: *multiplier,
                addend: *addend,
            })
        }
    }
}

pub const fn implementation_for(function: F32MathFunction) -> F32MathImplementation {
    function.required_implementation()
}
