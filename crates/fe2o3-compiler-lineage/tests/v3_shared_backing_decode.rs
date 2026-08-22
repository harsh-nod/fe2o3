use std::{ffi::OsString, sync::Arc};

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::DeviceTargetV1;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV2,
};

const TARGET: &str = "gfx942:sramecc+:xnack-";

fn invocation(seed: u8) -> RustcInvocationDescriptorV3 {
    let pins = [
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
        [seed.wrapping_add(6); 32],
    ];
    let closure = CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5])
        .expect("nonzero fixture closure");
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "shared_backing_fixture".into(),
            "crates/shared-backing-fixture/src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .expect("valid rustc unit");
    let environment = CompileEnvironmentV2::from_child_environment([
        (
            OsString::from("CARGO_CFG_TARGET_ARCH"),
            OsString::from("amdgcn"),
        ),
        (OsString::from("FE2O3_TARGET"), OsString::from(TARGET)),
        (
            OsString::from("FE2O3_HSACO_DIR"),
            OsString::from("/workspace/fe2o3/target/fe2o3"),
        ),
        (
            OsString::from("FE2O3_VERIFY_KERNEL_IR"),
            OsString::from("1"),
        ),
    ])
    .expect("valid exact environment");
    let v2 = RustcInvocationDescriptorV2::new(pins[3], pins[5], rustc, environment)
        .expect("valid V2 invocation");
    RustcInvocationDescriptorV3::new(v2, closure).expect("matching closure")
}

fn payload(label: &str, seed: u8) -> Vec<u8> {
    format!("fe2o3/shared-backing/{label}/seed-{seed:03}").into_bytes()
}

fn receipts(seed: u8) -> OrderedInertSemanticLineageReceiptsV3 {
    OrderedInertSemanticLineageReceiptsV3::new(
        InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(payload("inventory", seed))
            .unwrap(),
        InertRustcPreflightPlanReceiptV3::from_canonical_preimage(payload("preflight", seed))
            .unwrap(),
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(payload("mir", seed)).unwrap(),
        InertMiddleEndReceiptV3::from_canonical_preimage(payload("middle-end", seed)).unwrap(),
        InertKernelIrReceiptV3::from_canonical_preimage(payload("kir", seed)).unwrap(),
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(payload("mir-to-kir", seed))
            .unwrap(),
        InertFormalMemoryReceiptV3::from_canonical_preimage(payload("formal-memory", seed))
            .unwrap(),
        InertProofBindingReceiptV3::from_canonical_preimage(payload("proof-binding", seed))
            .unwrap(),
        InertTargetBindingReceiptV3::from_canonical_preimage(payload("target-binding", seed))
            .unwrap(),
        InertDataLayoutReceiptV3::from_canonical_preimage(payload("data-layout", seed)).unwrap(),
        InertAbiReceiptV3::from_canonical_preimage(payload("abi", seed)).unwrap(),
        InertExportManifestReceiptV3::from_canonical_preimage(payload("exports", seed)).unwrap(),
        InertAmdgpuLoweringReceiptV3::from_canonical_preimage(payload("amdgpu", seed)).unwrap(),
        InertSemanticToLlvmReceiptV3::from_canonical_preimage(payload("llvm", seed)).unwrap(),
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(payload(
            "final-module",
            seed,
        ))
        .unwrap(),
    )
}

fn capsule(seed: u8) -> InertProductionSemanticCapsuleV3 {
    InertProductionSemanticCapsuleV3::new(
        invocation(seed),
        DeviceTargetV1::parse(TARGET).unwrap(),
        receipts(seed),
    )
    .expect("valid fixture capsule")
}

fn byte_range(start: usize, end: usize) -> std::ops::Range<usize> {
    start..end
}

fn receipt_preimages(capsule: &InertProductionSemanticCapsuleV3) -> [&[u8]; 15] {
    let receipts = capsule.receipts();
    [
        receipts.rustc_identity_inventory().canonical_preimage(),
        receipts.rustc_preflight_plan().canonical_preimage(),
        receipts.semantic_mir().canonical_preimage(),
        receipts.middle_end().canonical_preimage(),
        receipts.kernel_ir().canonical_preimage(),
        receipts.mir_to_kir_correspondence().canonical_preimage(),
        receipts.formal_memory().canonical_preimage(),
        receipts.proof_binding().canonical_preimage(),
        receipts.target_binding().canonical_preimage(),
        receipts.data_layout().canonical_preimage(),
        receipts.abi().canonical_preimage(),
        receipts.export_manifest().canonical_preimage(),
        receipts.amdgpu_lowering().canonical_preimage(),
        receipts.semantic_to_llvm().canonical_preimage(),
        receipts
            .final_compiler_module_commitment()
            .canonical_preimage(),
    ]
}

#[test]
fn shared_decode_retains_exact_outer_backing_and_preserves_identity() {
    let constructed = capsule(0x41);
    let canonical = constructed.canonical_bytes().to_vec();
    let borrowed = InertProductionSemanticCapsuleV3::decode(&canonical).unwrap();

    let prefix = 37;
    let mut outer = vec![0xa5; prefix];
    outer.extend_from_slice(&canonical);
    outer.extend_from_slice(&[0x5a; 19]);
    let range = prefix..prefix.checked_add(canonical.len()).unwrap();
    let backing: Arc<[u8]> = outer.into();
    let decoded =
        InertProductionSemanticCapsuleV3::decode_shared(backing.clone(), range.clone()).unwrap();

    assert_eq!(decoded, constructed);
    assert_eq!(decoded, borrowed);
    assert_eq!(decoded.identity(), constructed.identity());
    assert_eq!(decoded.canonical_bytes(), canonical);
    assert!(decoded.identity().matches_canonical_bytes(&canonical));
    assert!(std::ptr::eq(
        decoded.canonical_bytes().as_ptr(),
        backing[range.clone()].as_ptr()
    ));

    let canonical_start = backing[range.clone()].as_ptr() as usize;
    let canonical_end = canonical_start.checked_add(range.len()).unwrap();
    for preimage in receipt_preimages(&decoded) {
        let start = preimage.as_ptr() as usize;
        let end = start.checked_add(preimage.len()).unwrap();
        assert!(start >= canonical_start);
        assert!(end <= canonical_end);
    }

    drop(backing);
    assert_eq!(decoded.canonical_bytes(), canonical);
    assert_eq!(receipt_preimages(&decoded).len(), 15);
}

#[test]
fn hostile_shared_ranges_are_rejected_without_index_arithmetic_wraparound() {
    let canonical = capsule(0x52).canonical_bytes().to_vec();
    let backing: Arc<[u8]> = canonical.clone().into();

    assert_eq!(
        InertProductionSemanticCapsuleV3::decode_shared(backing.clone(), byte_range(usize::MAX, 0),),
        Err(LineageDecodeErrorV3::InvalidLength(u64::MAX))
    );
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode_shared(backing.clone(), 0..usize::MAX),
        Err(LineageDecodeErrorV3::Truncated)
    );
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode_shared(backing.clone(), usize::MAX..usize::MAX,),
        Err(LineageDecodeErrorV3::Truncated)
    );
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode_shared(
            backing,
            0..canonical.len().checked_sub(1).unwrap(),
        ),
        Err(LineageDecodeErrorV3::Truncated)
    );
}

#[test]
fn shared_decode_documents_the_tight_successful_retained_payload_bound() {
    assert_eq!(
        InertProductionSemanticCapsuleV3::MAX_SUCCESSFUL_DECODE_RETAINED_BYTES,
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + MAX_DESCRIPTOR_BYTES_V3
    );
}
