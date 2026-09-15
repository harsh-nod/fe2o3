mod numerical_policy_projection_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticExecutionCapabilityContractV1, SemanticExecutionCapabilityOperationV1,
        SemanticExecutionCapabilitySignatureV1,
    };

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        None,
        Provenance(usize),
        Source,
        PolicyIsContext,
        WrongReference,
        WrongResult,
        ExtraArgument,
        MissingIssue,
        DuplicateIssue,
        ForgedContext,
        MutableBorrow,
        RawAddress,
        MovedBorrow,
    }

    fn fixture(
        mutation: Mutation,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
    ) {
        let (mut types, mut callables, original) = capability_index_fixture();
        let capability = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(231)),
            SemanticLayoutIdentityV1::from_sha256(bytes(232)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ));
        let contract = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                context: CAP_INDEX_CONTEXT_BORROW,
                capability,
                policy: if matches!(mutation, Mutation::PolicyIsContext) {
                    types[CAP_INDEX_CONTEXT.index() as usize].identity()
                } else {
                    SemanticTypeIdentityV1::from_sha256(bytes(233))
                },
            },
            SemanticExecutionCapabilitySignatureV1::new(&[CAP_INDEX_CONTEXT_BORROW], capability)
                .unwrap(),
            capability_index_provenance(match mutation {
                Mutation::Provenance(axis) => Some(axis),
                _ => None,
            }),
            SemanticFunctionIdentityV1::from_sha256(bytes(
                if matches!(mutation, Mutation::Source) {
                    234
                } else {
                    116
                },
            )),
        )
        .unwrap();
        let callee = SemanticCallableIdV1::from_index(callables.len() as u32);
        callables.push(capability_index_callable(
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            &[(
                CAP_INDEX_CONTEXT_BORROW,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
            )],
            capability,
        ));
        let mut locals = original.locals().to_vec();
        assert_eq!(locals.len(), 12);
        for (tag, ty) in [
            (235, capability),
            (236, CAP_INDEX_CONTEXT_BORROW),
            (237, CAP_INDEX_CONTEXT),
        ] {
            locals.push(local(tag, ty, SemanticLocalRoleV1::Temporary));
        }
        let mut blocks = original.blocks().to_vec();
        let added_block = u32::try_from(blocks.len()).unwrap();
        let context_issue = |target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        typed_place(1, CAP_INDEX_CONTEXT),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        blocks[0] = block(
            160,
            vec![],
            if matches!(mutation, Mutation::MissingIssue) {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, added_block))
            } else {
                context_issue(added_block)
            },
        );
        let place = typed_place(
            if matches!(mutation, Mutation::ForgedContext) {
                14
            } else {
                1
            },
            CAP_INDEX_CONTEXT,
        );
        let borrow = match mutation {
            Mutation::RawAddress => SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place,
            },
            _ => SemanticRvalueKindV1::Borrow {
                kind: if matches!(mutation, Mutation::MutableBorrow) {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place,
            },
        };
        let receiver = if matches!(mutation, Mutation::WrongReference) {
            typed_operand(1, CAP_INDEX_CONTEXT)
        } else if matches!(mutation, Mutation::MovedBorrow) {
            SemanticOperandV1::Move(typed_place(13, CAP_INDEX_CONTEXT_BORROW))
        } else {
            typed_operand(13, CAP_INDEX_CONTEXT_BORROW)
        };
        let mut arguments = vec![receiver];
        if matches!(mutation, Mutation::ExtraArgument) {
            arguments.push(typed_operand(13, CAP_INDEX_CONTEXT_BORROW));
        }
        blocks.push(block(
            238,
            vec![typed_assignment(13, CAP_INDEX_CONTEXT_BORROW, borrow)],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    callee,
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        typed_place(
                            12,
                            if matches!(mutation, Mutation::WrongResult) {
                                CAP_INDEX_CONTEXT
                            } else {
                                capability
                            },
                        ),
                        cfg_edge(
                            SemanticEdgeRoleV1::CallReturn,
                            if matches!(mutation, Mutation::DuplicateIssue) {
                                added_block + 1
                            } else {
                                1
                            },
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ));
        if matches!(mutation, Mutation::DuplicateIssue) {
            blocks.push(block(239, vec![], context_issue(1)));
        }
        (
            types,
            callables,
            projection_function_with_locals(blocks, locals),
        )
    }

    fn validate(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Result<
        (IntrinsicProjectionV1, Vec<ProductionRankedOperationV1>),
        ProductionRankedProjectionErrorV1,
    > {
        let provenance = capability_index_provenance(None);
        numerical_policy_v1::validate_roots(
            types,
            callables,
            function,
            provenance.root(),
            provenance.kernel_binding(),
        )?;
        project_capability_index_fixture(types, callables, function)
    }

    include!("context_borrow_tests.rs");

    #[test]
    fn numerical_policy_ranked_projection_preserves_semantics_and_adds_no_memory_authority() {
        let (original_types, original_callables, original) = capability_index_fixture();
        let (_, original_operations) =
            project_capability_index_fixture(&original_types, &original_callables, &original)
                .unwrap();
        for mutation in [Mutation::None, Mutation::MovedBorrow] {
            let (types, callables, function) = fixture(mutation);
            let retained = (callables.clone(), function.clone());
            let (projected, operations) = validate(&types, &callables, &function).unwrap();
            assert_eq!(operations, original_operations);
            assert_eq!((callables, function), retained);
            assert!(projected.tensor_layouts.iter().all(Option::is_none));
            assert!(
                projected
                    .capability_read_effects
                    .iter()
                    .all(Option::is_none)
            );
            assert!(projected.direct_write_effects.iter().all(Option::is_none));
        }
    }

    #[test]
    fn numerical_policy_ranked_projection_rejects_substitution_and_unissued_borrows() {
        for mutation in [
            Mutation::Source,
            Mutation::PolicyIsContext,
            Mutation::WrongReference,
            Mutation::WrongResult,
            Mutation::ExtraArgument,
            Mutation::MissingIssue,
            Mutation::DuplicateIssue,
            Mutation::ForgedContext,
            Mutation::MutableBorrow,
            Mutation::RawAddress,
        ]
        .into_iter()
        .chain((0..7).map(Mutation::Provenance))
        {
            let (types, callables, function) = fixture(mutation);
            assert!(
                validate(&types, &callables, &function).is_err(),
                "{mutation:?}"
            );
        }
    }
}
