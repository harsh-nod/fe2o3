// Included inside ordinary_helper_effect_only_tests_v1, after the retained-call tests.

fn tuple_result_store_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let seed = source_fixture(ResultShape::Tuple, BodyShape::Straight);
    let semantic = seed.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let result_local = root.locals().len() as u32 - 1;
    assert_eq!(root.locals()[result_local as usize].ty(), RESULT);
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(result_local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), A_U32).unwrap()],
        A_U32,
    )
    .unwrap();
    let mut blocks = root.blocks().to_vec();
    assert_eq!(blocks[1].statements().len(), 1);
    let old = &blocks[1];
    let SemanticStatementKindV1::Assign(assignment) = old.statements()[0].kind() else {
        panic!("fixture must contain the original ordinary Global Store");
    };
    blocks[1] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        vec![SemanticStatementV1::new(
            old.statements()[0].source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                assignment.destination().clone(),
                SemanticRvalueV1::new(
                    A_U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                ),
            )),
        )],
        old.terminator().clone(),
    )
    .unwrap();
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

fn assert_tuple_store_uses_first_call_result_v1(owner: &Owner, budget: &mut Budget<'_>) {
    use fe2o3_kernel_ir::{
        AddressSpace, CanonicalKirDefinitionCoordinateV1 as Definition, OperationKind, ScalarType,
    };
    let floor = budget.storage();
    let (inventory, storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(owner, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let root = &inventory.functions()[0];
    let mut call = None;
    let mut store = None;
    for operation in &inventory.operations()[root.operations.clone()] {
        budget.charge_work(2).unwrap();
        match &operation.operation.kind {
            OperationKind::Call { callee, .. } if *callee == owner.module().functions[1].id => {
                assert!(call.replace(operation).is_none());
            }
            OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global => {
                assert!(store.replace(operation).is_none());
            }
            OperationKind::Load { .. } | OperationKind::Alloca { .. } => {
                panic!("tuple SSA projection must not invent memory");
            }
            _ => {}
        }
    }
    let call = call.unwrap();
    let store = store.unwrap();
    assert_eq!(call.results.len(), 2);
    assert_eq!(
        inventory.definitions()[call.results.start].ty.as_scalar(),
        Some(ScalarType::U32)
    );
    assert_eq!(
        inventory.definitions()[call.results.start + 1]
            .ty
            .as_scalar(),
        Some(ScalarType::U64)
    );
    assert_eq!(store.operands.len(), 2);
    let actual = inventory.uses()[store.operands.start + 1].definition;
    // Fixed-size test oracle for this acyclic three-block source only. It is
    // not the proposed production value-flow algorithm or a proof constructor.
    assert!(inventory.definitions().len() <= 256);
    let mut pending = [usize::MAX; 128];
    let mut visited = [false; 256];
    let mut count = 1;
    let mut leaves = 0;
    pending[0] = actual;
    while count != 0 {
        budget.charge_work(3).unwrap();
        count -= 1;
        let definition = pending[count];
        if visited[definition] {
            continue;
        }
        visited[definition] = true;
        let row = &inventory.definitions()[definition];
        assert_eq!(row.ty.as_scalar(), Some(ScalarType::U32));
        match row.coordinate {
            Definition::Result { operation, result } => {
                assert_eq!(operation, call.coordinate);
                assert_eq!(result, 0);
                leaves += 1;
            }
            Definition::BlockArgument { .. } => {
                let mut incoming = 0;
                for edge in &inventory.edge_arguments()[root.edge_arguments.clone()] {
                    budget.charge_work(2).unwrap();
                    if edge.target_definition == definition {
                        assert!(count < pending.len());
                        pending[count] = edge.incoming_definition;
                        count += 1;
                        incoming += 1;
                    }
                }
                assert!(incoming > 0);
            }
            Definition::FunctionArgument { .. } => panic!("Store still uses a kernel argument"),
        }
    }
    assert_eq!(leaves, 1);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn tuple_result_store_is_real_typed_call_result_flow_in_n_b_and_o() {
    let source = tuple_result_store_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            assert_tuple_store_uses_first_call_result_v1(source.executable(), budget);
            assert_tuple_store_uses_first_call_result_v1(bound, budget);
            assert_tuple_store_uses_first_call_result_v1(checked.owner(), budget);
        });
        with_global_view(&source, profile, |view, budget| {
            let call = ordinary_call_coordinate_v1(view, 0, 1);
            let binding = view
                .retained_ordinary_call_bindings_v1(ROOT, ROOT, call, budget)
                .unwrap()
                .unwrap();
            assert_eq!(binding.results.len(), 2);
            assert_eq!(binding.results[0].scalar, fe2o3_kernel_ir::ScalarType::U32);
            assert_eq!(binding.results[1].scalar, fe2o3_kernel_ir::ScalarType::U64);
            assert_eq!(binding.diagnostic.callee.index(), 1);
        });
    }
}

