use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaPlannerErrorV1, SsaVariableIdV1};
use fe2o3_pliron::ProductionSemanticSsaErrorV1;

fn reference_call_owner(
    cross_block: bool,
    move_argument: bool,
    dead_reference: bool,
) -> ProductionSemanticMirOwnerV1 {
    let base = defined_helper_owner_v1(DefinedHelperFixtureV1::Valid);
    let semantic = base.semantic();
    let original = &semantic.functions()[0];
    let source = SemanticSourceProvenanceV1::unavailable();
    let scalar = SemanticTypeIdV1::from_index(1);
    let reference = SemanticTypeIdV1::from_index(2);
    assert_eq!(semantic.types().len(), 2);
    assert_eq!(semantic.functions().len(), 2);
    let mut types = semantic.types().to_vec();
    types.push(reference_type(
        244,
        scalar,
        8,
        8,
        SemanticMutabilityV1::Mutable,
    ));
    let mut locals = original.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(245)),
        reference,
        SemanticLocalRoleV1::Temporary,
        source,
    ));
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(246)),
        scalar,
        SemanticLocalRoleV1::Temporary,
        source,
    ));
    let assignment = |local, ty, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let mut setup = vec![
        assignment(
            2,
            scalar,
            SemanticRvalueKindV1::Use(scalar_constant(scalar, 0, 8)),
        ),
        assignment(
            3,
            reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: local_place(2, scalar),
            },
        ),
    ];
    if dead_reference {
        setup.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
        ));
    }
    let call_block = u32::from(cross_block);
    let destination = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap()],
        scalar,
    )
    .unwrap();
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![if move_argument {
                SemanticOperandV1::Move(local_place(1, scalar))
            } else {
                SemanticOperandV1::Copy(local_place(1, scalar))
            }],
            Some(SemanticCallDestinationV1::new(
                destination,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(call_block + 1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let mut blocks = if cross_block {
        vec![
            block(
                240,
                setup,
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            block(241, vec![], call),
        ]
    } else {
        vec![block(240, setup, call)]
    };
    blocks.push(block(
        242,
        vec![assignment(
            4,
            scalar,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(2, scalar))),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let root = SemanticFunctionDeclV1::new(
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
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn assert_reference_call_source(owner: &ProductionSemanticMirOwnerV1, cross_block: bool) {
    let scalar = SemanticTypeIdV1::from_index(1);
    let root = &owner.semantic().functions()[0];
    let call_block = usize::from(cross_block);
    assert_eq!(root.blocks().len(), call_block + 2);
    let SemanticTerminatorKindV1::Call(call) = root.blocks()[call_block].terminator().kind() else {
        panic!("the admitted source must retain the actual projected Call")
    };
    assert_eq!(call.callee(), SemanticCallableIdV1::from_index(1));
    assert_eq!(call.arguments().len(), 1);
    let (SemanticOperandV1::Copy(argument) | SemanticOperandV1::Move(argument)) =
        &call.arguments()[0]
    else {
        panic!("source helper argument must be the independent scalar local")
    };
    assert_eq!(argument, &local_place(1, scalar));
    let destination = call.destination().unwrap();
    assert_eq!(
        destination.place().local(),
        SemanticLocalIdV1::from_index(3)
    );
    assert_eq!(destination.place().ty(), scalar);
    assert_eq!(destination.place().projections().len(), 1);
    assert_eq!(
        destination.place().projections()[0].kind(),
        SemanticProjectionKindV1::Dereference
    );
    assert_eq!(destination.edge().role(), SemanticEdgeRoleV1::CallReturn);
    if cross_block {
        assert!(root.blocks()[1].statements().is_empty());
    }
    // The Call reads reference3 only through its destination, never as an
    // argument or synthetic source read.
    let SemanticStatementKindV1::Assign(borrow) = root.blocks()[0].statements()[1].kind() else {
        panic!("expected the actual reference-producing source assignment")
    };
    assert_eq!(
        borrow.destination().local(),
        SemanticLocalIdV1::from_index(3)
    );
    assert!(
        matches!(borrow.value().kind(), SemanticRvalueKindV1::Borrow { place, .. }
        if place == &local_place(2, scalar))
    );
}

fn assert_reference_call_lowering(lowered: &ProductionSemanticKirOwnerV1, cross_block: bool) {
    let root_id = SemanticFunctionIdV1::from_index(0);
    let root_mapping = lowered
        .correspondence()
        .lowered_functions()
        .iter()
        .find(|row| row.semantic_function() == root_id)
        .unwrap();
    let function = lowered
        .module()
        .function(root_mapping.kernel_ir_function())
        .unwrap();
    let body = function.body.as_ref().unwrap();
    let allocas = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|op| matches!(op.kind, OperationKind::Alloca { .. }))
        .collect::<Vec<_>>();
    assert_eq!(allocas.len(), 1);
    assert!(matches!(
        allocas[0].kind,
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U64),
            count: None,
            address_space: fe2o3_kernel_ir::AddressSpace::Private,
            alignment: 8,
        }
    ));
    let pointer = allocas[0].results[0].id;
    let call_block = u32::from(cross_block);
    let spans = lowered
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.correspondence_owner() == root_id
                && span.semantic_function() == root_id
                && span.semantic_block() == SemanticBlockIdV1::from_index(call_block)
        })
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 1);
    let span = spans[0];
    assert_eq!(span.operation_count(), 2);
    let block = body
        .blocks
        .iter()
        .find(|block| block.id == span.kernel_ir_block())
        .unwrap();
    let first = span.first_operation_ordinal() as usize;
    assert_eq!(first + 2, block.operations.len());
    let call = &block.operations[first];
    let helper_mapping = lowered
        .correspondence()
        .lowered_functions()
        .iter()
        .find(|row| row.semantic_function() == SemanticFunctionIdV1::from_index(1))
        .unwrap();
    assert!(
        matches!(&call.kind, OperationKind::Call { callee, arguments }
        if callee == helper_mapping.kernel_ir_function() && arguments.len() == 1)
    );
    assert_eq!(call.results.len(), 1);
    assert_eq!(call.results[0].ty, Type::Scalar(ScalarType::U64));
    let store = &block.operations[first + 1];
    assert!(
        matches!(store.kind, OperationKind::Store { pointer: actual, value,
        access: fe2o3_kernel_ir::MemoryAccess {
            address_space: fe2o3_kernel_ir::AddressSpace::Private, alignment: 8, volatile: false,
        },
    } if actual == pointer && value == call.results[0].id)
    );
    assert_eq!(
        store.memory_effects(),
        [fe2o3_kernel_ir::MemoryEffect::Write(
            fe2o3_kernel_ir::AddressSpace::Private
        )]
    );
    assert!(
        matches!(&block.terminator, Some(Terminator::Branch { target, .. })
        if *target == BlockId(call_block + 1))
    );
    let continuation = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(call_block + 1))
        .unwrap();
    assert!(continuation.operations.iter().any(
        |op| matches!(op.kind, OperationKind::Load { pointer: actual, .. }
        if actual == pointer)
    ));
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|op| matches!(op.kind, OperationKind::Call { .. }))
            .count(),
        1
    );
    // The pointer is the scalar Alloca itself, not a reloaded pointer slot.
    // Only the separate private guarded fixture claims an address Load prefix.
    assert!(!block.operations[first..].iter().any(|op| matches!(
        op.kind,
        OperationKind::Load { .. } | OperationKind::GetElementPointer { .. }
    )));
}

