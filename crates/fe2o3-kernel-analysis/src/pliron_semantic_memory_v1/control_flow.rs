//! Closed-graph validation before native dominance and exact path coverage.
use super::*;
use crate::pliron_function_inventory::{ClosedPlironSsaErrorV1, preflight_closed_pliron_ssa_v1};
use dialect_kernel::*;
use pliron::{
    builtin::op_interfaces::OneRegionInterface,
    builtin::types::FunctionType,
    common_traits::Verify,
    graph::{
        dominance::compute_dominator_tree, traversals::region::topological_order_by_component,
    },
    r#type::Typed,
};
use std::collections::HashMap;

type E = PlironSemanticMemoryErrorV1;

pub(super) struct Work {
    used: usize,
    limit: usize,
}

impl Work {
    pub(super) fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    #[cfg(test)]
    pub(super) fn used(&self) -> usize {
        self.used
    }

    pub(super) fn charge(&mut self, count: usize) -> Result<(), E> {
        self.used = self.used.checked_add(count).ok_or(E::ResourceLimit)?;
        if self.used > self.limit {
            return Err(E::ResourceLimit);
        }
        Ok(())
    }

    fn product(&mut self, count: usize, width: usize) -> Result<(), E> {
        self.charge(count.checked_mul(width).ok_or(E::ResourceLimit)?)
    }

    fn attributes(&mut self, attributes: &pliron::attribute::AttributeDict) -> Result<(), E> {
        let count = attributes.0.len();
        // Bound key sorting before verifiers inspect closed attribute schemas.
        if count > 64 {
            return Err(E::ResourceLimit);
        }
        self.product(count, count)?;
        for (key, _) in attributes.0.iter() {
            let bytes = key.as_ref().len();
            if bytes > 128 {
                return Err(E::ResourceLimit);
            }
            self.product(bytes, count)?;
        }
        Ok(())
    }
}

pub(super) fn is_terminator(op: &dyn Op) -> bool {
    op.downcast_ref::<ReturnOp>().is_some()
        || op.downcast_ref::<BranchOp>().is_some()
        || op.downcast_ref::<BranchArgsOp>().is_some()
        || op.downcast_ref::<IndexLessThanBranchOp>().is_some()
        || op.downcast_ref::<IndexLessThanBranchArgsOp>().is_some()
        || op.downcast_ref::<IndexEqualBranchOp>().is_some()
        || op.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
}