#[test]
fn tuple_result_store_refuses_source_expression_before_checked_o_callback() {
    let source = tuple_result_store_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let mut called = false;
            let result = with_projected_checked_output_roots_v1(
                &source,
                bound,
                checked,
                profile,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |_, _, _| {
                    called = true;
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "GPU semantic scalar operand uses an unsupported place projection"
                ))
            ));
            assert!(!called);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn tuple_result_store_capture_is_source_rooted_and_names_the_actual_n_use() {
    use fe2o3_kernel_ir::{OperationKind, ScalarType};
    use fe2o3_lower_mir_kernel::{
        SemanticKirSourceStoreDefinitionV1 as Definition,
        SemanticKirSourceStoreOperandV1 as Operand,
    };
    let source = tuple_result_store_source_v1();
    source.replay_source_store_value_uses_v1().unwrap();
    let [row] = source.source_store_value_uses_v1() else {
        panic!("one ordinary Global source Store must have one captured RHS use");
    };
    assert_eq!(row.correspondence_owner(), ROOT);
    assert_eq!(row.semantic_function(), ROOT);
    assert_eq!(
        row.source_statement(),
        (SemanticBlockIdV1::from_index(1), 0)
    );
    assert_eq!(row.operand(), Operand::AssignmentRvalue);
    assert_eq!(row.source_type(), A_U32);
    assert_eq!(row.component(), 0);
    assert_eq!(row.scalar(), ScalarType::U32);
    let body = source.executable().module().functions[0]
        .body
        .as_ref()
        .unwrap();
    let (store_block, store_operation) = row.store();
    let block = body
        .blocks
        .iter()
        .find(|block| block.id == store_block)
        .unwrap();
    assert!(matches!(&block.operations[store_operation as usize].kind,
        OperationKind::Store { value, .. } if *value == row.value()));
    match row.definition() {
        Definition::BlockArgument { block, argument } => {
            let block = body
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)
                .unwrap();
            assert_eq!(block.parameters[argument as usize].id, row.value());
        }
        Definition::OperationResult {
            block,
            operation,
            result,
        } => {
            let block = body
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)
                .unwrap();
            assert_eq!(
                block.operations[operation as usize].results[result as usize].id,
                row.value()
            );
        }
        Definition::FunctionArgument(_) => panic!("the tuple result is not a source parameter"),
    }
    // These are separate obligations: capture concerns source->N only. The
    // genuine binder and optimizer still establish B/O on each target, and
    // existing backend value-expression refusal remains unchanged.
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            assert_tuple_store_uses_first_call_result_v1(source.executable(), budget);
            assert_tuple_store_uses_first_call_result_v1(bound, budget);
            assert_tuple_store_uses_first_call_result_v1(checked.owner(), budget);
        });
    }
}

include!("canonical_memory_analysis_v1_tests.rs");
