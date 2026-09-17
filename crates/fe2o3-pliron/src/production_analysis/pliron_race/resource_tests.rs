#[cfg(test)]
mod status_tests {
    use super::*;

    include!("numeric_diagnostics_tests.rs");

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
            static_publication: Vec::new(),
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
        // One effect and two invocations make 24 raw-evaluator queries. Each
        // query visits one definition: 24*8 stack/map operations plus 24*3
        // query lifecycles. The single-query peak has six map-capacity units,
        // three eight-unit frames, and eight invocation decode items.
        // The single symbolic pair also prepays twelve root queries and
        // three comparisons: 12*4+3=51. These queries allocate nothing.
        // Publication inventory adds eight work units and 64 scratch units.
        const EXACT_WORK: usize = 3_075;
        const EXACT_RETAINED: usize = 1_272;
        const EXACT_PEAK: usize = 68_566;
        let exact = race_resource_upper_bound_for_shape_v1(
            census,
            Some((2, 1)),
            None,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((2, 1)),
                None,
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
                Some((2, 1)),
                None,
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
            None,
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
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 4_000_000),
        )
        .unwrap();
        assert_eq!(scalar_only.retained_storage_upper_bound(), PER_FINDING);
        assert!(bound.peak_storage_upper_bound() > scalar_only.peak_storage_upper_bound());
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                Some((64, 1)),
                None,
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
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn race_early_paths_charge_one_finding_and_its_construction() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        for (shape, work) in [
            (None, 1_051_211),
            (Some((0, 1)), 1_051_211),
            (Some((1, 1)), 2_715),
        ] {
            const RETAINED: usize = 1_272;
            // Effect collection (88), four signal/class sets (8), one retained
            // diagnostic and one construction temporary. No exact map/query.
            const PEAK: usize = 88 + 8 + 2 * RETAINED + 64;
            let bound = race_resource_upper_bound_for_shape_v1(
                census,
                shape,
                None,
                ProductionAnalysisResourceLimitsV1::new(work, PEAK),
            )
            .unwrap();
            assert_eq!(bound.work_upper_bound(), work);
            assert_eq!(bound.retained_storage_upper_bound(), RETAINED);
            assert_eq!(bound.peak_storage_upper_bound(), PEAK);
            for limits in [
                ProductionAnalysisResourceLimitsV1::new(work - 1, PEAK),
                ProductionAnalysisResourceLimitsV1::new(work, PEAK - 1),
            ] {
                assert!(
                    race_resource_upper_bound_for_shape_v1(census, shape, None, limits).is_err()
                );
            }
        }
    }

    #[test]
    fn race_unreachable_fallback_does_not_precharge_ordered_finding_vectors() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4_096,
            blocks: 385,
            ranked_accesses: 64,
            identifier_bytes: 32_768,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let bound = race_resource_upper_bound_for_shape_v1(census, None, None, limits).unwrap();
        const PER_FINDING: usize = 3 * 8 + 64 + 1_024 + 160;
        assert_eq!(bound.retained_storage_upper_bound(), PER_FINDING);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            64 * (8 + 16 + 64) + 385 * 11 + 64 * 8 + 2 * PER_FINDING + 64
        );
        let without_text = ProductionAnalysisInputCensusV1 {
            identifier_bytes: 0,
            ..census
        };
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(without_text, None, None, limits),
            Ok(bound),
        );
        let enumerated =
            race_resource_upper_bound_for_shape_v1(census, Some((2, 1)), None, limits).unwrap();
        assert_eq!(
            enumerated.retained_storage_upper_bound(),
            64 * 64 * PER_FINDING
        );
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(without_text, Some((2, 1)), None, limits),
            Ok(enumerated),
        );
    }

    #[test]
    fn race_relation_shape_matches_domain_and_minimum_pair_work_gates() {
        for extents in [
            &[][..],
            &[0],
            &[1],
            &[65_536],
            &[77_791_232],
            &[u64::MAX, 2],
            &[u64::MAX, u64::MAX, u64::MAX],
        ] {
            assert_eq!(
                presburger_invocation_shape_for_resource_v1(extents),
                None,
                "{extents:?}"
            );
        }
        for (extents, expected) in [
            (&[65_537][..], (65_537, 1)),
            (&[262_144][..], (262_144, 1)),
            (&[131_072, 1, 1][..], (131_072, 3)),
        ] {
            assert_eq!(
                presburger_invocation_shape_for_resource_v1(extents),
                Some(expected)
            );
        }
        for extents in [&[262_145][..], &[131_073, 1, 1][..]] {
            assert_eq!(presburger_invocation_shape_for_resource_v1(extents), None);
        }
    }

    #[test]
    fn race_relation_maps_are_temporary_and_pairs_bound_four_walks() {
        let shape = Some((65_537, 1));
        let (work, storage) = presburger_relation_resource_upper_bound_v1(shape, 1).unwrap();
        assert_eq!(work, 65_537 * (16 * 8 * 8 + 128 * 8 + 256));
        assert_eq!(storage, 65_537 * (9 * 8 + 64) + 8 * 8 * 8 + 64 * 8 + 128);
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(shape, 0),
            Ok((0, 0))
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(shape, usize::MAX),
            Ok((work * 3, storage))
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(Some((usize::MAX, 1)), 1),
            Err(race_resource_overflow_v1())
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(Some((1, usize::MAX)), 1),
            Err(race_resource_overflow_v1())
        );
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let bound = race_resource_upper_bound_for_shape_v1(
            census,
            None,
            shape,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        assert_eq!(bound.retained_storage_upper_bound(), 1_272);
        assert_eq!(bound.work_upper_bound(), 1_051_211 + work);
        assert_eq!(bound.peak_storage_upper_bound(), 2_704 + storage);
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Ok(bound)
        );
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "work upper bound",
            })
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn race_remainder_relation_enumerates_beyond_exact_trace_limit() {
        use dialect_kernel::{IndexBinaryKindAttr, IndexBinaryOp, RankedViewType, ReturnOp};
        use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op};

        let context = &mut Context::new();
        dialect_kernel::register_dialect(
            context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(context).unwrap();
        let function_type = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(
            context,
            "remainder_relation_resource".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(context);
        let invocation = InvocationIndexOp::new(context, 0, 65_537);
        let modulus = IndexConstantOp::new(context, 65_537);
        let index = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Remainder,
            invocation.result(context),
            modulus.result(context),
        );
        let memory_type = RankedViewType::new(context, 32, true, vec![65_537]).unwrap();
        let memory =
            RankedViewOp::new_in_space(context, memory_type, vec![], MemorySpaceAttr::Global)
                .unwrap();
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            memory.result(context),
            vec![index.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        for operation in [
            invocation.get_operation(),
            modulus.get_operation(),
            index.get_operation(),
            memory.get_operation(),
            write.get_operation(),
            ret.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_sparse_indices(context, &function);
        let sparse = analyses.sparse_indices().unwrap();
        assert_eq!(static_invocation_shape_for_resource_v1(sparse, None), None);
        assert!(sparse.fact(index.result(context)).affine().is_none());
        assert_eq!(
            presburger_invocation_shape_for_resource_v1(sparse.launch_extents()),
            Some((65_537, 1))
        );
        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert!(report.is_clean(), "{:#?}", report.findings());
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
                None,
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
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(race_resource_overflow_v1())
        );
    }
}
