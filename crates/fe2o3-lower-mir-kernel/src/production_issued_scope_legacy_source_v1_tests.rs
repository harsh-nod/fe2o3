use super::*;

// Pure typed AST helpers reused without NativeSource test dependencies.
const SCOPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const SCOPE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const PIPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const PIPE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

fn pipeline_types() -> Vec<SemanticTypeDeclV1> {
    let mut result = types();
    result.truncate(4);
    for (tag, pointee) in [(150, SCOPE), (152, PIPE)] {
        result.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(0),
                1,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ));
        result.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
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
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
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
                            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                            0,
                            1,
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

fn pipeline_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let mode = if matches!(ty, UNIT | SCOPE | PIPE) {
        SemanticAbiPassModeV1::Ignore
    } else {
        let reference = matches!(ty, SCOPE_REF | PIPE_REF);
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, reference, false, false, true),
                if ty == BOOL {
                    SemanticAbiExtensionV1::ZeroExtend
                } else {
                    SemanticAbiExtensionV1::None
                },
                0,
                None,
            )
            .unwrap(),
        )
    };
    SemanticAbiValueV1::new(ty, mode)
}

fn pipeline_intrinsic(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs.iter().copied().map(pipeline_abi_value).collect(),
        pipeline_abi_value(output),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

fn pipeline_call(
    callable: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: (u32, SemanticTypeIdV1),
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callable),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination.0, destination.1),
                edge(SemanticEdgeRoleV1::CallReturn, next),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn pipeline_borrow(
    local: u32,
    ty: SemanticTypeIdV1,
    source: u32,
    pointee: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    assignment(
        local,
        ty,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: place(source, pointee),
        },
    )
}

fn pipeline_switch(discriminant: SemanticOperandV1, yes: u32, no: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
        )
        .unwrap(),
    }
}

fn pipeline_fixture(
    loop_carried: bool,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        pipeline_intrinsic(
            160,
            &[],
            SCOPE,
            SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope: SCOPE },
        ),
        pipeline_intrinsic(
            161,
            &[SCOPE_REF],
            PIPE,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                scope: SCOPE,
                pipeline: PIPE,
                buffers: 2,
                elements: 64,
                prefetch_distance: 1,
            },
        ),
        pipeline_intrinsic(
            162,
            &[PIPE_REF, U64, U64, U32],
            UNIT,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                pipeline: PIPE,
                element: U32,
            },
        ),
        pipeline_intrinsic(
            163,
            &[PIPE_REF, U64, U64],
            U32,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead {
                pipeline: PIPE,
                element: U32,
            },
        ),
    ];
    for (index, event) in [
        SemanticWorkgroupPipelineEventV1::Stage,
        SemanticWorkgroupPipelineEventV1::Commit,
        SemanticWorkgroupPipelineEventV1::Wait,
        SemanticWorkgroupPipelineEventV1::Consume,
        SemanticWorkgroupPipelineEventV1::Discard,
        SemanticWorkgroupPipelineEventV1::Release,
    ]
    .into_iter()
    .enumerate()
    {
        callables.push(pipeline_intrinsic(
            164 + index as u8,
            &[PIPE_REF, U64],
            UNIT,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
                pipeline: PIPE,
                event,
            },
        ));
    }
    let event_call = |callable, next| {
        pipeline_call(
            callable,
            vec![value(6, PIPE_REF), value(8, U64)],
            (0, UNIT),
            next,
        )
    };
    let reborrow = || pipeline_borrow(6, PIPE_REF, 10, PIPE);
    let blocks = vec![
        block(180, vec![], pipeline_call(1, vec![], (3, SCOPE), 1)),
        block(
            181,
            vec![pipeline_borrow(4, SCOPE_REF, 3, SCOPE)],
            pipeline_call(2, vec![value(4, SCOPE_REF)], (5, PIPE), 2),
        ),
        block(
            182,
            vec![
                assignment(8, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                assignment(7, U32, SemanticRvalueKindV1::Use(constant(U32, 7, 4))),
            ],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            183,
            vec![assignment(
                9,
                BOOL,
                if loop_carried {
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(8, U64),
                        right: value(2, U64),
                    }
                } else {
                    SemanticRvalueKindV1::Use(constant(BOOL, 1, 1))
                },
            )],
            pipeline_switch(value(9, BOOL), 4, 14),
        ),
        block(
            184,
            vec![
                assignment(
                    10,
                    PIPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(5, PIPE))),
                ),
                reborrow(),
            ],
            event_call(5, 5),
        ),
        block(
            185,
            vec![reborrow()],
            pipeline_call(
                3,
                vec![
                    value(6, PIPE_REF),
                    value(8, U64),
                    constant(U64, 0, 8),
                    value(7, U32),
                ],
                (0, UNIT),
                6,
            ),
        ),
        block(186, vec![reborrow()], event_call(6, 7)),
        block(187, vec![reborrow()], event_call(7, 8)),
        block(188, vec![], pipeline_switch(value(1, BOOL), 9, 11)),
        block(189, vec![reborrow()], event_call(8, 10)),
        block(
            190,
            vec![reborrow()],
            pipeline_call(
                4,
                vec![value(6, PIPE_REF), value(8, U64), constant(U64, 0, 8)],
                (7, U32),
                12,
            ),
        ),
        block(
            191,
            vec![
                reborrow(),
                assignment(7, U32, SemanticRvalueKindV1::Use(constant(U32, 7, 4))),
            ],
            event_call(9, 12),
        ),
        block(192, vec![reborrow()], event_call(10, 13)),
        block(
            193,
            vec![
                assignment(
                    5,
                    PIPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, PIPE))),
                ),
                assignment(
                    8,
                    U64,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: value(8, U64),
                        right: constant(U64, 1, 8),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Goto(edge(
                SemanticEdgeRoleV1::Goto,
                if loop_carried { 3 } else { 14 },
            )),
        ),
        block(
            194,
            vec![assignment(
                11,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Equal,
                    left: value(7, U32),
                    right: constant(U32, 7, 4),
                },
            )],
            SemanticTerminatorKindV1::Assert {
                condition: value(11, BOOL),
                expected: true,
                message: SemanticAssertMessageV1::NullPointerDereference,
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 15),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(195, vec![], SemanticTerminatorKindV1::Return),
    ];
    let local_types = [
        UNIT, BOOL, U64, SCOPE, SCOPE_REF, PIPE, PIPE_REF, U32, U64, BOOL, PIPE, BOOL,
    ];
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([200 + index as u8; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 | 2 => SemanticLocalRoleV1::Argument(index as u32 - 1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source,
            )
        })
        .collect();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([220; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(pipeline_abi_value(BOOL)),
            SemanticAbiArgumentV1::source(pipeline_abi_value(U64)),
        ],
        pipeline_abi_value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([221; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"native_pipeline_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([226; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        pipeline_types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "native_pipeline",
            [226; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}

fn scope_reborrow_source(successor: bool) -> ProductionSemanticSsaOwnerV1 {
    let (base, _) = pipeline_fixture(false);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let mut blocks = original.blocks().to_vec();
    let mut locals = original.locals().to_vec();
    assert_eq!(locals.len(), 12);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([212; 32]),
        SCOPE_REF,
        SemanticLocalRoleV1::Temporary,
        original.source(),
    ));
    let reborrow = assignment(
        12,
        SCOPE_REF,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCOPE)
                        .unwrap(),
                ],
                SCOPE,
            )
            .unwrap(),
        },
    );
    let create = pipeline_call(2, vec![value(12, SCOPE_REF)], (5, PIPE), 2);
    if successor {
        assert_eq!(blocks.len(), 16);
        blocks[1] = block(
            181,
            original.blocks()[1].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 16)),
        );
        blocks.push(block(196, vec![reborrow], create));
    } else {
        let mut statements = original.blocks()[1].statements().to_vec();
        statements.push(reborrow);
        blocks[1] = block(181, statements, create);
    }
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.callables().to_vec(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    drop(base);
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    ssa
}

