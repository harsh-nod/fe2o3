use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn fixture(
    mode: SemanticAbiPassModeV1,
    inputs: Vec<SemanticTypeIdV1>,
    output: SemanticTypeIdV1,
) -> (SemanticFunctionAbiV1, Vec<SemanticTypeDeclV1>) {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let args = inputs
        .iter()
        .map(|ty| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                *ty,
                SemanticAbiPassModeV1::Direct(attributes),
            ))
        })
        .collect();
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs,
        output,
        args,
        SemanticAbiValueV1::new(output, mode),
    )
    .unwrap();
    let types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )];
    (abi, types)
}

#[test]
fn math_current_accepts_only_empty_ignored_zst_abi() {
    let (abi, types) = fixture(
        SemanticAbiPassModeV1::Ignore,
        vec![],
        SemanticTypeIdV1::from_index(0),
    );
    assert!(current_abi(&abi, &types));
    assert!(!current_abi(&abi, &[]));
    let (argument_abi, _) = fixture(
        SemanticAbiPassModeV1::Ignore,
        vec![SemanticTypeIdV1::from_index(0)],
        SemanticTypeIdV1::from_index(0),
    );
    assert!(!current_abi(&argument_abi, &types));
}

#[test]
fn math_current_rejects_materialized_return_abi() {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    for mode in [
        SemanticAbiPassModeV1::Direct(attributes),
        SemanticAbiPassModeV1::Pair { first: attributes, second: attributes },
    ] {
        let (abi, types) = fixture(mode, vec![], SemanticTypeIdV1::from_index(0));
        assert!(!current_abi(&abi, &types));
    }
}

#[test]
fn math_current_rejects_non_zst_wrapper_and_missing_output() {
    let (abi, mut types) = fixture(
        SemanticAbiPassModeV1::Ignore,
        vec![],
        SemanticTypeIdV1::from_index(0),
    );
    types[0] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    );
    assert!(!current_abi(&abi, &types));
    let (abi, types) = fixture(
        SemanticAbiPassModeV1::Ignore,
        vec![],
        SemanticTypeIdV1::from_index(1),
    );
    assert!(!current_abi(&abi, &types));
}
