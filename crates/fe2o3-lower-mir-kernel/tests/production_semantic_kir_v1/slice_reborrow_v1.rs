use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, ValueId,
};
use fe2o3_mir_model::{
    SsaBlockIdV1, SsaEventV1, SsaPlannerErrorV1, SsaResolvedEventV1, SsaVariableIdV1,
};
use fe2o3_pliron::{
    ProductionSemanticSsaErrorV1, ProductionSemanticSsaEventRoleV1 as EventRole,
    ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole, ProductionSemanticSsaOwnerV1,
};

#[derive(Clone, Copy)]
enum Shape {
    SameBlock,
    Phi,
    MovedSource,
}

fn assignment(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn whole_slice_place(local: u32) -> SemanticPlaceV1 {
    let slice = SemanticTypeIdV1::from_index(2);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice).unwrap()],
        slice,
    )
    .unwrap()
}

fn reborrow(local: u32, kind: SemanticBorrowKindV1) -> SemanticStatementV1 {
    assignment(
        3,
        SemanticTypeIdV1::from_index(3),
        SemanticRvalueKindV1::Borrow {
            kind,
            place: whole_slice_place(local),
        },
    )
}

fn projections(kind: SemanticBorrowKindV1) -> Vec<SemanticStatementV1> {
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let slice = SemanticTypeIdV1::from_index(2);
    vec![
        assignment(
            4,
            SemanticTypeIdV1::from_index(5),
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: SemanticOperandV1::Copy(local_place(3, SemanticTypeIdV1::from_index(3))),
            },
        ),
        assignment(
            5,
            SemanticTypeIdV1::from_index(4),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(3),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice)
                            .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                            u32_ty,
                        )
                        .unwrap(),
                    ],
                    u32_ty,
                )
                .unwrap(),
            },
        ),
    ]
}

