//! Proves one immutable equal-input meet has no normalization effects.
//! No source facts, cache, or graph admission are constructed here.
use super::*;

#[cfg(test)]
#[path = "unchanged_join_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "join_probe70_tests.rs"]
mod probe_tests;

pub(super) fn can_reuse(left: &Flow, right: &Flow, budget: &mut Budget) -> Result<bool> {
    budget.charge(1)?;
    if left.values.len() != right.values.len()
        || left.dead.len() != right.dead.len()
        || left.escaped.len() != right.escaped.len()
    {
        return Ok(false);
    }
    if !same_set(&left.dead, &right.dead, budget)?
        || !same_set(&left.escaped, &right.escaped, budget)?
    {
        return Ok(false);
    }
    // Compare and normalize in one borrowed walk. The first difference stops
    // the probe before inspecting unrelated payload; no temporary is built.
    let mut dead = left.dead.iter().peekable();
    for ((&local, value), (&incoming_local, incoming)) in
        left.values.iter().zip(right.values.iter())
    {
        budget.charge(2)?;
        if local != incoming_local {
            return Ok(false);
        }
        while dead.peek().is_some_and(|&&target| target < local) {
            budget.charge(1)?;
            dead.next();
        }
        if dead.peek().is_some_and(|&&target| target == local)
            || !same_normalized_value(value, incoming, left, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn normalized(left: &Flow, budget: &mut Budget) -> Result<bool> {
    // Equal key sets introduce no missing-key kills. Merge the existing sorted
    // dead keys with value keys without allocating a temporary target set.
    let mut dead = left.dead.iter().peekable();
    for (&local, value) in left.values.iter() {
        budget.charge(1)?;
        while dead.peek().is_some_and(|&&target| target < local) {
            budget.charge(1)?;
            dead.next();
        }
        if dead.peek().is_some_and(|&&target| target == local)
            || !targets_unchanged(value, left, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_set(left: &BTreeSet<u32>, right: &BTreeSet<u32>, budget: &mut Budget) -> Result<bool> {
    // The caller has checked equal lengths. One debit for each consumed key.
    for (left, right) in left.iter().zip(right) {
        budget.charge(2)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_normalized_value(
    left: &Value,
    right: &Value,
    flow: &Flow,
    budget: &mut Budget,
) -> Result<bool> {
    budget.charge(2)?;
    match (left, right) {
        (Value::Opaque, Value::Opaque) => Ok(true),
        (Value::Reference(left), Value::Reference(right)) => {
            Ok(left == right && reference_unchanged(left, flow, budget)?)
        }
        (Value::Fields(left), Value::Fields(right)) => same_fields(left, right, flow, budget),
        (
            Value::Variants {
                ty,
                possible,
                fields,
            },
            Value::Variants {
                ty: right_ty,
                possible: right_possible,
                fields: right_fields,
            },
        ) if ty == right_ty && possible == right_possible => {
            same_fields(fields, right_fields, flow, budget)
        }
        _ => Ok(false),
    }
}

fn same_fields(
    left: &[Option<Value>],
    right: &[Option<Value>],
    flow: &Flow,
    budget: &mut Budget,
) -> Result<bool> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (Some(left), Some(right)) => {
                if !same_normalized_value(left, right, flow, budget)? {
                    return Ok(false);
                }
            }
            (None, None) => budget.charge(2)?,
            _ => {
                budget.charge(2)?;
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn reference_unchanged(reference: &Reference, flow: &Flow, budget: &mut Budget) -> Result<bool> {
    // Same logical scalar-key model as Values lookups, not a bound on std
    // BTreeSet comparisons, allocator work, or RSS.
    let lookup = |entries: usize| (usize::BITS - entries.max(1).leading_zeros()) as usize;
    budget.charge(lookup(flow.dead.len()) + lookup(flow.escaped.len()))?;
    Ok(!flow.dead.contains(&reference.target) && !flow.escaped.contains(&reference.target))
}

fn targets_unchanged(value: &Value, flow: &Flow, budget: &mut Budget) -> Result<bool> {
    budget.charge(1)?;
    match value {
        Value::Opaque => Ok(true),
        Value::Reference(reference) => reference_unchanged(reference, flow, budget),
        Value::Fields(fields) | Value::Variants { fields, .. } => {
            // Inspect all fields, including inactive enum slots: the original
            // final invalidation traverses those slots too.
            for field in fields {
                match field {
                    Some(value) if !targets_unchanged(value, flow, budget)? => return Ok(false),
                    None => budget.charge(1)?,
                    _ => {}
                }
            }
            Ok(true)
        }
    }
}
