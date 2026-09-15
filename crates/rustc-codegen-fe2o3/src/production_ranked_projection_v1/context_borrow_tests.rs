// Included inside numerical_policy_projection_tests to reuse its closed issuer
// and policy-call fixture. These are projection components, not source receipts.
mod ordered_context_transport {
    use super::*;
    use context_borrow_v1::Borrow;

    #[derive(Clone, Copy, Debug)]
    enum Change {
        None,
        UnissuedOwner,
        RawPointer,
        DeadParameter,
        MovedParameter,
        DeadReceiver,
    }

    fn origin(borrow: Borrow) -> ProjectedCapabilityValueV1 {
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext {
            context: CAP_INDEX_CONTEXT,
            borrow,
        })
    }

    fn dereference(local: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, CAP_INDEX_CONTEXT)
                    .unwrap(),
            ],
            CAP_INDEX_CONTEXT,
        )
        .unwrap()
    }

    fn ordered_fixture(
        change: Change,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
        usize,
    ) {
        let (mut types, callables, original) = fixture(Mutation::None);
        let reference = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(241)),
            SemanticLayoutIdentityV1::from_sha256(bytes(242)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    CAP_INDEX_CONTEXT,
                    if matches!(change, Change::RawPointer) {
                        SemanticPointerKindV1::Raw
                    } else {
                        SemanticPointerKindV1::Reference
                    },
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        let mut locals = original.locals().to_vec();
        assert_eq!(locals.len(), 15);
        for tag in [243, 244, 245] {
            locals.push(local(tag, reference, SemanticLocalRoleV1::Temporary));
        }
        let mut blocks = original.blocks().to_vec();
        let borrow_block = blocks.len() - 1;
        let parameter_block = blocks.len();
        let receiver_block = parameter_block + 1;
        let policy_call = blocks[borrow_block].terminator().kind().clone();
        let source = typed_place(
            if matches!(change, Change::UnissuedOwner) {
                14
            } else {
                1
            },
            CAP_INDEX_CONTEXT,
        );
        let borrow = if matches!(change, Change::RawPointer) {
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: source,
            }
        } else {
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: source,
            }
        };
        blocks[borrow_block] = block(
            238,
            vec![typed_assignment(15, reference, borrow)],
            SemanticTerminatorKindV1::Goto(cfg_edge(
                SemanticEdgeRoleV1::Goto,
                parameter_block as u32,
            )),
        );
        let mut parameter = vec![typed_assignment(
            16,
            reference,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(15, reference))),
        )];
        match change {
            Change::DeadParameter => parameter.push(statement(
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(16)),
            )),
            Change::MovedParameter => parameter.push(typed_assignment(
                17,
                reference,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(16, reference))),
            )),
            _ => {}
        }
        blocks.push(block(
            239,
            parameter,
            SemanticTerminatorKindV1::Goto(cfg_edge(
                SemanticEdgeRoleV1::Goto,
                receiver_block as u32,
            )),
        ));
        let mut receiver = vec![typed_assignment(
            13,
            CAP_INDEX_CONTEXT_BORROW,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: dereference(16),
            },
        )];
        if matches!(change, Change::DeadReceiver) {
            receiver.push(statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(13),
            )));
        }
        blocks.push(block(240, receiver, policy_call));
        (
            types,
            callables,
            projection_function_with_locals(blocks, locals),
            borrow_block,
        )
    }

    #[test]
    fn context_policy_cross_block_exclusive_parameter_reborrows_shared() {
        let (types, callables, function, _) = ordered_fixture(Change::None);
        let retained = (callables.clone(), function.clone());
        let (_, original_operations) = {
            let (types, callables, function) = fixture(Mutation::None);
            validate(&types, &callables, &function).unwrap()
        };
        let (projected, operations) = validate(&types, &callables, &function).unwrap();
        assert_eq!(operations, original_operations);
        assert_eq!((callables, function), retained);
        assert!(
            projected
                .capability_read_effects
                .iter()
                .all(Option::is_none)
        );
        assert!(projected.direct_write_effects.iter().all(Option::is_none));
    }

    #[test]
    fn context_policy_lost_origins_and_raw_pointers_cannot_reborrow_authority() {
        for change in [
            Change::UnissuedOwner,
            Change::RawPointer,
            Change::DeadParameter,
            Change::MovedParameter,
            Change::DeadReceiver,
        ] {
            let (types, callables, function, _) = ordered_fixture(change);
            let error = validate(&types, &callables, &function).err();
            assert!(
                matches!(
                    error,
                    Some(ProductionRankedProjectionErrorV1::Incomplete(
                        "numerical-policy receiver lacks the dominating retained shared KernelContext origin"
                    ))
                ),
                "{change:?}: {error:?}"
            );
        }
    }

    #[test]
    fn context_policy_reborrow_kind_pointee_and_projection_are_exact() {
        for current in [Borrow::Owned, Borrow::Shared, Borrow::Exclusive] {
            let state = HashMap::from([(1, origin(current))]);
            let place = if current == Borrow::Owned {
                typed_place(1, CAP_INDEX_CONTEXT)
            } else {
                dereference(1)
            };
            for kind in [
                SemanticBorrowKindV1::Shared,
                SemanticBorrowKindV1::Mutable,
                SemanticBorrowKindV1::Fake,
            ] {
                let expected = match (current, kind) {
                    (_, SemanticBorrowKindV1::Shared) => Some(origin(Borrow::Shared)),
                    (Borrow::Owned | Borrow::Exclusive, SemanticBorrowKindV1::Mutable) => {
                        Some(origin(Borrow::Exclusive))
                    }
                    _ => None,
                };
                assert_eq!(capability_borrow_origin_v1(&state, &place, kind), expected);
            }
            let wrong_shape = if current == Borrow::Owned {
                dereference(1)
            } else {
                typed_place(1, CAP_INDEX_CONTEXT)
            };
            assert_eq!(
                capability_borrow_origin_v1(&state, &wrong_shape, SemanticBorrowKindV1::Shared),
                None
            );
            for projections in [
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(0),
                        CAP_INDEX_CONTEXT,
                    )
                    .unwrap(),
                ],
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Dereference,
                        CAP_INDEX_CONTEXT
                    )
                    .unwrap();
                    2
                ],
            ] {
                let place = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    projections,
                    CAP_INDEX_CONTEXT,
                )
                .unwrap();
                assert_eq!(
                    capability_borrow_origin_v1(&state, &place, SemanticBorrowKindV1::Shared),
                    None
                );
            }
            let wrong_pointee = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                place
                    .projections()
                    .iter()
                    .map(|p| SemanticProjectionV1::new(p.kind().clone(), U64_TYPE).unwrap())
                    .collect(),
                U64_TYPE,
            )
            .unwrap();
            assert_eq!(
                capability_borrow_origin_v1(&state, &wrong_pointee, SemanticBorrowKindV1::Shared),
                None
            );
        }
    }

    #[test]
    fn context_policy_only_shared_copies_preserve_origin_and_meets_stay_exact() {
        let (types, _, function, _) = ordered_fixture(Change::None);
        for current in [Borrow::Owned, Borrow::Shared, Borrow::Exclusive] {
            let local = match current {
                Borrow::Owned => 1,
                Borrow::Shared => 13,
                Borrow::Exclusive => 16,
            };
            let ty = function.locals()[local].ty();
            for moved in [false, true] {
                let mut state = HashMap::from([(local, origin(current))]);
                let operand = if moved {
                    SemanticOperandV1::Move(typed_place(local as u32, ty))
                } else {
                    typed_operand(local as u32, ty)
                };
                consume_capability_operand_v1(&types, &function, &mut state, &operand);
                assert_eq!(
                    capability_known_origin_v1(&state, &operand).is_some(),
                    !moved && current == Borrow::Shared
                );
            }
            for other in [
                None,
                Some(Borrow::Owned),
                Some(Borrow::Shared),
                Some(Borrow::Exclusive),
            ] {
                let mut state = HashMap::from([(1, origin(current))]);
                let incoming =
                    other.map_or_else(HashMap::new, |other| HashMap::from([(1, origin(other))]));
                let _ = merge_capability_states_v1(&mut state, &incoming).unwrap();
                assert_eq!(
                    state.get(&1),
                    (other == Some(current)).then(|| origin(current)).as_ref()
                );
            }
        }
    }

    #[test]
    fn context_policy_receiver_requires_the_exact_shared_origin_not_exclusive() {
        let (types, callables, function) = fixture(Mutation::None);
        let SemanticTerminatorKindV1::Call(call) =
            function.blocks().last().unwrap().terminator().kind()
        else {
            panic!("fixture must retain the actual policy call");
        };
        for value in [
            None,
            Some(ProjectedCapabilityValueV1::Invalid),
            Some(origin(Borrow::Owned)),
            Some(origin(Borrow::Exclusive)),
            Some(ProjectedCapabilityValueV1::Known(
                ProjectedCapabilityOriginV1::KernelContext {
                    context: U64_TYPE,
                    borrow: Borrow::Shared,
                },
            )),
            Some(origin(Borrow::Shared)),
        ] {
            let state = value.clone().map_or_else(HashMap::new, |value| HashMap::from([(13, value)]));
            let error =
                numerical_policy_v1::validate_receiver(&types, &callables, call, &state).err();
            if value == Some(origin(Borrow::Shared)) {
                assert!(error.is_none(), "{error:?}");
            } else {
                assert!(
                    matches!(
                        error,
                        Some(ProductionRankedProjectionErrorV1::Incomplete(
                            "numerical-policy receiver lacks the dominating retained shared KernelContext origin"
                        ))
                    ),
                    "{value:?}: {error:?}"
                );
            }
        }
    }

    #[test]
    fn context_policy_transport_does_not_resurrect_after_storage_or_assignment_kills() {
        let (types, _, original, borrow_block) = ordered_fixture(Change::None);
        let parameter_block = borrow_block + 1;
        let receiver_block = borrow_block + 2;
        for kill in [
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(16),
            )),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(16),
            )),
            statement(SemanticStatementKindV1::Deinitialize(typed_place(
                16,
                original.locals()[16].ty(),
            ))),
            typed_assignment(
                16,
                original.locals()[16].ty(),
                SemanticRvalueKindV1::Use(typed_operand(17, original.locals()[17].ty())),
            ),
        ] {
            let mut blocks = original.blocks().to_vec();
            let mut statements = blocks[parameter_block].statements().to_vec();
            statements.push(kill);
            blocks[parameter_block] = block(
                239,
                statements,
                blocks[parameter_block].terminator().kind().clone(),
            );
            let function = projection_function_with_locals(blocks, original.locals().to_vec());
            let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
            // Synthetic component seed only; the integration positive above
            // obtains this origin from the original closed issuer call.
            let mut state = HashMap::from([(1, origin(Borrow::Owned))]);
            for block in [borrow_block, parameter_block, receiver_block] {
                transfer_capability_statements_v1(
                    &types, &function, block, &mut state, &dominance, None, None,
                )
                .unwrap();
            }
            assert_eq!(
                capability_known_origin_v1(&state, &typed_operand(13, CAP_INDEX_CONTEXT_BORROW)),
                None
            );
        }
    }
}
