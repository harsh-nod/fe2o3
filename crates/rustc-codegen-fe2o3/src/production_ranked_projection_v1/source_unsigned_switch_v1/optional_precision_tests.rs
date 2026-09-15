//! Exact rollback of optional work; injected records remain component fixtures.
use super::*;

#[path = "optional_precision_followup_tests.rs"]
mod followup_tests;

#[derive(Default)]
struct Settings {
    first_pass: bool,
    node_work: Option<usize>,
    shared_work: Option<usize>,
    prebound_extent: bool,
    next_value: Option<u32>,
    next_argument: Option<usize>,
    mandatory: bool,
    observe_uncommitted: bool,
}

#[derive(Debug, PartialEq)]
struct Outputs {
    operations: Vec<ProductionRankedOperationV1>,
    arguments: Vec<Option<u32>>,
    extents: Vec<Option<u32>>,
    next_argument: usize,
    next_value: u32,
}

struct Probe {
    function: SemanticFunctionDeclV1,
    result: Result<Vec<Option<ProjectedDeterministicSwitchV1>>, ProductionRankedProjectionErrorV1>,
    before: Outputs,
    after: Outputs,
    work_before: usize,
    work_after: usize,
    direct: Vec<Option<GuardPredicateV1>>,
}

fn probe(f: &Fixture, equality: bool, settings: Settings) -> Probe {
    let original = f.function();
    let mut locals = original.locals().to_vec();
    if equality {
        locals[7] = SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([99; 32]),
            WORD,
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceProvenanceV1::unavailable(),
        );
    }
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([91; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([92; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([93; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([94; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi(if equality { vec![WORD] } else { vec![] }, UNIT),
        locals,
        SemanticBlockIdV1::from_index(0),
        f.blocks.clone(),
    )
    .unwrap();
    let retained = function.clone();
    let inventory = assertion_definition_inventory(&function).unwrap();
    let mut proof = SemanticAssertProofsV1::new(&f.types, &function).unwrap();
    assert!(proof.block_dominates(0, 5).unwrap());
    let prior_dominance = proof.dominance.clone();
    let definition_storage = proof.definition_counts.as_ptr();
    let mut indices = vec![None; f.locals.len()];
    for slot in &mut indices[1..=3] {
        *slot = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
    }
    let constants = vec![None; f.locals.len()];
    let mut arguments = vec![Some(73); f.locals.len()];
    let mut extents = vec![None; f.locals.len()];
    extents[1] = Some(8);
    if settings.prebound_extent {
        extents[0] = Some(7);
    }
    let mut allocations = vec![None; f.locals.len()];
    let allocation = AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 4,
        writable: true,
        singleton_object: false,
    };
    allocations[7] = Some(allocation);
    let mut uses = recorded_comparison(&function);
    if equality {
        uses.source_unsigned_comparisons
            .get_mut(&(5, 0))
            .unwrap()
            .left = Some(allocation);
        for (block_index, block) in function.blocks().iter().enumerate().skip(6) {
            let Some(statement) = block.statements().first() else {
                continue;
            };
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            uses.source_unsigned_comparisons.insert(
                (block_index, 0),
                source_unsigned_switch_v1::SourceComparisonV1 {
                    assignment,
                    left: Some(allocation),
                    right: None,
                },
            );
        }
    }
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    let mut next_value = settings.next_value.unwrap_or(1);
    let mut next_argument = settings.next_argument.unwrap_or(9);
    let mut direct = vec![None; f.locals.len()];
    let mut projector = TotalUnsignedIndexProjectorV1::new(
        &f.types,
        &function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        &mut proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap()
    .with_invocation_roots(&f.callables, &indices, 1024)
    .unwrap();
    if settings.first_pass {
        projector
            .retain_unsigned_comparison_predicates_v1(&mut direct)
            .unwrap();
        assert!(direct.last().unwrap().is_some());
        assert_eq!(projector.node_work, 61);
    }
    if let Some(nodes) = settings.node_work {
        projector.node_work = nodes;
    }
    if let Some(work) = settings.shared_work {
        projector
            .assertion_proofs
            .charge(work.saturating_sub(projector.assertion_proofs.work))
            .unwrap();
    }
    let before = Outputs {
        operations: projector.operations.clone(),
        arguments: projector.argument_slots.to_vec(),
        extents: extents.clone(),
        next_argument: *projector.next_argument,
        next_value: *projector.next_value,
    };
    let work_before = projector.assertion_proofs.work;
    let result = if settings.observe_uncommitted {
        let result = projector.optional_uncommitted_scan_for_test_v1(&uses, &allocations, &mut extents);
        drop(projector);
        result
    } else if settings.mandatory {
        let result = projector.source_unsigned_switches_v1(&uses, &allocations, &mut extents);
        drop(projector);
        result
    } else {
        projector.optional_source_unsigned_switches_v1(&uses, &allocations, &mut extents)
    };
    assert_eq!(proof.definition_counts.as_ptr(), definition_storage);
    for (key, value) in prior_dominance {
        assert_eq!(proof.dominance.get(&key), Some(&value));
    }
    assert_eq!(function, retained);
    Probe {
        function: function.clone(),
        result,
        before,
        after: Outputs {
            operations,
            arguments,
            extents,
            next_argument,
            next_value,
        },
        work_before,
        work_after: proof.work,
        direct,
    }
}

fn equality_fixture() -> Fixture {
    let mut f = checked_fixture(SemanticBinaryOpV1::Equal);
    f.replace_block(
        5,
        vec![assign(
            5,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: copy(7, WORD),
                right: field(6, 0, WORD),
            },
        )],
        switch(BOOL, 3, 4),
    );
    f
}

fn long_prefix_fixture() -> Fixture {
    let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let mut statements = f.blocks[2].statements().to_vec();
    let mut local = 4;
    for _ in 0..55 {
        let next = f.locals.len() as u32;
        f.locals.push(WORD);
        statements.push(assign(
            next,
            WORD,
            SemanticRvalueKindV1::Use(copy(local, WORD)),
        ));
        local = next;
    }
    let flag = f.locals.len() as u32;
    f.locals.push(BOOL);
    let mut terminal = f.blocks[2].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert { target, .. } = &mut terminal else {
        unreachable!()
    };
    *target = edge(SemanticEdgeRoleV1::AssertSuccess, 6);
    f.replace_block(2, statements, terminal);
    f.blocks.push(block(
        6,
        vec![assign(
            flag,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: copy(local, WORD),
                right: constant(WORD, 1024, 8),
            },
        )],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(flag, BOOL),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 3),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
            )
            .unwrap(),
        },
    ));
    f
}

#[test]
fn optional_source_precision_preserves_first_pass_at_cumulative_node_limit() {
    let p = probe(
        &long_prefix_fixture(),
        false,
        Settings {
            first_pass: true,
            ..Settings::default()
        },
    );
    assert!(
        p.result.unwrap().is_empty(),
        "no unsupported source predicate is published"
    );
    assert_eq!(
        p.after, p.before,
        "all provisional ranked state is rolled back"
    );
    assert!(
        p.direct.last().unwrap().is_some(),
        "established first-pass guard remains"
    );
    assert!(
        p.work_after > p.work_before,
        "failed precision still costs proof work"
    );
}

#[test]
fn optional_source_precision_keeps_small_checked_guard_and_exact_extent() {
    for equality in [false, true] {
        let f = if equality {
            equality_fixture()
        } else {
            checked_fixture(SemanticBinaryOpV1::GreaterOrEqual)
        };
        let p = probe(&f, equality, Settings::default());
        assert!(p.result.unwrap()[5].is_some());
        assert!(p.after.operations.len() > p.before.operations.len());
        assert_eq!(p.after.arguments, p.before.arguments);
        assert_eq!(p.after.extents[0], if equality { Some(9) } else { None });
        assert!(p.work_after > p.work_before);
    }
}

#[test]
fn optional_source_precision_rolls_back_new_extent_and_keeps_prebound_slots() {
    for prebound_extent in [false, true] {
        let p = probe(
            &equality_fixture(),
            true,
            Settings {
                node_work: Some(MAX_PURE_UNIFORM_INDEX_NODES_V1 - 2),
                prebound_extent,
                ..Settings::default()
            },
        );
        assert!(p.result.unwrap().is_empty());
        assert_eq!(p.after, p.before);
        assert!(p.work_after > p.work_before);
    }
}

#[test]
fn optional_source_precision_shared_exhaustion_is_still_fatal_and_not_refunded() {
    let p = probe(
        &equality_fixture(),
        true,
        Settings {
            shared_work: Some(MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            ..Settings::default()
        },
    );
    assert!(matches!(
        p.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(p.after, p.before);
    assert!(p.work_after >= MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn optional_source_precision_ranked_id_and_argument_overflow_remain_fatal() {
    for next_value in [true, false] {
        let p = probe(
            &equality_fixture(),
            true,
            Settings {
                next_value: next_value.then_some(u32::MAX),
                next_argument: (!next_value).then_some(usize::MAX),
                ..Settings::default()
            },
        );
        assert!(matches!(
            p.result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "too many ranked SSA values" | "too many volatile load slice extent arguments"
            ))
        ));
        assert_eq!(p.after, p.before);
        assert!(p.work_after > p.work_before);
    }
}

#[test]
fn optional_source_precision_does_not_change_mandatory_node_limit_or_counter_overflow() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let p = probe(
        &f,
        false,
        Settings {
            node_work: Some(MAX_PURE_UNIFORM_INDEX_NODES_V1),
            mandatory: true,
            ..Settings::default()
        },
    );
    assert!(matches!(
        p.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "pure uniform index expression exceeds its node limit"
        ))
    ));
    let p = probe(
        &f,
        false,
        Settings {
            node_work: Some(usize::MAX),
            ..Settings::default()
        },
    );
    assert!(matches!(
        p.result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "pure uniform index work accounting overflowed"
        ))
    ));
    assert_eq!(p.after, p.before);
}

