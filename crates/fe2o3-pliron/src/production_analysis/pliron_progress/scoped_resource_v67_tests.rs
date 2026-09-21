#[cfg(test)]
mod progress_scoped_resource_v67_tests {
    use super::*;

    fn entry_headers() -> usize {
        use std::mem::size_of;
        (size_of::<Option<pliron::value::Value>>()
            + size_of::<Option<usize>>()
            + size_of::<pliron::value::Value>()
            + 2 * size_of::<pliron::r#type::TypeHandle>())
        .div_ceil(size_of::<usize>())
    }

    #[test]
    fn scoped_progress_has_literal_work_and_peak_boundaries() {
        let cases = [
            (
                ProductionAnalysisInputCensusV1 {
                    blocks: 1,
                    operations: 66,
                    operands: 2,
                    results: 65,
                    attributes: 67,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                1_423,
                1_041,
                1_865,
                1_555,
                1_862,
            ),
            (
                ProductionAnalysisInputCensusV1 {
                    blocks: 2,
                    operations: 3,
                    operands: 4,
                    block_arguments: 2,
                    attributes: 1,
                    successors: 3,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                5_017,
                5_586,
                5_725 + entry_headers(),
                5_029,
                5_722 + entry_headers(),
            ),
        ];
        for (census, work, retained, peak, old_work, old_peak) in cases {
            let exact = preflight_scoped_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            )
            .unwrap();
            assert_eq!(exact.work_upper_bound(), work);
            assert_eq!(exact.retained_storage_upper_bound(), retained);
            assert_eq!(exact.peak_storage_upper_bound(), peak);
            let legacy = preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(old_work, old_peak),
            )
            .unwrap();
            assert_eq!(legacy.work_upper_bound(), old_work);
            assert_eq!(legacy.peak_storage_upper_bound(), old_peak);
            for (limits, resource) in [
                (
                    ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
                    "work upper bound",
                ),
                (
                    ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
                    "peak storage upper bound",
                ),
            ] {
                assert_eq!(
                    preflight_scoped_progress_resource_upper_bound_v1(census, limits),
                    Err(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::Progress,
                        resource,
                    }),
                );
            }
        }
    }

