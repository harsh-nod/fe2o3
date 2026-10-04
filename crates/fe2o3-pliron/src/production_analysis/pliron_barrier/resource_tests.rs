#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn barrier_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 2,
            operations: 3,
            successors: 5,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Work: 3 outer + 256 fixed + 32*3 operations + 512*2 graph
        // + 96*5 graph edges + 5*(20*256) location lifecycle
        // + 8*2*(2+5) worst-case block-map work + 2*(4*256) summary
        // payload + 2*4096 diagnostic rendering.
        // Peak: 5120 retained + 120*2 graph slots + 11*5 edge slots
        // + 6*3 local coordinates + 1024*2 summary payload + 96 fixed
        // + 4*1024 transient summaries + 2*4096 text.
        const EXACT_WORK: usize = 37_811;
        const EXACT_RETAINED: usize = 5_120;
        const EXACT_PEAK: usize = 19_865;
        let exact = preflight_barrier_convergence_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(FALLBACK_BARRIER_EDGE_WORK_V1, 5_120);
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_barrier_convergence_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_barrier_convergence_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn fallback_edge_work_scales_with_hostile_successor_count() {
        let base = preflight_barrier_convergence_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 64,
                operations: 256,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let hostile = preflight_barrier_convergence_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 64,
                operations: 256,
                successors: 1_024,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(
            hostile.work_upper_bound() - base.work_upper_bound(),
            1_024 * (96 + 8 * 64 + 4 * 256 + 4 * 256 + 8 * 256 + 4 * 256)
        );
    }

    #[test]
    fn deep_condensed_fallback_has_a_literal_temporary_boundary() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 512,
            operations: 512,
            successors: 511,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Work additionally reserves 8*512*(512+511) block-map visits.
        // Graph/SCC/control owners: 120*512 + 11*511 + 96.
        // Local coordinates: 6*512; summary payload: 1024*512.
        // Transient summaries/text: 4096+8192; retained report: 5120.
        const EXACT_WORK: usize = 7_667_360;
        const EXACT_PEAK: usize = 611_925;
        let exact = preflight_barrier_convergence_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_barrier_convergence_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::BarrierConvergence,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn combined_fallback_diagnostic_is_utf8_bounded_and_stops_early() {
        use std::cell::Cell;

        struct ManyFragments<'a>(&'a Cell<usize>);
        impl fmt::Display for ManyFragments<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..100_000 {
                    self.0.set(self.0.get() + 1);
                    formatter.write_str("界")?;
                }
                Ok(())
            }
        }

        let writes = Cell::new(0);
        let bounded = bounded_barrier_diagnostic_v1(ManyFragments(&writes));
        assert!(bounded.len() <= MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1);
        assert!(bounded.ends_with("..."));
        assert!(writes.get() < 100_000);

        let path_detail = "界".repeat(4_096);
        let pipeline_detail = "pipeline".repeat(4_096);
        let combined = bounded_barrier_diagnostic_v1(BarrierFallbackDiagnosticV1 {
            trace_failure: &PlironTraceFailureV1::DynamicLaunch { dimension: 7 },
            path_detail: &path_detail,
            epoch_detail: &pipeline_detail,
        });
        assert!(combined.len() <= MAX_PLIRON_BARRIER_DIAGNOSTIC_BYTES_V1);
        assert!(combined.ends_with("..."));
    }

    #[test]
    fn barrier_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_barrier_convergence_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    successors: usize::MAX,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(barrier_resource_overflow_v1())
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn every_barrier_finding_has_the_shared_status() {
        let incomplete = [
            PlironBarrierFindingV1::BoundsPrerequisiteRejected,
            PlironBarrierFindingV1::AnalysisIncomplete {
                detail: "unresolved".to_owned(),
            },
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            PlironBarrierFindingV1::DivergentBarrierTrace {
                first_invocation: vec![0],
                first_trace: vec![(0, 0)],
                second_invocation: vec![1],
                second_trace: vec![],
            },
            PlironBarrierFindingV1::DivergentBarrierPaths {
                first_trace: vec![(0, 0)],
                second_trace: vec![],
            },
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_barrier_finding_dominates_an_incomplete_finding() {
        let report = PlironBarrierReportV1 {
            findings: vec![
                PlironBarrierFindingV1::AnalysisIncomplete {
                    detail: "unresolved".to_owned(),
                },
                PlironBarrierFindingV1::DivergentBarrierPaths {
                    first_trace: vec![(0, 0)],
                    second_trace: vec![],
                },
            ],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironBarrierReportV1 { findings: vec![] }.status(),
            KernelCheckStatusV1::Clean
        );
    }

    #[test]
    fn fallback_barrier_path_event_vectors_are_bounded() {
        let summary = BarrierPathBlockSummaryV1 {
            normal: Some(vec![(0, 0); MAX_FALLBACK_BARRIER_PATH_EVENTS_V1]),
            trapped_prefix: None,
        };
        let error = prepend_barrier_path_v1(&[(1, 0)], summary).unwrap_err();
        assert!(
            matches!(error, BarrierPathFailureV1::Incomplete(detail) if detail.contains("more than 256 events"))
        );
    }

    #[test]
    fn fallback_barrier_max_vectors_cover_comparisons_and_divergence_clones() {
        let full = vec![(0, 0); MAX_FALLBACK_BARRIER_PATH_EVENTS_V1];
        let Ok(prepended) = prepend_barrier_path_v1(
            &[],
            BarrierPathBlockSummaryV1 {
                normal: Some(full.clone()),
                trapped_prefix: Some(full.clone()),
            },
        ) else {
            panic!("maximum valid barrier traces must prepend")
        };
        assert_eq!(prepended.normal.as_deref(), Some(full.as_slice()));
        assert_eq!(prepended.trapped_prefix.as_deref(), Some(full.as_slice()));

        let mut late_mismatch = full.clone();
        *late_mismatch.last_mut().unwrap() = (1, 1);
        let mut complete = BarrierPathBlockSummaryV1 {
            normal: Some(full.clone()),
            trapped_prefix: Some(full.clone()),
        };
        let error = merge_barrier_path_summary_v1(
            &mut complete,
            BarrierPathBlockSummaryV1 {
                normal: Some(full.clone()),
                trapped_prefix: Some(late_mismatch),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BarrierPathFailureV1::Divergent {
                first_trace,
                second_trace,
            } if first_trace.len() == 256 && second_trace.len() == 256
        ));

        let mut final_prefix_mismatch = full.clone();
        *final_prefix_mismatch.last_mut().unwrap() = (2, 2);
        let mut complete = BarrierPathBlockSummaryV1 {
            normal: Some(full.clone()),
            trapped_prefix: None,
        };
        let error = merge_barrier_path_summary_v1(
            &mut complete,
            BarrierPathBlockSummaryV1 {
                normal: Some(full),
                trapped_prefix: Some(final_prefix_mismatch),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BarrierPathFailureV1::Divergent {
                first_trace,
                second_trace,
            } if first_trace.len() == 256 && second_trace.len() == 256
        ));
    }
}
