use super::*;
use fe2o3_kernel_ir::{AddressSpace, BasicBlock, Constant, MemoryAccess, ValueId};

#[derive(Clone, Copy)]
enum IndexSource {
    Argument,
    Literal,
    Phi,
}

#[derive(Clone, Copy)]
enum Effect {
    Assign,
    Store,
    WriteRead,
    VolatileStore,
}

fn indexed_place(index: u32, invalid_projection: bool) -> SemanticPlaceV1 {
    let scalar = SemanticTypeIdV1::from_index(1);
    let projection = if invalid_projection {
        SemanticProjectionKindV1::ConstantIndex {
            offset: 0,
            minimum_length: 1,
            from_end: false,
        }
    } else {
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index))
    };
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                SemanticTypeIdV1::from_index(2),
            )
            .unwrap(),
            SemanticProjectionV1::new(projection, scalar).unwrap(),
        ],
        scalar,
    )
    .unwrap()
}

fn assignment(destination: SemanticPlaceV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    let ty = destination.ty();
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(value)),
        )),
    )
}

fn request(
    index: IndexSource,
    effect: Effect,
    mutability: SemanticMutabilityV1,
    invalid_projection: bool,
) -> InertSemanticMirRequestV1 {
    // Reuse only the qualified layout/ABI declarations, then admit the new AST.
    let original_owner = indexed_slice_borrow_owner();
    let semantic = original_owner.semantic();
    let original = &semantic.functions()[0];
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let slice_ref = SemanticTypeIdV1::from_index(3);
    let u64_ty = SemanticTypeIdV1::from_index(4);
    // The indexed store has no thin-reference result. Keep only its actual type closure.
    let mut types = semantic.types()[..4].to_vec();
    types.push(semantic.types()[5].clone());
    let mut abi = original.abi().clone();
    if mutability == SemanticMutabilityV1::Mutable {
        let reference = &types[3];
        types[3] = SemanticTypeDeclV1::new(
            reference.identity(),
            reference.layout_identity(),
            reference.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(2),
                    SemanticPointerKindV1::Reference,
                    mutability,
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
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
        let SemanticAbiPassModeV1::Pair { second, .. } = abi.arguments()[0].value().mode() else {
            panic!("the source slice must retain its scalar-pair ABI")
        };
        let mutable_argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            slice_ref,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    Some(4),
                )
                .unwrap(),
                second: *second,
            },
        ));
        abi = SemanticFunctionAbiV1::from_rustc(
            abi.identity(),
            abi.layout_identity(),
            abi.canon_abi(),
            abi.extern_abi(),
            abi.can_unwind(),
            abi.c_variadic(),
            2,
            vec![mutable_argument, abi.arguments()[1].clone()],
            abi.return_value().clone(),
        )
        .unwrap();
    }
    // Remap the index argument without changing its qualified ABI attributes.
    assert!(abi.arguments()[1].value().adjusted().is_none());
    assert!(abi.arguments()[1].value().pointee_override().is_none());
    abi = SemanticFunctionAbiV1::from_rustc(
        abi.identity(),
        abi.layout_identity(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.c_variadic(),
        2,
        vec![
            abi.arguments()[0].clone(),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                u64_ty,
                abi.arguments()[1].mode().clone(),
            )),
        ],
        abi.return_value().clone(),
    )
    .unwrap();
    abi = abi
        .with_source_argument_ownership(vec![
            match mutability {
                SemanticMutabilityV1::Immutable => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticMutabilityV1::Mutable => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            },
            SemanticSourceArgumentOwnershipV1::ByValue,
        ])
        .unwrap();
    let mut locals = original.locals()[..3].to_vec();
    let index_argument = &original.locals()[2];
    locals[2] = SemanticLocalDeclV1::new(
        index_argument.identity(),
        u64_ty,
        index_argument.role(),
        index_argument.source(),
    );
    for (tag, ty) in [(180, u32_ty), (181, u64_ty)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let index_local = match index {
        IndexSource::Argument => 2,
        IndexSource::Literal | IndexSource::Phi => 4,
    };
    let destination = indexed_place(index_local, invalid_projection);
    let value = scalar_constant(u32_ty, 42, 4);
    let mut effects = vec![match effect {
        Effect::Assign | Effect::WriteRead => assignment(destination.clone(), value),
        Effect::Store | Effect::VolatileStore => SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                destination.clone(),
                value,
                match effect {
                    Effect::VolatileStore => SemanticVolatilityV1::Volatile,
                    _ => SemanticVolatilityV1::NonVolatile,
                },
                None,
            )),
        ),
    }];
    if matches!(effect, Effect::WriteRead) {
        effects.push(assignment(
            local_place(3, u32_ty),
            SemanticOperandV1::Copy(destination),
        ));
    }
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let blocks = match index {
        IndexSource::Argument => vec![block(190, effects, SemanticTerminatorKindV1::Return)],
        IndexSource::Literal => {
            let mut statements = vec![assignment(
                local_place(4, u64_ty),
                scalar_constant(u64_ty, 3, 8),
            )];
            statements.extend(effects);
            vec![block(190, statements, SemanticTerminatorKindV1::Return)]
        }
        IndexSource::Phi => vec![
            block(
                190,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(local_place(2, u64_ty)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                191,
                vec![assignment(
                    local_place(4, u64_ty),
                    scalar_constant(u64_ty, 1, 8),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                192,
                vec![assignment(
                    local_place(4, u64_ty),
                    SemanticOperandV1::Copy(local_place(2, u64_ty)),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(193, effects, SemanticTerminatorKindV1::Return),
        ],
    };
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn source_owner(
    index: IndexSource,
    effect: Effect,
    mutability: SemanticMutabilityV1,
) -> ProductionSemanticMirOwnerV1 {
    let admitted = request(index, effect, mutability, false)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn lower(index: IndexSource, effect: Effect) -> ProductionSemanticKirOwnerV1 {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        source_owner(index, effect, SemanticMutabilityV1::Mutable),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    assert_eq!(
        lowered.module().functions[0].signature.parameters,
        vec![
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::Scalar(ScalarType::U64),
        ]
    );
    lowered
}

fn check_address(block: &BasicBlock, first: usize, carrier: ValueId, index: ValueId) -> ValueId {
    let operations = &block.operations[first..first + 3];
    assert!(matches!(
        operations[0].kind,
        OperationKind::Cast { kind: CastKind::Bitcast, value, ref to }
            if value == index && *to == Type::INDEX
    ));
    assert_eq!(operations[0].results[0].ty, Type::INDEX);
    assert!(matches!(operations[1].kind, OperationKind::SliceData { slice } if slice == carrier));
    assert!(matches!(
        operations[2].kind,
        OperationKind::GetElementPointer { base, offset }
            if base == operations[1].results[0].id && offset == operations[0].results[0].id
    ));
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    assert_eq!(operations[1].results[0].ty, pointer);
    assert_eq!(operations[2].results[0].ty, pointer);
    operations[2].results[0].id
}

fn check_write(block: &BasicBlock, first: usize, carrier: ValueId, index: ValueId) {
    assert_eq!(
        block.operations[first].kind,
        OperationKind::Constant(Constant::U32(42))
    );
    let pointer = check_address(block, first + 1, carrier, index);
    assert_eq!(
        block.operations[first + 4].kind,
        OperationKind::Store {
            pointer,
            value: block.operations[first].results[0].id,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        }
    );
}

fn check_span(
    lowered: &ProductionSemanticKirOwnerV1,
    block: u32,
    statement: u32,
    first: u32,
    count: u32,
) {
    let span = lowered
        .correspondence()
        .statement_operation_spans()
        .iter()
        .find(|span| {
            span.semantic_function().index() == 0
                && span.semantic_block().index() == block
                && span.statement_ordinal() == statement
        })
        .unwrap();
    assert_eq!(span.first_operation_ordinal(), first);
    assert_eq!(span.operation_count(), count);
}

#[test]
fn mutable_slice_assignment_uses_the_actual_argument_index_and_carrier() {
    let lowered = lower(IndexSource::Argument, Effect::Assign);
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks[0].operations.len(), 5);
    check_write(&body.blocks[0], 0, body.parameters[0], body.parameters[1]);
    check_span(&lowered, 0, 0, 0, 5);
}

#[test]
fn mutable_slice_assignment_preserves_a_source_literal_index_value() {
    let lowered = lower(IndexSource::Literal, Effect::Assign);
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let block = &body.blocks[0];
    assert_eq!(block.operations.len(), 6);
    assert_eq!(
        block.operations[0].kind,
        OperationKind::Constant(Constant::U64(3))
    );
    check_write(
        block,
        1,
        body.parameters[0],
        block.operations[0].results[0].id,
    );
    check_span(&lowered, 0, 0, 0, 1);
    check_span(&lowered, 0, 1, 1, 5);
}

#[test]
fn mutable_slice_assignment_consumes_the_exact_joined_ssa_index() {
    let lowered = lower(IndexSource::Phi, Effect::Assign);
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let join = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(3))
        .unwrap();
    assert_eq!(join.parameters.len(), 1);
    assert_eq!(join.parameters[0].ty, Type::Scalar(ScalarType::U64));
    assert_eq!(join.operations.len(), 5);
    check_write(join, 0, body.parameters[0], join.parameters[0].id);
    let literal = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(1))
        .unwrap();
    assert_eq!(literal.operations.len(), 1);
    assert_eq!(
        literal.operations[0].kind,
        OperationKind::Constant(Constant::U64(1))
    );
    assert_eq!(
        literal.terminator,
        Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![literal.operations[0].results[0].id],
        })
    );
    let copied = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(2))
        .unwrap();
    assert!(copied.operations.is_empty());
    assert_eq!(
        copied.terminator,
        Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![body.parameters[1]],
        })
    );
    check_span(&lowered, 3, 0, 0, 5);
}

