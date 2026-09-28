// Raw semantic component parity controls, not admitted owners or proof certificates.
// The original graph and inventory are test setup, then borrowed by the strict seam.
mod original_meter_assertion_parity_v1 {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::analyze_assertion_components_for_test_v1;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    const LIMIT: usize = 256 * 1024 * 1024;
    const FLOOR: usize = 73;

    fn parity_with_types(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Vec<bool> {
        // These are the real existing component outputs, not fabricated rich tables.
        let graph = projected_loop_cfg_graph_v1(function).unwrap();
        let inventory = assertion_definition_inventory(function).unwrap();
        let mut legacy = SemanticAssertProofsV1::new(types, function).unwrap();
        let expected = legacy.analyze_existing_assertions_v1().unwrap();
        let expected_work = legacy.work;
        drop(legacy);
        assert_eq!(expected.len(), function.blocks().len());
        let assertions = function
            .blocks()
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Assert { .. }
                )
            })
            .count();
        assert!(
            assertions > 0,
            "fixture must exercise real Assert terminators"
        );
        for (block, &decision) in function.blocks().iter().zip(&expected) {
            if !matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ) {
                assert!(!decision);
            }
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0usize;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            analyze_assertion_components_for_test_v1(
                types,
                function,
                &graph,
                &inventory,
                &mut resources,
            )
        };
        let (actual, actual_work) = result.expect("strict fixture analysis must complete");
        assert_eq!(actual, expected, "complete strict/legacy decision mask");
        assert_eq!(
            actual_work, expected_work,
            "exact legacy logical work, not extra resource work"
        );
        assert!(budget.work() >= expected_work);
        assert!(owned > 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
        // The actual mask is gone before the test owner refunds its accepted credits.
        drop(actual);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        expected
    }
    fn parity(function: &SemanticFunctionDeclV1) -> Vec<bool> {
        parity_with_types(&assertion_proof_types(), function)
    }

    #[test]
    fn original_meter_complete_nonzero_masks_and_logical_work_match() {
        for (shape, accepted) in [
            (AssertionProofShape::Complete, true),
            (AssertionProofShape::MissingPath, false),
            (AssertionProofShape::SameSuccessor, false),
            (AssertionProofShape::Reassignment, false),
            (AssertionProofShape::CallDestination, false),
            (AssertionProofShape::Unresolved, false),
            (AssertionProofShape::NarrowCast, false),
            (AssertionProofShape::SignedSource, false),
            (AssertionProofShape::Overflow, false),
        ] {
            assert_eq!(parity(&assertion_proof_function(shape))[6], accepted);
        }
    }

    #[test]
    fn original_meter_checked_chain_full_masks_match_known_results() {
        assert_eq!(
            parity(&checked_arithmetic_chain_function(false)),
            [true, true, false]
        );
        assert_eq!(
            parity(&checked_arithmetic_chain_function(true)),
            [false, true, false]
        );
    }

    #[test]
    fn original_meter_literal_shift_exact_and_hostile_full_masks_match() {
        for (ty, width, shifts) in [
            (SCALAR_TYPE, 32, &[0, 8, 31][..]),
            (U64_TYPE, 64, &[0, 8, 16, 24, 32, 40, 48, 56, 63][..]),
        ] {
            for &shift in shifts {
                for op in [
                    SemanticBinaryOpV1::ShiftLeft,
                    SemanticBinaryOpV1::ShiftRight,
                ] {
                    let f = literal_shift_assert_function(
                        op,
                        shift,
                        shift,
                        width,
                        ty,
                        true,
                        LiteralShiftConditionFixtureV1::Exact,
                    );
                    let decisions = parity(&f);
                    assert!(decisions[1] && decisions[2]);
                }
            }
        }
        for fixture in [
            LiteralShiftConditionFixtureV1::Reversed,
            LiteralShiftConditionFixtureV1::ReassignedCast,
            LiteralShiftConditionFixtureV1::SignedCheckCarrier,
            LiteralShiftConditionFixtureV1::WrongWidthCheckCarrier,
        ] {
            let f = literal_shift_assert_function(
                SemanticBinaryOpV1::ShiftRight,
                8,
                8,
                32,
                SCALAR_TYPE,
                true,
                fixture,
            );
            assert!(!proves_literal_shift_fixture_v1(&f, 1));
            assert!(!proves_literal_shift_fixture_v1(&f, 2));
            // General constant/range reasoning may still prove a different condition.
            // The specialized refusal is not asserted to be the whole evaluator result.
            parity(&f);
        }
        for (op, message, cast, width, ty, expected) in [
            (
                SemanticBinaryOpV1::ShiftRight,
                32,
                32,
                32,
                SCALAR_TYPE,
                true,
            ),
            (SemanticBinaryOpV1::ShiftRight, 8, 8, 64, SCALAR_TYPE, true),
            (SemanticBinaryOpV1::ShiftRight, 8, 7, 32, SCALAR_TYPE, true),
            (SemanticBinaryOpV1::Add, 8, 8, 32, SCALAR_TYPE, true),
            (SemanticBinaryOpV1::ShiftRight, 8, 8, 32, SCALAR_TYPE, false),
            (SemanticBinaryOpV1::ShiftRight, 64, 64, 64, U64_TYPE, true),
        ] {
            let f = literal_shift_assert_function(
                op,
                message,
                cast,
                width,
                ty,
                expected,
                LiteralShiftConditionFixtureV1::Exact,
            );
            assert!(!proves_literal_shift_fixture_v1(&f, 1));
            assert!(!proves_literal_shift_fixture_v1(&f, 2));
            parity(&f);
        }
    }

    #[test]
    fn original_meter_subtraction_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (
                UnsignedSubtractionBoundHostilityV1::ExactGreaterOrEqual,
                true,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::ExactProjectedSourceSameLocal,
                true,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::ExactLessThanFalse,
                true,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::ExactReversedLessOrEqual,
                true,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::ExactReversedGreaterThanFalse,
                true,
            ),
            (UnsignedSubtractionBoundHostilityV1::WrongLeft, false),
            (UnsignedSubtractionBoundHostilityV1::WrongRight, false),
            (UnsignedSubtractionBoundHostilityV1::RedefinedLeft, false),
            (UnsignedSubtractionBoundHostilityV1::RedefinedRight, false),
            (UnsignedSubtractionBoundHostilityV1::EscapedLeft, false),
            (UnsignedSubtractionBoundHostilityV1::EscapedRight, false),
            (
                UnsignedSubtractionBoundHostilityV1::WrongGreaterOrEqualPolarity,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::WrongLessThanPolarity,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::WrongReversedLessOrEqualPolarity,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::WrongReversedGreaterThanPolarity,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::NonDominatingMerge,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::SameBlockRedefinedLeft,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::BranchRedefinedLeft,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::CallDestinationRedefinedLeft,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::EscapedLeftAfterGuard,
                false,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::LoopReauthenticatesAfterRedefinition,
                true,
            ),
            (
                UnsignedSubtractionBoundHostilityV1::BackedgeRedefinesLeftWithoutReauthentication,
                false,
            ),
        ] {
            let (function, at) = unsigned_subtraction_bound_function(case);
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_product_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (StrictProductBoundHostilityV1::Exact, true),
            (StrictProductBoundHostilityV1::ExactGreaterOrEqual, true),
            (
                StrictProductBoundHostilityV1::ExactReversedGreaterThan,
                true,
            ),
            (
                StrictProductBoundHostilityV1::ExactReversedLessOrEqual,
                true,
            ),
            (StrictProductBoundHostilityV1::WrongFactor, false),
            (StrictProductBoundHostilityV1::RedefinedBoundedValue, false),
            (StrictProductBoundHostilityV1::RedefinedFactor, false),
            (StrictProductBoundHostilityV1::NonDominatingBound, false),
            (
                StrictProductBoundHostilityV1::NonDominatingUpperProduct,
                false,
            ),
            (
                StrictProductBoundHostilityV1::OverflowableUpperProduct,
                false,
            ),
            (StrictProductBoundHostilityV1::EqualityGuard, false),
            (StrictProductBoundHostilityV1::NonStrictGuard, false),
        ] {
            let function = strict_product_bound_function(case);
            let at = 3;
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_flat_index_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (
                FlatIndexHostilityV1::Exact(StrictSuccessFormV1::ReversedLessOrEqual),
                true,
            ),
            (
                FlatIndexHostilityV1::Exact(StrictSuccessFormV1::ReversedGreaterThan),
                true,
            ),
            (
                FlatIndexHostilityV1::Exact(StrictSuccessFormV1::GreaterOrEqual),
                true,
            ),
            (
                FlatIndexHostilityV1::Exact(StrictSuccessFormV1::LessThan),
                true,
            ),
            (FlatIndexHostilityV1::ExactSwappedOperands, true),
            (FlatIndexHostilityV1::WrongExtent, false),
            (FlatIndexHostilityV1::RedefinedIndex, false),
            (FlatIndexHostilityV1::RedefinedExtent, false),
            (FlatIndexHostilityV1::RedefinedOffset, false),
            (FlatIndexHostilityV1::RedefinedOffsetAlias, false),
            (FlatIndexHostilityV1::GuardCapturedThenRootRedefined, false),
            (
                FlatIndexHostilityV1::ExpectedCapturedThenRootRedefined,
                false,
            ),
            (
                FlatIndexHostilityV1::MultiHopGuardCapturedThenRootRedefined,
                false,
            ),
            (FlatIndexHostilityV1::RedefinedOuter, false),
            (FlatIndexHostilityV1::EscapedIndex, false),
            (FlatIndexHostilityV1::EscapedExtent, false),
            (FlatIndexHostilityV1::EscapedOffset, false),
            (FlatIndexHostilityV1::EscapedOffsetAlias, false),
            (FlatIndexHostilityV1::EscapedOuter, false),
            (FlatIndexHostilityV1::CallRedefinedIndex, false),
            (FlatIndexHostilityV1::CallRedefinedOffset, false),
            (FlatIndexHostilityV1::CallRedefinedOffsetAlias, false),
            (
                FlatIndexHostilityV1::CallRedefinedRootAfterGuardCapture,
                false,
            ),
            (FlatIndexHostilityV1::NonDominatingIndexBound, false),
            (FlatIndexHostilityV1::NonDominatingOffsetBound, false),
            (FlatIndexHostilityV1::NonDominatingOuterProduct, false),
            (FlatIndexHostilityV1::OverflowableOuterProduct, false),
            (FlatIndexHostilityV1::NonStrictIndexGuard, false),
            (FlatIndexHostilityV1::NonStrictOffsetGuard, false),
            (FlatIndexHostilityV1::MissingAuthenticatedRowProduct, false),
            (FlatIndexHostilityV1::ExactCheckedProjectionOffset, true),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetRedefined,
                false,
            ),
            (FlatIndexHostilityV1::CheckedProjectionOffsetEscaped, false),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetCallRedefined,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetNonDominating,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetDirectUse,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetStaleAlias,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetWrongEdge,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionIndexDifferentUseSite,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetLoopReauthenticated,
                true,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetPostUseRedefinedBackedge,
                false,
            ),
            (
                FlatIndexHostilityV1::CheckedProjectionOffsetPostUseCallRedefinedBackedge,
                false,
            ),
        ] {
            let function = flat_index_function(case);
            let at = 7;
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_nested_flat_index_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (NestedFlatIndexHostilityV1::Exact, true),
            (
                NestedFlatIndexHostilityV1::ExactCheckedProjectionExtent,
                true,
            ),
            (NestedFlatIndexHostilityV1::ExactSwappedOperands, true),
            (NestedFlatIndexHostilityV1::WrongHeadLimit, false),
            (NestedFlatIndexHostilityV1::WrongHeadStride, false),
            (NestedFlatIndexHostilityV1::MissingIndexBound, false),
            (NestedFlatIndexHostilityV1::WrongIndexBound, false),
            (NestedFlatIndexHostilityV1::MissingInnerBound, false),
            (NestedFlatIndexHostilityV1::LaneRangeReachesExtent, false),
            (NestedFlatIndexHostilityV1::MissingOuterProduct, false),
            (NestedFlatIndexHostilityV1::OverflowableOuterProduct, false),
            (NestedFlatIndexHostilityV1::ExtraTailBeyondCapacity, false),
            (
                NestedFlatIndexHostilityV1::MissingRowProductAssertion,
                false,
            ),
            (NestedFlatIndexHostilityV1::MissingBaseAssertion, false),
            (NestedFlatIndexHostilityV1::MissingFirstAssertion, false),
            (
                NestedFlatIndexHostilityV1::StructurallyUnrelatedBaseAdd,
                false,
            ),
            (NestedFlatIndexHostilityV1::RedefinedIndexAfterGuard, false),
            (NestedFlatIndexHostilityV1::RedefinedLaneAfterGuard, false),
            (NestedFlatIndexHostilityV1::EscapedIndex, false),
            (NestedFlatIndexHostilityV1::EscapedLane, false),
        ] {
            let function = nested_flat_index_function(case);
            let at = 8;
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_relational_nested_flat_index_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (RelationalNestedFlatIndexHostilityV1::Exact, true),
            (
                RelationalNestedFlatIndexHostilityV1::ExactSwappedProducts,
                true,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::WrongDimensionUpper,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::WrongDimensionStride,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::MissingInnerBound,
                false,
            ),
            (RelationalNestedFlatIndexHostilityV1::WrongInnerBound, false),
            (
                RelationalNestedFlatIndexHostilityV1::MissingDimensionAssertion,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::MissingOuterWitness,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::MissingInnerProductAssertion,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::MissingBaseAssertion,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::MissingFirstAssertion,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::ExtraTailReachesStride,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::RedefinedInnerAfterGuard,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::RedefinedUpperBetweenDimensionAndGuard,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::RedefinedUpperAfterGuard,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::RedefinedStrideBetweenProducts,
                false,
            ),
            (
                RelationalNestedFlatIndexHostilityV1::RedefinedLaneAfterGuard,
                false,
            ),
            (RelationalNestedFlatIndexHostilityV1::EscapedInner, false),
            (RelationalNestedFlatIndexHostilityV1::EscapedUpper, false),
            (RelationalNestedFlatIndexHostilityV1::EscapedStride, false),
            (RelationalNestedFlatIndexHostilityV1::EscapedLane, false),
        ] {
            let function = relational_nested_flat_index_function(case);
            let at = 9;
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_scaled_remainder_positive_and_hostile_masks_match() {
        for (case, accepted) in [
            (ScaledRemainderHostilityV1::Exact, true),
            (ScaledRemainderHostilityV1::WrongNumerator, false),
            (ScaledRemainderHostilityV1::WrongDivisorExtent, false),
            (ScaledRemainderHostilityV1::WrongDivisorScale, false),
            (ScaledRemainderHostilityV1::UnstableNumerator, false),
            (ScaledRemainderHostilityV1::UnstableExtent, false),
            (ScaledRemainderHostilityV1::UnstableScale, false),
            (ScaledRemainderHostilityV1::MissingSumAssertion, false),
            (ScaledRemainderHostilityV1::MissingHeadAssertion, false),
            (ScaledRemainderHostilityV1::RejectingDivisibilityEdge, false),
            (
                ScaledRemainderHostilityV1::NonDominatingDivisibilityEdge,
                false,
            ),
            (
                ScaledRemainderHostilityV1::MismatchedDivisibilityScale,
                false,
            ),
            (ScaledRemainderHostilityV1::OffsetNotBelowScale, true),
            (ScaledRemainderHostilityV1::ZeroDivisor, false),
        ] {
            let function = scaled_remainder_function(case);
            let at = 4;
            let decisions = parity(&function);
            assert_eq!(
                decisions[at], accepted,
                "selected target Assert case {case:?}"
            );
        }
    }

    #[test]
    fn original_meter_zero_guard_and_subtract_restoration_masks_match() {
        for op in [SemanticBinaryOpV1::Equal, SemanticBinaryOpV1::NotEqual] {
            assert!(parity(&compared_zero_guard_assertion_function(op, 0))[1]);
        }
        assert!(
            !parity(&compared_zero_guard_assertion_function(
                SemanticBinaryOpV1::Equal,
                1
            ))[1]
        );
        for guarded in [true, false] {
            let actual = parity(&guarded_unsigned_subtract_function(guarded));
            assert_eq!(actual[1], guarded);
            assert_eq!(actual[2], guarded);
        }
    }

    #[test]
    fn original_meter_address_escaped_constants_and_checked_results_refuse() {
        let (types, function) = address_escaped_constant_assert_function();
        assert!(!parity_with_types(&types, &function)[0]);
        assert!(!parity(&address_escaped_checked_overflow_assert_function())[0]);
    }

    #[test]
    fn original_meter_quotient_fixture_existing_asserts_only_match() {
        // This does not qualify the direct quotient query in the original anchor.
        // Separate fixed strict-query controls cover that branch.
        for case in [
            QuotientBoundHostilityV1::Exact,
            QuotientBoundHostilityV1::RejectingEdge,
            QuotientBoundHostilityV1::NonDominating,
            QuotientBoundHostilityV1::AliasedNumeratorMutation,
            QuotientBoundHostilityV1::UnstableDivisor,
            QuotientBoundHostilityV1::MismatchedProduct,
        ] {
            parity(&quotient_bound_function(case));
        }
        // OverflowableProduct removes this fixture's sole Assert, so it belongs
        // to the direct-query control, not this nonempty-Assert harness.
    }

    fn alias_component(alias_count: usize, initial: u128) -> SemanticFunctionDeclV1 {
        let mut statements = vec![typed_assignment(
            1,
            BOOL_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(BOOL_TYPE, initial, 1)),
        )];
        let mut locals = vec![
            local(0, BOOL_TYPE, SemanticLocalRoleV1::Return),
            local(1, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
        ];
        for offset in 0..alias_count {
            let destination = u32::try_from(offset + 2).unwrap();
            let source = u32::try_from(offset + 1).unwrap();
            statements.push(typed_assignment(
                destination,
                BOOL_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(source, BOOL_TYPE)),
            ));
            locals.push(local(
                u8::try_from((offset + 2) % 255).unwrap(),
                BOOL_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let terminal = u32::try_from(alias_count + 1).unwrap();
        projection_function_with_locals(
            vec![
                block(
                    207,
                    statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(terminal, BOOL_TYPE),
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(typed_constant(
                            U64_TYPE, 1, 8,
                        )),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(208, vec![], SemanticTerminatorKindV1::Return),
            ],
            locals,
        )
    }

    fn expression_component(depth: usize, expected_value: u128) -> SemanticFunctionDeclV1 {
        let checked_field = |local: u32| {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(local),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE)
                            .unwrap(),
                    ],
                    U64_TYPE,
                )
                .unwrap(),
            )
        };
        let mut statements = vec![typed_assignment(
            1,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
        )];
        let mut locals = vec![
            local(0, U64_TYPE, SemanticLocalRoleV1::Return),
            local(1, U64_TYPE, SemanticLocalRoleV1::Temporary),
        ];
        for offset in 0..depth {
            let destination = u32::try_from(offset + 2).unwrap();
            let left = if offset == 0 {
                typed_operand(1, U64_TYPE)
            } else {
                checked_field(destination - 1)
            };
            statements.push(typed_assignment(
                destination,
                CHECKED_U64_TYPE,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    left,
                    typed_constant(U64_TYPE, 0, 8),
                )),
            ));
            locals.push(local(
                u8::try_from((offset + 2) % 255).unwrap(),
                CHECKED_U64_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let predicate = u32::try_from(depth + 2).unwrap();
        statements.push(typed_assignment(
            predicate,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: checked_field(predicate - 1),
                right: typed_constant(U64_TYPE, expected_value, 8),
            },
        ));
        locals.push(local(209, BOOL_TYPE, SemanticLocalRoleV1::Temporary));
        projection_function_with_locals(
            vec![
                block(
                    209,
                    statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(predicate, BOOL_TYPE),
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(typed_constant(
                            U64_TYPE, 1, 8,
                        )),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(210, vec![], SemanticTerminatorKindV1::Return),
            ],
            locals,
        )
    }

    #[test]
    fn original_meter_deep_alias_worklist_preserves_true_and_false_results() {
        // A bounded strict stress fixture, not a claim of the legacy 32768 maximum.
        for depth in [1, 64, 256] {
            for initial in [0, 1] {
                assert_eq!(
                    parity(&alias_component(depth, initial)),
                    [initial == 1, false]
                );
            }
        }
    }
    #[test]
    fn original_meter_deep_checked_expression_worklist_preserves_exact_results() {
        // These actual checked field-zero paths exercise heap worklists and payload copies.
        // They do not claim the legacy 16384-depth resource envelope.
        for depth in [1, 64, 256] {
            for value in [0, 1] {
                assert_eq!(
                    parity(&expression_component(depth, value)),
                    [value == 0, false]
                );
            }
        }
    }
}
