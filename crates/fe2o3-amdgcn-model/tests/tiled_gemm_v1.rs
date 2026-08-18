use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, TiledGemmLoweringErrorV1,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_tiled_gemm_v1_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn profile() -> TiledGemmV1Profile {
    TiledGemmV1Profile::exact_gfx942_xnack_minus_cov6()
}

#[test]
fn lowers_only_the_canonical_tiled_graph_to_exact_gfx942_llvm() {
    let expected_profile = profile();
    let output =
        lower_tiled_gemm_v1_to_gfx942_llvm_ir(&tiled_gemm_v1_module(), expected_profile.clone())
            .expect("canonical tiled GEMM LLVM");
    assert_eq!(output.profile(), &expected_profile);
    let llvm = output.as_str();

    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT), "{llvm}");
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @tiled_gemm_v1(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len",
        "ptr addrspace(1) %arg1.data, i64 %arg1.len",
        "ptr addrspace(1) %arg2.data, i64 %arg2.len",
        "ptr addrspace(1) %arg3.data, i64 %arg3.len",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
        "\"unsafe-fp-math\"=\"false\"",
        "\"fp-contract\"=\"off\"",
        "!0 = !{i32 64, i32 1, i32 1}",
        "urem i64",
        "udiv i64",
        "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(",
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }

    assert_eq!(llvm.matches("define amdgpu_kernel").count(), 1, "{llvm}");
    assert_eq!(llvm.matches("urem i64").count(), 1, "{llvm}");
    assert_eq!(llvm.matches("udiv i64").count(), 1, "{llvm}");
    assert_eq!(
        llvm.matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1,
        "{llvm}"
    );
    assert_eq!(
        llvm.matches(" = load i16, ptr addrspace(1)").count(),
        8,
        "{llvm}"
    );
    assert_eq!(
        llvm.matches(" = load float, ptr addrspace(1)").count(),
        4,
        "{llvm}"
    );
    assert_eq!(llvm.matches("store float ").count(), 4, "{llvm}");

    for (component, result) in ["%v59", "%v60", "%v61", "%v62"].into_iter().enumerate() {
        let extraction = format!("{result} = extractelement <4 x float> %matrix.");
        let extraction_suffix = format!(".mfma, i64 {component}");
        assert!(
            llvm.lines()
                .any(|line| line.contains(&extraction) && line.contains(&extraction_suffix)),
            "missing MFMA result component {component}\n{llvm}"
        );
        assert!(
            llvm.contains(&format!("store float {result}, ptr addrspace(1)")),
            "MFMA result {result} is not globally observable\n{llvm}"
        );
    }

    for forbidden in [
        "call fast ",
        " fadd ",
        " fsub ",
        " fmul ",
        " fdiv ",
        " frem ",
        " reassoc ",
        " nnan ",
        " ninf ",
        " nsz ",
        " arcp ",
        " contract ",
        " afn ",
        "addrspace(3)",
        "llvm.amdgcn.s.barrier",
        "comgr",
        "COMGR",
    ] {
        assert!(!llvm.contains(forbidden), "found {forbidden:?}\n{llvm}");
    }
}

#[test]
fn rejects_every_nonexact_profile_before_lowering() {
    let mut profiles = Vec::new();

    let mut cov5 = profile();
    cov5.code_object_version = 5;
    profiles.push(cov5);

    let mut wave32 = profile();
    wave32.wave_width = WaveWidth::Wave32;
    profiles.push(wave32);

    let mut wrong_workgroup = profile();
    wrong_workgroup.workgroup_size = WorkgroupSize::new(128, 1, 1);
    profiles.push(wrong_workgroup);

    let mut wrong_bridge = profile();
    wrong_bridge.bf16_bridge.bit_preserving = false;
    profiles.push(wrong_bridge);

    for nonexact in profiles {
        assert!(matches!(
            lower_tiled_gemm_v1_to_gfx942_llvm_ir(&tiled_gemm_v1_module(), nonexact),
            Err(TiledGemmLoweringErrorV1::Profile(
                TiledGemmV1Error::UnsupportedProfile
            ))
        ));
    }
}

#[test]
fn rejects_valid_but_noncanonical_kernel_ir() {
    let mut renamed = tiled_gemm_v1_module();
    renamed.id = "tests::renamed_tiled_gemm".into();
    assert!(matches!(
        lower_tiled_gemm_v1_to_gfx942_llvm_ir(&renamed, profile()),
        Err(TiledGemmLoweringErrorV1::Profile(
            TiledGemmV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut wrong_lane_map = tiled_gemm_v1_module();
    let operations = &mut wrong_lane_map.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::Binary { lhs, rhs, .. } = &mut operations[7].kind else {
        panic!("canonical lane remainder");
    };
    std::mem::swap(lhs, rhs);
    assert!(matches!(
        lower_tiled_gemm_v1_to_gfx942_llvm_ir(&wrong_lane_map, profile()),
        Err(TiledGemmLoweringErrorV1::Profile(
            TiledGemmV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut detached_store = tiled_gemm_v1_module();
    let operations = &mut detached_store.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let store = operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Store { value, .. } => Some(value),
            _ => None,
        })
        .expect("canonical global store");
    *store = ValueId(25);
    assert!(matches!(
        lower_tiled_gemm_v1_to_gfx942_llvm_ir(&detached_store, profile()),
        Err(TiledGemmLoweringErrorV1::Profile(
            TiledGemmV1Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn generic_gfx942_lowering_still_rejects_index_division_and_remainder() {
    let module = tiled_gemm_v1_module();
    let remainder = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &module,
        &KernelId::new(TILED_GEMM_V1_KERNEL_ID),
    )
    .expect_err("generic exact-target lowering must reject tiled index remainder");
    assert!(remainder.to_string().contains("does not lower Remainder"));

    let mut divide_only = module;
    let operations = &mut divide_only.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::Binary { op, .. } = &mut operations[7].kind else {
        panic!("canonical lane remainder");
    };
    *op = BinaryOp::Add;
    let divide = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &divide_only,
        &KernelId::new(TILED_GEMM_V1_KERNEL_ID),
    )
    .expect_err("generic exact-target lowering must reject tiled index division");
    assert!(divide.to_string().contains("does not lower Divide"));
}

#[test]
#[ignore = "requires an upstream LLVM opt with gfx942 MFMA support"]
fn upstream_llvm_o2_keeps_mfma_results_observable() {
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT to upstream opt");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-tiled-gemm-v1-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("tiled-gemm-v1.ll");
    let optimized = directory.join("tiled-gemm-v1-o2.ll");
    let llvm = lower_tiled_gemm_v1_to_gfx942_llvm_ir(&tiled_gemm_v1_module(), profile())
        .unwrap()
        .into_string();
    fs::write(&input, llvm).unwrap();

    let result = Command::new(opt)
        .args(["-S", "-passes=default<O2>"])
        .arg(&input)
        .arg("-o")
        .arg(&optimized)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "upstream opt rejected tiled GEMM V1: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let optimized = fs::read_to_string(&optimized).unwrap();
    let _ = fs::remove_dir_all(&directory);

    assert_eq!(
        optimized
            .matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k")
            .count(),
        1,
        "{optimized}"
    );
    assert_eq!(optimized.matches("store float ").count(), 4, "{optimized}");
}
