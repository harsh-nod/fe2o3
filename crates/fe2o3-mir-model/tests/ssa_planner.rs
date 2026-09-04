use fe2o3_mir_model::{
    SsaArgumentV1, SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaDefinitionIdV1,
    SsaEdgeIdV1, SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaPlannerErrorV1, SsaPlannerLimitsV1,
    SsaPlannerResourceV1, SsaResolvedEventV1, SsaValueV1, SsaVariableIdV1, plan_ssa_v1,
    plan_ssa_with_limits_v1,
};

fn v(identity: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(identity)
}

fn b(identity: u32) -> SsaBlockIdV1 {
    SsaBlockIdV1::new(identity)
}

fn edge(role: u16, target: u32, definitions: &[u32]) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(role),
        b(target),
        definitions.iter().copied().map(v).collect(),
    )
}

fn block(events: &[SsaEventV1], edges: Vec<SsaEdgeInputV1>) -> SsaBlockInputV1 {
    SsaBlockInputV1::new(events.to_vec(), edges)
}

fn arguments(arguments: &[SsaArgumentV1]) -> Vec<(u32, SsaValueV1)> {
    arguments
        .iter()
        .map(|argument| (argument.variable().get(), argument.value()))
        .collect()
}

#[test]
fn plans_pruned_merges_and_transport_for_a_diamond() {
    let input = SsaConstructionInputV1::new(
        b(0),
        2,
        vec![true, true],
        vec![v(0), v(1)],
        vec![
            block(&[], vec![edge(1, 1, &[]), edge(2, 2, &[])]),
            block(&[SsaEventV1::Define(v(0))], vec![edge(1, 3, &[])]),
            block(&[SsaEventV1::Define(v(0))], vec![edge(1, 3, &[])]),
            block(&[SsaEventV1::Use(v(0)), SsaEventV1::Use(v(1))], vec![]),
        ],
    );

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.promoted_variables(), &[v(0), v(1)]);
    assert_eq!(plan.live_in(b(3)), Some([v(0), v(1)].as_slice()));
    assert_eq!(plan.merge_variables(b(3)), Some([v(0)].as_slice()));
    assert_eq!(plan.transport_variables(b(3)), Some([v(0)].as_slice()));
    assert_eq!(
        plan.resolved_event(b(3), 0),
        Some(&SsaResolvedEventV1::Use {
            variable: v(0),
            value: SsaValueV1::BlockArgument {
                block: b(3),
                variable: v(0),
            },
        })
    );
    assert_eq!(
        arguments(plan.edge_arguments(SsaEdgeIdV1::new(b(1), 0)).unwrap()),
        vec![(0, SsaValueV1::Definition(SsaDefinitionIdV1::new(2)))]
    );
    assert_eq!(plan.definition_count(), 4);
    plan.verify_replay(&input, SsaPlannerLimitsV1::default())
        .unwrap();
    assert_eq!(plan.identity(), plan_ssa_v1(&input).unwrap().identity());
}

#[test]
fn distinguishes_parallel_edges_and_edge_definitions() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![v(0)],
        vec![
            block(&[], vec![edge(1, 1, &[0]), edge(2, 1, &[])]),
            block(&[SsaEventV1::Use(v(0))], vec![]),
        ],
    );

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.merge_variables(b(1)), Some([v(0)].as_slice()));
    assert_eq!(
        arguments(plan.edge_arguments(SsaEdgeIdV1::new(b(0), 0)).unwrap()),
        vec![(0, SsaValueV1::Definition(SsaDefinitionIdV1::new(1)))]
    );
    assert_eq!(
        arguments(plan.edge_arguments(SsaEdgeIdV1::new(b(0), 1)).unwrap()),
        vec![(0, SsaValueV1::Definition(SsaDefinitionIdV1::new(0)))]
    );
}

#[test]
fn edge_sensitive_liveness_reports_the_exact_undefined_edge() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![],
        vec![
            block(&[], vec![edge(1, 1, &[0]), edge(2, 1, &[])]),
            block(&[SsaEventV1::Use(v(0))], vec![]),
        ],
    );

    assert_eq!(
        plan_ssa_v1(&input).unwrap_err(),
        SsaPlannerErrorV1::UndefinedAtEdge {
            edge: SsaEdgeIdV1::new(b(0), 1),
            target: b(1),
            variable: v(0),
        }
    );
}

