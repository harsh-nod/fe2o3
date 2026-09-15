use super::*;

const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const MUT_ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const SHARED_ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

type Evidence = InertCanonicalSemanticCallExpansionEvidenceV1;

fn closure_types() -> Vec<SemanticTypeDeclV1> {
    let mut result = types();
    result.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(4)),
        SemanticLayoutIdentityV1::from_sha256(identity(4)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    result.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(5)),
        SemanticLayoutIdentityV1::from_sha256(identity(5)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![PTR, U32]).unwrap()),
    ));
    for (tag, mutability) in [
        (6, SemanticMutabilityV1::Mutable),
        (7, SemanticMutabilityV1::Immutable),
    ] {
        result.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(identity(tag)),
                SemanticLayoutIdentityV1::from_sha256(identity(tag)),
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
                            if mutability == SemanticMutabilityV1::Mutable {
                                SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                            } else {
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                            },
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

fn closure_abi(
    tag: u32,
    receiver: SemanticTypeIdV1,
    tuple: SemanticTypeIdV1,
    fields: Vec<SemanticAbiArgumentV1>,
) -> std::result::Result<SemanticFunctionAbiV1, SemanticMirErrorV1> {
    let receiver_value = if receiver == ENV {
        SemanticAbiValueV1::new(ENV, SemanticAbiPassModeV1::Ignore)
    } else {
        SemanticAbiValueV1::new(
            receiver,
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
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    };
    let mut arguments = vec![SemanticAbiArgumentV1::source(receiver_value)];
    arguments.extend(fields);
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![receiver, tuple],
        U32,
        arguments,
        direct(U32),
    )?
    .with_source_argument_ownership(vec![
        if receiver == ENV {
            SemanticSourceArgumentOwnershipV1::ByValue
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        },
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
}

fn fields() -> Vec<SemanticAbiArgumentV1> {
    vec![
        SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(PTR)),
        SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(U32)),
    ]
}

fn locals(entries: &[(SemanticTypeIdV1, SemanticLocalRoleV1)]) -> Vec<SemanticLocalDeclV1> {
    entries
        .iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(index as u32 + 1)),
                *ty,
                *role,
                provenance(),
            )
        })
        .collect()
}

fn rebuild(
    original: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
    role: SemanticFunctionRoleV1,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        original.identity(),
        role,
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
}

fn projected(
    index: u32,
    projection: SemanticProjectionKindV1,
    ty: SemanticTypeIdV1,
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(index),
        vec![SemanticProjectionV1::new(projection, ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn assign(index: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(index, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

fn closure_call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            function_id(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination, U32),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn functions() -> Vec<SemanticFunctionDeclV1> {
    use SemanticLocalRoleV1::{Argument, Return, Temporary};
    use SemanticOperandV1::{Copy, Move};
    use SemanticRvalueKindV1::{Borrow, Use};

    let root = function(
        0,
        true,
        false,
        vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
    );
    let mut root_locals = root.locals().to_vec();
    for (index, ty) in [(5, ENV), (6, TUPLE)] {
        root_locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(index + 1)),
            ty,
            Temporary,
            provenance(),
        ));
    }
    let root = rebuild(
        &root,
        root.abi().clone(),
        root_locals,
        vec![
            block(
                0,
                vec![
                    assign(
                        5,
                        ENV,
                        Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ENV,
                            SemanticConstantValueV1::ZeroSized,
                        ))),
                    ),
                    assign(
                        6,
                        TUPLE,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                vec![Copy(place(1, PTR)), Move(place(2, U32))],
                            )
                            .unwrap(),
                        ),
                    ),
                ],
                closure_call(1, vec![Move(place(5, ENV)), Move(place(6, TUPLE))], 4),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
        root.role(),
    );

    // Local IDs are canonical identity order, deliberately not argument order.
    let adapter = function(
        1,
        false,
        false,
        vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
    );
    let adapter = rebuild(
        &adapter,
        closure_abi(2, ENV, TUPLE, fields()).unwrap(),
        locals(&[
            (U32, Return),
            (SHARED_ENV, Temporary),
            (TUPLE, Argument(1)),
            (MUT_ENV, Temporary),
            (ENV, Argument(0)),
        ]),
        vec![
            block(
                0,
                vec![
                    assign(
                        3,
                        MUT_ENV,
                        Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: place(4, ENV),
                        },
                    ),
                    assign(
                        1,
                        SHARED_ENV,
                        Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: projected(3, SemanticProjectionKindV1::Dereference, ENV),
                        },
                    ),
                ],
                closure_call(
                    2,
                    vec![Move(place(1, SHARED_ENV)), Move(place(2, TUPLE))],
                    0,
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
        adapter.role(),
    );

    let closure = function(
        2,
        false,
        false,
        vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
    );
    let closure = rebuild(
        &closure,
        closure_abi(3, SHARED_ENV, TUPLE, fields()).unwrap(),
        locals(&[
            (U32, Return),
            (PTR, Temporary),
            (SHARED_ENV, Argument(0)),
            (U32, Temporary),
            (TUPLE, Argument(1)),
        ]),
        vec![block(
            0,
            vec![
                assign(
                    1,
                    PTR,
                    Use(Move(projected(4, SemanticProjectionKindV1::Field(0), PTR))),
                ),
                assign(
                    3,
                    U32,
                    Use(Move(projected(4, SemanticProjectionKindV1::Field(1), U32))),
                ),
                SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        memory(),
                        Copy(place(3, U32)),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
                assign(0, U32, Use(Move(place(3, U32)))),
            ],
            SemanticTerminatorKindV1::Return,
        )],
        closure.role(),
    );
    vec![root, adapter, closure]
}

