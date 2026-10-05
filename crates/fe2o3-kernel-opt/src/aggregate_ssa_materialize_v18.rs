use super::*;

fn ensure<T>(rows: &mut Vec<T>, additional: usize, meter: &mut Meter<'_, '_>) -> Result<()> {
    let needed = add(rows.len(), additional)?;
    if needed <= rows.capacity() {
        return Ok(());
    }
    let extra = needed - rows.capacity();
    meter.work(rows.len())?;
    meter.reserve(
        extra
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    rows.try_reserve_exact(additional)
        .map_err(|_| Resource::Allocation)?;
    meter.capacity::<T>(needed, rows.capacity())?;
    Ok(())
}
fn arguments_mut(t: &mut Terminator, ordinal: usize) -> Result<&mut Vec<ValueId>> {
    match t {
        Terminator::Branch { arguments, .. } if ordinal == 0 => Ok(arguments),
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } if ordinal < 2 => Ok(if ordinal == 0 {
            then_arguments
        } else {
            else_arguments
        }),
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } if ordinal <= cases.len() => {
            if ordinal == cases.len() {
                Ok(default_arguments)
            } else {
                Ok(&mut cases[ordinal].arguments)
            }
        }
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } if ordinal <= cases.len() => {
            if ordinal == cases.len() {
                Ok(default_arguments)
            } else {
                Ok(&mut cases[ordinal].arguments)
            }
        }
        _ => Err(Error::Inconsistent("original successor occurrence")),
    }
}
#[cfg(test)]
pub(super) fn arguments(t: &Terminator, ordinal: usize) -> Result<&[ValueId]> {
    match t {
        Terminator::Branch { arguments, .. } if ordinal == 0 => Ok(arguments),
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } if ordinal < 2 => Ok(if ordinal == 0 {
            then_arguments
        } else {
            else_arguments
        }),
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } if ordinal <= cases.len() => {
            if ordinal == cases.len() {
                Ok(default_arguments)
            } else {
                Ok(&cases[ordinal].arguments)
            }
        }
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } if ordinal <= cases.len() => {
            if ordinal == cases.len() {
                Ok(default_arguments)
            } else {
                Ok(&cases[ordinal].arguments)
            }
        }
        _ => Err(Error::Inconsistent("checked successor occurrence")),
    }
}
pub(super) fn apply(
    i: &Inventory<'_>,
    w: &Witness,
    output: &mut Module,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(i.owner().canonical_bytes().len())?;
    if output != i.owner().module() {
        return Err(Error::Inconsistent("private candidate is exact original"));
    }
    let (mut op_index, mut block_index, mut parameter, mut argument) = (0, 0, 0, 0);
    for (function, f) in output.functions.iter_mut().enumerate() {
        let Some(body) = &mut f.body else {
            continue;
        };
        for (local, block) in body.blocks.iter_mut().enumerate() {
            // All candidate-growth admission occurs before the associated edit;
            // any later refusal discards the entire private candidate.
            while parameter < w.parameters().len() && w.parameters()[parameter].block == block_index
            {
                let p = w.parameters()[parameter];
                ensure(&mut block.parameters, 1, meter)?;
                meter.push(&mut block.parameters, ValueDef::new(p.value, p.ty.ty()))?;
                parameter += 1;
            }
            let visits = block
                .operations
                .len()
                .checked_mul(size_of::<Operation>() + 3)
                .ok_or(Resource::Arithmetic)?;
            meter.work(visits)?;
            let mut malformed = false;
            block.operations.retain_mut(|operation| {
                let action = w.actions()[op_index];
                op_index += 1;
                match action {
                    Action::Retain => true,
                    Action::Remove => false,
                    Action::Copy(value) => {
                        if let Some(condition) = w.conditions()[function] {
                            operation.kind = Kind::Select {
                                condition,
                                true_value: value,
                                false_value: value,
                            };
                        } else {
                            malformed = true;
                        }
                        true
                    }
                }
            });
            if malformed {
                return Err(Error::Inconsistent("copy has concrete condition"));
            }
            if local == 0
                && let Some(condition) = w.conditions()[function]
            {
                let (mut results, _) = meter.table(1)?;
                meter.push(&mut results, ValueDef::new(condition, Type::BOOL))?;
                ensure(&mut block.operations, 1, meter)?;
                meter.work(block.operations.len())?;
                block.operations.insert(
                    0,
                    Operation::new(results, Kind::Constant(Constant::Bool(true))),
                );
            }
            let original = &i.blocks()[block_index];
            for (ordinal, edge) in original.edges.clone().enumerate() {
                let args = arguments_mut(
                    block
                        .terminator
                        .as_mut()
                        .ok_or(Error::Inconsistent("candidate terminator"))?,
                    ordinal,
                )?;
                while argument < w.arguments().len() && w.arguments()[argument].edge == edge {
                    ensure(args, 1, meter)?;
                    meter.push(args, w.arguments()[argument].value)?;
                    argument += 1;
                }
            }
            block_index += 1;
        }
    }
    if op_index != w.actions().len()
        || block_index != i.blocks().len()
        || parameter != w.parameters().len()
        || argument != w.arguments().len()
    {
        return Err(Error::Inconsistent("complete mutation traversal"));
    }
    Ok(())
}
