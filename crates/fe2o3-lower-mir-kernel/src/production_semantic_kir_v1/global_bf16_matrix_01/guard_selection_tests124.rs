fn projection_value(graph: &mut CapabilitySsaGraphV1<'_>, change: Change) -> SsaValueV1 {
    graph
        .use_value(
            5,
            if matches!(change, Change::ForwardedWholeResult) {
                14
            } else {
                10
            },
        )
        .unwrap()
}

#[test]
fn warmed_guard_preserves_explicit_otherwise_nonordinal_and_alias_captures() {
    for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
        for change in [
            Change::None,
            Change::Otherwise,
            Change::MovedError,
            Change::NonOrdinal,
            Change::ForwardedWholeResult,
            Change::SameOwnerOk,
        ] {
            let f = with_result_join(role, change);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let value = projection_value(&mut graph, change);
            for _ in 0..2 {
                assert!(
                    graph
                        .selected_variant(&f.types, value, id(15), 0, 5)
                        .unwrap()
                );
            }
            for _ in 0..2 {
                let captured = global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    5,
                )
                .unwrap();
                assert_eq!(
                    (
                        captured.construction().block,
                        captured.construction().statement
                    ),
                    (1, Some(1)),
                    "{role:?} {change:?}"
                );
                assert_eq!(captured.global_bind().block, 0);
                assert_eq!(captured.contract(), f.contract);
            }
        }
    }
}

#[test]
fn warmed_false_guard_preserves_unproved_projection_rejections() {
    for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
        for change in [
            Change::WrongTag,
            Change::StaleTag,
            Change::NoGuard,
            Change::Bypass,
            Change::SharedTarget,
            Change::WrongVariant,
            Change::AmbiguousOtherwise,
        ] {
            let f = with_result_join(role, change);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let value = projection_value(&mut graph, change);
            for _ in 0..2 {
                assert!(
                    !graph
                        .selected_variant(&f.types, value, id(15), 0, 5)
                        .unwrap()
                );
            }
            for _ in 0..2 {
                let result = global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    5,
                );
                assert!(
                    matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "global BF16 Result needs variant-sensitive payload SSA custody",
                            ..
                        })
                    ),
                    "{role:?} {change:?}"
                );
            }
        }
    }
}

#[test]
fn warmed_true_guard_never_approves_payload_origins_or_loan_generations() {
    for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
        for (change, expected) in [
            (
                Change::DifferentOk,
                "global BF16 capture merges different Global issuers",
            ),
            (
                Change::DeadGlobal,
                "capability loan crosses a move, overwrite, deinitialization or storage death",
            ),
            (
                Change::UnknownValue,
                "capability authority originates from an entry parameter or missing definition",
            ),
            (
                Change::OnlyErr,
                "global BF16 guarded Result has no matching payload origin",
            ),
            (
                Change::CyclicResult,
                "global BF16 capture has cyclic or excessive SSA custody",
            ),
            (
                Change::CyclicLoan,
                "capability loan crosses a cycle without a proven storage generation",
            ),
        ] {
            let f = with_result_join(role, change);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let value = projection_value(&mut graph, change);
            for _ in 0..2 {
                assert!(
                    graph
                        .selected_variant(&f.types, value, id(15), 0, 5)
                        .unwrap(),
                    "{role:?} {change:?}"
                );
            }
            for _ in 0..2 {
                let result = global_bf16_live_v1::capture_component(
                    &mut graph,
                    &f.types,
                    &f.callables,
                    &f.context,
                    5,
                );
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail, ..
                    }) if detail == expected),
                    "{role:?} {change:?}: expected {expected}"
                );
                assert!(
                    graph
                        .selected_variant(&f.types, value, id(15), 0, 5)
                        .unwrap()
                );
            }
        }
    }
}

fn install_blocks(f: &mut Fixture, blocks: Vec<SemanticBasicBlockV1>) {
    let old = &f.function;
    f.function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap();
}

fn guard_edge(target: u32, role: SemanticEdgeRoleV1) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn guard_block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminal: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([220 + index; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminal),
    )
    .unwrap()
}

fn guard_switch(values: &[(u128, u32)], otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(13), vec![], id(3)).unwrap(),
        ),
        targets: SemanticSwitchTargetsV1::new(
            values
                .iter()
                .map(|&(value, target)| {
                    SemanticSwitchTargetV1::new(
                        value,
                        guard_edge(target, SemanticEdgeRoleV1::SwitchValue),
                    )
                })
                .collect(),
            guard_edge(otherwise, SemanticEdgeRoleV1::SwitchOtherwise),
        )
        .unwrap(),
    }
}

