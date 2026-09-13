use super::*;

#[path = "tests/contradictory_traps.rs"]
mod contradictory_traps;

fn model() -> PrefixModel {
    let site = |block, operation| ConditionalPrefixSiteV1 { block, operation };
    let views = (0..3)
        .map(|index| ConditionalPrefixExtentV1 {
            view: site(0, index + 2),
            allocation_origin: u64::from(index) + 17,
            noalias_class: u64::from(index) + 17,
            source: ConditionalPrefixExtentSourceV1::RankedEntryArgument(index),
        })
        .collect();
    PrefixModel {
        views,
        writable: vec![true, false, false],
        output: 0,
        contract: site(0, 5),
        invocation: site(0, 1),
        launch: ConditionalPrefixLaunchV1 {
            layout: site(0, 0),
            grid_identity: 41,
            declared_workitems: 0,
        },
        blocks: vec![
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: site(0, 6),
                    extent: 0,
                    yes: 1,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: site(1, 0),
                    extent: 1,
                    yes: 2,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![(site(2, 0), 1, false)],
                terminator: PrefixTerminator::LessThan {
                    site: site(2, 1),
                    extent: 2,
                    yes: 3,
                    no: 4,
                },
            },
            PrefixBlock {
                accesses: vec![(site(3, 0), 2, false), (site(3, 1), 0, true)],
                terminator: PrefixTerminator::Goto(4),
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return,
            },
        ],
    }
}

#[test]
fn topological_walk_has_linear_work_even_with_reverse_block_storage() {
    for count in [1, 2, 16, 48, MAX_CONDITIONAL_PREFIX_BLOCKS_V1] {
        let expected = std::iter::once(0)
            .chain((1..count).rev())
            .collect::<Vec<_>>();
        let mut blocks = vec![
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return
            };
            count
        ];
        for pair in expected.windows(2) {
            blocks[pair[0]].terminator = PrefixTerminator::Goto(pair[1]);
        }
        let cost = 4 * count + 2 * (count - 1);
        let mut used = 0;
        assert_eq!(topological_blocks(&blocks, &mut used).unwrap(), expected);
        assert_eq!(used, cost);
        let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - cost;
        assert_eq!(topological_blocks(&blocks, &mut exact).unwrap(), expected);
        assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);
        let mut short = MAX_CONDITIONAL_PREFIX_WORK_V1 - cost + 1;
        assert_eq!(
            topological_blocks(&blocks, &mut short),
            Err(ConditionalPrefixDerivationErrorV1::Limit("work"))
        );
    }
}

#[test]
fn pure_derivation_rejects_missing_extra_or_negated_guards_and_all_cycles() {
    assert!(derive_model(&model()).is_ok());
    for mutation in 0..10 {
        let mut model = model();
        match mutation {
            0 => model.blocks[0].terminator = PrefixTerminator::Goto(1),
            1 => model.blocks[1].terminator = PrefixTerminator::Goto(2),
            2 => {
                let PrefixTerminator::LessThan { yes, no, .. } = &mut model.blocks[0].terminator
                else {
                    unreachable!()
                };
                std::mem::swap(yes, no);
            }
            3 => model.blocks[4].terminator = PrefixTerminator::Goto(0),
            4 => model.blocks[4].terminator = PrefixTerminator::Goto(99),
            5 => model.blocks[4].accesses.push((
                ConditionalPrefixSiteV1 {
                    block: 4,
                    operation: 0,
                },
                0,
                true,
            )),
            6 => model.blocks.push(PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Return,
            }),
            7 => model.views[1].allocation_origin = model.views[0].allocation_origin,
            8 => model.views[1].noalias_class = model.views[0].noalias_class,
            9 => {
                model.blocks[4].terminator = PrefixTerminator::LessThan {
                    site: ConditionalPrefixSiteV1 {
                        block: 4,
                        operation: 0,
                    },
                    extent: 0,
                    yes: 5,
                    no: 6,
                }
            }
            _ => unreachable!(),
        }
        if mutation == 9 {
            model.blocks.extend([
                PrefixBlock {
                    accesses: vec![],
                    terminator: PrefixTerminator::Return,
                },
                PrefixBlock {
                    accesses: vec![],
                    terminator: PrefixTerminator::Return,
                },
            ]);
        }
        assert!(derive_model(&model).is_err(), "mutation {mutation}");
    }
}

fn small_extent(binding: ConditionalPrefixExtentV1, lengths: &[u64; 3]) -> u64 {
    match binding.source {
        ConditionalPrefixExtentSourceV1::RankedEntryArgument(i) => lengths[i as usize],
        ConditionalPrefixExtentSourceV1::Constant(value) => value,
    }
}

