//! One mandatory relation for the source byte interpreter and the original
//! canonical byte interpreter. Cut locators only select where the independent
//! value, heap, effect and control obligations must be proved.

use super::super::{
    LocalRole, ScalarV30, Terminator,
    boundary::{Boundaries, ControlInput},
    invocations::InvocationPlan,
};
use super::source_function::{
    SourceCallHintsV85, SourceCutHintsV85, SourceEntryHintsV85, SourceStepHintsV85,
};
use super::{
    Error, Resource, Result, Writer,
    component_demands::ComponentDemandsV42,
    slots::SourceSlots,
    source_frame_demands::{self, ComponentCut},
    source_function::SourceByteProgram,
    vector,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, FormalIndexWidth, FunctionRole,
};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block, SsaEdgeIdV1 as Edge, SsaResolvedEventV1 as Event, SsaValueV1 as Value,
    SsaVariableIdV1 as Variable, semantic_mir_v1::SemanticBlockIdV1 as SourceBlock,
};
use std::{mem::size_of, ops::Range};

#[path = "original_semantic_mir_invocation_paired_generate_v36.rs"]
mod generate;

#[path = "original_semantic_mir_expanded_scalar_bindings_v196.rs"]
mod expanded_scalar;
pub(super) use expanded_scalar::ExpandedScalarBindingsV196;

#[path = "original_semantic_mir_expanded_live_values_v213.rs"]
mod expanded_live;
pub(super) use expanded_live::emit_source_cut_values_v213;

#[path = "original_semantic_mir_invocation_logical_bindings_v38.rs"]
mod logical;
use logical::LogicalBinding;

#[path = "original_semantic_mir_aggregate_bindings_v42.rs"]
mod aggregate_bindings;
use aggregate_bindings::AggregateBindingV42;
#[path = "original_semantic_mir_enum_bindings_v49.rs"]
mod enum_bindings;
use enum_bindings::EnumBinding;
#[path = "original_semantic_mir_object_returns_v42.rs"]
mod object_returns;
use object_returns::ObjectReturnsV42;

#[derive(Clone, Copy, Debug)]
enum SourceValue {
    Local(usize),
    Slot { descriptor: usize, bits: u32 },
    Aggregate(usize),
    Enum(usize),
    ReturnSnapshot,
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
    step_hints: Option<SourceStepHintsV85>,
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
    aggregates: Vec<AggregateBindingV42>,
    enums: Vec<EnumBinding>,
    component_demands: Vec<Option<ComponentDemandsV42<'slots, 'view, 'source>>>,
    object_returns: ObjectReturnsV42<'slots, 'view, 'source>,
    required: usize,
}

