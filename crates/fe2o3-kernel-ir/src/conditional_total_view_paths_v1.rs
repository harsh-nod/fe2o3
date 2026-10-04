//! Conditional CFG coverage and input-guard implications over retained read domains.
use super::{
    BlockState, Budget, CastKind, ComparePredicate, ConditionalTotalViewAddressDomainV1,
    ConditionalTotalViewFactsV1, ConditionalTotalViewReadV1, Definition, Derived, Function,
    FunctionOperationLocation, IndexedControlFlow, OperationKind, ResourceError, Terminator,
    Unsupported, ValueId, allocate, allowed_operation, definition, position, refuse,
    verification_find_last_by_v1,
};

fn switch_predicate(
    selector: ValueId,
    definitions: &[Definition<'_>],
    predicate: ValueId,
    budget: &mut Budget<'_>,
) -> Derived<bool> {
    let selector = definition(definitions, selector, budget)?;
    budget.charge_work(2)?;
    Ok(
        matches!(selector.operation.kind, OperationKind::Cast { kind: CastKind::ZeroExtend, value, .. } if value == predicate),
    )
}

fn implied_input_predicate(
    condition: ValueId,
    definitions: &[Definition<'_>],
    facts: &ConditionalTotalViewFactsV1<'_>,
    reads: &[ConditionalTotalViewReadV1],
    budget: &mut Budget<'_>,
) -> Derived<[Option<usize>; 2]> {
    let Some(position) =
        verification_find_last_by_v1(definitions, 1, budget, |row| row.value.cmp(&condition))?
    else {
        return Ok([None, None]);
    };
    let condition = definitions[position];
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = condition.operation.kind
    else {
        return Ok([None, None]);
    };
    if lhs != facts.index {
        return Ok([None, None]);
    }
    let Some(position) =
        verification_find_last_by_v1(definitions, 1, budget, |row| row.value.cmp(&rhs))?
    else {
        return Ok([None, None]);
    };
    let length = definitions[position];
    let OperationKind::SliceLength { slice } = length.operation.kind else {
        return Ok([None, None]);
    };
    let mut choice = [None, None];
    for read in reads {
        budget.charge_work(4)?;
        if read.slice == slice && read.index == lhs {
            choice[1] = Some(0);
            if read.access_domain == ConditionalTotalViewAddressDomainV1::GlobalLaunch {
                choice[0] = Some(0);
            }
        }
    }
    Ok(choice)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_paths(
    function: &Function,
    flow: &IndexedControlFlow,
    definitions: &[Definition<'_>],
    states: &mut [BlockState],
    facts: &ConditionalTotalViewFactsV1<'_>,
    pointer_location: FunctionOperationLocation,
    selected_offset: bool,
    reads: Option<&[ConditionalTotalViewReadV1]>,
    budget: &mut Budget<'_>,
) -> Derived<ConditionalTotalViewAddressDomainV1> {
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let mut reachable_count = 0_usize;
    let mut order = allocate::<usize>(states.len(), budget)?;
    for (source, block) in body.blocks.iter().enumerate() {
        budget.charge_work(2)?;
        if !states[source].reachable {
            continue;
        }
        reachable_count = reachable_count
            .checked_add(1)
            .ok_or(ResourceError::Arithmetic)?;
        let lookup = position(flow, block.id, budget)?;
        if lookup != source {
            return Err(ResourceError::Accounting.into());
        }
        let edges = flow
            .outgoing_edges(block.id)
            .ok_or(ResourceError::Accounting)?;
        for edge in edges {
            budget.charge_work(2)?;
            let target = position(
                flow,
                flow.edge_target(edge).ok_or(ResourceError::Accounting)?,
                budget,
            )?;
            states[target].indegree = states[target]
                .indegree
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
        }
    }
    for (index, state) in states.iter().enumerate() {
        budget.charge_work(1)?;
        if state.reachable && state.indegree == 0 {
            order.push(index);
        }
    }
    let mut cursor = 0;
    while cursor < order.len() {
        budget.charge_work(2)?;
        let source = order[cursor];
        cursor += 1;
        let block = &body.blocks[source];
        position(flow, block.id, budget)?;
        for edge in flow
            .outgoing_edges(block.id)
            .ok_or(ResourceError::Accounting)?
        {
            budget.charge_work(2)?;
            let target = position(
                flow,
                flow.edge_target(edge).ok_or(ResourceError::Accounting)?,
                budget,
            )?;
            states[target].indegree = states[target]
                .indegree
                .checked_sub(1)
                .ok_or(ResourceError::Accounting)?;
            if states[target].indegree == 0 {
                order.push(target);
            }
        }
    }
    if order.len() != reachable_count {
        return refuse(Unsupported::Cycle);
    }
    let entry = states.first_mut().ok_or(ResourceError::Accounting)?;
    entry.counts = [1, 1];
    let mut address_domain = ConditionalTotalViewAddressDomainV1::GuardedOutput;
    for source in order {
        budget.charge_work(4)?;
        let block = &body.blocks[source];
        let mut counts = states[source].counts;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            budget.charge_work(8)?;
            let location = FunctionOperationLocation::new(block.id, ordinal);
            if matches!(operation.kind, OperationKind::Call { .. }) {
                // The discovery pass cannot yet exclude bounds-panic paths.
                // No callee identity is trusted: every reachable call still fails.
                if reads.is_some() && counts != [0, 0] {
                    return refuse(Unsupported::Call { location });
                }
            } else if !allowed_operation(operation) {
                return refuse(Unsupported::Operation { location });
            }
            if location == pointer_location && !selected_offset && counts[0] != 0 {
                address_domain = ConditionalTotalViewAddressDomainV1::GlobalLaunch;
            }
            if location == facts.store_location {
                for (case, count) in counts.iter_mut().enumerate() {
                    if case == 0 && facts.store_predicate.is_some() {
                        continue;
                    }
                    *count = ((*count & 1) << 1) | (u8::from(*count & 6 != 0) << 2);
                }
            }
        }
        let terminator = block.terminator.as_ref().ok_or(ResourceError::Accounting)?;
        let choice = match terminator {
            Terminator::Return { values } if values.is_empty() => {
                for (case, count) in counts.iter().copied().enumerate() {
                    let expected = if case == 0 { 1 } else { 2 };
                    if reads.is_some() && count != 0 && count != expected {
                        return refuse(Unsupported::WriteCount {
                            block: block.id,
                            predicate_true: case == 1,
                        });
                    }
                }
                continue;
            }
            Terminator::Unreachable => {
                if reads.is_some() && counts != [0, 0] {
                    return refuse(Unsupported::AbnormalExit { block: block.id });
                }
                continue;
            }
            Terminator::Branch { .. } => [None, None],
            Terminator::ConditionalBranch { condition, .. } if *condition == facts.predicate => {
                [Some(1), Some(0)]
            }
            Terminator::ConditionalBranch { condition, .. } => match reads {
                Some(reads) => {
                    implied_input_predicate(*condition, definitions, facts, reads, budget)?
                }
                None => [None, None],
            },
            Terminator::Switch {
                selector, cases, ..
            } if cases.len() == 2 => {
                budget.charge_work(4)?;
                let choices = match (cases[0].value, cases[1].value) {
                    (0, 1) => [0, 1],
                    (1, 0) => [1, 0],
                    _ => return refuse(Unsupported::Terminator { block: block.id }),
                };
                if switch_predicate(*selector, definitions, facts.predicate, budget)? {
                    [Some(choices[0]), Some(choices[1])]
                } else {
                    let selector = definition(definitions, *selector, budget)?;
                    let OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value,
                        ..
                    } = selector.operation.kind
                    else {
                        return refuse(Unsupported::Terminator { block: block.id });
                    };
                    let implied = match reads {
                        Some(reads) => {
                            implied_input_predicate(value, definitions, facts, reads, budget)?
                        }
                        None => [None, None],
                    };
                    if reads.is_some() && implied == [None, None] {
                        return refuse(Unsupported::Terminator { block: block.id });
                    }
                    implied.map(|choice| choice.map(|_| choices[1]))
                }
            }
            _ => return refuse(Unsupported::Terminator { block: block.id }),
        };
        position(flow, block.id, budget)?;
        for edge in flow
            .outgoing_edges(block.id)
            .ok_or(ResourceError::Accounting)?
        {
            budget.charge_work(6)?;
            let ordinal = flow.edge(edge).ok_or(ResourceError::Accounting)?.ordinal();
            let target = position(
                flow,
                flow.edge_target(edge).ok_or(ResourceError::Accounting)?,
                budget,
            )?;
            for (case, count) in counts.iter().copied().enumerate() {
                if choice[case].is_none_or(|selected| selected == ordinal) {
                    states[target].counts[case] |= count;
                }
            }
        }
    }
    Ok(address_domain)
}
