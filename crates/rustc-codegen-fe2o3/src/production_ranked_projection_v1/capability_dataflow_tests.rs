fn capability_diamond_fixture_v1() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let function = projection_function_with_locals(
        vec![
            block(
                211,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: constant(0),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                212,
                vec![],
                neutral_test_call_v1(0, vec![], 1, SCALAR_TYPE, 3),
            ),
            block(
                213,
                vec![],
                neutral_test_call_v1(0, vec![], 2, SCALAR_TYPE, 3),
            ),
            block(214, vec![], SemanticTerminatorKindV1::Return),
        ],
        vec![
            local(211, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(212, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(213, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    );
    let callable = capability_index_callable(
        SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane: SCALAR_TYPE,
            wave_width: 64,
        },
        &[],
        SCALAR_TYPE,
    );
    (function, vec![callable])
}

#[test]
fn capability_dataflow_coalesces_pending_joins_without_losing_invalidations() {
    let (function, callables) = capability_diamond_fixture_v1();
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut work = 0;
    let entries = propagate_capability_dataflow_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &[None; 3],
        &[None; 3],
        &[None; 3],
        &HashMap::new(),
        0,
        &mut work,
    )
    .unwrap();
    assert_eq!(
        work, 16,
        "the join must run once after both predecessors merge"
    );
    assert!(entries[3].as_ref().unwrap().is_empty());
    for local in [1, 2] {
        assert!(
            capability_known_origin_v1(
                entries[3].as_ref().unwrap(),
                &typed_operand(local, SCALAR_TYPE)
            )
            .is_none()
        );
    }
}

#[test]
fn capability_dataflow_keeps_the_existing_work_ceiling_after_coalescing() {
    let (function, callables) = capability_diamond_fixture_v1();
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    for remaining in [16, 15] {
        let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - remaining;
        let result = propagate_capability_dataflow_v1(
            &types,
            &callables,
            &function,
            &dominance,
            &[None; 3],
            &[None; 3],
            &[None; 3],
            &HashMap::new(),
            0,
            &mut work,
        );
        if remaining == 16 {
            result.unwrap();
            assert_eq!(work, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "capability dataflow exceeds the charged projection limit"
                ))
            ));
        }
    }
}

#[test]
fn capability_dataflow_requeues_a_visited_loop_header_for_new_backedge_facts() {
    let (diamond, callables) = capability_diamond_fixture_v1();
    let blocks = vec![
        block(
            211,
            vec![],
            neutral_test_call_v1(0, vec![], 1, SCALAR_TYPE, 1),
        ),
        block(
            212,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        block(
            213,
            vec![statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(1),
            ))],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
    ];
    let function = projection_function_with_locals(blocks, diamond.locals().to_vec());
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut work = 0;
    let entries = propagate_capability_dataflow_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &[None; 3],
        &[None; 3],
        &[None; 3],
        &HashMap::new(),
        0,
        &mut work,
    )
    .unwrap();
    assert_eq!(work, 25);
    for entry in entries {
        assert!(entry.unwrap().is_empty());
    }
}

