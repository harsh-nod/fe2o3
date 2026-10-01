use super::*;

fn pointer_type(tag: u8, pointee: TypeId, bytes: u64, alignment: u64) -> Type {
    Type::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                PointerKind::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                PointerMetadata::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    bytes,
                    alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn indexed_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    indexed_fixture_kind(types, functions, callables, false);
}

fn disjoint_indexed_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    indexed_fixture_kind(types, functions, callables, true);
}

fn indexed_fixture_kind(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
    disjoint: bool,
) {
    fixture(types, functions, callables);
    let word = TypeId::from_index(0);
    let unit = TypeId::from_index(1);
    let descriptor = functions[0].locals()[4].ty();
    let raw = functions[0].locals()[8].ty();
    let mutable = TypeId::from_index(types.len() as u32);
    types.push(pointer_type(206, descriptor, 16, 8));
    let witness = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([207; 32]),
        SemanticLayoutIdentityV1::from_sha256([207; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            types[raw.index() as usize].layout().backend_repr().clone(),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        Shape::Aggregate(SemanticAggregateTypeV1::new(vec![raw, unit]).unwrap()),
    ));
    let reference = TypeId::from_index(types.len() as u32);
    types.push(pointer_type(208, word, 4, 4));
    let option = TypeId::from_index(types.len() as u32);
    let pointer = BackendPrimitive::pointer(0, 8, 8);
    let nonnull = SemanticScalarValidityRangeV1::new(1, u64::MAX.into());
    let niche = SemanticLayoutNicheV1::new(0, pointer, nonnull).unwrap();
    let nullable = BackendScalar::initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0));
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([209; 32]),
            SemanticLayoutIdentityV1::from_sha256([209; 32]),
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                8,
                8,
                BackendRepr::scalar(nullable),
                false,
                SemanticEnumLayoutV1::new(
                    vec![
                        SemanticEnumVariantLayoutV1::from_rustc(
                            0,
                            8,
                            8,
                            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                            BackendRepr::memory(true),
                            None,
                            false,
                            None,
                            8,
                            0,
                            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                        )
                        .unwrap(),
                        SemanticEnumVariantLayoutV1::from_rustc(
                            1,
                            8,
                            8,
                            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
                            BackendRepr::scalar(BackendScalar::initialized(pointer, nonnull)),
                            Some(niche),
                            false,
                            None,
                            8,
                            0,
                            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                        )
                        .unwrap(),
                    ],
                    SemanticEnumEncodingV1::Niche(
                        SemanticNicheEnumEncodingV1::new(
                            0,
                            SemanticNicheSourceV1::new(
                                vec![SemanticNichePathComponentV1::Field(0)],
                                0,
                            )
                            .unwrap(),
                            niche,
                            nullable,
                            1,
                            0,
                            0,
                            0,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            )
            .unwrap(),
            Shape::Enum {
                discriminant: word,
                variants: vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![reference]).unwrap(),
                    ),
                ]
                .into_boxed_slice(),
            },
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                        4,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let owned_witness = if disjoint {
        let owned = TypeId::from_index(types.len() as u32);
        types.push(Type::new(
            SemanticTypeIdentityV1::from_sha256([210; 32]),
            SemanticLayoutIdentityV1::from_sha256([210; 32]),
            types[witness.index() as usize].layout().clone(),
            types[witness.index() as usize].shape().clone(),
        ));
        owned
    } else {
        witness
    };
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let unique = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    let result = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let issue = SemanticCallableIdV1::from_index(callables.len() as u32);
    let mut intrinsics = vec![(
        241,
        vec![],
        SemanticAbiValueV1::new(witness, SemanticAbiPassModeV1::Direct(plain)),
        vec![],
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: witness,
            raw_index: raw,
        },
    )];
    if disjoint {
        intrinsics.push((
            242,
            vec![SemanticAbiValueV1::new(
                witness,
                SemanticAbiPassModeV1::Direct(plain),
            )],
            SemanticAbiValueV1::new(owned_witness, SemanticAbiPassModeV1::Direct(plain)),
            vec![SemanticSourceArgumentOwnershipV1::ByValue],
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                input_witness: witness,
                output_witness: owned_witness,
                raw_index: raw,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
            },
        ));
    }
    intrinsics.push((
        242 + u8::from(disjoint),
        vec![
            SemanticAbiValueV1::new(mutable, SemanticAbiPassModeV1::Direct(unique)),
            SemanticAbiValueV1::new(owned_witness, SemanticAbiPassModeV1::Direct(plain)),
        ],
        SemanticAbiValueV1::new(option, SemanticAbiPassModeV1::Direct(result)),
        vec![
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            if disjoint {
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            } else {
                SemanticSourceArgumentOwnershipV1::ByValue
            },
        ],
        if disjoint {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                disjoint_slice: descriptor,
                index_witness: owned_witness,
                element: word,
                raw_index: raw,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
            }
        } else {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice: descriptor,
                index_witness: witness,
                element: word,
                raw_index: raw,
            }
        },
    ));
    for (tag, inputs, output, ownership, operation) in intrinsics {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            functions[0].abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs,
            output,
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap();
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                functions[0].source(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        });
    }
    let extra = u32::from(disjoint);
    let get = SemanticCallableIdV1::from_index(issue.index() + 1 + extra);
    for root in 0..2 {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        assert_eq!(locals.len(), 9);
        for (ordinal, ty) in [mutable, witness, option, word, reference, word]
            .into_iter()
            .enumerate()
        {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([230 + ordinal as u8; 32]),
                ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        if disjoint {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([236; 32]),
                owned_witness,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        let place =
            |local: u32, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let edge = |role, block| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
        };
        let assign = |destination, value| {
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(destination, value)),
            )
        };
        let invoke = |callee, arguments, local, ty, target| {
            SemanticTerminatorV1::new(
                source,
                Terminator::Call(
                    SemanticDirectCallV1::new_callable(
                        callee,
                        arguments,
                        Some(SemanticCallDestinationV1::new(
                            place(local, ty),
                            edge(SemanticEdgeRoleV1::CallReturn, target),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            )
        };
        let mut blocks = old.blocks().to_vec();
        let first = blocks.len() as u32;
        let Terminator::Call(length_call) = blocks[0].terminator().kind() else {
            panic!();
        };
        let resume = length_call.destination().unwrap().edge().target().index();
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            source,
            blocks[0].statements().to_vec(),
            invoke(
                length_call.callee(),
                length_call.arguments().to_vec(),
                8,
                raw,
                first,
            ),
        )
        .unwrap();
        let block = |offset: u32, statements, end| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([248 + offset as u8; 32]),
                source,
                statements,
                end,
            )
            .unwrap()
        };
        blocks.push(block(
            0,
            vec![
                SemanticStatementV1::new(
                    source,
                    Statement::StorageDead(SemanticLocalIdV1::from_index(6)),
                ),
                assign(
                    place(9, mutable),
                    SemanticRvalueV1::new(
                        mutable,
                        Rvalue::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: place(4, descriptor),
                        },
                    ),
                ),
            ],
            invoke(issue, vec![], 10, witness, first + 1),
        ));
        if disjoint {
            blocks.push(block(
                1,
                vec![],
                invoke(
                    SemanticCallableIdV1::from_index(issue.index() + 1),
                    vec![Operand::Move(place(10, witness))],
                    15,
                    owned_witness,
                    first + 2,
                ),
            ));
        }
        blocks.push(block(
            1 + extra,
            vec![],
            invoke(
                get,
                vec![
                    Operand::Move(place(9, mutable)),
                    Operand::Move(place(if disjoint { 15 } else { 10 }, owned_witness)),
                ],
                11,
                option,
                first + 2 + extra,
            ),
        ));
        blocks.push(block(
            2 + extra,
            vec![assign(
                place(12, word),
                SemanticRvalueV1::new(word, Rvalue::Discriminant(place(11, option))),
            )],
            SemanticTerminatorV1::new(
                source,
                Terminator::SwitchInt {
                    discriminant: Operand::Copy(place(12, word)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, first + 3 + extra),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, resume),
                    )
                    .unwrap(),
                },
            ),
        ));
        let payload = Place::new(
            SemanticLocalIdV1::from_index(11),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), option).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), reference).unwrap(),
            ],
            reference,
        )
        .unwrap();
        let deref = Place::new(
            SemanticLocalIdV1::from_index(13),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, word).unwrap()],
            word,
        )
        .unwrap();
        blocks.push(block(
            3 + extra,
            vec![
                assign(
                    place(13, reference),
                    SemanticRvalueV1::new(reference, Rvalue::Use(Operand::Move(payload))),
                ),
                assign(
                    place(14, word),
                    SemanticRvalueV1::new(word, Rvalue::Use(Operand::Copy(deref))),
                ),
                SemanticStatementV1::new(
                    source,
                    Statement::StorageDead(SemanticLocalIdV1::from_index(13)),
                ),
                SemanticStatementV1::new(
                    source,
                    Statement::StorageDead(SemanticLocalIdV1::from_index(11)),
                ),
            ],
            SemanticTerminatorV1::new(
                source,
                Terminator::Goto(edge(SemanticEdgeRoleV1::Goto, resume)),
            ),
        ));
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

