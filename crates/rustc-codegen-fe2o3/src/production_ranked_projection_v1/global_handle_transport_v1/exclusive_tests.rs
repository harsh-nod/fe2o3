use super::super::exclusive;
use super::*;

fn mixed() -> SemanticAssignmentV1 {
    construct(
        12,
        MIXED,
        None,
        vec![
            SemanticOperandV1::Move(place(1, REF)),
            SemanticOperandV1::Move(place(9, MUT_REF)),
        ],
    )
}

fn extraction(field: u32, moving: bool) -> SemanticAssignmentV1 {
    let (local, ty) = if field == 0 { (5, REF) } else { (9, MUT_REF) };
    let place = use_path(12, &[(SemanticProjectionKindV1::Field(field), ty)]);
    assign(
        local,
        ty,
        SemanticRvalueKindV1::Use(if moving {
            SemanticOperandV1::Move(place)
        } else {
            SemanticOperandV1::Copy(place)
        }),
    )
}

fn function_with_blocks(
    original: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
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
}

fn block(
    id: u8,
    statements: Vec<SemanticStatementKindV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([id + 1; 32]),
        source,
        statements
            .into_iter()
            .map(|kind| SemanticStatementV1::new(source, kind))
            .collect(),
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    ))
}

fn run(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &mut ProjectedCapabilityStateV1,
    block: usize,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    transfer_capability_statements_metered_v1(
        types, function, block, state, &dominance, None, None, None, work,
    )
}

fn build(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &mut ProjectedCapabilityStateV1,
) {
    let assignment = mixed();
    let value = exclusive::assignment(types, function, state, &assignment, &mut 0)
        .unwrap()
        .unwrap();
    assert!(matches!(
        value,
        ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_)
    ));
    consume_capability_rvalue_operands_v1(types, function, assignment.value().kind(), state);
    state.insert(12, value);
}

#[test]
fn exclusive_global_capture_production_transfer_retains_shared_leaf_and_consumes_mutable_leaf() {
    let (types, original, mut state) = fixture();
    let input = state[&1].clone();
    let output = state[&9].clone();
    let function = function_with_blocks(
        &original,
        vec![
            block(0, vec![SemanticStatementKindV1::Assign(mixed())], goto(1)),
            block(
                1,
                vec![SemanticStatementKindV1::Assign(extraction(0, false))],
                goto(2),
            ),
            block(
                2,
                vec![SemanticStatementKindV1::Assign(extraction(1, false))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let mut work = 0;
    run(&types, &function, &mut state, 0, &mut work).unwrap();
    assert_eq!(state[&1], ProjectedCapabilityValueV1::Invalid);
    assert_eq!(state[&9], ProjectedCapabilityValueV1::Invalid);
    let carrier = state[&12].clone();
    assert!(!projected_value_is_shared_read_v1(&carrier));
    run(&types, &function, &mut state, 1, &mut work).unwrap();
    assert_eq!(state[&5], input);
    assert_eq!(state[&12], carrier);
    run(&types, &function, &mut state, 2, &mut work).unwrap();
    assert_eq!(state[&9], output);
    assert_eq!(state[&12], ProjectedCapabilityValueV1::Invalid);
    assert_eq!(state[&5], input);
    run(&types, &function, &mut state, 2, &mut work).unwrap();
    assert!(
        !state.contains_key(&9),
        "second extraction cannot recover exclusive authority"
    );
}

#[test]
fn exclusive_global_capture_same_bytes_other_owner_and_type_arena_reject() {
    let (types, function, mut state) = fixture();
    build(&types, &function, &mut state);
    let foreign = function.clone();
    let foreign_types = types.clone();
    for (types, owner) in [(&types[..], &foreign), (&foreign_types[..], &function)] {
        assert_eq!(
            exclusive::assignment(types, owner, &state, &extraction(1, true), &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Invalid)
        );
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
            extraction(0, false).value().kind().clone()
        else {
            panic!()
        };
        assert!(!exclusive::preserves_shared_leaf_copy(
            types, owner, &state, &place
        ));
    }
}

#[test]
fn exclusive_global_capture_full_provenance_and_allocation_aliases_reject() {
    let (types, function, baseline) = fixture();
    for mutation in 0..6 {
        let mut state = baseline.clone();
        let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view)) =
            state.get_mut(&9).unwrap()
        else {
            panic!()
        };
        match mutation {
            0 => view.allocation.allocation_origin = 1,
            1 => view.borrow = Some(SemanticBorrowKindV1::Shared),
            2 => view.contract = SemanticCapabilityMemoryContractV1::global_read_only(),
            3 => view.allocation.writable = false,
            4 => {
                view.provenance = SemanticKernelCapabilityProvenanceV1::new(
                    SemanticFunctionIdV1::from_index(1),
                    view.provenance.kernel_binding(),
                    view.provenance.frontend_unit(),
                    view.provenance.kernel_marker(),
                    view.provenance.target_brand(),
                    view.provenance.launch_brand(),
                    view.provenance.issuance(),
                )
                .unwrap()
            }
            5 => {
                view.provenance = SemanticKernelCapabilityProvenanceV1::new(
                    view.provenance.root(),
                    view.provenance.kernel_binding(),
                    view.provenance.frontend_unit(),
                    view.provenance.kernel_marker(),
                    view.provenance.target_brand(),
                    view.provenance.launch_brand(),
                    SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([91; 32]),
                )
                .unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &mixed(), &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Invalid),
            "mutation {mutation}"
        );
    }
}

#[test]
fn exclusive_global_capture_wrong_fields_and_widths_are_not_selected_by_type() {
    let (types, function, mut state) = fixture();
    let swapped = construct(12, MIXED, None, vec![copy(9, MUT_REF), copy(1, REF)]);
    assert_eq!(
        exclusive::assignment(&types, &function, &state, &swapped, &mut 0).unwrap(),
        Some(ProjectedCapabilityValueV1::Invalid)
    );
    build(&types, &function, &mut state);
    for kind in [
        SemanticProjectionKindV1::Field(2),
        SemanticProjectionKindV1::Field(0),
        SemanticProjectionKindV1::Dereference,
    ] {
        let bad = assign(
            9,
            MUT_REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(12, &[(kind, MUT_REF)]))),
        );
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &bad, &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Invalid)
        );
    }
}