#[test]
fn nonvolatile_slice_store_uses_the_shared_assignment_route() {
    let lowered = lower(IndexSource::Argument, Effect::Store);
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks[0].operations.len(), 5);
    check_write(&body.blocks[0], 0, body.parameters[0], body.parameters[1]);
    check_span(&lowered, 0, 0, 0, 5);
}

#[test]
fn mutable_slice_write_then_read_keeps_both_ordered_memory_effects() {
    let lowered = lower(IndexSource::Argument, Effect::WriteRead);
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let block = &body.blocks[0];
    assert_eq!(block.operations.len(), 9);
    check_write(block, 0, body.parameters[0], body.parameters[1]);
    let pointer = check_address(block, 5, body.parameters[0], body.parameters[1]);
    assert_eq!(
        block.operations[8].kind,
        OperationKind::Load {
            pointer,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        }
    );
    assert_eq!(
        block.operations[8].results[0].ty,
        Type::Scalar(ScalarType::U32)
    );
    check_span(&lowered, 0, 0, 0, 5);
    check_span(&lowered, 0, 1, 5, 4);
}

fn check_refused(effect: Effect, mutability: SemanticMutabilityV1) {
    let result = ProductionSemanticKirOwnerV1::try_lower(
        source_owner(IndexSource::Argument, effect, mutability),
        ProductionSemanticKirLimitsV1::default(),
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::PlaceProjectionUnavailable {
            function: 0,
            block: 0,
            statement: Some(0),
            local: 1,
            binding: "ordinary value",
            projection,
            ..
        }) if projection == "Index(SemanticLocalIdV1(2))"
    ));
}

#[test]
fn shared_slice_assignment_and_store_do_not_gain_write_access() {
    check_refused(Effect::Assign, SemanticMutabilityV1::Immutable);
    check_refused(Effect::Store, SemanticMutabilityV1::Immutable);
}

#[test]
fn volatile_slice_store_keeps_its_existing_unsupported_boundary() {
    check_refused(Effect::VolatileStore, SemanticMutabilityV1::Mutable);
}

#[test]
fn slice_constant_index_projection_is_rejected_by_source_admission() {
    assert!(matches!(
        request(
            IndexSource::Argument,
            Effect::Assign,
            SemanticMutabilityV1::Mutable,
            true
        )
        .admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            ..
        })
    ));
}