fn request(functions: Vec<SemanticFunctionDeclV1>) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(250))),
        closure_types(),
        vec![],
        vec![],
        vec![],
        functions,
        vec![function_id(0)],
    )
    .unwrap()
}

fn source() -> AdmittedInertSemanticMirV1 {
    request(functions())
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
}

fn expand(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    SemanticCallExpansionV1::try_new(source, SemanticCallExpansionLimitsV1::default()).unwrap()
}

fn replace_call(function: &mut SemanticFunctionDeclV1, call: SemanticDirectCallV1) {
    let mut blocks = function.blocks().to_vec();
    blocks[0] = block(
        0,
        blocks[0].statements().to_vec(),
        SemanticTerminatorKindV1::Call(call),
    );
    *function = rebuild(
        function,
        function.abi().clone(),
        function.locals().to_vec(),
        blocks,
        function.role(),
    );
}

fn first_call(function: &SemanticFunctionDeclV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind() else {
        panic!()
    };
    call
}

#[test]
fn nested_closure_source_tuple_transfers_preserve_moves_borrows_and_origins() {
    let source = source();
    let bytes = source.canonical_encoding().to_vec();
    let decoded =
        AdmittedInertSemanticMirV1::decode_canonical(&bytes, SemanticMirLimitsV1::default())
            .unwrap();
    let expansion = expand(&source);
    assert_eq!(expansion, expand(&decoded));
    expansion.verify_replay(&decoded).unwrap();
    let root = &expansion.roots()[0];
    assert_eq!(root.instances().len(), 3);
    assert_eq!(
        root.instances()[2].parent(),
        Some(SemanticCallInstanceIdV1(1))
    );
    assert_eq!(root.instances()[2].depth(), 2);
    assert_eq!(root.body().abi(), source.functions()[0].abi());

    for child in 1..=2 {
        let instance = &root.instances()[child];
        let callee = &source.functions()[child];
        assert_eq!(instance.function_identity(), callee.identity());
        assert_eq!(callee.abi().source_input_types().len(), 2);
        assert_eq!(callee.abi().adjusted_arguments().len(), 3);
        assert_eq!(callee.abi().source_argument_ownership().len(), 2);
        let parent = &root.instances()[child - 1];
        let call = first_call(&source.functions()[child - 1]);
        let entry = parent.block_start() as usize;
        let transfers = root.body().blocks()[entry]
            .statements()
            .iter()
            .zip(root.block_origins()[entry].statements())
            .filter_map(|(statement, origin)| match origin {
                SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument } => {
                    assert_eq!(callee.index(), child as u32);
                    Some((statement, *argument))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            transfers.len(),
            2,
            "FnAbi fields must not become source transfers"
        );
        for (statement, argument) in transfers {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!()
            };
            let original_local = callee
                .locals()
                .iter()
                .position(|local| local.role() == SemanticLocalRoleV1::Argument(argument))
                .unwrap();
            assert_eq!(
                assignment.destination(),
                &place(
                    instance.local_start() + original_local as u32,
                    callee.abi().source_input_types()[argument as usize]
                )
            );
            let expected = remap::operand(
                &call.arguments()[argument as usize],
                parent,
                &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap(),
            )
            .unwrap();
            assert_eq!(
                assignment.value().kind(),
                &SemanticRvalueKindV1::Use(expected)
            );
            assert_eq!(
                statement.source(),
                source.functions()[child - 1].blocks()[0]
                    .terminator()
                    .source()
            );
        }
    }

    // Normalized tuple field moves and the explicit reborrow remain source nodes.
    for (instance_index, statement_count) in [(1, 2), (2, 4)] {
        let instance = &root.instances()[instance_index];
        let entry = instance.block_start() as usize;
        for index in 0..statement_count {
            let expected = remap::statement(
                &source.functions()[instance_index].blocks()[0].statements()[index],
                instance,
                &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap(),
            )
            .unwrap();
            assert_eq!(root.body().blocks()[entry].statements()[index], expected);
            assert_eq!(
                root.block_origins()[entry].statements()[index],
                SemanticExpandedStatementOriginV1::Source {
                    statement: index as u32
                }
            );
        }
    }
    let mut returns = 0;
    for (block, origin) in root.body().blocks().iter().zip(root.block_origins()) {
        assert!(!matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Call(_)
        ));
        for (statement, origin) in block.statements().iter().zip(origin.statements()) {
            if let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = origin {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    panic!()
                };
                assert_eq!(
                    assignment.value().kind(),
                    &SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(
                        root.instances()[callee.index() as usize].local_start(),
                        U32
                    )))
                );
                returns += 1;
            }
        }
    }
    assert_eq!(returns, 2);
    assert_eq!(source.canonical_encoding(), bytes);
    assert!(!expansion.grants_proof_or_artifact_authority());
}

