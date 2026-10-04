fn mixed_selected_memory_owner() -> ProductionSemanticSsaOwnerV1 {
    let prior = distinct_origin_join_owner();
    let source = prior.source_semantic();
    let mut types = source.types().to_vec();
    let scalar_backend = |bits| {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
            SemanticScalarValidityRangeV1::new(0, if bits == 64 { u64::MAX.into() } else { 1 }),
        )
    };
    let pointer_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let slice = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(declaration(
        231,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: U32 },
    ));
    let shared_slice = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(
        declaration(
            232,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(pointer_backend, scalar_backend(64)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
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
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(
        declaration(
            233,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(pointer_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
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
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        4,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(declaration(
        234,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(scalar_backend(8)),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = |alignment| {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                true,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            alignment,
            Some(4),
        )
        .unwrap()
    };
    let make_abi = |kernel, inputs, ownership| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([if kernel { 235 } else { 236 }; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            if kernel { 2 } else { 1 },
            inputs,
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap()
    };
    let root_abi = make_abi(
        true,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                CARRIER,
                SemanticAbiPassModeV1::Pair {
                    first: plain,
                    second: plain,
                },
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                shared_slice,
                SemanticAbiPassModeV1::Pair {
                    first: shared(0),
                    second: plain,
                },
            )),
        ],
        vec![
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ],
    );
    let helper_abi = make_abi(
        false,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            reference,
            SemanticAbiPassModeV1::Direct(shared(4)),
        ))],
        vec![SemanticSourceArgumentOwnershipV1::SharedBorrow],
    );
    let root = &source.functions()[0];
    let mut locals = root.locals().to_vec();
    locals[2] = SemanticLocalDeclV1::new(
        locals[2].identity(),
        shared_slice,
        SemanticLocalRoleV1::Argument(1),
        provenance(),
    );
    let selected = locals.len() as u32;
    let index = selected + 1;
    let length = selected + 2;
    let condition = selected + 3;
    for (ordinal, ty) in [reference, INDEX, INDEX, boolean].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([237 + ordinal as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            provenance(),
        ));
    }
    let mut issued = root.blocks()[3].statements().to_vec();
    issued.push(assign(
        selected,
        reference,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: dereference(7),
        },
    ));
    let indexed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let blocks = vec![
        root.blocks()[0].clone(),
        root.blocks()[1].clone(),
        root.blocks()[2].clone(),
        block(
            3,
            issued,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
        ),
        block(
            4,
            vec![
                assign(
                    index,
                    INDEX,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            INDEX,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 8).unwrap(),
                            ),
                        ),
                    )),
                ),
                assign(
                    length,
                    INDEX,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: SemanticOperandV1::Copy(place(2, shared_slice)),
                    },
                ),
                assign(
                    condition,
                    boolean,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: SemanticOperandV1::Copy(place(index, INDEX)),
                        right: SemanticOperandV1::Copy(place(length, INDEX)),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(place(condition, boolean)),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: SemanticOperandV1::Copy(place(length, INDEX)),
                    index: SemanticOperandV1::Copy(place(index, INDEX)),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 5),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            5,
            vec![assign(
                selected,
                reference,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: indexed,
                },
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
        ),
        block(
            6,
            vec![],
            call(
                1,
                vec![SemanticOperandV1::Copy(place(selected, reference))],
                0,
                UNIT,
                7,
            ),
        ),
        block(
            7,
            root.blocks()[9].statements().to_vec(),
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let mixed_root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root_abi,
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let previous = &source.functions()[1];
    let helper = SemanticFunctionDeclV1::new(
        previous.identity(),
        previous.role(),
        previous.item_definition_identity(),
        previous.monomorphization_identity(),
        previous.generic_type_arguments_identity(),
        previous.const_generic_arguments_identity(),
        previous.source(),
        helper_abi,
        [UNIT, reference, U32, reference]
            .into_iter()
            .enumerate()
            .map(|(ordinal, ty)| {
                SemanticLocalDeclV1::new(
                    previous.locals()[ordinal].identity(),
                    ty,
                    previous.locals()[ordinal].role(),
                    provenance(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![block(
            100,
            vec![
                assign(
                    3,
                    reference,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: dereference(1),
                    },
                ),
                assign(
                    2,
                    U32,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        dereference(3),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
                root.blocks()[9].statements()[0].clone(),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    admitted_owner(types, vec![mixed_root, helper], source.callables().to_vec())
}

#[test]
fn selected_memory_mixed_issuer_and_descriptor_roots_keep_separate_conditional_bounds() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        mixed_selected_memory_owner(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let rows = retained_selected_memory(original);
            assert_eq!(
                (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                (1, 1, 0)
            );
            assert_eq!(rows.selected.len(), 1);
            let access = &rows.selected[0];
            assert!(!access.writing);
            assert_eq!(access.leaves.len(), 2);
            let mut issued = 0;
            let mut descriptor = 0;
            for leaf in &access.leaves {
                match leaf.origin {
                    PendingSourceSelectedLeafOriginV30::Issued(row) => {
                        assert_eq!(row.root_parameter, 0);
                        issued += 1;
                    }
                    PendingSourceSelectedLeafOriginV30::Descriptor(row) => {
                        assert_eq!(row.access, AccessMode::ReadOnly);
                        assert_eq!(row.space, AddressSpace::Global);
                        descriptor += 1;
                    }
                }
            }
            assert_eq!((issued, descriptor), (1, 1));
            assert_eq!(access.obligations.len(), 2);
            assert!(
                access
                    .obligations
                    .iter()
                    .all(|row| row.guard.is_some() && !row.at_access)
            );
            assert_eq!(
                rows.retained_storage(budget)?,
                selected_memory_storage_oracle(rows)
            );
            reached.set(true);
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(reached.get());
}
