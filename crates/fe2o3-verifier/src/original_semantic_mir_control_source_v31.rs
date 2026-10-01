//! Complete source-owned control roster and concrete invocation-prefix replay.

use super::super::Inventory;
use super::{
    Error, ExpressionV30, NodeV30, Resource, Result, ScalarV30, Writer,
    canonical::{
        self,
        control::{TargetBlock, TargetBranch},
    },
    control::SourceControl,
    control_pair::{BlockBindings, BlockPair},
    invocations::InvocationPlan,
    relation, vector,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
};
use fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18 as Correspondence;
use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1;
use std::mem::size_of;

pub(super) struct RootControl {
    pub original: SourceControl,
    pub bindings: Vec<Option<BlockBindings>>,
    pub targets: Vec<TargetBlock>,
    /// Unique original block for each actual block; invocation blocks are None.
    pub original_blocks: Vec<Option<usize>>,
    pub pairs: Vec<Option<BlockPair>>,
    /// Actual function-local blocks, in invocation order, before the source entry.
    pub prefix: Vec<usize>,
    pub physical: usize,
    pub block_start: usize,
    pub definition_start: usize,
}

pub(super) struct CompleteControl {
    pub roots: Vec<RootControl>,
    pub census: [usize; 6],
}

fn mismatch() -> Error {
    Error::Statement("original MIR complete CFG source or invocation binding differs")
}

