use dialect_amdgcn::{ScalarGemmLoweringErrorV1, lower_scalar_gemm_v1_to_gfx942_llvm_ir};
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

    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "target datalayout = \"e-p:64:64-p1:64:64",
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
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }
    assert_eq!(llvm.matches("load float").count(), 2, "{llvm}");
    assert_eq!(llvm.matches("store float").count(), 1, "{llvm}");
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
fn generic_lowering_still_rejects_integer_divide_and_remainder() {
    let error = dialect_amdgcn::lower_compiler_module_to_gfx942_llvm_ir(&scalar_gemm_v1_module())
        .expect_err("generic gfx942 profile must not inherit GEMM division support");
    assert!(
        error.to_string().contains("does not lower Divide"),
        "{error}"
    );
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
    for forbidden in ["v_fma", "v_fmac"] {
        assert!(
            !recurrence.contains(forbidden),
            "found {forbidden:?} in GEMM recurrence:\n{recurrence}"
        );
    }
}
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