fn run_indexed(
    work: usize,
    storage: usize,
    fault: Option<u8>,
    disjoint: bool,
) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(0);
    let attacked = std::cell::Cell::new(false);
    let result = super::super::super::super::invocations::tests::run_captured_callable_transform(
        work,
        storage,
        if disjoint {
            disjoint_indexed_fixture
        } else {
            indexed_fixture
        },
        capture,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                let paired = super::super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                for root in 0..2 {
                    let row = plan.instance(root, 0, out)?;
                    let semantic = slots
                        .correspondence(out)?
                        .source(out.budget)?
                        .source_semantic(out.budget)?;
                    let function = &semantic.functions()[row.function.index() as usize];
                    let indexed = function.blocks().len() - 3;
                    assert!(program.in_place_call(root, 0, indexed, out)?);
                    let relation = slots.correspondence(out)?;
                    let archive = relation.source(out.budget)?.source_ssa(out.budget)?;
                    let definitions = archive
                        .plan_for_function(row.function)
                        .unwrap()
                        .plan()
                        .edge_definitions(fe2o3_mir_model::SsaEdgeIdV1::new(
                            fe2o3_mir_model::SsaBlockIdV1::new(indexed as u32),
                            0,
                        ))
                        .unwrap();
                    let [definition] = definitions else {
                        panic!("one exact original Option definition");
                    };
                    assert_eq!(definition.variable().get(), 11);
                    let endpoint =
                        relation.ssa_typed_endpoint_v36(root, 0, definition.value(), out.budget)?;
                    assert_eq!(
                        endpoint.source_type(out.budget)?,
                        function.locals()[11].ty()
                    );
                    assert_eq!(endpoint.source_local(out.budget)?.index(), 11);
                    let presence = endpoint
                        .enum_pointer_presence_v52(out.budget)?
                        .expect("checked presence locator");
                    assert_eq!(
                        presence.physical_type(out.budget)?,
                        Some(&fe2o3_kernel_ir::Type::BOOL)
                    );
                    assert_eq!(
                        presence.source_type(out.budget)?,
                        function.locals()[12].ty()
                    );
                    assert_eq!(endpoint.enum_variant_fields_v47(0, out.budget)?, Some(0));
                    assert_eq!(endpoint.enum_variant_fields_v47(1, out.budget)?, Some(1));
                    let payload = endpoint.enum_field_v47(1, 0, out.budget)?;
                    assert_eq!(payload.source_type(out.budget)?, function.locals()[13].ty());
                    assert!(
                        payload.reference(out.budget)?.is_none(),
                        "locator does not issue an ordinary loan"
                    );
                    reached.set(reached.get() + 1);
                    if root == 1 {
                        if let Some(fault) = fault {
                            let denied = match fault {
                                0 => endpoint.enum_discriminant_v47(out.budget).map(|_| ()),
                                1 => endpoint.enum_field_v47(0, 0, out.budget).map(|_| ()),
                                2 => {
                                    let mut foreign_work =
                                        fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                                    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
                                    foreign.reserve_storage(out.budget.storage())?;
                                    let before =
                                        (foreign.work(), foreign.storage(), foreign.peak_storage());
                                    let denied = endpoint
                                        .enum_pointer_presence_v52(&mut foreign)
                                        .map(|_| ());
                                    assert_eq!(
                                        (foreign.work(), foreign.storage(), foreign.peak_storage()),
                                        before
                                    );
                                    denied
                                }
                                _ => unreachable!(),
                            }
                            .expect_err("exact presence/variant/custody attack");
                            match fault {
                                0 => assert!(matches!(
                                    denied,
                                    SourceError::Binding("SSA carrier has no enum discriminant")
                                )),
                                1 => assert!(matches!(
                                    denied,
                                    SourceError::Binding("original enum field is out of range")
                                )),
                                2 => assert!(matches!(
                                    denied,
                                    SourceError::Resource(Resource::Accounting)
                                )),
                                _ => unreachable!(),
                            }
                            let before = (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                            );
                            assert!(endpoint.enum_pointer_presence_v52(out.budget).is_err());
                            assert_eq!(
                                (
                                    out.budget.work(),
                                    out.budget.storage(),
                                    out.budget.peak_storage()
                                ),
                                before
                            );
                            attacked.set(true);
                            return Ok(());
                        }
                    }
                }
                program.emit(out)?;
                paired.emit(out)?;
                assert!(
                    out.text
                        .contains("invocation_source_descriptor_index_v52(cursor.source")
                );
                assert!(
                    out.text
                        .contains("invocation_source_enum_field_current_v47(consumed, ty, value")
                );
                Ok(())
            })
        },
    );
    if fault.is_some() {
        assert_eq!(reached.get(), 2);
        assert!(attacked.get());
        assert!(
            result.0.is_err(),
            "sticky query refusal must reach the owner boundary"
        );
    } else if result.0.is_ok() {
        assert_eq!(reached.get(), 2);
    }
    result
}

