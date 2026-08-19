//! Positive scalar and GEMM-core lowering tests.

mod support;

use dialect_amdgcn::AmdgcnPlironLlvmProfileV1;
use fe2o3_lower_amdgcn_llvm::lower_amdgcn_to_pliron_llvm_v1;

#[test]
fn lowers_scalar_memory_arithmetic_into_a_live_verified_graph() {
    let source = support::scalar_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed scalar lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::ScalarMemoryArithmetic
    );
    assert_eq!(lowered.source_identity(), source.identity());
    assert_eq!(
        lowered.source_handoff().encode_canonical(),
        source.encode_canonical()
    );
    let inspection = lowered.inspect_live_graph().expect("live graph inspection");
    assert_eq!(inspection, lowered.construction_inspection());
    assert_eq!(inspection.function_count(), 1);
    assert_eq!(inspection.block_count(), 1);
    assert_eq!(inspection.block_argument_count(), 3);
    assert_eq!(inspection.operation_count(), 4);
    assert!(inspection.strict_float());
    assert!(inspection.exact_memory_alignment());
    assert!(!lowered.grants_artifact_authority());
}

#[test]
fn lowers_gemm_control_flow_phi_to_a_block_argument() {
    let source = support::gemm_control_flow_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed GEMM-core lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::ScalarControlFlowGemm
    );
    let inspection = lowered.inspect_live_graph().expect("live graph inspection");
    assert_eq!(inspection.function_count(), 1);
    assert_eq!(inspection.block_count(), 4);
    assert_eq!(inspection.block_argument_count(), 5);
    assert_eq!(inspection.operation_count(), 7);
    assert!(inspection.strict_float());
    assert!(inspection.exact_memory_alignment());
}

#[test]
fn lowers_tiled_data_representation_into_a_live_verified_graph() {
    let source = support::tiled_data_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed tiled-data lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::TiledDataRepresentationGemm
    );
    let inspection = lowered.inspect_live_graph().expect("live graph inspection");
    assert_eq!(inspection, lowered.construction_inspection());
    assert_eq!(inspection.global_count(), 2);
    assert_eq!(inspection.function_count(), 1);
    assert_eq!(inspection.block_count(), 1);
    assert_eq!(inspection.block_argument_count(), 3);
    assert_eq!(inspection.operation_count(), 14);
    assert!(inspection.strict_float());
    assert!(inspection.exact_memory_alignment());
}

#[test]
fn lowers_every_typed_intrinsic_declaration_and_call() {
    let source = support::intrinsic_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed intrinsic lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::TiledDataRepresentationGemm
    );
    let inspection = lowered.inspect_live_graph().expect("live graph inspection");
    assert_eq!(inspection, lowered.construction_inspection());
    assert_eq!(inspection.global_count(), 0);
    assert_eq!(inspection.intrinsic_count(), 11);
    assert_eq!(inspection.function_count(), 1);
    assert_eq!(inspection.block_count(), 1);
    assert_eq!(inspection.block_argument_count(), 3);
    assert_eq!(inspection.operation_count(), 21);
    assert!(inspection.strict_float());
    assert!(inspection.exact_memory_alignment());
}

#[test]
fn receipt_contains_the_exact_canonical_typed_source() {
    let source = support::scalar_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    let source_bytes = source.encode_canonical();
    assert!(
        lowered
            .receipt()
            .as_bytes()
            .windows(source_bytes.as_bytes().len())
            .any(|window| window == source_bytes.as_bytes())
    );
    assert_ne!(lowered.receipt().identity().as_bytes(), [0; 32]);
}