#[test]
fn accepts_irreducible_control_flow_and_places_pruned_idf_merges() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![v(0)],
        vec![
            block(&[], vec![edge(1, 1, &[]), edge(2, 2, &[])]),
            block(
                &[SsaEventV1::Use(v(0)), SsaEventV1::Define(v(0))],
                vec![edge(1, 2, &[]), edge(2, 3, &[])],
            ),
            block(
                &[SsaEventV1::Use(v(0)), SsaEventV1::Define(v(0))],
                vec![edge(1, 1, &[]), edge(2, 3, &[])],
            ),
            block(&[SsaEventV1::Use(v(0))], vec![]),
        ],
    );

    let plan = plan_ssa_v1(&input).unwrap();
    for block in [b(1), b(2), b(3)] {
        assert_eq!(plan.live_in(block), Some([v(0)].as_slice()));
        assert_eq!(plan.merge_variables(block), Some([v(0)].as_slice()));
        assert_eq!(plan.transport_variables(block), Some([v(0)].as_slice()));
    }
}

#[test]
fn models_external_entry_as_an_edge_for_a_cyclic_entry() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![v(0)],
        vec![block(&[SsaEventV1::Use(v(0))], vec![edge(1, 0, &[])])],
    );

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.merge_variables(b(0)), Some([v(0)].as_slice()));
    assert_eq!(plan.transport_variables(b(0)), Some([v(0)].as_slice()));
    assert_eq!(
        arguments(plan.entry_arguments()),
        vec![(0, SsaValueV1::Definition(SsaDefinitionIdV1::new(0)))]
    );
    assert_eq!(
        plan.resolved_event(b(0), 0),
        Some(&SsaResolvedEventV1::Use {
            variable: v(0),
            value: SsaValueV1::BlockArgument {
                block: b(0),
                variable: v(0),
            },
        })
    );
}

#[test]
fn rejects_a_cyclic_entry_live_in_without_an_external_definition() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![],
        vec![block(&[SsaEventV1::Use(v(0))], vec![edge(1, 0, &[])])],
    );

    assert_eq!(
        plan_ssa_v1(&input).unwrap_err(),
        SsaPlannerErrorV1::UndefinedAtEntry { variable: v(0) }
    );
}

#[test]
fn prunes_unreachable_cfg_from_analysis_and_identity() {
    let reachable = block(&[], vec![]);
    let first = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![],
        vec![
            reachable.clone(),
            block(&[SsaEventV1::Define(v(0))], vec![edge(1, 1, &[])]),
        ],
    );
    let second = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![],
        vec![
            reachable,
            block(&[SsaEventV1::Use(v(0)), SsaEventV1::Define(v(0))], vec![]),
        ],
    );

    let first_plan = plan_ssa_v1(&first).unwrap();
    let second_plan = plan_ssa_v1(&second).unwrap();
    assert_eq!(first_plan.identity(), second_plan.identity());
    assert_eq!(first_plan.resources().reachable_blocks(), 1);
    assert_eq!(first_plan.resources().pruned_blocks(), 1);
    assert!(!first_plan.is_reachable(b(1)));
    assert_eq!(first_plan.live_in(b(1)), None);
    assert_eq!(first_plan.edge_arguments(SsaEdgeIdV1::new(b(1), 0)), None);
}

#[test]
fn promotability_is_an_explicit_adapter_owned_boundary() {
    let input = SsaConstructionInputV1::new(
        b(0),
        2,
        vec![true, false],
        vec![v(0)],
        vec![block(
            &[
                SsaEventV1::Use(v(1)),
                SsaEventV1::Define(v(0)),
                SsaEventV1::Define(v(1)),
            ],
            vec![],
        )],
    );

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.promoted_variables(), &[v(0)]);
    assert_eq!(plan.resolved_event(b(0), 0), None);
    assert!(matches!(
        plan.resolved_event(b(0), 1),
        Some(SsaResolvedEventV1::Define {
            variable,
            value: SsaValueV1::Definition(_),
        }) if *variable == v(0)
    ));
    assert_eq!(plan.resolved_event(b(0), 2), None);
}

