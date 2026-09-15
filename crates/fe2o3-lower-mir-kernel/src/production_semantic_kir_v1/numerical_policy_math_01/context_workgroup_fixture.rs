// Complete the synthetic ordered Context fixture with an executed mutable
// consumer. An unused callable must not substitute for this source use.
fn append_context_workgroup_fixture(
    types: &mut Vec<SemanticTypeDeclV1>,
    base: &AdmittedInertSemanticMirV1,
) -> SemanticCallableDeclV1 {
    assert_eq!(types.len(), 14);
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([223; 32]),
        SemanticLayoutIdentityV1::from_sha256([223; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    for (index, fields, offsets) in [(15u8, vec![], vec![]), (16, vec![ty(15)], vec![0])] {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([209 + index; 32]),
            SemanticLayoutIdentityV1::from_sha256([209 + index; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        ));
    }
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([226; 32]),
        SemanticLayoutIdentityV1::from_sha256([226; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: scalar,
                second: scalar,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ty(14), ty(14), ty(16), ty(15)]).unwrap(),
        ),
    ));
    let SemanticDefinedCapabilityContractV1::KernelMathDerive(getter) =
        base.functions()[1].defined_capability_contract().unwrap()
    else {
        unreachable!()
    };
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
            context: ty(13),
            workgroup: ty(17),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(13)], ty(17)).unwrap(),
        getter.provenance(),
        types[15].identity(),
        types[16].identity(),
        None,
        SemanticFunctionIdentityV1::from_sha256([184; 32]),
    )
    .unwrap();
    let attributes = |pointer| {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(pointer, None, pointer, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap()
    };
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([184; 32]),
        SemanticLayoutIdentityV1::from_sha256([184; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(13)],
        ty(17),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(13),
            SemanticAbiPassModeV1::Direct(attributes(true)),
        ))],
        SemanticAbiValueV1::new(
            ty(17),
            SemanticAbiPassModeV1::Pair {
                first: attributes(false),
                second: attributes(false),
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
    .unwrap();
    terminal(
        184,
        abi,
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
    )
}
