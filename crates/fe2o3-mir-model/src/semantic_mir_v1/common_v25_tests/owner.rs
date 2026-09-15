use super::*;

pub(super) fn preliminary() -> AdmittedInertSemanticMirV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes: Box::new([]),
                memory_order_source_indices: Box::new([]),
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([1; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([1; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([1; 32])),
        vec![ty],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_for_wire_version(
        SemanticMirWireVersionV1::V24,
        SemanticMirLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn unbound_row() -> SemanticTransposeOwnedFlowV1 {
    let call = |function| SemanticOwnedSourceCallSiteV1 {
        function: SemanticFunctionIdV1::from_index(function),
        block: SemanticBlockIdV1::from_index(0),
    };
    SemanticTransposeOwnedFlowV1::from_encoded_parts(
        SemanticTransposeOwnedFlowSitesV1 {
            issue: call(0),
            capture: SemanticOwnedSourceStatementSiteV1 {
                function: call(0).function,
                block: call(0).block,
                statement: 0,
            },
            capture_field: 0,
            matrix_call: call(0),
            closure_call: call(1),
            stage: call(2),
            publish: call(0),
            workgroup_local: SemanticLocalIdV1::from_index(0),
        },
        vec![],
        [
            (call(0).function, [1; 32]),
            (call(1).function, [2; 32]),
            (call(2).function, [3; 32]),
        ],
        [4; 32],
    )
    .unwrap()
}

#[test]
fn common_v25_consuming_footer_moves_tables_and_reencodes_exactly() {
    let old = preliminary();
    let functions = old.functions().as_ptr();
    let locals = old.functions()[0].locals().as_ptr();
    let blocks = old.functions()[0].blocks().as_ptr();
    let types = old.types().as_ptr();
    let mut expected = old.canonical_encoding().to_vec();
    expected[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&25u16.to_le_bytes());
    expected.extend_from_slice(&[0; 4]);
    let new = old
        .with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(new.wire_version(), SemanticMirWireVersionV1::V25);
    assert_eq!(new.functions().as_ptr(), functions);
    assert_eq!(new.functions()[0].locals().as_ptr(), locals);
    assert_eq!(new.functions()[0].blocks().as_ptr(), blocks);
    assert_eq!(new.types().as_ptr(), types);
    assert_eq!(new.canonical_encoding(), expected);
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v25_canonical(
        new.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), new.canonical_encoding());
}

#[test]
fn common_v25_consuming_footer_revalidates_rows_not_digests() {
    assert!(matches!(
        preliminary()
            .with_transpose_owned_flows_v25(vec![unbound_row()], SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn common_v25_consuming_footer_rejects_existing_footer_before_replacement() {
    let mut old = preliminary();
    // Private test-only corruption isolates the mandatory empty-footer guard.
    // This row is deliberately not an admitted/source-authenticated flow.
    old.request.transpose_owned_flows = vec![unbound_row()].into_boxed_slice();
    assert!(matches!(
        old.with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn common_v25_consuming_footer_does_not_reset_the_encoding_limit() {
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, 1)
        .unwrap();
    assert!(matches!(
        preliminary().with_transpose_owned_flows_v25(vec![], limits),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            max: 1,
            ..
        })
    ));
}

#[test]
fn common_v25_consuming_footer_revalidates_the_entire_retained_request() {
    let mut old = preliminary();
    old.request.types[0].layout = SemanticTypeLayoutV1::new(Some(1), 1).unwrap();
    assert!(matches!(
        old.with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}