#[test]
fn exclusive_global_capture_lifetime_deinitialize_and_projected_write_kill() {
    for mutation in 0..5 {
        let (types, original, mut state) = fixture();
        let kill = match mutation {
            0 => SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(12)),
            1 => SemanticStatementKindV1::Deinitialize(place(12, MIXED)),
            2 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                use_path(12, &[(SemanticProjectionKindV1::Field(1), MUT_REF)]),
                SemanticRvalueV1::new(MUT_REF, SemanticRvalueKindV1::Use(copy(9, MUT_REF))),
            )),
            3 => SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(12)),
            4 => SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                use_path(12, &[(SemanticProjectionKindV1::Field(1), MUT_REF)]),
                copy(9, MUT_REF),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            _ => unreachable!(),
        };
        let function = function_with_blocks(
            &original,
            vec![block(
                0,
                vec![kill, SemanticStatementKindV1::Assign(extraction(1, false))],
                SemanticTerminatorKindV1::Return,
            )],
        );
        build(&types, &function, &mut state);
        run(&types, &function, &mut state, 0, &mut 0).unwrap();
        assert!(!state.contains_key(&9), "kill {mutation}");
    }
}

#[test]
fn exclusive_global_capture_join_and_mutating_backedge_do_not_reintroduce_custody() {
    let (types, original, mut state) = fixture();
    let function = function_with_blocks(
        &original,
        vec![
            block(0, vec![], goto(1)),
            block(
                1,
                vec![SemanticStatementKindV1::Assign(extraction(1, false))],
                goto(0),
            ),
        ],
    );
    build(&types, &function, &mut state);
    let mut agreeing = state.clone();
    let carrier = state[&12].clone();
    assert!(
        merge_capability_states_v1(&mut agreeing, &state).unwrap(),
        "meet compacts consumed Invalid entries"
    );
    assert_eq!(agreeing[&12], carrier);
    assert!(!merge_capability_states_v1(&mut agreeing, &state).unwrap());
    let mut backedge = state.clone();
    run(&types, &function, &mut backedge, 1, &mut 0).unwrap();
    assert!(merge_capability_states_v1(&mut state, &backedge).unwrap());
    assert!(!state.contains_key(&12));
    assert_eq!(
        exclusive::assignment(&types, &function, &state, &extraction(1, false), &mut 0).unwrap(),
        None
    );
    let mut missing = agreeing.clone();
    missing.remove(&12);
    merge_capability_states_v1(&mut agreeing, &missing).unwrap();
    assert!(!agreeing.contains_key(&12));
}

