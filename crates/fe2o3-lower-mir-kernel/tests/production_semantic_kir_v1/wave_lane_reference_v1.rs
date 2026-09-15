use super::*;
use fe2o3_kernel_ir::ValueId;
use fe2o3_mir_model::{SsaBlockIdV1, SsaPlannerErrorV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticSsaErrorV1, ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LANE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const FRAGMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const MUTABLE_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

#[derive(Clone, Copy, Debug)]
enum Shape {
    Reborrow,
    Mutable,
    Copy,
    Move,
    Phi,
    MovedReference,
    Field,
    RawAddress,
}

fn aggregate_type(
    tag: u8,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    size: u64,
    backend: SemanticBackendReprV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size,
            4,
            SemanticFieldsShapeV1::arbitrary(offsets.clone(), (0..fields.len() as u32).collect())
                .unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            backend,
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    )
}

fn fixture_types() -> Vec<SemanticTypeDeclV1> {
    let lane = aggregate_type(
        202,
        vec![U32],
        vec![0],
        4,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        )),
    );
    let fragment = aggregate_type(
        205,
        vec![F32; 4],
        vec![0, 4, 8, 12],
        16,
        SemanticBackendReprV1::memory(true),
    );
    let raw = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(206)),
        SemanticLayoutIdentityV1::from_sha256(bytes(206)),
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
                LANE,
                SemanticPointerKindV1::Raw,
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
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    );
    vec![
        unit_type(),
        scalar_type(
            201,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        lane,
        reference_type(203, LANE, 4, 4, SemanticMutabilityV1::Immutable),
        scalar_type(204, SemanticScalarTypeV1::Float { bits: 32 }),
        fragment,
        raw,
        reference_type(207, LANE, 4, 4, SemanticMutabilityV1::Mutable),
    ]
}

fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn dereference(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, LANE).unwrap()],
        LANE,
    )
    .unwrap()
}

fn borrow(local: u32, place: SemanticPlaceV1) -> SemanticStatementV1 {
    assign(
        local,
        REFERENCE,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        },
    )
}

fn call(
    callable: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callable),
            arguments,
            Some(SemanticCallDestinationV1::new(
                local_place(destination, ty),
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

fn zero_call(target: u32) -> SemanticTerminatorKindV1 {
    call(
        2,
        vec![SemanticOperandV1::Copy(local_place(3, REFERENCE))],
        4,
        FRAGMENT,
        target,
    )
}

fn source_owner(shape: Shape) -> ProductionSemanticMirOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let first_reference = if matches!(shape, Shape::Mutable) {
        MUTABLE_REFERENCE
    } else {
        REFERENCE
    };
    let locals = [
        UNIT,
        LANE,
        first_reference,
        REFERENCE,
        FRAGMENT,
        U32,
        REFERENCE,
        LANE,
        RAW,
        U32,
        MUTABLE_REFERENCE,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(210 + index as u8)),
            ty,
            match index {
                0 => SemanticLocalRoleV1::Return,
                5 => SemanticLocalRoleV1::Argument(0),
                _ => SemanticLocalRoleV1::Temporary,
            },
            source,
        )
    })
    .collect();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(220)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(U32))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let issue = block(221, vec![], call(1, vec![], 1, LANE, 1));
    let first_borrow = if matches!(shape, Shape::Mutable) {
        assign(
            2,
            MUTABLE_REFERENCE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: local_place(1, LANE),
            },
        )
    } else {
        borrow(2, local_place(1, LANE))
    };
    let blocks = if matches!(shape, Shape::Phi) {
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        vec![
            issue,
            block(
                222,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(local_place(5, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(
                223,
                vec![assign(
                    7,
                    LANE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, LANE))),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                224,
                vec![assign(
                    7,
                    LANE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(local_place(1, LANE))),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                225,
                vec![borrow(2, local_place(7, LANE)), borrow(3, dereference(2))],
                zero_call(5),
            ),
            block(226, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        let mut statements = if matches!(shape, Shape::Copy | Shape::Move) {
            vec![]
        } else {
            vec![first_borrow]
        };
        match shape {
            Shape::Reborrow | Shape::Mutable => statements.push(borrow(3, dereference(2))),
            Shape::Copy | Shape::Move => {
                let operand = if matches!(shape, Shape::Move) {
                    SemanticOperandV1::Move(local_place(1, LANE))
                } else {
                    SemanticOperandV1::Copy(local_place(1, LANE))
                };
                statements.push(assign(7, LANE, SemanticRvalueKindV1::Use(operand)));
                statements.push(borrow(2, local_place(7, LANE)));
                statements.push(borrow(3, dereference(2)));
            }
            Shape::MovedReference => {
                statements.push(assign(
                    6,
                    REFERENCE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(local_place(2, REFERENCE))),
                ));
                statements.push(borrow(3, dereference(2)));
            }
            Shape::Field => {
                let field = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, LANE)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap(),
                    ],
                    U32,
                )
                .unwrap();
                statements.push(assign(
                    9,
                    U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                ));
                statements.push(borrow(3, dereference(2)));
            }
            Shape::RawAddress => {
                statements.push(assign(
                    8,
                    RAW,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Immutable,
                        place: dereference(2),
                    },
                ));
                statements.push(borrow(3, dereference(2)));
            }
            Shape::Phi => unreachable!(),
        }
        vec![
            issue,
            block(222, statements, zero_call(2)),
            block(223, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(220)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(220)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(220)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(220)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(220)),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"wave_lane_reference".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(227)),
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
    let borrowed_lane = SemanticAbiValueV1::new(
        REFERENCE,
        SemanticAbiPassModeV1::Direct(
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
                4,
                Some(4),
            )
            .unwrap(),
        ),
    );
    let fragment_return = SemanticAbiValueV1::new(
        FRAGMENT,
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
                Some(4),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        },
    );
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        compiler_intrinsic_callable(
            228,
            vec![],
            direct_abi_value(LANE),
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: LANE,
                wave_width: 64,
            },
        ),
        compiler_intrinsic_callable(
            229,
            vec![borrowed_lane],
            fragment_return,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                lane: LANE,
                fragment: FRAGMENT,
                contract: SemanticMfmaAccumulatorContractV1 {
                    profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
                    distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
                    wave_width: 64,
                },
            },
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        fixture_types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn lower(shape: Shape) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    ProductionSemanticKirOwnerV1::try_lower(
        source_owner(shape),
        ProductionSemanticKirLimitsV1::default(),
    )
}

