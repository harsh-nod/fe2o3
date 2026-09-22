use super::*;

// Constituent setup plus a fixed six-block prefix, stopping before the new
// eight-unit query. Never run Facts::derive, build::derive or the query as oracle.
fn before_first_dead_query(loops: &Loops<'_, '_>, floor: usize) -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let expected = resources::scoped(&mut budget, |meter: &mut Meter<'_, '_>| {
        let limits = Limits::default();
        meter.derive(|b| Ok(loops.replay(loops.inventory(), limits, b)?))?;
        meter.reserve(size_of::<CanonicalKirInductionFactsV1<'_, '_, '_>>())?;
        let (rows, _) = meter.table::<Row>(loops.recurrences.len())?;
        let (sparse, _) = sparse_inputs(loops.inventory(), limits, meter)?;
        let (mut scratch, _) = Scratch::new(loops.inventory(), meter)?;
        let expected = meter.derive(|b| {
            let i = loops.inventory();
            assert_eq!(i.functions().len(), 1);
            assert_eq!(i.blocks().len(), 6);
            b.charge_work(2)?;
            with_canonical_kir_control_flow_v1(
                i.owner(),
                Function(0),
                Default::default(),
                b,
                |_, b| {
                    b.charge_work(2)?;
                    assert_eq!(loops.natural_loop(0, b)?.header, coordinate(1));
                    scratch.members(loops, 0, b)?;
                    let natural = loops.natural_loop(0, b)?;
                    let header = block_index(i, natural.header, b)?;
                    assert_eq!(loops.latch_edges(0, b)?.len(), 1);
                    b.charge_work(scratch.degrees.len())?;
                    scratch.degrees.fill(0);
                    scratch.reset(b)?;
                    let members = loops.members(0, b)?;
                    assert_eq!(members, [coordinate(1), coordinate(2), coordinate(4)]);
                    for block in &members[..2] {
                        b.charge_work(2)?;
                        let position = block_index(i, *block, b)?;
                        for _ in i.blocks()[position].operations.clone() {
                            b.charge_work(1)?;
                        }
                        if position == header {
                            continue;
                        }
                        let actual = &i.edges()[i.blocks()[position].edges.start];
                        b.charge_work(3)?;
                        let target = block_index(i, actual.target, b)?;
                        assert_eq!(
                            actual.coordinate,
                            Edge {
                                source: coordinate(2),
                                successor: 0
                            }
                        );
                        assert_eq!(i.blocks()[target].coordinate, coordinate(5));
                        assert_eq!(scratch.members[target], 0);
                        return Ok::<_, Error>((b.work(), b.peak_storage()));
                    }
                    panic!("fixed body side-edge prefix")
                },
            )
        })?;
        drop(scratch);
        drop(sparse);
        drop(rows);
        Ok::<_, Error>(expected)
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    expected
}

#[test]
fn dead_exit_first_query_has_an_independent_exact_denied_work_prefix() {
    with_loops(
        with_dead_exit(ScalarType::U64, 3, false, false),
        |loops, caller| {
            let floor = caller.storage();
            let (accepted, peak) = before_first_dead_query(loops, floor);
            for remaining in 0..8 {
                let limit = accepted + remaining;
                let short = measure(loops, floor, limit, LIMIT);
                let Err(Error::Resource(Resource::Work(error))) = short.result else {
                    panic!("new direct-Bool query admission: {short:?}")
                };
                assert_eq!((error.actual(), error.limit()), (accepted + 8, limit));
                assert_eq!(
                    (
                        short.work,
                        short.peak,
                        short.failed_work,
                        short.failed_storage
                    ),
                    (accepted, peak, Some(accepted + 8), None),
                );
            }
        },
    );
}

fn with_dead_exit(s: ScalarType, n: u64, value: bool, outer: bool) -> Module {
    let mut module = fixture(s, Some((0, n)), 1, false);
    let body = module.functions[0].body.as_mut().unwrap();
    let literal = KirOperation::effect_free(
        ValueDef::new(ValueId(60), Type::BOOL),
        OperationKind::Constant(Constant::Bool(value)),
    );
    body.blocks[if outer { 0 } else { 2 }]
        .operations
        .push(literal);
    body.blocks[2].terminator = Some(condition(
        60,
        if value { 71 } else { 101 },
        if value { 101 } else { 71 },
    ));
    body.blocks
        .push(basic(71, vec![], vec![], branch(11, vec![ValueId(5)])));
    body.blocks
        .push(basic(101, vec![], vec![], Terminator::Unreachable));
    module
}

#[test]
fn direct_bool_dead_exit_both_polarities_widths_trips_and_definition_locations() {
    let mut cases = 0;
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for n in [0, 1, 3, 8] {
            for value in [false, true] {
                for outer in [false, true] {
                    with_facts(with_dead_exit(scalar, n, value, outer), |facts, _| {
                        let fact = guarded(facts);
                        assert_eq!(fact.guard_distance(), Distance::Literal(n));
                        assert_eq!(fact.iteration_scope(), Iterations::NormalHeaderCompletion);
                        assert_eq!(
                            fact.guarded_update(),
                            if n == 0 {
                                Update::NoUpdate
                            } else {
                                Update::NonWrapping
                            }
                        );
                        assert!(!facts.grants_authority());
                        cases += 1;
                    });
                }
            }
        }
    }
    assert_eq!(cases, 64);
}

#[test]
fn direct_bool_dead_exit_does_not_admit_live_dynamic_or_derived_conditions() {
    for mode in 0..4 {
        let mut module = with_dead_exit(ScalarType::U32, 3, false, false);
        let function = &mut module.functions[0];
        if mode == 1 {
            function.signature.parameters.push(Type::BOOL);
            function.body.as_mut().unwrap().parameters.push(ValueId(61));
        }
        let body = function.body.as_mut().unwrap();
        match mode {
            0 => {
                body.blocks[2].operations.last_mut().unwrap().kind =
                    OperationKind::Constant(Constant::Bool(true))
            }
            1 => {
                let Some(Terminator::ConditionalBranch { condition, .. }) =
                    &mut body.blocks[2].terminator
                else {
                    panic!("condition")
                };
                *condition = ValueId(61);
            }
            2 => {
                body.blocks[2].operations.last_mut().unwrap().kind = OperationKind::Compare {
                    predicate: ComparePredicate::NotEqual,
                    lhs: ValueId(9),
                    rhs: ValueId(9),
                }
            }
            3 => {
                let Some(Terminator::ConditionalBranch { else_target, .. }) =
                    &mut body.blocks[2].terminator
                else {
                    panic!("condition")
                };
                *else_target = BlockId(100);
                body.blocks[4].terminator = Some(Terminator::Return { values: vec![] });
            }
            _ => unreachable!(),
        }
        with_facts(module, |facts, _| {
            if facts.rows().is_empty() {
                // Both successors leave: there is no reachable backedge recurrence.
                assert_eq!(mode, 3);
            } else {
                assert_eq!(guarded(facts).iteration_scope(), Iterations::Unavailable);
            }
        });
    }
}

#[test]
fn dead_external_edge_does_not_prune_an_internal_body_cycle() {
    let mut module = with_dead_exit(ScalarType::U32, 3, false, false);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(61));
    let Some(Terminator::ConditionalBranch { else_target, .. }) = &mut body.blocks[2].terminator
    else {
        panic!("body condition")
    };
    *else_target = BlockId(72);
    body.blocks
        .push(basic(72, vec![], vec![], condition(61, 72, 71)));
    with_facts(module, |facts, _| {
        let fact = guarded(facts);
        assert_eq!(fact.guarded_update(), Update::NonWrapping);
        assert_eq!(fact.iteration_scope(), Iterations::Unavailable);
    });
}