#[test]
fn resource_limits_are_inclusive_and_fail_closed() {
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![v(0)],
        vec![block(&[SsaEventV1::Use(v(0))], vec![])],
    );
    let baseline = plan_ssa_v1(&input).unwrap();
    let report = baseline.resources();
    let exact = SsaPlannerLimitsV1::try_new(
        1,
        1,
        0,
        1,
        0,
        report.output_items(),
        report.storage_words(),
        report.work_units(),
    )
    .unwrap();
    let exact_plan = plan_ssa_with_limits_v1(&input, exact).unwrap();
    assert_eq!(exact_plan.identity(), baseline.identity());

    let insufficient_work = SsaPlannerLimitsV1::try_new(
        1,
        1,
        0,
        1,
        0,
        report.output_items(),
        report.storage_words(),
        report.work_units() - 1,
    )
    .unwrap();
    assert!(matches!(
        plan_ssa_with_limits_v1(&input, insufficient_work),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));

    let insufficient_variables = SsaPlannerLimitsV1::try_new(
        0,
        1,
        0,
        1,
        0,
        report.output_items(),
        report.storage_words(),
        report.work_units(),
    )
    .unwrap();
    assert_eq!(
        plan_ssa_with_limits_v1(&input, insufficient_variables).unwrap_err(),
        SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::Variables,
            required: 1,
            limit: 0,
        }
    );
}

#[test]
fn work_limit_charges_unreachable_input_validation() {
    let event_count = 4_096;
    let input = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![false],
        vec![],
        vec![
            block(&[], vec![]),
            block(&vec![SsaEventV1::Use(v(0)); event_count], vec![]),
        ],
    );
    let defaults = SsaPlannerLimitsV1::default();
    let limits = SsaPlannerLimitsV1::try_new(
        1,
        2,
        0,
        event_count,
        0,
        defaults.max_output_items(),
        defaults.max_storage_words(),
        64,
    )
    .unwrap();

    assert!(matches!(
        plan_ssa_with_limits_v1(&input, limits),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn nonpromotable_events_use_sparse_plan_storage() {
    let event_count = 4_096;
    let empty = SsaConstructionInputV1::new(b(0), 1, vec![false], vec![], vec![block(&[], vec![])]);
    let events = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![false],
        vec![],
        vec![block(&vec![SsaEventV1::Define(v(0)); event_count], vec![])],
    );

    let empty_plan = plan_ssa_v1(&empty).unwrap();
    let event_plan = plan_ssa_v1(&events).unwrap();
    assert_eq!(event_plan.resources().output_items(), 0);
    assert_eq!(
        event_plan.resources().storage_words(),
        empty_plan.resources().storage_words()
    );
    assert_eq!(
        event_plan.resolved_event(b(0), (event_count - 1) as u32),
        None
    );
    assert!(event_plan.resources().work_units() > empty_plan.resources().work_units());
}

#[test]
fn block_bitsets_scale_with_promotable_not_semantic_variables() {
    const BLOCK_COUNT: usize = 2_048;
    const VARIABLE_COUNT: usize = 65_536;

    let make_blocks = || {
        (0..BLOCK_COUNT)
            .map(|block_index| {
                let edges = if block_index + 1 == BLOCK_COUNT {
                    vec![]
                } else {
                    vec![edge(1, (block_index + 1) as u32, &[])]
                };
                block(&[], edges)
            })
            .collect()
    };
    let none = SsaConstructionInputV1::new(
        b(0),
        VARIABLE_COUNT as u32,
        vec![false; VARIABLE_COUNT],
        vec![],
        make_blocks(),
    );
    let mut one_promotable = vec![false; VARIABLE_COUNT];
    one_promotable[VARIABLE_COUNT - 1] = true;
    let one = SsaConstructionInputV1::new(
        b(0),
        VARIABLE_COUNT as u32,
        one_promotable,
        vec![],
        make_blocks(),
    );

    let none_plan = plan_ssa_v1(&none).unwrap();
    let one_plan = plan_ssa_v1(&one).unwrap();
    assert_eq!(none_plan.promoted_variables(), []);
    assert_eq!(
        one_plan.promoted_variables(),
        [v((VARIABLE_COUNT - 1) as u32)]
    );
    assert!(
        one_plan.resources().storage_words() > none_plan.resources().storage_words(),
        "one promoted variable must add its block-domain bitsets"
    );
    assert!(
        one_plan.resources().storage_words() - none_plan.resources().storage_words()
            < BLOCK_COUNT * 8,
        "one promoted variable must cost O(blocks), independent of nonpromotable locals"
    );
}

#[test]
fn compact_promoted_domain_crosses_bit_words_for_merges_kills_and_edge_definitions() {
    for variable_count in [63_u32, 64, 65] {
        let variables = (0..variable_count).map(v).collect::<Vec<_>>();
        let definitions = (0..variable_count).collect::<Vec<_>>();
        let uses = variables
            .iter()
            .copied()
            .map(SsaEventV1::Use)
            .collect::<Vec<_>>();
        let input = SsaConstructionInputV1::new(
            b(0),
            variable_count,
            vec![true; variable_count as usize],
            variables.clone(),
            vec![
                block(&[], vec![edge(1, 1, &[]), edge(2, 2, &[])]),
                block(&uses, vec![edge(1, 3, &[])]),
                block(&[], vec![edge(1, 3, &definitions)]),
                block(&uses, vec![]),
            ],
        );

        let plan = plan_ssa_v1(&input).unwrap();
        assert_eq!(plan.live_in(b(3)), Some(variables.as_slice()));
        assert_eq!(plan.merge_variables(b(3)), Some(variables.as_slice()));
        assert_eq!(plan.transport_variables(b(3)), Some(variables.as_slice()));
        assert_eq!(
            plan.edge_arguments(SsaEdgeIdV1::new(b(2), 0))
                .unwrap()
                .len(),
            variable_count as usize
        );
        plan.verify_replay(&input, SsaPlannerLimitsV1::default())
            .unwrap();

        let last = v(variable_count - 1);
        let killed = SsaConstructionInputV1::new(
            b(0),
            variable_count,
            vec![true; variable_count as usize],
            variables,
            vec![
                block(&[SsaEventV1::Kill(last)], vec![edge(1, 1, &[])]),
                block(&[SsaEventV1::Use(last)], vec![]),
            ],
        );
        assert_eq!(
            plan_ssa_v1(&killed).unwrap_err(),
            SsaPlannerErrorV1::UndefinedAtUse {
                block: b(1),
                event: 0,
                variable: last,
            }
        );
    }
}

#[test]
fn sparse_dominance_frontiers_scale_for_long_linear_cfg() {
    const BLOCK_COUNT: usize = 12_000;
    let blocks = (0..BLOCK_COUNT)
        .map(|block_index| {
            let edges = if block_index + 1 == BLOCK_COUNT {
                vec![]
            } else {
                vec![edge(1, (block_index + 1) as u32, &[])]
            };
            block(&[], edges)
        })
        .collect();
    let input = SsaConstructionInputV1::new(b(0), 0, vec![], vec![], blocks);

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.reverse_postorder().len(), BLOCK_COUNT);
    assert_eq!(plan.resources().reachable_blocks(), BLOCK_COUNT);
}