#[cfg_attr(test, track_caller)]
fn mismatch() -> Error {
    #[cfg(test)]
    eprintln!(
        "paired source-cut refusal at {}\n{}",
        std::panic::Location::caller(),
        std::backtrace::Backtrace::force_capture(),
    );
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

fn check_root_census(
    functions: &[fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>],
    seen: &[bool],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if functions.len() != seen.len() {
        return Err(mismatch());
    }
    out.budget.charge_work(functions.len())?;
    if functions.iter().zip(seen).any(|(function, selected)| {
        *selected != (function.function.role == FunctionRole::KernelEntry)
    }) {
        return Err(Error::Statement(
            "paired original roots differ from the complete canonical kernel-entry census",
        ));
    }
    Ok(())
}

impl<'slots, 'view, 'source> PairedInvocations<'slots, 'view, 'source> {
    pub(super) fn expanded_scalar_bindings<'target>(
        &self,
        target: &'target super::tile_target::TileTargetV176<'slots, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<ExpandedScalarBindingsV196<'target, 'slots, 'view, 'source>> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            ExpandedScalarBindingsV196::derive(self.slots, target, out)
        })
    }

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
        if count > inventory.functions().len() {
            return Err(mismatch());
        }
        let mut total = 0usize;
        for root in 0..count {
            out.budget.charge_work(1)?;
            total = add(total, plan.root(root, out)?.instances.len())?;
        }
        let mut component_demands = vector(semantic.functions().len(), out)?;
        out.budget.charge_work(semantic.functions().len())?;
        component_demands.resize_with(semantic.functions().len(), || None);
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
            aggregates: vector(0, out)?,
            enums: vector(0, out)?,
            component_demands,
            object_returns: ObjectReturnsV42::derive(slots, width, out)?,
            required: 0,
        };
        // Root coordinates are complete-module ordinals. The inventory can
        // retain verified non-entry helpers that are not invocation roots.
        let mut seen = vector(inventory.functions().len(), out)?;
        out.budget.charge_work(inventory.functions().len())?;
        seen.resize(inventory.functions().len(), false);
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
            out.budget.charge_work(1)?;
            if physical.function.role != FunctionRole::KernelEntry {
                return Err(Error::Statement(
                    "paired original root names a non-entry canonical function",
                ));
            }
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
                            .binary_search_by_key(&(local as u32), |row| row.variable().get());
                        let parameter = if instance == 0 {
                            result.slots.descriptor_parameter(
                                root,
                                argument as usize,
                                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                                ),
                                out,
                            )?
                        } else {
                            None
                        };
                        let binding = if let Ok(at) = at {
                            result.binding(
                                plan,
                                root,
                                instance,
                                local,
                                entries[at].value(),
                                frame,
                                &physical.definitions,
                                None,
                                out,
                            )?
                        } else {
                            // An address-observable source descriptor need not
                            // have an SSA entry. Its exact nominal ABI recipe,
                            // not an invented SSA value, binds the native Slice.
                            out.budget.charge_work(10)?;
                            let coordinate = parameter.ok_or_else(mismatch)?;
                            let Definition::FunctionArgument { function, argument } = coordinate
                            else {
                                return Err(mismatch());
                            };
                            let index = add(physical.definitions.start, argument as usize)?;
                            let definition =
                                inventory.definitions().get(index).ok_or_else(mismatch)?;
                            if function != physical.coordinate
                                || !physical.definitions.contains(&index)
                                || definition.coordinate != coordinate
                                || !matches!(definition.ty, fe2o3_kernel_ir::Type::Slice(_))
                                || result.slots.has_original_object(
                                    root,
                                    instance,
                                    local as u32,
                                    out,
                                )?
                                || result
                                    .slots
                                    .legacy_descriptor_by_source(
                                        root,
                                        instance,
                                        local as u32,
                                        "paired-entry-scalar",
                                        out,
                                    )?
                                    .is_some()
                            {
                                return Err(mismatch());
                            }
                            Binding {
                                source: SourceValue::Local(add(row.locals.start, local)?),
                                logical: LogicalBinding::Plain,
                                definition: Some(index),
                                frame,
                            }
                        };
                        if let Some(parameter) = parameter {
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
                    if slots.has_original_object(
                        root,
                        parent,
                        destination.place().local().index(),
                        out,
                    )? {
                        if !definitions.is_empty() {
                            return Err(mismatch());
                        }
                        let definition = result.object_returns.definition(
                            root,
                            parent,
                            site.index(),
                            destination.place().local().index(),
                            destination.place().ty(),
                            out,
                        )?;
                        if !physical.definitions.contains(&definition) {
                            return Err(mismatch());
                        }
                        Some(Binding {
                            source: SourceValue::ReturnSnapshot,
                            logical: LogicalBinding::Plain,
                            definition: Some(definition),
                            frame: frame - 1,
                        })
                    } else {
                        match definitions {
                            [definition]
                                if definition.variable().get()
                                    == destination.place().local().index() =>
                            {
                                Some(result.returned_binding(
                                    plan,
                                    root,
                                    parent,
                                    definition.variable().get() as usize,
                                    definition.value(),
                                    frame - 1,
                                    &physical.definitions,
                                    destination.place(),
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
                    let location = location.ok_or(Error::Statement(
                        "original reachable source block has no paired canonical entry",
                    ))?;
                    if location.function != physical.coordinate {
                        return Err(Error::Statement(
                            "original source block entry names a foreign canonical function",
                        ));
                    }
                    let cut = cuts.get_mut(location.block as usize).ok_or_else(mismatch)?;
                    if cut.is_some() {
                        return Err(Error::Statement(
                            "distinct original source blocks share one paired canonical entry",
                        ));
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
                            Some(ComponentCut::at(ordinal)),
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
                        | Terminator::Assert { .. }
                        | Terminator::Abort => End::Ordinary,
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
                if matches!(
                    binding.source,
                    SourceValue::Aggregate(_) | SourceValue::Enum(_)
                ) {
                    return Err(Error::Statement(
                        "native aggregate argument requires original ABI component reconstruction",
                    ));
                }
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
                step_hints: program.step_hints(root, out)?,
            });
        }
        if result.instances.len() != total {
            return Err(mismatch());
        }
        check_root_census(inventory.functions(), &seen, out)?;
        result.required = out.budget.storage();
        Ok(result)
    }

    fn binding(
        &mut self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        local: usize,
        value: Value,
        frame: usize,
        physical: &Range<usize>,
        demanded_at: Option<ComponentCut>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Binding> {
        let slots = self.slots;
        let relation = slots.correspondence(out)?;
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
            return Err(Error::Statement(
                "original SSA endpoint differs from its paired source local or type",
            ));
        }
        if matches!(
            semantic
                .types()
                .get(declaration.ty().index() as usize)
                .ok_or_else(mismatch)?
                .shape(),
            super::super::Shape::Enum { .. }
        ) {
            return self.enum_binding(
                root,
                instance,
                local,
                row.function,
                declaration.ty(),
                &endpoint,
                row.locals.start,
                frame,
                physical,
                out,
            );
        }
        if self.is_aggregate_binding(declaration.ty(), &endpoint, out)? {
            return self.aggregate_binding(
                root,
                instance,
                local,
                row.function,
                declaration.ty(),
                &endpoint,
                row.locals.start,
                frame,
                physical,
                demanded_at,
                out,
            );
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
            "paired-scalar-binding",
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
        &mut self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        parent: usize,
        site: SourceBlock,
        boundaries: &Boundaries<'_>,
        frame: usize,
        physical: &Range<usize>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Vec<Binding>> {
        let demands = source_frame_demands::caller_demands(
            self.slots,
            plan,
            root,
            parent,
            site,
            boundaries,
            &mut self.component_demands,
            out,
        )?;
        let mut result = vector(demands.len(), out)?;
        for demand in &demands {
            out.budget.charge_work(1)?;
            let binding = self.binding(
                plan,
                root,
                parent,
                demand.local,
                demand.value,
                frame,
                physical,
                Some(demand.components),
                out,
            )?;
            if demand.components.overwritten.is_some() {
                let SourceValue::Aggregate(index) = binding.source else {
                    return Err(mismatch());
                };
                if self
                    .aggregates
                    .get(index)
                    .ok_or_else(mismatch)?
                    .components
                    .is_empty()
                {
                    return Err(mismatch());
                }
            }
            result.push(binding);
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

    pub(super) fn uses_cut_summary_v96(
        &self,
        slots: &SourceSlots<'_, '_>,
        root: usize,
        pc: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.check_cut_summary_owner_v96(slots, out)?;
        generate::uses_cut_summary(self, root, pc, out)
    }

    pub(super) fn check_cut_summary_owner_v96(
        &self,
        slots: &SourceSlots<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        if !std::ptr::eq(self.slots, slots) {
            return Err(mismatch());
        }
        self.check(out)
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
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
        + h::<Option<SourceStepHintsV85>>()
        + h::<RootArgument>()
        + h::<Instance>()
        + h::<Cut>()
        + h::<Binding>()
        + h::<ComponentCut>()
        + h::<source_frame_demands::SourceDemand>()
        + h::<Vec<source_frame_demands::SourceDemand>>()
        + size_of::<std::slice::Iter<'_, source_frame_demands::SourceDemand>>()
        + h::<AggregateBindingV42>()
        + h::<Vec<AggregateBindingV42>>()
        + h::<Vec<EnumBinding>>()
        + enum_bindings::headers()
        + h::<Vec<Option<ComponentDemandsV42<'_, '_, '_>>>>()
        + aggregate_bindings::headers()
        + object_returns::headers()
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
        + h::<&[fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>]>()
        + h::<&[bool]>()
        + h::<FunctionRole>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<Option<Definition>>()
        + h::<std::result::Result<usize, usize>>()
        + h::<Value>()
        + h::<Variable>()
        + h::<Edge>()
        + 64 * size_of::<usize>()
        + 48 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocation_paired_v36_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_semantic_mir_invocation_step_v85_tests.rs"]
mod step_tests;

#[cfg(test)]
#[path = "original_semantic_mir_aggregate_bindings_v42_tests.rs"]
pub(super) mod aggregate_tests;
