use super::*;

fn call_fixture(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    moved: bool,
) -> SemanticTypeIdV1 {
    logical_fixture(types, functions, Fixture::Direct);
    let helper_index = functions.len() - 1;
    let old = &functions[helper_index];
    let enumeration = old.locals()[4].ty();
    let word = SemanticTypeIdV1::from_index(0);
    let declaration = &types[enumeration.index() as usize];
    let layout = declaration.layout();
    let SemanticRustcVariantsV1::Multiple(enumerated_layout) = layout.variants() else {
        panic!("genuine enum layout");
    };
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let payload = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let variant_layouts = enumerated_layout
        .variants()
        .iter()
        .map(|variant| {
            SemanticEnumVariantLayoutV1::from_rustc(
                variant.variant_index(),
                variant.rustc_size_bytes(),
                variant.alignment_bytes(),
                variant.fields().clone(),
                SemanticBackendReprV1::scalar_pair(tag, payload),
                variant.largest_niche(),
                variant.is_uninhabited(),
                variant.max_repr_alignment_bytes(),
                variant.unadjusted_abi_alignment_bytes(),
                variant.randomization_seed(),
                variant.aggregate().clone(),
            )
            .unwrap()
        })
        .collect();
    let enumerated_layout =
        SemanticEnumLayoutV1::new(variant_layouts, enumerated_layout.encoding().clone()).unwrap();
    types[enumeration.index() as usize] = SemanticTypeDeclV1::new(
        declaration.identity(),
        declaration.layout_identity(),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            layout.size_bytes().unwrap(),
            layout.alignment_bytes(),
            SemanticBackendReprV1::scalar_pair(tag, payload),
            false,
            enumerated_layout,
        )
        .unwrap(),
        declaration.shape().clone(),
    );
    let SemanticAbiPassModeV1::Direct(attributes) = old.abi().arguments()[0].mode() else {
        panic!("genuine word input ABI");
    };
    assert_eq!(old.abi().canon_abi(), SemanticCanonAbiV1::Rust);
    // The full-range u8 tag is not a bool or a foreign-ABI integer.
    let tag_attributes = SemanticAbiValueAttributesV1::new(
        attributes.regular(),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let mode = SemanticAbiPassModeV1::Pair {
        first: tag_attributes,
        second: *attributes,
    };
    let abi = SemanticFunctionAbiV1::new(
        old.abi().identity(),
        old.abi().layout_identity(),
        old.abi().canon_abi(),
        false,
        false,
        vec![
            SemanticAbiValueV1::new(enumeration, mode.clone()),
            old.abi().arguments()[1].value().clone(),
        ],
        SemanticAbiValueV1::new(enumeration, mode),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let source = old.source();
    let mut locals = old.locals().to_vec();
    for (local, role) in [
        (0, SemanticLocalRoleV1::Return),
        (1, SemanticLocalRoleV1::Argument(0)),
    ] {
        locals[local] =
            SemanticLocalDeclV1::new(locals[local].identity(), enumeration, role, source);
    }
    let body = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0, enumeration),
            SemanticRvalueV1::new(
                enumeration,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, enumeration))),
            ),
        )),
    );
    functions[helper_index] = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        abi,
        locals,
        old.entry(),
        vec![
            SemanticBasicBlockV1::new(
                old.blocks()[0].identity(),
                source,
                vec![body],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    for root in 0..helper_index {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        assert_eq!(locals.len(), 4);
        locals.extend([
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([242; 32]),
                enumeration,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([243; 32]),
                word,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
        ]);
        let variant = u32::try_from(root).unwrap();
        assert!(variant < 2);
        let SemanticTypeShapeV1::Enum { variants, .. } =
            types[enumeration.index() as usize].shape()
        else {
            panic!("original enum variants");
        };
        let discriminant = variants[variant as usize].discriminant();
        let mut blocks = Vec::new();
        for block in 0..5 {
            let statements = if block == 0 {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(4, enumeration),
                        SemanticRvalueV1::new(
                            enumeration,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::EnumVariant(variant),
                                    vec![SemanticOperandV1::Copy(place(1, word))],
                                )
                                .unwrap(),
                            ),
                        ),
                    )),
                )]
            } else if block == 2 {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(5, word),
                        SemanticRvalueV1::new(
                            word,
                            SemanticRvalueKindV1::Discriminant(place(4, enumeration)),
                        ),
                    )),
                )]
            } else if block == 3 {
                let field = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(4),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Downcast(variant),
                            enumeration,
                        )
                        .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word)
                            .unwrap(),
                    ],
                    word,
                )
                .unwrap();
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(3, word),
                        SemanticRvalueV1::new(
                            word,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field)),
                        ),
                    )),
                )]
            } else {
                vec![]
            };
            let terminator = if block < 2 {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(helper_index as u32),
                        vec![
                            if moved {
                                SemanticOperandV1::Move(place(4, enumeration))
                            } else {
                                SemanticOperandV1::Copy(place(4, enumeration))
                            },
                            SemanticOperandV1::Copy(place(2, word)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            place(4, enumeration),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(block + 1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            } else if block == 2 {
                // A call result is not authenticated by its caller's earlier constructor.
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(5, word)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            discriminant,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(3),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(4),
                        ),
                    )
                    .unwrap(),
                }
            } else if block == 3 {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(4),
                ))
            } else {
                SemanticTerminatorKindV1::Return
            };
            blocks.push(
                SemanticBasicBlockV1::new(
                    old.blocks().get(block as usize).map_or_else(
                        || SemanticBlockIdentityV1::from_sha256([244; 32]),
                        SemanticBasicBlockV1::identity,
                    ),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap(),
            );
        }
        functions[root] = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
    enumeration
}

