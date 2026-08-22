use fe2o3_amdgcn_model::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, TiledGemmLdsLoweringErrorV1,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::{
    MatrixOperationKind, OperationKind, TILED_GEMM_LDS_V1_KERNEL_ID, TiledGemmLdsV1Error,
    TiledGemmLdsV1Profile, WaveWidth, tiled_gemm_lds_v1_module,
};

fn profile() -> TiledGemmLdsV1Profile {
    TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6()
}

#[test]
fn lowers_only_the_canonical_lds_graph_to_exact_gfx942_llvm() {
    let expected_profile = profile();
    let output = lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
        &tiled_gemm_lds_v1_module(),
        expected_profile.clone(),
    )
    .expect("canonical tiled GEMM LDS LLVM");
    assert_eq!(output.profile(), &expected_profile);
    let llvm = output.as_str();

    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT), "{llvm}");
    let definition = llvm
        .lines()
        .find(|line| line.starts_with("define amdgpu_kernel"))
        .expect("canonical Slice1 definition");
    assert_eq!(
        definition,
        "define amdgpu_kernel void @tiled_gemm_lds_v1(ptr addrspace(1) noalias nocapture readonly align 2 %arg0.data, i64 %arg0.len, ptr addrspace(1) noalias nocapture readonly align 2 %arg1.data, i64 %arg1.len, ptr addrspace(1) noalias nocapture align 4 %arg2.data, i64 %arg2.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {",
        "{llvm}"
    );
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "!0 = !{i32 64, i32 1, i32 1}",
        "!1 = !{!\"read_only\", !\"none\", !\"read_only\", !\"none\", !\"read_write\", !\"none\"}",
        "!2 = !{!\"ushort*\", !\"ulong\", !\"ushort*\", !\"ulong\", !\"float*\", !\"ulong\"}",
        "!3 = !{!\"const\", !\"\", !\"const\", !\"\", !\"restrict\", !\"\"}",
        "fence syncscope(\"workgroup\") release",
        "call void asm sideeffect \"s_barrier\", \"\"()",
        "fence syncscope(\"workgroup\") acquire",
        "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(",
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }

    let lds_tiles = llvm
        .lines()
        .filter(|line| {
            line.starts_with('@')
                && line.contains("internal addrspace(3) global [256 x i16] undef, align 16")
        })
        .collect::<Vec<_>>();
    assert_eq!(lds_tiles.len(), 2, "{llvm}");
    assert_ne!(lds_tiles[0], lds_tiles[1], "{llvm}");
    assert_eq!(llvm.matches("store i16 ").count(), 8, "{llvm}");
    assert_eq!(
        llvm.matches(" = load i16, ptr addrspace(3)").count(),
        8,
        "{llvm}"
    );
    assert_eq!(
        llvm.matches("call void asm sideeffect \"s_barrier\", \"\"()")
            .count(),
        1,
        "{llvm}"
    );
    assert_eq!(
        llvm.matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1,
        "{llvm}"
    );
    assert_eq!(llvm.matches("store float ").count(), 4, "{llvm}");

    for forbidden in [
        "atomicrmw",
        "cmpxchg",
        "@__ocml_",
        "@__ockl_",
        "comgr",
        "COMGR",
        "writeonly align 4 %arg2.data",
    ] {
        assert!(!llvm.contains(forbidden), "found {forbidden:?}\n{llvm}");
    }

    assert_eq!(definition.matches("noalias nocapture").count(), 3);
    assert_eq!(definition.matches("readonly align 2").count(), 2);
    assert_eq!(definition.matches("align 4 %arg2.data").count(), 1);
    for attachment in [
        "!kernel_arg_access_qual !1",
        "!kernel_arg_type !2",
        "!kernel_arg_base_type !2",
        "!kernel_arg_type_qual !3",
    ] {
        assert_eq!(llvm.matches(attachment).count(), 1, "{attachment}\n{llvm}");
    }
}

#[test]
fn rejects_every_nonexact_lds_profile_before_lowering() {
    let mut profiles = Vec::new();

    let mut cov5 = profile();
    cov5.code_object_version = 5;
    profiles.push(cov5);

    let mut wave32 = profile();
    wave32.wave_width = WaveWidth::Wave32;
    profiles.push(wave32);

    let mut wrong_workgroup = profile();
    wrong_workgroup.workgroup_size.x = 128;
    profiles.push(wrong_workgroup);

    let mut one_allocation = profile();
    one_allocation.lds_allocations = 1;
    profiles.push(one_allocation);

    let mut wrong_alignment = profile();
    wrong_alignment.lds_alignment = 8;
    profiles.push(wrong_alignment);

    let mut zero_lds = profile();
    zero_lds.static_lds_bytes = 0;
    profiles.push(zero_lds);

    for nonexact in profiles {
        assert!(matches!(
            lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(&tiled_gemm_lds_v1_module(), nonexact),
            Err(TiledGemmLdsLoweringErrorV1::Profile(
                TiledGemmLdsV1Error::UnsupportedProfile
            ))
        ));
    }
}

#[test]
fn rejects_aliasing_tiles_alignment_drift_and_barrier_removal() {
    let mut aliasing = tiled_gemm_lds_v1_module();
    let operations = &mut aliasing.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let first_lds = operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::WorkgroupMemory(_)))
        .unwrap()
        .results[0]
        .id;
    let second_store = operations
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => match &mut matrix.kind {
                MatrixOperationKind::LdsStore { base, .. } => Some(base),
                _ => None,
            },
            _ => None,
        })
        .nth(1)
        .unwrap();
    *second_store = first_lds;
    assert!(matches!(
        lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(&aliasing, profile()),
        Err(TiledGemmLdsLoweringErrorV1::Profile(
            TiledGemmLdsV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut weak_alignment = tiled_gemm_lds_v1_module();
    let second_tile = weak_alignment.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .nth(1)
        .unwrap();
    second_tile.alignment = 8;
    assert!(matches!(
        lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(&weak_alignment, profile()),
        Err(TiledGemmLdsLoweringErrorV1::Profile(
            TiledGemmLdsV1Error::NonCanonicalKernelIr
        ))
    ));

    let mut no_barrier = tiled_gemm_lds_v1_module();
    no_barrier.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .retain(|operation| !matches!(operation.kind, OperationKind::WorkgroupBarrier(_)));
    assert!(matches!(
        lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(&no_barrier, profile()),
        Err(TiledGemmLdsLoweringErrorV1::Profile(_))
    ));
}

#[test]
fn generic_exact_target_lowering_selects_integer_ops_without_workload_knowledge() {
    let llvm = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &tiled_gemm_lds_v1_module(),
        &TILED_GEMM_LDS_V1_KERNEL_ID.into(),
    )
    .expect("generic gfx942 lowering selects ordinary integer operations by type");
    assert!(llvm.contains("urem i64"));
    assert!(llvm.contains("udiv i64"));
    assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
}
