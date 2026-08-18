use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir,
    lower_compiler_module_to_llvm_ir, lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fe2o3-gfx942-math-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn math_kernel(
    parameters: Vec<Type>,
    result: Type,
    float: FloatOperation,
    capabilities: impl IntoIterator<Item = TargetCapability>,
) -> Module {
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let mut block = BasicBlock::new(BlockId(0));
    let declaration = float.declaration();
    let mut operation = float.operation(ValueId(parameters.len() as u32));
    operation.results[0].ty = result;
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "math_impl",
        Signature::new(parameters, vec![]),
        parameter_ids,
        vec![block],
    );
    function.required_capabilities.extend(capabilities);
    let mut kernel = Kernel::new(
        "math_kernel",
        "math_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tests::gfx942_math");
    module.functions.push(function);
    module.functions.push(declaration);
    module.kernels.push(kernel);
    module
}

fn packed_bf16_fma_module() -> Module {
    math_kernel(
        vec![Type::Scalar(ScalarType::U32); 3],
        Type::Scalar(ScalarType::U32),
        FloatOperation::Bf16x2FusedMultiplyAdd {
            value: ValueId(0),
            multiplier: ValueId(1),
            addend: ValueId(2),
        },
        [TargetCapability::BFloat16],
    )
}

fn f16_divide_module() -> Module {
    math_kernel(
        vec![Type::Scalar(ScalarType::F16); 2],
        Type::Scalar(ScalarType::F16),
        FloatOperation::WidenedBinary {
            format: NarrowFloatFormat::F16,
            op: WidenedFloatBinaryOp::Divide,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
        [TargetCapability::Float16],
    )
}

fn two_kernel_ocml_module() -> Module {
    let shared_helper = Function::internal_helper(
        "shared_math_helper",
        Signature::new(vec![Type::F32], vec![Type::F32]),
        vec![ValueId(0)],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![],
            terminator: Some(Terminator::Return {
                values: vec![ValueId(0)],
            }),
        }],
    );
    let shared_call = |result| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), Type::F32),
            OperationKind::Call {
                callee: "shared_math_helper".into(),
                arguments: vec![ValueId(0)],
            },
        )
    };

    let sin = FloatOperation::F32Math {
        function: F32MathFunction::Sin,
        implementation: F32MathImplementation::OcmlAbiV1,
        arguments: vec![ValueId(1)],
    };
    let mut math_block = BasicBlock::new(BlockId(0));
    math_block.operations = vec![shared_call(1), sin.operation(ValueId(2))];
    math_block.terminator = Some(Terminator::Return { values: vec![] });
    let math_entry = Function::kernel_entry(
        "z_math_entry",
        Signature::new(vec![Type::F32], vec![]),
        vec![ValueId(0)],
        vec![math_block],
    );

    let mut plain_block = BasicBlock::new(BlockId(0));
    plain_block.operations = vec![shared_call(1)];
    plain_block.terminator = Some(Terminator::Return { values: vec![] });
    let plain_entry = Function::kernel_entry(
        "a_plain_entry",
        Signature::new(vec![Type::F32], vec![]),
        vec![ValueId(0)],
        vec![plain_block],
    );

    let mut math_kernel = Kernel::new(
        "math_kernel",
        "z_math_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    math_kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut plain_kernel = Kernel::new(
        "plain_kernel",
        "a_plain_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    plain_kernel.workgroup_size = Some(WorkgroupSize::new(128, 1, 1));

    let mut module = Module::new("tests::gfx942_two_kernel_ocml");
    module
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    module.functions = vec![math_entry, sin.declaration(), plain_entry, shared_helper];
    module.kernels = vec![plain_kernel, math_kernel];
    module
}

#[test]
fn packed_bf16_fma_has_exact_gfx942_golden_ir() {
    let llvm =
        lower_kernel_to_gfx942_llvm_ir(&packed_bf16_fma_module(), &"math_kernel".into()).unwrap();
    assert_eq!(llvm, include_str!("fixtures/gfx942_bf16x2_fma.ll"));
    assert_eq!(
        llvm.matches("call float @llvm.experimental.constrained.fma.f32")
            .count(),
        2
    );
    assert!(!llvm.contains("contract"));
    assert!(!llvm.contains(" fast "));
}