fn call_program(moved: bool, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    let completed = std::cell::Cell::new(false);
    let result = super::super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            call_fixture(types, functions, moved);
        },
        |plan, out| {
            super::super::super::super::source_function::tests::with_slots(
                plan,
                out,
                |slots, out| {
                    for root in 0..2 {
                        let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                        let context = body.context(out)?;
                        let ty = context.function.locals()[4].ty();
                        for block in 0..2 {
                            let operand = body.call_argument(block, 0, out)?;
                            assert_eq!(operand.ty, ty);
                            assert!(operand.scalar().is_none());
                            assert!(matches!(operand.kind, OperandKind::Enum {
                                    local, source_type, moved: original_move,
                                } if local == context.locals.start + 4
                                    && source_type == ty && original_move == moved));
                        }
                    }
                    let mut source =
                        super::super::super::super::source_function::SourceByteProgram::derive(
                            plan, slots, out,
                        )?;
                    let paired = super::super::super::super::paired::PairedInvocations::derive(
                        plan,
                        &source,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        out,
                    )?;
                    let bindings =
                        super::super::super::super::byte_bindings::SourceByteBindings::derive(
                            slots, out,
                        )?;
                    slots.emit(out)?;
                    source.emit(out)?;
                    bindings.emit(out)?;
                    paired.emit(out)?;
                    for root in 0..2 {
                        for instance in 1..3 {
                            let enter =
                                format!("spec fn invocation_source_enter_{root}_{instance}_v36(");
                            let wrapper = out
                                .text
                                .split(&enter)
                                .nth(1)
                                .expect("actual internal entry")
                                .split("spec fn ")
                                .next()
                                .unwrap();
                            assert!(wrapper.contains(&format!("invocation_source_entry_select_v167(source, invocation_source_entry_refuses_{root}_{instance}_v167(source, arguments, little_endian), invocation_source_entry_body_{root}_{instance}_v167(source, arguments, little_endian))")));
                            let refuses = out.text.split_once(&format!("spec fn invocation_source_entry_refuses_{root}_{instance}_v167("))
                                .unwrap().1.split_once("\n}\n").unwrap().0;
                            assert!(refuses.contains("InvocationSourceValueV42::Enum(value)"));
                            assert!(
                                refuses
                                    .contains("invocation_source_enum_snapshot_current_v50(source")
                            );
                            let body = out
                                .text
                                .split_once(&format!(
                                    "spec fn invocation_source_entry_body_{root}_{instance}_v167("
                                ))
                                .unwrap()
                                .1
                                .split_once("\n}\n")
                                .unwrap()
                                .0;
                            assert!(body.contains("InvocationSourceValueV42::Enum(value)"));
                            assert!(
                                body.contains(
                                    "invocation_source_enum_snapshot_current_v50(entered"
                                )
                            );
                            let returned =
                                format!("spec fn invocation_source_return_{root}_{instance}_v36(");
                            let body = out
                                .text
                                .split(&returned)
                                .nth(1)
                                .expect("actual internal return")
                                .split("spec fn ")
                                .next()
                                .unwrap();
                            assert!(body.contains("invocation_source_enum_snapshot_v50(source"));
                        }
                        let name = format!("spec fn invocation_paired_control_values_{root}_v36(");
                        let body = out
                            .text
                            .split(&name)
                            .nth(1)
                            .expect("actual snapshot relation")
                            .split("spec fn ")
                            .next()
                            .unwrap();
                        assert!(body.contains("InvocationSourceValueV42::Enum(value)"));
                        assert!(
                            body.contains("invocation_source_enum_snapshot_current_v50(source")
                        );
                        assert!(body.contains("value.fields.contains_key(0)"));
                    }
                    completed.set(true);
                    Ok(())
                },
            )
        },
    );
    if result.0.is_ok() {
        assert!(
            completed.get(),
            "successful enum call callback must complete"
        );
    }
    result
}

