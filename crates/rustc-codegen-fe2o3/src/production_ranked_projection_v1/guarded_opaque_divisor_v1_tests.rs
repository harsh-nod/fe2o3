mod guarded_opaque_divisor_tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum Shape {
        Guarded,
        Missing,
        WrongEdge,
        Bypass,
        LaterGuard,
        ReassignedBefore,
        ReassignedAfter,
        Escaped,
        UnknownOperand,
        TwoUses,
        Singleton,
    }

    fn fixture(
        ty: SemanticTypeIdV1,
        operation: SemanticBinaryOpV1,
        shape: Shape,
        literal: Option<u128>,
    ) -> SemanticFunctionDeclV1 {
        let byte_width = if ty == U8_TYPE {
            1
        } else if ty == U16_TYPE {
            2
        } else if ty == U64_TYPE {
            8
        } else if ty == U128_TYPE {
            16
        } else {
            4
        };
        let rhs = || {
            literal.map_or_else(
                || typed_operand(2, ty),
                |value| typed_constant(ty, value, byte_width),
            )
        };
        let expression = |destination| {
            typed_assignment(
                destination,
                ty,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: typed_operand(
                        if matches!(shape, Shape::Singleton) {
                            2
                        } else if matches!(shape, Shape::UnknownOperand) {
                            6
                        } else {
                            1
                        },
                        ty,
                    ),
                    right: rhs(),
                },
            )
        };
        let mut entry = vec![];
        if matches!(shape, Shape::Escaped) {
            entry.push(typed_assignment(
                5,
                POINTER_TYPE,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: typed_place(2, ty),
                },
            ));
        }
        let mut body = vec![];
        if matches!(shape, Shape::ReassignedBefore) {
            body.push(typed_assignment(
                2,
                ty,
                SemanticRvalueKindV1::Use(typed_constant(ty, 0, byte_width)),
            ));
        }
        body.push(expression(3));
        if matches!(shape, Shape::ReassignedAfter) {
            body.push(typed_assignment(
                2,
                ty,
                SemanticRvalueKindV1::Use(typed_constant(ty, 0, byte_width)),
            ));
        }
        if matches!(shape, Shape::TwoUses) {
            body.push(expression(4));
        }
        let entry_terminator = match shape {
            Shape::Missing | Shape::LaterGuard => {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
            }
            Shape::WrongEdge => zero_switch(2, ty, 1, 2),
            Shape::Bypass => zero_switch(7, BOOL_TYPE, 1, 3),
            _ => zero_switch(2, ty, 2, 1),
        };
        let body_terminator = if matches!(shape, Shape::LaterGuard) {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3))
        } else {
            zero_switch(3, ty, 2, 2)
        };
        projection_function_with_locals(
            vec![
                block(211, entry, entry_terminator),
                block(212, body, body_terminator),
                block(213, vec![], SemanticTerminatorKindV1::Return),
                block(214, vec![], zero_switch(2, ty, 2, 1)),
            ],
            vec![
                local(211, ty, SemanticLocalRoleV1::Return),
                local(212, ty, SemanticLocalRoleV1::Argument(0)),
                local(213, ty, SemanticLocalRoleV1::Argument(1)),
                local(214, ty, SemanticLocalRoleV1::Temporary),
                local(215, ty, SemanticLocalRoleV1::Temporary),
                local(216, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(217, ty, SemanticLocalRoleV1::Temporary),
                local(218, BOOL_TYPE, SemanticLocalRoleV1::Argument(2)),
            ],
        )
    }

    fn with_projector<T>(
        function: &SemanticFunctionDeclV1,
        run: impl FnOnce(&mut DeterministicScalarProjectorV1<'_>) -> T,
    ) -> T {
        let types = optional_selector_types();
        let effects = DefinedCallableEmptyEffectSummariesV1 {
            decisions: Box::new([]),
        };
        let constants = constant_locals(function).unwrap();
        let definitions = local_definition_counts(function);
        let count = function.locals().len();
        let indexes = vec![None; count];
        let allocations = vec![None; count];
        let predicates = vec![None; count];
        let mut arguments = vec![None; count];
        let mut next_argument = 0;
        let mut operations = vec![];
        let mut next_value = 0;
        let mut projector = DeterministicScalarProjectorV1::new(
            &types,
            &[],
            &effects,
            function,
            &constants,
            &definitions,
            &indexes,
            &allocations,
            &predicates,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        run(&mut projector)
    }

    fn check_actual(
        p: &mut DeterministicScalarProjectorV1<'_>,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let SemanticStatementKindV1::Assign(assignment) =
            p.function.blocks()[1].statements()[0].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } = assignment.value().kind()
        else {
            unreachable!()
        };
        let (operation, left, right, ty) = (
            *operation,
            left.clone(),
            right.clone(),
            assignment.value().result_type(),
        );
        p.guarded_divisor_is_total_v1(
            ScalarAssignmentSiteV1 {
                block: 1,
                statement: 0,
            },
            ty,
            operation,
            &left,
            &right,
        )
    }

    #[test]
    fn all_supported_unsigned_widths_and_both_operations_are_dependency_only() {
        for (ty, expected_bits) in [
            (U8_TYPE, 8),
            (U16_TYPE, 16),
            (SCALAR_TYPE, 32),
            (U64_TYPE, 64),
        ] {
            for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
                let function = fixture(ty, operation, Shape::Guarded, None);
                let before = function.clone();
                with_projector(&function, |p| {
                    assert!(matches!(
                        p.types.get(ty.index() as usize).map(|ty| ty.shape()),
                        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits,
                        })) if *bits == expected_bits
                    ));
                    let summary = p.resolve_local(3).unwrap().unwrap();
                    assert!(
                        matches!(&summary, DeterministicScalarSummaryV1::Derived(v) if v.len() == 2)
                    );
                    p.materialize(summary).unwrap();
                    assert!(p.operations.iter().any(|operation|
                        matches!(operation, ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                            if dependencies.len() == 2)));
                    assert!(!p.operations.iter().any(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::IndexBinary {
                            kind: IndexBinaryKindAttr::Divide | IndexBinaryKindAttr::Remainder,
                            ..
                        }
                    )));
                    assert!(p.guarded_divisor_proofs.as_ref().unwrap().work > 0);
                });
                assert_eq!(function, before);
            }
        }
    }

    #[test]
    fn actual_switch_projection_uses_an_opaque_join_not_entry_arithmetic() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Guarded,
            None,
        );
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &function,
            &vec![None; function.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        let value = switches[1].as_ref().unwrap().discriminant;
        assert!(operations.iter().any(|operation| matches!(operation,
            ProductionRankedOperationV1::DeterministicJoin { result, dependencies }
            if value == ProductionRankedValueV1::Local(*result) && dependencies.len() == 2)));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::IndexBinary {
                kind: IndexBinaryKindAttr::Divide,
                ..
            }
        )));
    }

    #[test]
    fn singleton_dependency_remains_opaque() {
        for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
            let function = fixture(SCALAR_TYPE, operation, Shape::Singleton, None);
            with_projector(&function, |p| {
                let summary = p.resolve_local(3).unwrap().unwrap();
                assert!(
                    matches!(&summary, DeterministicScalarSummaryV1::Derived(v) if v.len() == 1)
                );
                let result = p.materialize(summary).unwrap();
                assert!(matches!(p.operations.last().unwrap(),
                    ProductionRankedOperationV1::DeterministicJoin { result: id, dependencies }
                    if result == ProductionRankedValueV1::Local(*id) && dependencies.len() == 1));
            });
        }
    }

    #[test]
    fn missing_wrong_bypassed_and_later_guards_keep_exact_divisor_refusal() {
        for shape in [
            Shape::Missing,
            Shape::WrongEdge,
            Shape::Bypass,
            Shape::LaterGuard,
        ] {
            for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
                let function = fixture(SCALAR_TYPE, operation, shape, None);
                with_projector(&function, |p| {
                    assert!(!check_actual(p).unwrap(), "{shape:?}");
                    assert!(
                        matches!(
                            p.resolve_local(3),
                            Err(ProductionRankedProjectionErrorV1::UnprovenDeterministicDivisor(_))
                        ),
                        "{shape:?}"
                    );
                    assert!(!p.operations.iter().any(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::DeterministicJoin { .. }
                    )));
                });
            }
        }
    }

    #[test]
    fn reassignment_escape_and_unknown_inputs_do_not_gain_a_summary() {
        for shape in [
            Shape::ReassignedBefore,
            Shape::ReassignedAfter,
            Shape::Escaped,
        ] {
            let function = fixture(SCALAR_TYPE, SemanticBinaryOpV1::Divide, shape, None);
            with_projector(&function, |p| {
                let statement = usize::from(matches!(shape, Shape::ReassignedBefore));
                assert!(
                    !p.guarded_divisor_is_total_v1(
                        ScalarAssignmentSiteV1 {
                            block: 1,
                            statement
                        },
                        SCALAR_TYPE,
                        SemanticBinaryOpV1::Divide,
                        &typed_operand(1, SCALAR_TYPE),
                        &typed_operand(2, SCALAR_TYPE),
                    )
                    .unwrap(),
                    "{shape:?}"
                );
                assert!(
                    matches!(
                        p.resolve_local(3),
                        Err(ProductionRankedProjectionErrorV1::UnprovenDeterministicDivisor(_))
                    ),
                    "{shape:?}"
                );
            });
        }
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::UnknownOperand,
            None,
        );
        with_projector(&function, |p| {
            assert_eq!(p.resolve_local(3).unwrap(), None);
            assert!(p.guarded_divisor_proofs.is_none());
        });
    }

    #[test]
    fn exact_literal_path_is_unchanged_and_does_not_construct_range_engine() {
        for operation in [SemanticBinaryOpV1::Divide, SemanticBinaryOpV1::Remainder] {
            for value in [1, 16, u128::from(u32::MAX)] {
                let function = fixture(SCALAR_TYPE, operation, Shape::Missing, Some(value));
                with_projector(&function, |p| {
                    assert!(matches!(
                        p.resolve_local(3).unwrap(),
                        Some(DeterministicScalarSummaryV1::Exact(_))
                    ));
                    assert!(p.guarded_divisor_proofs.is_none());
                    assert!(p.operations.iter().any(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::IndexBinary {
                            kind: IndexBinaryKindAttr::Divide | IndexBinaryKindAttr::Remainder,
                            ..
                        }
                    )));
                    assert!(!p.operations.iter().any(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::DeterministicJoin { .. }
                    )));
                });
            }
        }
    }

    #[test]
    fn zero_constant_is_not_proved_by_a_guard_on_an_unrelated_local() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Guarded,
            Some(0),
        );
        with_projector(&function, |p| {
            assert!(!check_actual(p).unwrap());
            assert!(matches!(
                p.resolve_local(3),
                Err(ProductionRankedProjectionErrorV1::UnprovenDeterministicDivisor(_))
            ));
        });
    }

    #[test]
    fn signed_bool_wide_and_nonscalar_types_stay_outside_extension() {
        for ty in [
            I32_TYPE,
            BOOL_TYPE,
            U128_TYPE,
            ARRAY_TYPE,
            F64_TYPE,
            SemanticTypeIdV1::from_index(u32::MAX),
        ] {
            let function = fixture(ty, SemanticBinaryOpV1::Divide, Shape::Guarded, None);
            with_projector(&function, |p| {
                assert!(!check_actual(p).unwrap());
                assert!(p.guarded_divisor_proofs.is_none());
            });
        }
    }

    #[test]
    fn source_site_operation_and_each_operand_are_bound_exactly() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Guarded,
            None,
        );
        with_projector(&function, |p| {
            let lhs = typed_operand(1, SCALAR_TYPE);
            let rhs = typed_operand(2, SCALAR_TYPE);
            for site in [
                ScalarAssignmentSiteV1 {
                    block: 0,
                    statement: 0,
                },
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 1,
                },
                ScalarAssignmentSiteV1 {
                    block: usize::MAX,
                    statement: 0,
                },
            ] {
                assert!(
                    !p.guarded_divisor_is_total_v1(
                        site,
                        SCALAR_TYPE,
                        SemanticBinaryOpV1::Divide,
                        &lhs,
                        &rhs
                    )
                    .unwrap()
                );
            }
            let site = ScalarAssignmentSiteV1 {
                block: 1,
                statement: 0,
            };
            for operation in [SemanticBinaryOpV1::Remainder, SemanticBinaryOpV1::Add] {
                assert!(
                    !p.guarded_divisor_is_total_v1(site, SCALAR_TYPE, operation, &lhs, &rhs)
                        .unwrap()
                );
            }
            assert!(
                !p.guarded_divisor_is_total_v1(
                    site,
                    U64_TYPE,
                    SemanticBinaryOpV1::Divide,
                    &lhs,
                    &rhs
                )
                .unwrap()
            );
            assert!(
                !p.guarded_divisor_is_total_v1(
                    site,
                    SCALAR_TYPE,
                    SemanticBinaryOpV1::Divide,
                    &typed_operand(1, U64_TYPE),
                    &rhs
                )
                .unwrap()
            );
            assert!(
                !p.guarded_divisor_is_total_v1(
                    site,
                    SCALAR_TYPE,
                    SemanticBinaryOpV1::Divide,
                    &lhs,
                    &typed_operand(2, U64_TYPE)
                )
                .unwrap()
            );
            assert!(
                !p.guarded_divisor_is_total_v1(
                    site,
                    SCALAR_TYPE,
                    SemanticBinaryOpV1::Divide,
                    &rhs,
                    &rhs
                )
                .unwrap()
            );
            assert!(
                !p.guarded_divisor_is_total_v1(
                    site,
                    SCALAR_TYPE,
                    SemanticBinaryOpV1::Divide,
                    &lhs,
                    &lhs
                )
                .unwrap()
            );
            assert!(check_actual(p).unwrap());
        });
    }

    #[test]
    fn stale_site_evidence_is_not_shared_between_function_owners() {
        let guarded = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Guarded,
            None,
        );
        let unguarded = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Missing,
            None,
        );
        with_projector(&guarded, |p| assert!(check_actual(p).unwrap()));
        with_projector(&unguarded, |p| assert!(!check_actual(p).unwrap()));
    }

    #[test]
    fn refusal_keeps_the_actual_assignment_diagnostic() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Remainder,
            Shape::Missing,
            None,
        );
        with_projector(&function, |p| {
            let Err(ProductionRankedProjectionErrorV1::UnprovenDeterministicDivisor(diagnostic)) =
                p.resolve_local(3)
            else {
                unreachable!("the unguarded actual assignment must refuse")
            };
            let site = diagnostic.site.unwrap();
            assert_eq!((site.block, site.statement, site.destination), (1, 0, 3));
            assert_eq!(site.source, function.blocks()[1].statements()[0].source());
            assert_eq!(diagnostic.operation, SemanticBinaryOpV1::Remainder);
            assert_eq!(
                diagnostic.summary,
                DeterministicDivisorSummaryDiagnosticV1::Exact
            );
        });
    }

    #[test]
    fn one_cached_engine_retains_cumulative_work_across_actual_assignments() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::TwoUses,
            None,
        );
        with_projector(&function, |p| {
            assert!(matches!(
                p.resolve_local(3).unwrap(),
                Some(DeterministicScalarSummaryV1::Derived(_))
            ));
            let first = p.guarded_divisor_proofs.as_ref().unwrap();
            let identity = std::ptr::from_ref(first);
            let work = first.work;
            let cached = first.zero_exclusion.len();
            assert!(cached > 0);
            assert!(matches!(
                p.resolve_local(4).unwrap(),
                Some(DeterministicScalarSummaryV1::Derived(_))
            ));
            let second = p.guarded_divisor_proofs.as_ref().unwrap();
            assert_eq!(identity, std::ptr::from_ref(second));
            assert!(second.work > work);
            assert_eq!(second.zero_exclusion.len(), cached);
        });
    }

    #[test]
    fn range_work_exact_limit_one_short_and_overflow_propagate() {
        let function = fixture(
            SCALAR_TYPE,
            SemanticBinaryOpV1::Divide,
            Shape::Guarded,
            None,
        );
        let measured = with_projector(&function, |p| {
            assert!(check_actual(p).unwrap());
            p.guarded_divisor_proofs.as_ref().unwrap().work
        });
        assert!(measured > 8 && measured < MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        with_projector(&function, |p| {
            let mut proof = SemanticAssertProofsV1::new(p.types, p.function).unwrap();
            proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - measured;
            p.guarded_divisor_proofs = Some(proof);
            assert!(check_actual(p).unwrap());
            assert_eq!(
                p.guarded_divisor_proofs.as_ref().unwrap().work,
                MAX_PROJECTED_LOOP_GRAPH_WORK_V1
            );
        });
        with_projector(&function, |p| {
            let mut proof = SemanticAssertProofsV1::new(p.types, p.function).unwrap();
            proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - measured + 1;
            p.guarded_divisor_proofs = Some(proof);
            assert!(matches!(
                check_actual(p),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform induction CFG analysis exceeds its work limit"
                ))
            ));
        });
        with_projector(&function, |p| {
            let mut proof = SemanticAssertProofsV1::new(p.types, p.function).unwrap();
            proof.work = usize::MAX;
            p.guarded_divisor_proofs = Some(proof);
            assert!(matches!(
                check_actual(p),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform induction CFG analysis work overflow"
                ))
            ));
        });
    }
}
