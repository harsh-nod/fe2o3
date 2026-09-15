use super::super::{SimRuntimeBackendErrorV1, validate_compiler_packing_plan_v2};
use fe2o3_kernel_ir::{
    ExecutionCapabilityProvenanceV1, ExecutionTypeIdentityV1, FunctionId, PhaseKeyV1,
    PhaseLoanStateV1, ReusablePhaseTokenRoleV1, ReusablePhaseTokenTypeV1, ScalarType,
    SemanticArgumentOwnershipV1, SemanticArgumentStorageV2, SemanticComponentStorageBindingV2,
    SemanticKernargSlotV2, SemanticKernelStorageV2, SemanticKirComponentRepresentationV2,
    SemanticKirComponentStorageV2, SemanticStorageProjectionV2, Type,
};

fn token(state: PhaseLoanStateV1) -> Type {
    let token = ReusablePhaseTokenTypeV1 {
        provenance: ExecutionCapabilityProvenanceV1 {
            root: FunctionId::new("entry"),
            kernel_binding: [1; 32],
            frontend_unit: [2; 32],
            kernel_marker: [3; 32],
            target_brand: [4; 32],
            launch_brand: [5; 32],
            issuance: [6; 32],
        },
        phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
        owner_source: ExecutionTypeIdentityV1::new([8; 32]),
        owner_anchor_epoch: [9; 32],
        outer_brand: [10; 32],
        phase_brand: [11; 32],
        initial_epoch: [12; 32],
        role: ReusablePhaseTokenRoleV1::OwnerLoan(state),
    };
    assert!(token.is_complete());
    Type::ReusablePhaseToken(token)
}

#[test]
fn phase_token_cannot_substitute_for_exact_packed_scalar() {
    let component = SemanticKirComponentStorageV2::new(
        vec![SemanticStorageProjectionV2::Field { index: 0 }],
        0,
        7,
        SemanticKirComponentRepresentationV2::ScalarValue,
        SemanticKernargSlotV2::new(0, 8, 8),
        None,
    );
    let storage = SemanticKernelStorageV2::new(
        0,
        0,
        0,
        8,
        8,
        vec![SemanticArgumentStorageV2::new(
            0,
            1,
            0,
            SemanticArgumentOwnershipV1::ByValue,
            SemanticComponentStorageBindingV2::exact(vec![component]),
        )],
    );
    validate_compiler_packing_plan_v2(&storage, &[Type::Scalar(ScalarType::U64)]).unwrap();
    for state in [
        PhaseLoanStateV1::Active,
        PhaseLoanStateV1::Sealed,
        PhaseLoanStateV1::Returned,
    ] {
        assert!(
            matches!(validate_compiler_packing_plan_v2(&storage, &[token(state)]),
            Err(SimRuntimeBackendErrorV1::UnsupportedBundle(detail))
                if detail.contains("no caller-provided physical slot"))
        );
    }
}

#[test]
fn phase_token_cannot_be_omitted_as_a_zero_width_parameter() {
    let storage = SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![]);
    assert!(matches!(
        validate_compiler_packing_plan_v2(&storage, &[token(PhaseLoanStateV1::Returned)]),
        Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
            if detail.contains("does not cover every KIR parameter")));
}
