//! Full verification of the closed, flat SSA owner admitted by prescan and closure.
//!
//! Keep PLIRON's recursive structural/interface verification. Only its generic
//! dominance walker is replaced: one region preflight/tree, and no copied use
//! rosters or per-operand region preflights. This does not establish the still
//! separate complete generic-verifier resource bound.

use super::*;
use pliron::{common_traits::Verify, graph::dominance::DomInfo, value::DefiningEntity};

#[derive(Debug)]
pub(super) enum Failure {
    OwnerMismatch,
    Structural(pliron::result::Error),
    NotSsa,
    Dominance {
        block: usize,
        operation: usize,
        operand: usize,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => formatter.write_str("SSA order index belongs to another owner"),
            Self::Structural(error) => fmt::Display::fmt(error, formatter),
            Self::NotSsa => formatter.write_str("identity verification requires an SSA region"),
            Self::Dominance {
                block,
                operation,
                operand,
            } => write!(
                formatter,
                "block {block} op {operation} operand {operand} is not dominated by its definition"
            ),
        }
    }
}

/// Caller must have admitted this exact immutable owner's prescan and def-use
/// closure. In particular, there are no nested regions or foreign users/defs.
pub(super) fn verify(
    context: &Context,
    function: &FuncOp,
    mut order: def_use_closure_v1::CheckedOrder<'_>,
) -> Result<(), Failure> {
    if !order.belongs_to(context, function) {
        return Err(Failure::OwnerMismatch);
    }
    #[cfg(test)]
    assert!(!PANIC_NEXT.replace(false), "injected scoped verifier panic");
    #[cfg(test)]
    trace(|trace| trace.structural_verifications += 1);
    function
        .get_operation()
        .try_deref(context)
        .map_err(Failure::Structural)?
        .verify(context)
        .map_err(Failure::Structural)?;
    let region = function.get_region(context);
    if !region.deref(context).has_ssa_dominance(context) {
        return Err(Failure::NotSsa);
    }

    // Borrow the tree once: even cache hits in DomInfo repeat the complete
    // region preflight. This cache is scoped to the current immutable capture.
    let mut dominance = DomInfo::default();
    #[cfg(test)]
    trace(|trace| trace.tree_requests += 1);
    let tree = dominance.get_dom_tree(context, region);
    let entry = region.deref(context).get_head();
    for (block_index, block) in region.deref(context).iter(context).enumerate() {
        for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
            let raw = operation.deref(context);
            for (operand_index, operand) in raw.operands().enumerate() {
                #[cfg(test)]
                trace(|trace| trace.operand_visits += 1);
                let (definition_block, same_block, entry_argument) = match operand.defining_entity()
                {
                    DefiningEntity::Block(definition) => {
                        (Some(definition), true, Some(definition) == entry)
                    }
                    DefiningEntity::Op(definition) => {
                        let parent = definition.deref(context).get_parent_block();
                        let ordered = parent == Some(block) && {
                            #[cfg(test)]
                            trace(|trace| trace.order_queries += 1);
                            order.strictly_precedes(definition, operation_index)
                        };
                        (parent, ordered, false)
                    }
                };
                let dominates = if definition_block == Some(block) {
                    same_block
                } else if entry_argument {
                    // Prescan/closure admitted this exact flat region. Its entry
                    // parameters are available even in an unreachable block;
                    // this does not extend availability to operation results.
                    true
                } else {
                    // The tree indexes only entry-reachable blocks. Non-entry
                    // definitions still require actual cross-block dominance.
                    tree.contains(&block)
                        && definition_block.is_some_and(|definition| {
                            #[cfg(test)]
                            trace(|trace| trace.cross_block_queries += 1);
                            tree.dominates(&definition, &block)
                        })
                };
                if !dominates {
                    return Err(Failure::Dominance {
                        block: block_index,
                        operation: operation_index,
                        operand: operand_index,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Trace {
    structural_verifications: usize,
    tree_requests: usize,
    operand_visits: usize,
    cross_block_queries: usize,
    order_queries: usize,
}

#[cfg(test)]
thread_local! {
    static TRACE: std::cell::Cell<Trace> = const { std::cell::Cell::new(Trace {
        structural_verifications: 0,
        tree_requests: 0,
        operand_visits: 0,
        cross_block_queries: 0,
        order_queries: 0,
    }) };
    static PANIC_NEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn trace(update: impl FnOnce(&mut Trace)) {
    let mut trace = TRACE.get();
    update(&mut trace);
    TRACE.set(trace);
}

#[cfg(test)]
mod tests;
