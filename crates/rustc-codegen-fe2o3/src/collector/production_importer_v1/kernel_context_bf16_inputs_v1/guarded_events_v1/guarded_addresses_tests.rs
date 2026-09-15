use super::*;

fn recipe(project: bool, role_b: bool) -> Formula {
    if project {
        projected(role_b, &mut |_| true).unwrap()
    } else {
        reference(role_b, &mut |_| true).unwrap()
    }
}

#[test]
fn both_roles_and_independent_builders_have_guarded_unsigned_addresses() {
    for project in [false, true] {
        for role_b in [false, true] {
            check(&recipe(project, role_b), &mut |_| true).unwrap();
        }
    }
}

#[test]
fn exact_index_extent_relation_is_required_for_each_event() {
    for component in 0..4 {
        for mutation in 0..3 {
            let mut p = recipe(true, false);
            let event = p.events[component];
            let physical = (0..p.len())
                .find(|&i| {
                    matches!(p.node(i as Id),
                Ok(Node::Compare(Compare::Less, index, 7)) if index == event.index)
                })
                .unwrap();
            p.nodes[physical] = match mutation {
                0 => Node::Bool(true),
                1 => Node::Compare(Compare::Less, event.index, 6),
                2 => Node::Compare(Compare::Less, 1, 7),
                _ => unreachable!(),
            };
            assert_eq!(check(&p, &mut |_| true), Err(Error::Changed));
        }
    }
}

#[test]
fn missing_overflow_witnesses_reject_even_when_tail_bounds_remain() {
    for project in [false, true] {
        for role_b in [false, true] {
            let p = recipe(project, role_b);
            let mut tested_add = 0;
            let mut tested_mul = 0;
            for i in 0..p.len() {
                let mut changed = recipe(project, role_b);
                match p.node(i as Id).unwrap() {
                    Node::Compare(Compare::LessEqual, _, room)
                        if matches!(p.node(room), Ok(Node::Binary(Binary::Subtract, ..))) =>
                    {
                        changed.nodes[i] = Node::Bool(true);
                        // The component-zero +0 operation cannot overflow and
                        // needs no witness. Every other changed witness fails.
                        let Node::Compare(_, b, _) = p.nodes[i] else {
                            unreachable!()
                        };
                        if word(&p, b, 0) {
                            continue;
                        }
                        tested_add += 1;
                    }
                    Node::Select {
                        boolean: true,
                        yes,
                        no,
                        ..
                    } if boolean(&p, yes, true)
                        && matches!(p.node(no), Ok(Node::Compare(Compare::LessEqual, ..))) =>
                    {
                        changed.nodes[i] = Node::Bool(true);
                        tested_mul += 1;
                    }
                    _ => continue,
                }
                assert_eq!(
                    check(&changed, &mut |_| true),
                    Err(Error::Changed),
                    "node {i}"
                );
            }
            assert!(tested_add >= 10);
            assert_eq!(tested_mul, 4);
        }
    }
}

#[test]
fn multiply_requires_same_operands_and_total_zero_stride_witness() {
    for case in 0..4 {
        let mut p = recipe(true, true);
        let selection = (0..p.len())
            .find(|&i| matches!(p.nodes[i], Node::Select { boolean: false, .. }))
            .unwrap();
        let Node::Select {
            condition, yes, no, ..
        } = p.nodes[selection]
        else {
            unreachable!()
        };
        match case {
            0 => p.nodes[usize::from(yes)] = Node::Word(0),
            1 => {
                p.nodes[selection] = Node::Select {
                    boolean: false,
                    condition,
                    yes,
                    no: 3,
                }
            }
            2 => {
                let Node::Compare(op, _, zero) = p.nodes[usize::from(condition)] else {
                    unreachable!()
                };
                p.nodes[usize::from(condition)] = Node::Compare(op, 3, zero);
            }
            3 => {
                let quotient = (selection + 1..p.len())
                    .find(|&i| {
                        matches!(p.nodes[i],
                    Node::Binary(Binary::Divide, _, d) if usize::from(d) == selection)
                    })
                    .unwrap();
                let bound = (quotient + 1..p.len())
                    .find(|&i| {
                        matches!(p.nodes[i],
                    Node::Compare(Compare::LessEqual, _, q) if usize::from(q) == quotient)
                    })
                    .unwrap();
                p.nodes[bound] = Node::Compare(Compare::LessEqual, no, quotient as Id);
            }
            _ => unreachable!(),
        }
        assert_eq!(check(&p, &mut |_| true), Err(Error::Changed), "case {case}");
    }
}

#[test]
fn forward_edges_type_drift_and_zero_address_divisor_are_rejected() {
    for case in 0..3 {
        let mut p = recipe(true, false);
        let divide = (0..p.len())
            .find(|&i| matches!(p.nodes[i], Node::Binary(Binary::Divide, 0, _)))
            .unwrap();
        let Node::Binary(_, a, b) = p.nodes[divide] else {
            unreachable!()
        };
        match case {
            0 => p.nodes[divide] = Node::Binary(Binary::Divide, divide as Id, b),
            1 => p.nodes[usize::from(b)] = Node::Bool(true),
            2 => p.nodes[usize::from(b)] = Node::Word(0),
            _ => unreachable!(),
        }
        assert_eq!(a, 0);
        assert_eq!(
            check(&p, &mut |_| true),
            Err(if case == 2 {
                Error::Changed
            } else {
                Error::Graph
            })
        );
    }
}

#[test]
fn charged_check_has_exact_boundary_and_never_refunds_work() {
    let p = recipe(true, false);
    let mut charged = 0;
    check(&p, &mut |n| {
        charged += n;
        true
    })
    .unwrap();
    assert!(charged > NODES);
    let mut budget = charged;
    check(&p, &mut |n| {
        let Some(next) = budget.checked_sub(n) else {
            return false;
        };
        budget = next;
        true
    })
    .unwrap();
    assert_eq!(budget, 0);
    let mut budget = charged - 1;
    assert_eq!(
        check(&p, &mut |n| {
            let Some(next) = budget.checked_sub(n) else {
                return false;
            };
            budget = next;
            true
        }),
        Err(Error::Budget)
    );
    assert!(budget < charged - 1);
}

#[test]
fn address_theorem_does_not_discharge_pending_lane_numerical_premise() {
    let mut p = recipe(false, true);
    // Address safety is conditional on guards even for arbitrary u64 lane.
    // This is not acceptance of a changed live producer: LiveEvents separately
    // compares the exact source's lane<64 precondition during graph replay.
    p.nodes[usize::from(p.precondition)] = Node::Bool(false);
    check(&p, &mut |_| true).unwrap();
    let mut proved = 0;
    for lane in [0, 15, 63, 64, u64::MAX] {
        for stride in [0, 1, 64, u64::MAX] {
            let input = [lane, 0, u64::MAX, u64::MAX, stride, 0, 0, u64::MAX];
            for (component, (guard, index)) in evaluate(&p, input).unwrap().into_iter().enumerate()
            {
                if !guard {
                    continue;
                }
                let row = u128::from(lane / 16) * 4 + component as u128;
                let column = u128::from(lane % 16);
                let exact = row * u128::from(stride) + column;
                assert_eq!(u128::from(index), exact);
                assert!(index < input[7]);
                proved += 1;
            }
        }
    }
    assert!(proved > 0);
}
