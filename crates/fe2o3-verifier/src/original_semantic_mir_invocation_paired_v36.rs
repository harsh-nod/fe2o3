//! One mandatory relation for the source byte interpreter and the original
//! canonical byte interpreter. Cut locators only select where the independent
//! value, heap, effect and control obligations must be proved.

use super::super::{
    LocalRole, ScalarV30, Terminator,
    boundary::{Boundaries, ControlInput},
    invocations::InvocationPlan,
};
use super::{
    Error, Resource, Result, Writer, slots::SourceSlots, source_function::SourceByteProgram, vector,
};
use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, FormalIndexWidth};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block, SsaEdgeIdV1 as Edge, SsaResolvedEventV1 as Event, SsaValueV1 as Value,
    SsaVariableIdV1 as Variable, semantic_mir_v1::SemanticBlockIdV1 as SourceBlock,
};
use std::{mem::size_of, ops::Range};

#[path = "original_semantic_mir_invocation_paired_generate_v36.rs"]
mod generate;

#[path = "original_semantic_mir_invocation_logical_bindings_v38.rs"]
mod logical;
use logical::LogicalBinding;

#[derive(Clone, Copy, Debug)]
enum SourceValue {
    Local(usize),
    Slot { descriptor: usize, bits: u32 },
}

#[derive(Clone, Copy, Debug)]
struct Binding {
    source: SourceValue,
    logical: LogicalBinding,
    definition: Option<usize>,
    frame: usize,
}

#[derive(Clone, Copy, Debug)]
enum End {
    Ordinary,
    Call(usize),
    Return,
}

struct Cut {
    source: usize,
    instance: usize,
    live: Vec<Binding>,
    end: End,
}

struct Instance {
    owners: Vec<u32>,
    suspended: Vec<Binding>,
    arguments: Vec<Binding>,
    returned: Option<Binding>,
    outgoing: Vec<(u32, Vec<Binding>)>,
}