#[test]
fn original_enum_internal_calls_snapshot_copy_move_and_return_before_cleanup() {
    for moved in [false, true] {
        call_program(moved, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn original_enum_internal_call_emission_has_exact_and_one_short_resources() {
    let measured = call_program(true, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = call_program(true, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for (work, storage, work_short) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let error = call_program(true, work, storage).0.unwrap_err();
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

#[test]
fn original_enum_call_operand_classification_rejects_wrong_local_type_and_projection() {
    let completed = std::cell::Cell::new(false);
    let result = super::super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            call_fixture(types, functions, true);
        },
        |plan, out| {
            super::super::super::super::source_function::tests::with_slots(
                plan,
                out,
                |slots, out| {
                    for root in 0..2 {
                        let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                        let context = body.context(out)?;
                        let original = body.call_argument(0, 0, out)?;
                        assert!(matches!(
                            original.kind,
                            OperandKind::Enum { moved: true, .. }
                        ));
                        let ty = original.ty;
                        let wrong_local =
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ty)
                                .unwrap();
                        let projected = SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(4),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Downcast(0),
                                    ty,
                                )
                                .unwrap(),
                            ],
                            ty,
                        )
                        .unwrap();
                        for place in [wrong_local, projected] {
                            let error = context.typed_operand(&SemanticOperandV1::Move(place), out)
                            .err().expect("bounded enum operand classifier must reject the malformed place");
                            assert!(matches!(
                                error,
                                Error::Statement(
                                    "original MIR typed byte statement is not modeled"
                                )
                            ));
                        }
                        for (block, ordinal) in [(2, 0), (0, 2)] {
                            let error = body
                                .call_argument(block, ordinal, out)
                                .err()
                                .expect("only original call arguments have a descriptor");
                            assert!(matches!(
                                error,
                                Error::Statement(
                                    "original MIR typed byte statement identity or layout differs"
                                )
                            ));
                        }
                        assert!(matches!(
                            body.call_argument(0, 0, out)?.kind,
                            OperandKind::Enum { moved: true, .. }
                        ));
                    }
                    completed.set(true);
                    Ok(())
                },
            )
        },
    );
    result.0.unwrap();
    assert!(
        completed.get(),
        "all original contexts and bounded negative queries must run"
    );
}

