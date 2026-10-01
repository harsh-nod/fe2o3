//! Exact all-invocation source cuts for concrete call/return segment replay.
//! Locators identify endpoints; they are not a source-equivalence certificate.
use super::super::super::Inventory;
use super::super::{canonical::control::TargetBlock, control_pair::BlockBindings};
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18 as Correspondence;
use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1 as SourceBlock;

struct ActualBody {
    bindings: Vec<Option<BlockBindings>>,
    suspended: Vec<Suspended>,
    inputs: Vec<(u32, Option<usize>)>,
    returned: Option<Option<usize>>,
}

#[derive(Clone, Copy)]
struct Suspended {
    local: usize,
    definition: Option<usize>,
    scalar: super::super::ScalarV30,
}

struct ActualRoot {
    physical: usize,
    targets: Vec<TargetBlock>,
    /// (Global body row, original function-local block), never a function guess.
    cuts: Vec<Option<(usize, usize)>>,
    boundaries: Vec<bool>,
    capacity: usize,
}

#[path = "original_semantic_mir_invocation_segments_v35.rs"]
mod segments;

pub(super) struct ActualInvocations<'m, 'a, 'plan, 'view, 'source, 'i> {
    original: &'m InvocationBodies<'a, 'plan, 'view, 'source>,
    inventory: &'i Inventory<'i>,
    roots: Vec<ActualRoot>,
    bodies: Vec<Option<ActualBody>>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR invocation body differs from its actual source cuts")
}