fn source_owner(shape: Shape, mutability: SemanticMutabilityV1) -> ProductionSemanticMirOwnerV1 {
    // Reuse only the admitted type/layout/ABI fixture, then re-admit a new AST.
    // The original fixture's indexed thin borrow is not the new reborrow oracle.
    let original_owner = indexed_slice_borrow_owner();
    let semantic = original_owner.semantic();
    let original = &semantic.functions()[0];
    let slice_ref = SemanticTypeIdV1::from_index(3);
    let mut types = semantic.types().to_vec();
    let kind = match mutability {
        SemanticMutabilityV1::Immutable => SemanticBorrowKindV1::Shared,
        SemanticMutabilityV1::Mutable => SemanticBorrowKindV1::Mutable,
    };
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    };
    let mut locals = original.locals()[..3].to_vec();
    locals.extend([
        local(180, slice_ref, SemanticLocalRoleV1::Temporary),
        local(
            181,
            SemanticTypeIdV1::from_index(5),
            SemanticLocalRoleV1::Temporary,
        ),
        local(
            182,
            SemanticTypeIdV1::from_index(4),
            SemanticLocalRoleV1::Temporary,
        ),
    ]);
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
        types[4] = u32_reference_type(67, SemanticTypeIdV1::from_index(1), mutability);
        let SemanticAbiPassModeV1::Pair { second, .. } = abi.arguments()[0].value().mode() else {
            panic!("the admitted slice fixture must retain its scalar-pair ABI")
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
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let blocks = match shape {
        Shape::SameBlock | Shape::MovedSource => {
            let mut statements = Vec::new();
            if matches!(shape, Shape::MovedSource) {
                locals.push(local(183, slice_ref, SemanticLocalRoleV1::Temporary));
                statements.push(assignment(
                    6,
                    slice_ref,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(local_place(1, slice_ref))),
                ));
            }
            statements.push(reborrow(1, kind));
            statements.extend(projections(kind));
            vec![block(184, statements, SemanticTerminatorKindV1::Return)]
        }
        Shape::Phi => {
            locals.push(local(183, slice_ref, SemanticLocalRoleV1::Argument(2)));
            abi = SemanticFunctionAbiV1::from_rustc(
                abi.identity(),
                abi.layout_identity(),
                abi.canon_abi(),
                abi.extern_abi(),
                abi.can_unwind(),
                abi.c_variadic(),
                3,
                vec![
                    abi.arguments()[0].clone(),
                    abi.arguments()[1].clone(),
                    abi.arguments()[0].clone(),
                ],
                abi.return_value().clone(),
            )
            .unwrap();
            vec![
                block(
                    184,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(local_place(
                            2,
                            SemanticTypeIdV1::from_index(5),
                        )),
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
                    185,
                    vec![reborrow(1, kind)],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    186,
                    vec![reborrow(6, kind)],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(187, projections(kind), SemanticTerminatorKindV1::Return),
            ]
        }
    };
    let borrow_ownership = match mutability {
        SemanticMutabilityV1::Immutable => SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticMutabilityV1::Mutable => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    };
    let mut ownership = vec![borrow_ownership, SemanticSourceArgumentOwnershipV1::ByValue];
    if matches!(shape, Shape::Phi) {
        ownership.push(borrow_ownership);
    }
    abi = abi
        .with_source_argument_ownership(ownership.clone())
        .unwrap();
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
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(
        admitted.functions()[0].abi().source_argument_ownership(),
        ownership
    );
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn check_ssa_occurrences(mutability: SemanticMutabilityV1) {
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        source_owner(Shape::SameBlock, mutability),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 0);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let view = owner.occurrences_v1().unwrap();
    let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert!(rows.elisions().is_empty());
    let variable = SsaVariableIdV1::new;
    let expected = [
        (
            0,
            OperandRole::RvaluePlace,
            EventRole::BaseUse,
            SsaEventV1::Use(variable(1)),
        ),
        (
            0,
            OperandRole::Destination,
            EventRole::DestinationDefine,
            SsaEventV1::Define(variable(3)),
        ),
        (
            1,
            OperandRole::RvalueOperand(0),
            EventRole::BaseUse,
            SsaEventV1::Use(variable(3)),
        ),
        (
            1,
            OperandRole::Destination,
            EventRole::DestinationDefine,
            SsaEventV1::Define(variable(4)),
        ),
        (
            2,
            OperandRole::RvaluePlace,
            EventRole::BaseUse,
            SsaEventV1::Use(variable(3)),
        ),
        (
            2,
            OperandRole::RvaluePlace,
            EventRole::ProjectionIndexUse(1),
            SsaEventV1::Use(variable(2)),
        ),
        (
            2,
            OperandRole::Destination,
            EventRole::DestinationDefine,
            SsaEventV1::Define(variable(5)),
        ),
    ];
    assert_eq!(rows.events().len(), expected.len());
    for (ordinal, (row, (statement, operand, role, event))) in
        rows.events().iter().zip(expected).enumerate()
    {
        assert_eq!(
            row.site(),
            Site::Statement {
                block: SsaBlockIdV1::new(0),
                statement
            }
        );
        assert_eq!(row.ordinal(), u32::try_from(ordinal).unwrap());
        assert_eq!(
            (row.operand(), row.role(), row.event()),
            (operand, role, event)
        );
        assert!(row.is_reachable());
        assert!(row.is_promoted());
        assert!(row.resolved().is_some());
    }
    let Some(SsaResolvedEventV1::Define {
        variable: defined,
        value,
    }) = rows.events()[1].resolved()
    else {
        panic!("the reborrow must define an actual SSA value")
    };
    assert_eq!(defined, variable(3));
    for ordinal in [2, 4] {
        assert_eq!(
            rows.events()[ordinal].resolved(),
            Some(SsaResolvedEventV1::Use {
                variable: variable(3),
                value,
            })
        );
    }
    owner.verify_replay().unwrap();
    assert!(!owner.grants_proof_or_artifact_authority());
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn check_moved_source(mutability: SemanticMutabilityV1) {
    // U1, K1, D6 precede the reborrow's U1 at event 3.
    assert!(matches!(
        ProductionSemanticSsaOwnerV1::try_new(
            source_owner(Shape::MovedSource, mutability),
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::Planner {
            function,
            error: SsaPlannerErrorV1::UndefinedAtUse { block, event: 3, variable },
        }) if function.index() == 0 && block == SsaBlockIdV1::new(0)
            && variable == SsaVariableIdV1::new(1)
    ));
}

fn slice_access(mutability: SemanticMutabilityV1) -> AccessMode {
    match mutability {
        SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
        SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
    }
}

fn slice_type(mutability: SemanticMutabilityV1) -> Type {
    Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        slice_access(mutability),
    )
}

