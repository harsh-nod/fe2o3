use super::*;

#[derive(Clone, Copy)]
enum IndexArgument {
    Constant,
    Copy,
    Move,
}

fn projected_call_source(
    cross_block: bool,
    argument: IndexArgument,
    initialized: bool,
    after_call: Vec<SemanticStatementV1>,
) -> ProductionSemanticMirOwnerV1 {
    let base = one_block(vec![]);
    let semantic = base.semantic();
    let original = &semantic.functions()[0];
    let source = SemanticSourceProvenanceV1::unavailable();
    let call_block = u32::from(cross_block);
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![match argument {
                IndexArgument::Constant => number(99),
                IndexArgument::Copy => SemanticOperandV1::Copy(whole(2, SCALAR)),
                IndexArgument::Move => SemanticOperandV1::Move(whole(2, SCALAR)),
            }],
            Some(SemanticCallDestinationV1::new(
                indexed(1, SCALAR),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(call_block + 1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let mut statements = Vec::new();
    if initialized {
        statements.push(initialize(1, false));
    }
    statements.push(index(7));
    let mut blocks = if cross_block {
        vec![
            block(
                190,
                statements,
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            block(191, vec![], call),
        ]
    } else {
        vec![block(190, statements, call)]
    };
    blocks.push(block(192, after_call, SemanticTerminatorKindV1::Return));
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(210)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(SCALAR))],
        direct_abi_value(SCALAR),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(211)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(212)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(213)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(214)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(215)),
        source,
        helper_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(216)),
                SCALAR,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(217)),
                SCALAR,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            218,
            vec![assign(
                whole(0, SCALAR),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(1, SCALAR))),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn assert_saved_call_address(owner: &ProductionSemanticKirOwnerV1, call_block: usize) {
    owner.verify_equivalence().unwrap();
    verify_module(owner.module()).unwrap();
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, false);
    let operations = &body.blocks[call_block].operations;
    let (call_index, call) = operations
        .iter()
        .enumerate()
        .find(|(_, operation)| matches!(operation.kind, OperationKind::Call { .. }))
        .expect("one actual scalar helper call");
    assert_eq!(call.results.len(), 1);
    let result = call.results[0].id;
    let stores = operations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, operation)| match operation.kind {
            OperationKind::Store { pointer, value, .. } if value == result => {
                Some((ordinal, pointer))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(store_index, pointer)] = stores.as_slice() else {
        panic!("expected exactly one store of the actual call result")
    };
    let (address_index, _) = operations
        .iter()
        .enumerate()
        .find(|(_, operation)| {
            operation.results.first().is_some_and(|value| value.id == *pointer)
                && matches!(operation.kind, OperationKind::GetElementPointer { base, .. } if base == slot)
        })
        .expect("store uses the actual retained-array element address");
    assert!(
        address_index < call_index,
        "destination address must precede the call"
    );
    assert!(
        call_index < *store_index,
        "result store must follow the call"
    );
}

#[test]
fn projected_call_terminator_emits_one_saved_address_and_one_result_store() {
    for cross_block in [false, true] {
        for (argument, expected_operations) in [
            (IndexArgument::Constant, 5),
            (IndexArgument::Copy, 4),
            (IndexArgument::Move, 4),
        ] {
            let owner = lower(projected_call_source(cross_block, argument, true, vec![])).unwrap();
            let call_block = u32::from(cross_block);
            assert_saved_call_address(&owner, call_block as usize);
            let mut spans = owner
                .correspondence()
                .terminator_operation_spans()
                .iter()
                .filter(|span| {
                    span.correspondence_owner().index() == 0
                        && span.semantic_function().index() == 0
                        && span.semantic_block().index() == call_block
                });
            let span = spans.next().expect("actual source call terminator span");
            assert!(spans.next().is_none());
            assert_eq!(span.operation_count(), expected_operations);
            let body = owner.module().functions[0].body.as_ref().unwrap();
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == span.kernel_ir_block())
                .unwrap();
            let start = span.first_operation_ordinal() as usize;
            let operations = &block.operations[start..start + expected_operations as usize];
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| {
                        matches!(operation.kind, OperationKind::GetElementPointer { .. })
                    })
                    .count(),
                1
            );
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| { matches!(operation.kind, OperationKind::Call { .. }) })
                    .count(),
                1
            );
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| { matches!(operation.kind, OperationKind::Store { .. }) })
                    .count(),
                1
            );
            assert!(
                !operations
                    .iter()
                    .any(|operation| { matches!(operation.kind, OperationKind::Load { .. }) })
            );
        }
    }
}

#[test]
fn projected_call_index_is_restored_from_predecessor_without_an_incidental_use() {
    let owner = lower(projected_call_source(
        true,
        IndexArgument::Constant,
        true,
        vec![read(fixed(1, 7, false, SCALAR))],
    ))
    .unwrap();
    assert_saved_call_address(&owner, 1);
}

#[test]
fn projected_call_same_block_and_argument_use_controls_compile() {
    for (cross_block, argument) in [
        (false, IndexArgument::Constant),
        (true, IndexArgument::Copy),
    ] {
        let owner = lower(projected_call_source(cross_block, argument, true, vec![])).unwrap();
        owner.verify_equivalence().unwrap();
        verify_module(owner.module()).unwrap();
    }
}

#[test]
fn projected_call_snapshots_index_before_argument_move_without_restoring_it() {
    for cross_block in [false, true] {
        let owner = lower(projected_call_source(
            cross_block,
            IndexArgument::Move,
            true,
            vec![read(fixed(1, 7, false, SCALAR))],
        ))
        .unwrap();
        assert_saved_call_address(&owner, usize::from(cross_block));
    }
}

#[test]
fn projected_call_partial_write_does_not_initialize_the_whole_array() {
    let owner = lower(projected_call_source(
        false,
        IndexArgument::Constant,
        false,
        vec![],
    ))
    .unwrap();
    assert_saved_call_address(&owner, 0);
    let error = lower(projected_call_source(
        false,
        IndexArgument::Constant,
        false,
        vec![read(fixed(1, 7, false, SCALAR))],
    ))
    .unwrap_err();
    assert_partial_read(error);
}
