//! Exact-target tests for append-only production compiler-module admission.

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
    ProductionGfx950CompilerFfiEnvelopeKindV1, construct_production_gfx950_ocml_exp_envelope_v1,
};
use fe2o3_llvm_worker_handoff::{
    MeasuredLlvmLldBuildV1, WorkerAdmissionErrorV3, WorkerAdmissionRequestV3,
};
use sha2::{Digest as _, Sha256};

const LLVM_IR: &[u8] =
    b"target triple = \"amdgcn-amd-amdhsa\"\ndefine amdgpu_kernel void @kernel() { ret void }\n";

fn handoff(target: &str, code_object: CodeObjectVersion) -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse(target)
        .unwrap_or_else(|error| panic!("invalid test target {target:?}: {error}"));
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, code_object).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        code_object,
        envelope,
        manifest,
        LLVM_IR,
    )
    .unwrap()
}

fn gfx950_ocml_handoff(module: &[u8]) -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx950:xnack-").unwrap();
    let envelope = construct_production_gfx950_ocml_exp_envelope_v1([0x38; 32]).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            "__ocml_exp_f32",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        module,
    )
    .unwrap()
}

fn admit(
    handoff: &CompilerModuleHandoffV2,
) -> Result<fe2o3_llvm_worker_handoff::AdmittedWorkerRequestV3, WorkerAdmissionErrorV3> {
    WorkerAdmissionRequestV3::new(
        handoff.canonical_bytes(),
        *handoff.identity().sha256(),
        MeasuredLlvmLldBuildV1::exact(),
    )
    .admit()
}

#[test]
fn exact_gfx942_and_gfx950_targets_survive_admission_without_relabeling() {
    for (target, expected_profile) in [
        ("gfx942:xnack-", ProductionAmdTargetProfileV1::Gfx942),
        ("gfx950:xnack-", ProductionAmdTargetProfileV1::Gfx950),
    ] {
        let handoff = handoff(target, CodeObjectVersion::V6);
        let admitted = admit(&handoff).unwrap();

        assert_eq!(admitted.target_profile(), expected_profile);
        assert_eq!(admitted.handoff().target().to_string(), target);
        assert_eq!(admitted.handoff_identity(), handoff.identity());
        assert_eq!(
            admitted.handoff().canonical_bytes(),
            handoff.canonical_bytes()
        );
        assert!(!admitted.authenticates_worker_measurement());
        assert!(!admitted.grants_object_authority());
        assert!(!admitted.grants_link_authority());
        assert!(!admitted.grants_publication_authority());
        assert!(!admitted.grants_load_authority());
        assert!(!admitted.grants_launch_authority());
    }
}

#[test]
fn feature_and_processor_substitutions_fail_closed() {
    for target in [
        "gfx942:xnack+",
        "gfx942:sramecc+:xnack-",
        "gfx950:xnack+",
        "gfx950:sramecc+:xnack-",
        "gfx1100",
    ] {
        let handoff = handoff(target, CodeObjectVersion::V6);
        assert_eq!(
            admit(&handoff),
            Err(WorkerAdmissionErrorV3::TargetPolicySubstitution),
            "unexpected admission for {target}"
        );
    }
}

#[test]
fn cross_target_identity_and_wire_relabeling_fail_closed() {
    let gfx942 = handoff("gfx942:xnack-", CodeObjectVersion::V6);
    let gfx950 = handoff("gfx950:xnack-", CodeObjectVersion::V6);
    assert_eq!(
        WorkerAdmissionRequestV3::new(
            gfx942.canonical_bytes(),
            *gfx950.identity().sha256(),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .admit(),
        Err(WorkerAdmissionErrorV3::HandoffIdentityMismatch)
    );

    let mut relabeled = gfx942.canonical_bytes().to_vec();
    let target = relabeled
        .windows(b"gfx942:xnack-".len())
        .position(|window| window == b"gfx942:xnack-")
        .unwrap();
    relabeled[target..target + b"gfx950:xnack-".len()].copy_from_slice(b"gfx950:xnack-");
    let identity: [u8; 32] = Sha256::digest(&relabeled).into();
    assert!(matches!(
        WorkerAdmissionRequestV3::new(&relabeled, identity, MeasuredLlvmLldBuildV1::exact(),)
            .admit(),
        Err(WorkerAdmissionErrorV3::Decode(_))
    ));
}

#[test]
fn code_object_version_is_exactly_v6() {
    let handoff = handoff("gfx950:xnack-", CodeObjectVersion::V5);
    assert_eq!(
        admit(&handoff),
        Err(WorkerAdmissionErrorV3::CodeObjectVersionSubstitution)
    );
}

#[test]
fn exact_gfx950_ocml_exp_import_is_retained_without_link_authority() {
    let handoff = gfx950_ocml_handoff(
        b"target triple = \"amdgcn-amd-amdhsa\"\n\
          declare float @__ocml_exp_f32(float)\n\
          define amdgpu_kernel void @kernel() {\n\
          entry:\n\
            %value = call float @__ocml_exp_f32(float 0.000000e+00)\n\
            ret void\n\
          }\n",
    );
    let admitted = admit(&handoff).unwrap();
    assert_eq!(
        admitted.gfx950_compiler_ffi_kind(),
        Some(ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 {
            canonical_kernel_ir_identity: [0x38; 32],
        })
    );
    assert!(!admitted.grants_link_authority());
}

#[test]
fn gfx950_ocml_envelope_without_exact_declaration_and_call_fails_closed() {
    for module in [
        LLVM_IR,
        b"target triple = \"amdgcn-amd-amdhsa\"\n\
          declare float @__ocml_exp2_f32(float)\n\
          define amdgpu_kernel void @kernel() { ret void }\n"
            .as_slice(),
    ] {
        assert_eq!(
            admit(&gfx950_ocml_handoff(module)),
            Err(WorkerAdmissionErrorV3::Gfx950DeviceFfiPolicySubstitution)
        );
    }
}

#[test]
fn gfx950_no_ffi_envelope_cannot_hide_an_ocml_import_in_llvm() {
    let target = DeviceTargetV1::parse("gfx950:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
    ])
    .unwrap();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        b"target triple = \"amdgcn-amd-amdhsa\"\n\
          declare float @__ocml_exp_f32(float)\n\
          define amdgpu_kernel void @kernel() { ret void }\n",
    )
    .unwrap();
    assert_eq!(
        admit(&handoff),
        Err(WorkerAdmissionErrorV3::Gfx950DeviceFfiPolicySubstitution)
    );
}