impl<'m, 'a, 'plan, 'view, 'source, 'i> ActualInvocations<'m, 'a, 'plan, 'view, 'source, 'i> {
    pub(super) fn derive(
        original: &'m InvocationBodies<'a, 'plan, 'view, 'source>,
        relation: &'i Correspondence<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        original.check(out)?;
        let source = original.transfers.plan(out)?.source(out)?;
        if !std::ptr::eq(source, relation.source(out.budget)?) {
            return Err(mismatch());
        }
        out.budget.reserve_storage(headers())?;
        let inventory = relation.inventory(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        let plan = original.transfers.plan(out)?;
        if original.roots.len() != inventory.functions().len() {
            return Err(mismatch());
        }
        let mut roots = vector(original.roots.len(), out)?;
        let mut bodies = vector(original.bodies.len(), out)?;
        let mut seen = vector(original.roots.len(), out)?;
        out.budget.charge_work(original.roots.len())?;
        seen.resize(original.roots.len(), false);
        for (root, range) in original.roots.iter().enumerate() {
            let declaration = plan.root(root, out)?;
            if declaration.instances != *range || range.start != bodies.len() {
                return Err(mismatch());
            }
            let physical = declaration.physical;
            if seen.get(physical) != Some(&false) {
                return Err(mismatch());
            }
            seen[physical] = true;
            let function = inventory.functions().get(physical).ok_or_else(mismatch)?;
            let mut cuts = vector(function.blocks.len(), out)?;
            out.budget.charge_work(function.blocks.len())?;
            cuts.resize(function.blocks.len(), None);
            for index in range.clone() {
                out.budget.charge_work(3)?;
                let instance = index.checked_sub(range.start).ok_or(Resource::Arithmetic)?;
                let row = plan.instance(root, instance, out)?;
                let body = original.bodies.get(index).ok_or_else(mismatch)?;
                let Some(body) = body else {
                    if row.active {
                        return Err(mismatch());
                    }
                    for block in 0..row.blocks.len() {
                        out.budget.charge_work(1)?;
                        let id = source_block(block)?;
                        if relation
                            .source_block_entry(root, instance, id, out.budget)?
                            .is_some()
                        {
                            return Err(mismatch());
                        }
                    }
                    bodies.push(None);
                    continue;
                };
                if !row.active
                    || (body.root, body.instance) != (root, instance)
                    || body.blocks != row.blocks
                    || body.locals != row.locals
                {
                    return Err(mismatch());
                }
                let mut bindings = vector(body.control.blocks.len(), out)?;
                for (block, original_block) in body.control.blocks.iter().enumerate() {
                    out.budget.charge_work(3)?;
                    let id = source_block(block)?;
                    let location = relation.source_block_entry(root, instance, id, out.budget)?;
                    let Some(original_block) = original_block else {
                        if location.is_some() {
                            return Err(mismatch());
                        }
                        bindings.push(None);
                        continue;
                    };
                    let location = location.ok_or_else(mismatch)?;
                    if location.function != function.coordinate {
                        return Err(mismatch());
                    }
                    let cut = cuts.get_mut(location.block as usize).ok_or_else(mismatch)?;
                    if cut.is_some() {
                        return Err(mismatch());
                    }
                    *cut = Some((index, block));
                    let mut live = vector(original_block.live.len(), out)?;
                    for value in &original_block.live {
                        out.budget.charge_work(2)?;
                        let definition = relation.ssa_scalar_definition_v30(
                            root,
                            instance,
                            value.value,
                            out.budget,
                        )?;
                        scalar_definition(
                            inventory,
                            &function.definitions,
                            definition,
                            *body
                                .control
                                .types
                                .get(value.local as usize)
                                .ok_or_else(mismatch)?,
                        )?;
                        live.push((value.local, definition));
                    }
                    let mut assignments = vector(original_block.program.assignments.len(), out)?;
                    for assignment in &original_block.program.assignments {
                        out.budget.charge_work(1)?;
                        let definition = relation.assignment_scalar_definition_v30(
                            root,
                            instance,
                            id,
                            assignment.statement,
                            out.budget,
                        )?;
                        scalar_definition(
                            inventory,
                            &function.definitions,
                            definition,
                            original_block
                                .program
                                .nodes
                                .get(assignment.value)
                                .ok_or_else(mismatch)?
                                .scalar,
                        )?;
                        assignments.push(definition);
                    }
                    bindings.push(Some(BlockBindings {
                        physical: location,
                        live,
                        assignments,
                    }));
                }
                let original_plan = ssa
                    .plan_for_function(row.function)
                    .ok_or_else(mismatch)?
                    .plan();
                let entries = original_plan.entry_definitions();
                let mut inputs = vector(body.control.initial.len(), out)?;
                for &(local, _) in &body.control.initial {
                    out.budget.charge_work(
                        (usize::BITS - entries.len().max(1).leading_zeros()) as usize + 3,
                    )?;
                    let at = entries
                        .binary_search_by_key(&local, |entry| entry.variable().get())
                        .map_err(|_| mismatch())?;
                    let definition = relation.ssa_scalar_definition_v30(
                        root,
                        instance,
                        entries[at].value(),
                        out.budget,
                    )?;
                    scalar_definition(
                        inventory,
                        &function.definitions,
                        definition,
                        body.control.types[local as usize],
                    )?;
                    inputs.push((local, definition));
                }
                let returned = if let Some(transfer) = body.returned {
                    let call = original.transfer(transfer, out)?;
                    let (parent, site) = row.incoming.ok_or_else(mismatch)?;
                    let parent_row = plan.instance(root, parent, out)?;
                    let parent_plan = ssa
                        .plan_for_function(parent_row.function)
                        .ok_or_else(mismatch)?
                        .plan();
                    let edges = parent_plan
                        .edge_definitions(fe2o3_mir_model::SsaEdgeIdV1::new(
                            fe2o3_mir_model::SsaBlockIdV1::new(site.index()),
                            0,
                        ))
                        .ok_or_else(mismatch)?;
                    let local = call
                        .returned
                        .destination
                        .checked_sub(parent_row.locals.start)
                        .ok_or_else(mismatch)?;
                    out.budget
                        .charge_work(edges.len().checked_add(3).ok_or(Resource::Arithmetic)?)?;
                    let definition = match edges {
                        [entry] if entry.variable().get() as usize == local => relation
                            .ssa_scalar_definition_v30(root, parent, entry.value(), out.budget)?,
                        [] if call.returned.scalar == super::super::ScalarV30::Unit => None,
                        _ => return Err(mismatch()),
                    };
                    scalar_definition(
                        inventory,
                        &function.definitions,
                        definition,
                        call.returned.scalar,
                    )?;
                    Some(definition)
                } else {
                    None
                };
                let suspended = segments::suspended(original, &bodies, index, out)?;
                bodies.push(Some(ActualBody {
                    bindings,
                    suspended,
                    inputs,
                    returned,
                }));
            }
            let mut targets = vector(function.blocks.len(), out)?;
            let mut capacity = function.definitions.len();
            for block in function.blocks.clone() {
                let target = TargetBlock::derive(inventory, block, out)?;
                capacity = capacity
                    .checked_add(target.program.nodes.len())
                    .ok_or(Resource::Arithmetic)?;
                targets.push(target);
            }
            let mut boundaries = vector(cuts.len(), out)?;
            for cut in &cuts {
                out.budget.charge_work(1)?;
                boundaries.push(cut.is_some());
            }
            roots.push(ActualRoot {
                physical,
                targets,
                cuts,
                boundaries,
                capacity,
            });
        }
        out.budget.charge_work(seen.len())?;
        if bodies.len() != original.bodies.len() || seen.iter().any(|seen| !seen) {
            return Err(mismatch());
        }
        Ok(Self {
            original,
            inventory,
            roots,
            bodies,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        self.original.check(out)?;
        if out.budget.storage() < self.required {
            return Err(self
                .original
                .transfers
                .plan(out)?
                .source(out)?
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    /// Checks scalar equations and complete physical block coverage. No memory
    /// interpretation, dynamic allocation identity, or proof authority is issued.
    pub(super) fn check_segments(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        segments::check(self, out)
    }
}

fn scalar_definition(
    inventory: &Inventory<'_>,
    range: &Range<usize>,
    definition: Option<usize>,
    scalar: super::super::ScalarV30,
) -> Result<()> {
    match definition {
        Some(definition)
            if range.contains(&definition)
                && super::super::canonical::scalar(
                    inventory
                        .definitions()
                        .get(definition)
                        .ok_or_else(mismatch)?
                        .ty,
                )? == scalar =>
        {
            Ok(())
        }
        None if scalar == super::super::ScalarV30::Unit => Ok(()),
        _ => Err(mismatch()),
    }
}

fn source_block(block: usize) -> Result<SourceBlock> {
    Ok(SourceBlock::from_index(
        u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
    ))
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        + h::<ActualBody>()
        + h::<ActualRoot>()
        + h::<BlockBindings>()
        + h::<Vec<ActualRoot>>()
        + h::<Vec<Option<ActualBody>>>()
        + h::<Vec<Option<BlockBindings>>>()
        + h::<Vec<Option<(usize, usize)>>>()
        + h::<Vec<(u32, Option<usize>)>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<TargetBlock>>()
        + h::<Vec<bool>>()
        + h::<TargetBlock>()
        + h::<Vec<Suspended>>()
        + h::<Suspended>()
        + h::<Option<Option<usize>>>()
        + h::<&fe2o3_mir_model::SsaConstructionPlanV1>()
        + h::<&[fe2o3_mir_model::SsaArgumentV1]>()
        + h::<fe2o3_mir_model::SsaEdgeIdV1>()
        + h::<&Correspondence<'_>>()
        + h::<&Inventory<'_>>()
        + h::<&InvocationBodies<'_, '_, '_, '_>>()
        + h::<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>()
        + h::<Option<usize>>()
        + h::<SourceBlock>()
        + h::<Range<usize>>()
        + h::<Option<(usize, usize)>>()
        + h::<(
            &Inventory<'_>,
            &Range<usize>,
            Option<usize>,
            super::super::ScalarV30,
        )>()
        + 16 * size_of::<usize>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocation_actual_v35_tests.rs"]
mod tests;
