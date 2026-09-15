use super::*;

fn setup(role: bool) -> (Formula, LiveEvents) {
    let p = formula::projected(role, &mut |_| true).unwrap();
    let reference = formula::reference(role, &mut |_| true).unwrap();
    assert!(p.len() < NODES && reference.len() < NODES);
    let graph = LiveEvents::new(&p, [None; INPUTS], &mut |_| true).unwrap();
    (reference, graph)
}

#[test]
fn independent_reference_matches_real_guarded_u16_producers_for_both_roles() {
    for role in [false, true] {
        let (reference, graph) = setup(role);
        graph.verify(&reference, &mut |_| true).unwrap();
        let results = graph.results().unwrap();
        for i in 0..4 {
            assert!(results[..i].iter().all(|previous| previous != &results[i]));
        }
        assert!(
            graph
                .reads
                .iter()
                .all(|read| read.as_ref().unwrap().guarded(&graph.context).is_some())
        );
        let def = Operation::get_op_dyn(graph.view.defining_op().unwrap(), &graph.context);
        assert!(
            def.downcast_ref::<RankedViewOp>()
                .unwrap()
                .allocation_origin(&graph.context)
                .is_none_or(|n| n == 0)
        );
    }
}

#[test]
fn source_leaf_extent_volatility_and_scalar_mutations_reject() {
    for case in 0..7 {
        let (expected, mut graph) = setup(false);
        let read = graph.reads[0].as_ref().unwrap();
        match case {
            0 => read.set_attr_kernel_semantic_read_volatility(
                &mut graph.context,
                SemanticReadVolatilityAttr::NonVolatile,
            ),
            1 => read.set_attr_kernel_semantic_read_ordering(
                &mut graph.context,
                SemanticReadOrderingAttr::Acquire,
            ),
            2 => read.set_attr_kernel_semantic_read_space(
                &mut graph.context,
                MemorySpaceAttr::Workgroup,
            ),
            3 => {
                read.set_attr_kernel_semantic_read_bit_width(&mut graph.context, DimensionAttr(32))
            }
            4 => graph.constants[7] = Some(1),
            5 => graph.constants[1] = Some(1),
            6 => graph.constants[2] = Some(0),
            _ => unreachable!(),
        }
        // Candidate mutation BEFORE correspondence construction; production has
        // no API to refresh an admitted graph's epoch or source bindings.
        graph.epoch = graph.context.ir_mutation_attempt_epoch().unwrap();
        assert!(
            matches!(
                graph.verify(&expected, &mut |_| true),
                Err(Error::Changed | Error::Graph)
            ),
            "case {case}"
        );
    }
}

#[test]
fn changed_live_guard_and_offset_fail_exact_structural_correspondence() {
    for kind in [true, false] {
        let (mut reference, graph) = setup(false);
        let other = formula::reference(true, &mut |_| true).unwrap();
        // Wrong independent source role changes the physical offset and guard.
        if kind {
            reference = other;
        } else {
            reference.events[0] = reference.events[1];
        }
        assert!(matches!(
            graph.verify(&reference, &mut |_| true),
            Err(Error::Changed)
        ));
    }
}

#[test]
fn read_reorder_omission_duplicate_and_stale_epoch_reject() {
    for case in 0..4 {
        let (expected, mut graph) = setup(false);
        match case {
            0 => graph.reads.swap(0, 1),
            1 => graph.reads[0] = None,
            2 => {
                graph.reads[1] = graph.reads[0]
                    .as_ref()
                    .map(|o| SemanticTypedReadOp::from_operation(o.get_operation()))
            }
            3 => {
                let read = graph.reads[0].as_ref().unwrap();
                read.set_attr_kernel_semantic_read_volatility(
                    &mut graph.context,
                    SemanticReadVolatilityAttr::Volatile,
                );
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                graph.verify(&expected, &mut |_| true),
                Err(Error::Roster | Error::Changed)
            ),
            "case {case}"
        );
    }
}

#[test]
fn unexpected_effect_cannot_join_the_retained_source_event_graph() {
    for kind in [AccessKindAttr::Read, AccessKindAttr::Write] {
        let (expected, mut graph) = setup(false);
        let view = if kind == AccessKindAttr::Write {
            let ty =
                RankedViewType::new(&mut graph.context, 16, true, vec![DYNAMIC_EXTENT]).unwrap();
            let view = RankedViewOp::new_in_space(
                &mut graph.context,
                ty,
                vec![graph.leaves[7].0],
                MemorySpaceAttr::Global,
            )
            .unwrap();
            graph.append(&view);
            view.result(&graph.context)
        } else {
            graph.view
        };
        let extra =
            RankedAccessOp::new(&mut graph.context, kind, view, vec![graph.leaves[7].0]).unwrap();
        graph.append(&extra);
        graph.epoch = graph.context.ir_mutation_attempt_epoch().unwrap();
        assert!(matches!(
            graph.verify(&expected, &mut |_| true),
            Err(Error::Graph)
        ));
    }
}