#[test]
fn rust_call_rejects_wrong_source_local_roles_and_types_before_expansion() {
    for (role, ty) in [
        (SemanticLocalRoleV1::Temporary, TUPLE),
        (SemanticLocalRoleV1::Argument(0), TUPLE),
        (SemanticLocalRoleV1::Argument(2), TUPLE),
        (SemanticLocalRoleV1::Argument(1), U32),
    ] {
        let mut functions = functions();
        let closure = &functions[2];
        let mut locals = closure.locals().to_vec();
        locals[4] = SemanticLocalDeclV1::new(locals[4].identity(), ty, role, provenance());
        functions[2] = rebuild(
            closure,
            closure.abi().clone(),
            locals,
            closure.blocks().to_vec(),
            closure.role(),
        );
        assert!(
            matches!(request(functions).admit(SemanticMirLimitsV1::default()), Err(SemanticMirErrorV1::InvalidLocalRoles { function }) if function == function_id(2))
        );
    }
}

#[test]
fn rust_call_rejects_wrong_abi_field_roles_types_counts_and_non_tuple_tail() {
    for fields in [
        vec![
            SemanticAbiArgumentV1::source(direct(PTR)),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(U32)),
        ],
        vec![
            SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(PTR)),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(U32)),
        ],
        vec![
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(PTR)),
            SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(U32)),
            SemanticAbiArgumentV1::hidden(
                SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                direct(PTR),
            ),
        ],
    ] {
        assert_eq!(
            closure_abi(3, SHARED_ENV, TUPLE, fields),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
    for (tuple, fields) in [
        (
            TUPLE,
            vec![SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(PTR))],
        ),
        (
            TUPLE,
            vec![
                SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(U32)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(U32)),
            ],
        ),
        (U32, fields()),
    ] {
        let mut functions = functions();
        let closure = &functions[2];
        functions[2] = rebuild(
            closure,
            closure_abi(3, SHARED_ENV, tuple, fields).unwrap(),
            closure.locals().to_vec(),
            closure.blocks().to_vec(),
            closure.role(),
        );
        if tuple == U32 {
            let call = first_call(&functions[1]);
            let changed = SemanticDirectCallV1::new(
                function_id(2),
                vec![
                    call.arguments()[0].clone(),
                    SemanticOperandV1::Move(place(0, U32)),
                ],
                call.destination().cloned(),
                call.unwind(),
            )
            .unwrap();
            replace_call(&mut functions[1], changed);
        }
        assert_eq!(
            request(functions)
                .admit(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
}

#[test]
fn rust_call_operands_and_result_are_checked_against_actual_callee() {
    let base = functions();
    let original = first_call(&base[1]);
    for (callee, arguments, destination) in [
        (
            1,
            original.arguments().to_vec(),
            original.destination().cloned(),
        ),
        (
            2,
            vec![
                SemanticOperandV1::Move(place(3, MUT_ENV)),
                original.arguments()[1].clone(),
            ],
            original.destination().cloned(),
        ),
        (
            2,
            vec![
                original.arguments()[0].clone(),
                SemanticOperandV1::Move(place(0, U32)),
            ],
            original.destination().cloned(),
        ),
        (
            2,
            original.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                place(2, TUPLE),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
        ),
    ] {
        let mut functions = base.clone();
        replace_call(
            &mut functions[1],
            SemanticDirectCallV1::new(
                function_id(callee),
                arguments,
                destination,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        assert!(matches!(
            request(functions).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::TypeMismatch { .. })
        ));
    }
    let mut functions = base;
    let call = first_call(&functions[1]);
    let flattened = SemanticDirectCallV1::new(
        function_id(2),
        vec![
            call.arguments()[0].clone(),
            SemanticOperandV1::Move(projected(2, SemanticProjectionKindV1::Field(0), PTR)),
            SemanticOperandV1::Move(projected(2, SemanticProjectionKindV1::Field(1), U32)),
        ],
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap();
    replace_call(&mut functions[1], flattened);
    assert!(matches!(
        request(functions).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { .. })
    ));
}

#[test]
fn rust_call_unwind_and_non_helper_roles_still_fail_expansion() {
    for unwind in [
        SemanticUnwindActionV1::Continue,
        SemanticUnwindActionV1::Terminate,
        SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 1)),
    ] {
        let mut functions = functions();
        let call = first_call(&functions[1]);
        let changed = SemanticDirectCallV1::new(
            function_id(2),
            call.arguments().to_vec(),
            call.destination().cloned(),
            unwind,
        )
        .unwrap();
        replace_call(&mut functions[1], changed);
        let source = request(functions)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        assert!(
            matches!(SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default()), Err(SemanticCallExpansionErrorV1::Unsupported { function, .. }) if function == function_id(2))
        );
    }
    let mut functions = functions();
    let closure = &functions[2];
    functions[2] = rebuild(
        closure,
        closure.abi().clone(),
        closure.locals().to_vec(),
        closure.blocks().to_vec(),
        SemanticFunctionRoleV1::DeviceFfiExport,
    );
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(250))),
        closure_types(),
        vec![],
        vec![],
        vec![],
        functions,
        vec![function_id(0), function_id(2)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(
        matches!(SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default()), Err(SemanticCallExpansionErrorV1::Unsupported { function, .. }) if function == function_id(2))
    );
}

