// Source extraction supplies the equations; solving these integer-indexed rows
// alone grants no producer, storage, lifetime or source correspondence authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionIdentityEquationKindV1 {
    Input,
    Producer,
    Merge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutionIdentityEquationV1 {
    kind: ExecutionIdentityEquationKindV1,
    inputs: std::ops::Range<usize>,
}

#[derive(Default)]
struct ExecutionIdentityEquationsV1 {
    rows: Vec<ExecutionIdentityEquationV1>,
    inputs: Vec<usize>,
}

fn execution_identity_error_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "nominal identity equations differ from their original source",
    )
}

impl ExecutionIdentityEquationsV1 {
    fn push(
        &mut self,
        kind: ExecutionIdentityEquationKindV1,
        inputs: &[usize],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let index = self.rows.len();
        let first = self.inputs.len();
        let end = argument_sum_v1(&[first, inputs.len()])?;
        for &input in inputs {
            emission_push_v1(&mut self.inputs, input, budget)?;
        }
        emission_push_v1(
            &mut self.rows,
            ExecutionIdentityEquationV1 {
                kind,
                inputs: first..end,
            },
            budget,
        )?;
        Ok(index)
    }

    fn solve(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<usize>, ProductionSemanticKirErrorV1> {
        scoped_slot_attempt_v29(budget, |budget| {
            let floor = budget.storage();
            // Output classes, reverse counts, ranges, reverse edges, pending
            // counts and the one-enqueue-per-node work queue.
            budget.reserve_storage(argument_sum_v1(&[
                argument_product_v1(5, std::mem::size_of::<Vec<usize>>())?,
                std::mem::size_of::<Vec<std::ops::Range<usize>>>(),
            ])?)?;
            let count = self.rows.len();
            let mut classes = emission_vec_v1(count, budget)?;
            let mut positions = emission_vec_v1(count, budget)?;
            let mut ranges = emission_vec_v1(count, budget)?;
            let mut reverse = emission_vec_v1(self.inputs.len(), budget)?;
            let mut pending = emission_vec_v1(count, budget)?;
            let mut queue = emission_vec_v1(count, budget)?;
            budget.charge_work(argument_sum_v1(&[
                argument_product_v1(count, 3)?,
                self.inputs.len(),
            ])?)?;
            classes.resize(count, usize::MAX);
            positions.resize(count, 0usize);
            reverse.resize(self.inputs.len(), 0usize);
            let mut end = 0;
            for row in &self.rows {
                budget.charge_work(3)?;
                if row.inputs.start != end || row.inputs.end < end {
                    return Err(execution_identity_error_v1());
                }
                let inputs = self
                    .inputs
                    .get(row.inputs.clone())
                    .ok_or_else(execution_identity_error_v1)?;
                if matches!(row.kind, ExecutionIdentityEquationKindV1::Input) && !inputs.is_empty()
                    || matches!(row.kind, ExecutionIdentityEquationKindV1::Merge)
                        && inputs.is_empty()
                {
                    return Err(execution_identity_error_v1());
                }
                pending.push(inputs.len());
                for &input in inputs {
                    budget.charge_work(2)?;
                    let position = positions
                        .get_mut(input)
                        .ok_or_else(execution_identity_error_v1)?;
                    *position = argument_sum_v1(&[*position, 1])?;
                }
                end = row.inputs.end;
            }
            if end != self.inputs.len() {
                return Err(execution_identity_error_v1());
            }
            let mut next = 0;
            for position in &mut positions {
                budget.charge_work(2)?;
                let end = argument_sum_v1(&[next, *position])?;
                ranges.push(next..end);
                *position = next;
                next = end;
            }
            if next != reverse.len() {
                return Err(execution_identity_error_v1());
            }
            for (target, row) in self.rows.iter().enumerate() {
                budget.charge_work(2)?;
                for &input in &self.inputs[row.inputs.clone()] {
                    budget.charge_work(3)?;
                    let position = positions[input];
                    *reverse
                        .get_mut(position)
                        .ok_or_else(execution_identity_error_v1)? = target;
                    positions[input] = argument_sum_v1(&[position, 1])?;
                }
                if row.kind != ExecutionIdentityEquationKindV1::Merge && pending[target] == 0 {
                    classes[target] = target;
                    queue.push(target);
                }
            }
            // Unknown -> one source origin is the only non-error transition.
            // A merge can propagate its first known origin around a cycle, but
            // every remaining input must eventually arrive with that same root.
            let mut cursor = 0;
            while cursor < queue.len() {
                budget.charge_work(2)?;
                let input = queue[cursor];
                cursor = argument_sum_v1(&[cursor, 1])?;
                let origin = classes[input];
                for &target in &reverse[ranges[input].clone()] {
                    budget.charge_work(4)?;
                    pending[target] = pending[target]
                        .checked_sub(1)
                        .ok_or_else(execution_identity_error_v1)?;
                    match self.rows[target].kind {
                        ExecutionIdentityEquationKindV1::Input => {
                            return Err(execution_identity_error_v1());
                        }
                        ExecutionIdentityEquationKindV1::Producer => {
                            if pending[target] == 0 {
                                if classes[target] != usize::MAX {
                                    return Err(execution_identity_error_v1());
                                }
                                classes[target] = target;
                                queue.push(target);
                            }
                        }
                        ExecutionIdentityEquationKindV1::Merge => {
                            if classes[target] == usize::MAX {
                                classes[target] = origin;
                                queue.push(target);
                            } else if classes[target] != origin {
                                return Err(execution_identity_error_v1());
                            }
                        }
                    }
                }
            }
            budget.charge_work(argument_product_v1(count, 2)?)?;
            if classes.iter().any(|class| *class == usize::MAX)
                || pending.iter().any(|pending| *pending != 0)
                || queue.len() != count
            {
                return Err(execution_identity_error_v1());
            }
            let retained = argument_product_v1(classes.capacity(), std::mem::size_of::<usize>())?;
            drop((positions, ranges, reverse, pending, queue));
            let scratch = budget
                .storage()
                .checked_sub(floor)
                .and_then(|bytes| bytes.checked_sub(retained))
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(scratch)?;
            Ok(classes)
        })
    }
}