#[test]
fn original_enum_call_storage_does_not_initialize_moved_or_inactive_payloads() {
    for moved in [false, true] {
        call_program(moved, LIMIT, LIMIT).0.unwrap();
        for invalid in 0..3 {
            let inactive = invalid == 1;
            let unguarded = invalid == 2;
            let reached = std::cell::Cell::new(false);
            let transform =
                |types: &mut Vec<SemanticTypeDeclV1>,
                 functions: &mut Vec<SemanticFunctionDeclV1>| {
                    let enumeration = call_fixture(types, functions, moved);
                    for root in 0..functions.len() - 1 {
                        let old = &functions[root];
                        let word = old.locals()[3].ty();
                        let variant = u32::try_from(root).unwrap();
                        let field = SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(4),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Downcast(if inactive {
                                        1 - variant
                                    } else {
                                        variant
                                    }),
                                    enumeration,
                                )
                                .unwrap(),
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word)
                                    .unwrap(),
                            ],
                            word,
                        )
                        .unwrap();
                        let selected = if unguarded {
                            2
                        } else if inactive {
                            3
                        } else {
                            0
                        };
                        let mut blocks = old.blocks().to_vec();
                        let block = &blocks[selected];
                        let statements = if unguarded {
                            block.statements().to_vec()
                        } else if inactive {
                            let mut statements = block.statements().to_vec();
                            let SemanticStatementKindV1::Assign(original) = statements[0].kind()
                            else {
                                panic!("original payload read");
                            };
                            statements[0] = SemanticStatementV1::new(
                                block.source(),
                                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                    original.destination().clone(),
                                    SemanticRvalueV1::new(
                                        word,
                                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field)),
                                    ),
                                )),
                            );
                            statements
                        } else {
                            let mut statements = block.statements().to_vec();
                            assert_eq!(statements.len(), 1);
                            statements.push(SemanticStatementV1::new(
                                block.source(),
                                SemanticStatementKindV1::Deinitialize(field),
                            ));
                            statements
                        };
                        blocks[selected] = SemanticBasicBlockV1::new(
                            block.identity(),
                            block.source(),
                            statements,
                            if unguarded {
                                SemanticTerminatorV1::new(
                                    block.source(),
                                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::Goto,
                                        SemanticBlockIdV1::from_index(3),
                                    )),
                                )
                            } else {
                                block.terminator().clone()
                            },
                        )
                        .unwrap();
                        functions[root] = SemanticFunctionDeclV1::new(
                            old.identity(),
                            old.role(),
                            old.item_definition_identity(),
                            old.monomorphization_identity(),
                            old.generic_type_arguments_identity(),
                            old.const_generic_arguments_identity(),
                            old.source(),
                            old.abi().clone(),
                            old.locals().to_vec(),
                            old.entry(),
                            blocks,
                        )
                        .unwrap()
                        .with_kernel_entry(old.kernel_entry().unwrap().clone());
                    }
                };
            if invalid == 0 {
                let error = super::super::super::super::super::invocations::tests::try_source_ssa_transform(transform)
                    .err()
                    .expect("passing a partially deinitialized enum must fail at SSA admission");
                assert_eq!(
                    error,
                    fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                        function: SemanticFunctionIdV1::from_index(0),
                        block: 0,
                        statement: None,
                        local: 4,
                        violation:
                            fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    }
                );
                assert!(!reached.get());
                continue;
            }
            let result =
                super::super::super::super::super::invocations::tests::run_source_transform(
                    LIMIT,
                    LIMIT,
                    transform,
                    |_, _| {
                        reached.set(true);
                        Ok(())
                    },
                );
            if unguarded {
                // Both genuine identity calls preserve the original constructor
                // snapshot. Its still-current tag is authority without a Switch.
                result.0.unwrap();
                assert!(reached.get());
                continue;
            }
            let error = result.0.unwrap_err();
            let expected = "source enum payload differs from its original guarded value";
            assert!(format!("{error:?}").contains(expected), "{error:?}");
            assert!(!reached.get(), "invalid source must not reach the callback");
        }
    }
}

