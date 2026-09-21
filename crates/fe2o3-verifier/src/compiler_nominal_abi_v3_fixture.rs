use super::*;

// Constructed admitted semantic fixtures exercise actual public owning stages.
// They are not an ordinary-rustc callback or signed execution evidence.
pub(super) fn semantic(erased: bool) -> ProductionSemanticMirOwnerV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let layout = SemanticLayoutIdentityV1::from_sha256([71; 32]);
    let unit = SemanticTypeIdV1::from_index(0);
    let mut types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            Repr::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        Shape::Unit,
    )];
    for (i, kind) in [Kind::Usize, Kind::Isize, Kind::Ordinary, Kind::Ordinary]
        .into_iter()
        .enumerate()
    {
        let signed = i % 2 == 1;
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([10 + i as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([20 + i as u8; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    Repr::scalar(BackendScalar::initialized(
                        Primitive::integer(signed, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                Shape::Scalar(SourceScalar::Integer { signed, bits: 64 }),
            )
            .with_rust_type_kind(kind)
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
            ),
        );
    }
    let slice = SemanticTypeIdV1::from_index(5);
    let shared = SemanticTypeIdV1::from_index(6);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([15; 32]),
        SemanticLayoutIdentityV1::from_sha256([25; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            8,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 8,
                count: 0,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            Repr::memory(false),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        Shape::Slice {
            element: SemanticTypeIdV1::from_index(3),
        },
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([16; 32]),
            SemanticLayoutIdentityV1::from_sha256([26; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                Repr::scalar_pair(
                    BackendScalar::initialized(
                        Primitive::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    BackendScalar::initialized(
                        Primitive::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    slice,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        Extension::None,
        0,
        None,
    )
    .unwrap();
    let shared_attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        Extension::None,
        0,
        Some(8),
    )
    .unwrap();
    let mut args = (1..=4)
        .map(|i| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(i),
                Mode::Direct(attrs),
            ))
        })
        .collect::<Vec<_>>();
    args.push(SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        shared,
        Mode::Pair {
            first: shared_attrs,
            second: attrs,
        },
    )));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([32; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        5,
        args,
        SemanticAbiValueV1::new(unit, Mode::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::SharedBorrow,
    ])
    .unwrap();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            provenance,
        )
    };
    let mut locals = vec![local(40, unit, SemanticLocalRoleV1::Return)];
    for i in 0..4 {
        locals.push(local(
            41 + i,
            SemanticTypeIdV1::from_index(u32::from(i) + 1),
            SemanticLocalRoleV1::Argument(u32::from(i)),
        ));
    }
    locals.push(local(45, shared, SemanticLocalRoleV1::Argument(4)));
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance,
            statements,
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    };
    let blocks = if erased {
        locals.push(local(46, unit, SemanticLocalRoleV1::Temporary));
        vec![
            block(
                50,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(6), vec![], unit)
                                .unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(51, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![block(50, vec![], SemanticTerminatorKindV1::Return)]
    };
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        provenance,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(NAME.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(BINDING),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let mut functions = vec![root];
    if erased {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([61; 32]),
            SemanticLayoutIdentityV1::from_sha256([73; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, Mode::Ignore),
        )
        .unwrap();
        let u64_ty = SemanticTypeIdV1::from_index(3);
        let place = |index| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], u64_ty).unwrap()
        };
        // Real helper-local memory requires the independently checked UnitLocal route.
        let statements = vec![
            SemanticStatementV1::new(
                provenance,
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    place(1),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        u64_ty,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(11, 8).unwrap()),
                    )),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ),
            SemanticStatementV1::new(
                provenance,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2),
                    SemanticRvalueV1::new(
                        u64_ty,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            place(1),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                )),
            ),
        ];
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([60; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([60; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([60; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([60; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([60; 32]),
                provenance,
                abi,
                vec![
                    local(62, unit, SemanticLocalRoleV1::Return),
                    local(64, u64_ty, SemanticLocalRoleV1::Temporary),
                    local(65, u64_ty, SemanticLocalRoleV1::Temporary),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![block(63, statements, SemanticTerminatorKindV1::Return)],
            )
            .unwrap(),
        );
    }
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v35(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(admitted.types()[1].rust_type_kind(), Kind::Usize);
    assert_eq!(admitted.types()[2].rust_type_kind(), Kind::Isize);
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}
