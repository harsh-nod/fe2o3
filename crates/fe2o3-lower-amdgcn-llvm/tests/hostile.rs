//! Hostile typed-substitution and fail-closed admission tests.

mod support;

use dialect_amdgcn::{AmdgcnPlironLlvmRejectionV1, UnsupportedGlobalV1};
use fe2o3_llvm_handoff::{
    CallingConventionV2, Gfx942HandoffV2, NamedMetadataV1, ObligationKindV1, OriginKindV1,
    ScalarTypeV1, ValueTypeV2,
};
use fe2o3_lower_amdgcn_llvm::{LoweringErrorV1, lower_amdgcn_to_pliron_llvm_v1};

fn rejection(source: &Gfx942HandoffV2) -> AmdgcnPlironLlvmRejectionV1 {
    match lower_amdgcn_to_pliron_llvm_v1(source) {
        Err(LoweringErrorV1::Admission(rejection)) => rejection,
        Err(other) => panic!("unexpected non-admission failure: {other}"),
        Ok(_) => panic!("hostile source was admitted"),
    }
}

#[test]
fn rejects_substituted_calling_convention_before_graph_construction() {
    assert_eq!(
        rejection(&support::handoff_with_helper_c_calling_convention()),
        AmdgcnPlironLlvmRejectionV1::UnsupportedCallingConvention(CallingConventionV2::C)
    );
}

#[test]
fn rejects_substituted_metadata_origin_and_obligation_with_named_categories() {
    assert_eq!(
        rejection(&support::handoff_with_named_metadata(
            NamedMetadataV1::OpenClSpirVersion2_0
        )),
        AmdgcnPlironLlvmRejectionV1::UnsupportedNamedMetadata(
            NamedMetadataV1::OpenClSpirVersion2_0
        )
    );
    assert_eq!(
        rejection(&support::handoff_with_origin(OriginKindV1::RustSource)),
        AmdgcnPlironLlvmRejectionV1::UnsupportedOrigin(OriginKindV1::RustSource)
    );
    assert_eq!(
        rejection(&support::handoff_missing_obligation(
            ObligationKindV1::PreserveTargetFeatures
        )),
        AmdgcnPlironLlvmRejectionV1::MissingObligation(ObligationKindV1::PreserveTargetFeatures)
    );
}

#[test]
fn rejects_unsupported_type_and_tiled_gemm_global_with_named_categories() {
    assert_eq!(
        rejection(&support::handoff_with_f64_parameter()),
        AmdgcnPlironLlvmRejectionV1::UnsupportedValueType(ValueTypeV2::Scalar(ScalarTypeV1::F64))
    );
    assert_eq!(
        rejection(&support::handoff_with_scalar_global()),
        AmdgcnPlironLlvmRejectionV1::UnsupportedGlobal(UnsupportedGlobalV1::Scalar)
    );
}

#[test]
fn corrupted_canonical_target_or_policy_bytes_fail_at_the_typed_source_gate() {
    let source = support::scalar_handoff();
    let mut bytes = source.encode_canonical().as_bytes().to_vec();
    let index = bytes.len() / 3;
    bytes[index] ^= 0x80;
    assert!(Gfx942HandoffV2::decode_canonical(&bytes).is_err());
}
