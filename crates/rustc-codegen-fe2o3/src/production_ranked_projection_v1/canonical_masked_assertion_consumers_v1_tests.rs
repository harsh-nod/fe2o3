mod masked_source_assertion_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryErrorV1 as QueryError;

    fn successor(block: u32) -> SemanticBlockIdV1 {
        SemanticBlockIdV1::from_index(block)
    }

    fn masked_source(
        operation: SemanticBinaryOpV1,
        mask: Option<u128>,
        message_operation: SemanticBinaryOpV1,
    ) -> SemanticFunctionDeclV1 {
        assertion_root(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_U32, SemanticLocalRoleV1::Argument(0)),
                (A_U32, SemanticLocalRoleV1::Argument(1)),
                (A_U32, SemanticLocalRoleV1::Temporary),
                (A_BOOL, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![A_U32, A_U32],
            vec![
                // The existing fixture appends its real private write here.
                block(
                    210,
                    vec![],
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        successor(1),
                    )),
                ),
                block(
                    211,
                    vec![
                        typed_assignment(
                            3,
                            A_U32,
                            match mask {
                                Some(bits) => SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::BitAnd,
                                    left: typed_operand(2, A_U32),
                                    right: typed_constant(A_U32, bits, 4),
                                },
                                None => SemanticRvalueKindV1::Use(typed_operand(2, A_U32)),
                            },
                        ),
                        typed_assignment(
                            4,
                            A_BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: typed_operand(3, A_U32),
                                right: typed_constant(A_U32, 32, 4),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Move(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], A_BOOL)
                                .unwrap(),
                        ),
                        expected: true,
                        message: SemanticAssertMessageV1::Overflow {
                            operation: message_operation,
                            left: typed_operand(1, A_U32),
                            right: typed_operand(3, A_U32),
                        },
                        target: SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertSuccess,
                            successor(2),
                        ),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    212,
                    vec![typed_assignment(
                        5,
                        A_U32,
                        SemanticRvalueKindV1::Binary {
                            operation,
                            left: typed_operand(1, A_U32),
                            right: SemanticOperandV1::Move(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(3),
                                    vec![],
                                    A_U32,
                                )
                                .unwrap(),
                            ),
                        },
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
    }

    #[test]
    fn masked_assertion_genuine_elision_requires_independent_source_replay() {
        for operation in [
            SemanticBinaryOpV1::ShiftLeft,
            SemanticBinaryOpV1::ShiftRight,
        ] {
            let owner = assertion_materialized(masked_source(operation, Some(31), operation));
            let function = &owner.semantic_ssa().source_semantic().functions()[0];
            with_canonical_assertions_v1(&owner, |session| {
                let floor = session.retained_floor_for_test_v1();
                {
                    let mut legacy = session.for_source(ROOT, ROOT);
                    assert_eq!(
                        legacy.condition(1, true, successor(2))?,
                        ProjectedAssertionConditionV1::ElidedByExistingRule
                    );
                    assert!(!legacy.masked_assertion_source_proved_v1(
                        function,
                        1,
                        true,
                        successor(2)
                    )?);
                    assert!(matches!(
                        projected_cfg_terminator(function, 1, &[], false, &mut legacy, &[], &[]),
                        Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 1, .. })
                    ));
                }
                session.with_source_masked_assertions_v1(ROOT, ROOT, |facts| {
                    assert!(facts.masked_assertion_source_proved_v1(
                        function,
                        1,
                        true,
                        successor(2)
                    )?);
                    assert_eq!(
                        facts.condition(1, true, successor(2))?,
                        ProjectedAssertionConditionV1::ElidedByExistingRule
                    );
                    projected_cfg_terminator(function, 1, &[], false, facts, &[], &[])?;
                    Ok(())
                })?;
                assert_eq!(session.retained_floor_for_test_v1(), floor);
                Ok(())
            })
            .unwrap();
            // This is ranked assertion projection, not final checked-output or native admission.
            assertion_project(owner).unwrap();
        }
    }

    #[test]
    fn masked_assertion_raw_wrong_mask_and_message_remain_unproved() {
        let left = SemanticBinaryOpV1::ShiftLeft;
        for (mask, message) in [
            (None, left),
            (Some(63), left),
            (Some(30), left),
            (Some(31), SemanticBinaryOpV1::ShiftRight),
        ] {
            let owner = assertion_materialized(masked_source(left, mask, message));
            let function = &owner.semantic_ssa().source_semantic().functions()[0];
            with_canonical_assertions_v1(&owner, |session| {
                session.with_source_masked_assertions_v1(ROOT, ROOT, |facts| {
                    assert!(!facts.masked_assertion_source_proved_v1(
                        function,
                        1,
                        true,
                        successor(2)
                    )?);
                    // A mask of 30 is mathematically bounded but deliberately outside this
                    // exact source rule. Native SCCP is separately permitted to prove it.
                    if mask != Some(30) && message == left {
                        assert!(matches!(
                            projected_cfg_terminator(function, 1, &[], false, facts, &[], &[]),
                            Err(ProductionRankedProjectionErrorV1::UnprovenAssert { block: 1, .. })
                        ));
                    }
                    Ok(())
                })
            })
            .unwrap();
        }
    }

    #[test]
    fn masked_assertion_table_binds_actual_function_expected_and_successor() {
        let operation = SemanticBinaryOpV1::ShiftLeft;
        let owner = assertion_materialized(masked_source(operation, Some(31), operation));
        let function = &owner.semantic_ssa().source_semantic().functions()[0];
        let cloned = function.clone();
        with_canonical_assertions_v1(&owner, |session| {
            session.with_source_masked_assertions_v1(ROOT, ROOT, |facts| {
                for (actual, block, expected, target) in [
                    (&cloned, 1, true, successor(2)),
                    (function, 0, true, successor(1)),
                    (function, 1, false, successor(2)),
                    (function, 1, true, successor(0)),
                    (function, 99, true, successor(2)),
                ] {
                    assert!(matches!(
                        facts.masked_assertion_source_proved_v1(actual, block, expected, target),
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::Binding(_)
                        ))
                    ));
                }
                let proved =
                    facts.masked_assertion_source_proved_v1(function, 1, true, successor(2))?;
                assert!(proved);
                // The source fact is genuine; these graph outcomes independently test
                // the final merge rule and do not fabricate a retained graph report.
                assert!(!projected_assertion_is_proved_v1(
                    ProjectedAssertionConditionV1::Bool(false),
                    true,
                    proved,
                ));
                Ok(())
            })
        })
        .unwrap();
    }

    #[test]
    fn masked_assertion_query_exact_work_storage_and_one_short_are_fail_closed() {
        let operation = SemanticBinaryOpV1::ShiftRight;
        let owner = assertion_materialized(masked_source(operation, Some(31), operation));
        let function = &owner.semantic_ssa().source_semantic().functions()[0];
        with_canonical_assertions_v1(&owner, |session| {
            let floor = session.retained_floor_for_test_v1() + 23;
            let profile = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result = session.with_source_masked_assertions_query_budget_v1(
                    &mut budget,
                    ROOT,
                    ROOT,
                    |facts| {
                        assert!(facts.masked_assertion_source_proved_v1(
                            function,
                            1,
                            true,
                            successor(2)
                        )?);
                        Ok(())
                    },
                );
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.storage(),
                )
            };
            let full = profile(usize::MAX, usize::MAX);
            full.0.unwrap();
            assert_eq!(full.3, floor);
            let exact = profile(full.1, full.2);
            exact.0.unwrap();
            assert_eq!((exact.1, exact.2, exact.3), (full.1, full.2, floor));
            let short = profile(full.1 - 1, full.2);
            assert!(matches!(
                short.0,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Work(_))
                ))
            ));
            assert_eq!(short.3, floor);
            let short = profile(full.1, full.2 - 1);
            assert!(matches!(
                short.0,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::MaskedAssertion(QueryError::Resource(
                        Resource::Storage(_)
                    )) | CanonicalAssertionErrorV1::Resource(Resource::Storage(_))
                ))
            ));
            assert_eq!(short.3, floor);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn masked_assertion_callback_error_and_panic_release_only_query_table() {
        let operation = SemanticBinaryOpV1::ShiftLeft;
        let owner = assertion_materialized(masked_source(operation, Some(31), operation));
        with_canonical_assertions_v1(&owner, |session| {
            let floor = session.retained_floor_for_test_v1() + 31;
            for panic in [false, true] {
                let mut work = Work::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                budget.reserve_storage(floor).unwrap();
                let dropped = std::cell::Cell::new(0);
                struct Local<'a>(&'a std::cell::Cell<usize>);
                impl Drop for Local<'_> {
                    fn drop(&mut self) {
                        self.0.set(self.0.get() + 1);
                    }
                }
                let result: Result<(), _> = session.with_source_masked_assertions_query_budget_v1(
                    &mut budget,
                    ROOT,
                    ROOT,
                    |_| {
                        let _local = Local(&dropped);
                        if panic {
                            panic!("backend assertion callback");
                        }
                        Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "callback sentinel",
                        ))
                    },
                );
                if panic {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::MaskedAssertion(QueryError::Panicked)
                        ))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "callback sentinel"
                        ))
                    ));
                }
                assert_eq!(dropped.get(), 1);
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > 0);
            }
            Ok(())
        })
        .unwrap();
    }
}