#[test]
fn original_enum_calls_refuse_stale_discriminants_and_post_guard_reassignment() {
    for moved in [false, true] {
        for after_guard in [false, true] {
            let reached = std::cell::Cell::new(false);
            let result =
                super::super::super::super::super::invocations::tests::run_source_transform(
                    LIMIT,
                    LIMIT,
                    |types, functions| {
                        let enumeration = call_fixture(types, functions, moved);
                        for root in 0..functions.len() - 1 {
                            let old = &functions[root];
                            let word = old.locals()[1].ty();
                            let place = |local, ty| {
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(local),
                                    vec![],
                                    ty,
                                )
                                .unwrap()
                            };
                            let replacement = SemanticStatementV1::new(
                                old.source(),
                                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                    place(4, enumeration),
                                    SemanticRvalueV1::new(
                                        enumeration,
                                        SemanticRvalueKindV1::Aggregate(
                                            SemanticAggregateRvalueV1::new(
                                                SemanticAggregateKindV1::EnumVariant(
                                                    1 - root as u32,
                                                ),
                                                vec![SemanticOperandV1::Copy(place(1, word))],
                                            )
                                            .unwrap(),
                                        ),
                                    ),
                                )),
                            );
                            let mut blocks = old.blocks().to_vec();
                            let selected = if after_guard { 3 } else { 2 };
                            let block = &blocks[selected];
                            let mut statements = block.statements().to_vec();
                            if after_guard {
                                statements.insert(0, replacement);
                            } else {
                                statements.push(replacement);
                            }
                            blocks[selected] = SemanticBasicBlockV1::new(
                                block.identity(),
                                block.source(),
                                statements,
                                block.terminator().clone(),
                            )
                            .unwrap();
                            functions[root] = SemanticFunctionDeclV1::new(
                                old.identity(),
                                old.role(),
                                old.item_definition_identity(),
                                old.monomorphization_identity(),
                                old.generic_type_arguments_identity(),
                                old.const_generic_arguments_identity(),
                                old.source(),
                                old.abi().clone(),
                                old.locals().to_vec(),
                                old.entry(),
                                blocks,
                            )
                            .unwrap()
                            .with_kernel_entry(old.kernel_entry().unwrap().clone());
                        }
                    },
                    |_, _| {
                        reached.set(true);
                        Ok(())
                    },
                );
            let error = result.0.unwrap_err();
            let detail = format!("{error:?}");
            assert!(
                detail.contains("source enum payload differs from its original guarded value")
                    || detail.contains("source reference reads an uninitialized partial holder"),
                "{error:?}"
            );
            assert!(
                !reached.get(),
                "an old tag cannot authenticate the reassigned enum"
            );
        }
    }
}

#[test]
fn original_enum_snapshots_preserve_source_completeness_and_pointer_escape_guards() {
    let values = include_str!("original_semantic_mir_source_enum_values_v47.vrs");
    let frames = include_str!("original_semantic_mir_invocation_source_frames_v36.rs");
    let aggregate = include_str!("original_semantic_mir_source_aggregate_values_v42.vrs");
    assert!(values.contains("!moved && !invocation_source_enum_type_copyable_v47(source_type)"));
    assert!(values.contains("invocation_source_enum_complete_v47(value)"));
    assert!(values.contains("invocation_source_enum_snapshot_v50(source, local, source_type"));
    assert!(values.contains("value: InvocationSourceValueV42::Enum(value)"));
    assert!(frames.contains("InvocationSourceValueV42::Enum(value) => exists|field: int|"));
    assert!(
        frames
            .contains("invocation_source_enum_snapshot_current_v50(source, value, little_endian)")
    );
    assert!(aggregate.contains("InvocationSourceOperandV36::Enum { local, source_type, moved }"));
}