#[test]
fn exclusive_global_capture_existing_work_and_storage_bounds_remain_fatal() {
    let (types, function, mut state) = fixture();
    let mut work = 0;
    let value = exclusive::assignment(&types, &function, &state, &mixed(), &mut work)
        .unwrap()
        .unwrap();
    let spent = work;
    for missing in [0, 1, spent] {
        let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - spent + missing;
        let result = exclusive::assignment(&types, &function, &state, &mixed(), &mut work);
        if missing == 0 {
            assert_eq!(result.unwrap(), Some(value.clone()));
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "capability dataflow exceeds the charged projection limit"
                ))
            ));
        }
        assert!(work >= MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    }
    state.insert(12, value);
    assert_eq!(
        capability_state_storage_units_v1(&state).unwrap(),
        state.len() + 3
    );
    let operand = SemanticOperandV1::Copy(place(12, MIXED));
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(matches!(
        exclusive::charge_statement(&state, &SemanticStatementKindV1::Assume(operand), &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow exceeds the charged projection limit"
        ))
    ));
    assert!(work > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    for local in 0..MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 {
        state
            .entry(local)
            .or_insert(ProjectedCapabilityValueV1::Invalid);
    }
    assert!(matches!(
        exclusive::assignment(&types, &function, &state, &mixed(), &mut 0),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability states exceed the charged storage limit"
        ))
    ));
}

fn extend_function(
    original: &SemanticFunctionDeclV1,
    local_types: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut locals = original.locals().to_vec();
    for ty in local_types {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([locals.len() as u8 + 1; 32]),
            *ty,
            SemanticLocalRoleV1::Temporary,
            original.source(),
        ));
    }
    SemanticFunctionDeclV1::new(
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
}

#[test]
fn exclusive_global_capture_eight_field_steps_succeed_ninth_loses_all_custody() {
    let (mut types, original, mut state) = fixture();
    let mut nested = vec![MUT_REF];
    for _ in 0..9 {
        nested.push(add_type(
            &mut types,
            aggregate(vec![*nested.last().unwrap()]),
        ));
    }
    // Complete arenas/owner before constructing any owner-tied capture.
    let function = extend_function(&original, &nested[1..], original.blocks().to_vec());
    let original_value = state[&9].clone();
    let mut source = 9;
    for depth in 1..=9 {
        let destination = (original.locals().len() + depth - 1) as u32;
        let assignment = construct(
            destination,
            nested[depth],
            None,
            vec![SemanticOperandV1::Move(place(source, nested[depth - 1]))],
        );
        let result = exclusive::assignment(&types, &function, &state, &assignment, &mut 0)
            .unwrap()
            .unwrap();
        if depth == 9 {
            assert_eq!(result, ProjectedCapabilityValueV1::Invalid);
            break;
        }
        assert!(matches!(
            result,
            ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_)
        ));
        consume_capability_rvalue_operands_v1(
            &types,
            &function,
            assignment.value().kind(),
            &mut state,
        );
        state.insert(destination as usize, result);
        let path = nested[..depth]
            .iter()
            .rev()
            .map(|ty| (SemanticProjectionKindV1::Field(0), *ty))
            .collect::<Vec<_>>();
        let extraction = assign(
            9,
            MUT_REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(destination, &path))),
        );
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &extraction, &mut 0).unwrap(),
            Some(original_value.clone())
        );
        source = destination;
    }
}

#[test]
fn exclusive_global_capture_four_handles_and_sixteen_fields_are_closed_bounds() {
    for field_count in [16, 17] {
        let (mut types, original, state) = fixture();
        let mut fields = vec![REF, REF, REF, MUT_REF];
        fields.resize(field_count, SCALAR);
        let ty = add_type(&mut types, aggregate(fields));
        let destination = original.locals().len() as u32;
        let function = extend_function(&original, &[ty], original.blocks().to_vec());
        let mut operands = vec![copy(1, REF), copy(2, REF), copy(15, REF), copy(9, MUT_REF)];
        operands.resize(
            field_count,
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SCALAR,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
            )),
        );
        let result = exclusive::assignment(
            &types,
            &function,
            &state,
            &construct(destination, ty, None, operands),
            &mut 0,
        )
        .unwrap();
        if field_count == 16 {
            let Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(value)) = result else {
                panic!()
            };
            assert_eq!(value.storage_units(), 5);
        } else {
            assert_eq!(
                result, None,
                "over-bound aggregate is not scanned or authorized"
            );
        }
    }
}