#[test]
fn descriptor_indexed_option_uses_real_intrinsic_some_edge_and_complete_source_pair() {
    for disjoint in [false, true] {
        run_indexed(LIMIT, LIMIT, None, disjoint).0.unwrap();
    }
}

#[test]
fn descriptor_indexed_presence_refuses_discriminant_reinterpretation_absent_field_and_foreign_owner()
 {
    for disjoint in [false, true] {
        run_indexed(LIMIT, LIMIT, None, disjoint).0.unwrap();
        for fault in 0..3 {
            assert!(run_indexed(LIMIT, LIMIT, Some(fault), disjoint).0.is_err());
        }
    }
}

#[test]
fn descriptor_indexed_complete_source_route_has_exact_and_one_short_resources() {
    for disjoint in [false, true] {
        let generous = run_indexed(LIMIT, LIMIT, None, disjoint);
        generous.0.unwrap();
        let exact = run_indexed(generous.1, generous.3, None, disjoint);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (generous.1, generous.2, generous.3)
        );
        for work in [false, true] {
            let short = run_indexed(
                generous.1 - usize::from(work),
                generous.3 - usize::from(!work),
                None,
                disjoint,
            );
            if work {
                assert!(
                    matches!(short.0, Err(Error::Source(SourceError::Resource(Resource::Work(error)))) if error.actual() == generous.1 && error.limit() == generous.1 - 1)
                );
            } else {
                assert!(
                    matches!(short.0, Err(Error::Source(SourceError::Resource(Resource::Storage(error)))) if error.actual() == generous.3 && error.limit() == generous.3 - 1)
                );
            }
        }
    }
}
