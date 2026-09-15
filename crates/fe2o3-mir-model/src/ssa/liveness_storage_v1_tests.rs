//! Seeded liveness must retain the original plan identities and rejection behavior.

use crate::ssa::*;
use sha2::{Digest, Sha256};

fn variable(index: usize) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index as u32)
}

fn edge(role: u16, target: usize, definitions: &[usize]) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(role),
        SsaBlockIdV1::new(target as u32),
        definitions.iter().copied().map(variable).collect(),
    )
}

fn reference(input: &SsaConstructionInputV1, seeded: bool) -> Vec<Vec<bool>> {
    let blocks = input.blocks().len();
    let variables = input.variable_count() as usize;
    assert!(blocks <= 32 && variables <= 129);
    let mut reachable = vec![false; blocks];
    let mut pending = vec![input.entry().get() as usize];
    while let Some(block) = pending.pop() {
        if std::mem::replace(&mut reachable[block], true) {
            continue;
        }
        pending.extend(
            input.blocks()[block]
                .edges()
                .iter()
                .map(|edge| edge.target().get() as usize),
        );
    }
    let mut uses = vec![vec![false; variables]; blocks];
    let mut definitions = uses.clone();
    for (block, source) in input.blocks().iter().enumerate() {
        if !reachable[block] {
            continue;
        }
        for event in source.events() {
            let variable = event.variable().get() as usize;
            if !input.promotable()[variable] {
                continue;
            }
            match event {
                SsaEventV1::Use(_) if !definitions[block][variable] => uses[block][variable] = true,
                SsaEventV1::Define(_) | SsaEventV1::Kill(_) => definitions[block][variable] = true,
                SsaEventV1::Use(_) => {}
            }
        }
    }
    let mut live = if seeded {
        uses.clone()
    } else {
        vec![vec![false; variables]; blocks]
    };
    for _ in 0..=blocks * variables {
        let mut changed = false;
        for block in (0..blocks).rev() {
            if !reachable[block] {
                continue;
            }
            for variable in 0..variables {
                let out = input.blocks()[block].edges().iter().any(|edge| {
                    live[edge.target().get() as usize][variable]
                        && !edge.definitions().contains(&self::variable(variable))
                });
                let seed = if seeded {
                    live[block][variable]
                } else {
                    uses[block][variable]
                };
                let next = seed || out && !definitions[block][variable];
                assert!(
                    !live[block][variable] || next,
                    "fixed facts make liveness monotone"
                );
                changed |= live[block][variable] != next;
                live[block][variable] = next;
            }
        }
        if !changed {
            return live;
        }
    }
    panic!("finite bit lattice must converge");
}

fn check(input: &SsaConstructionInputV1) -> SsaConstructionPlanV1 {
    let expected = reference(input, false);
    assert_eq!(expected, reference(input, true));
    let plan = plan_ssa_v1(input).unwrap();
    for (block, row) in expected.iter().enumerate() {
        let block = SsaBlockIdV1::new(block as u32);
        if !plan.is_reachable(block) {
            assert!(plan.live_in(block).is_none());
            continue;
        }
        let expected = row
            .iter()
            .enumerate()
            .filter_map(|(index, live)| live.then_some(variable(index)))
            .collect::<Vec<_>>();
        assert_eq!(plan.live_in(block).unwrap(), expected);
    }
    plan.verify_replay(input, SsaPlannerLimitsV1::default())
        .unwrap();
    plan
}

#[test]
fn seeded_liveness_matches_three_matrix_recurrence() {
    let mut identities = Sha256::new();
    let mut state = 0x718cad4e09e25f63u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for variables in [1, 2, 63, 64, 65, 129] {
        for blocks in [1, 2, 5, 17] {
            for _ in 0..8 {
                let mut sources = Vec::new();
                for block in 0..blocks {
                    let mut events = Vec::new();
                    for _ in 0..4 {
                        let variable = variable(next() as usize % variables);
                        match next() % 3 {
                            0 => events.push(SsaEventV1::Use(variable)),
                            1 => events.push(SsaEventV1::Define(variable)),
                            _ => events
                                .extend([SsaEventV1::Kill(variable), SsaEventV1::Define(variable)]),
                        }
                    }
                    let mut edges = vec![edge(1, (block + 1) % blocks, &[])];
                    let defined = next() as usize % variables;
                    edges.push(edge(2, next() as usize % blocks, &[defined]));
                    edges.push(edge(3, next() as usize % blocks, &[]));
                    sources.push(SsaBlockInputV1::new(events, edges));
                }
                let input = SsaConstructionInputV1::new(
                    SsaBlockIdV1::new(0),
                    variables as u32,
                    (0..variables).map(|variable| variable % 5 != 3).collect(),
                    (0..variables).map(variable).collect(),
                    sources,
                );
                identities.update(check(&input).identity().as_bytes());
            }
        }
    }
    let digest: [u8; 32] = identities.finalize().into();
    assert_eq!(
        digest,
        [
            0xd8, 0x53, 0xa6, 0xd6, 0xb5, 0x13, 0xfe, 0xb2, 0x0a, 0xc9, 0xcb, 0x8a, 0x45, 0x3b,
            0x11, 0x6e, 0x98, 0x69, 0x54, 0x82, 0x78, 0x10, 0x15, 0x17, 0xc7, 0x2c, 0x76, 0x30,
            0x16, 0xa7, 0xb4, 0x3f,
        ]
    );
}

