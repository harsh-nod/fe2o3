use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LEADER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const DISCRIMINANT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const LEADER_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const OPTIONAL_LEADER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ELEMENT_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const SLICE_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const ELEMENT_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const OPTIONAL_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);

fn pointer_scalar(non_null: bool) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(u128::from(non_null), u64::MAX.into()),
    )
}

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(tag + 1)),
        layout,
        shape,
    )
}

fn zero_sized(tag: u8) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn pointer_type(
    tag: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    info: SemanticAbiPointeeInfoV1,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(pointer_scalar(kind == SemanticPointerKindV1::Reference)),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(Some(info), None),
    )
}

fn variant(
    index: u32,
    size: u64,
    alignment: u64,
    offsets: Vec<u64>,
    backend: SemanticBackendReprV1,
    niche: Option<SemanticLayoutNicheV1>,
) -> SemanticEnumVariantLayoutV1 {
    let order = (0..u32::try_from(offsets.len()).unwrap()).collect();
    SemanticEnumVariantLayoutV1::from_rustc(
        index,
        size,
        alignment,
        SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
        backend,
        niche,
        false,
        None,
        alignment,
        0,
        SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
    )
    .unwrap()
}

fn option_shape(payload: SemanticTypeIdV1) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Enum {
        discriminant: DISCRIMINANT,
        variants: vec![
            SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
            SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![payload]).unwrap()),
        ]
        .into_boxed_slice(),
    }
}

fn typed_layouts(base: &AdmittedInertSemanticMirV1) -> Vec<SemanticTypeDeclV1> {
    let mut types = scalar_types(base);
    types.push(zero_sized(162));
    types.push(declaration(
        164,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8,
        }),
    ));
    types.push(pointer_type(
        166,
        LEADER,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            0,
            1,
        )
        .unwrap(),
    ));
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    types.push(declaration(
        168,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            1,
            1,
            SemanticBackendReprV1::scalar(tag),
            false,
            SemanticEnumLayoutV1::new(
                vec![
                    variant(0, 1, 1, vec![], SemanticBackendReprV1::memory(true), None),
                    // The zero-sized payload is after the reserved one-byte tag.
                    variant(1, 1, 1, vec![1], SemanticBackendReprV1::scalar(tag), None),
                ],
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        option_shape(LEADER),
    ));
    let raw_info = SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
    types.push(pointer_type(
        170,
        U64,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        raw_info,
    ));
    types.push(zero_sized(172));
    let raw_index = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types.push(
        declaration(
            174,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer_scalar(false),
                    second: raw_index,
                },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ELEMENT_POINTER, U64, MARKER]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_scalar_pointee_info(Some(raw_info), None),
        ),
    );
    types.push(pointer_type(
        176,
        SLICE,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            16,
            8,
        )
        .unwrap(),
    ));
    types.push(pointer_type(
        178,
        U64,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            8,
            8,
        )
        .unwrap(),
    ));
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    // Reserving null expands the source range at its end, preserving wrapping 1..0.
    let nullable_tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, 0),
    );
    types.push(
        declaration(
            180,
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                8,
                8,
                SemanticBackendReprV1::scalar(nullable_tag),
                false,
                SemanticEnumLayoutV1::new(
                    vec![
                        variant(0, 0, 1, vec![], SemanticBackendReprV1::memory(true), None),
                        variant(
                            1,
                            8,
                            8,
                            vec![0],
                            SemanticBackendReprV1::scalar(pointer_scalar(true)),
                            Some(source_niche),
                        ),
                    ],
                    SemanticEnumEncodingV1::Niche(
                        SemanticNicheEnumEncodingV1::new(
                            0,
                            SemanticNicheSourceV1::new(
                                vec![SemanticNichePathComponentV1::Field(0)],
                                0,
                            )
                            .unwrap(),
                            source_niche,
                            nullable_tag,
                            1,
                            0,
                            0,
                            0,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            )
            .unwrap(),
            option_shape(ELEMENT_REFERENCE),
        )
        .with_rustc_abi_properties(
            // Nullable option return: no guaranteed dereferenceable pointee.
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_scalar_pointee_info(Some(raw_info), None),
        ),
    );
    assert_eq!(types.len(), 12);
    types
}

fn direct(ty: SemanticTypeIdV1, extension: SemanticAbiExtensionV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                extension,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn reference_argument(
    ty: SemanticTypeIdV1,
    shared: bool,
    size: u64,
    alignment: u64,
) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    shared,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                size,
                (alignment > 1).then_some(alignment),
            )
            .unwrap(),
        ),
    )
}

