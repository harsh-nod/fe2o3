use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir,
    lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-matrix-{}-{nonce}",
            std::process::id()
        ));
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

fn lds(result: u32, element: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(result),
            Type::pointer(
                element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element,
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 16,
        }),
    )
}

fn matrix_module() -> Module {
    let bf16 = Type::Scalar(ScalarType::Bf16);
    let mut next = 7;
    let mut matrix_operation = |matrix: MatrixOperation| {
        let results = matrix
            .result_types()
            .into_iter()
            .map(|ty| {
                let result = ValueDef::new(ValueId(next), ty);
                next += 1;
                result
            })
            .collect();
        Operation::new(results, OperationKind::Matrix(matrix))
    };

    let load_a = matrix_operation(MatrixOperation::lds_load(ValueId(4), MatrixElement::Bf16));
    let load_b = matrix_operation(MatrixOperation::lds_load(ValueId(5), MatrixElement::Bf16));
    let mma = matrix_operation(MatrixOperation::multiply_accumulate(
        [ValueId(7), ValueId(8), ValueId(9), ValueId(10)],
        [ValueId(11), ValueId(12), ValueId(13), ValueId(14)],
        [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
    ));
    let store = matrix_operation(MatrixOperation::lds_store(
        ValueId(6),
        [ValueId(15), ValueId(16), ValueId(17), ValueId(18)],
        MatrixElement::F32,
    ));

    let block = BasicBlock {
        id: BlockId(0),
        parameters: vec![],
        operations: vec![
            lds(4, bf16.clone()),
            lds(5, bf16),
            lds(6, Type::F32),
            load_a,
            load_b,
            mma,
            store,
        ],
        terminator: Some(Terminator::Return { values: vec![] }),
    };
    let mut function = Function::kernel_entry(
        "matrix_impl",
        Signature::new(vec![Type::F32; 4], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();

    let mut kernel = Kernel::new(
        "matrix_kernel",
        "matrix_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::gfx942_matrix");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn exact_gfx942_matrix_profile_lowers_mfma_and_xor4_lds() {
    let llvm = lower_kernel_to_gfx942_llvm_ir(&matrix_module(), &"matrix_kernel".into()).unwrap();

    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(llvm.contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64\""));
    assert!(llvm.contains("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(3)").count(), 8);
    assert_eq!(llvm.matches("store float ").count(), 4);
    assert_eq!(llvm.matches(" = xor i32 ").count(), 12);
    assert!(llvm.contains(" = shl i32 %matrix.0.3.swizzle.row, 2"));
    let compiler_module = lower_compiler_module_to_gfx942_llvm_ir(&matrix_module()).unwrap();
    assert_eq!(
        compiler_module
            .matches("llvm.amdgcn.mfma.f32.16x16x16bf16.1k")
            .count(),
        2
    );
}

#[test]
fn baseline_and_multi_wave_workgroups_fail_closed() {
    let module = matrix_module();
    let baseline = lower_kernel_to_llvm_ir(&module, &"matrix_kernel".into()).unwrap_err();
    assert!(baseline.contains(LoweringDiagnosticCode::UnsupportedCapability));

    let mut multi_wave = module;
    multi_wave.kernels[0].workgroup_size = Some(WorkgroupSize::new(128, 1, 1));
    let errors = lower_kernel_to_gfx942_llvm_ir(&multi_wave, &"matrix_kernel".into()).unwrap_err();
    assert!(errors.contains(LoweringDiagnosticCode::UnsupportedMatrixOperation));
    assert!(errors.to_string().contains("exactly one full wave64"));
}

#[test]
fn divergent_matrix_placement_is_rejected() {
    let mut module = matrix_module();
    let function = &mut module.functions[0];
    function
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::Bool));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(19));
    let matrix_operations = body.blocks[0].operations.split_off(3);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(19),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    body.blocks.push(BasicBlock {
        id: BlockId(1),
        parameters: vec![],
        operations: matrix_operations,
        terminator: Some(Terminator::Return { values: vec![] }),
    });
    body.blocks.push(BasicBlock {
        id: BlockId(2),
        parameters: vec![],
        operations: vec![],
        terminator: Some(Terminator::Return { values: vec![] }),
    });

    let errors = lower_kernel_to_gfx942_llvm_ir(&module, &"matrix_kernel".into()).unwrap_err();
    assert!(errors.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence));
    assert!(errors.to_string().contains("convergent matrix"));
}

#[test]
#[ignore = "requires ROCm LLVM tools with gfx942 support"]
fn rocm_compiles_and_inspects_gfx942_matrix_object() {
    let directory = TemporaryDirectory::new();
    let input = directory.join("matrix.ll");
    let object = directory.join("matrix.o");
    fs::write(
        &input,
        lower_kernel_to_gfx942_llvm_ir(&matrix_module(), &"matrix_kernel".into()).unwrap(),
    )
    .unwrap();

    let compile = Command::new("/opt/rocm/llvm/bin/clang")
        .args([
            "-x",
            "ir",
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-mcode-object-version=6",
            "-nogpulib",
            "-c",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "clang failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let disassembly = Command::new("/opt/rocm/llvm/bin/llvm-objdump")
        .args(["-d", "--mcpu=gfx942"])
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        disassembly.status.success(),
        "llvm-objdump failed:\n{}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let disassembly = String::from_utf8_lossy(&disassembly.stdout);
    assert!(
        disassembly.contains("v_mfma_f32_16x16x16_bf16"),
        "missing gfx942 MFMA instruction:\n{disassembly}"
    );
}
