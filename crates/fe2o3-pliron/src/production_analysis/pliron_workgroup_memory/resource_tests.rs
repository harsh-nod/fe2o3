#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn workgroup_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 2,
            operations: 3,
            successors: 7,
            workgroup_ranked_accesses: 1,
            collective_transpose_candidates: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Work: 3*4 inventory + 7*418 edge lifecycle + 2*36 retained
        // summaries + 36 issue projection. Storage: 2,096 retained plus
        // 3*8 + 2*6 + 7*2 + 2*36 + 2*36 temporary.
        const EXACT_WORK: usize = 3_046;
        const EXACT_RETAINED: usize = 2_096;
        const EXACT_PEAK: usize = 2_290;
        let exact = preflight_workgroup_memory_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(COLLECTIVE_TRANSPOSE_EDGE_WORK_V1, 418);
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_workgroup_memory_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_workgroup_memory_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn collective_lifecycle_charges_every_high_fanout_successor() {
        let without_edges = preflight_workgroup_memory_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 64,
                operations: 192,
                collective_transpose_candidates: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let with_edges = preflight_workgroup_memory_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 64,
                operations: 192,
                successors: 512,
                collective_transpose_candidates: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(
            with_edges.work_upper_bound() - without_edges.work_upper_bound(),
            512 * (36 + 98 + 8 * 8 * 3 + (8 * 8 + 1) + 27)
        );
        assert_eq!(
            with_edges.peak_storage_upper_bound() - without_edges.peak_storage_upper_bound(),
            512 * 2
        );
    }

    #[test]
    fn ordinary_lds_does_not_reserve_collective_transpose_cfg_state() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 2,
            operations: 3,
            successors: 7,
            workgroup_ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_workgroup_memory_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(48, 2_120),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 3 * 4 + (8 * 3 + 12));
        assert_eq!(exact.retained_storage_upper_bound(), 2_096);
        assert_eq!(exact.peak_storage_upper_bound(), 2_120);
        assert!(matches!(
            preflight_workgroup_memory_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(47, 2_120),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "work upper bound",
            })
        ));
        assert!(matches!(
            preflight_workgroup_memory_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(48, 2_119),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "peak storage upper bound",
            })
        ));
    }

    #[test]
    fn unavailable_prepared_issue_cache_reserves_one_incomplete_finding() {
        let trace = super::super::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1::from_counts_for_test_v1(
            64, 1, 128,
        );
        let memory_order =
            super::super::pliron_memory_order::preflight_memory_order_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1::default(),
                trace,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            )
            .unwrap();
        let census = ProductionAnalysisInputCensusV1 {
            operations: 3,
            workgroup_ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_prepared_workgroup_memory_resource_upper_bound_v1(
            census,
            memory_order,
            Err(PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
                PlironMemoryOrderFailureV1::VersionLimitExceeded,
            )),
            ProductionAnalysisResourceLimitsV1::new(48, 2_120),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 48);
        assert_eq!(exact.retained_storage_upper_bound(), 2_096);
        assert_eq!(exact.peak_storage_upper_bound(), 2_120);
    }

    #[test]
    fn reserved_collective_without_ranked_access_keeps_its_rejection_slot() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 2,
            operations: 3,
            successors: 7,
            collective_transpose_candidates: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_workgroup_memory_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(3_010, 2_290),
        )
        .unwrap();
        // There is no memory-order issue projection, but the reserved
        // collective still traverses its CFG and may retain one diagnostic.
        assert_eq!(exact.work_upper_bound(), 3 * 4 + 7 * 418 + 2 * 36);
        assert_eq!(exact.retained_storage_upper_bound(), 2_096);
        assert_eq!(exact.peak_storage_upper_bound(), 2_290);
    }

    #[test]
    fn prepared_empty_issue_cache_does_not_reserve_quadratic_issue_output() {
        let trace = super::super::pliron_invocation_trace::ProductionInvocationTraceResourceAdmissionV1::from_counts_for_test_v1(
            64, 1, 128,
        );
        let memory_order =
            super::super::pliron_memory_order::preflight_memory_order_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1::default(),
                trace,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            )
            .unwrap();
        assert_eq!(memory_order.issue_upper_bound(), 4_096);
        let census = ProductionAnalysisInputCensusV1 {
            operations: 3,
            workgroup_ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_prepared_workgroup_memory_resource_upper_bound_v1(
            census,
            memory_order,
            Ok(&[]),
            ProductionAnalysisResourceLimitsV1::new(12, 1_048),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 12);
        assert_eq!(exact.retained_storage_upper_bound(), 1_024);
        assert_eq!(exact.peak_storage_upper_bound(), 1_048);
        assert!(matches!(
            preflight_prepared_workgroup_memory_resource_upper_bound_v1(
                census,
                memory_order,
                Ok(&[]),
                ProductionAnalysisResourceLimitsV1::new(11, 1_048),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "work upper bound",
            })
        ));
        assert!(matches!(
            preflight_prepared_workgroup_memory_resource_upper_bound_v1(
                census,
                memory_order,
                Ok(&[]),
                ProductionAnalysisResourceLimitsV1::new(12, 1_047),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
                resource: "peak storage upper bound",
            })
        ));
    }

    #[test]
    fn workgroup_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_workgroup_memory_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    successors: usize::MAX,
                    collective_transpose_candidates: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(workgroup_resource_overflow_v1())
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn conflict() -> PlironWorkgroupMemoryFindingV1 {
        PlironWorkgroupMemoryFindingV1::ConflictingEffects {
            indices: vec![0],
            first_invocation: vec![0],
            first_block: 0,
            first_operation: 0,
            first_access: AccessKindAttr::Write,
            second_invocation: vec![1],
            second_block: 0,
            second_operation: 0,
            second_access: AccessKindAttr::Read,
        }
    }

    #[test]
    fn every_workgroup_memory_finding_has_the_shared_status() {
        let incomplete = [
            PlironWorkgroupMemoryFindingV1::BoundsPrerequisiteRejected,
            PlironWorkgroupMemoryFindingV1::BarrierPrerequisiteRejected,
            PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: "unresolved".to_owned(),
            },
            PlironWorkgroupMemoryFindingV1::FindingLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                invocation: vec![0],
                block: 0,
                operation: 0,
                indices: vec![0],
            },
            conflict(),
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_workgroup_finding_dominates_an_incomplete_finding() {
        let report = PlironWorkgroupMemoryReportV1 {
            findings: vec![
                PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                    detail: "unresolved".to_owned(),
                },
                conflict(),
            ],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironWorkgroupMemoryReportV1 { findings: vec![] }.status(),
            KernelCheckStatusV1::Clean
        );
    }

    #[test]
    fn collective_transpose_path_summaries_are_bounded_to_the_valid_trace_length() {
        let event = CollectiveTransposePathEventV1::Allocation {
            location: PlironTraceLocationV1 {
                block: 0,
                operation: 0,
            },
            access: AccessKindAttr::Write,
            allocation_origin: GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
            noalias_class: GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
        };
        let summary = CollectiveTransposePathSummaryV1 {
            normal: Some(Vec::new()),
            trapped: Vec::new(),
        };
        let error = prepend_collective_path_v1(&[event; 4], summary).unwrap_err();
        assert!(error.contains("more than 3 events"));
    }

    #[test]
    fn neutral_workgroup_effects_do_not_claim_the_reserved_transpose_lifecycle() {
        let (origin, class) = dialect_kernel::neutral_workgroup_allocation_contract_v1([29; 32]);
        assert!(!is_reserved_collective_transpose_identity_v1(origin, class));
        assert!(is_reserved_collective_transpose_identity_v1(
            GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
            GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
        ));
    }

    #[test]
    fn collective_transpose_trap_paths_may_precede_or_complete_the_normal_trace() {
        let write = CollectiveTransposePathEventV1::Allocation {
            location: PlironTraceLocationV1 {
                block: 0,
                operation: 0,
            },
            access: AccessKindAttr::Write,
            allocation_origin: GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
            noalias_class: GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
        };
        let barrier = CollectiveTransposePathEventV1::Barrier {
            location: PlironTraceLocationV1 {
                block: 1,
                operation: 0,
            },
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
        };
        let read = CollectiveTransposePathEventV1::Allocation {
            location: PlironTraceLocationV1 {
                block: 2,
                operation: 0,
            },
            access: AccessKindAttr::Read,
            allocation_origin: GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
            noalias_class: GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
        };
        let mut merged = CollectiveTransposePathSummaryV1 {
            normal: Some(vec![write, barrier, read]),
            trapped: Vec::new(),
        };
        merge_collective_path_v1(
            &mut merged,
            CollectiveTransposePathSummaryV1 {
                normal: None,
                trapped: vec![Vec::new(), vec![write, barrier, read]],
            },
        )
        .unwrap();
        validate_collective_trap_paths_v1(merged.normal.as_deref().unwrap(), &merged.trapped)
            .unwrap();

        let error = validate_collective_trap_paths_v1(
            merged.normal.as_deref().unwrap(),
            &[vec![write, barrier]],
        )
        .unwrap_err();
        assert!(error.contains("partially executes"));
    }

    #[test]
    fn collective_transpose_max_trace_roster_exercises_late_dedup_matches() {
        let trace = |base: usize| {
            (0..MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1)
                .map(|offset| CollectiveTransposePathEventV1::Allocation {
                    location: PlironTraceLocationV1 {
                        block: 0,
                        operation: base + offset,
                    },
                    access: AccessKindAttr::Write,
                    allocation_origin: GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
                    noalias_class: GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
                })
                .collect::<Vec<_>>()
        };
        let normal = trace(100);
        let traps = (0..MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1)
            .map(|index| trace(index * MAX_COLLECTIVE_TRANSPOSE_PATH_EVENTS_V1))
            .collect::<Vec<_>>();
        let prepended = prepend_collective_path_v1(
            &[],
            CollectiveTransposePathSummaryV1 {
                normal: Some(normal.clone()),
                trapped: traps.clone(),
            },
        )
        .unwrap();
        assert_eq!(
            prepended.trapped.len(),
            MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1
        );

        let late_match = traps.last().unwrap().clone();
        let mut complete = CollectiveTransposePathSummaryV1 {
            normal: Some(normal.clone()),
            trapped: traps,
        };
        merge_collective_path_v1(
            &mut complete,
            CollectiveTransposePathSummaryV1 {
                normal: Some(normal),
                trapped: vec![late_match; MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1],
            },
        )
        .unwrap();
        assert_eq!(
            complete.trapped.len(),
            MAX_COLLECTIVE_TRANSPOSE_TRAP_TRACES_V1
        );
    }
}
