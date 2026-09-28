// Raw component resource/probe controls only. No admitted owner or certificate.
mod original_meter_assertion_resource_v1 {
    use super::*;
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::{
        AssertionFixedProbeResultV1, AssertionFixedProbeV1,
        analyze_assertion_components_for_test_v1, analyze_assertion_fixed_probe_for_test_v1,
    };
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticConstantBytesV1;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    const LIMIT: usize = 256 * 1024 * 1024;
    const FLOOR: usize = 83;

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    struct Observation {
        ok: bool,
        work: usize,
        peak: usize,
        failed_work: bool,
        failed_storage: bool,
        logical: Option<usize>,
        mask_len: Option<usize>,
    }
    fn mask_probe(function: &SemanticFunctionDeclV1, w: usize, p: usize) -> Observation {
        let types = assertion_proof_types();
        let graph = projected_loop_cfg_graph_v1(function).unwrap();
        let inventory = assertion_definition_inventory(function).unwrap();
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, p);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let slot = &budget as *const Budget<'_> as usize;
        let mut owned = 0;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            analyze_assertion_components_for_test_v1(
                &types,
                function,
                &graph,
                &inventory,
                &mut resources,
            )
        };
        let observation = Observation {
            ok: result.is_ok(),
            work: budget.work(),
            peak: budget.peak_storage(),
            failed_work: budget.failed_work().is_some(),
            failed_storage: budget.failed_storage().is_some(),
            logical: result.as_ref().ok().map(|(_, work)| *work),
            mask_len: result.as_ref().ok().map(|(mask, _)| mask.len()),
        };
        // Drop actual complete/partial output and errors before owner-only refund.
        drop(result);
        assert_eq!(&budget as *const Budget<'_> as usize, slot);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + owned);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observation
    }
    #[test]
    fn strict_whole_evaluator_exact_and_one_short_boundaries_are_fail_closed() {
        let f = assertion_proof_function(AssertionProofShape::Complete);
        let exact = mask_probe(&f, LIMIT, LIMIT);
        assert!(exact.ok && exact.logical.unwrap() > 0);
        assert_eq!(exact.mask_len, Some(f.blocks().len()));
        assert_eq!(mask_probe(&f, exact.work, exact.peak), exact);
        let short_work = mask_probe(&f, exact.work - 1, exact.peak);
        assert!(!short_work.ok && short_work.failed_work && !short_work.failed_storage);
        assert_eq!((short_work.mask_len, short_work.logical), (None, None));
        let short_storage = mask_probe(&f, exact.work, exact.peak - 1);
        assert!(!short_storage.ok && !short_storage.failed_work && short_storage.failed_storage);
        assert_eq!(
            (short_storage.mask_len, short_storage.logical),
            (None, None)
        );
    }
    #[test]
    fn partial_frame_table_and_evaluator_refusals_never_return_a_partial_mask() {
        let f = assertion_proof_function(AssertionProofShape::Complete);
        let exact = mask_probe(&f, LIMIT, LIMIT);
        for w in [0, 16, 64, exact.work / 2, exact.work - 1] {
            let denied = mask_probe(&f, w, LIMIT);
            assert!(!denied.ok && denied.failed_work);
            assert_eq!((denied.mask_len, denied.logical), (None, None));
        }
        for p in [FLOOR, FLOOR + 1, exact.peak / 2, exact.peak - 1] {
            let denied = mask_probe(&f, LIMIT, p);
            assert!(!denied.ok && denied.failed_storage);
            assert_eq!((denied.mask_len, denied.logical), (None, None));
        }
        // Reusing the immutable input after partial refusals gives the same result.
        assert_eq!(mask_probe(&f, exact.work, exact.peak), exact);
    }
    #[test]
    fn already_sticky_original_work_or_storage_refusal_precedes_any_reservation() {
        let f = assertion_proof_function(AssertionProofShape::Complete);
        let types = assertion_proof_types();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let inventory = assertion_definition_inventory(&f).unwrap();
        for storage in [false, true] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            let original = budget.work_ledger_identity_v1();
            let mut owned = 0;
            let result = {
                let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
                analyze_assertion_components_for_test_v1(
                    &types,
                    &f,
                    &graph,
                    &inventory,
                    &mut resources,
                )
            };
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    )
                ))
            ));
            assert_eq!(
                (
                    owned,
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage()
                ),
                (0, 0, FLOOR, FLOOR)
            );
            assert!(budget.work_ledger_identity_v1() == original);
        }
    }
    #[test]
    fn strict_component_constructor_rejects_unmetered_or_mismatched_tables() {
        let f = assertion_proof_function(AssertionProofShape::Complete);
        let types = assertion_proof_types();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let mut inventory = assertion_definition_inventory(&f).unwrap();
        assert!(
            analyze_assertion_components_for_test_v1(
                &types,
                &f,
                &graph,
                &inventory,
                &mut PreparationResourcesV1::unmetered()
            )
            .is_err()
        );
        inventory.counts.pop();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            analyze_assertion_components_for_test_v1(&types, &f, &graph, &inventory, &mut resources)
        };
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "prepared assertion source tables differ from the exact function"
            ))
        ));
        drop(result);
        assert!(owned > 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    fn direct_probe(
        types: &[SemanticTypeDeclV1],
        f: &SemanticFunctionDeclV1,
        probe: AssertionFixedProbeV1<'_>,
    ) -> (bool, usize) {
        let graph = projected_loop_cfg_graph_v1(f).unwrap();
        let inventory = assertion_definition_inventory(f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            analyze_assertion_fixed_probe_for_test_v1(
                types,
                f,
                &graph,
                &inventory,
                probe,
                &mut resources,
            )
        }
        .unwrap();
        // Test results have owned payloads. Do not refund while returning them:
        // the test owner retains all credits until the caller's comparison ends.
        // This helper cannot transfer a live metered payload beyond its Budget.
        match result {
            (AssertionFixedProbeResultV1::Flat(value), logical) => {
                assert_eq!(budget.storage(), FLOOR + owned);
                assert!(budget.work_ledger_identity_v1() == identity);
                budget.release_storage(owned).unwrap();
                (value, logical)
            }
            (AssertionFixedProbeResultV1::Quotient(value), logical) => {
                // Compare complete actual value here, then return a scalar-only
                // observation represented by Flat. No owned factor escapes.
                let mut old = SemanticAssertProofsV1::new(types, f).unwrap();
                let numerator = typed_operand(1, U64_TYPE);
                let divisor = typed_operand(2, U64_TYPE);
                let use_site = ScalarAssignmentSiteV1 {
                    block: 3,
                    statement: f.blocks()[3].statements().len() - 1,
                };
                let expected = old
                    .quotient_strict_product_upper_bound_v1(&numerator, &divisor, use_site)
                    .unwrap();
                assert_eq!(logical, old.work);
                assert_eq!(
                    value.as_ref().map(|v| v.maximum),
                    expected.as_ref().map(|v| v.maximum)
                );
                assert_eq!(
                    value.as_ref().map(|v| v.factor_use),
                    expected.as_ref().map(|v| v.factor_use)
                );
                if let (Some(actual), Some(expected)) = (&value, &expected) {
                    assert!(same_semantic_operand_value_v1(
                        &actual.factor,
                        &expected.factor
                    ));
                }
                let present = value.is_some();
                drop(value);
                drop(expected);
                drop(old);
                assert_eq!(budget.storage(), FLOOR + owned);
                assert!(budget.work_ledger_identity_v1() == identity);
                budget.release_storage(owned).unwrap();
                (present, logical)
            }
        }
    }
    #[test]
    fn real_direct_quotient_probe_matches_complete_legacy_bound_and_hostile_refusals() {
        let types = assertion_proof_types();
        for (hostility, accepted) in [
            (QuotientBoundHostilityV1::Exact, true),
            (QuotientBoundHostilityV1::RejectingEdge, false),
            (QuotientBoundHostilityV1::NonDominating, false),
            (QuotientBoundHostilityV1::AliasedNumeratorMutation, false),
            (QuotientBoundHostilityV1::UnstableDivisor, false),
            (QuotientBoundHostilityV1::MismatchedProduct, false),
            (QuotientBoundHostilityV1::OverflowableProduct, false),
        ] {
            let f = quotient_bound_function(hostility);
            let numerator = typed_operand(1, U64_TYPE);
            let divisor = typed_operand(2, U64_TYPE);
            let (actual, _) = direct_probe(
                &types,
                &f,
                AssertionFixedProbeV1::Quotient {
                    numerator: &numerator,
                    divisor: &divisor,
                    use_site: ScalarAssignmentSiteV1 {
                        block: 3,
                        statement: f.blocks()[3].statements().len() - 1,
                    },
                },
            );
            assert_eq!(actual, accepted);
        }
    }
    #[test]
    fn real_recursive_flat_path_matches_legacy_at_exact_and_zero_depth() {
        let types = assertion_proof_types();
        let f = nested_flat_index_function(NestedFlatIndexHostilityV1::Exact);
        let site = ScalarAssignmentSiteV1 {
            block: 8,
            statement: 1,
        };
        let SemanticStatementKindV1::Assign(a) = f.blocks()[8].statements()[1].kind() else {
            panic!("fixture changed")
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = a.value().kind() else {
            panic!("fixture changed")
        };
        for remaining in [0, 8] {
            let offsets = vec![(checked.right().clone(), site)];
            let mut old = SemanticAssertProofsV1::new(&types, &f).unwrap();
            let mut old_offsets = offsets.clone();
            let expected = old
                .proves_strictly_bounded_unsigned_flat_index_path_v1(
                    checked.left(),
                    site,
                    site,
                    U64_TYPE,
                    &mut old_offsets,
                    remaining,
                )
                .unwrap();
            if remaining == 0 {
                assert!(!expected);
            }
            let (actual, logical) = direct_probe(
                &types,
                &f,
                AssertionFixedProbeV1::FlatPath {
                    operand: checked.left(),
                    operand_use: site,
                    sum_site: site,
                    scalar_type: U64_TYPE,
                    offsets: &offsets,
                    remaining,
                },
            );
            assert_eq!(actual, expected);
            assert_eq!(logical, old.work);
        }
    }
    #[test]
    fn real_nested_type_boundary_probes_match_legacy() {
        let types = assertion_proof_types();
        for (left, right) in [(I32_TYPE, I32_TYPE), (U64_TYPE, BOOL_TYPE)] {
            let f = projection_function_with_locals(
                vec![block(219, vec![], SemanticTerminatorKindV1::Return)],
                vec![
                    local(219, left, SemanticLocalRoleV1::Return),
                    local(220, left, SemanticLocalRoleV1::Argument(0)),
                    local(221, right, SemanticLocalRoleV1::Argument(1)),
                ],
            );
            let checked = SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                typed_operand(1, left),
                typed_operand(2, right),
            );
            let site = ScalarAssignmentSiteV1 {
                block: 0,
                statement: 0,
            };
            let mut old = SemanticAssertProofsV1::new(&types, &f).unwrap();
            let expected = old
                .proves_strictly_bounded_unsigned_nested_flat_index_v1(&checked, site)
                .unwrap();
            assert!(!expected);
            let (actual, logical) = direct_probe(
                &types,
                &f,
                AssertionFixedProbeV1::Nested {
                    checked: &checked,
                    sum_site: site,
                },
            );
            assert_eq!(actual, expected);
            assert_eq!(logical, old.work);
        }
    }
    #[test]
    fn fixed_unwind_after_real_mask_drops_evaluator_before_owner_refund() {
        let f = assertion_proof_function(AssertionProofShape::Complete);
        let types = assertion_proof_types();
        let graph = projected_loop_cfg_graph_v1(&f).unwrap();
        let inventory = assertion_definition_inventory(&f).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let original = budget.work_ledger_identity_v1();
        let mut owned = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            analyze_assertion_fixed_probe_for_test_v1(
                &types,
                &f,
                &graph,
                &inventory,
                AssertionFixedProbeV1::PanicAfterRealMask,
                &mut resources,
            )
        }));
        assert!(result.is_err());
        drop(result);
        assert!(owned > 0 && budget.work() > 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        assert!(budget.work_ledger_identity_v1() == original);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    fn bytes_function(count: usize) -> SemanticFunctionDeclV1 {
        let bytes = SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![7; count]).unwrap()),
        ));
        projection_function_with_locals(
            vec![
                block(
                    210,
                    vec![typed_assignment(
                        1,
                        BOOL_TYPE,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: bytes.clone(),
                            right: typed_constant(SCALAR_TYPE, 9, 4),
                        },
                    )],
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(1, BOOL_TYPE),
                        expected: false,
                        message: SemanticAssertMessageV1::DivisionByZero(bytes),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(211, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(210, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(211, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }
    #[test]
    fn real_expression_payload_copies_pay_actual_byte_storage_before_clone() {
        // Raw unsupported byte constants deliberately remain unproved. This is
        // payload accounting through the real binary task, not source admission.
        let types = assertion_proof_types();
        let small = bytes_function(1);
        let large = bytes_function(8192);
        for f in [&small, &large] {
            assert_eq!(
                SemanticAssertProofsV1::analyze(&types, f).unwrap(),
                [false; 2]
            );
        }
        let low = mask_probe(&small, LIMIT, LIMIT);
        let high = mask_probe(&large, LIMIT, LIMIT);
        assert!(low.ok && high.ok);
        assert!(high.peak >= low.peak + 8191);
        assert!(high.work >= low.work + 8191);
        assert_eq!(high.logical, low.logical);
        assert!(!mask_probe(&large, high.work, high.peak - 1).ok);
    }
}
