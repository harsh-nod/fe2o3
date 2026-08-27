//! Positive scalar, control-flow, vector, and local-memory lowering tests.

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
    assert_ne!(lowered.non_graph_envelope().identity().as_bytes(), [0; 32]);
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
fn lowers_control_flow_phi_to_a_block_argument() {
    let source = support::control_flow_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed control-flow lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::ScalarControlFlow
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
fn lowers_vector_and_local_memory_into_a_live_verified_graph() {
    let source = support::vector_local_data_handoff();
    let lowered =
        lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed vector/local-memory lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::VectorAndLocalMemory
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
fn lowers_multiple_bounded_vector_and_local_array_shapes() {
    for (lanes, elements) in [(4, 256), (4, 33), (8, 256), (8, 33), (16, 1024)] {
        let source = support::bounded_shape_handoff(lanes, elements);
        let lowered = lower_amdgcn_to_pliron_llvm_v1(&source)
            .unwrap_or_else(|error| panic!("shape ({lanes}, {elements}) failed: {error:?}"));
        assert_eq!(
            lowered.profile(),
            AmdgcnPlironLlvmProfileV1::VectorAndLocalMemory
        );
        let inspection = lowered.inspect_live_graph().unwrap();
        assert_eq!(inspection.global_count(), 1);
        assert_eq!(inspection.function_count(), 1);
        assert!(inspection.operation_count() > 4);
        assert_eq!(lowered.source_identity(), source.identity());
    }
}

#[test]
fn lowers_every_typed_intrinsic_declaration_and_call() {
    let source = support::intrinsic_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).expect("typed intrinsic lowering");
    assert_eq!(
        lowered.profile(),
        AmdgcnPlironLlvmProfileV1::VectorAndLocalMemory
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
