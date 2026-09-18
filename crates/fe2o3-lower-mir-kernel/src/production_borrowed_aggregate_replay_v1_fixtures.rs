use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const MUT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SHARED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn field(local: u32, borrowed: bool, index: u32) -> SemanticPlaceV1 {
    let mut projections = vec![];
    if borrowed {
        projections
            .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV).unwrap());
    }
    projections
        .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), U32).unwrap());
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), projections, U32).unwrap()
}

fn assign(destination: SemanticPlaceV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = destination.ty();
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

fn scalar(value: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(u128::from(value), 4).unwrap()),
    ))
}

fn source_types() -> Vec<SemanticTypeDeclV1> {
    let mut result = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
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
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                4,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        ),
    ];
    for (tag, mutability) in [
        (4, SemanticMutabilityV1::Mutable),
        (5, SemanticMutabilityV1::Immutable),
    ] {
        result.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        ENV,
                        SemanticPointerKindV1::Reference,
                        mutability,
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
                            match mutability {
                                SemanticMutabilityV1::Mutable => {
                                    SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                                }
                                SemanticMutabilityV1::Immutable => {
                                    SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                                }
                            },
                            8,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
    }
    result
}

fn source_abi(tag: u8, reference: Option<SemanticTypeIdV1>) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if reference.is_some() {
            SemanticCanonAbiV1::Rust
        } else {
            SemanticCanonAbiV1::GpuKernel
        },
        if reference.is_some() {
            SemanticExternAbiV1::Rust
        } else {
            SemanticExternAbiV1::GpuKernel
        },
        false,
        false,
        u32::from(reference.is_some()),
        reference
            .into_iter()
            .map(|ty| {
                if ty == ENV {
                    return SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        ty,
                        SemanticAbiPassModeV1::Cast {
                            pad_i32: false,
                            cast: SemanticAbiCastV1::new(
                                [None; 8], None,
                                SemanticAbiUniformV1::new(
                                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(), 8,
                                ).unwrap(),
                                SemanticAbiValueAttributesV1::plain(),
                            ),
                        },
                    ));
                }
                let shared = ty == SHARED;
                let attributes = SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        shared,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    8,
                    Some(4),
                )
                .unwrap();
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Direct(attributes),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(
        reference
            .into_iter()
            .map(|ty| {
                if ty == ENV {
                    SemanticSourceArgumentOwnershipV1::ByValue
                } else if ty == MUT {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                }
            })
            .collect(),
    )
    .unwrap()
}

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([tag; 32]),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn source_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn source_call(
    helper: u32,
    reference_local: u32,
    reference_ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(helper),
            vec![SemanticOperandV1::Move(place(
                reference_local,
                reference_ty,
            ))],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn source_function(
    tag: u8,
    reference: Option<SemanticTypeIdV1>,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if reference.is_some() {
            SemanticFunctionRoleV1::InternalHelper
        } else {
            SemanticFunctionRoleV1::KernelRoot
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        source_abi(tag, reference),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if reference.is_some() {
        return function;
    }
    function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"borrowed_replay_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([90; 32]),
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
    ))
}

pub(super) fn owner_source() -> ProductionSemanticSsaOwnerV1 {
    source_owner(false, 10)
}

fn source_owner(seed: bool, initial: u32) -> ProductionSemanticSsaOwnerV1 {
    source_owner_with_split_helper(seed, initial, false)
}