pub(super) fn validate(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
    work: &mut Work,
) -> Result<(), E> {
    let blocks = inventory.blocks();
    if blocks.is_empty() {
        return Err(E::UnsupportedControlFlow);
    }
    work.product(blocks.len(), 8)?;
    work.product(inventory.operations().len(), 8)?;
    work.attributes(&function.get_operation().deref(context).attributes)?;
    let region = function.get_region(context);
    for (b, block) in blocks.iter().copied().enumerate() {
        if block.deref(context).get_parent_region() != Some(region) {
            return Err(E::InvalidGraph);
        }
        let raw = block.deref(context);
        work.attributes(&raw.attributes)?;
        work.product(
            raw.get_num_arguments(),
            raw.get_num_arguments().saturating_add(4),
        )?;
        for value in raw.arguments() {
            // Index edge transport is interpreted by the existing trace engine.
            // Typed memory results are not silently transported through a phi.
            if b != 0 && !value.get_type(context).deref(context).is::<IndexType>() {
                return Err(E::UnsupportedControlFlow);
            }
        }
        let body = inventory.block_operations(b);
        if body.is_empty() {
            return Err(E::UnsupportedControlFlow);
        }
        for site in body {
            let pointer = site.pointer();
            let raw = pointer.deref(context);
            work.attributes(&raw.attributes)?;
            if raw.get_parent_block() != Some(block) || raw.num_regions() != 0 {
                return Err(E::InvalidGraph);
            }
            work.product(
                raw.get_num_results(),
                raw.get_num_results().saturating_add(4),
            )?;
            work.product(
                raw.get_num_operands(),
                raw.get_num_operands().saturating_add(4),
            )?;
            work.product(
                raw.get_num_successors(),
                raw.get_num_successors().saturating_add(4),
            )?;
            let op = Operation::get_op_dyn(pointer, context);
            if !collect::supported(op.as_ref())
                || is_terminator(op.as_ref()) != (site.operation() + 1 == body.len())
                || (!is_terminator(op.as_ref()) && raw.get_num_successors() != 0)
            {
                return Err(E::UnsupportedOperation {
                    site: PlironSemanticMemorySiteV1::new(b, site.operation()),
                });
            }
        }
    }
    let entry = blocks[0].deref(context);
    let arguments = entry
        .arguments()
        .map(|value| value.get_type(context))
        .collect();
    // Intern the bounded expected signature instead of cloning an unchecked
    // incoming signature. ReturnOp has no output operands in this subset.
    // A cache-miss rejection may advance the epoch; never reset that attempt.
    if function.get_type(context) != FunctionType::get(context, arguments, vec![]).into() {
        return Err(E::InvalidGraph);
    }
    let closed =
        preflight_closed_pliron_ssa_v1(context, function, inventory, |n| work.charge(n).is_ok())
            .map_err(|error| match error {
                ClosedPlironSsaErrorV1::InvalidGraph => E::InvalidGraph,
                ClosedPlironSsaErrorV1::ResourceLimit => E::ResourceLimit,
                ClosedPlironSsaErrorV1::NonDominatingOperand(site) => E::NonDominatingOperand {
                    site: PlironSemanticMemorySiteV1::new(site.block(), site.operation()),
                },
            })?;
    function.verify(context).map_err(|_| E::InvalidGraph)?;
    function
        .verify_interfaces(context)
        .map_err(|_| E::InvalidGraph)?;
    for site in inventory.operations() {
        let op = Operation::get_op_dyn(site.pointer(), context);
        op.verify(context).map_err(|_| E::InvalidGraph)?;
        op.verify_interfaces(context).map_err(|_| E::InvalidGraph)?;
    }
    let edges = closed.edges;
    work.product(blocks.len().checked_add(edges).ok_or(E::ResourceLimit)?, 8)?;
    let components = topological_order_by_component(context, &region);
    if components.len() != 1
        || components[0].len() != blocks.len()
        || components[0].first() != blocks.first()
    {
        return Err(E::UnsupportedControlFlow);
    }
    let rpo = components[0]
        .iter()
        .enumerate()
        .map(|(i, b)| (*b, i))
        .collect::<HashMap<_, _>>();
    for site in inventory.operations() {
        for successor in site.pointer().deref(context).successors() {
            if rpo[&blocks[site.block()]] >= rpo[&successor] {
                return Err(E::UnsupportedControlFlow);
            }
        }
    }
    // In a reachable DAG Cooper's algorithm stabilizes in two sweeps. Each
    // predecessor intersection still costs up to two full parent-chain walks.
    // Reserve both this tree and the one built by structural verification.
    work.product(blocks.len().checked_mul(8).ok_or(E::ResourceLimit)?, edges)?;
    let dominators = compute_dominator_tree(context, &region);
    for site in inventory.operations() {
        for value in site.pointer().deref(context).operands() {
            let (b, _) = closed.definitions[&value];
            if b != site.block() {
                work.charge(blocks.len())?;
                if !dominators.contains(&blocks[b])
                    || !dominators.contains(&blocks[site.block()])
                    || !dominators.dominates(&blocks[b], &blocks[site.block()])
                {
                    return Err(E::NonDominatingOperand {
                        site: PlironSemanticMemorySiteV1::new(site.block(), site.operation()),
                    });
                }
            }
        }
    }
    Ok(())
}