fn return_and_unwind(defined_at_entry: bool) -> SsaConstructionInputV1 {
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        2,
        vec![true, true],
        if defined_at_entry {
            vec![variable(0)]
        } else {
            vec![]
        },
        vec![
            SsaBlockInputV1::new(vec![], vec![edge(1, 1, &[0]), edge(2, 2, &[])]),
            SsaBlockInputV1::new(vec![], vec![edge(1, 3, &[])]),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![edge(1, 3, &[])]),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![]),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(1))], vec![edge(1, 4, &[])]),
        ],
    )
}

#[test]
fn return_definition_does_not_reach_unwind_and_dead_blocks_stay_dead() {
    let plan = check(&return_and_unwind(true));
    assert_eq!(plan.live_in(SsaBlockIdV1::new(0)).unwrap(), [variable(0)]);
    assert!(!plan.is_reachable(SsaBlockIdV1::new(4)));
    assert_eq!(
        plan.identity().to_string(),
        "6c368a746c6b1df026050b3dbaf5e768dcc2372de4f2e501de982b928b94c03b"
    );
    assert!(matches!(
        plan_ssa_v1(&return_and_unwind(false)),
        Err(SsaPlannerErrorV1::UndefinedAtEntry { .. })
            | Err(SsaPlannerErrorV1::UndefinedAtEdge { .. })
            | Err(SsaPlannerErrorV1::UndefinedAtUse { .. })
    ));
}

#[test]
fn kills_still_reject_undefined_uses() {
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![true],
        vec![variable(0)],
        vec![SsaBlockInputV1::new(
            vec![SsaEventV1::Kill(variable(0)), SsaEventV1::Use(variable(0))],
            vec![],
        )],
    );
    assert!(matches!(
        plan_ssa_v1(&input),
        Err(SsaPlannerErrorV1::UndefinedAtUse { .. })
    ));
}

#[test]
fn dense_flow_fits_without_the_duplicate_matrix_and_limits_stay_enforced() {
    let blocks = 4096;
    let variables = 10048;
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        variables,
        vec![true; variables as usize],
        vec![],
        (0..blocks)
            .map(|block| {
                SsaBlockInputV1::new(
                    vec![],
                    if block + 1 < blocks {
                        vec![edge(1, block + 1, &[])]
                    } else {
                        vec![]
                    },
                )
            })
            .collect(),
    );
    let plan =
        plan_ssa_v1(&input).expect("the actual smaller allocation fits the unchanged ceiling");
    assert_eq!(
        plan.identity().to_string(),
        "6f6cf414afe81d33dab0a7af18bcecdbe565ef503a14166091e8d4e1a8ce672a"
    );
    let storage = plan.resources().storage_words();
    let old_duplicate_words = blocks * (variables as usize).div_ceil(64);
    assert!(storage <= HARD_MAX_SSA_STORAGE_WORDS_V1);
    // This fixture has no kills, so the new rows contain headers and no payload.
    let window_headers = (blocks * std::mem::size_of::<(usize, Box<[u64]>)>()).div_ceil(8);
    let removed_definition_words = old_duplicate_words - window_headers;
    assert!(storage + removed_definition_words <= HARD_MAX_SSA_STORAGE_WORDS_V1);
    assert!(
        storage + removed_definition_words + old_duplicate_words > HARD_MAX_SSA_STORAGE_WORDS_V1
    );
    let defaults = SsaPlannerLimitsV1::default();
    let limits = |storage| {
        SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            defaults.max_events(),
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            storage,
            defaults.max_work_units(),
        )
        .unwrap()
    };
    assert_eq!(
        plan_ssa_with_limits_v1(&input, limits(storage)).unwrap(),
        plan
    );
    assert!(matches!(
        plan_ssa_with_limits_v1(&input, limits(storage - 1)),
        Err(SsaPlannerErrorV1::ResourceLimitExceeded {
            resource: SsaPlannerResourceV1::StorageWords,
            ..
        })
    ));
}
