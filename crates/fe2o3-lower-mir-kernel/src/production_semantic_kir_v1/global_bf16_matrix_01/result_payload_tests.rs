mod result_payload_tests {
    use super::*;

    mod guard_selection_tests124 {
        use super::*;
        include!("guard_selection_tests124.rs");
    }

    #[derive(Clone, Copy, Debug)]
    enum Change {
        None,
        Otherwise,
        MovedError,
        NonOrdinal,
        ForwardedWholeResult,
        SameOwnerOk,
        WrongTag,
        StaleTag,
        NoGuard,
        Bypass,
        SharedTarget,
        WrongVariant,
        DifferentOk,
        DeadGlobal,
        AmbiguousOtherwise,
        UnknownValue,
        OnlyErr,
        CyclicResult,
        CyclicLoan,
    }

    fn with_result_join(role: SemanticMfmaOperandRoleV1, change: Change) -> Fixture {
        let mut f = source_fixture(role, Mutation::None);
        let old_enum = &f.types[15];
        f.types[15] = SemanticTypeDeclV1::new(
            old_enum.identity(),
            old_enum.layout_identity(),
            old_enum.layout().clone(),
            SemanticTypeShapeV1::Enum {
                discriminant: id(3),
                variants: vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![id(8)]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![id(3)]).unwrap(),
                    ),
                ]
                .into_boxed_slice(),
            },
        );
        let source = SemanticSourceProvenanceV1::unavailable();
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], id(ty)).unwrap()
        };
        let edge = |target, role| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let assign = |destination: SemanticPlaceV1, kind| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination.clone(),
                    SemanticRvalueV1::new(destination.ty(), kind),
                )),
            )
        };
        let block = |index: u8, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([180 + index; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let goto = |target| SemanticTerminatorKindV1::Goto(edge(target, SemanticEdgeRoleV1::Goto));
        let construct = |local, variant, matrix| {
            assign(
                place(local, 15),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(variant),
                        vec![if variant == 0 {
                            SemanticOperandV1::Move(place(matrix, 8))
                        } else if matches!(change, Change::MovedError) {
                            SemanticOperandV1::Move(place(16, 3))
                        } else {
                            SemanticOperandV1::Copy(place(16, 3))
                        }],
                    )
                    .unwrap(),
                ),
            )
        };
        let original = f.function.blocks();
        let mut prefix = original[1].statements()[..2].to_vec();
        let SemanticStatementKindV1::Assign(matrix) = original[1].statements()[1].kind() else {
            unreachable!()
        };
        prefix.push(assign(place(12, 8), matrix.value().kind().clone()));
        if matches!(change, Change::WrongTag | Change::StaleTag) {
            prefix.push(construct(
                14,
                u32::from(matches!(change, Change::StaleTag)),
                12,
            ));
        }
        let mut guard = vec![assign(
            place(13, 3),
            SemanticRvalueKindV1::Discriminant(place(
                if matches!(change, Change::WrongTag | Change::ForwardedWholeResult) {
                    14
                } else {
                    10
                },
                15,
            )),
        )];
        if matches!(change, Change::ForwardedWholeResult) {
            guard.insert(
                0,
                assign(
                    place(14, 15),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(10, 15))),
                ),
            );
        }
        if matches!(change, Change::StaleTag) {
            guard.push(assign(
                place(10, 15),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(14, 15))),
            ));
        }
        let mut consumer = original[2].statements().to_vec();
        if matches!(change, Change::ForwardedWholeResult) {
            consumer[0] = assign(
                place(11, 8),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(14),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Downcast(0),
                                id(15),
                            )
                            .unwrap(),
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(8))
                                .unwrap(),
                        ],
                        id(8),
                    )
                    .unwrap(),
                )),
            );
        }
        if matches!(change, Change::DeadGlobal) {
            consumer.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(6)),
            ));
        }
        f.call = SemanticDirectCallV1::new_callable(
            f.call.callee(),
            f.call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                place(5, 12),
                edge(6, SemanticEdgeRoleV1::CallReturn),
            )),
            f.call.unwind(),
        )
        .unwrap();
        let otherwise = matches!(change, Change::Otherwise | Change::AmbiguousOtherwise);
        let through_cycle = matches!(change, Change::CyclicResult | Change::CyclicLoan);
        let branches = SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                if matches!(change, Change::NonOrdinal) {
                    17
                } else {
                    u128::from(otherwise || matches!(change, Change::WrongVariant))
                },
                edge(
                    if otherwise {
                        6
                    } else if through_cycle {
                        7
                    } else {
                        5
                    },
                    SemanticEdgeRoleV1::SwitchValue,
                ),
            )],
            edge(
                if otherwise || matches!(change, Change::SharedTarget) {
                    5
                } else {
                    6
                },
                SemanticEdgeRoleV1::SwitchOtherwise,
            ),
        )
        .unwrap();
        let mut blocks = vec![
            original[0].clone(),
            block(
                1,
                prefix,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, 2)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(2, SemanticEdgeRoleV1::SwitchValue),
                        )],
                        edge(3, SemanticEdgeRoleV1::SwitchOtherwise),
                    )
                    .unwrap(),
                },
            ),
            block(
                2,
                vec![construct(
                    10,
                    u32::from(matches!(change, Change::OnlyErr)),
                    8,
                )],
                goto(4),
            ),
            block(
                3,
                vec![if matches!(change, Change::UnknownValue) {
                    assign(
                        place(10, 15),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(15, 15))),
                    )
                } else {
                    construct(
                        10,
                        u32::from(!matches!(change, Change::DifferentOk | Change::SameOwnerOk)),
                        if matches!(change, Change::DifferentOk) {
                            12
                        } else {
                            8
                        },
                    )
                }],
                goto(if matches!(change, Change::Bypass) {
                    5
                } else {
                    4
                }),
            ),
            block(
                4,
                guard,
                if matches!(change, Change::NoGuard) {
                    goto(5)
                } else {
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(13, 3)),
                        targets: branches,
                    }
                },
            ),
            block(5, consumer, SemanticTerminatorKindV1::Call(f.call.clone())),
            block(6, vec![], SemanticTerminatorKindV1::Return),
        ];
        if through_cycle {
            blocks.push(block(
                7,
                if matches!(change, Change::CyclicResult) {
                    vec![assign(
                        place(10, 15),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(10, 15))),
                    )]
                } else {
                    vec![]
                },
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, 2)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(5, SemanticEdgeRoleV1::SwitchValue),
                        )],
                        edge(
                            if matches!(change, Change::CyclicResult) {
                                4
                            } else {
                                7
                            },
                            SemanticEdgeRoleV1::SwitchOtherwise,
                        ),
                    )
                    .unwrap(),
                },
            ));
        }
        if matches!(change, Change::NonOrdinal) {
            let original = &f.types[15];
            f.types[15] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::Enum {
                    discriminant: id(3),
                    variants: vec![(17, 8), (83, 3)]
                        .into_iter()
                        .map(|(tag, payload)| {
                            SemanticEnumVariantV1::new(
                                tag,
                                SemanticAggregateTypeV1::new(vec![id(payload)]).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            );
        }
        if matches!(change, Change::AmbiguousOtherwise) {
            let original = &f.types[15];
            let SemanticTypeShapeV1::Enum {
                discriminant,
                variants,
            } = original.shape()
            else {
                unreachable!()
            };
            let mut variants = variants.to_vec();
            variants.push(SemanticEnumVariantV1::new(
                2,
                SemanticAggregateTypeV1::new(vec![id(8)]).unwrap(),
            ));
            f.types[15] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::Enum {
                    discriminant: *discriminant,
                    variants: variants.into_boxed_slice(),
                },
            );
        }
        let mut locals = f.function.locals().to_vec();
        for (index, ty) in [(13, 3), (14, 15), (15, 15), (16, 3)] {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([120 + index; 32]),
                id(ty),
                if index == 15 && matches!(change, Change::UnknownValue) {
                    SemanticLocalRoleV1::Argument(4)
                } else if index == 16 {
                    SemanticLocalRoleV1::Argument(if matches!(change, Change::UnknownValue) {
                        5
                    } else {
                        4
                    })
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            ));
        }
        let old = &f.function;
        f.function = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap();
        f
    }

    fn capture(
        f: &Fixture,
        plan: &fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1,
        limit: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), limit)?;
        global_bf16_live_v1::capture_component(&mut graph, &f.types, &f.callables, &f.context, 5)
            .map(|_| ())
    }

    #[test]
    fn global_bf16_guarded_result_all_incoming_explicit_and_otherwise() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            for change in [
                Change::None,
                Change::Otherwise,
                Change::MovedError,
                Change::NonOrdinal,
                Change::ForwardedWholeResult,
                Change::SameOwnerOk,
            ] {
                let f = with_result_join(role, change);
                let plan = plan(&f);
                assert!(plan.plan().is_reachable(SsaBlockIdV1::new(2)));
                assert!(plan.plan().is_reachable(SsaBlockIdV1::new(3)));
                let mut graph =
                    CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
                let incoming = graph.incoming(4, 10).unwrap();
                assert_eq!(incoming.len(), 2);
                assert_ne!(incoming[0], incoming[1]);
                let captured = global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    5,
                )
                .unwrap();
                assert_eq!(
                    (
                        captured.construction().block,
                        captured.construction().statement
                    ),
                    (1, Some(1))
                );
                assert_eq!(captured.global_bind().block, 0);
            }
        }
    }

    #[test]
    fn global_bf16_guarded_result_rejects_wrong_tag_stale_version_and_unproved_edges() {
        for change in [
            Change::WrongTag,
            Change::StaleTag,
            Change::NoGuard,
            Change::Bypass,
            Change::SharedTarget,
            Change::WrongVariant,
            Change::AmbiguousOtherwise,
        ] {
            let f = with_result_join(SemanticMfmaOperandRoleV1::A, change);
            let plan = plan(&f);
            assert!(
                matches!(
                    capture(&f, &plan, 65_536),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "global BF16 Result needs variant-sensitive payload SSA custody",
                        ..
                    })
                ),
                "{change:?}"
            );
        }
    }

    #[test]
    fn global_bf16_guarded_result_keeps_conflicting_payload_owner_rejection() {
        let f = with_result_join(SemanticMfmaOperandRoleV1::B, Change::DifferentOk);
        let plan = plan(&f);
        assert!(matches!(
            capture(&f, &plan, 65_536),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "global BF16 capture merges different Global issuers",
                ..
            })
        ));
    }

    #[test]
    fn global_bf16_guarded_result_does_not_restore_a_dead_global_loan() {
        let f = with_result_join(SemanticMfmaOperandRoleV1::A, Change::DeadGlobal);
        let plan = plan(&f);
        assert!(matches!(
            capture(&f, &plan, 65_536),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                ..
            })
        ));
    }

    #[test]
    fn global_bf16_guarded_result_rejects_unknown_enum_origin() {
        let f = with_result_join(SemanticMfmaOperandRoleV1::A, Change::UnknownValue);
        let plan = plan(&f);
        assert!(
            plan.plan()
                .entry_definitions()
                .iter()
                .any(|definition| definition.variable().get() == 15)
        );
        assert!(matches!(
            capture(&f, &plan, 65_536),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability authority originates from an entry parameter or missing definition",
                ..
            })
        ));
    }

    #[test]
    fn global_bf16_guarded_result_requires_a_real_matching_payload() {
        let f = with_result_join(SemanticMfmaOperandRoleV1::B, Change::OnlyErr);
        let plan = plan(&f);
        assert!(matches!(
            capture(&f, &plan, 65_536),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "global BF16 guarded Result has no matching payload origin",
                ..
            })
        ));
    }

    #[test]
    fn global_bf16_guarded_result_rejects_cyclic_ssa_custody_and_loan_generations() {
        for (change, expected) in [
            (
                Change::CyclicResult,
                "global BF16 capture has cyclic or excessive SSA custody",
            ),
            (
                Change::CyclicLoan,
                "capability loan crosses a cycle without a proven storage generation",
            ),
        ] {
            let f = with_result_join(SemanticMfmaOperandRoleV1::A, change);
            let plan = plan(&f);
            assert!(plan.plan().is_reachable(SsaBlockIdV1::new(7)));
            assert!(
                matches!(capture(&f, &plan, 65_536), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == expected),
                "{change:?}"
            );
        }
    }

    #[test]
    fn global_bf16_guarded_result_uses_one_existing_work_ceiling() {
        let f = with_result_join(SemanticMfmaOperandRoleV1::A, Change::None);
        let plan = plan(&f);
        let (mut low, mut high) = (0, 65_536);
        capture(&f, &plan, high).unwrap();
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            match capture(&f, &plan, middle) {
                Ok(()) => high = middle,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    ..
                }) => low = middle,
                other => panic!("only the original work ceiling may reject: {other:?}"),
            }
        }
        capture(&f, &plan, high).unwrap();
        assert!(
            matches!(capture(&f, &plan, high - 1), Err(ProductionSemanticKirErrorV1::ResourceLimit { resource: ProductionSemanticKirResourceV1::AnalysisWork, limit, .. }) if limit == high - 1)
        );
    }
}