fn assert_issued_lane_and_typed_consumer(
    owner: &ProductionSemanticKirOwnerV1,
    consumer_block: u32,
) -> ValueId {
    owner.verify_equivalence().unwrap();
    verify_module(owner.module()).unwrap();
    assert_eq!(owner.module().functions.len(), 1);
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let lanes = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Wave(fe2o3_kernel_ir::WaveOperation {
                    kind: fe2o3_kernel_ir::WaveOperationKind::LaneId,
                    ..
                })
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(lanes.len(), 1, "a reborrow must not issue another lane");
    assert_eq!(lanes[0].results.len(), 1);
    assert_eq!(lanes[0].results[0].ty, Type::Scalar(ScalarType::U32));
    let zeros = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Constant(fe2o3_kernel_ir::Constant::F32Bits(0))
            )
        })
        .count();
    assert_eq!(
        zeros, 4,
        "the actual typed zero-accumulator consumer must lower"
    );
    assert!(
        !body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::Alloca { .. }
                    | OperationKind::Load { .. }
                    | OperationKind::Store { .. }
            ))
    );
    let issue = owner
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .find(|span| span.semantic_block().index() == 0)
        .unwrap();
    assert_eq!(issue.operation_count(), 1);
    let consumer = owner
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .find(|span| span.semantic_block().index() == consumer_block)
        .unwrap();
    assert_eq!(consumer.operation_count(), 4);
    for span in owner.correspondence().statement_operation_spans() {
        assert_eq!(
            span.operation_count(),
            0,
            "Borrow/Copy/Move must preserve the issued wrapper without operations"
        );
    }
    lanes[0].results[0].id
}

#[test]
fn wave_lane_value_aliases_then_reference_reborrow_keep_one_issued_lane() {
    for shape in [Shape::Reborrow, Shape::Mutable, Shape::Copy, Shape::Move] {
        let owner = lower(shape).unwrap();
        assert_issued_lane_and_typed_consumer(&owner, 1);
    }
}

#[test]
fn wave_lane_value_phi_then_reference_reborrow_transports_the_issued_value() {
    // The phi transports the issued lane value, not a reference alias.
    let owner = lower(Shape::Phi).unwrap();
    let lane = assert_issued_lane_and_typed_consumer(&owner, 4);
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let join_id = owner
        .correspondence()
        .blocks()
        .iter()
        .find(|row| row.semantic_block().index() == 4)
        .unwrap()
        .kernel_ir_block();
    let join = body
        .blocks
        .iter()
        .find(|block| block.id == join_id)
        .unwrap();
    assert_eq!(join.parameters.len(), 1);
    assert_eq!(join.parameters[0].ty, Type::Scalar(ScalarType::U32));
    for source in [2, 3] {
        let predecessor_id = owner
            .correspondence()
            .blocks()
            .iter()
            .find(|row| row.semantic_block().index() == source)
            .unwrap()
            .kernel_ir_block();
        let predecessor = body
            .blocks
            .iter()
            .find(|block| block.id == predecessor_id)
            .unwrap();
        assert!(
            matches!(predecessor.terminator.as_ref(), Some(Terminator::Branch { target, arguments }) if *target == join_id && arguments == &[lane])
        );
    }
}

#[test]
fn wave_lane_reference_move_does_not_revive_the_old_reference() {
    // U1,D2 then U2,K2,D6 precede the forbidden reborrow U2 at event5.
    assert!(matches!(
        ProductionSemanticSsaOwnerV1::try_new(source_owner(Shape::MovedReference), ProductionSemanticSsaLimitsV1::default()),
        Err(ProductionSemanticSsaErrorV1::Planner { function, error: SsaPlannerErrorV1::UndefinedAtUse { block, event: 5, variable } })
            if function.index() == 0 && block == SsaBlockIdV1::new(1) && variable == SsaVariableIdV1::new(2)
    ));
}

#[test]
fn wave_lane_reference_field_use_keeps_the_retained_storage_gate() {
    // A field use invalidates transparent-borrow classification before lowering.
    assert!(
        matches!(lower(Shape::Field), Err(ProductionSemanticKirErrorV1::RetainedLocalStorage {
            function: 0, retained_locals, retained_count: 1,
        }) if retained_locals.len() == 1 && retained_locals[0].0 == 1 && retained_locals[0].1 == LANE.index())
    );
}

#[test]
fn wave_lane_reference_raw_address_keeps_the_retained_storage_gate() {
    // The source form is admitted, but cannot promote an escaping borrow chain.
    assert!(matches!(
        lower(Shape::RawAddress),
        Err(ProductionSemanticKirErrorV1::RetainedLocalStorage {
            function: 0,
            retained_locals,
            retained_count: 1,
        }) if retained_locals.len() == 1 && retained_locals[0].0 == 1 && retained_locals[0].1 == LANE.index()
    ));
}