#[test]
fn rust_call_variadics_and_unwinding_abi_are_rejected_before_expansion() {
    let base = closure_abi(3, SHARED_ENV, TUPLE, fields()).unwrap();
    for (can_unwind, variadic) in [(true, false), (false, true)] {
        let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
            base.identity(),
            base.layout_identity(),
            base.canon_abi(),
            base.extern_abi(),
            can_unwind,
            variadic,
            base.fixed_count(),
            base.source_input_types().to_vec(),
            base.source_output_type(),
            base.arguments().to_vec(),
            base.return_value().clone(),
        );
        if variadic {
            assert_eq!(abi, Err(SemanticMirErrorV1::InvalidFunctionAbi));
        } else {
            let mut functions = functions();
            let closure = &functions[2];
            functions[2] = rebuild(
                closure,
                abi.unwrap(),
                closure.locals().to_vec(),
                closure.blocks().to_vec(),
                closure.role(),
            );
            assert_eq!(
                request(functions)
                    .admit(SemanticMirLimitsV1::default())
                    .unwrap_err(),
                SemanticMirErrorV1::InvalidFunctionAbi
            );
        }
    }
    let mut functions = functions();
    let call = first_call(&functions[1]);
    let mut arguments = call.arguments().to_vec();
    arguments.push(SemanticOperandV1::Copy(place(0, U32)));
    let changed = SemanticDirectCallV1::new_with_variadic_argument_abis(
        function_id(2),
        arguments,
        vec![direct(U32)],
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap();
    replace_call(&mut functions[1], changed);
    assert!(matches!(
        request(functions).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { .. })
    ));
}