#[test]
fn dead_exit_induction_replay_rejects_forged_completion_and_distance() {
    with_facts(
        with_dead_exit(ScalarType::U64, 3, false, false),
        |facts, budget| {
            let floor = budget.storage();
            let saved = facts.rows[0];
            let Outcome::Guarded(ref mut fact) = facts.rows[0].outcome else {
                panic!("guarded")
            };
            fact.iterations = Iterations::Unavailable;
            assert!(matches!(
                facts.replay(facts.loops(), Limits::default(), budget),
                Err(Error::ReplayMismatch)
            ));
            assert_eq!(budget.storage(), floor);
            facts.rows[0] = saved;
            let Outcome::Guarded(ref mut fact) = facts.rows[0].outcome else {
                panic!("guarded")
            };
            fact.distance = Distance::Literal(2);
            assert!(matches!(
                facts.replay(facts.loops(), Limits::default(), budget),
                Err(Error::ReplayMismatch)
            ));
            assert_eq!(budget.storage(), floor);
            facts.rows[0] = saved;
            facts
                .replay(facts.loops(), Limits::default(), budget)
                .unwrap();
        },
    );
}

#[derive(Debug)]
struct Measured {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(loops: &Loops<'_, '_>, floor: usize, w: usize, s: usize) -> Measured {
    let mut work = Work::new(w);
    let (result, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = CanonicalKirInductionFactsV1::derive(loops, Limits::default(), &mut budget)
            .map(|(facts, receipt)| {
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(
                    receipt.retained_storage(),
                    size_of_val(&facts) + facts.rows.capacity() * size_of::<Row>()
                );
                assert_eq!(
                    guarded(&facts).iteration_scope(),
                    Iterations::NormalHeaderCompletion
                );
                drop(facts);
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Measured {
        result,
        work: work.work(),
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn dead_exit_induction_paid_boundaries_preserve_ledger_and_backing() {
    with_loops(
        with_dead_exit(ScalarType::U64, 3, false, false),
        |loops, caller| {
            let floor = caller.storage();
            let full = measure(loops, floor, LIMIT, LIMIT);
            assert!(full.result.is_ok(), "{full:?}");
            let exact = measure(loops, floor, full.work, full.peak);
            assert!(exact.result.is_ok(), "{exact:?}");
            assert_eq!(
                (
                    exact.work,
                    exact.peak,
                    exact.failed_work,
                    exact.failed_storage
                ),
                (full.work, full.peak, None, None)
            );
            let short = measure(loops, floor, full.work - 1, full.peak);
            let Err(Error::Resource(Resource::Work(error))) = short.result else {
                panic!("final replay work: {short:?}")
            };
            assert_eq!((error.actual(), error.limit()), (full.work, full.work - 1));
            assert_eq!(
                (short.work, short.failed_work, short.failed_storage),
                (full.work - 3, Some(full.work), None)
            );
            let storage = measure(loops, floor, LIMIT, full.peak - 1);
            let Err(Error::Resource(Resource::Storage(error))) = storage.result else {
                panic!("typed induction storage: {storage:?}")
            };
            assert_eq!((error.actual(), error.limit()), (full.peak, full.peak - 1));
            assert_eq!(
                (storage.failed_work, storage.failed_storage),
                (None, Some(full.peak))
            );
        },
    );
}
