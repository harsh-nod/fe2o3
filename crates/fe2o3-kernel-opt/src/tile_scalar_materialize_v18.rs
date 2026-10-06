use super::*;

pub(super) fn apply(
    inventory: &Inventory<'_>,
    plan: &Plan,
    candidate: &mut Module,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(inventory.owner().canonical_bytes().len())?;
    if candidate != inventory.owner().module() {
        return Err(Error::Inconsistent("tile candidate is exact original"));
    }
    let mut ordinal = 0;
    for function in &mut candidate.functions {
        let Some(body) = &mut function.body else {
            continue;
        };
        for block in &mut body.blocks {
            let start = ordinal;
            let mut count = 0;
            for original in &block.operations {
                meter.work(1)?;
                count = add(count, output_count(plan.actions[ordinal], original))?;
                ordinal += 1;
            }
            let (mut emitted, _) = meter.table(count)?;
            for (local, mut original) in std::mem::take(&mut block.operations)
                .into_iter()
                .enumerate()
            {
                let index = start + local;
                match plan.actions[index] {
                    Action::Retain => meter.push(&mut emitted, original)?,
                    Action::Load(recipe) => {
                        for position in 0..recipe.operation_count() {
                            meter.work(32)?;
                            // Bounded scalar recipe: <=2 definitions, <=1 boxed pointee.
                            meter.reserve(2 * size_of::<ValueDef>() + size_of::<Type>())?;
                            let operation = recipe
                                .emit_operation(position)
                                .ok_or(Error::Inconsistent("tile emission coordinate"))?;
                            if operation.results.capacity() > 2 {
                                return Err(Resource::Accounting.into());
                            }
                            meter.push(&mut emitted, operation)?;
                        }
                    }
                    Action::Fragment => {}
                    Action::Parts(recipe) => {
                        let count = original.results.len();
                        for (position, definition) in original.results.into_iter().enumerate() {
                            let (value, condition) = part(recipe, position, count)?;
                            let (mut results, _) = meter.table(1)?;
                            meter.push(&mut results, definition)?;
                            meter.push(
                                &mut emitted,
                                Operation::new(
                                    results,
                                    Kind::Select {
                                        condition,
                                        true_value: value,
                                        false_value: value,
                                    },
                                ),
                            )?;
                        }
                    }
                    Action::Scope => {
                        let row = &inventory.operations()[index];
                        let Kind::Execution(Execution::ScopeEnd { discarded, .. }) =
                            &mut original.kind
                        else {
                            return Err(Error::Inconsistent("tile scope operation"));
                        };
                        meter.work(discarded.len())?;
                        let mut position = row.operands.start + 1;
                        discarded.retain(|_| {
                            let keep = plan.roles[inventory.uses()[position].definition].is_none();
                            position += 1;
                            keep
                        });
                        meter.push(&mut emitted, original)?;
                    }
                }
            }
            block.operations = emitted;
        }
    }
    if ordinal != plan.actions.len() {
        return Err(Error::Inconsistent("tile operation census"));
    }
    Ok(())
}
