// Source-shape component regression. The real AMD callback remains the original
// strict scoped_bf16_complete_source_inputs_ test with four observed read slots.
mod global_carrier_tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum Change {
        None,
        DeadA,
        DeadB,
        OverwriteB,
        ForeignRoot,
    }

    fn p(local: u32, ty: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], id(ty)).unwrap()
    }

    fn assign(local: u32, ty: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                p(local, ty),
                SemanticRvalueV1::new(id(ty), value),
            )),
        )
    }

    fn borrowed(
        reference: u32,
        owner: u32,
        reference_type: u32,
        owner_type: u32,
    ) -> SemanticStatementV1 {
        assign(
            reference,
            reference_type,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: p(owner, owner_type),
            },
        )
    }

    fn edge(block: u32, role: SemanticEdgeRoleV1) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
    }

    fn block(
        index: u8,
        statements: Vec<SemanticStatementV1>,
        terminal: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([210 + index; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            statements,
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminal),
        )
        .unwrap()
    }

    fn fixture(change: Change, role: SemanticMfmaOperandRoleV1) -> Fixture {
        let mut f = source_fixture(role, Mutation::None);
        assert_eq!(f.types.len(), 16);
        f.types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([36; 32]),
            SemanticLayoutIdentityV1::from_sha256([36; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(24),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![id(7), id(7), id(2)]).unwrap(),
            ),
        ));
        let SemanticTerminatorKindV1::Call(bind_a) = f.function.blocks()[0].terminator().kind()
        else {
            unreachable!()
        };
        let bind_b = SemanticDirectCallV1::new_callable(
            bind_a.callee(),
            bind_a.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                p(13, 6),
                edge(2, SemanticEdgeRoleV1::CallReturn),
            )),
            bind_a.unwind().clone(),
        )
        .unwrap();
        let SemanticStatementKindV1::Assign(a) = f.function.blocks()[1].statements()[1].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Aggregate(matrix) = a.value().kind() else {
            unreachable!()
        };
        let matrix_value = |reference| {
            let mut fields = matrix.operands().to_vec();
            fields[0] = SemanticOperandV1::Copy(p(reference, 7));
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, fields).unwrap(),
            )
        };
        let field = |field| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(16),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), id(7))
                        .unwrap(),
                ],
                id(7),
            )
            .unwrap()
        };
        let mut statements = vec![
            borrowed(7, 6, 7, 6),
            borrowed(14, 13, 7, 6),
            assign(
                15,
                16,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            SemanticOperandV1::Copy(p(7, 7)),
                            SemanticOperandV1::Copy(p(14, 7)),
                            SemanticOperandV1::Copy(p(3, 2)),
                        ],
                    )
                    .unwrap(),
                ),
            ),
            assign(
                16,
                16,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p(15, 16))),
            ),
            assign(
                17,
                7,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0))),
            ),
            assign(
                18,
                7,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1))),
            ),
            assign(
                22,
                7,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(18),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, id(6))
                                .unwrap(),
                        ],
                        id(6),
                    )
                    .unwrap(),
                },
            ),
            assign(8, 8, matrix_value(17)),
            assign(19, 8, matrix_value(22)),
            borrowed(1, 8, 9, 8),
            borrowed(20, 19, 9, 8),
        ];
        match change {
            Change::DeadA | Change::DeadB => statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(
                    if matches!(change, Change::DeadA) {
                        6
                    } else {
                        13
                    },
                )),
            )),
            Change::OverwriteB => statements.push(assign(
                13,
                6,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p(6, 6))),
            )),
            Change::None | Change::ForeignRoot => {}
        }
        let call_a = SemanticDirectCallV1::new_callable(
            f.call.callee(),
            f.call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                p(5, 12),
                edge(3, SemanticEdgeRoleV1::CallReturn),
            )),
            f.call.unwind().clone(),
        )
        .unwrap();
        let mut args = f.call.arguments().to_vec();
        args[0] = SemanticOperandV1::Copy(p(20, 9));
        let call_b = SemanticDirectCallV1::new_callable(
            f.call.callee(),
            args,
            Some(SemanticCallDestinationV1::new(
                p(21, 12),
                edge(4, SemanticEdgeRoleV1::CallReturn),
            )),
            f.call.unwind().clone(),
        )
        .unwrap();
        let mut locals = f.function.locals().to_vec();
        for (local, ty) in (13u8..).zip([6, 7, 16, 16, 7, 7, 8, 9, 12, 7]) {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([120 + local; 32]),
                id(ty),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
        }
        f.function = SemanticFunctionDeclV1::new(
            f.function.identity(),
            f.function.role(),
            f.function.item_definition_identity(),
            f.function.monomorphization_identity(),
            f.function.generic_type_arguments_identity(),
            f.function.const_generic_arguments_identity(),
            f.function.source(),
            f.function.abi().clone(),
            locals,
            f.function.entry(),
            vec![
                block(0, vec![], SemanticTerminatorKindV1::Call(bind_a.clone())),
                block(1, vec![], SemanticTerminatorKindV1::Call(bind_b)),
                block(
                    2,
                    statements,
                    SemanticTerminatorKindV1::Call(call_a.clone()),
                ),
                block(3, vec![], SemanticTerminatorKindV1::Call(call_b)),
                block(4, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        f.call = call_a;
        if matches!(change, Change::ForeignRoot) {
            f.context.selected_root = SemanticFunctionIdV1::from_index(1);
        }
        f
    }

    #[test]
    fn global_carrier_live_each_field_keeps_its_exact_issuer_and_original_ssa_use() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let f = fixture(Change::None, role);
            let plan = plan(&f);
            for local in [6, 13] {
                assert!(
                    plan.plan()
                        .promoted_variables()
                        .iter()
                        .any(|v| v.get() == local)
                );
                assert!(plan.plan().resolved_events(SsaBlockIdV1::new(2)).unwrap().iter().any(|(_, event)|
                    matches!(event, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)));
            }
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let first = global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                2,
            )
            .unwrap();
            let second = global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                3,
            )
            .unwrap();
            assert_eq!(
                (first.global_bind().block, second.global_bind().block),
                (0, 1)
            );
            assert_eq!(
                (first.global_bind().local, second.global_bind().local),
                (6, 13)
            );
            assert_ne!(first.global(), second.global());
            assert_ne!(first.matrix(), second.matrix());
            assert_eq!(
                (
                    first.construction().statement,
                    second.construction().statement
                ),
                (Some(7), Some(8))
            );
            assert_eq!(first.global(), graph.use_value(2, 6).unwrap());
            assert_eq!(second.global(), graph.use_value(2, 13).unwrap());
        }
    }

    #[test]
    fn global_carrier_live_each_owner_death_rejects_only_its_actual_loan() {
        for (change, dead_block, live_block) in [(Change::DeadA, 2, 3), (Change::DeadB, 3, 2)] {
            let f = fixture(change, SemanticMfmaOperandRoleV1::A);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            assert!(
                matches!(
                    global_bf16_live_v1::capture_component(
                        &mut graph,
                        &f.types,
                        &f.callables,
                        &f.context,
                        dead_block
                    ),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                        ..
                    })
                ),
                "{change:?}"
            );
            let live = global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                live_block,
            )
            .unwrap();
            assert_eq!(
                live.global_bind().local,
                if live_block == 2 { 6 } else { 13 }
            );
        }
    }

    #[test]
    fn global_carrier_live_overwritten_sibling_does_not_become_the_other_issuer() {
        let f = fixture(Change::OverwriteB, SemanticMfmaOperandRoleV1::B);
        let plan = plan(&f);
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
        let live = global_bf16_live_v1::capture_component(
            &mut graph,
            &f.types,
            &f.callables,
            &f.context,
            2,
        )
        .unwrap();
        assert_eq!(live.global_bind().local, 6);
        assert!(matches!(
            global_bf16_live_v1::capture_component(
                &mut graph,
                &f.types,
                &f.callables,
                &f.context,
                3
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                ..
            })
        ));
    }

    #[test]
    fn global_carrier_live_group_transparency_cannot_substitute_another_root() {
        let f = fixture(Change::ForeignRoot, SemanticMfmaOperandRoleV1::A);
        let plan = plan(&f);
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
        for block in [2, 3] {
            assert!(
                global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    block
                )
                .is_err()
            );
        }
    }
}