#[test]
fn exclusive_global_capture_wrong_reference_kind_metadata_width_and_pointee_reject() {
    for mutation in 0..6 {
        let (mut types, function, state) = fixture();
        let declaration = &types[MUT_REF.index() as usize];
        let pointer = SemanticPointerTypeV1::new_with_kind(
            if mutation == 4 { SCALAR } else { GLOBAL },
            if mutation == 0 {
                SemanticPointerKindV1::Raw
            } else {
                SemanticPointerKindV1::Reference
            },
            if mutation == 1 {
                SemanticMutabilityV1::Immutable
            } else {
                SemanticMutabilityV1::Mutable
            },
            if mutation == 2 { 1 } else { 0 },
            if mutation == 3 { 32 } else { 64 },
            if mutation == 5 {
                SemanticPointerMetadataV1::SliceLength
            } else {
                SemanticPointerMetadataV1::None
            },
        )
        .unwrap();
        types[MUT_REF.index() as usize] = SemanticTypeDeclV1::new(
            declaration.identity(),
            declaration.layout_identity(),
            declaration.layout().clone(),
            SemanticTypeShapeV1::Pointer(pointer),
        );
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &mixed(), &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Invalid),
            "pointer mutation {mutation}"
        );
    }
}

#[test]
fn exclusive_global_capture_terminal_operand_consumption_is_metered() {
    let (types, function, mut state) = fixture();
    build(&types, &function, &mut state);
    let argument = copy(12, MIXED);
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![argument.clone()],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let original = state.clone();
    let mut work = 0;
    exclusive::charge_terminator(&state, &call, &mut work).unwrap();
    assert_eq!(work, 36);
    assert_eq!(state, original);
    work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(matches!(
        exclusive::charge_terminator(&state, &call, &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow exceeds the charged projection limit"
        ))
    ));
    assert_eq!(state, original);
    // Existing normal/unknown-call consumption, not a summary authorizing an opaque call.
    consume_capability_operand_v1(&types, &function, &mut state, &argument);
    assert_eq!(state[&12], ProjectedCapabilityValueV1::Invalid);
}

#[test]
fn exclusive_global_capture_preflight_accounts_sparse_capacity_and_live_storage() {
    let (types, function, state) = fixture();
    let mut expanded = state.clone();
    expanded.reserve(2048);
    let mut small_work = 0;
    let mut large_work = 0;
    assert_eq!(
        exclusive::assignment(&types, &function, &state, &mixed(), &mut small_work).unwrap(),
        exclusive::assignment(&types, &function, &expanded, &mixed(), &mut large_work).unwrap()
    );
    assert_eq!(
        large_work - small_work,
        expanded.capacity() - state.capacity()
    );
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - large_work + 1;
    let before = expanded.clone();
    assert!(matches!(
        exclusive::assignment(&types, &function, &expanded, &mixed(), &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow exceeds the charged projection limit"
        ))
    ));
    assert_eq!(expanded, before);
    assert!(work > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
    assert!(
        std::mem::size_of::<exclusive::Exclusive>()
            <= std::mem::size_of::<ProjectedCapabilityValueV1>()
    );
    assert!(std::mem::size_of::<Capture>() <= std::mem::size_of::<ProjectedCapabilityValueV1>());
    eprintln!(
        "exclusive-unit work={small_work} sparse_work={large_work} state_len={} state_capacity={} sparse_capacity={} fact_bytes={} path_bytes={} carrier_bytes={}",
        state.len(),
        state.capacity(),
        expanded.capacity(),
        std::mem::size_of::<ProjectedCapabilityValueV1>(),
        std::mem::size_of::<Capture>(),
        std::mem::size_of::<exclusive::Exclusive>()
    );
}

fn add_type(types: &mut Vec<SemanticTypeDeclV1>, shape: SemanticTypeShapeV1) -> SemanticTypeIdV1 {
    let ty = SemanticTypeIdV1::from_index(types.len() as u32);
    let layout = match &shape {
        SemanticTypeShapeV1::Pointer(_) => SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Aggregate(fields) => {
            let mut size = 0;
            let offsets = fields
                .fields()
                .iter()
                .map(|field| {
                    let offset = size;
                    size += types[field.index() as usize].layout().size_bytes().unwrap();
                    offset
                })
                .collect();
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                8,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        }
        _ => panic!("fixture only adds exact field aggregates and thin pointers"),
    };
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([types.len() as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([types.len() as u8 + 1; 32]),
        layout,
        shape,
    ));
    ty
}

