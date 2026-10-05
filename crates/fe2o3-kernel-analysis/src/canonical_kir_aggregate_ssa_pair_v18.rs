use super::*;

fn arguments(t: &Terminator, ordinal: usize) -> Result<&[ValueId]> {
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
fn same_head(a: &Terminator, b: &Terminator) -> bool {
    match (a, b) {
        (Terminator::Branch { target: a, .. }, Terminator::Branch { target: b, .. }) => a == b,
        (
            Terminator::ConditionalBranch {
                condition: a,
                then_target: at,
                else_target: ae,
                ..
            },
            Terminator::ConditionalBranch {
                condition: b,
                then_target: bt,
                else_target: be,
                ..
            },
        ) => a == b && at == bt && ae == be,
        (
            Terminator::Switch {
                selector: a,
                cases: ac,
                default_target: ad,
                ..
            },
            Terminator::Switch {
                selector: b,
                cases: bc,
                default_target: bd,
                ..
            },
        ) => {
            a == b
                && ad == bd
                && ac.len() == bc.len()
                && ac
                    .iter()
                    .zip(bc)
                    .all(|(a, b)| a.value == b.value && a.target == b.target)
        }
        (
            Terminator::IntegerSwitch {
                selector: a,
                cases: ac,
                default_target: ad,
                ..
            },
            Terminator::IntegerSwitch {
                selector: b,
                cases: bc,
                default_target: bd,
                ..
            },
        ) => {
            a == b
                && ad == bd
                && ac.len() == bc.len()
                && ac
                    .iter()
                    .zip(bc)
                    .all(|(a, b)| a.value == b.value && a.target == b.target)
        }
        (Terminator::Return { values: a }, Terminator::Return { values: b }) => a == b,
        (Terminator::Unreachable, Terminator::Unreachable) => true,
        _ => false,
    }
}

/// Allocation-free actual-pair comparison. This never invokes the materializer,
/// trusts output identities, or accepts omitted rows as unchanged operations.
pub(super) fn check(
    i: &Inventory<'_>,
    w: &Witness,
    b: &Module,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    let a = i.owner().module();
    if a.id != b.id
        || a.storage_layouts != b.storage_layouts
        || a.kernels != b.kernels
        || a.required_capabilities != b.required_capabilities
        || a.functions.len() != b.functions.len()
    {
        return Err(Error::Inconsistent("exact module and storage table"));
    }
    let (mut op, mut block, mut parameter, mut argument) = (0, 0, 0, 0);
    for (function, (a, b)) in a.functions.iter().zip(&b.functions).enumerate() {
        meter.work(3)?;
        if a.id != b.id
            || a.signature != b.signature
            || a.role != b.role
            || a.required_capabilities != b.required_capabilities
        {
            return Err(Error::Inconsistent("exact function metadata"));
        }
        let (a, b) = match (&a.body, &b.body) {
            (None, None) => continue,
            (Some(a), Some(b)) => (a, b),
            _ => return Err(Error::Inconsistent("exact body roster")),
        };
        if a.parameters != b.parameters || a.blocks.len() != b.blocks.len() {
            return Err(Error::Inconsistent(
                "original function arguments and blocks",
            ));
        }
        for (local, (a, b)) in a.blocks.iter().zip(&b.blocks).enumerate() {
            meter.work(5)?;
            if a.id != b.id
                || b.parameters.get(..a.parameters.len()) != Some(a.parameters.as_slice())
            {
                return Err(Error::Inconsistent("original block arguments"));
            }
            let mut extra = a.parameters.len();
            while parameter < w.parameters.len() && w.parameters[parameter].block == block {
                let expected = w.parameters[parameter];
                meter.work(1)?;
                if b.parameters.get(extra) != Some(&ValueDef::new(expected.value, expected.ty.ty()))
                {
                    return Err(Error::Inconsistent("exact secondary SSA parameter"));
                }
                extra += 1;
                parameter += 1;
            }
            if extra != b.parameters.len() {
                return Err(Error::Inconsistent(
                    "complete secondary SSA parameter roster",
                ));
            }
            let mut out = 0;
            if local == 0
                && let Some(condition) = w.conditions[function]
            {
                let actual = b
                    .operations
                    .get(out)
                    .ok_or(Error::Inconsistent("synthetic condition exists"))?;
                if actual.results.as_slice() != [ValueDef::new(condition, Type::BOOL)]
                    || actual.kind != Kind::Constant(Constant::Bool(true))
                {
                    return Err(Error::Inconsistent("exact concrete true condition"));
                }
                out += 1;
            }
            for original in &a.operations {
                meter.work(2)?;
                let action = w.actions[op];
                op += 1;
                if action == Action::Remove {
                    continue;
                }
                let actual = b
                    .operations
                    .get(out)
                    .ok_or(Error::Inconsistent("complete final operation roster"))?;
                match action {
                    Action::Retain if original != actual => {
                        return Err(Error::Inconsistent("exact retained operation"));
                    }
                    Action::Copy(value) => {
                        let condition = w.conditions[function]
                            .ok_or(Error::Inconsistent("checked concrete condition"))?;
                        if actual.results != original.results
                            || actual.kind
                                != (Kind::Select {
                                    condition,
                                    true_value: value,
                                    false_value: value,
                                })
                        {
                            return Err(Error::Inconsistent("exact original load SSA value"));
                        }
                    }
                    _ => {}
                }
                out += 1;
            }
            if out != b.operations.len() {
                return Err(Error::Inconsistent("no unclaimed final operation"));
            }
            let (at, bt) = (
                a.terminator
                    .as_ref()
                    .ok_or(Error::Inconsistent("original terminator"))?,
                b.terminator
                    .as_ref()
                    .ok_or(Error::Inconsistent("final terminator"))?,
            );
            if !same_head(at, bt) {
                return Err(Error::Inconsistent("exact original control flow"));
            }
            for (ordinal, edge) in i.blocks()[block].edges.clone().enumerate() {
                let a = arguments(at, ordinal)?;
                let b = arguments(bt, ordinal)?;
                if b.get(..a.len()) != Some(a) {
                    return Err(Error::Inconsistent("original edge arguments"));
                }
                let mut extra = a.len();
                while argument < w.arguments.len() && w.arguments[argument].edge == edge {
                    meter.work(1)?;
                    if b.get(extra) != Some(&w.arguments[argument].value) {
                        return Err(Error::Inconsistent("exact secondary SSA edge value"));
                    }
                    extra += 1;
                    argument += 1;
                }
                if extra != b.len() {
                    return Err(Error::Inconsistent("complete secondary SSA edge roster"));
                }
            }
            block += 1;
        }
    }
    if op != w.actions.len()
        || block != i.blocks().len()
        || parameter != w.parameters.len()
        || argument != w.arguments.len()
    {
        return Err(Error::Inconsistent("complete independent traversal"));
    }
    Ok(())
}
