#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn report_payload_receipt_distinguishes_owned_capacity_from_clone_lengths() {
        let report = PlironSemanticRefinementReportV1::validation_payload_test_report_v1();
        let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let receipt = report.validation_payload_receipt_v1(limits).unwrap();
        let owner_storage = 4 * 3 + 6 * 4 + 3 + 32 + 64;
        assert_eq!(receipt.owner_storage_for_test_v1(), owner_storage);
        let copied_records_and_text = 4 * 2 + 6 + 2 + 5;
        let clone_storage = copied_records_and_text + 1;
        let census = 48 + 11;
        let clone_work = (6 + 2 + 6 + 3 + 3 + 6) + copied_records_and_text + 7;
        let comparison_work = (6 + 2 + 6 + 3) + copied_records_and_text + 7;
        assert_eq!(
            receipt
                .payload_bounds(report.pass(), owner_storage)
                .unwrap(),
            (
                census + 3 * clone_work + 2 * comparison_work + 4 * (3 + 2),
                clone_storage
            ),
        );
        assert!(
            receipt
                .payload_bounds(report.pass(), owner_storage - 1)
                .is_err()
        );
        assert!(
            receipt
                .payload_bounds(KernelCheckPassKindV1::TensorLayout, owner_storage)
                .is_err()
        );
        let clone = report.try_clone_validation_payload_v1().unwrap();
        assert_eq!(clone, report);
        let cloned_receipt = clone.validation_payload_receipt_v1(limits).unwrap();
        assert_eq!(cloned_receipt.owner_storage_for_test_v1(), clone_storage);
        assert!(
            report
                .validation_payload_receipt_v1(ProductionAnalysisResourceLimitsV1::new(
                    census - 1,
                    usize::MAX
                ),)
                .is_err()
        );
    }

    #[test]
    fn report_payload_fallback_retains_census_for_each_nested_findings_owner() {
        let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        for owner in 0..6 {
            let mut report = PlironSemanticRefinementReportV1::validation_payload_test_report_v1();
            let nonempty = owner >= 3;
            match owner % 3 {
                0 => {
                    report.findings.reserve_exact(1);
                    if nonempty {
                        report
                            .findings
                            .push(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
                    }
                }
                1 => report
                    .progress
                    .set_validation_findings_for_test_v1(nonempty),
                _ => report
                    .effect_refinement
                    .set_validation_findings_for_test_v1(nonempty),
            }
            let receipt = report.validation_payload_receipt_v1(limits).unwrap();
            assert!(!receipt.is_exact());
            assert_eq!(
                receipt.payload_bounds(report.pass(), 1000).unwrap(),
                (48 + 9 * 1000, 1000)
            );
            assert!(report.try_clone_validation_payload_v1().is_err());
        }
    }

    #[test]
    fn report_payload_clone_allocation_failure_is_typed_and_retryable() {
        let report = PlironSemanticRefinementReportV1::validation_payload_test_report_v1();
        super::super::pliron_report_payload_receipt::fail_next_payload_allocation_v1();
        let error = report.try_clone_validation_payload_v1().unwrap_err();
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::ReportValidation
        );
        assert_eq!(error.resource, "report payload allocation");
        assert_eq!(report.try_clone_validation_payload_v1().unwrap(), report);
    }

    #[test]
    fn semantic_finding_cardinality_covers_two_paths_and_the_cap_marker() {
        assert_eq!(semantic_finding_cardinalities_v1(0).unwrap(), (0, 0));
        assert_eq!(semantic_finding_cardinalities_v1(1).unwrap(), (2, 2));
        assert_eq!(
            semantic_finding_cardinalities_v1(MAX_PLIRON_SEMANTIC_FINDINGS_V1 / 2).unwrap(),
            (
                MAX_PLIRON_SEMANTIC_FINDINGS_V1,
                MAX_PLIRON_SEMANTIC_FINDINGS_V1,
            )
        );
        assert_eq!(
            semantic_finding_cardinalities_v1(MAX_PLIRON_SEMANTIC_FINDINGS_V1).unwrap(),
            (
                MAX_PLIRON_SEMANTIC_FINDINGS_V1 * 2,
                MAX_PLIRON_SEMANTIC_FINDINGS_V1 + 1,
            )
        );
        assert_eq!(
            semantic_finding_cardinalities_v1(usize::MAX),
            Err(semantic_resource_overflow_v1())
        );
    }

    #[test]
    fn semantic_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 23,
            identifier_bytes: 96,
            max_operation_arity: 7,
            semantic_definitions: 5,
            semantic_refinement_contracts: 7,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_semantic_refinement_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        // S=12 semantic operations, D=5 definitions, K=16,400 node cells,
        // A=24 finding attempts, and Y=8,320 finding-payload cells.
        // work = D(D+1)K + 8S^2 + N + 15S + AY
        // retained = AY + 4D + 6S
        // temporary = DK + S(3*arity+48) + 8S + Y.
        assert_eq!(exact.work_upper_bound(), 693_035);
        assert_eq!(exact.retained_storage_upper_bound(), 199_772);
        assert_eq!(exact.peak_storage_upper_bound(), 291_016);
        assert_eq!(
            preflight_semantic_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_semantic_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn semantic_bound_clean_empty_subset_is_linear_and_storage_free() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 50,
            identifier_bytes: 32_768,
            max_operation_arity: 256,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_semantic_refinement_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(50, 0),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 50);
        assert_eq!(exact.retained_storage_upper_bound(), 0);
        assert_eq!(exact.peak_storage_upper_bound(), 0);
        assert_eq!(
            preflight_semantic_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(49, 0),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                resource: "work upper bound",
            })
        );
    }

    #[test]
    fn semantic_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_semantic_refinement_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    operations: usize::MAX,
                    semantic_definitions: usize::MAX,
                    semantic_refinement_contracts: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(semantic_resource_overflow_v1())
        );
    }

    #[test]
    fn semantic_description_is_iterative_and_bounded() {
        let mut nodes = vec![SemanticNodeV1::Symbol(0)];
        for identity in 0..10_000 {
            nodes.push(SemanticNodeV1::Binary(
                SemanticBinaryKindAttr::Add,
                identity,
                0,
            ));
        }
        let table = SemanticExpressionTableV1 {
            nodes,
            facts: HashMap::new(),
            typed_root_commitments: Vec::new(),
        };
        let description = table.describe(10_000);
        assert_eq!(description.len(), MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
        assert!(description.ends_with("..."));
    }

    #[test]
    fn typed_semantic_description_stops_during_debug_rendering() {
        let scalar =
            SemanticTypedScalarV1::new(dialect_kernel::SemanticScalarKindAttr::Bool, 1).unwrap();
        let mut expression = SemanticTypedExpressionV1::Symbol { symbol: 7, scalar };
        for _ in 0..127 {
            expression = SemanticTypedExpressionV1::Unary {
                operation: dialect_kernel::SemanticTypedUnaryKindAttr::Not,
                scalar,
                operand: Box::new(expression),
            };
        }
        expression
            .validate()
            .expect("valid maximum-depth expression");
        let table = SemanticExpressionTableV1 {
            nodes: vec![SemanticNodeV1::TypedExpression(expression)],
            facts: HashMap::new(),
            typed_root_commitments: Vec::new(),
        };
        let description = table.describe(0);
        assert_eq!(description.len(), MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
        assert!(description.ends_with("..."));
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn mismatch() -> PlironSemanticRefinementFindingV1 {
        PlironSemanticRefinementFindingV1::ExpressionMismatch {
            block: 0,
            operation: 0,
            actual: "s0".to_owned(),
            expected: "s1".to_owned(),
        }
    }

    #[test]
    fn every_semantic_finding_has_the_shared_status() {
        let incomplete = [
            PlironSemanticRefinementFindingV1::BoundsPrerequisiteRejected,
            PlironSemanticRefinementFindingV1::UnresolvedExpression {
                block: 0,
                operation: 0,
                value: "v0".to_owned(),
            },
            PlironSemanticRefinementFindingV1::ResourceLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }
        assert_eq!(mismatch().status(), KernelCheckStatusV1::Rejected);
    }

    #[test]
    fn rejected_semantic_finding_dominates_an_incomplete_finding() {
        let report = PlironSemanticRefinementReportV1 {
            findings: vec![
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block: 0,
                    operation: 0,
                    value: "v0".to_owned(),
                },
                mismatch(),
            ],
            reference_obligations: 1,
            policy_checked_reference_obligations: 0,
            numerical_obligations: 0,
            policy_checked_numerical_obligations: 0,
            collective_contracts: 0,
            policy_checked_collective_contracts: 0,
            typed_root_commitments: Vec::new(),
            numerical_certificates: Vec::new(),
            progress: PlironProgressReportV1::clean(),
            effect_refinement: clean_effect_refinement_report_v1(),
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironSemanticRefinementReportV1 {
                findings: vec![],
                reference_obligations: 0,
                policy_checked_reference_obligations: 0,
                numerical_obligations: 0,
                policy_checked_numerical_obligations: 0,
                collective_contracts: 0,
                policy_checked_collective_contracts: 0,
                typed_root_commitments: Vec::new(),
                numerical_certificates: Vec::new(),
                progress: PlironProgressReportV1::clean(),
                effect_refinement: clean_effect_refinement_report_v1(),
            }
            .status(),
            KernelCheckStatusV1::Clean
        );
    }
}