#[test]
fn strict_ocml_call_has_exact_golden_ir() {
    let module = math_kernel(
        vec![Type::F32],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::OcmlAbiV1,
            arguments: vec![ValueId(0)],
        },
        [],
    );
    let llvm = lower_kernel_to_gfx942_llvm_ir(&module, &"math_kernel".into()).unwrap();
    assert_eq!(llvm, include_str!("fixtures/gfx942_sin_f32.ll"));
}

#[test]
fn baseline_missing_capability_and_mutated_contracts_fail_closed() {
    let module = packed_bf16_fma_module();
    assert!(
        lower_kernel_to_llvm_ir(&module, &"math_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );

    let mut missing = module.clone();
    missing.functions[0].required_capabilities.clear();
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&missing, &"math_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );

    let mutated = math_kernel(
        vec![Type::F32],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::ConstrainedLlvm,
            arguments: vec![ValueId(0)],
        },
        [],
    );
    assert!(matches!(
        lower_kernel_to_gfx942_llvm_ir(&mutated, &"math_kernel".into()),
        Err(error) if error.contains(LoweringDiagnosticCode::InputVerification(
            DiagnosticCode::InvalidFloatOperation
        ))
    ));
}

#[test]
fn constrained_math_uses_fixed_rounding_and_exception_metadata() {
    let module = math_kernel(
        vec![Type::F32; 3],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::FusedMultiplyAdd,
            implementation: F32MathImplementation::ConstrainedLlvm,
            arguments: vec![ValueId(0), ValueId(1), ValueId(2)],
        },
        [],
    );
    let llvm = lower_kernel_to_gfx942_llvm_ir(&module, &"math_kernel".into()).unwrap();
    assert!(llvm.contains("metadata !\"round.tonearest\", metadata !\"fpexcept.ignore\""));
    assert!(llvm.contains("\"denormal-fp-math-f32\"=\"ieee,ieee\""));
    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
}

#[test]
fn f16_widen_divide_narrow_sequence_is_explicit() {
    let llvm = lower_kernel_to_gfx942_llvm_ir(&f16_divide_module(), &"math_kernel".into()).unwrap();
    assert!(llvm.contains("define internal float @__fe2o3_f16_to_f32_v1"));
    assert!(llvm.contains("call i32 @llvm.ctlz.i32(i32 %fraction, i1 false)"));
    assert!(llvm.contains("define internal i16 @__fe2o3_f32_to_f16_rne_v1"));
    assert!(llvm.contains("%normal.tie.up = and i1 %normal.equal, %normal.odd"));
    assert!(llvm.contains("%subnormal.tie.up = and i1 %subnormal.equal, %subnormal.odd"));
    assert_eq!(
        llvm.matches("call float @llvm.experimental.constrained.fdiv.f32")
            .count(),
        1
    );
    assert!(llvm.contains("metadata !\"round.tonearest\", metadata !\"fpexcept.ignore\""));
}

#[test]
fn compiler_module_path_preserves_the_gfx942_float_contract() {
    let module = packed_bf16_fma_module();
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
    assert_eq!(
        llvm.matches("declare float @llvm.experimental.constrained.fma.f32")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("call float @llvm.experimental.constrained.fma.f32")
            .count(),
        2
    );
    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(
        lower_compiler_module_to_llvm_ir(&module)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );
}

#[test]
fn two_kernel_shared_helper_and_ocml_module_has_exact_golden_ir() {
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&two_kernel_ocml_module()).unwrap();
    assert_eq!(llvm, include_str!("fixtures/gfx942_two_kernel_ocml.ll"));
    assert_eq!(
        llvm.matches("define amdgpu_kernel void @math_kernel")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("define amdgpu_kernel void @plain_kernel")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("define internal float @shared_math_helper")
            .count(),
        1
    );
    assert_eq!(llvm.matches("call float @shared_math_helper").count(), 2);
    assert_eq!(llvm.matches("declare float @__ocml_sin_f32").count(), 1);
    assert_eq!(llvm.matches("call float @__ocml_sin_f32").count(), 1);
    assert!(!llvm.contains("__fe2o3_ir_float_v1_sin_f32"));
}

