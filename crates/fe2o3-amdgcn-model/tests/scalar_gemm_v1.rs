use fe2o3_amdgcn_model::{
    GFX942_UPSTREAM_LLVM_DATA_LAYOUT_V1, ScalarGemmLoweringErrorV1,
    lower_scalar_gemm_v1_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn requirements() -> ScalarGemmTargetRequirementsV1 {
    ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6()
}

#[test]
fn lowers_exact_cyclic_ssa_gemm_to_strict_gfx942_llvm() {
    let output = lower_scalar_gemm_v1_to_gfx942_llvm_ir(&scalar_gemm_v1_module(), requirements())
        .expect("canonical scalar GEMM LLVM");
    assert_eq!(output.requirements(), requirements());
    let llvm = output.as_str();
    assert!(llvm.starts_with(&format!(
        "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{GFX942_UPSTREAM_LLVM_DATA_LAYOUT_V1}\"\n"
    )));

    assert!(llvm.contains(concat!(
        "define amdgpu_kernel void @scalar_gemm_v1(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len, ",
        "ptr addrspace(1) %arg1.data, i64 %arg1.len, ",
        "ptr addrspace(1) %arg2.data, i64 %arg2.len, ",
        "i32 %arg3, i32 %arg4, i32 %arg5)"
    )));

    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "target datalayout = \"e-m:e-p:64:64-p1:64:64",
        "define amdgpu_kernel void @scalar_gemm_v1(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len",
        "i32 %arg3, i32 %arg4, i32 %arg5",
        "%v19 = phi i32",
        "%v20 = phi float",
        "%v15 = udiv i64 %v6, %v8",
        "%v16 = urem i64 %v6, %v8",
        "%v31 = fmul float %v28, %v30",
        "%v32 = fadd float %v20, %v31",
        "store float %v35, ptr addrspace(1) %v36, align 4",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"fp-contract\"=\"off\"",
        "\"amdgpu-flat-work-group-size\"=\"256,256\"",
        "!0 = !{i32 256, i32 1, i32 1}",
        "%v10 = mul i64 %v7, %v8",
        "br i1 %v11, label %bb1, label %bb5",
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }
    assert_eq!(llvm.matches("load float").count(), 2, "{llvm}");
    assert_eq!(llvm.matches("store float").count(), 1, "{llvm}");
    assert_eq!(llvm.matches("define amdgpu_kernel").count(), 1, "{llvm}");
    assert!(!llvm.contains("@__fe2o3_scalar_gemm_v1_impl"), "{llvm}");
    let guard = llvm.find("br i1 %v11").unwrap();
    let division = llvm.find("%v15 = udiv i64 %v6, %v8").unwrap();
    assert!(
        guard < division,
        "division must remain below the active guard\n{llvm}"
    );
    for forbidden in [
        "llvm.fma",
        "llvm.fmuladd",
        " fast ",
        " contract ",
        " reassoc ",
        "comgr",
        "COMGR",
    ] {
        assert!(!llvm.contains(forbidden), "found {forbidden:?}\n{llvm}");
    }
}