impl CompleteControl {
    pub(super) fn derive(relation: &Correspondence<'_>, out: &mut Writer<'_, '_>) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = relation.source(out.budget)?;
        let invocations = InvocationPlan::derive(source, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        let count = source.root_count(out.budget)?;
        out.budget
            .charge_work(count.checked_add(3).ok_or(Resource::Arithmetic)?)?;
        if count == 0 || count != semantic.roots().len() || count != inventory.functions().len() {
            return Err(mismatch());
        }
        let mut seen = vector(count, out)?;
        seen.resize(count, false);
        let mut roots = vector(count, out)?;
        let mut census = [
            count,
            0usize,
            0,
            0,
            inventory.operations().len(),
            inventory.definitions().len(),
        ];
        for root in 0..count {
            let (original, physical) = source.root(root, out.budget)?;
            if semantic.roots().get(root) != Some(&original) || seen.get(physical) != Some(&false) {
                return Err(mismatch());
            }
            seen[physical] = true;
            // Invocation is not modeled by relabeling a helper as a root. The
            // whole request refuses until a call-step relation is available.
            let invocation_root = invocations.root(root, out)?;
            let invocation = invocations.instance(root, 0, out)?;
            if invocation_root.instances.len() != 1
                || !invocation.active
                || invocation.function != original
                || invocation.incoming.is_some()
                || invocation_root.function != original
                || invocation_root.physical != physical
                || !invocations.calls(root, 0, out)?.is_empty()
            {
                return Err(Error::Statement(
                    "original MIR invocation control is not modeled",
                ));
            }
            let function = semantic
                .functions()
                .get(original.index() as usize)
                .ok_or_else(mismatch)?;
            let ssa = source.source_ssa(out.budget)?;
            let plan = ssa.plan_for_function(original).ok_or_else(mismatch)?.plan();
            let control = SourceControl::derive(semantic.types(), function, plan, out)?;
            let actual = inventory.functions().get(physical).ok_or_else(mismatch)?;
            if actual.function.signature.parameters.len() != control.arguments {
                return Err(mismatch());
            }
            for &(local, argument) in &control.initial {
                out.budget.charge_work(3)?;
                let expected = ScalarV30::from_source(
                    semantic.types(),
                    function.locals()[local as usize].ty(),
                )?;
                let definition = actual
                    .definitions
                    .start
                    .checked_add(argument as usize)
                    .ok_or(Resource::Arithmetic)?;
                let row = inventory
                    .definitions()
                    .get(definition)
                    .ok_or_else(mismatch)?;
                if row.coordinate
                    != (Definition::FunctionArgument {
                        function: actual.coordinate,
                        argument,
                    })
                    || canonical::scalar(row.ty)? != expected
                {
                    return Err(mismatch());
                }
            }
            let mut bindings = vector(control.blocks.len(), out)?;
            let mut mapped = vector(actual.blocks.len(), out)?;
            let mut original_blocks = vector(actual.blocks.len(), out)?;
            out.budget.charge_work(actual.blocks.len())?;
            mapped.resize(actual.blocks.len(), false);
            original_blocks.resize(actual.blocks.len(), None);
            for (ordinal, block) in control.blocks.iter().enumerate() {
                out.budget.charge_work(2)?;
                let id = SemanticBlockIdV1::from_index(
                    u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                );
                let location = relation.source_block_entry(root, 0, id, out.budget)?;
                let Some(block) = block else {
                    if location.is_some() {
                        return Err(mismatch());
                    }
                    bindings.push(None);
                    continue;
                };
                let location = location.ok_or_else(mismatch)?;
                let slot = location.block as usize;
                if location.function != actual.coordinate || mapped.get(slot) != Some(&false) {
                    return Err(mismatch());
                }
                mapped[slot] = true;
                original_blocks[slot] = Some(ordinal);
                let mut live = vector(block.live.len(), out)?;
                for input in &block.live {
                    live.push((
                        input.local,
                        relation.ssa_scalar_definition_v30(root, 0, input.value, out.budget)?,
                    ));
                }
                let mut assignments = vector(block.program.assignments.len(), out)?;
                for assignment in &block.program.assignments {
                    assignments.push(relation.assignment_scalar_definition_v30(
                        root,
                        0,
                        id,
                        assignment.statement,
                        out.budget,
                    )?);
                }
                census[2] = census[2]
                    .checked_add(block.program.statements)
                    .ok_or(Resource::Arithmetic)?;
                census[3] = census[3]
                    .checked_add(assignments.len())
                    .ok_or(Resource::Arithmetic)?;
                bindings.push(Some(BlockBindings {
                    physical: location,
                    live,
                    assignments,
                }));
            }
            let mut targets = vector(actual.blocks.len(), out)?;
            for block in actual.blocks.clone() {
                targets.push(TargetBlock::derive(inventory, block, out)?);
            }
            let prefix = invocation(
                &control,
                &bindings,
                &targets,
                &mut mapped,
                inventory,
                physical,
                out,
            )?;
            let mut pairs = vector(control.blocks.len(), out)?;
            for (block, binding) in bindings.iter().enumerate() {
                let Some(binding) = binding else {
                    pairs.push(None);
                    continue;
                };
                let target = targets
                    .get_mut(binding.physical.block as usize)
                    .ok_or_else(mismatch)?;
                pairs.push(Some(BlockPair::check(
                    &control, block, &bindings, inventory, target, out,
                )?));
            }
            census[1] = census[1].checked_add(1).ok_or(Resource::Arithmetic)?;
            roots.push(RootControl {
                original: control,
                bindings,
                targets,
                original_blocks,
                pairs,
                prefix,
                physical,
                block_start: actual.blocks.start,
                definition_start: actual.definitions.start,
            });
        }
        out.budget.charge_work(seen.len())?;
        if seen.iter().any(|seen| !seen) {
            return Err(mismatch());
        }
        Ok(Self { roots, census })
    }
}

/// Composes concrete preheader expressions over the real function arguments.
/// All unmapped blocks must form this finite, effect-free invocation prefix.
fn invocation(
    source: &SourceControl,
    bindings: &[Option<BlockBindings>],
    targets: &[TargetBlock],
    mapped: &mut [bool],
    inventory: &Inventory<'_>,
    physical: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<usize>> {
    out.budget.reserve_storage(invocation_headers())?;
    let function = inventory.functions().get(physical).ok_or_else(mismatch)?;
    let source_entry = bindings
        .get(source.entry.get() as usize)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let mut prefix = vector(targets.len(), out)?;
    let mut block = 0usize;
    while block != source_entry.physical.block as usize {
        out.budget.charge_work(4)?;
        let seen = mapped.get_mut(block).ok_or_else(mismatch)?;
        if *seen {
            return Err(mismatch());
        }
        *seen = true;
        let target = targets.get(block).ok_or_else(mismatch)?;
        if !matches!(target.branch, TargetBranch::Goto) || target.edges.len() != 1 {
            return Err(mismatch());
        }
        prefix.push(block);
        let edge = &target.edges[0];
        if edge.target.function != function.coordinate {
            return Err(mismatch());
        }
        block = edge.target.block as usize;
    }
    out.budget.charge_work(mapped.len())?;
    if mapped.iter().any(|mapped| !mapped) {
        return Err(mismatch());
    }

    let mut capacity = source.arguments;
    for &block in &prefix {
        out.budget.charge_work(1)?;
        capacity = capacity
            .checked_add(targets[block].program.nodes.len())
            .ok_or(Resource::Arithmetic)?;
    }
    let mut trace =
        super::target_trace::ConcreteTrace::arguments(inventory, physical, capacity, out)?;
    for &block in &prefix {
        let target = &targets[block];
        let appended = trace.append(target, out)?;
        trace.edge(appended, 0, out)?;
    }
    let classes = relation::classes([&trace.nodes, &[]], out)?;
    let mut arguments = vector(source.locals, out)?;
    out.budget.charge_work(source.locals)?;
    arguments.resize(source.locals, None);
    for &(local, ordinal) in &source.initial {
        out.budget.charge_work(1)?;
        arguments[local as usize] = Some(ordinal as usize);
    }
    for &(local, definition) in &source_entry.live {
        out.budget.charge_work(4)?;
        let expected = arguments
            .get(local as usize)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        let definition = definition.ok_or_else(mismatch)?;
        let slot = definition
            .checked_sub(function.definitions.start)
            .ok_or_else(mismatch)?;
        let actual = trace
            .values
            .get(slot)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        if classes[0].get(expected) != classes[0].get(actual) {
            return Err(mismatch());
        }
    }
    Ok(prefix)
}

fn invocation_headers() -> usize {
    size_of::<Vec<usize>>()
        + size_of::<Vec<NodeV30>>()
        + 2 * size_of::<Vec<Option<usize>>>()
        + size_of::<Vec<usize>>()
        + size_of::<[Vec<usize>; 2]>()
        + size_of::<Result<[Vec<usize>; 2]>>()
        + size_of::<Result<Vec<usize>>>()
        + size_of::<NodeV30>()
        + size_of::<ExpressionV30>()
        + size_of::<Result<ExpressionV30>>()
        + 12 * size_of::<usize>()
        + size_of::<Result<usize>>()
}

fn headers() -> usize {
    size_of::<CompleteControl>()
        + size_of::<Result<CompleteControl>>()
        + size_of::<InvocationPlan<'_, '_>>()
        + size_of::<Result<InvocationPlan<'_, '_>>>()
        + size_of::<RootControl>()
        + size_of::<SourceControl>()
        + size_of::<Result<SourceControl>>()
        + size_of::<TargetBlock>()
        + size_of::<Result<TargetBlock>>()
        + size_of::<BlockPair>()
        + size_of::<Result<BlockPair>>()
        + size_of::<BlockBindings>()
        + size_of::<[usize; 6]>()
        + 2 * size_of::<Vec<bool>>()
        + size_of::<Vec<RootControl>>()
        + size_of::<Vec<Option<BlockBindings>>>()
        + size_of::<Vec<Option<BlockPair>>>()
        + size_of::<Vec<TargetBlock>>()
        + size_of::<Vec<usize>>()
        + size_of::<Vec<(u32, Option<usize>)>>()
        + 2 * size_of::<Vec<Option<usize>>>()
        + size_of::<Result<Option<Block>>>()
        + size_of::<Result<Option<usize>>>()
        + 16 * size_of::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_mir_cfg_roster_and_invocation_frames_have_independent_field_oracles() {
        type RootFields = (
            SourceControl,
            Vec<Option<BlockBindings>>,
            Vec<TargetBlock>,
            Vec<Option<usize>>,
            Vec<Option<BlockPair>>,
            Vec<usize>,
            usize,
            usize,
            usize,
        );
        type CompleteFields = (Vec<RootControl>, [usize; 6]);
        assert_eq!(size_of::<RootControl>(), size_of::<RootFields>());
        assert_eq!(size_of::<CompleteControl>(), size_of::<CompleteFields>());
        let invocation = 2 * size_of::<Vec<usize>>()
            + size_of::<Vec<NodeV30>>()
            + 2 * size_of::<Vec<Option<usize>>>()
            + size_of::<[Vec<usize>; 2]>()
            + size_of::<Result<[Vec<usize>; 2]>>()
            + size_of::<Result<Vec<usize>>>()
            + size_of::<NodeV30>()
            + size_of::<ExpressionV30>()
            + size_of::<Result<ExpressionV30>>()
            + 12 * size_of::<usize>()
            + size_of::<Result<usize>>();
        assert_eq!(invocation_headers(), invocation);
        let roster = size_of::<CompleteFields>()
            + size_of::<Result<CompleteControl>>()
            + size_of::<InvocationPlan<'_, '_>>()
            + size_of::<Result<InvocationPlan<'_, '_>>>()
            + size_of::<RootFields>()
            + size_of::<SourceControl>()
            + size_of::<Result<SourceControl>>()
            + size_of::<TargetBlock>()
            + size_of::<Result<TargetBlock>>()
            + size_of::<BlockPair>()
            + size_of::<Result<BlockPair>>()
            + size_of::<BlockBindings>()
            + size_of::<[usize; 6]>()
            + 2 * size_of::<Vec<bool>>()
            + size_of::<Vec<RootControl>>()
            + size_of::<Vec<Option<BlockBindings>>>()
            + size_of::<Vec<Option<BlockPair>>>()
            + size_of::<Vec<TargetBlock>>()
            + size_of::<Vec<usize>>()
            + size_of::<Vec<(u32, Option<usize>)>>()
            + 2 * size_of::<Vec<Option<usize>>>()
            + size_of::<Result<Option<Block>>>()
            + size_of::<Result<Option<usize>>>()
            + 16 * size_of::<usize>();
        assert_eq!(headers(), roster);
    }
}
