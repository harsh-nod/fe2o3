use dialect_amdgcn::*;
use fe2o3_kernel_ir::*;

#[test]
fn recognizes_every_versioned_device_item_exactly() {
    let expected = [
        "fe2o3_device_f16_v1",
        "fe2o3_device_bf16_v1",
        "fe2o3_device_bf16x2_v1",
        "fe2o3_device_math_context_v1",
        "fe2o3_device_math_context_from_compiler_v1",
        "fe2o3_device_math_sqrt_f32_v1",
        "fe2o3_device_math_fma_f32_v1",
        "fe2o3_device_math_floor_f32_v1",
        "fe2o3_device_math_ceil_f32_v1",
        "fe2o3_device_math_trunc_f32_v1",
        "fe2o3_device_math_roundeven_f32_v1",
        "fe2o3_device_math_sin_f32_v1",
        "fe2o3_device_math_cos_f32_v1",
        "fe2o3_device_math_exp_f32_v1",
        "fe2o3_device_math_exp2_f32_v1",
        "fe2o3_device_math_log_f32_v1",
        "fe2o3_device_math_log2_f32_v1",
        "fe2o3_device_math_log10_f32_v1",
        "fe2o3_device_math_fma_bf16x2_v1",
    ];
    for name in expected {
        assert!(
            recognize_fe2o3_device_diagnostic_item(name).is_some(),
            "missing {name}"
        );
        assert!(recognize_fe2o3_device_diagnostic_item(&format!("{name}_mutated")).is_none());
    }
}

#[test]
fn value_items_fix_their_storage_layout_type() {
    assert_eq!(
        DeviceValueDiagnosticItem::F16.storage_type(),
        Type::Scalar(ScalarType::F16)
    );
    assert_eq!(
        DeviceValueDiagnosticItem::Bf16.storage_type(),
        Type::Scalar(ScalarType::Bf16)
    );
    assert_eq!(
        DeviceValueDiagnosticItem::Bf16x2.storage_type(),
        Type::Scalar(ScalarType::U32)
    );
}

#[test]
fn math_recognition_fixes_implementation_and_arity() {
    let sin = recognized_device_math_operation(
        DeviceMathDiagnosticItem::F32(F32MathFunction::Sin),
        &[ValueId(7)],
    )
    .unwrap();
    assert_eq!(
        sin,
        FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::OcmlAbiV1,
            arguments: vec![ValueId(7)],
        }
    );

    let fma = recognized_device_math_operation(
        DeviceMathDiagnosticItem::F32(F32MathFunction::FusedMultiplyAdd),
        &[ValueId(1), ValueId(2), ValueId(3)],
    )
    .unwrap();
    assert!(matches!(
        fma,
        FloatOperation::F32Math {
            implementation: F32MathImplementation::ConstrainedLlvm,
            ..
        }
    ));

    assert!(matches!(
        recognized_device_math_operation(
            DeviceMathDiagnosticItem::F32(F32MathFunction::FusedMultiplyAdd),
            &[ValueId(1)]
        ),
        Err(DeviceMathRecognitionError::WrongArity {
            expected: 3,
            actual: 1,
            ..
        })
    ));
    assert!(matches!(
        recognized_device_math_operation(DeviceMathDiagnosticItem::Context, &[]),
        Err(DeviceMathRecognitionError::NotAnOperation(_))
    ));
}