// Execute actual terminators and accesses, independently of proof paths/DNF.
// The small environment and per-invocation visited set bound the oracle itself.
fn execute_small_cfg(
    model: &PrefixModel,
    lengths: &[u64; 3],
    workitems: u64,
) -> Result<Vec<usize>, &'static str> {
    assert!(model.blocks.len() <= MAX_CONDITIONAL_PREFIX_BLOCKS_V1);
    assert!(workitems <= 8);
    assert!(
        model
            .views
            .iter()
            .all(|&view| small_extent(view, lengths) <= 8)
    );
    let mut writes = vec![0; small_extent(model.views[model.output], lengths) as usize];
    for i in 0..workitems {
        let mut block = 0;
        let mut visited = vec![false; model.blocks.len()];
        loop {
            let current = model.blocks.get(block).ok_or("invalid target")?;
            if std::mem::replace(&mut visited[block], true) {
                return Err("cycle");
            }
            for &(_, view, write) in &current.accesses {
                if i >= small_extent(model.views[view], lengths) {
                    return Err("out-of-bounds access");
                }
                if write {
                    if view != model.output {
                        return Err("unexpected output");
                    }
                    writes[i as usize] += 1;
                }
            }
            block = match current.terminator {
                PrefixTerminator::Return => break,
                PrefixTerminator::Trap(_) => return Err("reachable trap"),
                PrefixTerminator::Goto(target) => target,
                PrefixTerminator::LessThan {
                    extent, yes, no, ..
                } => {
                    if i < small_extent(model.views[extent], lengths) {
                        yes
                    } else {
                        no
                    }
                }
            };
        }
    }
    Ok(writes)
}

fn check_small_theorem(model: &PrefixModel) {
    let proof = derive_model(model).unwrap();
    for n in 0..=6_u64 {
        for a in 0..=6_u64 {
            for b in 0..=6_u64 {
                for workitems in 0..=8_u64 {
                    let lengths = [n, a, b];
                    let holds = proof.conditions.iter().all(|condition| match *condition {
                        ConditionalPrefixConditionV1::OutputExtentAtMostInput { output, input } => {
                            small_extent(output, &lengths) <= small_extent(input, &lengths)
                        }
                        ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems {
                            output,
                            ..
                        } => small_extent(output, &lengths) <= workitems,
                    });
                    let observed = match execute_small_cfg(model, &lengths, workitems) {
                        Ok(writes) => writes,
                        Err("reachable trap") if !holds => continue,
                        other => panic!("unexpected execution with conditions={holds}: {other:?}"),
                    };
                    let covers = observed.iter().all(|&count| count == 1);
                    assert_eq!(holds, covers, "n={n}, a={a}, b={b}, workitems={workitems}");
                }
            }
        }
    }
}

#[test]
fn pure_conditions_are_exact_for_short_inputs_padded_launches_and_zero_output() {
    check_small_theorem(&model());
}

fn permuted_guards(order: [usize; 3]) -> PrefixModel {
    let mut model = model();
    model.blocks[2].accesses.clear();
    model.blocks[3].accesses = [(1, false), (2, false), (0, true)]
        .into_iter()
        .enumerate()
        .map(|(operation, (view, writes))| {
            (
                ConditionalPrefixSiteV1 {
                    block: 3,
                    operation: operation as u32,
                },
                view,
                writes,
            )
        })
        .collect();
    for (block, bound) in order.into_iter().enumerate() {
        let PrefixTerminator::LessThan { extent, .. } = &mut model.blocks[block].terminator else {
            unreachable!()
        };
        *extent = bound;
    }
    model
}

#[test]
fn independent_cfg_oracle_checks_guard_permutations_and_reconvergent_exits() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut model = permuted_guards(order);
        check_small_theorem(&model);
        // Distinct early-exit blocks reconverge at the common return. A true
        // edge also takes a forward goto to an earlier-listed guard block.
        for guard in 0..3 {
            let exit = model.blocks.len();
            model.blocks.push(PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(4),
            });
            let PrefixTerminator::LessThan { no, .. } = &mut model.blocks[guard].terminator else {
                unreachable!()
            };
            *no = exit;
        }
        let bridge = model.blocks.len();
        model.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Goto(2),
        });
        let PrefixTerminator::LessThan { yes, .. } = &mut model.blocks[1].terminator else {
            unreachable!()
        };
        *yes = bridge;
        check_small_theorem(&model);
    }
}

