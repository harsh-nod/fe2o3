// Component source/SSA fixtures: none constructs a production admission owner.
mod source_inputs_tests {
    use super::super::capability_ssa_graph_01::CapabilitySsaGraphV1;
    use super::*;

    include!("result_payload_tests.rs");
    include!("global_carrier_tests.rs");

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        None,
        DeadGlobal,
        DeadMatrix,
        WrongVariant,
        ForeignBind,
        ForeignRoot,
        WrongPhysical,
        SameOwnerMeet,
        DifferentMatrixMeet,
        SplitReferenceMeet,
    }

    fn source_fixture(role: SemanticMfmaOperandRoleV1, mutation: Mutation) -> Fixture {
        let mut f = fixture(role);
        let source = SemanticSourceProvenanceV1::unavailable();
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], id(ty)).unwrap()
        };
        let constant = |bits| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                id(2),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 8).unwrap()),
            ))
        };
        let zst = || {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                id(0),
                SemanticConstantValueV1::ZeroSized,
            ))
        };
        let assign = |destination: SemanticPlaceV1, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination.clone(),
                    SemanticRvalueV1::new(destination.ty(), value),
                )),
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let edge =
            |to, role| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(to));
        f.types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([35; 32]),
            SemanticLayoutIdentityV1::from_sha256([35; 32]),
            SemanticTypeLayoutV1::new(Some(48), 8).unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: id(3),
                variants: vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![id(8)]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![id(8)]).unwrap(),
                    ),
                ]
                .into_boxed_slice(),
            },
        ));
        let bind = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![
                zst(),
                if matches!(mutation, Mutation::WrongPhysical) {
                    constant(0)
                } else {
                    SemanticOperandV1::Copy(place(9, 5))
                },
            ],
            Some(SemanticCallDestinationV1::new(
                place(6, 6),
                edge(1, SemanticEdgeRoleV1::CallReturn),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let mut fields = vec![SemanticOperandV1::Copy(place(7, 7))];
        // Distinct coordinates test exact source carriage, not merely equal types.
        fields.extend([
            constant(7),
            SemanticOperandV1::Copy(place(3, 2)),
            SemanticOperandV1::Copy(place(4, 2)),
            constant(31),
        ]);
        fields.extend([zst(), zst(), zst()]);
        let statements = vec![
            assign(
                place(7, 7),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(6, 6),
                },
            ),
            assign(
                place(8, 8),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Aggregate,
                        fields.clone(),
                    )
                    .unwrap(),
                ),
            ),
            assign(
                place(10, 15),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(0),
                        vec![SemanticOperandV1::Move(place(8, 8))],
                    )
                    .unwrap(),
                ),
            ),
        ];
        let payload = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(10),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Downcast(u32::from(matches!(
                        mutation,
                        Mutation::WrongVariant
                    ))),
                    id(15),
                )
                .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(8)).unwrap(),
            ],
            id(8),
        )
        .unwrap();
        let payload_move = assign(
            place(11, 8),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)),
        );
        let borrow = || {
            assign(
                place(1, 9),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(11, 8),
                },
            )
        };
        let mut consumer_statements = vec![payload_move, borrow()];
        match mutation {
            Mutation::DeadGlobal | Mutation::DeadMatrix => {
                consumer_statements.push(SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(
                        if matches!(mutation, Mutation::DeadGlobal) {
                            6
                        } else {
                            11
                        },
                    )),
                ))
            }
            _ => {}
        }
        let meet = matches!(
            mutation,
            Mutation::SameOwnerMeet | Mutation::DifferentMatrixMeet | Mutation::SplitReferenceMeet
        );
        let call_block = if meet { 5 } else { 2 };
        f.call = SemanticDirectCallV1::new_callable(
            f.call.callee(),
            f.call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                place(5, 12),
                edge(call_block + 1, SemanticEdgeRoleV1::CallReturn),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let mut blocks = vec![
            block(90, vec![], SemanticTerminatorKindV1::Call(bind)),
            block(
                91,
                statements,
                SemanticTerminatorKindV1::Goto(edge(2, SemanticEdgeRoleV1::Goto)),
            ),
        ];
        if meet {
            let split_reference = matches!(mutation, Mutation::SplitReferenceMeet);
            let transfer = || {
                assign(
                    place(12, 8),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(11, 8))),
                )
            };
            // Both paths remain reachable. No branch is pruned to force equality.
            blocks.push(block(
                92,
                vec![consumer_statements.remove(0)],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, 2)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(3, SemanticEdgeRoleV1::SwitchValue),
                        )],
                        edge(4, SemanticEdgeRoleV1::SwitchOtherwise),
                    )
                    .unwrap(),
                },
            ));
            blocks.push(block(
                93,
                vec![if split_reference {
                    borrow()
                } else {
                    transfer()
                }],
                SemanticTerminatorKindV1::Goto(edge(5, SemanticEdgeRoleV1::Goto)),
            ));
            let other = if matches!(mutation, Mutation::DifferentMatrixMeet) {
                vec![assign(
                    place(12, 8),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, fields)
                            .unwrap(),
                    ),
                )]
            } else if split_reference {
                vec![borrow()]
            } else {
                vec![transfer()]
            };
            blocks.push(block(
                94,
                other,
                SemanticTerminatorKindV1::Goto(edge(5, SemanticEdgeRoleV1::Goto)),
            ));
            blocks.push(block(
                95,
                if split_reference {
                    vec![]
                } else {
                    // Test owned-construction SSA equality before one real borrow.
                    vec![assign(
                        place(1, 9),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(12, 8),
                        },
                    )]
                },
                SemanticTerminatorKindV1::Call(f.call.clone()),
            ));
        } else {
            blocks.push(block(
                92,
                consumer_statements,
                SemanticTerminatorKindV1::Call(f.call.clone()),
            ));
        }
        blocks.push(block(99, vec![], SemanticTerminatorKindV1::Return));
        if matches!(mutation, Mutation::ForeignBind) {
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut f.callables[0]
            else {
                unreachable!()
            };
            *operation =
                SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: id(6) };
        }
        if matches!(mutation, Mutation::ForeignRoot) {
            f.context.selected_root = SemanticFunctionIdV1::from_index(1);
        }
        f.function = SemanticFunctionDeclV1::new(
            f.function.identity(),
            f.function.role(),
            f.function.item_definition_identity(),
            f.function.monomorphization_identity(),
            f.function.generic_type_arguments_identity(),
            f.function.const_generic_arguments_identity(),
            source,
            f.function.abi().clone(),
            [0, 9, 11, 2, 2, 12, 6, 7, 8, 5, 15, 8, 8]
                .into_iter()
                .enumerate()
                .map(|(i, ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([120 + i as u8; 32]),
                        id(ty),
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            2..=4 => SemanticLocalRoleV1::Argument((i - 2) as u32),
                            9 => SemanticLocalRoleV1::Argument(3),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                        source,
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        f
    }

    fn plan(f: &Fixture) -> fe2o3_pliron::ProductionSemanticSsaFunctionPlanV1 {
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &f.function,
            &f.types,
            &f.callables,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    #[test]
    fn global_bf16_source_inputs_retain_a_b_geometry_physical_and_payload_sites() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let f = source_fixture(role, Mutation::None);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let inputs = global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                2,
            )
            .unwrap();
            assert_eq!(inputs.contract(), f.contract);
            assert_eq!(
                (inputs.construction().block, inputs.construction().statement),
                (1, Some(1))
            );
            assert_eq!(inputs.global_bind().block, 0);
            assert_ne!(inputs.global(), inputs.matrix());
            assert!(
                matches!(inputs.physical().operand, SemanticOperandV1::Copy(p) if p.local().index() == 9)
            );
            assert_eq!(inputs.geometry()[0].site, inputs.construction());
            assert!(
                matches!(inputs.geometry()[1].operand, SemanticOperandV1::Copy(p) if p.local().index() == 3)
            );
            assert!(
                matches!(inputs.geometry()[2].operand, SemanticOperandV1::Copy(p) if p.local().index() == 4)
            );
            assert_eq!(inputs.lane().operand, &f.call.arguments()[1]);
            assert_eq!(inputs.bases()[0].operand, &f.call.arguments()[2]);
            assert_eq!(inputs.bases()[1].operand, &f.call.arguments()[3]);
            assert!(inputs.matches_use(&f.function, 2, &f.call));
            assert!(!inputs.matches_use(&f.function, 1, &f.call));
        }
    }

    #[test]
    fn global_bf16_source_inputs_reject_dead_owner_wrong_payload_root_bind_and_physical() {
        for mutation in [
            Mutation::DeadGlobal,
            Mutation::DeadMatrix,
            Mutation::WrongVariant,
            Mutation::ForeignBind,
            Mutation::ForeignRoot,
            Mutation::WrongPhysical,
        ] {
            let f = source_fixture(SemanticMfmaOperandRoleV1::A, mutation);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            assert!(
                global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    2
                )
                .is_err()
            );
        }
    }

    #[test]
    fn global_bf16_source_inputs_require_same_construction_on_all_incoming_paths() {
        for mutation in [Mutation::SameOwnerMeet, Mutation::DifferentMatrixMeet] {
            let f = source_fixture(SemanticMfmaOperandRoleV1::B, mutation);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            assert!(plan.plan().is_reachable(SsaBlockIdV1::new(3)));
            assert!(plan.plan().is_reachable(SsaBlockIdV1::new(4)));
            assert!(matches!(
                graph.use_value(5, 12).unwrap(),
                SsaValueV1::BlockArgument { block, variable }
                    if block.get() == 5 && variable.get() == 12
            ));
            let incoming = graph.incoming(5, 12).unwrap();
            assert_eq!(incoming.len(), 2);
            assert_ne!(incoming[0], incoming[1]);
            let result = global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                5,
            );
            assert_eq!(
                result.is_ok(),
                matches!(mutation, Mutation::SameOwnerMeet),
                "mutation={mutation:?} capture={result:?}"
            );
            match mutation {
                Mutation::SameOwnerMeet => {
                    let inputs = result.unwrap();
                    assert_eq!(
                        (inputs.construction().block, inputs.construction().statement),
                        (1, Some(1))
                    );
                }
                Mutation::DifferentMatrixMeet => assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "global BF16 capture merges different Global issuers",
                        ..
                    })
                )),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn global_bf16_split_reference_merge_keeps_unpromoted_owner_fail_closed() {
        let f = source_fixture(SemanticMfmaOperandRoleV1::B, Mutation::SplitReferenceMeet);
        let plan = plan(&f);
        assert!(
            plan.plan()
                .promoted_variables()
                .iter()
                .all(|local| local.get() != 11)
        );
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
        assert!(matches!(
            graph.use_value(5, 1).unwrap(),
            SsaValueV1::BlockArgument { .. }
        ));
        let incoming = graph.incoming(5, 1).unwrap();
        assert_eq!(incoming.len(), 2);
        let mut sites = Vec::new();
        for value in incoming {
            let site = graph.definition(value).unwrap();
            assert_eq!(site.local, 1);
            assert_eq!(site.statement, Some(0));
            sites.push(site.block);
            assert!(
                plan.plan()
                    .resolved_events(SsaBlockIdV1::new(site.block))
                    .unwrap()
                    .iter()
                    .all(|(_, event)| !matches!(event,
                    SsaResolvedEventV1::Use { variable, .. } if variable.get() == 11))
            );
            assert!(matches!(
                graph.use_value(site.block, 11),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "capability reference has no exact SSA use",
                    ..
                })
            ));
        }
        sites.sort_unstable();
        assert_eq!(sites, [3, 4]);
        assert!(matches!(
            global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                5,
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability reference has no exact SSA use",
                ..
            })
        ));
    }

    #[test]
    fn global_bf16_source_inputs_share_the_existing_graph_work_ceiling() {
        let f = source_fixture(SemanticMfmaOperandRoleV1::A, Mutation::None);
        let plan = plan(&f);
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
        global_bf16_live_v1::capture_component(&mut graph, &f.types, &f.callables, &f.context, 2)
            .unwrap();
        // Exhaust precisely the existing owner's remaining allowance.
        while graph.charge(1).is_ok() {}
        assert!(matches!(
            global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                2
            ),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                ..
            })
        ));
    }
}
