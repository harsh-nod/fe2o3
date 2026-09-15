use super::*;

fn setup(role_b: bool) -> (Formula, LiveEvents) {
    let source = formula::reference(role_b, &mut |_| true).unwrap();
    let projected = formula::projected(role_b, &mut |_| true).unwrap();
    let graph = LiveEvents::new(&projected, [None; INPUTS], &mut |_| true).unwrap();
    (source, graph)
}

#[test]
fn live_memory_observations_bind_exact_ordered_scalar_results() {
    for role in [false, true] {
        let (expected, graph) = setup(role);
        graph
            .with_live_read_sequence(&expected, &mut |_| true, |sequence, charge| {
                let results = sequence.result_ports(charge)?;
                sequence.check_result_ports(&results, charge)?;
                for ordinal in 0..4 {
                    let event = sequence.observation(ordinal).unwrap();
                    let read = graph.reads[ordinal].as_ref().unwrap();
                    assert_eq!(event.producer(), read.get_operation());
                    assert_eq!(event.result(), results[ordinal].value());
                    assert_eq!(event.view(), graph.view);
                    assert_eq!(
                        (event.guard(), event.fallback()),
                        read.guarded(sequence.context()).unwrap()
                    );
                    assert_eq!(event.index(), read.indices(sequence.context()).unwrap()[0]);
                    assert_eq!(
                        event.preceding(),
                        ordinal
                            .checked_sub(1)
                            .map(|i| sequence.observation(i).unwrap().producer())
                    );
                }
                assert!(sequence.observation(4).is_none());
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn unused_and_coincident_reads_remain_four_distinct_observations() {
    let expected = formula::reference(true, &mut |_| true).unwrap();
    let projected = formula::projected(true, &mut |_| true).unwrap();
    let inputs = [0, 0, 64, 64, 0, 0, 0, 1];
    assert_eq!(
        formula::evaluate(&expected, inputs).unwrap(),
        [(true, 0); 4]
    );
    let graph = LiveEvents::new(&projected, inputs.map(Some), &mut |_| true).unwrap();
    graph
        .with_live_read_sequence(&expected, &mut |_| true, |sequence, charge| {
            let ports = sequence.result_ports(charge)?;
            let results = ports.map(|port| port.value());
            for (i, result) in results.iter().enumerate() {
                assert!(!results[..i].contains(result));
                // No produced scalar is used by this component function. Its four
                // volatile operations must still be present in the observed chain.
                for op in graph
                    .function
                    .get_entry_block(sequence.context())
                    .deref(sequence.context())
                    .iter(sequence.context())
                {
                    let op = op.deref(sequence.context());
                    for operand in 0..op.get_num_operands() {
                        assert_ne!(op.get_operand(operand), *result);
                    }
                }
            }
            sequence.check_result_ports(&ports, charge)
        })
        .unwrap();
}

#[test]
fn result_reorder_duplicate_omission_and_foreign_context_function_reject() {
    let (expected, graph) = setup(false);
    let (_, foreign) = setup(false);
    graph
        .with_live_read_sequence(&expected, &mut |_| true, |sequence, charge| {
            let results = sequence.result_ports(charge)?;
            let mut changed = results;
            changed.swap(0, 1);
            assert_eq!(
                sequence.check_result_ports(&changed, charge),
                Err(Error::Changed)
            );
            changed = results;
            changed[1] = changed[0];
            assert_eq!(
                sequence.check_result_ports(&changed, charge),
                Err(Error::Changed)
            );
            assert_eq!(
                sequence.check_result_ports(&results[..3], charge),
                Err(Error::Roster)
            );
            let extra = [results[0], results[1], results[2], results[3], results[0]];
            assert_eq!(
                sequence.check_result_ports(&extra, charge),
                Err(Error::Roster)
            );
            // Reproducer: arena-local handles coincide but owners do not. A raw
            // Value+caller-selected Context API incorrectly accepted this case.
            assert_eq!(
                foreign.function.get_operation(),
                graph.function.get_operation()
            );
            assert_eq!(foreign.results()?, graph.results()?);
            foreign.with_live_read_sequence(&expected, charge, |foreign_sequence, charge| {
                assert_eq!(
                    sequence.check_result_ports(&foreign_sequence.result_ports(charge)?, charge),
                    Err(Error::Changed)
                );
                Ok(())
            })?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn read_only_context_mutation_is_caught_after_consumer_including_error_path() {
    for fail in [false, true] {
        let (expected, graph) = setup(false);
        let result = graph.with_live_read_sequence(&expected, &mut |_| true, |sequence, _| {
            let first = sequence.observation(0).unwrap();
            let other = sequence.observation(1).unwrap();
            Operation::replace_operand(first.producer(), sequence.context(), 3, other.result());
            if fail { Err(Error::Source) } else { Ok(()) }
        });
        assert_eq!(result, Err(Error::Changed));
    }
}

#[test]
fn source_guard_fallback_and_volatile_identity_are_rechecked_before_ports() {
    for case in 0..3 {
        let (expected, mut graph) = setup(false);
        let read = graph.reads[0].as_ref().unwrap();
        match case {
            0 => read.set_attr_kernel_semantic_read_volatility(
                &mut graph.context,
                SemanticReadVolatilityAttr::NonVolatile,
            ),
            1 => Operation::replace_operand(
                read.get_operation(),
                &graph.context,
                3,
                graph.results().unwrap()[1],
            ),
            2 => Operation::replace_operand(
                read.get_operation(),
                &graph.context,
                2,
                graph.reads[1]
                    .as_ref()
                    .unwrap()
                    .guarded(&graph.context)
                    .unwrap()
                    .0,
            ),
            _ => unreachable!(),
        }
        // Candidate mutation before receipt formation, not a production refresh.
        graph.epoch = graph.context.ir_mutation_attempt_epoch().unwrap();
        let mut called = false;
        assert_eq!(
            graph.with_live_read_sequence(&expected, &mut |_| true, |_, _| {
                called = true;
                Ok(())
            }),
            Err(Error::Changed)
        );
        assert!(!called);
    }
}

#[test]
fn extra_memory_effect_cannot_be_hidden_by_a_four_result_roster() {
    let (expected, mut graph) = setup(false);
    let extra = RankedAccessOp::new(
        &mut graph.context,
        AccessKindAttr::Read,
        graph.view,
        vec![graph.leaves[7].0],
    )
    .unwrap();
    graph.append(&extra);
    graph.epoch = graph.context.ir_mutation_attempt_epoch().unwrap();
    assert_eq!(
        graph.with_live_read_sequence(&expected, &mut |_| true, |_, _| Ok(())),
        Err(Error::Graph)
    );
}

#[test]
fn exact_shared_work_boundary_and_consumer_errors_are_preserved() {
    let (expected, graph) = setup(true);
    let mut used = 0usize;
    graph
        .with_live_read_sequence(
            &expected,
            &mut |n| {
                used += n;
                true
            },
            |sequence, charge| sequence.check_result_ports(&sequence.result_ports(charge)?, charge),
        )
        .unwrap();
    for offset in [0, 1] {
        let mut remaining = used - offset;
        let result = graph.with_live_read_sequence(
            &expected,
            &mut |n| {
                let Some(next) = remaining.checked_sub(n) else {
                    return false;
                };
                remaining = next;
                true
            },
            |sequence, charge| sequence.check_result_ports(&sequence.result_ports(charge)?, charge),
        );
        assert_eq!(
            result,
            if offset == 0 {
                Ok(())
            } else {
                Err(Error::Budget)
            }
        );
        assert!(remaining < used - offset);
    }
    assert_eq!(
        graph.with_live_read_sequence::<()>(&expected, &mut |_| true, |_, _| Err(Error::Source)),
        Err(Error::Source)
    );
}