struct Root {
    blocks: Range<usize>,
    cuts: Vec<Option<Cut>>,
    instances: Range<usize>,
    owner: u32,
    parameters: Vec<RootArgument>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootArgument {
    source: usize,
    definition: usize,
}

pub(super) struct PairedInvocations<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    roots: Vec<Root>,
    instances: Vec<Option<Instance>>,
    definitions: usize,
    locals: usize,
    width: FormalIndexWidth,
    census: [usize; 6],
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR paired byte relation differs from its exact source cuts")
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn block(index: usize) -> Result<Block> {
    Ok(Block::new(
        u32::try_from(index).map_err(|_| Resource::Arithmetic)?,
    ))
}

impl<'slots, 'view, 'source> PairedInvocations<'slots, 'view, 'source> {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        program: &SourceByteProgram<'slots, 'view, 'source>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let slots = program.source_slots(out)?;
        let relation = slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) || width == FormalIndexWidth::Unknown {
            return Err(mismatch());
        }
        let semantic = source.source_semantic(out.budget)?;
        let archive = source.source_ssa(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        let count = source.root_count(out.budget)?;
        if count != inventory.functions().len() {
            return Err(mismatch());
        }
        let mut total = 0usize;
        for root in 0..count {
            out.budget.charge_work(1)?;
            total = add(total, plan.root(root, out)?.instances.len())?;
        }
        let mut result = Self {
            slots,
            roots: vector(count, out)?,
            instances: vector(total, out)?,
            definitions: inventory.definitions().len(),
            locals: 0,
            width,
            census: [
                count,
                total,
                0,
                0,
                inventory.operations().len(),
                inventory.definitions().len(),
            ],
            required: 0,
        };
        let mut seen = vector(count, out)?;
        out.budget.charge_work(count)?;
        seen.resize(count, false);
        for root in 0..count {
            let scope = plan.root(root, out)?;
            out.budget.charge_work(3)?;
            if scope.instances.start != result.instances.len()
                || seen.get(scope.physical) != Some(&false)
            {
                return Err(mismatch());
            }
            seen[scope.physical] = true;
            let physical = inventory
                .functions()
                .get(scope.physical)
                .ok_or_else(mismatch)?;
            let mut cuts = vector(physical.blocks.len(), out)?;
            out.budget.charge_work(physical.blocks.len())?;
            cuts.resize_with(physical.blocks.len(), || None);
            for instance in 0..scope.instances.len() {
                let row = plan.instance(root, instance, out)?;
                out.budget.charge_work(4)?;
                if row.locals.start != result.locals {
                    return Err(mismatch());
                }
                result.locals = row.locals.end;
                let function = semantic
                    .functions()
                    .get(row.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let ssa = archive
                    .plan_for_function(row.function)
                    .ok_or_else(mismatch)?
                    .plan();
                if function.blocks().len() != row.blocks.len()
                    || function.locals().len() != row.locals.len()
                {
                    return Err(mismatch());
                }
                if !row.active {
                    for ordinal in 0..row.blocks.len() {
                        out.budget.charge_work(1)?;
                        if relation
                            .source_block_entry(
                                root,
                                instance,
                                SourceBlock::from_index(block(ordinal)?.get()),
                                out.budget,
                            )?
                            .is_some()
                        {
                            return Err(mismatch());
                        }
                    }
                    result.instances.push(None);
                    continue;
                }
                let mut successors = vector(function.blocks().len(), out)?;
                for declaration in function.blocks() {
                    out.budget.charge_work(2)?;
                    let mut edges = vector(declaration.terminator().kind().edge_count(), out)?;
                    declaration.terminator().kind().try_for_each_edge(|edge| {
                        out.budget.charge_work(1)?;
                        edges.push(Block::new(edge.target().index()));
                        Ok::<_, Error>(())
                    })?;
                    successors.push(edges);
                }
                let boundaries = Boundaries::derive(
                    ssa,
                    ControlInput {
                        entry: Block::new(function.entry().index()),
                        successors: &successors,
                    },
                    out,
                )?;
                let mut owners = vector(instance.checked_add(1).ok_or(Resource::Arithmetic)?, out)?;
                let mut current = instance;
                loop {
                    out.budget.charge_work(2)?;
                    let ancestor = plan.instance(root, current, out)?;
                    owners.push(ancestor.function.index());
                    match ancestor.incoming {
                        Some((parent, _)) if parent < current => current = parent,
                        None if current == 0 => break,
                        _ => return Err(mismatch()),
                    }
                }
                out.budget.charge_work(owners.len())?;
                owners.reverse();
                let frame = owners.len().checked_sub(1).ok_or(Resource::Arithmetic)?;
                let arguments_count = function.abi().source_input_types().len();
                let mut arguments = vector(arguments_count, out)?;
                let mut argument_rows = vector(arguments_count, out)?;
                out.budget.charge_work(arguments_count)?;
                argument_rows.resize(arguments_count, None);
                for (local, declaration) in function.locals().iter().enumerate() {
                    out.budget.charge_work(2)?;
                    if let LocalRole::Argument(argument) = declaration.role() {
                        let place = argument_rows
                            .get_mut(argument as usize)
                            .ok_or_else(mismatch)?;
                        if place.is_some()
                            || function.abi().source_input_types().get(argument as usize)
                                != Some(&declaration.ty())
                        {
                            return Err(mismatch());
                        }
                        let entries = ssa.entry_definitions();
                        out.budget.charge_work(
                            (usize::BITS - entries.len().max(1).leading_zeros()) as usize + 2,
                        )?;
                        let at = entries
                            .binary_search_by_key(&(local as u32), |row| row.variable().get())
                            .map_err(|_| mismatch())?;
                        let binding = result.binding(
                            plan,
                            root,
                            instance,
                            local,
                            entries[at].value(),
                            frame,
                            &physical.definitions,
                            out,
                        )?;
                        if instance == 0 {
                            if let Some(parameter) = result.slots.descriptor_parameter(
                                root,
                                argument as usize,
                                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                                ),
                                out,
                            )? {
                                out.budget.charge_work(2)?;
                                if binding
                                    .definition
                                    .and_then(|index| inventory.definitions().get(index))
                                    .map(|row| row.coordinate)
                                    != Some(parameter)
                                {
                                    return Err(mismatch());
                                }
                            }
                        }
                        *place = Some(binding);
                    }
                }
                for argument in &argument_rows {
                    out.budget.charge_work(1)?;
                    arguments.push(argument.ok_or_else(mismatch)?);
                }
                let suspended = result.suspended(plan, root, instance, out)?;
                let returned = if let Some((parent, site)) = row.incoming {
                    let parent_row = plan.instance(root, parent, out)?;
                    let parent_function =
                        &semantic.functions()[parent_row.function.index() as usize];
                    let call = match parent_function
                        .blocks()
                        .get(site.index() as usize)
                        .map(|row| row.terminator().kind())
                    {
                        Some(Terminator::Call(call)) => call,
                        _ => return Err(mismatch()),
                    };
                    let destination = call.destination().ok_or_else(mismatch)?;
                    let parent_ssa = archive
                        .plan_for_function(parent_row.function)
                        .ok_or_else(mismatch)?
                        .plan();
                    let definitions = parent_ssa
                        .edge_definitions(Edge::new(Block::new(site.index()), 0))
                        .ok_or_else(mismatch)?;
                    out.budget.charge_work(
                        definitions
                            .len()
                            .checked_add(2)
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    match definitions {
                        [definition]
                            if definition.variable().get()
                                == destination.place().local().index() =>
                        {
                            Some(result.binding(
                                plan,
                                root,
                                parent,
                                definition.variable().get() as usize,
                                definition.value(),
                                frame - 1,
                                &physical.definitions,
                                out,
                            )?)
                        }
                        [] if ScalarV30::from_source(
                            semantic.types(),
                            destination.place().ty(),
                        )? == ScalarV30::Unit =>
                        {
                            Some(Binding {
                                source: SourceValue::Local(add(
                                    parent_row.locals.start,
                                    destination.place().local().index() as usize,
                                )?),
                                definition: None,
                                logical: LogicalBinding::Plain,
                                frame: frame - 1,
                            })
                        }
                        _ => return Err(mismatch()),
                    }
                } else {
                    None
                };
                for (ordinal, declaration) in function.blocks().iter().enumerate() {
                    out.budget.charge_work(3)?;
                    let id = block(ordinal)?;
                    let location = relation.source_block_entry(
                        root,
                        instance,
                        SourceBlock::from_index(id.get()),
                        out.budget,
                    )?;
                    let Some(live) = ssa.live_in(id) else {
                        if location.is_some() {
                            return Err(mismatch());
                        }
                        continue;
                    };
                    let location = location.ok_or_else(mismatch)?;
                    if location.function != physical.coordinate {
                        return Err(mismatch());
                    }
                    let cut = cuts.get_mut(location.block as usize).ok_or_else(mismatch)?;
                    if cut.is_some() {
                        return Err(mismatch());
                    }
                    let mut bindings = vector(live.len(), out)?;
                    for &variable in live {
                        out.budget.charge_work(2)?;
                        bindings.push(result.binding(
                            plan,
                            root,
                            instance,
                            variable.get() as usize,
                            boundaries.value(id, variable, out)?,
                            frame,
                            &physical.definitions,
                            out,
                        )?);
                    }
                    let end = match declaration.terminator().kind() {
                        Terminator::Call(_) => {
                            let calls = plan.calls(root, instance, out)?;
                            out.budget.charge_work(
                                (usize::BITS - calls.len().max(1).leading_zeros()) as usize + 2,
                            )?;
                            let at = calls
                                .binary_search_by_key(&id.get(), |row| row.block.index())
                                .map_err(|_| mismatch())?;
                            match calls[at].child {
                                Some(child) => End::Call(add(scope.instances.start, child)?),
                                None if program.in_place_call(root, instance, ordinal, out)? => {
                                    End::Ordinary
                                }
                                None => return Err(mismatch()),
                            }
                        }
                        Terminator::Return => End::Return,
                        Terminator::Goto(_)
                        | Terminator::SwitchInt { .. }
                        | Terminator::Assert { .. } => End::Ordinary,
                        _ => return Err(mismatch()),
                    };
                    result.census[2] = add(result.census[2], declaration.statements().len())?;
                    out.budget.charge_work(declaration.statements().len())?;
                    for statement in declaration.statements() {
                        if matches!(statement.kind(), super::super::Statement::Assign(_)) {
                            result.census[3] = add(result.census[3], 1)?;
                        }
                    }
                    *cut = Some(Cut {
                        source: add(row.blocks.start, ordinal)?,
                        instance: add(scope.instances.start, instance)?,
                        live: bindings,
                        end,
                    });
                }
                let calls = plan.calls(root, instance, out)?;
                let mut outgoing = vector(calls.len(), out)?;
                for call in calls {
                    out.budget.charge_work(1)?;
                    if call.ssa_reachable && call.child.is_some() {
                        outgoing.push((
                            call.block.index(),
                            result.caller_carry(
                                plan,
                                root,
                                instance,
                                call.block,
                                &boundaries,
                                frame,
                                &physical.definitions,
                                out,
                            )?,
                        ));
                    }
                }
                result.instances.push(Some(Instance {
                    owners,
                    suspended,
                    arguments,
                    returned,
                    outgoing,
                }));
            }
            let entry = result
                .instances
                .get(scope.instances.start)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            let mut parameters = vector(physical.function.signature.parameters.len(), out)?;
            let mut parameter_seen = vector(physical.function.signature.parameters.len(), out)?;
            out.budget
                .charge_work(physical.function.signature.parameters.len())?;
            parameter_seen.resize(physical.function.signature.parameters.len(), false);
            for (argument, binding) in entry.arguments.iter().enumerate() {
                out.budget.charge_work(3)?;
                // A Unit source argument has no physical carrier. All other
                // bindings name an exact parameter, never an ordinal zip.
                let Some(index) = binding.definition else {
                    continue;
                };
                let definition = inventory.definitions().get(index).ok_or_else(mismatch)?;
                let Definition::FunctionArgument {
                    function,
                    argument: physical_argument,
                } = definition.coordinate
                else {
                    return Err(mismatch());
                };
                let physical_argument = physical_argument as usize;
                if function != physical.coordinate
                    || physical
                        .function
                        .signature
                        .parameters
                        .get(physical_argument)
                        != Some(definition.ty)
                    || parameter_seen.get(physical_argument) != Some(&false)
                {
                    return Err(mismatch());
                }
                parameter_seen[physical_argument] = true;
                parameters.push(RootArgument {
                    source: argument,
                    definition: index,
                });
            }
            out.budget.charge_work(parameter_seen.len())?;
            if parameter_seen.iter().any(|seen| !seen) {
                return Err(mismatch());
            }
            result.roots.push(Root {
                blocks: physical.blocks.clone(),
                cuts,
                instances: scope.instances.clone(),
                owner: scope.function.index(),
                parameters,
            });
        }
        out.budget.charge_work(seen.len())?;
        if result.instances.len() != total || seen.iter().any(|seen| !seen) {
            return Err(mismatch());
        }
        result.required = out.budget.storage();
        Ok(result)
    }

    fn binding(
        &self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        local: usize,
        value: Value,
        frame: usize,
        physical: &Range<usize>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Binding> {
        let relation = self.slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        let row = plan.instance(root, instance, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let declaration = semantic
            .functions()
            .get(row.function.index() as usize)
            .and_then(|function| function.locals().get(local))
            .ok_or_else(mismatch)?;
        let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
        out.budget.charge_work(6)?;
        if endpoint.source_function(out.budget)? != row.function
            || endpoint.source_local(out.budget)?.index() as usize != local
            || endpoint.source_type(out.budget)? != declaration.ty()
        {
            return Err(mismatch());
        }
        let definition = endpoint.original_definition(out.budget)?;
        let logical = LogicalBinding::derive(self.slots, plan, root, &endpoint, out)?;
        let inventory = relation.inventory(out.budget)?;
        match (definition, endpoint.physical_type(out.budget)?) {
            (Some(definition), Some(ty))
                if physical.contains(&definition)
                    && inventory
                        .definitions()
                        .get(definition)
                        .is_some_and(|row| row.ty == ty) =>
            {
                super::super::super::byte_function_v30::value_type(ty)?
            }
            (None, None)
                if ScalarV30::from_source(semantic.types(), declaration.ty())?
                    == ScalarV30::Unit =>
            {
                ()
            }
            _ => return Err(mismatch()),
        }
        let source = match self.slots.legacy_descriptor_by_source(
            root,
            instance,
            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
            out,
        )? {
            Some((descriptor, _)) => SourceValue::Slot {
                descriptor,
                bits: ScalarV30::from_source(semantic.types(), declaration.ty())?.width(),
            },
            None => SourceValue::Local(add(row.locals.start, local)?),
        };
        Ok(Binding {
            source,
            logical,
            definition,
            frame,
        })
    }

    fn suspended(
        &self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Vec<Binding>> {
        let row = plan.instance(root, instance, out)?;
        let Some((parent, site)) = row.incoming else {
            return vector(0, out);
        };
        let scope = plan.root(root, out)?;
        let parent_index = add(scope.instances.start, parent)?;
        let inherited = self
            .instances
            .get(parent_index)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        out.budget.charge_work(
            (usize::BITS - inherited.outgoing.len().max(1).leading_zeros()) as usize + 2,
        )?;
        let at = inherited
            .outgoing
            .binary_search_by_key(&site.index(), |row| row.0)
            .map_err(|_| mismatch())?;
        let carry = &inherited.outgoing[at].1;
        let mut result = vector(add(inherited.suspended.len(), carry.len())?, out)?;
        for &binding in inherited.suspended.iter().chain(carry) {
            out.budget.charge_work(1)?;
            result.push(binding);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn caller_carry(
        &self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        parent: usize,
        site: SourceBlock,
        boundaries: &Boundaries<'_>,
        frame: usize,
        physical: &Range<usize>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Vec<Binding>> {
        let caller = plan.instance(root, parent, out)?;
        let relation = self.slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = &semantic.functions()[caller.function.index() as usize];
        let ssa = source
            .source_ssa(out.budget)?
            .plan_for_function(caller.function)
            .ok_or_else(mismatch)?
            .plan();
        let call = match function
            .blocks()
            .get(site.index() as usize)
            .map(|row| row.terminator().kind())
        {
            Some(Terminator::Call(call)) => call,
            _ => return Err(mismatch()),
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        let next = Block::new(destination.edge().target().index());
        let live = ssa.live_in(next).ok_or_else(mismatch)?;
        let mut result = vector(live.len(), out)?;
        // Reconstruct the caller's exact post-statement SSA environment once.
        // This is independent of callee liveness and includes every Kill.
        let mut values = vector(caller.locals.len(), out)?;
        out.budget.charge_work(caller.locals.len())?;
        values.resize(caller.locals.len(), None);
        for &variable in ssa.live_in(Block::new(site.index())).ok_or_else(mismatch)? {
            out.budget.charge_work(1)?;
            *values
                .get_mut(variable.get() as usize)
                .ok_or_else(mismatch)? =
                Some(boundaries.value(Block::new(site.index()), variable, out)?);
        }
        for &(_, event) in ssa
            .resolved_events(Block::new(site.index()))
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(2)?;
            match event {
                Event::Define { variable, value } => {
                    *values
                        .get_mut(variable.get() as usize)
                        .ok_or_else(mismatch)? = Some(value)
                }
                Event::Kill { variable, .. } => {
                    *values
                        .get_mut(variable.get() as usize)
                        .ok_or_else(mismatch)? = None
                }
                Event::Use { .. } => (),
            }
        }
        for &variable in live {
            out.budget.charge_work(2)?;
            if variable.get() == destination.place().local().index() {
                continue;
            }
            let value = values
                .get(variable.get() as usize)
                .copied()
                .flatten()
                .ok_or_else(mismatch)?;
            result.push(self.binding(
                plan,
                root,
                parent,
                variable.get() as usize,
                value,
                frame,
                physical,
                out,
            )?);
        }
        Ok(result)
    }

    pub(super) fn census(&self) -> [usize; 6] {
        self.census
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        generate::emit(self, out)
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<PairedInvocations<'_, '_, '_>>()
        + h::<Root>()
        + h::<RootArgument>()
        + h::<Instance>()
        + h::<Cut>()
        + h::<Binding>()
        + logical::headers()
        + h::<SourceValue>()
        + h::<End>()
        + h::<Vec<Root>>()
        + h::<Vec<RootArgument>>()
        + h::<Vec<Option<Instance>>>()
        + h::<Vec<Option<Cut>>>()
        + h::<Vec<Binding>>()
        + h::<Vec<Option<Binding>>>()
        + h::<Vec<Option<Value>>>()
        + h::<Vec<u32>>()
        + h::<Vec<bool>>()
        + h::<Vec<Vec<Block>>>()
        + h::<Vec<Block>>()
        + h::<Boundaries<'_>>()
        + h::<ControlInput<'_>>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<Value>()
        + h::<Variable>()
        + h::<Edge>()
        + 64 * size_of::<usize>()
        + 48 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocation_paired_v36_tests.rs"]
mod tests;
