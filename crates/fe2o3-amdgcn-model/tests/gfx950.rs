use fe2o3_amd_target::AmdTargetId;
use fe2o3_amdgcn_model::{
    Gfx950LdsTranspose, Gfx950LoweringError, Gfx950MfmaFormat, Gfx950ScaledMfma,
    lower_gfx950_lds_transpose_to_llvm_ir, lower_gfx950_scaled_mfma_to_llvm_ir,
};

fn gfx950() -> AmdTargetId {
    AmdTargetId::parse("gfx950:xnack-").unwrap()
}

#[test]
fn scaled_mfma_fp4_and_fp8_emit_exact_format_immediates() {
    let fp4 = lower_gfx950_scaled_mfma_to_llvm_ir(
        gfx950(),
        Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp4E2M1Ocp, Gfx950MfmaFormat::Fp4E2M1Ocp),
        4,
        1,
        2,
        3,
    )
    .unwrap();
    assert_eq!(
        fp4,
        "%4 = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32> %1, <8 x i32> %2, <4 x float> %3, i32 4, i32 4, i32 0, i32 0, i32 0, i32 0)"
    );

    let fp8 = lower_gfx950_scaled_mfma_to_llvm_ir(
        gfx950(),
        Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp8E4M3Ocp, Gfx950MfmaFormat::Fp8E5M2Ocp),
        8,
        5,
        6,
        7,
    )
    .unwrap();
    assert_eq!(
        fp8,
        "%8 = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32> %5, <8 x i32> %6, <4 x float> %7, i32 0, i32 1, i32 0, i32 0, i32 0, i32 0)"
    );
    assert_eq!(
        Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp4E2M1Ocp, Gfx950MfmaFormat::Fp4E2M1Ocp,)
            .declaration(),
        "declare <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32>, <8 x i32>, <4 x float>, i32 immarg, i32 immarg, i32 immarg, i32, i32 immarg, i32)"
    );
    assert_eq!(
        Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp8E4M3Ocp, Gfx950MfmaFormat::Fp8E5M2Ocp,)
            .declaration(),
        "declare <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32>, <8 x i32>, <4 x float>, i32 immarg, i32 immarg, i32 immarg, i32, i32 immarg, i32)"
    );

    let mixed = Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp4E2M1Ocp, Gfx950MfmaFormat::Fp8E4M3Ocp);
    assert_eq!(
        mixed.declaration(),
        "declare <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32>, <8 x i32>, <4 x float>, i32 immarg, i32 immarg, i32 immarg, i32, i32 immarg, i32)"
    );
    assert_eq!(
        lower_gfx950_scaled_mfma_to_llvm_ir(gfx950(), mixed, 12, 9, 10, 11).unwrap(),
        "%12 = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(<8 x i32> %9, <8 x i32> %10, <4 x float> %11, i32 4, i32 0, i32 0, i32 0, i32 0, i32 0)"
    );
}

#[test]
fn fp4_uses_the_v8i32_abi_with_four_required_zero_upper_dwords() {
    assert_eq!(Gfx950MfmaFormat::Fp4E2M1Ocp.register_dwords(), 8);
    assert_eq!(Gfx950MfmaFormat::Fp4E2M1Ocp.meaningful_register_dwords(), 4);
    assert_eq!(Gfx950MfmaFormat::Fp4E2M1Ocp.required_zero_upper_dwords(), 4);
    assert_eq!(Gfx950MfmaFormat::Fp8E4M3Ocp.register_dwords(), 8);
    assert_eq!(Gfx950MfmaFormat::Fp8E4M3Ocp.required_zero_upper_dwords(), 0);
}

#[test]
fn transpose_loads_emit_exact_format_specific_intrinsics() {
    for (operation, expected_call, expected_declaration) in [
        (
            Gfx950LdsTranspose::B4,
            "%2 = call <2 x i32> @llvm.amdgcn.ds.read.tr4.b64.v2i32(ptr addrspace(3) %1)",
            "declare <2 x i32> @llvm.amdgcn.ds.read.tr4.b64.v2i32(ptr addrspace(3) nocapture)",
        ),
        (
            Gfx950LdsTranspose::B8,
            "%2 = call <2 x i32> @llvm.amdgcn.ds.read.tr8.b64.v2i32(ptr addrspace(3) %1)",
            "declare <2 x i32> @llvm.amdgcn.ds.read.tr8.b64.v2i32(ptr addrspace(3) nocapture)",
        ),
        (
            Gfx950LdsTranspose::B16,
            "%2 = call <4 x i16> @llvm.amdgcn.ds.read.tr16.b64.v4i16(ptr addrspace(3) %1)",
            "declare <4 x i16> @llvm.amdgcn.ds.read.tr16.b64.v4i16(ptr addrspace(3) nocapture)",
        ),
    ] {
        assert_eq!(
            lower_gfx950_lds_transpose_to_llvm_ir(gfx950(), operation, 2, 1).unwrap(),
            expected_call
        );
        assert_eq!(operation.declaration(), expected_declaration);
    }
}

#[test]
fn gfx950_operations_fail_closed_on_gfx942() {
    let gfx942 = AmdTargetId::parse("gfx942:xnack-").unwrap();
    let error = lower_gfx950_scaled_mfma_to_llvm_ir(
        gfx942,
        Gfx950ScaledMfma::new(Gfx950MfmaFormat::Fp4E2M1Ocp, Gfx950MfmaFormat::Fp4E2M1Ocp),
        4,
        1,
        2,
        3,
    )
    .unwrap_err();
    assert_eq!(error, Gfx950LoweringError::UnsupportedTarget(gfx942));
    assert_eq!(
        lower_gfx950_lds_transpose_to_llvm_ir(gfx942, Gfx950LdsTranspose::B8, 2, 1).unwrap_err(),
        Gfx950LoweringError::UnsupportedTarget(gfx942)
    );
}