#[test]
fn capability_sparse_meet_matches_dense_unknown_lattice() {
    let lane = ProjectedCapabilityOriginV1::Lane {
        root: 1,
        wave_width: 64,
    };
    let accumulator = |lane_root, value_root, flow_root| {
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::Accumulator(ProjectedMfmaAccumulatorV1 {
                contract: mfma_accumulator_contract(),
                lane_root: scoped_matrix_use_v1::Issuer::Legacy(lane_root),
                value_root,
                flow_root,
            }),
        ))
    };
    let values = [
        None,
        Some(ProjectedCapabilityValueV1::Invalid),
        Some(ProjectedCapabilityValueV1::Known(lane)),
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::Lane {
                root: 2,
                wave_width: 64,
            },
        )),
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::MatrixContext { root: 1 },
        )),
        Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::GlobalLoadedScalar { block: 3 },
        )),
        Some(ProjectedCapabilityValueV1::ConstructedEnum(
            ProjectedCapabilityEnumEnvelopeV1 {
                origin: lane,
                variants: [1; MAX_PROJECTED_CAPABILITY_ENUM_DEPTH_V1],
                depth: 1,
            },
        )),
        accumulator(1, 30, 30),
        accumulator(1, 30, 40),
        accumulator(1, 30, 50),
        accumulator(1, 31, 40),
        accumulator(2, 30, 40),
    ];
    let mut seed = 0x6c61_7474_6963_6501_u64;
    for _ in 0..4096 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            values[seed as usize % values.len()].clone()
        };
        let current = (0..8)
            .filter_map(|local| next().map(|value| (local, value)))
            .collect::<HashMap<_, _>>();
        let incoming = (0..8)
            .filter_map(|local| next().map(|value| (local, value)))
            .collect::<HashMap<_, _>>();
        let mut expected = HashMap::new();
        // The dense reference explicitly materializes unknown for every key
        // missing on either path, before normalizing its representation.
        for key in current.keys().chain(incoming.keys()) {
            let value = match (current.get(key), incoming.get(key)) {
                (Some(left), Some(right)) => merge_capability_values_v1(left.clone(), right.clone()),
                _ => ProjectedCapabilityValueV1::Invalid,
            };
            expected.insert(*key, value);
        }
        expected.retain(|_, value| *value != ProjectedCapabilityValueV1::Invalid);
        let mut actual = current.clone();
        let changed = merge_capability_states_v1(&mut actual, &incoming).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(changed, actual != current);
        assert!(!merge_capability_states_v1(&mut actual, &incoming).unwrap());
    }
}

#[test]
fn capability_sparse_meet_cannot_issue_an_incoming_only_origin() {
    let incoming = HashMap::from([(
        7,
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
            root: 1,
            wave_width: 64,
        }),
    )]);
    let mut current = HashMap::new();
    assert!(!merge_capability_states_v1(&mut current, &incoming).unwrap());
    assert!(current.is_empty());
}

#[test]
fn capability_dataflow_drops_consumed_origins_within_the_existing_work_budget() {
    let (_, callables) = capability_diamond_fixture_v1();
    let blocks = (0..128_u32)
        .map(|index| {
            let statements = if index == 0 {
                Vec::new()
            } else {
                vec![statement(SemanticStatementKindV1::Deinitialize(
                    typed_place(index, SCALAR_TYPE),
                ))]
            };
            let terminator = if index == 127 {
                SemanticTerminatorKindV1::Return
            } else {
                neutral_test_call_v1(0, vec![], index + 1, SCALAR_TYPE, index + 1)
            };
            block((index + 128) as u8, statements, terminator)
        })
        .collect();
    let locals = (0..128_u8)
        .map(|index| {
            local(
                index + 1,
                SCALAR_TYPE,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
            )
        })
        .collect();
    let function = projection_function_with_locals(blocks, locals);
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let initial_work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - 1024;
    let mut work = initial_work;
    let entries = propagate_capability_dataflow_v1(
        &types,
        &callables,
        &function,
        &dominance,
        &[None; 128],
        &[None; 128],
        &[None; 128],
        &HashMap::new(),
        0,
        &mut work,
    )
    .unwrap();
    assert_eq!(work - initial_work, 1017);
    for entry in entries {
        let entry = entry.unwrap();
        assert!(entry.len() <= 1);
        assert!(
            entry
                .values()
                .all(|value| *value != ProjectedCapabilityValueV1::Invalid)
        );
    }
}

#[test]
fn pipeline_free_projection_reuses_its_fixed_point_within_the_existing_budget() {
    // Two necessary traversals fit; repeating the unchanged fixed point does not.
    let statements_per_block = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 / 3 / 8 + 1;
    let blocks = (0..8)
        .map(|index| {
            block(
                220 + index as u8,
                vec![statement(SemanticStatementKindV1::Nop); statements_per_block],
                if index == 7 {
                    SemanticTerminatorKindV1::Return
                } else {
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, index + 1))
                },
            )
        })
        .collect();
    let function = projection_function_with_locals(
        blocks,
        vec![local(220, SCALAR_TYPE, SemanticLocalRoleV1::Return)],
    );
    let types = projection_types();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let effects =
        project_authenticated_capabilities_v1(&types, &[], &function, &dominance, &[None], &[None])
            .unwrap();
    assert_eq!(effects.layouts.len(), 8);
    assert!(effects.layouts.iter().all(Option::is_none));
    assert!(effects.global_views.iter().all(Option::is_none));
}
