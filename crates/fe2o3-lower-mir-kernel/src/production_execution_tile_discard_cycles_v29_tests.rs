mod cyclic_discard_tests {
    use super::*;

    fn conditional(yes: u32, no: u32) -> Terminator {
        Terminator::ConditionalBranch {
            condition: ValueId(1_000_002),
            then_target: BlockId(yes),
            then_arguments: vec![],
            else_target: BlockId(no),
            else_arguments: vec![],
        }
    }

    fn place(row: &mut PreparedLifecycleEventV29, block: usize) {
        row.block = block;
        row.witness.before.block = BlockId(block as u32);
        row.witness.after.block = BlockId(block as u32);
    }

    fn iteration(stage: u8) -> Fixture {
        let (_, mut events) = chain(stage);
        for row in &mut events[1..] {
            place(row, 1);
        }
        (body(vec![branch(1), conditional(1, 2), ret()]), events)
    }

    fn outer_scope(stage: u8) -> Fixture {
        let (_, mut events) = chain(stage);
        place(events.last_mut().unwrap(), 2);
        (body(vec![branch(1), conditional(1, 2), ret()]), events)
    }

    // Exercise the real canonical verifier on the importer's populated payload,
    // not only the preparer's own state machine or its success discriminant.
    fn canonical_replay((body, mut events): Fixture) {
        use fe2o3_kernel_ir::{
            AccessMode, AddressSpace, Kernel, LaunchDomain, LaunchExtent, Signature,
            StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18,
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(ROOM);
        let mut budget = ArgumentBudgetV1::new(&mut work, ROOM);
        budget.reserve_storage(FLOOR).unwrap();
        prepare_tile_discards_v29(&body, &mut events, &mut budget).unwrap();
        let mut blocks = body.blocks;
        for row in &mut events {
            assert_eq!(row.witness.before.first, 0);
            blocks[row.block]
                .operations
                .push(row.operation.take().unwrap());
        }
        let mut module = Module::new("cyclic_tile_discard");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    ),
                    Type::INDEX,
                    Type::BOOL,
                ],
                vec![],
            ),
            vec![ValueId(1_000_000), ValueId(1_000_001), ValueId(1_000_002)],
            blocks,
        ));
        module.kernels.push(Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        let limits = StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        };
        let retained = budget.storage();
        let (checked, _) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                limits,
                &mut budget,
            )
            .unwrap();
        assert_eq!(checked.module(), &module);
        assert_eq!(budget.storage(), retained);
        let (replayed, _) =
            VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                checked.canonical_bytes(),
                limits,
                &mut budget,
            )
            .unwrap();
        assert_eq!(replayed.module(), &module);
        assert_eq!(replayed.identity(), checked.identity());
        assert_eq!(replayed.canonical_bytes(), checked.canonical_bytes());
        assert_eq!(budget.storage(), retained);
        drop((checked, replayed, module, events));
        budget.release_storage(retained - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }

    #[test]
    fn cyclic_discard_balanced_iterations_and_loop_carried_descendants_replay() {
        for stage in 0..=2 {
            let expected = match stage {
                0 => vec![ValueId(2)],
                1 => vec![ValueId(3)],
                _ => vec![],
            };
            for build in [iteration as fn(u8) -> Fixture, outer_scope] {
                let observed = run_discard(build(stage), ROOM, ROOM);
                observed.result.unwrap();
                assert_eq!(observed.ends, vec![expected.clone()]);
                canonical_replay(build(stage));
            }
        }
    }

    fn topology(case: u8) -> Fixture {
        let (mut graph, events) = iteration(1);
        graph.blocks = match case {
            0 => {
                body(vec![
                    branch(1),
                    conditional(3, 4),
                    ret(),
                    branch(1),
                    conditional(1, 2),
                ])
                .blocks
            }
            1 => {
                body(vec![
                    conditional(1, 3),
                    conditional(3, 2),
                    ret(),
                    conditional(1, 2),
                ])
                .blocks
            }
            2 => {
                body(vec![
                    branch(1),
                    branch(3),
                    ret(),
                    conditional(3, 4),
                    conditional(1, 2),
                ])
                .blocks
            }
            3 => body(vec![branch(1), branch(1)]).blocks,
            4 => body(vec![conditional(1, 1), conditional(1, 2), ret()]).blocks,
            _ => unreachable!(),
        };
        (graph, events)
    }

    #[test]
    fn cyclic_discard_multiple_latches_irreducible_nested_and_nonreturning_cfgs_replay() {
        for case in 0..5 {
            let observed = run_discard(topology(case), ROOM, ROOM);
            observed.result.unwrap();
            assert_eq!(observed.ends, vec![vec![ValueId(3)]]);
            canonical_replay(topology(case));
        }
    }

    #[test]
    fn cyclic_discard_rejects_changed_backedge_state_reentry_and_stale_roles() {
        for fault in 0..7 {
            let (mut graph, mut events) = iteration(1);
            match fault {
                0 => graph.blocks[1].terminator = Some(conditional(0, 2)),
                1 => {
                    events.pop();
                }
                2 => {
                    events.push(event(1, 2, load(40, 1)));
                }
                3 => {
                    events.insert(events.len() - 1, event(1, 2, fragment(40, 2)));
                }
                4 => {
                    let fixture = outer_scope(1);
                    graph = fixture.0;
                    events = fixture.1;
                    place(&mut events[3], 1);
                }
                5 => {
                    let fixture = outer_scope(1);
                    graph = fixture.0;
                    events = fixture.1;
                    place(events.last_mut().unwrap(), 1);
                }
                6 => {
                    events.insert(events.len() - 1, event(1, 2, end(2)));
                }
                _ => unreachable!(),
            }
            refused(run_discard((graph, events), ROOM, ROOM));
        }
    }

    #[test]
    fn cyclic_discard_empty_unreachable_cycles_have_no_capability_authority() {
        let fixture = || {
            let (mut graph, events) = chain(2);
            let mut dead = BasicBlock::new(BlockId(1));
            dead.terminator = Some(branch(1));
            graph.blocks.push(dead);
            (graph, events)
        };
        let observed = run_discard(fixture(), ROOM, ROOM);
        observed.result.unwrap();
        canonical_replay(fixture());
        let (graph, mut events) = fixture();
        events.push(event(1, 2, load(40, 1)));
        refused(run_discard((graph, events), ROOM, ROOM));
        for missing in [false, true] {
            let (mut graph, events) = fixture();
            graph.blocks[1].terminator = if missing { None } else { Some(branch(99)) };
            refused(run_discard((graph, events), ROOM, ROOM));
        }
    }

    #[test]
    fn cyclic_discard_exact_and_one_short_resource_boundaries_are_typed() {
        for case in 0..5 {
            let measured = run_discard(topology(case), ROOM, ROOM);
            measured.result.unwrap();
            let exact = run_discard(topology(case), measured.work, measured.peak);
            exact.result.unwrap();
            assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
            assert_eq!(exact.ends, measured.ends);
            let short = run_discard(topology(case), measured.work - 1, measured.peak);
            assert!(matches!(
                short.result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            let short = run_discard(topology(case), measured.work, measured.peak - 1);
            assert!(matches!(
                short.result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        }
    }
}