#[test]
fn exclusive_global_capture_whole_copy_and_move_transfer_custody_once() {
    for moving in [false, true] {
        let (types, original, mut state) = fixture();
        let destination = original.locals().len() as u32;
        let operand = if moving {
            SemanticOperandV1::Move(place(12, MIXED))
        } else {
            copy(12, MIXED)
        };
        let function = extend_function(
            &original,
            &[MIXED],
            vec![block(
                0,
                vec![SemanticStatementKindV1::Assign(assign(
                    destination,
                    MIXED,
                    SemanticRvalueKindV1::Use(operand),
                ))],
                SemanticTerminatorKindV1::Return,
            )],
        );
        build(&types, &function, &mut state);
        let expected = state[&12].clone();
        run(&types, &function, &mut state, 0, &mut 0).unwrap();
        assert_eq!(state[&(destination as usize)], expected);
        assert_eq!(state[&12], ProjectedCapabilityValueV1::Invalid);
        let extracted = assign(
            9,
            MUT_REF,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                destination,
                &[(SemanticProjectionKindV1::Field(1), MUT_REF)],
            ))),
        );
        assert!(matches!(
            exclusive::assignment(&types, &function, &state, &extracted, &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Known(
                ProjectedCapabilityOriginV1::GlobalView(ProjectedGlobalViewV1 {
                    borrow: Some(SemanticBorrowKindV1::Mutable),
                    ..
                })
            ))
        ));
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &extraction(1, false), &mut 0)
                .unwrap(),
            None
        );
    }
}

#[test]
fn exclusive_global_capture_surrounding_borrow_or_address_escape_revokes_carrier() {
    for mode in 0..4 {
        let (mut types, original, mut state) = fixture();
        let mutable = mode % 2 != 0;
        let pointer_ty = add_type(
            &mut types,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    MIXED,
                    if mode < 2 {
                        SemanticPointerKindV1::Reference
                    } else {
                        SemanticPointerKindV1::Raw
                    },
                    if mutable {
                        SemanticMutabilityV1::Mutable
                    } else {
                        SemanticMutabilityV1::Immutable
                    },
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        let value = if mode < 2 {
            SemanticRvalueKindV1::Borrow {
                kind: if mutable {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: place(12, MIXED),
            }
        } else {
            SemanticRvalueKindV1::AddressOf {
                mutability: if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                place: place(12, MIXED),
            }
        };
        let destination = original.locals().len() as u32;
        let function = extend_function(
            &original,
            &[pointer_ty],
            vec![block(
                0,
                vec![
                    SemanticStatementKindV1::Assign(assign(destination, pointer_ty, value)),
                    SemanticStatementKindV1::Assign(extraction(1, false)),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
        build(&types, &function, &mut state);
        run(&types, &function, &mut state, 0, &mut 0).unwrap();
        assert_eq!(
            state[&12],
            ProjectedCapabilityValueV1::Invalid,
            "escape {mode}"
        );
        assert!(
            !state.contains_key(&(destination as usize)),
            "no borrowed carrier {mode}"
        );
        assert!(!state.contains_key(&9), "no extracted authority {mode}");
    }
}

#[test]
fn exclusive_global_capture_duplicate_mutable_and_handle_limit_reject() {
    for fields in [vec![MUT_REF, MUT_REF], vec![REF, REF, REF, REF, MUT_REF]] {
        let (mut types, original, state) = fixture();
        let ty = add_type(&mut types, aggregate(fields.clone()));
        let destination = original.locals().len() as u32;
        let function = extend_function(&original, &[ty], original.blocks().to_vec());
        let operands = if fields.len() == 2 {
            vec![copy(9, MUT_REF), copy(9, MUT_REF)]
        } else {
            vec![
                copy(1, REF),
                copy(2, REF),
                copy(15, REF),
                copy(16, REF),
                copy(9, MUT_REF),
            ]
        };
        let assignment = construct(destination, ty, None, operands);
        assert_eq!(
            exclusive::assignment(&types, &function, &state, &assignment, &mut 0).unwrap(),
            Some(ProjectedCapabilityValueV1::Invalid)
        );
    }
}