fn intrinsic(
    tag: u8,
    arguments: Vec<SemanticAbiValueV1>,
    result: SemanticAbiValueV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let count = arguments.len() as u32;
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(tag)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        count,
        arguments
            .into_iter()
            .map(SemanticAbiArgumentV1::source)
            .collect(),
        result,
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(test_bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(test_bytes(tag)),
    }
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn discriminant(
    destination: u32,
    source: u32,
    source_type: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination, DISCRIMINANT),
            SemanticRvalueV1::new(
                DISCRIMINANT,
                SemanticRvalueKindV1::Discriminant(place(source, source_type)),
            ),
        )),
    )
}

fn switch(local: u32, some: u32, none: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(local, DISCRIMINANT)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                test_edge(SemanticEdgeRoleV1::SwitchValue, some),
            )],
            test_edge(SemanticEdgeRoleV1::SwitchOtherwise, none),
        )
        .unwrap(),
    }
}

fn elision_request(borrow_in_some: bool) -> InertSemanticMirRequestV1 {
    let base = admitted_single_function_semantic();
    let original = &base.functions()[0];
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(190)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(191)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(reference_argument(
            SLICE_REFERENCE,
            false,
            16,
            8,
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let borrow = SemanticStatementV1::new(
        original.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(5, LEADER_REFERENCE),
            SemanticRvalueV1::new(
                LEADER_REFERENCE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(2, LEADER),
                },
            ),
        )),
    );
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        root_abi,
        vec![
            test_local(192, UNIT.index(), SemanticLocalRoleV1::Return),
            test_local(
                193,
                SLICE_REFERENCE.index(),
                SemanticLocalRoleV1::Argument(0),
            ),
            test_local(194, LEADER.index(), SemanticLocalRoleV1::Temporary),
            test_local(195, OPTIONAL_LEADER.index(), SemanticLocalRoleV1::Temporary),
            test_local(196, DISCRIMINANT.index(), SemanticLocalRoleV1::Temporary),
            test_local(
                197,
                LEADER_REFERENCE.index(),
                SemanticLocalRoleV1::Temporary,
            ),
            test_local(
                198,
                OPTIONAL_REFERENCE.index(),
                SemanticLocalRoleV1::Temporary,
            ),
            test_local(199, DISCRIMINANT.index(), SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            test_block(
                200,
                vec![],
                test_call(
                    1,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(3, OPTIONAL_LEADER),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                ),
            ),
            test_block(
                201,
                vec![discriminant(4, 3, OPTIONAL_LEADER)],
                if borrow_in_some {
                    switch(4, 2, 3)
                } else {
                    switch(4, 3, 2)
                },
            ),
            test_block(
                202,
                vec![borrow],
                test_call(
                    2,
                    vec![
                        SemanticOperandV1::Copy(place(1, SLICE_REFERENCE)),
                        SemanticOperandV1::Copy(place(5, LEADER_REFERENCE)),
                        number(0),
                    ],
                    Some(SemanticCallDestinationV1::new(
                        place(6, OPTIONAL_REFERENCE),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 4),
                    )),
                ),
            ),
            test_block(203, vec![], SemanticTerminatorKindV1::Return),
            test_block(
                204,
                vec![discriminant(7, 6, OPTIONAL_REFERENCE)],
                switch(7, 5, 6),
            ),
            test_block(205, vec![], SemanticTerminatorKindV1::Return),
            test_block(206, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        typed_layouts(&base),
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            intrinsic(
                220,
                vec![],
                direct(OPTIONAL_LEADER, SemanticAbiExtensionV1::ZeroExtend),
                SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                    grid_leader: LEADER,
                },
            ),
            intrinsic(
                221,
                vec![
                    reference_argument(SLICE_REFERENCE, false, 16, 8),
                    reference_argument(LEADER_REFERENCE, true, 0, 1),
                    direct(U64, SemanticAbiExtensionV1::None),
                ],
                direct(OPTIONAL_REFERENCE, SemanticAbiExtensionV1::None),
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                    disjoint_slice: SLICE,
                    grid_leader: LEADER,
                    element: U64,
                    raw_index: U64,
                },
            ),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn admitted_grid_leader_capture_records_only_the_authenticated_elision() {
    with_capture(admit_owner(elision_request(true)), |owner| {
        let view = owner.occurrences_v1().unwrap();
        let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        assert_eq!(view.function_count(), 1);
        assert!(std::ptr::eq(rows.owner(), owner));
        assert!(owner.plans()[0].implicit_entry_variables().is_empty());
        assert_eq!(rows.elisions(), &[statement(2, 0)]);
        assert_eq!(rows.entry_definitions().len(), 1);
        let entry = &rows.entry_definitions()[0];
        assert_eq!(
            (
                entry.ordinal(),
                entry.variable(),
                entry.origin(),
                entry.value()
            ),
            (
                0,
                variable(1),
                EntryOrigin::Argument(0),
                Some(definition(0))
            )
        );
        assert_eq!(rows.events().len(), 9);
        let elided = &rows.events()[3];
        assert_eq!(
            (
                elided.site(),
                elided.ordinal(),
                elided.operand(),
                elided.role(),
                elided.event()
            ),
            (
                statement(2, 0),
                0,
                Operand::ElidedBorrowDestination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(5))
            )
        );
        assert_eq!(
            elided.resolved(),
            Some(SsaResolvedEventV1::Define {
                variable: variable(5),
                value: definition(2)
            })
        );
        assert!(
            rows.events()
                .iter()
                .all(|row| row.event().variable() != variable(2))
        );
        let expected = [
            (
                1,
                0,
                SsaResolvedEventV1::Use {
                    variable: variable(3),
                    value: definition(4),
                },
            ),
            (
                1,
                1,
                SsaResolvedEventV1::Define {
                    variable: variable(4),
                    value: definition(1),
                },
            ),
            (
                1,
                2,
                SsaResolvedEventV1::Use {
                    variable: variable(4),
                    value: definition(1),
                },
            ),
            (
                2,
                0,
                SsaResolvedEventV1::Define {
                    variable: variable(5),
                    value: definition(2),
                },
            ),
            (
                2,
                1,
                SsaResolvedEventV1::Use {
                    variable: variable(1),
                    value: definition(0),
                },
            ),
            (
                2,
                2,
                SsaResolvedEventV1::Use {
                    variable: variable(5),
                    value: definition(2),
                },
            ),
            (
                4,
                0,
                SsaResolvedEventV1::Use {
                    variable: variable(6),
                    value: definition(5),
                },
            ),
            (
                4,
                1,
                SsaResolvedEventV1::Define {
                    variable: variable(7),
                    value: definition(3),
                },
            ),
            (
                4,
                2,
                SsaResolvedEventV1::Use {
                    variable: variable(7),
                    value: definition(3),
                },
            ),
        ];
        for (row, (block, ordinal, resolved)) in rows.events().iter().zip(expected) {
            let source_block = match row.site() {
                Site::Statement { block, .. } | Site::Terminator { block } => block,
            };
            assert_eq!(source_block, SsaBlockIdV1::new(block));
            assert_eq!(row.ordinal(), ordinal);
            assert!(row.is_reachable());
            assert!(row.is_promoted());
            assert_eq!(row.resolved(), Some(resolved));
        }
        assert_eq!(rows.successors().len(), 6);
        assert_eq!(rows.edge_definitions().len(), 2);
        for (row, (block, local, value)) in
            rows.edge_definitions().iter().zip([(0, 3, 4), (2, 6, 5)])
        {
            assert_eq!(row.edge(), SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0));
            assert_eq!(row.variable(), variable(local));
            assert_eq!(row.value(), Some(definition(value)));
        }
        assert_eq!(rows.constants().len(), 1);
        let constant = &rows.constants()[0];
        assert_eq!(
            (
                constant.site(),
                constant.operand(),
                constant.next_event(),
                constant.ty()
            ),
            (terminator(2), Operand::CallArgument(2), 3, U64)
        );
        owner.verify_replay().unwrap();
    });
}

#[test]
fn admitted_grid_leader_source_outside_some_does_not_gain_an_implicit_definition() {
    let admitted = elision_request(false)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let error =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap_err();
    assert_eq!(
        error,
        ProductionSemanticSsaErrorV1::Planner {
            function: SemanticFunctionIdV1::from_index(0),
            error: SsaPlannerErrorV1::UndefinedAtUse {
                block: SsaBlockIdV1::new(2),
                event: 0,
                variable: variable(2)
            },
        }
    );
}