#[test]
fn actual_live_guard_index_and_fallback_operand_changes_reject() {
    for case in 0..4 {
        let (expected, mut graph) = setup(false);
        let read = graph.reads[0].as_ref().unwrap();
        let (guard, fallback) = read.guarded(&graph.context).unwrap();
        match case {
            0 => {
                let op = SemanticTypedConstantOp::from_operation(fallback.defining_op().unwrap());
                op.set_attr_kernel_semantic_typed_constant_bits(
                    &mut graph.context,
                    SemanticConstantAttr(1),
                );
            }
            1 => Operation::replace_operand(
                read.get_operation(),
                &graph.context,
                1,
                graph.leaves[1].0,
            ),
            2 => {
                let other = graph.reads[1]
                    .as_ref()
                    .unwrap()
                    .guarded(&graph.context)
                    .unwrap()
                    .0;
                assert_ne!(guard, other);
                Operation::replace_operand(read.get_operation(), &graph.context, 2, other);
            }
            3 => graph.lane_precondition = Some(guard),
            _ => unreachable!(),
        }
        graph.epoch = graph.context.ir_mutation_attempt_epoch().unwrap();
        assert!(
            matches!(graph.verify(&expected, &mut |_| true), Err(Error::Changed)),
            "case {case}"
        );
    }
}

#[test]
fn source_bound_constants_do_not_become_free_symbols() {
    let p = formula::projected(false, &mut |_| true).unwrap();
    let expected = formula::reference(false, &mut |_| true).unwrap();
    let mut inputs = [None; INPUTS];
    inputs[1] = Some(0);
    inputs[4] = Some(64);
    let mut graph = LiveEvents::new(&p, inputs, &mut |_| true).unwrap();
    graph.verify(&expected, &mut |_| true).unwrap();
    graph.constants[4] = None;
    assert_eq!(graph.verify(&expected, &mut |_| true), Err(Error::Changed));
}

#[test]
fn exact_budget_boundary_no_refund_or_partially_published_graph() {
    fn run(remaining: &mut usize) -> Result<()> {
        let mut charge = |words| {
            if let Some(next) = remaining.checked_sub(words) {
                *remaining = next;
                true
            } else {
                false
            }
        };
        let p = formula::projected(false, &mut charge)?;
        let graph = LiveEvents::new(&p, [None; INPUTS], &mut charge)?;
        let expected = formula::reference(false, &mut charge)?;
        graph.verify(&expected, &mut charge)
    }
    let mut remaining = 1_000_000;
    run(&mut remaining).unwrap();
    let exact = 1_000_000 - remaining;
    let mut remaining = exact;
    run(&mut remaining).unwrap();
    assert_eq!(remaining, 0);
    let mut remaining = exact - 1;
    assert!(matches!(run(&mut remaining), Err(Error::Budget)));
    assert!(remaining < exact - 1);
}

fn source(role_b: bool, x: [u64; 8], component: u64) -> Option<u64> {
    let [lane, offset, rows, columns, stride, first, second, extent] = x;
    let (minor_base, reduction_base) = if role_b {
        (second, first)
    } else {
        (first, second)
    };
    let minor = minor_base.checked_add(lane & 15)?;
    let k = reduction_base
        .checked_add((lane >> 4).checked_mul(4)?)?
        .checked_add(component)?;
    let (row, column) = if role_b { (k, minor) } else { (minor, k) };
    if row >= rows || column >= columns {
        return None;
    }
    let index = offset
        .checked_add(row.checked_mul(stride)?)?
        .checked_add(column)?;
    (index < extent).then_some(index)
}

#[test]
fn source_checked_arithmetic_oracle_covers_dynamic_tails_zero_stride_and_overflow() {
    for role in [false, true] {
        let expected = formula::reference(role, &mut |_| true).unwrap();
        let projected = formula::projected(role, &mut |_| true).unwrap();
        for lane in 0..64 {
            for n in [0, 1, 15, 16, 17, 255, u32::MAX as u64, u64::MAX] {
                for (offset, stride, base, extent) in [
                    (0, n, 0, u64::MAX),
                    (0, 0, 0, 1),
                    (u64::MAX, n, 0, u64::MAX),
                    (0, n, u64::MAX, u64::MAX),
                    (3, n, 16, 256),
                ] {
                    let inputs = [lane, offset, n, n, stride, base, base, extent];
                    let reference = formula::evaluate(&expected, inputs).unwrap();
                    assert_eq!(reference, formula::evaluate(&projected, inputs).unwrap());
                    for component in 0..4 {
                        let actual = source(role, inputs, component as u64);
                        assert_eq!(reference[component].0, actual.is_some());
                        if let Some(index) = actual {
                            assert_eq!(reference[component].1, index);
                        }
                    }
                }
            }
        }
    }
}