#[test]
fn lowering_rejects_wrong_target_malformed_loop_and_noninjective_store() {
    let wrong_target = ScalarGemmTargetRequirementsV1 {
        architecture: ScalarGemmArchitectureV1::Other,
        ..requirements()
    };
    assert!(matches!(
        lower_scalar_gemm_v1_to_gfx942_llvm_ir(&scalar_gemm_v1_module(), wrong_target),
        Err(ScalarGemmLoweringErrorV1::Profile(
            ScalarGemmV1Error::UnsupportedTargetRequirements
        ))
    ));

    let mut malformed = scalar_gemm_v1_module();
    malformed.functions[0].body.as_mut().unwrap().blocks[3].terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![ValueId(32)],
    });
    assert!(matches!(
        lower_scalar_gemm_v1_to_gfx942_llvm_ir(&malformed, requirements()),
        Err(ScalarGemmLoweringErrorV1::Profile(
            ScalarGemmV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut noninjective = scalar_gemm_v1_module();
    let operation = &mut noninjective.functions[0].body.as_mut().unwrap().blocks[4].operations[0];
    let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind else {
        panic!("C GEP");
    };
    *offset = ValueId(16);
    assert!(matches!(
        lower_scalar_gemm_v1_to_gfx942_llvm_ir(&noninjective, requirements()),
        Err(ScalarGemmLoweringErrorV1::Profile(
            ScalarGemmV1Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn lowering_rejects_reordered_or_contraction_shaped_arithmetic_and_extra_roots() {
    let mut reordered = scalar_gemm_v1_module();
    let operations = &mut reordered.functions[0].body.as_mut().unwrap().blocks[3].operations;
    let OperationKind::Binary { lhs, rhs, .. } = &mut operations[10].kind else {
        panic!("canonical accumulation");
    };
    std::mem::swap(lhs, rhs);
    assert!(matches!(
        lower_scalar_gemm_v1_to_gfx942_llvm_ir(&reordered, requirements()),
        Err(ScalarGemmLoweringErrorV1::Profile(
            ScalarGemmV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut contraction = scalar_gemm_v1_module();
    contraction.functions.push(Function::external_import(
        "llvm.fma.f32",
        Signature::new(vec![Type::F32, Type::F32, Type::F32], vec![Type::F32]),
    ));
    assert!(matches!(
        lower_scalar_gemm_v1_to_gfx942_llvm_ir(&contraction, requirements()),
        Err(ScalarGemmLoweringErrorV1::Profile(
            ScalarGemmV1Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn generic_gfx942_lowering_supports_integer_divide_and_remainder_by_type() {
    fn assert_gfx942_accepts(module: &Module, opcode: &str) {
        let kernel = KernelId::new(SCALAR_GEMM_V1_KERNEL_ID);
        let outputs = [
            fe2o3_amdgcn_model::lower_device_module_to_gfx942_xnack_minus_llvm_ir(module).unwrap(),
            fe2o3_amdgcn_model::lower_kernel_to_gfx942_xnack_minus_llvm_ir(module, &kernel)
                .unwrap(),
        ];
        for output in outputs {
            assert!(output.contains(opcode), "missing {opcode} in:\n{output}");
        }
        assert!(fe2o3_amdgcn_model::lower_compiler_module_to_llvm_ir(module).is_err());
        assert!(fe2o3_amdgcn_model::lower_kernel_to_llvm_ir(module, &kernel).is_err());
    }

    assert_gfx942_accepts(&scalar_gemm_v1_module(), "udiv i64");

    let mut remainder_only = scalar_gemm_v1_module();
    remainder_only.functions[0].body.as_mut().unwrap().blocks[1].operations[0].kind =
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(6),
            rhs: ValueId(8),
        };
    assert_gfx942_accepts(&remainder_only, "urem i64");
}

#[test]
#[ignore = "requires upstream LLVM tools with gfx942 support"]
fn upstream_llvm_verifies_and_codegen_preserves_separate_mul_add() {
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT to upstream opt");
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC to upstream llc");
    let objdump =
        std::env::var("FE2O3_OBJDUMP").expect("set FE2O3_OBJDUMP to upstream llvm-objdump");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-scalar-gemm-v1-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("scalar-gemm-v1.ll");
    let object = directory.join("scalar-gemm-v1.o");
    let llvm = lower_scalar_gemm_v1_to_gfx942_llvm_ir(&scalar_gemm_v1_module(), requirements())
        .unwrap()
        .into_string();
    fs::write(&input, llvm).unwrap();

    let verify = Command::new(opt)
        .args(["-passes=verify", "-disable-output"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "upstream opt rejected scalar GEMM V1: {}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let compile = Command::new(llc)
        .args([
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-O2",
            "-filetype=obj",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "upstream llc rejected scalar GEMM V1: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let disassembly = Command::new(objdump)
        .args(["--disassemble", "--symbolize-operands"])
        .arg(&object)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&directory);
    assert!(
        disassembly.status.success(),
        "upstream llvm-objdump rejected scalar GEMM V1: {}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let text = String::from_utf8_lossy(&disassembly.stdout).to_ascii_lowercase();
    assert!(text.contains("scalar_gemm_v1"), "{text}");
    let recurrence_start = text.find("global_load_dword").expect("first A load");
    let recurrence_end = text[recurrence_start..]
        .find("global_store_dword")
        .map(|offset| recurrence_start + offset)
        .expect("C store after input loads");
    let recurrence = &text[recurrence_start..recurrence_end];
    let multiply = recurrence
        .find("v_mul_f32")
        .expect("separate GEMM multiply");
    let add = recurrence.find("v_add_f32").expect("separate GEMM add");
    assert!(
        multiply < add,
        "GEMM add must follow multiply:\n{recurrence}"
    );
    for forbidden in ["v_fma", "v_fmac", "v_mad", "v_mac"] {
        assert!(
            !recurrence.contains(forbidden),
            "found {forbidden:?} in GEMM recurrence:\n{recurrence}"
        );
    }
}
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
