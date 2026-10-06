use super::*;

fn numbers(launch_rank: usize) -> RaceResourcePreflightNumbersV1 {
    calculate_race_resource_upper_bound_for_shape_v1(
        ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 2,
            ..Default::default()
        },
        component_race_names_v1(),
        Some((4, launch_rank)),
        None,
    )
    .unwrap()
}

#[test]
fn witness_rank_refines_only_address_storage_for_rank_one_three_eight_and_nine() {
    for launch_rank in [1, 3, 8, 9] {
        let actual = numbers(launch_rank);
        let key_rank = launch_rank.max(MAX_RANKED_MEMORY_RANK);
        let previous = numbers(key_rank);
        let saving = actual.retained_effect_instances * 8 * (key_rank - launch_rank);
        assert_eq!(actual.rank, key_rank);
        assert_eq!(
            actual.address_state,
            actual.retained_effect_instances * (key_rank + 8 * launch_rank + 64)
        );
        assert_eq!(previous.address_state - actual.address_state, saving);
        assert_eq!(previous.temporary - actual.temporary, saving);
        assert_eq!(
            previous.bound.peak_storage_upper_bound() - actual.bound.peak_storage_upper_bound(),
            saving
        );
        assert_eq!(
            actual.bound.work_upper_bound(),
            previous.bound.work_upper_bound()
        );
        assert_eq!(
            actual.bound.retained_storage_upper_bound(),
            previous.bound.retained_storage_upper_bound()
        );
        assert_eq!(actual.effect_state, previous.effect_state);
        assert_eq!(actual.effect_pairs, previous.effect_pairs);
        assert_eq!(
            actual.retained_finding_count,
            previous.retained_finding_count
        );
        assert_eq!(actual.per_finding_storage, previous.per_finding_storage);
        assert_eq!(actual.attempted_finding, previous.attempted_finding);
        assert_eq!(
            actual.conflict_class_storage,
            previous.conflict_class_storage
        );
        assert_eq!(actual.raw_evaluation_work, previous.raw_evaluation_work);
        assert_eq!(
            actual.raw_evaluation_temporary,
            previous.raw_evaluation_temporary
        );
        assert_eq!(actual.presburger_work, previous.presburger_work);
        assert_eq!(actual.presburger_temporary, previous.presburger_temporary);
    }
}

#[test]
fn witness_coordinates_follow_launch_rank_not_ranked_address_rank() {
    for launch_rank in [1, 3, 8, 9] {
        let coordinates = decode_invocation(0, &vec![2; launch_rank]);
        assert_eq!(coordinates.len(), launch_rank);
        let mut state = AddressStateV1::default();
        for kind in [
            AccessKindAttr::Read,
            AccessKindAttr::Write,
            AccessKindAttr::AtomicRead,
            AccessKindAttr::AtomicWrite,
        ] {
            for ordinal in 0..2 {
                let mut invocation = coordinates.clone();
                invocation[0] = ordinal;
                insert_witness(
                    &mut state,
                    RankedRaceWitnessV1 {
                        location: RankedRaceLocationV1 {
                            block: 0,
                            operation: 0,
                        },
                        access: kind,
                        invocation,
                        grid: 0,
                        workgroup: None,
                        subgroup: None,
                        lane: None,
                        atomic_scope: None,
                    },
                );
            }
        }
        let retained = [
            state.reads,
            state.writes,
            state.atomic_reads,
            state.atomic_writes,
        ];
        let mut slots = 0;
        for pair in retained {
            for witness in [pair.first.unwrap(), pair.second.unwrap()] {
                assert_eq!(witness.invocation.len(), launch_rank);
                slots += witness.invocation.len();
            }
        }
        assert_eq!(slots, 8 * launch_rank);
    }
}

#[test]
fn no_exact_fallback_retains_no_address_state() {
    let actual = calculate_race_resource_upper_bound_for_shape_v1(
        ProductionAnalysisInputCensusV1 {
            ranked_accesses: 2,
            ..Default::default()
        },
        component_race_names_v1(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(actual.retained_effect_instances, 0);
    assert_eq!(actual.address_state, 0);
    assert_eq!(actual.raw_evaluation_work, 0);
    assert_eq!(actual.raw_evaluation_temporary, 0);
}

#[test]
fn zero_and_single_invocation_storage_stays_bounded() {
    for (invocations, launch_rank) in [(0, 3), (1, 0), (1, 3)] {
        let actual = calculate_race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: 2,
                ..Default::default()
            },
            component_race_names_v1(),
            Some((invocations, launch_rank)),
            None,
        )
        .unwrap();
        assert_eq!(actual.retained_effect_instances, 0);
        assert_eq!(actual.address_state, 0);
    }
}

#[test]
fn overflowing_launch_rank_fails_closed() {
    assert!(matches!(
        calculate_race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 { ranked_accesses: 1, ..Default::default() },
            component_race_names_v1(),
            Some((2, usize::MAX)),
            None,
        ),
        Err(error) if error == race_resource_overflow_v1()
    ));
}

#[test]
fn typed_v5_effect_population_has_exact_and_one_short_refined_bound() {
    // The authenticated effect population matches the retained V5 root. This
    // is not a replay of its full 221-block CFG or an extraction success claim.
    let mut context = context();
    let (function, _) = fixture_with_globals(&mut context, MemorySpaceAttr::Private, true);
    let census = census(&context, &function);
    let names = names(&context, &function);
    assert_eq!((census.ranked_accesses, census.allocation_effects), (8, 12));
    assert_eq!(names.private_ranked_accesses, 4);
    let actual =
        calculate_race_resource_upper_bound_for_shape_v1(census, names, Some((32_768, 3)), None)
            .unwrap();
    let previous =
        calculate_race_resource_upper_bound_for_shape_v1(census, names, Some((32_768, 8)), None)
            .unwrap();
    assert_eq!(actual.retained_effect_instances, 524_288);
    assert_eq!(actual.address_state, 50_331_648);
    assert_eq!(previous.address_state - actual.address_state, 20_971_520);
    assert_eq!(
        previous.bound.peak_storage_upper_bound() - actual.bound.peak_storage_upper_bound(),
        20_971_520
    );
    let work = actual.bound.work_upper_bound();
    let peak = actual.bound.peak_storage_upper_bound();
    assert_eq!(
        race_resource_upper_bound_for_shape_v1(
            census,
            names,
            Some((32_768, 3)),
            None,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        ),
        Ok(actual.bound),
    );
    for (work, peak, resource) in [
        (work - 1, peak, "work upper bound"),
        (work, peak - 1, "peak storage upper bound"),
    ] {
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                names,
                Some((32_768, 3)),
                None,
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            )
            .unwrap_err(),
            ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource,
            },
        );
    }
}
