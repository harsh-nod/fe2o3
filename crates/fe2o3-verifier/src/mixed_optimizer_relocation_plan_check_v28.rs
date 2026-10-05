//! Independent expression, availability and complete CFG-cut replay.
use super::*;

pub(super) fn replay(
    plan: &RelocationExpressionPlanV28<'_>,
    pair: &Pair<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.check_prior_denials_v1()?;
    if !std::ptr::eq(plan.input, pair.input())
        || !std::ptr::eq(plan.output, pair.output())
        || plan.origins.len() != pair.origins().len()
        || !std::ptr::eq(plan.origins.as_ptr(), pair.origins().as_ptr())
    {
        return Err(Error::Mismatch(
            "foreign checked relocation endpoints or rows",
        ));
    }
    let floor = budget.storage();
    let mut paid = 0;
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        type Frame = (Vec<usize>, Vec<usize>, [usize; 12], Option<usize>);
        reserve(
            budget,
            &mut paid,
            add(size_of::<Frame>(), align_of::<Frame>())?,
        )?;
        let (input, input_storage) = Inventory::derive_v18(pair.input(), budget)?;
        reserve(budget, &mut paid, input_storage.retained_storage())?;
        let (output, output_storage) = Inventory::derive_v18(pair.output(), budget)?;
        reserve(budget, &mut paid, output_storage.retained_storage())?;
        let mut operation_nodes =
            initialized(input.operations().len(), usize::MAX, budget, &mut paid)?;
        let mut positions = initialized(plan.nodes.len(), usize::MAX, budget, &mut paid)?;
        let mut operand_end = 0;
        let mut result_end = 0;
        let mut last = None;
        for (index, node) in plan.nodes.iter().enumerate() {
            budget.charge_work(12)?;
            let original = input
                .operations()
                .get(node.input)
                .ok_or(Error::Mismatch("node input"))?;
            let actual = output
                .operations()
                .get(node.output)
                .ok_or(Error::Mismatch("node output"))?;
            let origin = pair
                .origins()
                .get(node.input)
                .ok_or(Error::Mismatch("node origin"))?;
            if last.is_some_and(|previous| previous >= node.input)
                || operation_nodes[node.input] != usize::MAX
                || origin.hoist.is_none()
                || origin.input != original.coordinate
                || origin.output != actual.coordinate
                || original.operation != actual.operation
                || original.results.len() != actual.results.len()
                || node.operands.start != operand_end
                || node.results.start != result_end
                || node.operands.end != add(operand_end, original.operands.len())?
                || node.results.end != add(result_end, original.results.len())?
            {
                return Err(Error::Mismatch("complete exact moved operation"));
            }
            operation_nodes[node.input] = index;
            operand_end = node.operands.end;
            result_end = node.results.end;
            last = Some(node.input);
        }
        if operand_end != plan.operands.len()
            || result_end != plan.results.len()
            || plan.order.len() != plan.nodes.len()
            || pair.origins().len() != operation_nodes.len()
        {
            return Err(Error::Mismatch("complete expression spans"));
        }
        for (index, origin) in pair.origins().iter().enumerate() {
            budget.charge_work(3)?;
            if (operation_nodes[index] != usize::MAX) != origin.hoist.is_some() {
                return Err(Error::Mismatch("missing or invented moved expression"));
            }
        }
        for (position, node) in plan.order.iter().copied().enumerate() {
            budget.charge_work(3)?;
            let entry = positions
                .get_mut(node)
                .ok_or(Error::Mismatch("expression order node"))?;
            if *entry != usize::MAX {
                return Err(Error::Mismatch("duplicate expression order"));
            }
            *entry = position;
        }
        for (index, node) in plan.nodes.iter().enumerate() {
            let original = &input.operations()[node.input];
            let actual = &output.operations()[node.output];
            for (at, use_at) in node.operands.clone().zip(original.operands.clone()) {
                budget.charge_work(6)?;
                let operand = plan.operands[at];
                let definition = input.uses()[use_at].definition;
                if operand.input != definition {
                    return Err(Error::Mismatch("actual expression operand"));
                }
                let expected = match input.definitions()[definition].coordinate {
                    Definition::Result { operation, result } => {
                        let producer = operation_nodes[operation_index(&input, operation)?];
                        (producer != usize::MAX).then_some((producer, result as usize))
                    }
                    _ => None,
                };
                if operand.expression != expected {
                    return Err(Error::Mismatch(
                        "missing or substituted expression dependency",
                    ));
                }
                if let Some((dependency, result)) = expected {
                    if positions[dependency] >= positions[index]
                        || result >= plan.nodes[dependency].results.len()
                    {
                        return Err(Error::Mismatch("acyclic complete expression order"));
                    }
                }
            }
            for (result, at) in node.results.clone().enumerate() {
                budget.charge_work(4)?;
                if plan.results[at]
                    != (ResultBinding {
                        input: original.results.start + result,
                        output: actual.results.start + result,
                        node: index,
                        result,
                    })
                {
                    return Err(Error::Mismatch("exact moved result binding"));
                }
            }
        }

        input_availability(plan, pair, &input, &output, budget)?;
        let mut cursor = 0;
        for function in output.functions() {
            if function.blocks.is_empty() {
                continue;
            }
            with_flow(
                pair.output(),
                function.coordinate,
                Default::default(),
                budget,
                |flow, budget| {
                    for block in function.blocks.clone() {
                        let at = output.blocks()[block].coordinate;
                        for (result, binding) in plan.results.iter().enumerate() {
                            budget.charge_work(3)?;
                            let site =
                                output.operations()[plan.nodes[binding.node].output].coordinate;
                            let available = site.block.function == function.coordinate
                                && site.block != at
                                && flow.dominates(site.block, at, budget)?;
                            if available {
                                if plan.cuts.get(cursor) != Some(&CutBinding { block, result }) {
                                    return Err(Error::Mismatch(
                                        "complete CFG-cut expression bindings",
                                    ));
                                }
                                cursor += 1;
                            }
                        }
                    }
                    Ok::<(), Error>(())
                },
            )?;
        }
        if cursor != plan.cuts.len() {
            return Err(Error::Mismatch("extra CFG-cut expression binding"));
        }
        drop((input, output, operation_nodes, positions));
        Ok(())
    }));
    if budget.storage() == add(floor, paid)? {
        budget.release_storage(paid)?;
    } else {
        return Err(Resource::Accounting.into());
    }
    match checked {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// Every non-expression leaf must already be available in the input at the
// output preheader. Re-entry recomputes the expression after its leaves change.
pub(super) fn input_availability(
    plan: &RelocationExpressionPlanV28<'_>,
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    for function in input.functions() {
        if function.blocks.is_empty() {
            continue;
        }
        with_flow(
            pair.input(),
            function.coordinate,
            Default::default(),
            budget,
            |flow, budget| {
                for node in &plan.nodes {
                    budget.charge_work(2)?;
                    let target = output.operations()[node.output].coordinate;
                    if target.block.function != function.coordinate {
                        continue;
                    }
                    for operand in &plan.operands[node.operands.clone()] {
                        budget.charge_work(4)?;
                        let block = if let Some((dependency, _)) = operand.expression {
                            let producer =
                                output.operations()[plan.nodes[dependency].output].coordinate;
                            if producer.block == target.block
                                && producer.operation >= target.operation
                            {
                                return Err(Error::Mismatch("same-preheader expression order"));
                            }
                            producer.block
                        } else {
                            match input.definitions()[operand.input].coordinate {
                                Definition::FunctionArgument {
                                    function: owner, ..
                                } if owner == function.coordinate => continue,
                                Definition::BlockArgument { block, .. } => block,
                                Definition::Result { operation, .. } => operation.block,
                                _ => return Err(Error::Mismatch("leaf function")),
                            }
                        };
                        if block.function != function.coordinate
                            || !flow.dominates(block, target.block, budget)?
                        {
                            return Err(Error::Mismatch(
                                "unavailable expression leaf or dependency",
                            ));
                        }
                    }
                }
                Ok::<(), Error>(())
            },
        )?;
    }
    Ok(())
}
