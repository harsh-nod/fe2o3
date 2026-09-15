//! Overapproximated block reachability for one immutable race-check subject.
//! Unknown values keep every source edge; an unfinished walk grants no pruning.

use std::{collections::HashMap, ops::Range};

use dialect_kernel::{
    IndexEqualBranchArgsOp, IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp,
    ReturnOp, TrapOp, is_index_type,
};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation, value::Value};

use crate::pliron_analysis_witness::{
    MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1, evaluate_raw_index_at_invocation_v1,
};
use crate::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::{MAX_SPARSE_INDEX_USES_V1, SparseIndexAnalysisV1};

#[derive(Clone, Copy)]
enum Comparison {
    LessThan(Value, Value),
    Equal(Value, Value),
}

struct Block {
    successors: Range<usize>,
    comparison: Option<Comparison>,
}

pub(super) struct Reachability<'a> {
    context: &'a Context,
    sparse: &'a SparseIndexAnalysisV1,
    blocks: Vec<Block>,
    successors: Vec<usize>,
    entry: usize,
    visited: Vec<u64>,
    queue: Vec<usize>,
    generation: u64,
    complete: bool,
}

impl<'a> Reachability<'a> {
    pub(super) fn new(
        context: &'a Context,
        function: &FuncOp,
        inventory: &BoundedPlironFunctionInventoryV1,
        sparse: &'a SparseIndexAnalysisV1,
        work: &mut usize,
    ) -> Option<Self> {
        let source_blocks = inventory.blocks();
        if !charge(work, source_blocks.len()) {
            return None;
        }
        let mut indices = HashMap::new();
        indices.try_reserve(source_blocks.len()).ok()?;
        for (index, block) in source_blocks.iter().copied().enumerate() {
            if indices.insert(block, index).is_some() {
                return None;
            }
        }
        let entry = *indices.get(&function.get_entry_block(context))?;
        let mut blocks = Vec::new();
        let mut successors = Vec::new();
        blocks.try_reserve_exact(source_blocks.len()).ok()?;
        for (index, block) in source_blocks.iter().enumerate() {
            if !charge(work, 1) {
                return None;
            }
            let terminator = block.deref(context).get_terminator(context)?;
            if inventory.block_operations(index).last()?.pointer() != terminator {
                return None;
            }
            let raw = terminator.deref(context);
            let count = raw.get_num_successors();
            let end = successors.len().checked_add(count)?;
            if end > MAX_SPARSE_INDEX_USES_V1 || !charge(work, count) {
                return None;
            }
            let operation = Operation::get_op_dyn(terminator, context);
            if count == 0
                && operation.downcast_ref::<ReturnOp>().is_none()
                && operation.downcast_ref::<TrapOp>().is_none()
            {
                return None;
            }
            let start = successors.len();
            successors.try_reserve(count).ok()?;
            for successor in raw.successors() {
                successors.push(*indices.get(&successor)?);
            }
            let comparison = if count == 2 && raw.get_num_operands() >= 2 {
                let lhs = raw.get_operand(0);
                let rhs = raw.get_operand(1);
                if !is_index_type(lhs, context) || !is_index_type(rhs, context) {
                    None
                } else if operation.downcast_ref::<IndexLessThanBranchOp>().is_some()
                    || operation
                        .downcast_ref::<IndexLessThanBranchArgsOp>()
                        .is_some()
                {
                    Some(Comparison::LessThan(lhs, rhs))
                } else if operation.downcast_ref::<IndexEqualBranchOp>().is_some()
                    || operation.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
                {
                    Some(Comparison::Equal(lhs, rhs))
                } else {
                    None
                }
            } else {
                None
            };
            blocks.push(Block {
                successors: start..end,
                comparison,
            });
        }
        if !charge(work, source_blocks.len()) {
            return None;
        }
        let mut visited = Vec::new();
        visited.try_reserve_exact(source_blocks.len()).ok()?;
        visited.resize(source_blocks.len(), 0);
        let mut queue = Vec::new();
        queue.try_reserve_exact(source_blocks.len()).ok()?;
        Some(Self {
            context,
            sparse,
            blocks,
            successors,
            entry,
            visited,
            queue,
            generation: 0,
            complete: false,
        })
    }

    pub(super) fn prepare(&mut self, invocation: &[u64], work: &mut usize) {
        self.complete = false;
        if !charge(work, 1) {
            return;
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return;
        };
        self.generation = generation;
        self.queue.clear();
        self.visited[self.entry] = generation;
        self.queue.push(self.entry);
        let mut next = 0;
        while next < self.queue.len() {
            if !charge(work, 1) {
                return;
            }
            let block = &self.blocks[self.queue[next]];
            next += 1;
            // Sparse facts are the whole-function fixed point, never a
            // first-iteration environment. Raw evaluation rejects SSA cycles
            // and block arguments. No path-specific phi value is installed.
            let selected = match block.comparison {
                Some(Comparison::LessThan(lhs, rhs)) => self
                    .evaluate(lhs, invocation, work)
                    .zip(self.evaluate(rhs, invocation, work))
                    .map(|(lhs, rhs)| usize::from(lhs >= rhs)),
                Some(Comparison::Equal(lhs, rhs)) => self
                    .evaluate(lhs, invocation, work)
                    .zip(self.evaluate(rhs, invocation, work))
                    .map(|(lhs, rhs)| usize::from(lhs != rhs)),
                None => None,
            };
            if *work > MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 {
                return;
            }
            for (edge, successor) in self.successors[block.successors.clone()]
                .iter()
                .copied()
                .enumerate()
            {
                if !charge(work, 1) {
                    return;
                }
                if selected.is_some_and(|selected| selected != edge) {
                    continue;
                }
                if self.visited[successor] != generation {
                    self.visited[successor] = generation;
                    self.queue.push(successor);
                }
            }
        }
        self.complete = true;
    }

    pub(super) fn may_reach(&self, block: usize) -> bool {
        !self.complete
            || self
                .visited
                .get(block)
                .is_none_or(|stamp| *stamp == self.generation)
    }

    fn evaluate(&self, value: Value, invocation: &[u64], work: &mut usize) -> Option<u64> {
        if !charge(work, 1) {
            return None;
        }
        self.sparse
            .fact(value)
            .evaluate(invocation)
            .or_else(|| evaluate_raw_index_at_invocation_v1(self.context, value, invocation, work))
    }
}

fn charge(work: &mut usize, amount: usize) -> bool {
    *work = work.saturating_add(amount);
    *work <= MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1
}

#[cfg(test)]
mod tests;