#[test]
fn admitted_reference_destination_survives_same_and_cross_block_calls() {
    for cross_block in [false, true] {
        let owner = reference_call_owner(cross_block, false, false);
        assert_reference_call_source(&owner, cross_block);
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        assert_reference_call_lowering(&lowered, cross_block);
    }
}

#[test]
fn admitted_reference_destination_preserves_scalar_argument_move_order() {
    let owner = reference_call_owner(true, true, false);
    assert_reference_call_source(&owner, true);
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower(owner, ProductionSemanticKirLimitsV1::default())
            .unwrap();
    lowered.verify_equivalence().unwrap();
    assert_reference_call_lowering(&lowered, true);
}

#[test]
fn admitted_dead_reference_destination_is_rejected_by_semantic_ssa() {
    let owner = reference_call_owner(false, false, true);
    assert_reference_call_source(&owner, false);
    let Err(ProductionSemanticKirErrorV1::SemanticSsa(error)) =
        ProductionSemanticKirOwnerV1::try_lower(owner, ProductionSemanticKirLimitsV1::default())
    else {
        panic!("the dead destination reference must fail at its address use")
    };
    // D2, U2, D3, K3 precede the Call's address U3 and argument U1.
    assert_eq!(
        error,
        ProductionSemanticSsaErrorV1::Planner {
            function: SemanticFunctionIdV1::from_index(0),
            error: SsaPlannerErrorV1::UndefinedAtUse {
                block: SsaBlockIdV1::new(0),
                event: 4,
                variable: SsaVariableIdV1::new(3),
            },
        }
    );
}
