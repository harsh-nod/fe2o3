#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    fn trace_admission() -> ProductionInvocationTraceResourceAdmissionV1 {
        super::super::pliron_invocation_trace::invocation_trace_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 3,
                operations: 11,
                max_operation_arity: 4,
                max_successor_arity: 2,
                ..ProductionAnalysisInputCensusV1::default()
            },
            32,
            3,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap()
    }

    #[test]
    fn ownership_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 3,
            operations: 11,
            identifier_bytes: 48,
            ownership_contracts: 11,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_hierarchical_ownership_resource_upper_bound_v1(
            census,
            Some(trace_admission()),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert!(exact.work_upper_bound() > 0);
        assert!(exact.retained_storage_upper_bound() > 0);
        assert_eq!(
            preflight_hierarchical_ownership_resource_upper_bound_v1(
                census,
                Some(trace_admission()),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_hierarchical_ownership_resource_upper_bound_v1(
                census,
                Some(trace_admission()),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn ownership_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_hierarchical_ownership_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    operations: usize::MAX,
                    ownership_contracts: usize::MAX,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(ownership_resource_overflow_v1())
        );
    }

    #[test]
    fn contract_free_input_charges_only_the_inventory_scan() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4_096,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_hierarchical_ownership_resource_upper_bound_v1(
            census,
            Some(trace_admission()),
            ProductionAnalysisResourceLimitsV1::new(4_096, 0),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 4_096);
        assert_eq!(exact.retained_storage_upper_bound(), 0);
        assert_eq!(exact.peak_storage_upper_bound(), 0);
        assert!(matches!(
            preflight_hierarchical_ownership_resource_upper_bound_v1(
                census,
                Some(trace_admission()),
                ProductionAnalysisResourceLimitsV1::new(4_095, 0),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                resource: "work upper bound",
            })
        ));
    }

    #[test]
    fn unrelated_operations_do_not_multiply_ownership_storage() {
        let small = preflight_hierarchical_ownership_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 11,
                identifier_bytes: 48,
                ownership_contracts: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            Some(trace_admission()),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let large = preflight_hierarchical_ownership_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 4_096,
                identifier_bytes: 48,
                ownership_contracts: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            Some(trace_admission()),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        // One inventory visit and one per-contract lookup are the only terms
        // that depend on unrelated operations.
        assert_eq!(
            large.work_upper_bound() - small.work_upper_bound(),
            2 * (4_096 - 11)
        );
        assert_eq!(
            large.retained_storage_upper_bound(),
            small.retained_storage_upper_bound()
        );
        assert_eq!(
            large.peak_storage_upper_bound(),
            small.peak_storage_upper_bound()
        );
    }
}