#[test]
fn sparse_idf_does_not_scan_every_block_for_every_promoted_variable() {
    const BLOCK_COUNT: usize = 4_096;
    const VARIABLE_COUNT: usize = 4_096;
    let blocks = (0..BLOCK_COUNT)
        .map(|block_index| {
            let edges = if block_index + 1 == BLOCK_COUNT {
                vec![]
            } else {
                vec![edge(1, (block_index + 1) as u32, &[])]
            };
            block(&[], edges)
        })
        .collect();
    let input = SsaConstructionInputV1::new(
        b(0),
        VARIABLE_COUNT as u32,
        vec![true; VARIABLE_COUNT],
        vec![],
        blocks,
    );

    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(plan.resources().reachable_blocks(), BLOCK_COUNT);
    assert!(plan.resources().work_units() < BLOCK_COUNT * VARIABLE_COUNT);
}

#[test]
fn validates_canonical_edge_metadata_before_analysis() {
    let duplicate_definitions = SsaConstructionInputV1::new(
        b(0),
        1,
        vec![true],
        vec![],
        vec![block(&[], vec![edge(1, 0, &[0, 0])])],
    );
    assert_eq!(
        plan_ssa_v1(&duplicate_definitions).unwrap_err(),
        SsaPlannerErrorV1::NonCanonicalDefinitions {
            edge: Some(SsaEdgeIdV1::new(b(0), 0)),
        }
    );

    let reserved_role = SsaConstructionInputV1::new(
        b(0),
        0,
        vec![],
        vec![],
        vec![block(&[], vec![edge(0, 0, &[])])],
    );
    assert_eq!(
        plan_ssa_v1(&reserved_role).unwrap_err(),
        SsaPlannerErrorV1::InvalidEdgeRole {
            edge: SsaEdgeIdV1::new(b(0), 0),
        }
    );

    let unknown_target = SsaConstructionInputV1::new(
        b(0),
        0,
        vec![],
        vec![],
        vec![block(&[], vec![edge(1, 4, &[])])],
    );
    assert_eq!(
        plan_ssa_v1(&unknown_target).unwrap_err(),
        SsaPlannerErrorV1::UnknownTarget {
            edge: SsaEdgeIdV1::new(b(0), 0),
            target: b(4),
            block_count: 1,
        }
    );
}
