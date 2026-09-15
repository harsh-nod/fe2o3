use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

// Constructible inert metadata for this exact ABI lookup only. This is not an
// admitted source owner or a substitute for production expression tests.
fn metadata(
    local_ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    mode: SemanticAbiPassModeV1,
) -> SemanticFunctionDeclV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![SemanticAbiValueV1::new(scalar, mode)],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([3; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([4; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([5; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([6; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([7; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([8; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([9; 32]),
                local_ty,
                role,
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([10; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
}

#[test]
fn exact_source_parameter_rejects_foreign_type_role_and_missing_origin() {
    let ty = SemanticTypeIdV1::from_index(1);
    let value = metadata(
        ty,
        SemanticLocalRoleV1::Argument(0),
        SemanticSourceArgumentOwnershipV1::ByValue,
        direct(),
    );
    assert!(exact_by_value_argument(&value, 1));
    assert!(!exact_by_value_argument(&value, 0));
    assert!(!exact_by_value_argument(&value, 2));
    for (ty, role) in [
        (
            SemanticTypeIdV1::from_index(2),
            SemanticLocalRoleV1::Argument(0),
        ),
        (ty, SemanticLocalRoleV1::Temporary),
        (ty, SemanticLocalRoleV1::Argument(1)),
    ] {
        let changed = metadata(
            ty,
            role,
            SemanticSourceArgumentOwnershipV1::ByValue,
            direct(),
        );
        assert!(!exact_by_value_argument(&changed, 1));
    }
}

#[test]
fn source_ownership_cannot_be_inferred_from_a_scalar_or_direct_abi() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::Unspecified,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::RawPointer,
    ] {
        let function = metadata(
            SemanticTypeIdV1::from_index(1),
            SemanticLocalRoleV1::Argument(0),
            ownership,
            direct(),
        );
        assert!(!exact_by_value_argument(&function, 1));
    }
}

#[test]
fn exact_direct_parameter_cannot_be_replaced_by_ignored_or_pair_abi() {
    for mode in [
        SemanticAbiPassModeV1::Ignore,
        SemanticAbiPassModeV1::Pair {
            first: SemanticAbiValueAttributesV1::plain(),
            second: SemanticAbiValueAttributesV1::plain(),
        },
    ] {
        let function = metadata(
            SemanticTypeIdV1::from_index(1),
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceArgumentOwnershipV1::ByValue,
            mode,
        );
        assert!(!exact_by_value_argument(&function, 1));
    }
}
