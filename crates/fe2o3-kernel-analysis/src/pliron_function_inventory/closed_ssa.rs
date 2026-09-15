//! Close native def/use and predecessor traversals before invoking verifiers.
use super::*;
use pliron::value::Value;
use std::collections::HashMap;

pub(crate) enum ClosedPlironSsaErrorV1 {
    InvalidGraph,
    NonDominatingOperand(PlironOperationSiteV1),
    ResourceLimit,
}

pub(crate) struct ClosedPlironSsaV1 {
    pub(crate) definitions: HashMap<Value, (usize, Option<usize>)>,
    pub(crate) edges: usize,
}

/// Shape/count limits must be checked by the caller before this preflight.
/// This validates local custody and same-block order, not cross-block dominance.
pub(crate) fn preflight_closed_pliron_ssa_v1(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
    mut charge: impl FnMut(usize) -> bool,
) -> Result<ClosedPlironSsaV1, ClosedPlironSsaErrorV1> {
    use ClosedPlironSsaErrorV1 as E;
    let mut pay = |n| {
        if charge(n) {
            Ok(())
        } else {
            Err(E::ResourceLimit)
        }
    };
    let blocks = inventory.blocks();
    let mut definitions = HashMap::new();
    let mut block_ids = HashMap::new();
    let mut operations = HashMap::new();
    let mut expected_uses = HashMap::<Value, usize>::new();
    let mut incoming = vec![0usize; blocks.len()];
    for (b, block) in blocks.iter().copied().enumerate() {
        if block_ids.insert(block, b).is_some()
            || block.deref(context).get_parent_region() != Some(function.get_region(context))
        {
            return Err(E::InvalidGraph);
        }
        pay(block.deref(context).get_num_arguments() + 1)?;
        for value in block.deref(context).arguments() {
            if definitions.insert(value, (b, None)).is_some() {
                return Err(E::InvalidGraph);
            }
        }
        for site in inventory.block_operations(b) {
            let raw = site.pointer().deref(context);
            if operations.insert(site.pointer(), *site).is_some()
                || raw.get_parent_block() != Some(block)
            {
                return Err(E::InvalidGraph);
            }
            pay(raw.get_num_results() + 1)?;
            for value in raw.results() {
                if definitions
                    .insert(value, (b, Some(site.operation())))
                    .is_some()
                {
                    return Err(E::InvalidGraph);
                }
            }
        }
    }
    for site in inventory.operations() {
        let raw = site.pointer().deref(context);
        pay(raw.get_num_operands() + raw.get_num_successors())?;
        for value in raw.operands() {
            let &(b, ordinal) = definitions.get(&value).ok_or(E::InvalidGraph)?;
            let width = value.defining_op().map_or_else(
                || blocks[b].deref(context).get_num_arguments(),
                |op| op.deref(context).get_num_results(),
            );
            pay(width.saturating_mul(8))?;
            if b == site.block() && ordinal.is_some_and(|i| i >= site.operation()) {
                return Err(E::NonDominatingOperand(*site));
            }
            // Native same-block dominance walks operations; cross-block queries
            // can walk the entire dominator chain. Tree construction is separate.
            pay(if b == site.block() {
                ordinal.map_or(0, |i| site.operation() - i)
            } else {
                blocks.len()
            })?;
            *expected_uses.entry(value).or_default() += 1;
        }
        for successor in raw.successors() {
            let b = *block_ids.get(&successor).ok_or(E::InvalidGraph)?;
            incoming[b] = incoming[b].checked_add(1).ok_or(E::ResourceLimit)?;
        }
    }
    for (b, block) in blocks.iter().enumerate() {
        if block.num_preds(context) != incoming[b] {
            return Err(E::InvalidGraph);
        }
        pay(incoming[b])?;
        for usage in block.uses(context) {
            let site = operations.get(&usage.user_op()).ok_or(E::InvalidGraph)?;
            if site.operation() + 1 != inventory.block_operations(site.block()).len() {
                return Err(E::InvalidGraph);
            }
            let raw = usage.user_op().deref(context);
            pay(raw.get_num_successors())?;
            let slot = usage.try_find_index(context).map_err(|_| E::InvalidGraph)?;
            if raw.get_successor(slot) != *block || raw.get_successor_as_use(slot) != usage {
                return Err(E::InvalidGraph);
            }
        }
    }
    for value in definitions.keys() {
        let expected = expected_uses.get(value).copied().unwrap_or(0);
        if value.num_uses(context) != expected {
            return Err(E::InvalidGraph);
        }
        pay(expected)?;
        for usage in value.uses(context) {
            if !operations.contains_key(&usage.user_op()) {
                return Err(E::InvalidGraph);
            }
            let raw = usage.user_op().deref(context);
            pay(raw.get_num_operands())?;
            let slot = usage.try_find_index(context).map_err(|_| E::InvalidGraph)?;
            if raw.get_operand(slot) != *value || raw.get_operand_as_use(slot) != usage {
                return Err(E::InvalidGraph);
            }
        }
    }
    let edges = incoming
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(E::ResourceLimit)?;
    Ok(ClosedPlironSsaV1 { definitions, edges })
}