#[test]
fn two_kernel_gfx942_module_is_independent_of_input_order() {
    let baseline = lower_compiler_module_to_gfx942_llvm_ir(&two_kernel_ocml_module()).unwrap();
    let mut reordered = two_kernel_ocml_module();
    reordered.functions.reverse();
    reordered.kernels.reverse();
    assert_eq!(
        lower_compiler_module_to_gfx942_llvm_ir(&reordered).unwrap(),
        baseline
    );
}

#[test]
fn two_kernel_gfx942_module_rejects_duplicate_and_reserved_symbols() {
    let mut duplicate = two_kernel_ocml_module();
    duplicate.functions.push(
        duplicate
            .functions
            .iter()
            .find(|function| function.id.as_str() == "shared_math_helper")
            .unwrap()
            .clone(),
    );
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&duplicate)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::DuplicateFunction,
            ))
    );

    let mut reserved = two_kernel_ocml_module();
    reserved.kernels[0].id = "__ocml_sin_f32".into();
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&reserved)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::ConflictingSymbol)
    );

    let mut collision = two_kernel_ocml_module();
    collision.kernels[0].id = "shared_math_helper".into();
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&collision)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::ConflictingSymbol)
    );
}

#[test]
fn generated_support_symbols_cannot_be_substituted_by_user_ir() {
    let mut module = packed_bf16_fma_module();
    module.kernels[0].id = "__fe2o3_bf16_to_f32_v1".into();
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &"__fe2o3_bf16_to_f32_v1".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::ConflictingSymbol)
    );

    let mut ocml = packed_bf16_fma_module();
    ocml.kernels[0].id = "__ocml_sin_f32".into();
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&ocml)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::ConflictingSymbol)
    );
}

#[test]
#[ignore = "requires ROCm LLVM tools with gfx942 support"]
fn rocm_compiles_and_inspects_gfx942_float_modules() {
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD");
    let readelf = std::env::var("FE2O3_LLVM_READELF").expect("set FE2O3_LLVM_READELF");
    let directory = TemporaryDirectory::new();
    let modules = [
        ("bf16x2", packed_bf16_fma_module(), true),
        ("f16", f16_divide_module(), true),
        (
            "ocml",
            math_kernel(
                vec![Type::F32],
                Type::F32,
                FloatOperation::F32Math {
                    function: F32MathFunction::Sin,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                },
                [],
            ),
            false,
        ),
    ];

    for (name, module, self_contained) in modules {
        let llvm = lower_kernel_to_gfx942_llvm_ir(&module, &"math_kernel".into()).unwrap();
        let source = directory.join(&format!("{name}.ll"));
        let object = directory.join(&format!("{name}.o"));
        fs::write(&source, llvm).unwrap();
        let compile = Command::new(&llc)
            .args([
                "-mtriple=amdgcn-amd-amdhsa",
                "-mcpu=gfx942",
                "--amdhsa-code-object-version=6",
                "-filetype=obj",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "llc rejected {name}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        assert!(!fs::read(&object).unwrap().is_empty());

        if self_contained {
            let hsaco = directory.join(&format!("{name}.hsaco"));
            let link = Command::new(&lld)
                .arg("-shared")
                .arg(&object)
                .arg("-o")
                .arg(&hsaco)
                .output()
                .unwrap();
            assert!(
                link.status.success(),
                "lld rejected {name}: {}",
                String::from_utf8_lossy(&link.stderr)
            );
            let inspect = Command::new(&readelf)
                .args(["--notes"])
                .arg(&hsaco)
                .output()
                .unwrap();
            assert!(inspect.status.success());
            let notes = String::from_utf8(inspect.stdout).unwrap();
            assert!(notes.contains(".name:           math_kernel"));
            assert!(notes.contains(".wavefront_size: 64"));
            assert!(notes.contains(".max_flat_workgroup_size: 64"));
        }
    }
}