#[test]
fn optional_source_precision_journal_work_is_charged_before_extent_publication() {
    let fresh = probe(&equality_fixture(), true, Settings::default());
    let prebound = probe(
        &equality_fixture(),
        true,
        Settings {
            prebound_extent: true,
            ..Settings::default()
        },
    );
    assert!(fresh.result.unwrap()[5].is_some());
    assert!(prebound.result.unwrap()[5].is_some());
    assert!(fresh.work_after - fresh.work_before > prebound.work_after - prebound.work_before);
    assert_eq!(fresh.after.extents[0], Some(9));
    assert_eq!(prebound.after.extents[0], Some(7));
}

#[test]
fn optional_source_precision_later_miss_discards_all_provisional_switches() {
    let mut f = equality_fixture();
    let mut flags = vec![5];
    for _ in 0..8 {
        flags.push(f.locals.len() as u32);
        f.locals.push(BOOL);
    }
    for (i, flag) in flags.iter().copied().enumerate() {
        let block_index = i + 5;
        let terminal = SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(flag, BOOL),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 3),
                )],
                edge(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    if i + 1 == flags.len() {
                        4
                    } else {
                        block_index + 1
                    },
                ),
            )
            .unwrap(),
        };
        let statements = vec![assign(
            flag,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: copy(7, WORD),
                right: field(6, 0, WORD),
            },
        )];
        if block_index == 5 {
            f.replace_block(block_index, statements, terminal);
        } else {
            f.blocks.push(block(block_index, statements, terminal));
        }
    }
    let positive = probe(&f, true, Settings::default());
    assert_eq!(
        positive.result.unwrap().iter().flatten().count(),
        flags.len()
    );
    let limited = probe(
        &f,
        true,
        Settings {
            node_work: Some(MAX_PURE_UNIFORM_INDEX_NODES_V1 - 5),
            ..Settings::default()
        },
    );
    assert!(limited.result.unwrap().is_empty());
    assert_eq!(limited.after, limited.before);
    assert!(limited.work_after > limited.work_before);
}