#[test]
fn rust_call_evidence_replays_exact_source_ownership_abi_and_transfers() {
    let source = source();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let decoded = Evidence::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    decoded
        .verify_against_checked_expansion(&source, &expansion)
        .unwrap();
    assert_eq!(
        decoded.roots()[0].instances(),
        expansion.roots()[0].instances()
    );
    assert_eq!(
        decoded.roots()[0].block_origins(),
        expansion.roots()[0].block_origins()
    );
    assert!(!decoded.grants_proof_or_artifact_authority());

    let mut changed = functions();
    let closure = &changed[2];
    let abi = closure
        .abi()
        .clone()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ])
        .unwrap();
    changed[2] = rebuild(
        closure,
        abi,
        closure.locals().to_vec(),
        closure.blocks().to_vec(),
        closure.role(),
    );
    let mut substitutions = vec![changed];
    let mut changed = functions();
    let closure = &changed[2];
    changed[2] = rebuild(
        closure,
        closure_abi(33, SHARED_ENV, TUPLE, fields()).unwrap(),
        closure.locals().to_vec(),
        closure.blocks().to_vec(),
        closure.role(),
    );
    substitutions.push(changed);
    let mut changed = functions();
    let closure = &changed[2];
    let other_callee = function(
        3,
        false,
        false,
        vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
    );
    changed[2] = rebuild(
        &other_callee,
        closure.abi().clone(),
        closure.locals().to_vec(),
        closure.blocks().to_vec(),
        closure.role(),
    );
    substitutions.push(changed);
    let mut changed = functions();
    let call = first_call(&changed[1]);
    let copied_tuple = SemanticDirectCallV1::new(
        function_id(2),
        vec![
            call.arguments()[0].clone(),
            SemanticOperandV1::Copy(place(2, TUPLE)),
        ],
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap();
    replace_call(&mut changed[1], copied_tuple);
    substitutions.push(changed);
    for changed in substitutions {
        let changed = request(changed)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let changed_expansion = expand(&changed);
        changed_expansion.verify_replay(&changed).unwrap();
        assert_ne!(expansion.identity(), changed_expansion.identity());
        assert_eq!(
            expansion.verify_replay(&changed),
            Err(SemanticCallExpansionErrorV1::SourceMismatch)
        );
        assert_eq!(
            decoded.verify_against_checked_expansion(&changed, &changed_expansion),
            Err(SemanticCallExpansionEvidenceErrorV1::SourceMismatch)
        );
    }

    let mut corrupted = expand(&source);
    let origin = corrupted.roots[0].block_origins[0]
        .statements
        .iter_mut()
        .find(|origin| {
            matches!(
                origin,
                SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
            )
        })
        .unwrap();
    *origin = SemanticExpandedStatementOriginV1::ParameterTransfer {
        callee: SemanticCallInstanceIdV1(1),
        argument: 0,
    };
    assert_eq!(
        corrupted.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    );
    assert!(Evidence::from_checked_expansion(&source, &corrupted).is_err());
    assert!(
        decoded
            .verify_against_checked_expansion(&source, &corrupted)
            .is_err()
    );

    let mut corrupted = expand(&source);
    let root = &mut corrupted.roots[0];
    let tuple_transfer = root.block_origins[0]
        .statements
        .iter()
        .position(|origin| {
            matches!(
                origin,
                SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
            )
        })
        .unwrap();
    let mut blocks = root.body.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    let SemanticStatementKindV1::Assign(assignment) = statements[tuple_transfer].kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(tuple)) = assignment.value().kind()
    else {
        panic!()
    };
    statements[tuple_transfer] = transfer(
        assignment.destination().clone(),
        SemanticOperandV1::Copy(tuple.clone()),
        statements[tuple_transfer].source(),
    );
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    root.body = rebuild(
        &root.body,
        root.body.abi().clone(),
        root.body.locals().to_vec(),
        blocks,
        root.body.role(),
    );
    assert_ne!(
        *root.identity(),
        super::super::identity::root_content_identity(
            &source,
            root,
            &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        corrupted.verify_replay(&source),
        Err(SemanticCallExpansionErrorV1::ReplayMismatch)
    );
    assert!(Evidence::from_checked_expansion(&source, &corrupted).is_err());
    assert!(
        decoded
            .verify_against_checked_expansion(&source, &corrupted)
            .is_err()
    );
}
