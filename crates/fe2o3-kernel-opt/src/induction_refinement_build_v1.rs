use super::*;

pub(super) fn plan(
    inventory: &Inventory<'_>,
    facts: &Facts<'_, '_, '_>,
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<Vec<Row>> {
    meter.reserve(size_of::<Vec<Option<usize>>>())?;
    let (mut selected, selected_size) =
        meter.table::<Option<usize>>(inventory.operations().len())?;
    for _ in inventory.operations() {
        meter.push(&mut selected, None)?;
    }
    let mut count = 0usize;
    for (ordinal, fact) in facts.rows().iter().enumerate() {
        meter.work(20)?;
        let Outcome::Guarded(guarded) = fact.outcome() else {
            continue;
        };
        let recurrence = fact.recurrence();
        if guarded.guarded_update() != Update::NonWrapping
            || !matches!(
                recurrence.scalar(),
                ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
            )
        {
            continue;
        }
        let Definition::Result {
            operation: site,
            result: 0,
        } = recurrence.update()
        else {
            continue;
        };
        let at = operation_index(inventory, site)?;
        let operation = inventory.operations()[at].operation;
        if !matches!(
            operation.kind,
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                ..
            }
        ) || !matches!(operation.results.as_slice(), [sum, flag] if sum.ty == Type::Scalar(recurrence.scalar()) && flag.ty == Type::BOOL)
            || recurrence.overflow()
                != Some(Definition::Result {
                    operation: site,
                    result: 1,
                })
        {
            continue;
        }
        if selected[at].is_none() {
            selected[at] = Some(ordinal);
            count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    meter.work(4)?;
    let total = inventory
        .operations()
        .len()
        .checked_add(count)
        .ok_or(Resource::Arithmetic)?;
    if total > limits.operations {
        return Err(Error::OutputLimit {
            actual: total,
            limit: limits.operations,
        });
    }
    // Every block's final coordinate range is checked before any candidate copy.
    for block in inventory.blocks() {
        meter.work(2)?;
        let mut count = block.operations.len();
        for at in block.operations.clone() {
            meter.work(2)?;
            count = count
                .checked_add(usize::from(selected[at].is_some()))
                .ok_or(Resource::Arithmetic)?;
        }
        u32::try_from(count).map_err(|_| Resource::Arithmetic)?;
    }
    let (mut rows, _) = meter.table::<Row>(inventory.operations().len())?;
    for block in inventory.blocks() {
        meter.work(2)?;
        let mut next = 0u32;
        for at in block.operations.clone() {
            meter.work(4)?;
            let input = inventory.operations()[at].coordinate;
            let output = Site {
                block: block.coordinate,
                operation: next,
            };
            next = next.checked_add(1).ok_or(Resource::Arithmetic)?;
            let row = if let Some(ordinal) = selected[at] {
                let false_output = Site {
                    block: block.coordinate,
                    operation: next,
                };
                next = next.checked_add(1).ok_or(Resource::Arithmetic)?;
                Row::CheckedAddSplit {
                    input,
                    sum_output: output,
                    false_output,
                    induction_row_ordinal: ordinal,
                }
            } else {
                Row::Unchanged { input, output }
            };
            meter.push(&mut rows, row)?;
        }
    }
    drop(selected);
    meter.release(
        selected_size
            .checked_add(size_of::<Vec<Option<usize>>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(rows)
}

pub(super) fn materialize(
    inventory: &Inventory<'_>,
    rows: &[Row],
    candidate: &mut Module,
    meter: &mut Meter<'_, '_>,
) -> Result<usize> {
    let mut extra = 0usize;
    for block in inventory.blocks() {
        meter.work(3)?;
        let mut splits = 0usize;
        for at in block.operations.clone() {
            meter.work(2)?;
            if matches!(rows[at], Row::CheckedAddSplit { .. }) {
                splits = splits.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if splits == 0 {
            continue;
        }
        let count = block
            .operations
            .len()
            .checked_add(splits)
            .ok_or(Resource::Arithmetic)?;
        // The candidate's header/backing remain conservatively reserved. Pay an
        // additional operation Vec header while the old and new vectors coexist.
        meter.reserve(size_of::<Vec<Operation>>())?;
        let (mut operations, backing) = meter.table::<Operation>(count)?;
        extra = extra
            .checked_add(size_of::<Vec<Operation>>())
            .and_then(|n| n.checked_add(backing))
            .ok_or(Resource::Arithmetic)?;
        let target = candidate
            .functions
            .get_mut(block.coordinate.function.0 as usize)
            .and_then(|f| f.body.as_mut())
            .and_then(|b| b.blocks.get_mut(block.coordinate.block as usize))
            .ok_or(Error::Recipe("actual mutable block"))?;
        if target.id != block.block.id || target.operations.len() != block.operations.len() {
            return Err(Error::Recipe("candidate block payload"));
        }
        for (at, mut operation) in block.operations.clone().zip(target.operations.drain(..)) {
            meter.work(5)?;
            if matches!(rows[at], Row::CheckedAddSplit { .. }) {
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = operation.kind
                else {
                    return Err(Error::Recipe("selected actual CheckedAdd"));
                };
                if operation.results.len() != 2 {
                    return Err(Error::Recipe("selected result pair"));
                }
                // Numeric result keeps its actual old capacity. The temporary
                // new result Vec header and capacity are paid before creation.
                meter.reserve(size_of::<Vec<ValueDef>>())?;
                let (mut results, bytes) = meter.table::<ValueDef>(1)?;
                extra = extra
                    .checked_add(size_of::<Vec<ValueDef>>())
                    .and_then(|n| n.checked_add(bytes))
                    .ok_or(Resource::Arithmetic)?;
                let flag = operation
                    .results
                    .pop()
                    .ok_or(Error::Recipe("overflow definition"))?;
                meter.push(&mut results, flag)?;
                operation.kind = OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                };
                meter.push(&mut operations, operation)?;
                meter.push(
                    &mut operations,
                    Operation {
                        results,
                        kind: OperationKind::Constant(Constant::Bool(false)),
                    },
                )?;
            } else {
                meter.push(&mut operations, operation)?;
            }
        }
        if operations.len() != count {
            return Err(Error::Recipe("complete materialized block"));
        }
        target.operations = operations;
    }
    Ok(extra)
}

fn operation_index(inventory: &Inventory<'_>, site: Site) -> Result<usize> {
    let function = inventory
        .functions()
        .get(site.block.function.0 as usize)
        .filter(|f| f.coordinate == site.block.function)
        .ok_or(Error::Recipe("function ordinal"))?;
    let block_at = function
        .blocks
        .start
        .checked_add(site.block.block as usize)
        .filter(|at| *at < function.blocks.end)
        .ok_or(Error::Recipe("block ordinal"))?;
    let block = inventory
        .blocks()
        .get(block_at)
        .filter(|b| b.coordinate == site.block)
        .ok_or(Error::Recipe("actual block ordinal"))?;
    block
        .operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < block.operations.end && inventory.operations()[*at].coordinate == site)
        .ok_or(Error::Recipe("operation ordinal"))
}
