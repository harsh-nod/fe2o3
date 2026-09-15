use super::super::super::private_scalar_capture_v1::PrivateScalarReads;
use super::*;

const FLOAT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const FLOAT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const PHYSICAL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const VIEW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const VIEW_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
const CONTEXT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);

#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    WrongEdge,
    Dead,
    Move,
    Reassign,
    Escape,
    MissingPredecessor,
    CopyResult,
    MoveResult,
}

fn exact_types() -> Vec<SemanticTypeDeclV1> {
    let mut all = types();
    all.push(declaration(
        10,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    ));
    // Use the same exact reference layout, with the actual f32 pointee extent.
    all.push(
        reference_type(11, FLOAT, 4).with_rustc_abi_properties(
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
    let variants = (0..2)
        .map(|index| {
            let offsets = if index == 0 { vec![] } else { vec![8] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    all.push(declaration(
        12,
        SemanticTypeLayoutV1::enum_layout(
            16,
            8,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: U64,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![FLOAT]).unwrap()),
            ]
            .into(),
        },
    ));
    all.push(declaration(
        13,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 4,
                count: 0,
            },
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
        SemanticTypeShapeV1::Slice { element: FLOAT },
    ));
    all.push(
        declaration(
            14,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SLICE,
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
    all.push(
        declaration(
            15,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                *all[PHYSICAL.index() as usize].layout().backend_repr(),
                all[PHYSICAL.index() as usize].layout().largest_niche(),
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                ),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![PHYSICAL]).unwrap()),
        )
        .with_rustc_abi_properties(all[PHYSICAL.index() as usize].abi_properties()),
    );
    all.push(reference_type(16, VIEW, 16));
    all.push(declaration(
        17,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    all.push(
        reference_type(18, CONTEXT, 0).with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        1,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    all.push(fields_type(19, vec![FLOAT_REF]));
    all.push(declaration(
        20,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                FLOAT,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    all
}

fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let scalar = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let reference = |size, align| {
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
            size,
            align,
        )
        .unwrap()
    };
    let mode = if ty == UNIT || ty == CONTEXT {
        SemanticAbiPassModeV1::Ignore
    } else if ty == PHYSICAL {
        SemanticAbiPassModeV1::Pair {
            first: reference(0, Some(4)),
            second: scalar,
        }
    } else if ty == VIEW {
        SemanticAbiPassModeV1::Pair {
            first: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                Some(4),
            )
            .unwrap(),
            second: scalar,
        }
    } else if ty == OPTION {
        SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true,
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                16,
                Some(8),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        }
    } else {
        SemanticAbiPassModeV1::Direct(if ty == CONTEXT_REF {
            reference(0, None)
        } else if ty == VIEW_REF {
            reference(16, Some(8))
        } else {
            scalar
        })
    };
    SemanticAbiValueV1::new(ty, mode)
}

fn callable(
    id: u8,
    operation: SemanticCompilerIntrinsicOperationV1,
    inputs: &[(SemanticTypeIdV1, SemanticSourceArgumentOwnershipV1)],
    output: SemanticTypeIdV1,
) -> SemanticCallableDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([id; 32]),
        SemanticLayoutIdentityV1::from_sha256([90; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs
            .iter()
            .map(|(ty, _)| SemanticAbiArgumentV1::source(abi_value(*ty)))
            .collect(),
        abi_value(output),
    )
    .unwrap()
    .with_source_argument_ownership(inputs.iter().map(|(_, ownership)| *ownership).collect())
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([id; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([id; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([id; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([id; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([id; 32]),
            source(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([id; 32]),
    }
}

fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, ty),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn float() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        FLOAT,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
    ))
}

fn owner(
    change: Change,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([90; 32]),
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([92; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([93; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([94; 32]),
    )
    .unwrap();
    let contract = SemanticCapabilityMemoryContractV1::global_read_only();
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        callable(
            101,
            Op::KernelContextIssue { context: CONTEXT },
            &[],
            CONTEXT,
        ),
        callable(
            102,
            Op::CapabilityGlobalBindReadOnly {
                context: CONTEXT,
                physical: PHYSICAL,
                view: VIEW,
                element: FLOAT,
                contract,
                provenance,
                source_identity: SemanticFunctionIdentityV1::from_sha256([102; 32]),
            },
            &[(CONTEXT_REF, SharedBorrow), (PHYSICAL, SharedBorrow)],
            VIEW,
        ),
        callable(
            103,
            Op::CapabilityGlobalLoad {
                view: VIEW,
                option: OPTION,
                element: FLOAT,
                contract,
                provenance,
                source_identity: SemanticFunctionIdentityV1::from_sha256([103; 32]),
            },
            &[(VIEW_REF, SharedBorrow), (U64, ByValue)],
            OPTION,
        ),
    ];
    let mut local_types = vec![
        UNIT,
        PHYSICAL,
        U64,
        CONTEXT,
        CONTEXT_REF,
        VIEW,
        VIEW_REF,
        OPTION,
        U64,
        FLOAT,
        FLOAT,
        FLOAT_REF,
        ENV,
        FLOAT_REF,
        FLOAT,
    ];
    // Retain the shared fixture's nominal types in this root's inventory.
    local_types.extend([STORAGE, REF, WRAPPER, WRAPPER_REF, ENUM]);
    local_types.extend([RAW, OPTION]);
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(i, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                ty,
                match i {
                    0 => SemanticLocalRoleV1::Return,
                    1 | 2 => SemanticLocalRoleV1::Argument(i as u32 - 1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source(),
            )
        })
        .collect();
    let borrow = |local, ty, target, target_ty| {
        statement(
            local,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(target, target_ty),
            },
        )
    };
    let mut capture = vec![
        borrow(11, FLOAT_REF, 10, FLOAT),
        statement(
            12,
            ENV,
            aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![copy(11, FLOAT_REF)],
            ),
        ),
        statement(
            13,
            FLOAT_REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
                12,
                &[(SemanticProjectionKindV1::Field(0), FLOAT_REF)],
            ))),
        ),
    ];
    match change {
        Change::Dead => capture.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(10)),
        )),
        Change::Move => capture.push(statement(
            9,
            FLOAT,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, FLOAT))),
        )),
        Change::Reassign => capture.push(statement(10, FLOAT, SemanticRvalueKindV1::Use(float()))),
        Change::Escape => capture.push(statement(
            20,
            RAW,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(10, FLOAT),
            },
        )),
        _ => {}
    }
    capture.push(statement(
        14,
        FLOAT,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
            13,
            &[(SemanticProjectionKindV1::Dereference, FLOAT)],
        ))),
    ));
    let result = if matches!(change, Change::CopyResult | Change::MoveResult) {
        21
    } else {
        7
    };
    let mut tested_result = vec![];
    if result != 7 {
        tested_result.push(statement(
            result,
            OPTION,
            SemanticRvalueKindV1::Use(if matches!(change, Change::CopyResult) {
                copy(7, OPTION)
            } else {
                SemanticOperandV1::Move(place(7, OPTION))
            }),
        ));
    }
    tested_result.push(statement(
        8,
        U64,
        SemanticRvalueKindV1::Discriminant(place(result, OPTION)),
    ));
    let blocks = vec![
        block(0, vec![], call(1, vec![], 3, CONTEXT, 1)),
        block(
            1,
            vec![borrow(4, CONTEXT_REF, 3, CONTEXT)],
            call(2, vec![copy(4, CONTEXT_REF), copy(1, PHYSICAL)], 5, VIEW, 2),
        ),
        block(
            2,
            vec![borrow(6, VIEW_REF, 5, VIEW)],
            call(3, vec![copy(6, VIEW_REF), constant(0)], 7, OPTION, 3),
        ),
        block(
            3,
            tested_result,
            if matches!(change, Change::WrongEdge) {
                switch(8, 4, 8)
            } else {
                switch(8, 8, 4)
            },
        ),
        block(
            4,
            vec![
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(10)),
                ),
                statement(
                    9,
                    FLOAT,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projection(
                        result,
                        &[
                            (SemanticProjectionKindV1::Downcast(1), OPTION),
                            (SemanticProjectionKindV1::Field(0), FLOAT),
                        ],
                    ))),
                ),
            ],
            switch(2, 6, 5),
        ),
        block(
            5,
            vec![statement(
                10,
                FLOAT,
                SemanticRvalueKindV1::Use(copy(9, FLOAT)),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 7)),
        ),
        block(
            6,
            if matches!(change, Change::MissingPredecessor) {
                vec![]
            } else {
                vec![statement(10, FLOAT, SemanticRvalueKindV1::Use(float()))]
            },
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 7)),
        ),
        block(7, capture, SemanticTerminatorKindV1::Return),
        block(8, vec![], SemanticTerminatorKindV1::Unreachable),
    ];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([81; 32]),
        SemanticLayoutIdentityV1::from_sha256([82; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(abi_value(PHYSICAL)),
            SemanticAbiArgumentV1::source(abi_value(U64)),
        ],
        abi_value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SharedBorrow, ByValue])
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([83; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([84; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([85; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([86; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([87; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"scalar_enum_call_capture_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([89; 32])),
        exact_types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    fe2o3_mir_model::semantic_mir_v1::layout_diagnostics_v1::diagnose_type_layouts_v1(
        &request,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let request = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        request,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
}

pub(crate) fn scalar_result_owner() -> ProductionSemanticSsaOwnerV1 {
    owner(Change::None).unwrap()
}

fn roster(owner: &ProductionSemanticSsaOwnerV1) -> Vec<(usize, usize)> {
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let reads = PrivateScalarReads::for_root(owner, SemanticFunctionIdV1::from_index(0)).unwrap();
    eprintln!("{:?}", reads.observation_for(root.body()).unwrap());
    root.body()
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.statements()
                .iter()
                .enumerate()
                .filter_map(|(statement, value)| {
                    reads
                        .contains(root.body(), value)
                        .then_some((block, statement))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

#[test]
fn scalar_enum_call_return_guarded_payload_preserves_private_capture() {
    assert_eq!(roster(&scalar_result_owner()), vec![(7, 3)]);
}

#[test]
fn scalar_enum_call_return_copy_and_move_keep_exact_payload_initialization() {
    for change in [Change::CopyResult, Change::MoveResult] {
        assert_eq!(roster(&owner(change).unwrap()), vec![(7, 3)], "{change:?}");
    }
}

#[test]
fn scalar_enum_call_return_does_not_waive_variant_or_private_borrow_validity() {
    for change in [
        Change::WrongEdge,
        Change::Dead,
        Change::Move,
        Change::Reassign,
        Change::Escape,
        Change::MissingPredecessor,
    ] {
        let owner = owner(change).unwrap();
        assert!(roster(&owner).is_empty(), "{change:?}");
    }
}
