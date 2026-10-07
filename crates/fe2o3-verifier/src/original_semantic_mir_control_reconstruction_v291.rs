//! Owner-bound, closed acyclic control regions; no execution or progress proof.
use super::*;
use crate::mixed_optimizer_refinement_v26::Budget;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirControlFlowViewV18,
    CanonicalKirUseCoordinateV1 as Use, with_canonical_kir_control_flow_bytes_v18,
};

#[cfg(test)]
#[path = "original_semantic_mir_control_reconstruction_v291_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_semantic_mir_control_reconstruction_resources_v291_tests.rs"]
mod resource_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Term {
    Original(usize),
    Control(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Select {
    pub(super) condition: usize,
    pub(super) on_true: Term,
    pub(super) on_false: Term,
}

pub(super) struct Plan<'a, 'g> {
    input: &'a Inventory<'g>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
    failure: std::cell::Cell<Option<Resource>>,
    pub(super) root: Term,
    pub(super) controls: Vec<Select>,
}

impl Plan<'_, '_> {
    pub(super) fn check(&self, input: &Inventory<'_>, out: &mut Writer<'_, '_>) -> Result<()> {
        if !std::ptr::eq(self.input, input)
            || self.slot != out.budget as *const Budget<'_> as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || self
                .floor
                .checked_add(self.retained)
                .is_none_or(|floor| out.budget.storage() < floor)
        {
            if self.failure.get().is_none() {
                self.failure.set(Some(Resource::Accounting));
            }
        }
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if let Err(error) = out.budget.charge_work(4) {
            self.failure.set(Some(error));
            return Err(error.into());
        }
        Ok(())
    }

    pub(super) fn discard(self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(self.input, out)?;
        let retained = self.retained;
        drop(self);
        out.budget.release_storage(retained)?;
        Ok(())
    }
}

fn headers() -> usize {
    // Derive, DFS, exact-edge lookup, CFG callback and returned owner overlap.
    size_of::<Plan<'_, '_>>()
        + size_of::<Vec<Select>>()
        + size_of::<Vec<u8>>()
        + 2 * size_of::<Vec<Option<Term>>>()
        + size_of::<Vec<(usize, usize)>>()
        + size_of::<Result<Vec<Select>>>()
        + size_of::<Result<Vec<u8>>>()
        + 2 * size_of::<Result<Vec<Option<Term>>>>()
        + size_of::<Result<Vec<(usize, usize)>>>()
        + 4 * size_of::<Result<Term>>()
        + 4 * size_of::<Result<usize>>()
        + 2 * size_of::<Result<()>>()
        + size_of::<Result<Plan<'_, '_>>>()
        + 4 * size_of::<Term>()
        + 3 * size_of::<Select>()
        + 4 * size_of::<Block>()
        + 2 * size_of::<Function>()
        + 2 * size_of::<Definition>()
        + size_of::<RefusalFacts>()
        + 2 * size_of::<Type>()
        + 4 * size_of::<std::ops::Range<usize>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 4 * size_of::<Option<Block>>()
        + 4 * size_of::<Option<Term>>()
        + 4 * size_of::<bool>()
        + 4 * size_of::<Result<Block>>()
        + 2 * size_of::<Result<bool>>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + size_of::<std::slice::Iter<'_, Select>>()
        + 2 * size_of::<Result<usize>>()
        + 2 * size_of::<Option<Resource>>()
        + 2 * size_of::<Resource>()
        + size_of::<Result<()>>()
}

fn local_block(
    input: &Inventory<'_>,
    range: &std::ops::Range<usize>,
    block: Block,
) -> Result<usize> {
    let local = block.block as usize;
    let index = range.start.checked_add(local).ok_or(Resource::Arithmetic)?;
    if !range.contains(&index) || input.blocks().get(index).map(|row| row.coordinate) != Some(block)
    {
        return Err(mismatch());
    }
    Ok(local)
}

fn terminal(
    input: &Inventory<'_>,
    edge: usize,
    original: usize,
    argument: u32,
    owner: &std::ops::Range<usize>,
    out: &mut Writer<'_, '_>,
) -> Result<Term> {
    out.budget.charge_work(12)?;
    let edge = input.edges().get(edge).ok_or_else(mismatch)?;
    let index = edge
        .bindings
        .start
        .checked_add(argument as usize)
        .ok_or(Resource::Arithmetic)?;
    if !edge.bindings.contains(&index) {
        return Err(mismatch());
    }
    let binding = input.edge_arguments().get(index).ok_or_else(mismatch)?;
    let incoming = input
        .definitions()
        .get(binding.incoming_definition)
        .ok_or_else(mismatch)?;
    if binding.coordinate.edge != edge.coordinate
        || binding.coordinate.argument != argument
        || binding.target_definition != original
        || !owner.contains(&binding.incoming_definition)
        || incoming.ty != input.definitions()[original].ty
        || incoming.value != Some(binding.value)
        || edge.arguments.get(argument as usize) != Some(&binding.value)
    {
        return Err(mismatch());
    }
    Ok(Term::Original(binding.incoming_definition))
}

// Equal incoming definitions need no control-region hypothesis. In particular,
// a single incoming edge may be guarded by a branch that bypasses this block.
pub(super) fn identity(
    input: &Inventory<'_>,
    original: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Option<usize>> {
    let frame = 12 * size_of::<&()>()
        + 12 * size_of::<usize>()
        + 2 * size_of::<Option<usize>>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + 2 * size_of::<Result<Term>>()
        + size_of::<Result<Option<usize>>>()
        + size_of::<Definition>()
        + size_of::<Block>()
        + 2 * size_of::<Term>();
    out.budget.reserve_storage(frame)?;
    out.budget.charge_work(10)?;
    let row = input.definitions().get(original).ok_or_else(mismatch)?;
    let Definition::BlockArgument { block, argument } = row.coordinate else {
        return Err(mismatch());
    };
    let owner = input
        .functions()
        .get(block.function.0 as usize)
        .ok_or_else(mismatch)?;
    let local = local_block(input, &owner.blocks, block)?;
    let block_row = &input.blocks()[owner.blocks.start + local];
    if owner.coordinate != block.function
        || !owner.definitions.contains(&original)
        || block_row.parameters.start.checked_add(argument as usize) != Some(original)
        || !block_row.parameters.contains(&original)
        || !matches!(row.ty, Type::Scalar(ScalarType::U32 | ScalarType::Bool))
    {
        return Err(mismatch());
    }
    let mut first = None;
    let mut distinct = false;
    for index in owner.edges.clone() {
        out.budget.charge_work(3)?;
        let edge = &input.edges()[index];
        if edge.target != block {
            continue;
        }
        local_block(input, &owner.blocks, edge.coordinate.source)?;
        let Term::Original(value) =
            terminal(input, index, original, argument, &owner.definitions, out)?
        else {
            return Err(mismatch());
        };
        if let Some(first) = first {
            distinct |= first != value;
        } else {
            first = Some(value);
        }
    }
    let first = first.ok_or_else(mismatch)?;
    out.budget.release_storage(frame)?;
    Ok((!distinct).then_some(first))
}

pub(super) fn derive<'a, 'g>(
    input: &'a Inventory<'g>,
    original: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Plan<'a, 'g>> {
    let floor = out.budget.storage();
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(12)?;
    let row = input.definitions().get(original).ok_or_else(mismatch)?;
    let facts = RefusalFacts::new(original, row.coordinate, row.ty);
    let result = (|| {
        let Definition::BlockArgument {
            block: merge,
            argument,
        } = row.coordinate
        else {
            return Err(mismatch());
        };
        if !matches!(row.ty, Type::Scalar(ScalarType::U32 | ScalarType::Bool)) {
            return Err(mismatch());
        }
        let function = input
            .functions()
            .get(merge.function.0 as usize)
            .ok_or_else(mismatch)?;
        if function.coordinate != merge.function || !function.definitions.contains(&original) {
            return Err(mismatch());
        }
        let merge_local = local_block(input, &function.blocks, merge)?;
        let merge_row = &input.blocks()[function.blocks.start + merge_local];
        if merge_row.parameters.start.checked_add(argument as usize) != Some(original)
            || !merge_row.parameters.contains(&original)
        {
            return Err(mismatch());
        }
        let callback = |cfg: &mut CanonicalKirControlFlowViewV18<'_, '_>,
                        budget: &mut Budget<'_>| {
            budget.charge_work(3)?;
            if !std::ptr::eq(cfg.owner(), input.owner()) || !cfg.is_reachable(merge, budget)? {
                return Err(mismatch());
            }
            let mut best = None;
            for index in function.blocks.clone() {
                budget.charge_work(2)?;
                let candidate = input.blocks()[index].coordinate;
                if candidate != merge && cfg.dominates(candidate, merge, budget)? {
                    if let Some(current) = best {
                        if cfg.dominates(current, candidate, budget)? {
                            best = Some(candidate);
                        }
                    } else {
                        best = Some(candidate);
                    }
                }
            }
            best.ok_or_else(mismatch)
        };
        let capture = std::mem::size_of_val(&callback)
            .checked_mul(2)
            .and_then(|n| n.checked_add(std::mem::align_of_val(&callback)))
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(capture)?;
        // The byte-mode scope pays actual vector capacities, not legacy row cells.
        let anchor = with_canonical_kir_control_flow_bytes_v18(
            input.owner(),
            merge.function,
            Default::default(),
            out.budget,
            callback,
        )?;
        out.budget.release_storage(capture)?;
        let anchor_local = local_block(input, &function.blocks, anchor)?;
        let count = function.blocks.len();
        let scratch_floor = out.budget.storage();
        let mut colors = vector(count, out)?;
        let mut terms = vector(count, out)?;
        let mut edges = vector(function.edges.len(), out)?;
        let mut stack = vector(count, out)?;
        out.budget.charge_work(
            count
                .checked_mul(2)
                .and_then(|n| n.checked_add(function.edges.len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        colors.resize(count, 0u8);
        terms.resize(count, None);
        edges.resize(function.edges.len(), None);
        let scratch = out
            .budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(Resource::Accounting)?;
        // At most one selector per block. IDs are separate from original definitions.
        let mut controls = vector(count, out)?;
        colors[anchor_local] = 1;
        stack.push((anchor_local, 0usize));
        while let Some((local, successor)) = stack.last().copied() {
            out.budget.charge_work(12)?;
            let block = &input.blocks()[function.blocks.start + local];
            let expected = match block.terminator {
                Terminator::Branch { .. } => 1,
                Terminator::ConditionalBranch { .. } => 2,
                _ => return Err(mismatch()),
            };
            if block.edges.len() != expected {
                return Err(mismatch());
            }
            if successor < expected {
                let index = block
                    .edges
                    .start
                    .checked_add(successor)
                    .ok_or(Resource::Arithmetic)?;
                if !function.edges.contains(&index) {
                    return Err(mismatch());
                }
                let edge = &input.edges()[index];
                if edge.coordinate.source != block.coordinate
                    || edge.coordinate.successor as usize != successor
                {
                    return Err(mismatch());
                }
                let target = local_block(input, &function.blocks, edge.target)?;
                if edge.target == merge {
                    edges[index - function.edges.start] = Some(terminal(
                        input,
                        index,
                        original,
                        argument,
                        &function.definitions,
                        out,
                    )?);
                } else {
                    match colors[target] {
                        0 => {
                            colors[target] = 1;
                            stack.push((target, 0));
                            continue;
                        }
                        1 => return Err(mismatch()),
                        2 => {
                            edges[index - function.edges.start] =
                                Some(terms[target].ok_or_else(mismatch)?)
                        }
                        _ => return Err(mismatch()),
                    }
                }
                stack.last_mut().ok_or_else(mismatch)?.1 += 1;
            } else {
                let first = edges[block.edges.start - function.edges.start].ok_or_else(mismatch)?;
                let value =
                    if let Terminator::ConditionalBranch { condition, .. } = block.terminator {
                        out.budget.charge_work(12)?;
                        let use_row = input
                            .uses()
                            .get(block.terminator_uses.start)
                            .ok_or_else(mismatch)?;
                        let condition_row = input
                            .definitions()
                            .get(use_row.definition)
                            .ok_or_else(mismatch)?;
                        if !block.terminator_uses.contains(&block.terminator_uses.start)
                            || use_row.coordinate
                                != (Use::TerminatorOperand {
                                    block: block.coordinate,
                                    operand: 0,
                                })
                            || use_row.value != *condition
                            || condition_row.value != Some(*condition)
                            || !function.definitions.contains(&use_row.definition)
                            || condition_row.ty != &Type::Scalar(ScalarType::Bool)
                        {
                            return Err(mismatch());
                        }
                        let second = edges[block.edges.start + 1 - function.edges.start]
                            .ok_or_else(mismatch)?;
                        if first == second {
                            first
                        } else {
                            let node = controls.len();
                            if node >= count {
                                return Err(mismatch());
                            }
                            controls.push(Select {
                                condition: use_row.definition,
                                on_true: first,
                                on_false: second,
                            });
                            Term::Control(node)
                        }
                    } else {
                        first
                    };
                terms[local] = Some(value);
                colors[local] = 2;
                stack.pop();
            }
        }
        // Whole-function incoming census: no omitted entry, including unreachable
        // provenance, may enter the interior or contribute a phi argument.
        for index in function.edges.clone() {
            out.budget.charge_work(8)?;
            let edge = &input.edges()[index];
            let target = local_block(input, &function.blocks, edge.target)?;
            if target != anchor_local && (target == merge_local || colors[target] == 2) {
                let source = local_block(input, &function.blocks, edge.coordinate.source)?;
                if colors[source] != 2 || edges[index - function.edges.start].is_none() {
                    return Err(mismatch());
                }
            }
        }
        let root = terms[anchor_local].ok_or_else(mismatch)?;
        drop(stack);
        drop(edges);
        drop(terms);
        drop(colors);
        out.budget.release_storage(scratch)?;
        let retained = out
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        Ok(Plan {
            input,
            slot: out.budget as *const Budget<'_> as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            floor,
            retained,
            failure: std::cell::Cell::new(None),
            root,
            controls,
        })
    })();
    result.map_err(|error| trace_refusal(error, RefusalPhase::ControlRegion, facts))
}