    #[test]
    fn parallel_edge_payload_owner_and_extra_work_have_literal_boundaries() {
        use pliron::value::{DefiningEntity, Value};
        use std::mem::size_of;

        assert!(size_of::<Ptr<Operation>>() <= 2 * size_of::<usize>());
        assert!(size_of::<Ptr<BasicBlock>>() <= 2 * size_of::<usize>());
        assert!(size_of::<DefiningEntity>() <= 3 * size_of::<usize>());
        assert!(size_of::<Value>() <= 4 * size_of::<usize>());
        assert!(size_of::<Vec<Value>>() <= 3 * size_of::<usize>());
        assert!(size_of::<Option<Vec<Value>>>() <= 3 * size_of::<usize>());
        for (operands, block_arguments, base_work, base_peak, work, peak) in [
            (4, 2, 313, 5_706, 5_017, 5_725 + entry_headers()),
            (0, 0, 243, 5_684, 2_739, 5_687 + entry_headers()),
        ] {
            let census = ProductionAnalysisInputCensusV1 {
                blocks: 2,
                operations: 3,
                operands,
                block_arguments,
                attributes: 1,
                successors: 3,
                ..ProductionAnalysisInputCensusV1::default()
            };
            // E*(2+B*E)=24 queries, each 8+24E+11A extra visits.
            // A=0 still pays controls for more than two nullary successors.
            // One extra payload owner costs three header + four*A cells.
            let bound = preflight_scoped_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            )
            .unwrap();
            assert_eq!(
                bound.work_upper_bound(),
                base_work
                    + 24 * (8 + 24 * 3 + 11 * operands)
                    + 12 * (48 + (16 + 4 * block_arguments) * operands)
            );
            assert_eq!(
                bound.peak_storage_upper_bound(),
                base_peak + 3 + 4 * operands + entry_headers()
            );
            for (limits, resource) in [
                (
                    ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
                    "work upper bound",
                ),
                (
                    ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
                    "peak storage upper bound",
                ),
            ] {
                assert_eq!(
                    preflight_scoped_progress_resource_upper_bound_v1(census, limits),
                    Err(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::Progress,
                        resource,
                    }),
                );
            }
        }
    }

    #[test]
    fn scoped_progress_keeps_hard_caps_checked_overflow_and_error_order() {
        for (census, resource) in [
            (
                ProductionAnalysisInputCensusV1 {
                    blocks: MAX_PLIRON_PROGRESS_BLOCKS_V1 + 1,
                    identifier_bytes: usize::MAX,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                "progress structural hard limit",
            ),
            (
                ProductionAnalysisInputCensusV1 {
                    operands: MAX_PLIRON_PROGRESS_OPERANDS_V1,
                    results: MAX_PLIRON_PROGRESS_RESULTS_V1,
                    attributes: MAX_PLIRON_PROGRESS_ATTRIBUTES_V1,
                    block_arguments: MAX_PLIRON_PROGRESS_BLOCK_ARGUMENTS_V1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                "progress work hard limit",
            ),
        ] {
            let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
            assert_eq!(
                preflight_scoped_progress_resource_upper_bound_v1(census, limits),
                Err(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::Progress,
                    resource,
                }),
            );
        }
    }

    #[test]
    fn scoped_progress_structural_visits_keep_the_existing_meter_failure_prefix() {
        let inventory = StructuralInventoryV1 {
            regions: 1,
            blocks: 1,
            operations: 66,
            operands: 2,
            results: 65,
            attributes: 67,
            ..StructuralInventoryV1::default()
        };
        assert_eq!(inventory.structural_work(), Some(202));
        assert_eq!(inventory.verification_work(), 334);
        let limit = MAX_PLIRON_PROGRESS_WORK_UNITS_V1;
        let mut exact = ProgressWorkBudgetV1 {
            work_units: limit - 202,
        };
        assert_eq!(exact.charge(202), Ok(()));
        assert_eq!(exact.work_units, limit);
        let mut denied = ProgressWorkBudgetV1 {
            work_units: limit - 201,
        };
        assert_eq!(
            denied.charge(202),
            Err(PlironProgressFindingV1::ResourceLimitExceeded {
                resource: "work units",
                actual: limit + 1,
                limit,
            })
        );
        assert_eq!(denied.work_units, limit - 201);
        let overflow = StructuralInventoryV1 {
            regions: usize::MAX,
            blocks: 1,
            ..StructuralInventoryV1::default()
        };
        assert_eq!(overflow.structural_work(), None);
        assert_eq!(overflow.verification_work(), usize::MAX);
        let mut empty = ProgressWorkBudgetV1::default();
        assert!(
            empty
                .charge(overflow.structural_work().unwrap_or(usize::MAX))
                .is_err()
        );
        assert_eq!(empty.work_units, 0);
    }

    #[test]
    fn scoped_progress_removes_only_the_duplicate_work_and_retains_all_workspace() {
        for blocks in 0..=4 {
            for operations in 0..=8 {
                for operands in 0..=5 {
                    let census = ProductionAnalysisInputCensusV1 {
                        blocks,
                        operations,
                        operands,
                        results: operations,
                        successors: blocks,
                        attributes: operations,
                        block_arguments: blocks,
                        identifier_bytes: 17,
                        ..ProductionAnalysisInputCensusV1::default()
                    };
                    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
                    let legacy =
                        preflight_progress_resource_upper_bound_v1(census, limits).unwrap();
                    let scoped =
                        preflight_scoped_progress_resource_upper_bound_v1(census, limits).unwrap();
                    assert_eq!(
                        scoped.work_upper_bound() + operands * operations,
                        legacy.work_upper_bound()
                    );
                    assert_eq!(
                        scoped.retained_storage_upper_bound(),
                        legacy.retained_storage_upper_bound()
                    );
                    assert_eq!(
                        scoped.peak_storage_upper_bound(),
                        legacy.peak_storage_upper_bound() + 3
                    );
                }
            }
        }
    }
}