#[test]
fn independent_cfg_oracle_detects_early_exit_duplicate_writes_and_unguarded_reads() {
    let mut early = permuted_guards([0, 1, 2]);
    let PrefixTerminator::LessThan { yes, no, .. } = &mut early.blocks[2].terminator else {
        unreachable!()
    };
    std::mem::swap(yes, no);
    // Every stated inequality holds, but this CFG returns before the write.
    assert_eq!(execute_small_cfg(&early, &[2, 2, 2], 2), Ok(vec![0, 0]));
    assert!(derive_model(&early).is_err());

    let mut duplicate = permuted_guards([0, 1, 2]);
    let mut write = *duplicate.blocks[3].accesses.last().unwrap();
    write.0.operation += 1;
    duplicate.blocks[3].accesses.push(write);
    assert_eq!(execute_small_cfg(&duplicate, &[2, 2, 2], 2), Ok(vec![2, 2]));
    assert_eq!(
        derive_model(&duplicate).err(),
        Some(ConditionalPrefixDerivationErrorV1::DuplicateWrite)
    );

    let mut unguarded = permuted_guards([0, 1, 2]);
    let mut read = unguarded.blocks[3].accesses.remove(0);
    read.0 = ConditionalPrefixSiteV1 {
        block: 0,
        operation: 0,
    };
    unguarded.blocks[0].accesses.push(read);
    assert_eq!(
        execute_small_cfg(&unguarded, &[2, 1, 2], 2),
        Err("out-of-bounds access")
    );
    assert_eq!(
        derive_model(&unguarded).err(),
        Some(ConditionalPrefixDerivationErrorV1::UnguardedRead)
    );

    let mut cyclic = permuted_guards([0, 1, 2]);
    cyclic.blocks[4].terminator = PrefixTerminator::Goto(0);
    assert_eq!(execute_small_cfg(&cyclic, &[2, 2, 2], 2), Err("cycle"));
    assert_eq!(
        derive_model(&cyclic).err(),
        Some(ConditionalPrefixDerivationErrorV1::CyclicControlFlow)
    );
}

#[test]
fn pure_work_and_shape_budgets_fail_closed() {
    let mut work = 0;
    charge(&mut work, MAX_CONDITIONAL_PREFIX_WORK_V1).unwrap();
    assert!(charge(&mut work, 1).is_err());
    let mut overflowed_work = usize::MAX;
    assert!(charge(&mut overflowed_work, 1).is_err());
    let mut model = model();
    model.blocks.resize(
        MAX_CONDITIONAL_PREFIX_BLOCKS_V1 + 1,
        model.blocks[4].clone(),
    );
    assert_eq!(
        derive_model(&model).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("blocks/views"))
    );
}

#[test]
fn pure_dnf_term_and_guard_depth_budgets_fail_closed() {
    let mut branching = model();
    let original = branching.blocks.clone();
    branching.blocks.clear();
    for stage in 0..6 {
        let index = 3 * stage;
        branching.blocks.extend([
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::LessThan {
                    site: ConditionalPrefixSiteV1 {
                        block: index as u32,
                        operation: 0,
                    },
                    extent: 0,
                    yes: index + 1,
                    no: index + 2,
                },
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(index + 3),
            },
            PrefixBlock {
                accesses: vec![],
                terminator: PrefixTerminator::Goto(index + 3),
            },
        ]);
    }
    for mut block in original {
        for (site, _, _) in &mut block.accesses {
            site.block += 18;
        }
        match &mut block.terminator {
            PrefixTerminator::Goto(target) => *target += 18,
            PrefixTerminator::LessThan { site, yes, no, .. } => {
                site.block += 18;
                *yes += 18;
                *no += 18;
            }
            PrefixTerminator::Return | PrefixTerminator::Trap(_) => {}
        }
        branching.blocks.push(block);
    }
    assert!(matches!(
        derive_model(&branching).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit(
            "DNF terms" | "work"
        ))
    ));

    let mut deep = model();
    let write = deep.blocks[3].clone();
    deep.blocks[3].accesses.clear();
    for _ in 0..MAX_CONDITIONAL_PREFIX_GUARD_ATOMS_V1 {
        let index = deep.blocks.len();
        let previous = if index == 5 { 3 } else { index - 1 };
        deep.blocks[previous].terminator = PrefixTerminator::LessThan {
            site: ConditionalPrefixSiteV1 {
                block: previous as u32,
                operation: 0,
            },
            extent: 0,
            yes: index,
            no: 4,
        };
        deep.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Return,
        });
    }
    *deep.blocks.last_mut().unwrap() = write;
    assert_eq!(
        derive_model(&deep).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("guard atoms"))
    );
}
