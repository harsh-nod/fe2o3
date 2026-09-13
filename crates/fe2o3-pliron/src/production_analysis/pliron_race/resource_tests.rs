#[cfg(test)]
mod status_tests {
    use super::*;

    fn witness(
        access: AccessKindAttr,
        atomic_scope: Option<AtomicScopeAttr>,
    ) -> RankedRaceWitnessV1 {
        RankedRaceWitnessV1 {
            location: RankedRaceLocationV1 {
                block: 0,
                operation: 0,
            },
            access,
            invocation: vec![0],
            grid: 0,
            workgroup: Some(0),
            subgroup: Some(0),
            lane: Some(0),
            atomic_scope,
        }
    }

    fn conflict() -> RankedRaceFindingV1 {
        RankedRaceFindingV1::ConflictingEffects {
            view: "v0".to_owned(),
            indices: vec![0],
            first: witness(AccessKindAttr::Write, None),
            second: witness(AccessKindAttr::Read, None),
        }
    }

    #[test]
    fn every_race_finding_has_the_shared_status() {
        let incomplete = [
            RankedRaceFindingV1::BoundsPrerequisiteRejected,
            RankedRaceFindingV1::SparseIndexAnalysisFailed {
                detail: "unresolved".to_owned(),
            },
            RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 },
            RankedRaceFindingV1::LaunchDomainTooLarge {
                invocations: 2,
                limit: 1,
            },
            RankedRaceFindingV1::UnresolvedIndex {
                block: 0,
                operation: 0,
                dimension: 0,
                value: "i".to_owned(),
            },
            RankedRaceFindingV1::EffectInstanceLimitExceeded {
                actual: 2,
                limit: 1,
            },
            RankedRaceFindingV1::FindingLimitExceeded {
                actual: 2,
                limit: 1,
            },
            RankedRaceFindingV1::ExecutionLayoutUnavailable {
                detail: "missing".to_owned(),
            },
            RankedRaceFindingV1::AllocationContractUnavailable {
                detail: "missing".to_owned(),
            },
            RankedRaceFindingV1::HappensBeforeIncomplete {
                view: "v0".to_owned(),
                detail: "missing".to_owned(),
            },
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            conflict(),
            RankedRaceFindingV1::InsufficientAtomicScope {
                view: "v0".to_owned(),
                indices: vec![0],
                first: witness(
                    AccessKindAttr::AtomicWrite,
                    Some(AtomicScopeAttr::Workgroup),
                ),
                second: witness(AccessKindAttr::AtomicRead, Some(AtomicScopeAttr::Workgroup)),
            },
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_race_finding_dominates_an_incomplete_finding() {
        let report = RankedRaceReportV1 {
            findings: vec![RankedRaceFindingV1::BoundsPrerequisiteRejected, conflict()],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(clean().status(), KernelCheckStatusV1::Clean);
    }

    #[test]
    fn effect_pair_inventory_is_charged_before_enumeration() {
        assert!(effect_pair_inventory_fits_budget(1_447));
        assert!(!effect_pair_inventory_fits_budget(1_448));
        assert!(!effect_pair_inventory_fits_budget(usize::MAX));
    }

    #[test]
    fn race_resource_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // One effect and one invocation make 16 raw-evaluator queries. Each
        // query visits one definition: 16*8 stack/map operations plus 16*3
        // query lifecycles. The single-query peak has six map-capacity units,
        // three eight-unit frames, and eight invocation decode items.
        // The single symbolic pair also prepays twelve root queries and
        // three comparisons: 12*4+3=51. These queries allocate nothing.
        const EXACT_WORK: usize = 2_931;
        const EXACT_RETAINED: usize = 1_272;
        const EXACT_PEAK: usize = 68_366;
        let exact = race_resource_upper_bound_for_shape_v1(
            census,
            Some((1, 1)),
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((1, 1)),
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((1, 1)),
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn race_resource_bound_uses_effect_census_and_ordered_finding_pairs() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4_096,
            ranked_accesses: 2,
            allocation_effects: 20,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let bound = race_resource_upper_bound_for_shape_v1(
            census,
            Some((64, 1)),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 4_000_000),
        )
        .unwrap();
        // Witness coordinates have rank eight, names reserve 64 units, and
        // each finding also reserves 1024 diagnostic bytes and 160 fields.
        const PER_FINDING: usize = 3 * 8 + 64 + 1_024 + 160;
        assert_eq!(bound.retained_storage_upper_bound(), 22 * 22 * PER_FINDING);
        let scalar_only = race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: 0,
                allocation_effects: 0,
                ..census
            },
            Some((64, 1)),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 4_000_000),
        )
        .unwrap();
        assert_eq!(scalar_only.retained_storage_upper_bound(), PER_FINDING);
        assert!(bound.peak_storage_upper_bound() > scalar_only.peak_storage_upper_bound());
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((64, 1)),
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound()
                ),
            )
            .is_ok()
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((64, 1)),
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn race_resource_bound_rejects_effect_census_sum_overflow() {
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                ProductionAnalysisInputCensusV1 {
                    ranked_accesses: usize::MAX,
                    allocation_effects: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                Some((1, 1)),
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(race_resource_overflow_v1())
        );
    }

    #[test]
    fn raw_index_evaluator_bound_covers_a_deep_definition_dag() {
        const OPERATIONS: usize = 20_001;
        let (work, temporary) =
            raw_index_evaluation_resource_upper_bound_v1(OPERATIONS, 1, 3).unwrap();
        assert_eq!(work, OPERATIONS * 8 + 3);
        assert_eq!(temporary, OPERATIONS * 6 + (2 * OPERATIONS + 1) * 8 + 3);

        let capped = raw_index_evaluation_resource_upper_bound_v1(
            OPERATIONS,
            MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1,
            3,
        )
        .unwrap();
        assert_eq!(
            capped.0,
            (MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 + 1) * 8
                + MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 * 3
        );
    }

    #[test]
    fn race_resource_bound_rejects_pair_overflow() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: usize::MAX,
            ranked_accesses: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        };
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((1, 1)),
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(race_resource_overflow_v1())
        );
    }
}