#[test]
fn admitted_scope_reborrow_preserves_the_actual_pipeline_consumer() {
    for successor in [false, true] {
        let ssa = scope_reborrow_source(successor);
        let source_block = if successor { 16 } else { 1 };
        let source_statement = if successor { 0 } else { 1 };
        let semantic = ssa.source_semantic();
        assert!(matches!(
            semantic.callables()[1],
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent {
                    scope: SCOPE,
                },
                ..
            }
        ));
        let SemanticStatementKindV1::Assign(reborrow) =
            semantic.functions()[0].blocks()[source_block].statements()[source_statement].kind()
        else {
            panic!("actual reborrow assignment");
        };
        assert_eq!(reborrow.destination(), &place(12, SCOPE_REF));
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[crate::ProductionSourceLaunchRootInputV1::new(
                "native_pipeline",
                [226; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(ssa.occurrences_v1().is_none());
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        let retained = source
            .executable_storage()
            .retained_storage()
            .checked_add(source.assert_origin_storage().payload_storage())
            .unwrap();
        budget.reserve_storage(retained).unwrap();
        assert!(source.semantic_ssa().occurrences_v1().is_none());
        source.semantic_ssa().verify_replay().unwrap();
        fe2o3_kernel_ir::verify_module(source.executable().module()).unwrap();

        let spans = source.correspondence.statement_operation_spans();
        let new_span = spans
            .iter()
            .filter(|span| {
                span.correspondence_owner().index() == 0
                    && span.semantic_function().index() == 0
                    && span.semantic_block().index() == source_block as u32
                    && span.statement_ordinal() == source_statement as u32
            })
            .collect::<Vec<_>>();
        assert_eq!(new_span.len(), 1);
        assert_eq!(new_span[0].operation_count(), 0);
        let terminators = source.correspondence.terminator_operation_spans();
        for (block, count) in [(0, 0), (source_block as u32, 1)] {
            let span = terminators
                .iter()
                .find(|span| {
                    span.semantic_function().index() == 0 && span.semantic_block().index() == block
                })
                .unwrap();
            assert_eq!(span.operation_count(), count);
        }
        let allocations = source
            .executable()
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                fe2o3_kernel_ir::OperationKind::WorkgroupMemory(memory) => Some(memory),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(allocations.len(), 1);
        assert_eq!(
            allocations[0].extent,
            fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(128)
        );
        assert!(!source.grants_artifact_or_launch_authority());
        drop(source);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
