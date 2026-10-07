use super::super::super::super::{paired, source_function};
use super::super::super::descriptor_helpers;
use super::*;

fn helper_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
    mutable: bool,
    reborrow: bool,
) {
    if mutable {
        mutable_fixture(types, functions, callables);
    } else {
        fixture(types, functions, callables);
    }
    let input = if mutable { 11 } else { 6 };
    let reference = functions[0].locals()[input].ty();
    let descriptor = functions[0].locals()[4].ty();
    let word = TypeId::from_index(0);
    let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let old = &functions[2];
    let source = old.source();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            if mutable {
                None
            } else {
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
            },
            true,
            !mutable,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    let reference_abi =
        SemanticAbiValueV1::new(reference, SemanticAbiPassModeV1::Direct(attributes));
    // Reference returns retain validity/alignment, not argument-only alias,
    // capture, read-only, or dereferenceable-byte attributes.
    let return_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        Some(8),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::new(
        old.abi().identity(),
        old.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![reference_abi, old.abi().arguments()[1].value().clone()],
        SemanticAbiValueV1::new(reference, SemanticAbiPassModeV1::Direct(return_attributes)),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        if mutable {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        },
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let mut locals = old.locals().to_vec();
    for local in [0, 1] {
        locals[local] = SemanticLocalDeclV1::new(
            locals[local].identity(),
            reference,
            if local == 0 {
                SemanticLocalRoleV1::Return
            } else {
                SemanticLocalRoleV1::Argument(0)
            },
            source,
        );
    }
    let value = if reborrow {
        Rvalue::Borrow {
            kind: if mutable {
                SemanticBorrowKindV1::Mutable
            } else {
                SemanticBorrowKindV1::Shared
            },
            place: Place::new(
                SemanticLocalIdV1::from_index(1),
                vec![SemanticProjectionV1::new(Projection::Dereference, descriptor).unwrap()],
                descriptor,
            )
            .unwrap(),
        }
    } else {
        Rvalue::Use(if mutable {
            Operand::Move(place(1, reference))
        } else {
            Operand::Copy(place(1, reference))
        })
    };
    let blocks = vec![
        SemanticBasicBlockV1::new(
            old.blocks()[0].identity(),
            source,
            vec![SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(0, reference),
                    SemanticRvalueV1::new(reference, value),
                )),
            )],
            SemanticTerminatorV1::new(source, Terminator::Return),
        )
        .unwrap(),
    ];
    functions[2] = Function::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    for root in 0..2 {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        let result = locals.len() as u32;
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([249 + root as u8; 32]),
            reference,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let mut blocks = old.blocks().to_vec();
        if mutable {
            let entry = &blocks[0];
            let mut statements = entry.statements().to_vec();
            statements.insert(
                0,
                SemanticStatementV1::new(
                    source,
                    Statement::StorageLive(SemanticLocalIdV1::from_index(6)),
                ),
            );
            blocks[0] = SemanticBasicBlockV1::new(
                entry.identity(),
                source,
                statements,
                entry.terminator().clone(),
            )
            .unwrap();
        }
        for block in 0..blocks.len() {
            let declaration = &blocks[block];
            let Terminator::Call(call) = declaration.terminator().kind() else {
                continue;
            };
            if !matches!(callables[call.callee().index() as usize],
                SemanticCallableDeclV1::Defined { function } if function.index() == 2)
            {
                continue;
            }
            let input = if block == 1 { result } else { input as u32 };
            let operand = if mutable {
                Operand::Move(place(input, reference))
            } else {
                Operand::Copy(place(input, reference))
            };
            let mut statements = declaration.statements().to_vec();
            if mutable && block == 4 {
                // The shared Len child loan is no longer live before the mutable call.
                statements.push(SemanticStatementV1::new(
                    source,
                    Statement::StorageDead(SemanticLocalIdV1::from_index(6)),
                ));
            }
            let destination = call.destination().unwrap();
            blocks[block] = SemanticBasicBlockV1::new(
                declaration.identity(),
                source,
                statements,
                SemanticTerminatorV1::new(
                    source,
                    Terminator::Call(
                        SemanticDirectCallV1::new_callable(
                            call.callee(),
                            vec![operand, Operand::Copy(place(2, word))],
                            Some(SemanticCallDestinationV1::new(
                                place(result, reference),
                                destination.edge().clone(),
                            )),
                            call.unwind(),
                        )
                        .unwrap(),
                    ),
                ),
            )
            .unwrap();
        }
        // Consume the final helper result in the ordinary source model.
        let shared = old.locals()[6].ty();
        let mut statements = blocks[2].statements().to_vec();
        if mutable {
            statements.push(SemanticStatementV1::new(
                source,
                Statement::StorageLive(SemanticLocalIdV1::from_index(6)),
            ));
        }
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(6, shared),
                SemanticRvalueV1::new(
                    shared,
                    Rvalue::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: Place::new(
                            SemanticLocalIdV1::from_index(result),
                            vec![
                                SemanticProjectionV1::new(Projection::Dereference, descriptor)
                                    .unwrap(),
                            ],
                            descriptor,
                        )
                        .unwrap(),
                    },
                ),
            )),
        ));
        let Terminator::Call(length) = blocks[0].terminator().kind() else {
            panic!("Len before helpers");
        };
        let length_callable = length.callee();
        let length_type = old.locals()[8].ty();
        let return_block = blocks.len() as u32;
        blocks[2] = SemanticBasicBlockV1::new(
            blocks[2].identity(),
            source,
            statements,
            SemanticTerminatorV1::new(
                source,
                Terminator::Call(
                    SemanticDirectCallV1::new_callable(
                        length_callable,
                        vec![Operand::Copy(place(6, shared))],
                        Some(SemanticCallDestinationV1::new(
                            place(8, length_type),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(return_block),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([251 + root as u8; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, Terminator::Return),
            )
            .unwrap(),
        );
        functions[root] = Function::new(
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
}

fn run_helpers(
    work: usize,
    storage: usize,
    mutable: bool,
    reborrow: bool,
    hostile: bool,
) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(0usize);
    let result =
        super::super::super::super::super::invocations::tests::run_captured_callable_transform(
            work,
            storage,
            |types, functions, callables| {
                helper_fixture(types, functions, callables, mutable, reborrow)
            },
            capture,
            |plan, out| {
                source_function::tests::with_slots(plan, out, |slots, out| {
                    for root in 0..2 {
                        let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                        let operand =
                            descriptor_helpers::call_argument(slots, plan, root, 0, 4, 0, out)?
                                .expect("first real helper descriptor operand");
                        assert_eq!(operand.recipe.mutable, mutable);
                        assert_eq!(operand.moved, mutable);
                        assert_eq!(operand.recipe.origin, body.locals.start + 4);
                        let child = plan
                            .calls(root, 0, out)?
                            .iter()
                            .find(|call| call.block.index() == 4)
                            .unwrap()
                            .child
                            .unwrap();
                        assert_eq!(
                            body.invocation_argument(plan, 4, 0, child, out)?.scalar(),
                            None
                        );
                        let mut children = 0;
                        for instance in 1..plan.root(root, out)?.instances.len() {
                            let row = plan.instance(root, instance, out)?;
                            if !row.active || row.function.index() != 2 {
                                continue;
                            }
                            let entry = descriptor_helpers::entry_recipe(
                                slots,
                                plan,
                                root,
                                instance,
                                SemanticLocalIdV1::from_index(1),
                                out,
                            )?
                            .expect("helper nominal input");
                            let returned = descriptor_helpers::return_recipe(
                                slots,
                                plan,
                                root,
                                instance,
                                0,
                                SemanticLocalIdV1::from_index(0),
                                out,
                            )?
                            .expect("helper nominal return");
                            assert_eq!(entry.origin, operand.recipe.origin);
                            assert_eq!(returned.origin, entry.origin);
                            assert_eq!(returned.mutable, mutable);
                            let (parent, callsite) = row.incoming.unwrap();
                            assert_eq!(
                                descriptor_helpers::destination_recipe(
                                    slots,
                                    plan,
                                    root,
                                    parent,
                                    callsite.index() as usize,
                                    out
                                )?,
                                returned
                            );
                            if reborrow {
                                assert_eq!(returned.instance, instance);
                            } else {
                                assert_eq!(returned, entry);
                            }
                            children += 1;
                        }
                        assert_eq!(children, 2);
                        if hostile {
                            assert!(matches!(
                                descriptor_helpers::entry_recipe(
                                    slots,
                                    plan,
                                    root,
                                    0,
                                    SemanticLocalIdV1::from_index(6),
                                    out
                                ),
                                Err(Error::Statement(
                                    "original MIR byte frame entry differs from its exact invocation"
                                ))
                            ));
                            assert!(matches!(
                                descriptor_helpers::return_recipe(
                                    slots,
                                    plan,
                                    root,
                                    0,
                                    0,
                                    SemanticLocalIdV1::from_index(6),
                                    out
                                ),
                                Err(Error::Statement(
                                    "original MIR typed byte statement is not modeled"
                                ))
                            ));
                            assert!(matches!(
                                descriptor_helpers::destination_recipe(
                                    slots, plan, root, 0, 0, out
                                ),
                                Err(Error::Statement(
                                    "original MIR typed byte statement identity or layout differs"
                                ))
                            ));
                        }
                        reached.set(reached.get() + 1);
                    }
                    let mut program = source_function::SourceByteProgram::derive(plan, slots, out)?;
                    let paired = paired::PairedInvocations::derive(
                        plan,
                        &program,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        out,
                    )?;
                    let begin = out.text.len();
                    program.emit(out)?;
                    paired.emit(out)?;
                    let text = &out.text[begin..];
                    assert!(text.contains("InvocationSourceOperandV36::Descriptor"));
                    assert!(text.contains("invocation_source_descriptor_snapshot_install_v53"));
                    assert!(text.contains("InvocationSourceValueV42::Descriptor(snapshot) => invocation_source_descriptor_snapshot_current_v53"));
                    Ok(())
                })
            },
        );
    if result.0.is_ok() {
        assert_eq!(reached.get(), 2);
    }
    result
}

#[test]
fn original_descriptor_helpers_preserve_shared_copy_mutable_move_and_returned_reborrow() {
    for mutable in [false, true] {
        for reborrow in [false, true] {
            run_helpers(LIMIT, LIMIT, mutable, reborrow, false)
                .0
                .unwrap();
        }
    }
}

#[test]
fn original_descriptor_helper_queries_reject_non_argument_return_and_scalar_destinations() {
    for mutable in [false, true] {
        run_helpers(LIMIT, LIMIT, mutable, true, false).0.unwrap();
        run_helpers(LIMIT, LIMIT, mutable, true, true).0.unwrap();
    }
}

#[test]
fn original_descriptor_helpers_have_exact_complete_work_and_peak_boundaries() {
    for mutable in [false, true] {
        let measured = run_helpers(LIMIT, LIMIT, mutable, true, false);
        measured.0.unwrap();
        let exact = run_helpers(measured.1, measured.3, mutable, true, false);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
        for work in [true, false] {
            let failure = run_helpers(
                measured.1 - usize::from(work),
                measured.3 - usize::from(!work),
                mutable,
                true,
                false,
            );
            assert!(
                matches!((&failure.0, work),
                (Err(Error::Resource(Resource::Work(error))), true) if error.actual() == measured.1 && error.limit() == measured.1 - 1)
                    || matches!((&failure.0, work),
                (Err(Error::Source(SourceError::Resource(Resource::Work(error)))), true) if error.actual() == measured.1 && error.limit() == measured.1 - 1)
                    || matches!((&failure.0, work),
                (Err(Error::Resource(Resource::Storage(error))), false) if error.actual() == measured.3 && error.limit() == measured.3 - 1)
                    || matches!((&failure.0, work),
                (Err(Error::Source(SourceError::Resource(Resource::Storage(error)))), false) if error.actual() == measured.3 && error.limit() == measured.3 - 1),
                "{:?}",
                failure.0
            );
        }
    }
}

#[test]
fn descriptor_helper_snapshots_keep_version_frame_and_post_pop_install_obligations() {
    let runtime = include_str!("original_semantic_mir_source_descriptor_snapshots_v53.vrs");
    assert!(runtime.contains("source.logical.versions[recipe.origin] == reference.loan.version"));
    assert!(runtime.contains("source.machine.frames.active[i] == reference.loan.frame"));
    assert!(runtime.contains("snapshot.reference.loan.frame == frame"));
    assert!(runtime.contains("if recipe.mutable && !moved"));
    let frames = include_str!("original_semantic_mir_invocation_source_frames_v36.rs");
    assert!(
        frames
            .find("frames: byte_pop_frame_v30(source.machine.frames)")
            .unwrap()
            < frames
                .find("Some(destination) => invocation_source_return_install_v42(cleaned")
                .unwrap()
    );
    assert!(frames.contains("logical.descriptor_references[i].loan.frame == frame"));
}
