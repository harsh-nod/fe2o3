use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn identity(tag: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256([tag; 32])
}

fn declaration(
    index: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(index + 1),
        SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
        layout,
        shape,
    )
}

fn zst(index: u8) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn reference(
    index: u8,
    pointee: u32,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    declaration(
        index,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                id(pointee),
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn wrapper(fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    declaration(
        1,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        reference(
            0,
            1,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        ),
        wrapper(vec![id(2), id(4), id(7)]),
        reference(
            2,
            3,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        ),
        zst(3),
        reference(
            4,
            5,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        ),
        zst(5),
        declaration(
            6,
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ),
        zst(7),
    ]
}

fn provenance() -> SemanticKernelCapabilityProvenanceV1 {
    SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([20; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([21; 32]),
        identity(22),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([23; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([24; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([25; 32]),
    )
    .unwrap()
}

fn contract(
    types: &[SemanticTypeDeclV1],
    function: F32MathFunction,
) -> Result<PolicyMathSourceContractV1, &'static str> {
    PolicyMathSourceContractV1::new(
        PolicyMathTypesV1::new([0, 1, 2, 3, 4, 5, 6].map(id)),
        identity(30),
        identity(31),
        function,
        provenance(),
        SemanticFunctionIdentityV1::from_sha256([32; 32]),
        types,
    )
}

#[test]
fn every_fp32_consumer_preserves_policy_source_custody_and_numerical_requirements() {
    use F32MathFunction as F;
    for function in [
        F::Sqrt,
        F::FusedMultiplyAdd,
        F::Floor,
        F::Ceil,
        F::Truncate,
        F::RoundTiesEven,
        F::Sin,
        F::Cos,
        F::Exp,
        F::Exp2,
        F::Ln,
        F::Log2,
        F::Log10,
    ] {
        let contract = contract(&types(), function).unwrap();
        assert_eq!(contract.function(), function);
        assert_eq!(contract.policy(), identity(30));
        assert_eq!(contract.kernel_brand(), identity(31));
        assert_eq!(contract.provenance(), provenance());
        assert_eq!(
            contract.source_identity(),
            SemanticFunctionIdentityV1::from_sha256([32; 32])
        );
        assert_eq!(contract.types().all(), [0, 1, 2, 3, 4, 5, 6].map(id));
        assert_eq!(
            contract.arguments(),
            if function == F::FusedMultiplyAdd {
                vec![id(0), id(6), id(6), id(6)]
            } else {
                vec![id(0), id(6)]
            }
        );
        assert_eq!(
            contract.numerical_requirements(),
            (
                NumericalModeV1::StrictIeee,
                function.required_implementation()
            )
        );
    }
    assert!(contract(&types(), F::Abs).is_err());
}

#[test]
fn rejects_each_replaced_or_nonshared_reference() {
    for (index, pointee) in [(0, 1), (2, 3), (4, 5)] {
        for (kind, mutability, target) in [
            (
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                pointee,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                pointee,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                7,
            ),
        ] {
            let mut types = types();
            types[index] = reference(index as u8, target, kind, mutability);
            assert!(contract(&types, F32MathFunction::Exp).is_err());
        }
    }
}

#[test]
fn rejects_missing_reordered_or_erased_wrapper_fields() {
    for fields in [
        vec![],
        vec![id(2)],
        vec![id(2), id(4)],
        vec![id(4), id(2), id(7)],
        vec![id(2), id(2), id(7)],
        vec![id(2), id(4), id(6)],
        vec![id(2), id(4), id(99)],
    ] {
        let mut types = types();
        types[1] = wrapper(fields);
        assert!(contract(&types, F32MathFunction::Exp).is_err());
    }
}

#[test]
fn rejects_non_fp32_element_and_nonzero_capabilities() {
    for bits in [16, 64] {
        let mut types = types();
        let bytes = u64::from(bits / 8);
        types[6] = declaration(
            6,
            SemanticTypeLayoutV1::new(Some(bytes), bytes)
                .expect("non-FP32 element must still have a valid scalar layout"),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }),
        );
        assert_eq!(
            contract(&types, F32MathFunction::Exp),
            Err("policy math element must be FP32")
        );
    }
    for index in [3, 5] {
        let mut types = types();
        types[index] = declaration(
            index as u8,
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        );
        assert!(contract(&types, F32MathFunction::Exp).is_err());
    }
}

#[test]
fn rejects_missing_duplicate_zero_and_policy_aliased_type_identities() {
    for index in 0..7 {
        let mut duplicate = types();
        duplicate.push(duplicate[index].clone());
        assert!(contract(&duplicate, F32MathFunction::Exp).is_err());
        for identity in [identity(0), identity(30), identity(31)] {
            let mut types = types();
            let old = &types[index];
            types[index] = SemanticTypeDeclV1::new(
                identity,
                old.layout_identity(),
                old.layout().clone(),
                old.shape().clone(),
            );
            assert!(contract(&types, F32MathFunction::Exp).is_err());
        }
    }
    for length in 0..8 {
        assert!(contract(&types()[..length], F32MathFunction::Exp).is_err());
    }
}

#[test]
fn rejects_substituted_type_ids_and_incomplete_contract_identities() {
    for index in 0..7 {
        for replacement in [id(99), id(((index + 1) % 7) as u32)] {
            let mut ids = [0, 1, 2, 3, 4, 5, 6].map(id);
            ids[index] = replacement;
            assert!(
                PolicyMathSourceContractV1::new(
                    PolicyMathTypesV1::new(ids),
                    identity(30),
                    identity(31),
                    F32MathFunction::Sqrt,
                    provenance(),
                    SemanticFunctionIdentityV1::from_sha256([32; 32]),
                    &types()
                )
                .is_err()
            );
        }
    }
    for (policy, brand, source) in [(0, 31, 32), (30, 0, 32), (30, 30, 32), (30, 31, 0)] {
        assert!(
            PolicyMathSourceContractV1::new(
                PolicyMathTypesV1::new([0, 1, 2, 3, 4, 5, 6].map(id)),
                identity(policy),
                identity(brand),
                F32MathFunction::Sqrt,
                provenance(),
                SemanticFunctionIdentityV1::from_sha256([source; 32]),
                &types()
            )
            .is_err()
        );
    }
}
