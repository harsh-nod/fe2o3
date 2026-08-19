#![forbid(unsafe_code)]

//! Public scalar Pliron LLVM lowering V1 conformance tests.

use fe2o3_amdgcn_model::AddressSpace;
use fe2o3_amdgcn_pliron_llvm::{
    LoweringDiagnosticV1, MAX_CANONICAL_RECEIPT_BYTES_V1, MAX_DIAGNOSTIC_BYTES_V1,
    ScalarKernelModuleV1, ScalarOperationV1, TargetFeaturePolicyV1, lower_scalar_kernel_v1,
};
use fe2o3_llvm_handoff::{IdentityV1, ScalarTypeV1, StageIdentitiesV1};
use fe2o3_llvm_route_conformance::{
    ConformanceExpectationV1, ConformanceSemanticV1, ExpectedRejectionV1,
    GFX942_CONFORMANCE_CORPUS_V1, conformance_case_v1,
};

const EXERCISED_LOWERING_REJECTIONS: [ExpectedRejectionV1; 4] = [
    ExpectedRejectionV1::PlironLoweringUnsupportedCall,
    ExpectedRejectionV1::PlironLoweringUnsupportedType,
    ExpectedRejectionV1::PlironLoweringUnsupportedAddressSpace,
    ExpectedRejectionV1::PlironLoweringUnsupportedTargetPolicy,
];

#[test]
fn canonical_scalar_lane_produces_deterministic_handoff_and_receipt() {
    let case = conformance_case_v1("lane.pliron-lowering.canonical-deterministic")
        .expect("canonical lowering case must be declared");
    assert_eq!(case.expectation(), ConformanceExpectationV1::Represented);

    let request = scalar_request();
    let first = lower_scalar_kernel_v1(&request).expect("canonical lowering must succeed");
    let second = lower_scalar_kernel_v1(&request).expect("repeated lowering must succeed");

    assert_eq!(first.receipt(), second.receipt());
    assert!(!first.receipt().is_empty());
    assert!(first.receipt().len() <= MAX_CANONICAL_RECEIPT_BYTES_V1);
    assert_eq!(
        first.handoff().encode_canonical(),
        second.handoff().encode_canonical()
    );
    assert_eq!(first.handoff().identity(), second.handoff().identity());
    assert_eq!(
        first.handoff().stage_identities(),
        &request.stage_identities
    );
}

#[test]
fn unsupported_call_rejects_with_typed_bounded_diagnostic() {
    let mut request = scalar_request();
    request.operations[1] = ScalarOperationV1::Call;
    assert_lowering_rejection(
        "lane.pliron-lowering.unsupported-call",
        ExpectedRejectionV1::PlironLoweringUnsupportedCall,
        &request,
        LoweringDiagnosticV1::UnsupportedOperation(ScalarOperationV1::Call),
    );
}

#[test]
fn unsupported_type_rejects_with_typed_bounded_diagnostic() {
    let mut request = scalar_request();
    request.scalar_type = ScalarTypeV1::F64;
    assert_lowering_rejection(
        "lane.pliron-lowering.unsupported-type",
        ExpectedRejectionV1::PlironLoweringUnsupportedType,
        &request,
        LoweringDiagnosticV1::UnsupportedType(ScalarTypeV1::F64),
    );
}

#[test]
fn unsupported_address_space_rejects_with_typed_bounded_diagnostic() {
    let mut request = scalar_request();
    request.address_space = AddressSpace::BufferFatPointer;
    assert_lowering_rejection(
        "lane.pliron-lowering.unsupported-address-space",
        ExpectedRejectionV1::PlironLoweringUnsupportedAddressSpace,
        &request,
        LoweringDiagnosticV1::UnsupportedAddressSpace(AddressSpace::BufferFatPointer),
    );
}

#[test]
fn unsupported_target_policy_rejects_with_typed_bounded_diagnostic() {
    let mut request = scalar_request();
    request.target_policy = TargetFeaturePolicyV1::Gfx942Wave64XnackPlus;
    assert_lowering_rejection(
        "lane.pliron-lowering.unsupported-target-policy",
        ExpectedRejectionV1::PlironLoweringUnsupportedTargetPolicy,
        &request,
        LoweringDiagnosticV1::UnsupportedTargetPolicy(TargetFeaturePolicyV1::Gfx942Wave64XnackPlus),
    );
}

#[test]
fn every_declared_lowering_rejection_has_an_exercised_case() {
    let declared = GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .filter(|case| case.semantic() == ConformanceSemanticV1::PlironLoweringLane)
        .filter_map(|case| match case.expectation() {
            ConformanceExpectationV1::ExpectedRejection(rejection) => Some(rejection),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, EXERCISED_LOWERING_REJECTIONS);
}

fn scalar_request() -> ScalarKernelModuleV1 {
    ScalarKernelModuleV1::canonical(
        "conformance_scalar_module",
        "conformance_scalar_add",
        IdentityV1::new([0x41; 32]).expect("fixed origin identity is nonzero"),
        StageIdentitiesV1::new([0x11; 32], [0x22; 32], [0x33; 32])
            .expect("fixed stage identities are nonzero"),
    )
}

fn assert_lowering_rejection(
    name: &str,
    rejection: ExpectedRejectionV1,
    request: &ScalarKernelModuleV1,
    expected: LoweringDiagnosticV1,
) {
    let case = conformance_case_v1(name).expect("lowering rejection case must be declared");
    assert_eq!(
        case.expectation(),
        ConformanceExpectationV1::ExpectedRejection(rejection)
    );
    let actual = lower_scalar_kernel_v1(request)
        .err()
        .expect("unsupported lowering request must reject");
    assert_eq!(actual, expected);
    assert!(actual.to_string().len() <= MAX_DIAGNOSTIC_BYTES_V1);
}
