mod enum_carrier_trace_tests_v1 {
    use super::*;

    fn fixture(
        links: u32,
        actual_variant: u32,
        moved: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticFunctionDeclV1,
        SemanticTypeIdV1,
    ) {
        let mut types = assertion_proof_types();
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(249)),
            SemanticLayoutIdentityV1::from_sha256(bytes(249)),
            SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            SemanticTypeShapeV1::enum_type(
                U64_TYPE,
                vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap(),
                    ),
                ],
            )
            .unwrap(),
        ));
        let mut statements = vec![typed_assignment(
            1,
            ty,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(actual_variant),
                    vec![typed_constant(U64_TYPE, 9, 8)],
                )
                .unwrap(),
            ),
        )];
        let mut locals = vec![local(230, U64_TYPE, SemanticLocalRoleV1::Return)];
        for id in 1..=links {
            locals.push(local(230 + id as u8, ty, SemanticLocalRoleV1::Temporary));
            if id > 1 {
                let source = if moved {
                    SemanticOperandV1::Move(typed_place(id - 1, ty))
                } else {
                    typed_operand(id - 1, ty)
                };
                statements.push(typed_assignment(id, ty, SemanticRvalueKindV1::Use(source)));
            }
        }
        statements.push(typed_assignment(
            0,
            U64_TYPE,
            SemanticRvalueKindV1::Use(projected(links, ty, 0)),
        ));
        (
            types,
            projection_function_with_locals(
                vec![block(230, statements, SemanticTerminatorKindV1::Return)],
                locals,
            ),
            ty,
        )
    }

    fn projected(local: u32, ty: SemanticTypeIdV1, field: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ty).unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), U64_TYPE)
                        .unwrap(),
                ],
                U64_TYPE,
            )
            .unwrap(),
        )
    }

    fn replace(
        function: &SemanticFunctionDeclV1,
        statements: Vec<SemanticStatementV1>,
    ) -> SemanticFunctionDeclV1 {
        typed_global_fixture_with_body_v1(
            function,
            function.locals().to_vec(),
            vec![block(230, statements, SemanticTerminatorKindV1::Return)],
        )
    }

    fn check(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
    ) -> (IndexTraceOutcomeV1, String) {
        let indices = vec![None; function.locals().len()];
        let query = |enabled| {
            index_trace_query_mode_v1(
                types,
                &[],
                function,
                &indices,
                operand,
                0,
                function.blocks()[0].statements().len() - 1,
                enabled,
                true,
            )
        };
        let (off, off_text) = query(false);
        let (on, text) = query(true);
        assert_eq!(off, on);
        assert!(off_text.is_empty());
        assert!(text.lines().count() <= 5);
        assert!(text.len() < 4096);
        (on, text)
    }

    #[test]
    fn enum_carrier_trace_exact_wrong_variant_and_copy_move_chain_do_not_change_proof() {
        for moved in [false, true] {
            let (types, function, ty) = fixture(3, 1, moved);
            let (result, text) = check(&types, &function, &projected(3, ty, 0));
            assert!(result.summary.is_none());
            assert_eq!(text.lines().count(), 4);
            assert!(text.contains("link=0 use=bb0:s3 carrier=3"));
            assert!(text.contains("definition=Some((0, 2))"));
            assert!(text.contains(if moved {
                "rvalue=Use operand=Move(local=2,"
            } else {
                "rvalue=Use operand=Copy(local=2,"
            }));
            assert!(text.contains("Downcast(0)"));
            assert!(text.contains("rvalue=Aggregate kind=EnumVariant(1) arity=1"));
            assert!(text.ends_with("stop=not-direct-forwarding\n"));
        }
    }

    #[test]
    fn enum_carrier_trace_retains_later_numbered_original_definition_coordinates() {
        let (types, function, ty) = fixture(3, 1, false);
        let statements = function.blocks()[0].statements();
        let function = typed_global_fixture_with_body_v1(
            &function,
            function.locals().to_vec(),
            vec![
                block(
                    230,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(
                    231,
                    statements[2..].to_vec(),
                    SemanticTerminatorKindV1::Return,
                ),
                block(
                    232,
                    statements[..2].to_vec(),
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
            ],
        );
        let indices = vec![None; function.locals().len()];
        let operand = projected(3, ty, 0);
        let query = |enabled| {
            index_trace_query_mode_v1(
                &types,
                &[],
                &function,
                &indices,
                &operand,
                1,
                1,
                enabled,
                true,
            )
        };
        let (off, off_text) = query(false);
        let (on, text) = query(true);
        assert_eq!(off, on);
        assert!(off_text.is_empty() && on.summary.is_none());
        assert!(text.contains("link=0 use=bb1:s1"));
        assert!(text.contains("definition=Some((1, 0))"));
        assert!(text.contains("definition=Some((2, 1))"));
        assert!(text.contains("definition=Some((2, 0))"));
        assert!(text.contains("rvalue=Aggregate kind=EnumVariant(1)"));
    }

    #[test]
    fn enum_carrier_trace_success_is_silent_with_identical_counters_and_operations() {
        let (types, function, ty) = fixture(2, 0, false);
        let (result, text) = check(&types, &function, &projected(2, ty, 0));
        assert_eq!(result.summary.unwrap().2, Some(9));
        assert!(text.is_empty());
    }

    #[test]
    fn enum_carrier_trace_reports_four_links_without_chasing_the_remaining_chain() {
        let (types, function, ty) = fixture(8, 1, false);
        let (result, text) = check(&types, &function, &projected(8, ty, 0));
        assert!(result.summary.is_none());
        assert_eq!(text.lines().count(), 5);
        assert!(text.ends_with("next=4 stop=link-limit\n"));
        assert!(!text.contains("rvalue=Aggregate"));
    }

    #[test]
    fn enum_carrier_trace_cycle_does_not_evaluate_or_hide_original_rejection() {
        let (types, function, ty) = fixture(2, 1, false);
        let mut statements = function.blocks()[0].statements().to_vec();
        statements[0] = typed_assignment(1, ty, SemanticRvalueKindV1::Use(typed_operand(2, ty)));
        let function = replace(&function, statements);
        let (result, text) = check(&types, &function, &projected(2, ty, 0));
        assert!(result.summary.is_none());
        assert!(text.ends_with("next=2 stop=cycle\n"));
    }

    #[test]
    fn enum_carrier_trace_nonunique_or_missing_definition_never_selects_an_arm() {
        for missing in [false, true] {
            let (types, function, ty) = fixture(2, 1, false);
            let mut statements = function.blocks()[0].statements().to_vec();
            if missing {
                statements.remove(1);
            } else {
                statements.insert(2, statements[1].clone());
            }
            let function = replace(&function, statements);
            let (result, text) = check(&types, &function, &projected(2, ty, 0));
            assert!(result.summary.is_none());
            assert_eq!(text.lines().count(), 2);
            assert!(text.ends_with("stop=nonunique-or-missing-definition\n"));
            assert!(!text.contains("rvalue="));
        }
    }

    #[test]
    fn enum_carrier_trace_projected_forwarding_stops_at_the_actual_rvalue() {
        let (types, function, ty) = fixture(2, 1, false);
        let mut statements = function.blocks()[0].statements().to_vec();
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), ty).unwrap()],
            ty,
        )
        .unwrap();
        statements[1] = typed_assignment(
            2,
            ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)),
        );
        let function = replace(&function, statements);
        let (result, text) = check(&types, &function, &projected(2, ty, 0));
        assert!(result.summary.is_none());
        assert!(text.contains("rvalue=Use operand=Move(local=1,"));
        assert!(text.contains("projection_count=1"));
        assert!(text.ends_with("stop=projected-use\n"));
    }

    #[test]
    fn enum_carrier_trace_wide_aggregate_reports_only_prefix_and_requested_field() {
        let (types, function, ty) = fixture(1, 1, false);
        let mut statements = function.blocks()[0].statements().to_vec();
        statements[0] = typed_assignment(
            1,
            ty,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(1),
                    vec![typed_constant(U64_TYPE, 9, 8); 128],
                )
                .unwrap(),
            ),
        );
        let operand = projected(1, ty, 127);
        statements[1] = typed_assignment(0, U64_TYPE, SemanticRvalueKindV1::Use(operand.clone()));
        let function = replace(&function, statements);
        let (result, text) = check(&types, &function, &operand);
        assert!(result.summary.is_none());
        assert!(text.contains("arity=128"));
        assert!(text.contains("requested_operand127=Constant(type="));
        assert!(text.contains("operand_prefix_truncated=true"));
        assert_eq!(text.matches("Constant(type=").count(), 3);
    }
}
