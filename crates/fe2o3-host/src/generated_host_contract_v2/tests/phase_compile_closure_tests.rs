use super::{GeneratedHostContractErrorV2, generated_fields, validate_v13_physical_type};
use fe2o3_artifacts::PointerWidth;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, ExecutionCapabilityProvenanceV1, ExecutionTypeIdentityV1, FunctionId,
    PhaseKeyV1, PhaseLoanStateV1, ReusablePhaseTokenRoleV1, ReusablePhaseTokenTypeV1, ScalarType,
    Type,
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
fn phase_tokens_never_become_v13_physical_abi_fields() {
    let fields = generated_fields();
    let field = &fields[0];
    validate_v13_physical_type(
        &Type::slice(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        field,
        PointerWidth::Bits64,
    )
    .unwrap();
    for state in [
        PhaseLoanStateV1::Active,
        PhaseLoanStateV1::Sealed,
        PhaseLoanStateV1::Returned,
    ] {
        for ty in [
            token(state),
            Type::pointer(token(state), AddressSpace::Global, AccessMode::ReadOnly),
            Type::slice(token(state), AddressSpace::Global, AccessMode::ReadOnly),
        ] {
            assert!(matches!(
                validate_v13_physical_type(&ty, field, PointerWidth::Bits64),
                Err(GeneratedHostContractErrorV2::V13PhysicalAbi)
            ));
        }
    }
}