fn assert_projected_pair(
    block: &fe2o3_kernel_ir::BasicBlock,
    carrier: ValueId,
    index: ValueId,
    mutability: SemanticMutabilityV1,
) {
    // Length, exact U64->Index conversion, data extraction, indexed address.
    assert_eq!(block.operations.len(), 4);
    assert!(
        matches!(block.operations[0].kind, OperationKind::SliceLength { slice } if slice == carrier)
    );
    assert!(
        matches!(block.operations[1].kind, OperationKind::Cast { kind: CastKind::Bitcast, value, ref to } if value == index && *to == Type::INDEX)
    );
    assert!(
        matches!(block.operations[2].kind, OperationKind::SliceData { slice } if slice == carrier)
    );
    assert!(
        matches!(block.operations[3].kind, OperationKind::GetElementPointer { base, offset }
        if base == block.operations[2].results[0].id && offset == block.operations[1].results[0].id)
    );
    assert_eq!(block.operations[0].results[0].ty, Type::INDEX);
    assert_eq!(
        block.operations[2].results[0].ty,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            slice_access(mutability)
        )
    );
}

fn assert_zero_reborrow_span(lowered: &ProductionSemanticKirOwnerV1, block: u32) {
    let spans = lowered.correspondence().statement_operation_spans();
    let span = spans
        .iter()
        .find(|span| {
            span.semantic_function().index() == 0
                && span.semantic_block().index() == block
                && span.statement_ordinal() == 0
        })
        .unwrap();
    assert_eq!(span.operation_count(), 0);
    assert_eq!(span.first_operation_ordinal(), 0);
}

fn check_same_block_carrier(mutability: SemanticMutabilityV1) {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        source_owner(Shape::SameBlock, mutability),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let function = &lowered.module().functions[0];
    assert_eq!(
        function.signature.parameters,
        vec![slice_type(mutability), Type::Scalar(ScalarType::U64)]
    );
    let body = function.body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    assert_zero_reborrow_span(&lowered, 0);
    assert_projected_pair(
        &body.blocks[0],
        body.parameters[0],
        body.parameters[1],
        mutability,
    );
}

fn check_phi_carrier(mutability: SemanticMutabilityV1) {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        source_owner(Shape::Phi, mutability),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let function = &lowered.module().functions[0];
    assert_eq!(
        function.signature.parameters,
        vec![
            slice_type(mutability),
            Type::Scalar(ScalarType::U64),
            slice_type(mutability)
        ]
    );
    let body = function.body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 4);
    let join = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(3))
        .unwrap();
    assert_eq!(join.parameters.len(), 1);
    assert_eq!(join.parameters[0].ty, slice_type(mutability));
    for (source_block, parameter) in [(1, 0), (2, 2)] {
        assert_zero_reborrow_span(&lowered, source_block);
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == BlockId(source_block))
            .unwrap();
        assert!(block.operations.is_empty());
        assert_eq!(
            block.terminator,
            Some(Terminator::Branch {
                target: BlockId(3),
                arguments: vec![body.parameters[parameter]],
            })
        );
    }
    assert_projected_pair(join, join.parameters[0].id, body.parameters[1], mutability);
}

#[test]
fn same_type_slice_reborrow_retains_exact_ssa_use_define_and_no_elision() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        check_ssa_occurrences(mutability);
    }
}

#[test]
fn same_type_slice_reborrow_cannot_revive_a_moved_reference() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        check_moved_source(mutability);
    }
}

#[test]
fn same_type_slice_reborrow_preserves_the_single_data_length_carrier() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        check_same_block_carrier(mutability);
    }
}

#[test]
fn same_type_slice_reborrow_phi_transports_one_paired_carrier_from_each_source() {
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        check_phi_carrier(mutability);
    }
}
