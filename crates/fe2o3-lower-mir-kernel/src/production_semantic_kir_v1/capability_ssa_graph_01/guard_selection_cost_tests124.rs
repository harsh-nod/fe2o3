mod guard_selection_cost_tests124 {
    use super::*;
    use std::mem::size_of;

    const WORK: usize = 1_000_000;
    type Key = enum_guard::Key;

    fn ty(index: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(index)
    }

    fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticTypeDeclV1>) {
        let shapes = [
            SemanticTypeShapeV1::Unit,
            SemanticTypeShapeV1::Enum {
                discriminant: ty(2),
                variants: [17, 83]
                    .into_iter()
                    .map(|tag| {
                        SemanticEnumVariantV1::new(
                            tag,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
            SemanticTypeShapeV1::Unit,
        ];
        let types = shapes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| {
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([180 + index as u8; 32]),
                    SemanticLayoutIdentityV1::from_sha256([190 + index as u8; 32]),
                    SemanticTypeLayoutV1::new(Some(if index % 3 == 0 { 0 } else { 4 }), 4).unwrap(),
                    shape,
                )
            })
            .collect();
        let typed_place = |local, kind| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
        };
        let assign = |local, kind, value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                typed_place(local, kind),
                SemanticRvalueV1::new(ty(kind), value),
            )))
        };
        let edge = |target, role| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let guard = block(
            0,
            vec![
                assign(
                    1,
                    1,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::EnumVariant(0),
                            vec![],
                        )
                        .unwrap(),
                    ),
                ),
                assign(
                    3,
                    1,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(typed_place(1, 1))),
                ),
                assign(2, 2, SemanticRvalueKindV1::Discriminant(typed_place(3, 1))),
            ],
            None,
        );
        let guard = SemanticBasicBlockV1::new(
            guard.identity(),
            guard.source(),
            guard.statements().to_vec(),
            SemanticTerminatorV1::new(
                guard.source(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(typed_place(2, 2)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            17,
                            edge(1, SemanticEdgeRoleV1::SwitchValue),
                        )],
                        edge(2, SemanticEdgeRoleV1::SwitchOtherwise),
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
        let source = body(vec![
            guard,
            block(1, vec![], Some(3)),
            block(2, vec![], Some(3)),
            block(3, vec![], None),
        ]);
        let mut locals = source.locals().to_vec();
        locals[2] = SemanticLocalDeclV1::new(
            locals[2].identity(),
            ty(2),
            locals[2].role(),
            locals[2].source(),
        );
        let body = SemanticFunctionDeclV1::new(
            source.identity(),
            source.role(),
            source.item_definition_identity(),
            source.monomorphization_identity(),
            source.generic_type_arguments_identity(),
            source.const_generic_arguments_identity(),
            source.source(),
            source.abi().clone(),
            locals,
            source.entry(),
            source.blocks().to_vec(),
        )
        .unwrap();
        (body, types)
    }

    fn value(ssa: &SsaConstructionPlanV1, local: u32) -> SsaValueV1 {
        ssa.resolved_events(SsaBlockIdV1::new(0))
            .unwrap()
            .iter()
            .find_map(|(_, event)| match event {
                SsaResolvedEventV1::Define { variable, value } if variable.get() == local => {
                    Some(*value)
                }
                _ => None,
            })
            .unwrap()
    }

    fn query<'a>(
        graph: &mut CapabilitySsaGraphV1<'a>,
        types: &'a [SemanticTypeDeclV1],
        key: Key,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        graph.selected_variant(types, key.0, key.1, key.2, key.3)
    }

    fn cold(
        graph: &mut CapabilitySsaGraphV1<'_>,
        types: &[SemanticTypeDeclV1],
        key: Key,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        enum_guard::selected_variant(graph, types, key.0, key.1, key.2, key.3)
    }

    fn walk(mut rows: usize) -> usize {
        let mut cost = 1;
        while rows != 0 {
            cost += 12;
            rows >>= 1;
        }
        cost
    }

    fn publication(rows: usize) -> usize {
        let words = size_of::<(Key, bool)>().div_ceil(size_of::<usize>()) + 1;
        walk(rows) * (words + 1) + words
    }

    fn debit(remaining: &mut usize, charges: &[usize]) -> bool {
        for &charge in charges {
            let Some(next) = remaining.checked_sub(charge) else {
                return false;
            };
            *remaining = next;
        }
        true
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Snapshot {
        owner: Option<(usize, usize, usize, usize)>,
        rows: BTreeMap<Key, bool>,
    }

    fn snapshot(graph: &CapabilitySsaGraphV1<'_>) -> Snapshot {
        Snapshot {
            owner: graph.reuse.enum_guards.owner.map(|(body, ssa, types)| {
                (
                    body as *const _ as usize,
                    ssa as *const _ as usize,
                    types.as_ptr() as usize,
                    types.len(),
                )
            }),
            rows: graph.reuse.enum_guards.rows.clone(),
        }
    }

    fn work_failure<T: std::fmt::Debug>(
        result: &Result<T, ProductionSemanticKirErrorV1>,
        limit: usize,
    ) {
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit: actual_limit,
        }) if *actual == limit + 1 && *actual_limit == limit),
            "{result:?}"
        );
    }

    fn prepared<'a>(
        body: &'a SemanticFunctionDeclV1,
        ssa: &'a SsaConstructionPlanV1,
        types: &'a [SemanticTypeDeclV1],
        key: Key,
        warm: bool,
        hit: bool,
    ) -> CapabilitySsaGraphV1<'a> {
        let mut graph = CapabilitySsaGraphV1::new(body, ssa, WORK).unwrap();
        if warm {
            assert!(!query(&mut graph, types, (key.0, key.1, 100, key.3)).unwrap());
        }
        if hit {
            query(&mut graph, types, key).unwrap();
        }
        graph
    }

    #[test]
    fn inline_header_and_every_short_constructor_budget_are_paid_once() {
        let (body, _) = fixture();
        let plan = plan(&body);
        let word = size_of::<usize>();
        let memo_words = size_of::<enum_guard::Memo<'_>>().div_ceil(word);
        if word == 8 {
            assert_eq!(memo_words, 7);
        }
        let header = size_of::<CapabilityQueryReuseV1<'_>>().div_ceil(word);
        let needed = header + body.blocks().len() + body.locals().len();
        for allowance in 0..=needed {
            let result = CapabilitySsaGraphV1::new(&body, plan.plan(), allowance).map(|graph| {
                assert!(graph.reuse.enum_guards.owner.is_none());
                assert!(graph.reuse.enum_guards.rows.is_empty());
                graph.remaining
            });
            if allowance < needed {
                work_failure(&result, allowance);
            } else {
                assert_eq!(result.unwrap(), 0);
            }
        }
    }

    #[test]
    fn every_short_true_false_cold_warm_miss_and_hit_budget_has_exact_debits() {
        let (body, types) = fixture();
        let plan = plan(&body);
        for expected in [true, false] {
            let key = (value(plan.plan(), 1), ty(1), u32::from(!expected), 1);
            for (warm, hit) in [(false, false), (true, false), (true, true)] {
                let mut reference = prepared(&body, plan.plan(), &types, key, warm, hit);
                let state = snapshot(&reference);
                let before = reference.remaining;
                assert_eq!(cold(&mut reference, &types, key).unwrap(), expected);
                let cold_cost = before - reference.remaining;
                let rows = state.rows.len();
                let commit = publication(rows) + if state.owner.is_none() { 4 } else { 0 };
                let needed = 5 + walk(rows) + if hit { 0 } else { cold_cost + commit };
                for allowance in 0..=needed {
                    let mut graph = prepared(&body, plan.plan(), &types, key, warm, hit);
                    let mut reference = prepared(&body, plan.plan(), &types, key, warm, hit);
                    graph.remaining = allowance;
                    reference.remaining = allowance;
                    let actual = query(&mut graph, &types, key);
                    let mut success = debit(&mut reference.remaining, &[5, walk(rows)]);
                    if success && !hit {
                        let result = cold(&mut reference, &types, key);
                        success = result.is_ok();
                        if success {
                            assert_eq!(result.unwrap(), expected);
                            success = debit(&mut reference.remaining, &[publication(rows)]);
                            if success && state.owner.is_none() {
                                success = debit(&mut reference.remaining, &[4]);
                            }
                        } else {
                            work_failure(&result, WORK);
                        }
                    }
                    assert_eq!(
                        graph.remaining, reference.remaining,
                        "expected={expected}, warm={warm}, hit={hit}, allowance={allowance}"
                    );
                    assert_eq!(success, allowance == needed);
                    assert_eq!(graph.reuse.definitions, reference.reuse.definitions);
                    assert_eq!(graph.reuse.uses, reference.reuse.uses);
                    if success {
                        assert_eq!(actual.unwrap(), expected);
                    } else {
                        work_failure(&actual, WORK);
                        assert_eq!(snapshot(&graph), state);
                        // Retry from the subordinate caches actually completed before failure.
                        reference.remaining = WORK;
                        assert_eq!(cold(&mut reference, &types, key).unwrap(), expected);
                        graph.remaining = 5
                            + walk(rows)
                            + if hit {
                                0
                            } else {
                                WORK - reference.remaining + commit
                            };
                        assert_eq!(query(&mut graph, &types, key).unwrap(), expected);
                        assert_eq!(graph.remaining, 0);
                    }
                    let after = snapshot(&graph);
                    let mut expected_rows = state.rows.clone();
                    expected_rows.insert(key, expected);
                    assert_eq!(after.rows, expected_rows);
                    assert_eq!(
                        after.owner,
                        Some((
                            &body as *const _ as usize,
                            plan.plan() as *const _ as usize,
                            types.as_ptr() as usize,
                            types.len()
                        ))
                    );
                    assert!(graph.reuse.loans.is_empty());
                    assert!(graph.reuse.loan_regions.is_empty());
                }
            }
        }
    }

    #[test]
    fn failed_first_owner_publication_keeps_spent_work_and_allows_another_types_owner() {
        let (body, types) = fixture();
        let copied_types = types.clone();
        let plan = plan(&body);
        let key = (value(plan.plan(), 1), ty(0), 0, 1);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
        let state = snapshot(&graph);
        graph.remaining = 5 + walk(0) + publication(0) + 3;
        work_failure(&query(&mut graph, &types, key), WORK);
        assert_eq!(graph.remaining, 3);
        assert_eq!(snapshot(&graph), state);
        graph.remaining = 5 + walk(0) + publication(0) + 4;
        assert!(!query(&mut graph, &copied_types, key).unwrap());
        assert_eq!(graph.remaining, 0);
        assert_eq!(
            snapshot(&graph).owner.unwrap().2,
            copied_types.as_ptr() as usize
        );
    }

    #[test]
    fn equal_clones_and_types_pointer_or_length_changes_reject_before_lookup() {
        let (body, types) = fixture();
        let plan = plan(&body);
        let body_copy = body.clone();
        let ssa_copy = plan.plan().clone();
        let types_copy = types.clone();
        assert_eq!(body, body_copy);
        assert_eq!(plan.plan(), &ssa_copy);
        assert_eq!(types, types_copy);
        assert!(!std::ptr::eq(&body, &body_copy));
        assert!(!std::ptr::eq(plan.plan(), &ssa_copy));
        assert_ne!(types.as_ptr(), types_copy.as_ptr());
        assert_eq!(types.as_ptr(), types[..3].as_ptr());
        for expected in [true, false] {
            let key = (value(plan.plan(), 1), ty(1), u32::from(!expected), 1);
            for change in 1..=8 {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
                assert_eq!(query(&mut graph, &types, key).unwrap(), expected);
                let state = snapshot(&graph);
                graph.body = if change & 1 != 0 { &body_copy } else { &body };
                graph.ssa = if change & 2 != 0 {
                    &ssa_copy
                } else {
                    plan.plan()
                };
                let foreign_types = if change == 8 {
                    &types[..3]
                } else if change & 4 != 0 {
                    &types_copy[..]
                } else {
                    &types[..]
                };
                for attempted in [key, (key.0, ty(0), 0, key.3)] {
                    for allowance in 0..=5 {
                        graph.remaining = allowance;
                        let result = query(&mut graph, foreign_types, attempted);
                        if allowance < 5 {
                            work_failure(&result, WORK);
                            assert_eq!(graph.remaining, allowance);
                        } else {
                            assert!(matches!(
                                result,
                                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                            ));
                            assert_eq!(graph.remaining, 0);
                        }
                        assert_eq!(snapshot(&graph), state);
                    }
                }
                graph.body = &body;
                graph.ssa = plan.plan();
                graph.remaining = 5 + walk(1);
                assert_eq!(query(&mut graph, &types, key).unwrap(), expected);
                assert_eq!(graph.remaining, 0);
                assert_eq!(snapshot(&graph), state);
            }
        }
    }

    #[test]
    fn original_value_type_variant_and_use_block_each_separate_completed_rows() {
        let (body, types) = fixture();
        let plan = plan(&body);
        let root = value(plan.plan(), 1);
        let alias = value(plan.plan(), 3);
        assert_ne!(root, alias);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
        let cases = [
            ((root, ty(1), 0, 1), true),
            ((alias, ty(1), 0, 1), true),
            ((root, ty(0), 0, 1), false),
            ((root, ty(1), 1, 1), false),
            ((root, ty(1), 0, 3), false),
            ((root, ty(1), 1, 2), true),
        ];
        for (key, expected) in cases {
            assert_eq!(query(&mut graph, &types, key).unwrap(), expected);
        }
        assert_eq!(
            graph.reuse.enum_guards.rows,
            cases.into_iter().collect::<BTreeMap<_, _>>()
        );
        let state = snapshot(&graph);
        for (key, expected) in cases {
            graph.remaining = 5 + walk(cases.len());
            assert_eq!(query(&mut graph, &types, key).unwrap(), expected);
            assert_eq!(graph.remaining, 0);
            assert_eq!(snapshot(&graph), state);
        }
    }

    #[test]
    fn semantic_errors_never_publish_and_keep_original_error_and_work() {
        let (body, types) = fixture();
        let plan = plan(&body);
        let missing = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX));
        for bad in [missing, value(plan.plan(), 2)] {
            let key = (bad, ty(1), 0, 1);
            for warm in [false, true] {
                let mut graph = prepared(&body, plan.plan(), &types, key, warm, false);
                let mut reference = prepared(&body, plan.plan(), &types, key, warm, false);
                let state = snapshot(&graph);
                for _ in 0..2 {
                    let before = (graph.remaining, reference.remaining);
                    let expected = cold(&mut reference, &types, key);
                    assert!(matches!(
                        expected,
                        Err(ProductionSemanticKirErrorV1::Unsupported { .. }
                            | ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                    reuse_assert_same(query(&mut graph, &types, key), expected);
                    assert_eq!(
                        before.0 - graph.remaining,
                        5 + walk(state.rows.len()) + before.1 - reference.remaining
                    );
                    assert_eq!(snapshot(&graph), state);
                    assert_eq!(graph.reuse.definitions, reference.reuse.definitions);
                    assert_eq!(graph.reuse.uses, reference.reuse.uses);
                }
            }
        }
    }

    #[test]
    fn map_depth_boundaries_keep_exact_hit_and_publication_costs() {
        let (body, types) = fixture();
        let plan = plan(&body);
        let root = value(plan.plan(), 1);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
        for rows in 0..=8 {
            let key = (root, ty(0), rows as u32, 1);
            let before = graph.remaining;
            assert!(!query(&mut graph, &types, key).unwrap());
            assert_eq!(
                before - graph.remaining,
                5 + walk(rows) + publication(rows) + if rows == 0 { 4 } else { 0 }
            );
            let before = graph.remaining;
            assert!(!query(&mut graph, &types, key).unwrap());
            assert_eq!(before - graph.remaining, 5 + walk(rows + 1));
        }
    }

    #[test]
    fn one_shot_pays_overhead_but_repeated_guards_save_total_work() {
        let (body, types) = fixture();
        let plan = plan(&body);
        let key = (value(plan.plan(), 1), ty(1), 0, 1);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), WORK).unwrap();
        let added_header = size_of::<enum_guard::Memo<'_>>().div_ceil(size_of::<usize>());
        assert!(query(&mut graph, &types, key).unwrap());
        assert!(cold(&mut reference, &types, key).unwrap());
        // Both graphs paid today's header; remove its memo increment from the old baseline.
        let new_first = WORK - graph.remaining;
        let old_first = WORK - reference.remaining - added_header;
        assert_eq!(
            new_first - old_first,
            added_header + 5 + walk(0) + publication(0) + 4
        );
        let state = snapshot(&graph);
        let before = (graph.remaining, reference.remaining);
        for _ in 0..16 {
            let available = graph.remaining;
            assert!(query(&mut graph, &types, key).unwrap());
            assert_eq!(available - graph.remaining, 5 + walk(1));
            assert!(cold(&mut reference, &types, key).unwrap());
        }
        assert_eq!(snapshot(&graph), state);
        assert!(before.0 - graph.remaining < before.1 - reference.remaining);
        assert!(WORK - graph.remaining < WORK - reference.remaining - added_header);
    }
}