fn source_owner_with_split_helper(
    seed: bool,
    initial: u32,
    split_helper: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let borrow = |shared| {
        assign(
            place(
                if shared { 3 } else { 2 },
                if shared { SHARED } else { MUT },
            ),
            SemanticRvalueKindV1::Borrow {
                kind: if shared {
                    SemanticBorrowKindV1::Shared
                } else {
                    SemanticBorrowKindV1::Mutable
                },
                place: place(1, ENV),
            },
        )
    };
    let root = if seed {
        source_function(
            20,
            None,
            vec![local(21, UNIT, SemanticLocalRoleV1::Return)],
            vec![source_block(30, vec![], SemanticTerminatorKindV1::Return)],
        )
    } else {
        source_function(
            20,
            None,
            vec![
                local(21, UNIT, SemanticLocalRoleV1::Return),
                local(22, ENV, SemanticLocalRoleV1::Temporary),
                local(23, MUT, SemanticLocalRoleV1::Temporary),
                local(24, SHARED, SemanticLocalRoleV1::Temporary),
                local(25, U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                source_block(
                    30,
                    vec![
                        assign(
                            place(1, ENV),
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Aggregate,
                                    vec![scalar(initial), scalar(20)],
                                )
                                .unwrap(),
                            ),
                        ),
                        borrow(false),
                    ],
                    source_call(1, 2, MUT, 1),
                ),
                source_block(31, vec![borrow(false)], source_call(1, 2, MUT, 2)),
                source_block(32, vec![borrow(true)], source_call(2, 3, SHARED, 3)),
                source_block(
                    33,
                    vec![assign(
                        place(4, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, false, 1))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
    };
    let mut functions = vec![root];
    if !seed {
        for (tag, reference) in [(40, MUT), (50, SHARED)] {
            let mut statements = vec![assign(
                place(2, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(
                    1,
                    true,
                    if reference == MUT { 0 } else { 1 },
                ))),
            )];
            if reference == MUT {
                statements.push(assign(
                    field(1, true, 1),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
                ));
            }
            let blocks = if split_helper && reference == MUT {
                vec![
                    source_block(
                        tag + 4,
                        statements,
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(1),
                        )),
                    ),
                    source_block(tag + 5, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![source_block(
                    tag + 4,
                    statements,
                    SemanticTerminatorKindV1::Return,
                )]
            };
            functions.push(source_function(
                tag,
                Some(reference),
                vec![
                    local(tag + 1, UNIT, SemanticLocalRoleV1::Return),
                    local(tag + 2, reference, SemanticLocalRoleV1::Argument(0)),
                    local(tag + 3, U32, SemanticLocalRoleV1::Temporary),
                ],
                blocks,
            ));
        }
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let mut types = source_types();
    if seed {
        types.truncate(1);
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

struct Fixture {
    source: ProductionSemanticSsaOwnerV1,
    module: Module,
    correspondence: SemanticKirCorrespondenceV1,
    fields: Vec<BorrowedAggregateFieldCandidateV1>,
    calls: Vec<BorrowedAggregateCallCandidateV1>,
}

fn fixture() -> Fixture {
    fixture_with(false, false)
}

fn fixture_with(split_helper: bool, source_initialization: bool) -> Fixture {
    let source = source_owner_with_split_helper(false, 10, split_helper);
    // Seed only the unrelated correspondence header/defaults. Affected source,
    // native bodies and all locators below are constructed independently.
    let (_, mut correspondence) = lower_module(
        &source_owner(true, 10),
        ProductionSemanticKirLimitsV1::default(),
        None,
    )
    .unwrap();
    let names = [
        "borrowed_replay_root",
        "borrowed_replay_mut",
        "borrowed_replay_shared",
    ];
    let scalar = Type::Scalar(ScalarType::U32);
    let rw = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let ro = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadOnly);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let load = |result, pointer| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(pointer),
                access,
            },
        )
    };
    let store = |pointer, value| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(pointer),
                value: ValueId(value),
                access,
            },
        )
    };
    let call = |name: &str, args: &[u32]| {
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new(name),
                arguments: args.iter().copied().map(ValueId).collect(),
            },
        )
    };
    let mut root_blocks = (0..4)
        .map(|index| BasicBlock::new(BlockId(index)))
        .collect::<Vec<_>>();
    root_blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(0), rw.clone()),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), rw.clone()),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            OperationKind::Constant(Constant::U32(10)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Constant(Constant::U32(20)),
        ),
        store(0, 2),
        store(1, 3),
        call(names[1], &[0, 1]),
    ];
    if source_initialization {
        let operations = &mut root_blocks[0].operations;
        operations.swap(0, 2);
        operations.swap(1, 3);
        operations.swap(3, 4);
    }
    root_blocks[1].operations = vec![call(names[1], &[0, 1])];
    root_blocks[2].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), ro.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(0),
                to: ro.clone(),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), ro.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(1),
                to: ro.clone(),
            },
        ),
        call(names[2], &[4, 5]),
    ];
    root_blocks[3].operations = vec![load(6, 1)];
    for (index, block) in root_blocks.iter_mut().enumerate() {
        block.terminator = Some(if index == 3 {
            Terminator::Return { values: vec![] }
        } else {
            Terminator::Branch {
                target: BlockId(index as u32 + 1),
                arguments: vec![],
            }
        });
    }
    let mut mutable = BasicBlock::new(BlockId(0));
    mutable.operations = vec![load(2, 0), store(1, 2)];
    let mutable_blocks = if split_helper {
        mutable.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        let mut returned = BasicBlock::new(BlockId(1));
        returned.terminator = Some(Terminator::Return { values: vec![] });
        vec![mutable, returned]
    } else {
        mutable.terminator = Some(Terminator::Return { values: vec![] });
        vec![mutable]
    };
    let mut shared = BasicBlock::new(BlockId(0));
    shared.operations = vec![load(2, 1)];
    shared.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("borrowed-replay-test");
    module.functions = vec![
        Function::kernel_entry(
            names[0],
            Signature::new(vec![], vec![]),
            vec![],
            root_blocks,
        ),
        Function::internal_helper(
            names[1],
            Signature::new(vec![rw.clone(), rw], vec![]),
            vec![ValueId(0), ValueId(1)],
            mutable_blocks,
        ),
        Function::internal_helper(
            names[2],
            Signature::new(vec![ro.clone(), ro], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![shared],
        ),
    ];
    module.kernels.push(Kernel::new(
        names[0],
        names[0],
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let root = SemanticFunctionIdV1::from_index(0);
    correspondence.semantic_sha256 = *source.source_semantic().semantic_sha256().as_bytes();
    correspondence.function_count = 3;
    correspondence.lowered_functions = names
        .iter()
        .enumerate()
        .map(|(index, name)| SemanticKirFunctionCorrespondenceV1 {
            correspondence_owner: root,
            semantic_function: SemanticFunctionIdV1::from_index(index as u32),
            kernel_ir_function: FunctionId::new(*name),
            role: if index == 0 {
                SemanticKirFunctionRoleV1::KernelEntry
            } else {
                SemanticKirFunctionRoleV1::InternalHelper
            },
        })
        .collect();
    correspondence.borrowed_parameter_bindings = [(1, MUT), (2, SHARED)]
        .into_iter()
        .flat_map(|(function, reference_type)| {
            (0..2).map(move |field| SemanticKirBorrowedParameterBindingV1 {
                correspondence_owner: root,
                semantic_function: SemanticFunctionIdV1::from_index(function),
                semantic_local: SemanticLocalIdV1::from_index(1),
                reference_type,
                semantic_component_type: U32,
                projection: vec![field].into_boxed_slice(),
                transport: SemanticKirBorrowedParameterTransportV1::ScalarAddress,
                kernel_ir_value: ValueId(field),
            })
        })
        .collect();
    correspondence.blocks = source
        .source_semantic()
        .functions()
        .iter()
        .enumerate()
        .flat_map(|(function, source)| {
            source
                .blocks()
                .iter()
                .enumerate()
                .map(move |(block, body)| SemanticKirBlockCorrespondenceV1 {
                    correspondence_owner: root,
                    semantic_function: SemanticFunctionIdV1::from_index(function as u32),
                    semantic_block: SemanticBlockIdV1::from_index(block as u32),
                    kernel_ir_block: BlockId(block as u32),
                    source_statement_count: body.statements().len() as u32,
                })
        })
        .collect();
    correspondence.statement_operation_spans = [
        (
            0,
            0,
            0,
            if source_initialization { 0 } else { 2 },
            if source_initialization { 6 } else { 4 },
        ),
        (0, 0, 1, 6, 0),
        (0, 1, 0, 0, 0),
        (0, 2, 0, 0, 2),
        (0, 3, 0, 0, 1),
        (1, 0, 0, 0, 1),
        (1, 0, 1, 1, 1),
        (2, 0, 0, 0, 1),
    ]
    .into_iter()
    .map(
        |(function, block, statement, first, count)| SemanticKirStatementOperationSpanV1 {
            correspondence_owner: root,
            semantic_function: SemanticFunctionIdV1::from_index(function),
            semantic_block: SemanticBlockIdV1::from_index(block),
            statement_ordinal: statement,
            kernel_ir_block: BlockId(block),
            first_operation_ordinal: first,
            operation_count: count,
        },
    )
    .collect();
    let mut term = vec![
        (0, 0, 6, 1),
        (0, 1, 0, 1),
        (0, 2, 2, 1),
        (0, 3, 1, 0),
        (1, 0, 2, 0),
        (2, 0, 1, 0),
    ];
    if split_helper {
        term.insert(5, (1, 1, 0, 0));
    }
    correspondence.terminator_operation_spans = term
        .iter()
        .copied()
        .map(
            |(function, block, first, count)| SemanticKirTerminatorOperationSpanV1 {
                correspondence_owner: root,
                semantic_function: SemanticFunctionIdV1::from_index(function),
                semantic_block: SemanticBlockIdV1::from_index(block),
                kernel_ir_block: BlockId(block),
                first_operation_ordinal: first,
                operation_count: count,
            },
        )
        .collect();
    correspondence.call_returns = term
        .into_iter()
        .filter(|(function, block, _, _)| !split_helper || (*function, *block) != (1, 0))
        .map(|(function, block, first, count)| SemanticKirCallReturnV1 {
            correspondence_owner: root,
            semantic_function: SemanticFunctionIdV1::from_index(function),
            semantic_block: SemanticBlockIdV1::from_index(block),
            kind: if count == 1 {
                SemanticKirCallReturnKindV1::Call {
                    arguments_first: first,
                    call_operation: first,
                    destination_end: first + 1,
                    destination: SemanticKirCallDestinationV1::Local,
                    transport: CallComponentSpanV1::EMPTY,
                }
            } else {
                SemanticKirCallReturnKindV1::Return {
                    components: CallComponentSpanV1::EMPTY,
                }
            },
        })
        .collect();
    correspondence.synthetic_operation_spans = if source_initialization {
        Box::default()
    } else {
        vec![SemanticKirSyntheticOperationSpanV1 {
            correspondence_owner: root,
            semantic_function: root,
            rule: SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
            kernel_ir_block: BlockId(0),
            first_operation_ordinal: 0,
            operation_count: 2,
        }]
        .into_boxed_slice()
    };
    let anchor = BorrowedAggregateSourceAnchorV1::Occurrence(
        fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
            block: SsaBlockIdV1::new(0),
            statement: 0,
        },
    );
    let owner = BorrowedAggregateSourceOwnerV1 {
        root,
        function: root,
        local: SemanticLocalIdV1::from_index(1),
        lifetime_start: anchor,
    };
    let fields = (0..2)
        .map(|field| BorrowedAggregateFieldCandidateV1 {
            owner,
            path: BorrowedAggregateFieldPathV1 {
                fields: vec![field].into_boxed_slice(),
            },
            source_type: U32,
            source_definition: anchor,
            carrier: BorrowedAggregateCarrierCandidateV1::ScalarCell {
                pointer: BorrowedAggregateValueLocatorV1 {
                    function: FunctionId::new(names[0]),
                    value: ValueId(field),
                },
                allocation: BorrowedAggregateOperationLocatorV1 {
                    function: FunctionId::new(names[0]),
                    location: FunctionOperationLocation::new(
                        BlockId(0),
                        if source_initialization {
                            2 + field as usize * 2
                        } else {
                            field as usize
                        },
                    ),
                },
                initialization: BorrowedAggregateOperationLocatorV1 {
                    function: FunctionId::new(names[0]),
                    location: FunctionOperationLocation::new(
                        BlockId(0),
                        if source_initialization {
                            3 + field as usize * 2
                        } else {
                            field as usize + 4
                        },
                    ),
                },
            },
        })
        .collect();
    let mut calls = vec![];
    for (block, operation, callee, actual) in [(0, 6, 1, 0), (1, 0, 1, 0), (2, 2, 2, 4)] {
        for field in 0..2 {
            calls.push(BorrowedAggregateCallCandidateV1 {
                root,
                caller: root,
                block: SemanticBlockIdV1::from_index(block),
                call: BorrowedAggregateOperationLocatorV1 {
                    function: FunctionId::new(names[0]),
                    location: FunctionOperationLocation::new(BlockId(block), operation),
                },
                source_argument: 0,
                tuple_field: None,
                actual_owner: owner,
                actual_path: BorrowedAggregateFieldPathV1 {
                    fields: vec![field].into_boxed_slice(),
                },
                actual: BorrowedAggregateValueLocatorV1 {
                    function: FunctionId::new(names[0]),
                    value: ValueId(actual + field),
                },
                physical_argument: field,
                callee: SemanticFunctionIdV1::from_index(callee as u32),
                formal_local: SemanticLocalIdV1::from_index(1),
                formal_path: BorrowedAggregateFieldPathV1 {
                    fields: vec![field].into_boxed_slice(),
                },
                formal: BorrowedAggregateValueLocatorV1 {
                    function: FunctionId::new(names[callee]),
                    value: ValueId(field),
                },
                physical_parameter: field,
            });
        }
    }
    Fixture {
        source,
        module,
        correspondence,
        fields,
        calls,
    }
}

fn check(
    fixture: &Fixture,
    work_limit: usize,
    storage_limit: usize,
) -> Result<SealedBorrowedAggregateReplayV1, ProductionSemanticKirErrorV1> {
    check_with(fixture, work_limit, storage_limit, |result, _, _| result)
}

fn check_with<R>(
    fixture: &Fixture,
    work_limit: usize,
    storage_limit: usize,
    inspect: impl FnOnce(
        Result<SealedBorrowedAggregateReplayV1, ProductionSemanticKirErrorV1>,
        &mut ArgumentBudgetV1<'_>,
        usize,
    ) -> R,
) -> R {
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, VerifiedCanonicalKernelIrModuleV12};
    let mut preparation_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut preparation = ArgumentBudgetV1::new(&mut preparation_work, 100_000_000);
    let (graph, graph_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &fixture.module,
            &mut preparation,
        )
        .unwrap();
    preparation
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&graph, &mut preparation).unwrap();
    let floor = graph_storage.retained_storage() + inventory_storage.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, floor + storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = check_borrowed_aggregate_replay_v1(
        CanonicalCallSubjectV1 {
            semantic_ssa: &fixture.source,
            executable: &graph,
            correspondence: &fixture.correspondence,
        },
        &inventory,
        BorrowedAggregateReplayCandidatesV1 {
            fields: &fixture.fields,
            calls: &fixture.calls,
        },
        &mut budget,
    );
    let retained = match &result {
        Ok(sealed) => {
            assert_eq!(sealed.source_identity, fixture.source.identity());
            assert_eq!(&sealed.graph_identity, graph.canonical().identity());
            sealed.retained_storage()
        }
        Err(_) => 0,
    };
    assert_eq!(
        budget.storage(),
        floor + retained,
        "only sealed coverage survives scratch cleanup"
    );
    inspect(result, &mut budget, floor)
}