#[test]
fn nonordinal_guards_distinguish_literals_from_variant_indices_when_warmed() {
    for (values, otherwise, expected) in [
        (vec![(83, 6)], 5, true),
        (vec![(0, 5)], 6, false),
        (vec![(83, 5)], 6, false),
        (vec![(17, 5), (83, 5)], 6, false),
    ] {
        let mut f = with_result_join(SemanticMfmaOperandRoleV1::A, Change::NonOrdinal);
        let mut blocks = f.function.blocks().to_vec();
        blocks[4] = guard_block(
            4,
            blocks[4].statements().to_vec(),
            guard_switch(&values, otherwise),
        );
        install_blocks(&mut f, blocks);
        let plan = plan(&f);
        let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
        let value = projection_value(&mut graph, Change::NonOrdinal);
        for _ in 0..2 {
            assert_eq!(
                graph
                    .selected_variant(&f.types, value, id(15), 0, 5)
                    .unwrap(),
                expected,
                "{values:?} otherwise={otherwise}"
            );
        }
        let result = global_bf16_live_v1::capture_component(
            &mut graph,
            &f.types,
            &f.callables,
            &f.context,
            5,
        );
        if expected {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "global BF16 Result needs variant-sensitive payload SSA custody",
                    ..
                })
            ));
        }
    }
}

// The oracle uses a fixed adjacency matrix and transitive closure. Its edge
// roster is supplied by the fixture, independently of MIR edge enumeration.
fn oracle_reachable(edges: &[(usize, usize, usize)], omitted: Option<(usize, usize)>) -> [bool; 8] {
    let mut paths = [[false; 8]; 8];
    for (index, row) in paths.iter_mut().enumerate() {
        row[index] = true;
    }
    for &(source, ordinal, target) in edges {
        if omitted != Some((source, ordinal)) {
            paths[source][target] = true;
        }
    }
    for via in 0..8 {
        for source in 0..8 {
            for target in 0..8 {
                paths[source][target] =
                    paths[source][target] || (paths[source][via] && paths[via][target]);
            }
        }
    }
    paths[0]
}

#[test]
fn selected_guard_matches_independent_small_cfg_edge_ordinal_oracle() {
    for (targets, tail, bypass) in [
        ([5, 6, 6], 5, false),
        ([6, 5, 6], 5, false),
        ([6, 6, 5], 5, false),
        ([5, 5, 6], 5, false),
        ([6, 5, 5], 5, false),
        ([6, 7, 6], 5, false),
        ([6, 5, 7], 5, false),
        ([6, 7, 6], 7, false),
        ([6, 6, 7], 7, false),
        ([6, 6, 6], 7, false),
        ([6, 5, 6], 5, true),
        ([6, 6, 5], 5, true),
    ] {
        for otherwise_variant in [false, true] {
            let mut f = with_result_join(SemanticMfmaOperandRoleV1::A, Change::NonOrdinal);
            // Both forms have three edges. Variant 0 selects ordinal 1 in the
            // explicit form and ordinal 2 in the otherwise form.
            let (values, ordinals) = if otherwise_variant {
                (vec![(5, targets[0]), (83, targets[1])], [2, 1])
            } else {
                (vec![(5, targets[0]), (17, targets[1])], [1, 2])
            };
            let mut blocks = f.function.blocks().to_vec();
            blocks[4] = guard_block(
                4,
                blocks[4].statements().to_vec(),
                guard_switch(&values, targets[2]),
            );
            let predecessor_target = if bypass { 5 } else { 4 };
            blocks[3] = guard_block(
                3,
                blocks[3].statements().to_vec(),
                SemanticTerminatorKindV1::Goto(guard_edge(
                    predecessor_target,
                    SemanticEdgeRoleV1::Goto,
                )),
            );
            blocks.push(guard_block(
                7,
                vec![],
                SemanticTerminatorKindV1::Goto(guard_edge(tail, SemanticEdgeRoleV1::Goto)),
            ));
            install_blocks(&mut f, blocks);
            let edges = [
                (0, 0, 1),
                (1, 0, 2),
                (1, 1, 3),
                (2, 0, 4),
                (3, 0, predecessor_target as usize),
                (4, 0, targets[0] as usize),
                (4, 1, targets[1] as usize),
                (4, 2, targets[2] as usize),
                (5, 0, 6),
                (7, 0, tail as usize),
            ];
            let reachable = oracle_reachable(&edges, None);
            let plan = plan(&f);
            let mut graph = CapabilitySsaGraphV1::new(&f.function, plan.plan(), 65_536).unwrap();
            let value = graph.use_value(4, 10).unwrap();
            for (variant, ordinal) in ordinals.into_iter().enumerate() {
                let without_edge = oracle_reachable(&edges, Some((4, ordinal)));
                for use_block in 0..8 {
                    assert_eq!(
                        plan.plan()
                            .is_reachable(SsaBlockIdV1::new(use_block as u32)),
                        reachable[use_block]
                    );
                    let expected = reachable[use_block] && !without_edge[use_block];
                    for _ in 0..2 {
                        assert_eq!(
                            graph
                                .selected_variant(
                                    &f.types,
                                    value,
                                    id(15),
                                    variant as u32,
                                    use_block as u32,
                                )
                                .unwrap(),
                            expected,
                            "targets={targets:?} tail={tail} bypass={bypass} otherwise={otherwise_variant} variant={variant} use={use_block} ordinal={ordinal}"
                        );
                    }
                }
            }
        }
    }
}
