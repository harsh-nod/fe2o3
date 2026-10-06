//! Independent complete-graph comparison; never calls the materializer/emitter.
use super::*;

pub(super) fn compare_roles(
    input: &Inventory<'_>,
    plan: &Plan,
    projections: &[TileScalarRoleProjectionV162],
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    if input.definitions().len() != plan.roles.len() {
        return Err(Error::Inconsistent("tile role definition census"));
    }
    let mut actual = projections.iter();
    let mut previous = None;
    for (definition, recipe) in input.definitions().iter().zip(&plan.roles) {
        meter.work(4)?;
        let Some(recipe) = recipe else { continue };
        let row = actual
            .next()
            .ok_or(Error::Inconsistent("tile role projection missing"))?;
        if !matches!(
            definition.coordinate,
            CanonicalKirDefinitionCoordinateV1::Result { .. }
        ) || previous.is_some_and(|coordinate| coordinate >= row.input)
            || row.input != definition.coordinate
            || row.recipe != *recipe
        {
            return Err(Error::Inconsistent("tile role projection differs"));
        }
        previous = Some(row.input);
    }
    if actual.next().is_some() {
        return Err(Error::Inconsistent("tile role projection extra"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn compare(
    input: &Inventory<'_>,
    output: &Owner,
    plan: &Plan,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    compare_projected(input, output, plan, None, meter)
}

pub(super) fn compare_projected(
    input: &Inventory<'_>,
    output: &Owner,
    plan: &Plan,
    projections: Option<&[TileScalarOperationProjectionV159]>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    if projections.is_some_and(|rows| rows.len() != input.operations().len()) {
        return Err(Error::Inconsistent("tile projected operation census"));
    }
    meter.work(add(
        input.owner().canonical_bytes().len(),
        output.canonical_bytes().len(),
    )?)?;
    let a = input.owner().module();
    let b = output.module();
    if a.id != b.id
        || a.storage_layouts != b.storage_layouts
        || a.kernels != b.kernels
        || a.required_capabilities != b.required_capabilities
        || a.functions.len() != b.functions.len()
    {
        return Err(Error::Inconsistent("tile module metadata differs"));
    }
    let mut ordinal = 0;
    for (old, new) in a.functions.iter().zip(&b.functions) {
        if old.id != new.id
            || old.signature != new.signature
            || old.role != new.role
            || old.required_capabilities != new.required_capabilities
        {
            return Err(Error::Inconsistent("tile function metadata differs"));
        }
        let (old, new) = match (&old.body, &new.body) {
            (Some(old), Some(new)) => (old, new),
            (None, None) => continue,
            _ => return Err(Error::Inconsistent("tile function body presence differs")),
        };
        if old.parameters != new.parameters || old.blocks.len() != new.blocks.len() {
            return Err(Error::Inconsistent(
                "tile function parameters or CFG shape differ",
            ));
        }
        for (old, new) in old.blocks.iter().zip(&new.blocks) {
            if old.id != new.id
                || old.parameters != new.parameters
                || old.terminator != new.terminator
            {
                return Err(Error::Inconsistent(
                    "tile block parameters or exact successor occurrences differ",
                ));
            }
            let mut actual = new.operations.iter();
            for original in &old.operations {
                meter.work(1)?;
                let first = new.operations.len() - actual.len();
                match plan.actions[ordinal] {
                    Action::Retain => {
                        if actual.next() != Some(original) {
                            return Err(Error::Inconsistent("tile untouched operation differs"));
                        }
                    }
                    Action::Load(recipe) => {
                        for position in 0..recipe.operation_count() {
                            meter.work(32)?;
                            let operation = actual
                                .next()
                                .ok_or(Error::Inconsistent("tile scalar sequence truncated"))?;
                            recipe.check_operation(position, operation).map_err(|_| {
                                Error::Inconsistent("tile scalar refinement differs")
                            })?;
                        }
                    }
                    Action::Fragment => {}
                    Action::Parts(recipe) => {
                        for (position, definition) in original.results.iter().enumerate() {
                            meter.work(8)?;
                            let operation = actual
                                .next()
                                .ok_or(Error::Inconsistent("tile parts copies truncated"))?;
                            let elements = original.results.len() / 2;
                            if elements == 0 || original.results.len() != 2 * elements {
                                return Err(Error::Inconsistent("tile original parts arity"));
                            }
                            let (loaded, condition) = recipe
                                .component((position % elements) as u16)
                                .ok_or(Error::Inconsistent("tile original parts geometry"))?;
                            let value = if position < elements {
                                loaded
                            } else {
                                condition
                            };
                            if operation.results.as_slice() != std::slice::from_ref(definition)
                                || !matches!(operation.kind, Kind::Select { condition: actual, true_value, false_value }
                                    if actual == condition && true_value == value && false_value == value)
                            {
                                return Err(Error::Inconsistent(
                                    "tile parts copy result, type or operands differ",
                                ));
                            }
                        }
                    }
                    Action::Scope => {
                        let row = &input.operations()[ordinal];
                        let Kind::Execution(Execution::ScopeEnd {
                            workgroup,
                            discarded,
                        }) = &original.kind
                        else {
                            return Err(Error::Inconsistent("tile original scope"));
                        };
                        let operation = actual
                            .next()
                            .ok_or(Error::Inconsistent("tile scope missing"))?;
                        let Kind::Execution(Execution::ScopeEnd {
                            workgroup: actual_group,
                            discarded: actual_discarded,
                        }) = &operation.kind
                        else {
                            return Err(Error::Inconsistent("tile scope changed kind"));
                        };
                        if operation.results != original.results || actual_group != workgroup {
                            return Err(Error::Inconsistent("tile scope owner changed"));
                        }
                        let mut expected =
                            discarded.iter().enumerate().filter_map(|(index, value)| {
                                let definition =
                                    input.uses()[row.operands.start + 1 + index].definition;
                                plan.roles[definition].is_none().then_some(value)
                            });
                        meter.work(discarded.len())?;
                        if !expected.by_ref().eq(actual_discarded.iter()) {
                            return Err(Error::Inconsistent(
                                "tile scope discarded unrelated roles",
                            ));
                        }
                    }
                }
                if let Some(rows) = projections {
                    meter.work(4)?;
                    let row = rows[ordinal];
                    if row.input != input.operations()[ordinal].coordinate
                        || row.first as usize != first
                        || row.end as usize != new.operations.len() - actual.len()
                    {
                        return Err(Error::Inconsistent(
                            "tile operation span differs from actual replay",
                        ));
                    }
                }
                ordinal += 1;
            }
            if actual.next().is_some() {
                return Err(Error::Inconsistent("tile block has extra operations"));
            }
        }
    }
    if ordinal != input.operations().len() {
        return Err(Error::Inconsistent("tile complete operation census"));
    }
    Ok(())
}
