use super::*;

#[path = "source_argument_ownership_tests125.rs"]
mod source_argument_ownership_tests125;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const TAIL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn attributes(frozen: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            frozen,
            frozen.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            frozen,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        if frozen { 40 } else { 0 },
        Some(8),
    )
    .unwrap()
}

fn abi(
    fields: Vec<SemanticAbiArgumentV1>,
    attributes: SemanticAbiValueAttributesV1,
) -> SemanticFunctionAbiV1 {
    let mut args = vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        REFERENCE,
        SemanticAbiPassModeV1::Direct(attributes),
    ))];
    args.extend(fields);
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![REFERENCE, TAIL],
        UNIT,
        args,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap()
}

fn tuple(fields: Vec<SemanticTypeIdV1>) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap())
}

fn fixture(
    tail: SemanticTypeShapeV1,
    fields: Vec<SemanticAbiArgumentV1>,
    frozen: bool,
    actual: SemanticAbiValueAttributesV1,
) -> InertSemanticMirRequestV1 {
    let decl = |tag: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    let zero = || {
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap()
    };
    let unit_layout = || {
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
        .unwrap()
    };
    let tail_layout = match &tail {
        SemanticTypeShapeV1::Unit => unit_layout(),
        SemanticTypeShapeV1::Tuple(value) if !value.fields().is_empty() => {
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap()
        }
        _ => zero(),
    };
    let reference = decl(
        3,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ENV,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen },
                    if frozen { 40 } else { 0 },
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi(fields, actual),
        [
            (UNIT, SemanticLocalRoleV1::Return),
            (REFERENCE, SemanticLocalRoleV1::Argument(0)),
            (TAIL, SemanticLocalRoleV1::Argument(1)),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
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
        vec![
            decl(1, unit_layout(), SemanticTypeShapeV1::Unit),
            decl(
                2,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(40),
                    8,
                    SemanticBackendReprV1::memory(true),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Opaque,
            ),
            reference,
            decl(4, tail_layout, tail),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn predicate(request: &InertSemanticMirRequestV1) -> Result<(), SemanticMirErrorV1> {
    validate_rust_call_expansion(request, request.functions[0].abi())
}

#[test]
fn predicate_unit_and_empty_tuple_preserve_two_source_edges_and_one_physical_argument() {
    for tail in [SemanticTypeShapeV1::Unit, tuple(vec![])] {
        let request = fixture(tail, vec![], false, attributes(false));
        let abi = request.functions[0].abi();
        assert_eq!(abi.source_input_types(), [REFERENCE, TAIL]);
        assert_eq!(
            abi.source_argument_ownership(),
            [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
        );
        assert_eq!(abi.fixed_count(), 1);
        assert_eq!(abi.adjusted_arguments().len(), 1);
        assert_eq!(predicate(&request), Ok(()));
    }
}

#[test]
fn predicate_rejects_non_tuple_zst_and_extra_unit_field() {
    for tail in [
        SemanticTypeShapeV1::Opaque,
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ] {
        assert_eq!(
            predicate(&fixture(tail, vec![], false, attributes(false))),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
    let extra = SemanticAbiArgumentV1::rust_call_tuple_field(
        0,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    );
    assert_eq!(
        predicate(&fixture(
            SemanticTypeShapeV1::Unit,
            vec![extra],
            false,
            attributes(false)
        )),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}

#[test]
fn predicate_nonempty_tuple_keeps_exact_field_type_and_count() {
    let field = SemanticAbiArgumentV1::rust_call_tuple_field(
        0,
        SemanticAbiValueV1::new(REFERENCE, SemanticAbiPassModeV1::Direct(attributes(false))),
    );
    assert_eq!(
        predicate(&fixture(
            tuple(vec![REFERENCE]),
            vec![field.clone()],
            false,
            attributes(false)
        )),
        Ok(())
    );
    assert_eq!(
        predicate(&fixture(
            tuple(vec![REFERENCE]),
            vec![],
            false,
            attributes(false)
        )),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(
        predicate(&fixture(
            tuple(vec![UNIT]),
            vec![field],
            false,
            attributes(false)
        )),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}

#[test]
fn constructor_source_count_fixed_count_and_field_order_remain_exact() {
    let source = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        REFERENCE,
        SemanticAbiPassModeV1::Direct(attributes(false)),
    ));
    for (fixed, inputs, args) in [
        (0, vec![REFERENCE, TAIL], vec![source.clone()]),
        (1, vec![REFERENCE], vec![source.clone()]),
        (1, vec![REFERENCE, TAIL], vec![]),
        (
            1,
            vec![REFERENCE, TAIL],
            vec![source.clone(), source.clone()],
        ),
        (
            1,
            vec![REFERENCE, TAIL],
            vec![
                source,
                SemanticAbiArgumentV1::rust_call_tuple_field(
                    1,
                    SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
                ),
            ],
        ),
    ] {
        assert_eq!(
            SemanticFunctionAbiV1::from_rustc_with_source_signature(
                SemanticAbiIdentityV1::from_sha256([1; 32]),
                SemanticLayoutIdentityV1::from_sha256([1; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::RustCall,
                false,
                false,
                fixed,
                inputs,
                UNIT,
                args,
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
    assert!(matches!(
        abi(vec![], attributes(false))
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow,]),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn document_unit_rustcall_shared_closure_round_trip() {
    for frozen in [false, true] {
        let admitted = fixture(
            SemanticTypeShapeV1::Unit,
            vec![],
            frozen,
            attributes(frozen),
        )
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(admitted.canonical_encoding(), decoded.canonical_encoding());
        assert_eq!(admitted.functions(), decoded.functions());
        assert_eq!(admitted.functions()[0].abi().source_input_types().len(), 2);
        assert_eq!(admitted.functions()[0].abi().arguments().len(), 1);
    }
}

#[test]
fn document_empty_tuple_still_passes_and_shared_closure_attributes_stay_exact() {
    for frozen in [false, true] {
        fixture(tuple(vec![]), vec![], frozen, attributes(frozen))
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        let rejected = fixture(tuple(vec![]), vec![], frozen, attributes(!frozen))
            .admit_current_production(SemanticMirLimitsV1::default());
        assert!(
            matches!(rejected, Err(SemanticMirErrorV1::InvalidFunctionAbi)),
            "{rejected:?}"
        );
    }
}
