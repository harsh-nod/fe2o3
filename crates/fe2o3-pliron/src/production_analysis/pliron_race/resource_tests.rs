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
        // One effect and two invocations make 24 raw-evaluator queries. Each
        // query visits one definition: 24*8 stack/map operations plus 24*3
        // query lifecycles. The single-query peak has six map-capacity units,
        // three eight-unit frames, and eight invocation decode items.
        // The single symbolic pair also prepays twelve root queries and
        // three comparisons: 12*4+3=51. These queries allocate nothing.
        const EXACT_WORK: usize = 3_067;
        const EXACT_RETAINED: usize = 1_272;
        const EXACT_PEAK: usize = 68_502;
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
            (None, 1_051_203),
            (Some((0, 1)), 1_051_203),
            (Some((1, 1)), 2_707),
        ] {
            const RETAINED: usize = 1_272;
            // Effect collection (88), four signal/class sets (8), one retained
            // diagnostic and one construction temporary. No exact map/query.
            const PEAK: usize = 88 + 8 + 2 * RETAINED;
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
        const PER_FINDING: usize = 3 * 8 + 32_768 + 64 + 1_024 + 160;
        assert_eq!(bound.retained_storage_upper_bound(), PER_FINDING);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            64 * (8 + 16 + 32_768 + 64) + 385 * 11 + 64 * 8 + 2 * PER_FINDING
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(census, Some((2, 1)), None, limits).is_err()
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
        assert_eq!(bound.work_upper_bound(), 1_051_203 + work);
        assert_eq!(bound.peak_storage_upper_bound(), 2_640 + storage);
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

#[cfg(test)]
mod numeric_preflight_tests {
    use super::*;
    use std::{collections::BTreeMap, io};

    fn small_numbers() -> RaceResourcePreflightNumbersV1 {
        calculate_race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 1,
                ranked_accesses: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            Some((2, 1)),
            None,
        )
        .unwrap()
    }

    fn fields(bytes: &[u8]) -> BTreeMap<&str, usize> {
        let text = std::str::from_utf8(bytes).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.ends_with('\n'));
        let mut words = text.split_whitespace();
        assert_eq!(words.next(), Some("RACE_RESOURCE_PREFLIGHT_V1"));
        assert_eq!(words.next(), Some("stage=local_require"));
        assert_eq!(words.next(), Some("units=logical"));
        let mut result = BTreeMap::new();
        for word in words {
            let (key, value) = word.split_once('=').unwrap();
            assert!(result.insert(key, value.parse().unwrap()).is_none());
        }
        assert_eq!(result.len(), 40);
        result
    }

    #[test]
    fn race_numeric_preflight_exposes_existing_small_calculation_terms() {
        let numbers = small_numbers();
        let per_finding = 3 * 8 + usize::BITS as usize + 1_024 + 160;
        assert_eq!(numbers.census.operations, 1);
        assert_eq!(numbers.effects, 1);
        assert_eq!(numbers.effect_pairs, 1);
        assert_eq!(numbers.pairs, 1);
        assert_eq!(numbers.rank, 8);
        assert_eq!(numbers.potential_effect_instances, 2);
        assert_eq!(numbers.charged_effect_instances, 2);
        assert_eq!(numbers.retained_effect_instances, 2);
        assert_eq!(numbers.retained_finding_count, 1);
        assert_eq!(numbers.name_storage, usize::BITS as usize);
        assert_eq!(numbers.per_finding_storage, per_finding);
        assert_eq!(numbers.raw_evaluation_work, 24 * 8 + 24 * 3);
        assert_eq!(numbers.raw_evaluation_temporary, 6 + 3 * 8 + 8);
        assert_eq!(numbers.symbolic_work, 8 * 8 + 16);
        assert_eq!(numbers.presburger_work, 0);
        assert_eq!(numbers.presburger_temporary, 0);
        assert_eq!(numbers.work, 32 + 80 + 51 + 2 * 48 + 264);
        assert_eq!(numbers.effect_state, 8 + 16 + usize::BITS as usize);
        assert_eq!(numbers.address_state, 2 * (8 * 9 + 64));
        assert_eq!(numbers.attempted_finding, per_finding);
        assert_eq!(numbers.conflict_class_storage, 4_097 * 16);
        assert_eq!(
            numbers.temporary,
            numbers.effect_state + 272 + per_finding + 4_097 * 16 + 8 + 38
        );
        assert_eq!(
            numbers.bound.work_upper_bound(),
            numbers.work + 2 * per_finding
        );
        assert_eq!(numbers.bound.retained_storage_upper_bound(), per_finding);
        assert_eq!(
            numbers.bound.peak_storage_upper_bound(),
            per_finding + numbers.temporary
        );

        let mut output = Vec::new();
        let limits = ProductionAnalysisResourceLimitsV1::new(19, 23);
        write_race_resource_preflight_v1(true, &mut output, &numbers, limits);
        let fields = fields(&output);
        let core_work = 32 + 80 + 51 + 2 * 48 + 264;
        let temporary = (8 + 16 + usize::BITS as usize)
            + 2 * (8 * 9 + 64)
            + per_finding
            + 4_097 * 16
            + 8
            + (6 + 3 * 8 + 8);
        assert_eq!(
            fields,
            BTreeMap::from([
                ("blocks", 0),
                ("operations", 1),
                ("successors", 0),
                ("ranked_accesses", 1),
                ("allocation_effects", 0),
                ("identifier_bytes", 0),
                ("canonical_bytes", 0),
                ("static_present", 1),
                ("static_invocations", 2),
                ("static_rank", 1),
                ("presburger_present", 0),
                ("presburger_invocations", 0),
                ("presburger_rank", 0),
                ("effects", 1),
                ("effect_pairs", 1),
                ("pairs", 1),
                ("rank", 8),
                ("potential_instances", 2),
                ("charged_instances", 2),
                ("retained_instances", 2),
                ("finding_count", 1),
                ("name_storage", usize::BITS as usize),
                ("per_finding_storage", per_finding),
                ("core_work", core_work),
                ("raw_work", 24 * 8 + 24 * 3),
                ("presburger_work", 0),
                ("symbolic_work", 8 * 8 + 16),
                ("effect_state", 8 + 16 + usize::BITS as usize),
                ("address_state", 2 * (8 * 9 + 64)),
                ("attempted_finding", per_finding),
                ("conflict_class_storage", 4_097 * 16),
                ("raw_temporary", 6 + 3 * 8 + 8),
                ("presburger_temporary", 0),
                ("temporary", temporary),
                ("work", core_work + 2 * per_finding),
                ("retained", per_finding),
                ("peak", per_finding + temporary),
                ("remaining_work", 19),
                ("remaining_peak", 23),
                ("word_bits", usize::BITS as usize),
            ])
        );
        assert_eq!(fields["core_work"], numbers.work);
        assert_eq!(fields["work"], numbers.bound.work_upper_bound());
        assert_eq!(fields["retained"], per_finding);
        assert_eq!(fields["peak"], per_finding + numbers.temporary);
        assert_eq!(fields["remaining_work"], 19);
        assert_eq!(fields["remaining_peak"], 23);
        assert_eq!(fields["word_bits"], usize::BITS as usize);
    }

    #[test]
    fn race_numeric_preflight_observes_exact_limits_before_unchanged_admission() {
        let numbers = small_numbers();
        let work = numbers.bound.work_upper_bound();
        let peak = numbers.bound.peak_storage_upper_bound();
        for (work_limit, peak_limit, expected_resource) in [
            (work, peak, None),
            (work - 1, peak, Some("work upper bound")),
            (work, peak - 1, Some("peak storage upper bound")),
            (work - 1, peak - 1, Some("work upper bound")),
        ] {
            let limits = ProductionAnalysisResourceLimitsV1::new(work_limit, peak_limit);
            let mut calls = 0;
            let mut output = Vec::new();
            let result = finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                calls += 1;
                assert_eq!(actual, limits);
                assert_eq!(seen.bound, numbers.bound);
                write_race_resource_preflight_v1(true, &mut output, seen, actual);
            });
            assert_eq!(calls, 1);
            let fields = fields(&output);
            assert_eq!(fields["remaining_work"], work_limit);
            assert_eq!(fields["remaining_peak"], peak_limit);
            assert_eq!(fields["work"], work);
            assert_eq!(fields["peak"], peak);
            match expected_resource {
                None => assert_eq!(result, Ok(numbers.bound)),
                Some(resource) => assert_eq!(
                    result,
                    Err(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                        resource,
                    })
                ),
            }
        }
    }

    struct RefusingWriter {
        writes: usize,
    }

    impl io::Write for RefusingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            Err(io::Error::other("component diagnostic sink failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            panic!("numeric diagnostic must not flush the writer")
        }
    }

    struct UntouchedWriter;

    impl io::Write for UntouchedWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            panic!("disabled numeric diagnostic touched the writer")
        }

        fn flush(&mut self) -> io::Result<()> {
            panic!("disabled numeric diagnostic flushed the writer")
        }
    }

    #[test]
    fn race_numeric_preflight_writer_failure_and_disabled_output_preserve_results() {
        let numbers = small_numbers();
        for limits in [
            ProductionAnalysisResourceLimitsV1::new(
                numbers.bound.work_upper_bound(),
                numbers.bound.peak_storage_upper_bound(),
            ),
            ProductionAnalysisResourceLimitsV1::new(0, 0),
            ProductionAnalysisResourceLimitsV1::new(numbers.bound.work_upper_bound(), 0),
        ] {
            let expected = limits.require(
                ProductionAnalysisResourcePhaseV1::RaceFreedom,
                numbers.bound,
            );
            let mut writer = RefusingWriter { writes: 0 };
            let result = finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                write_race_resource_preflight_v1(true, &mut writer, seen, actual);
            });
            assert_eq!(writer.writes, 1);
            assert_eq!(result, expected);
            assert_eq!(
                finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                    write_race_resource_preflight_v1(false, &mut UntouchedWriter, seen, actual);
                }),
                expected
            );
        }
    }

    #[test]
    fn race_numeric_preflight_shapes_and_whole_effect_census_stay_distinct() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 3,
            blocks: 2,
            ranked_accesses: 1,
            allocation_effects: 2,
            identifier_bytes: 31,
            canonical_bytes: 47,
            ..ProductionAnalysisInputCensusV1::default()
        };
        for shape in [None, Some((0, 1)), Some((1, 1)), Some((2, 1))] {
            let numbers =
                calculate_race_resource_upper_bound_for_shape_v1(census, shape, Some((65_537, 1)))
                    .unwrap();
            let mut output = Vec::new();
            write_race_resource_preflight_v1(
                true,
                &mut output,
                &numbers,
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            let fields = fields(&output);
            assert_eq!(fields["static_present"], usize::from(shape.is_some()));
            assert_eq!(
                fields["static_invocations"],
                shape.map_or(0, |value| value.0)
            );
            assert_eq!(fields["static_rank"], shape.map_or(0, |value| value.1));
            assert_eq!(fields["presburger_present"], 1);
            assert_eq!(fields["presburger_invocations"], 65_537);
            assert_eq!(fields["presburger_rank"], 1);
            assert_eq!(fields["ranked_accesses"], 1);
            assert_eq!(fields["allocation_effects"], 2);
            assert_eq!(fields["effects"], 3);
            assert_eq!(fields["effect_pairs"], 6);
            assert_eq!(fields["identifier_bytes"], 31);
            assert_eq!(fields["canonical_bytes"], 47);
        }
    }

    #[test]
    fn race_numeric_preflight_overflow_has_no_complete_observation() {
        for census in [
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: usize::MAX,
                allocation_effects: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
            ProductionAnalysisInputCensusV1 {
                operations: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
        ] {
            let result = finish_race_resource_preflight_v1(
                calculate_race_resource_upper_bound_for_shape_v1(census, Some((1, 1)), None),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                |_, _| panic!("overflow emitted a complete numeric record"),
            );
            assert_eq!(result, Err(race_resource_overflow_v1()));
        }
    }

    #[test]
    fn race_numeric_preflight_fixed_roster_has_bounded_maximum_width_output() {
        // Deliberately synthetic writer values, not an admitted bound or census.
        let maximum = usize::MAX;
        let numbers = RaceResourcePreflightNumbersV1 {
            census: ProductionAnalysisInputCensusV1 {
                blocks: maximum,
                operations: maximum,
                successors: maximum,
                ranked_accesses: maximum,
                allocation_effects: maximum,
                identifier_bytes: maximum,
                canonical_bytes: maximum,
                ..ProductionAnalysisInputCensusV1::default()
            },
            invocation_shape: Some((maximum, maximum)),
            presburger_shape: Some((maximum, maximum)),
            effects: maximum,
            effect_pairs: maximum,
            pairs: maximum,
            rank: maximum,
            potential_effect_instances: maximum,
            charged_effect_instances: maximum,
            retained_effect_instances: maximum,
            retained_finding_count: maximum,
            name_storage: maximum,
            per_finding_storage: maximum,
            work: maximum,
            raw_evaluation_work: maximum,
            presburger_work: maximum,
            symbolic_work: maximum,
            effect_state: maximum,
            address_state: maximum,
            attempted_finding: maximum,
            conflict_class_storage: maximum,
            raw_evaluation_temporary: maximum,
            presburger_temporary: maximum,
            temporary: maximum,
            bound: ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::RaceFreedom,
                maximum,
                maximum,
                0,
            )
            .unwrap(),
        };
        let mut output = Vec::new();
        write_race_resource_preflight_v1(
            true,
            &mut output,
            &numbers,
            ProductionAnalysisResourceLimitsV1::new(maximum, maximum),
        );
        let fields = fields(&output);
        let numeric_bytes: usize = fields.values().map(|value| value.to_string().len()).sum();
        const FIXED_BYTES: usize = 659;
        assert_eq!(output.len() - numeric_bytes, FIXED_BYTES);
        assert!(output.len() <= FIXED_BYTES + 40 * usize::BITS as usize);
        assert_eq!(fields["remaining_work"], maximum);
        assert_eq!(fields["remaining_peak"], maximum);
        assert_eq!(fields["word_bits"], usize::BITS as usize);
    }
}
