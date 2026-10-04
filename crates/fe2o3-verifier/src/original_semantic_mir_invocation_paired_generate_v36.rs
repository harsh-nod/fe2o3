//! Mandatory byte/value/control obligations over actual block execution.
//! No proof assumption is constructed from a locator or a successful census.

use super::super::super::super::byte_function_v30::{emit_pointer_value_type, emit_value_type};
use super::*;
use std::fmt::Write as _;

#[path = "original_semantic_mir_source_conservation_generate_v81.rs"]
mod conservation;

#[path = "original_semantic_mir_invocation_step_generate_v85.rs"]
mod step;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn emit(model: &PairedInvocations<'_, '_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    emit!(out, "{PAIRED_V36}");
    emit!(out, "{}", conservation::SHARED);
    emit!(
        out,
        "{}",
        include_str!("original_semantic_mir_enum_bindings_v49.vrs")
    );
    for (root, row) in model.roots.iter().enumerate() {
        out.budget.charge_work(1)?;
        control(model, root, row, out)?;
        related(model, root, row, out)?;
        observed(model, root, row, out)?;
        initial(model, root, row, out)?;
        if let Some(hints) = &row.step_hints {
            conservation::emit(root, &hints.fuels, out)?;
            step::emit(model, root, row, hints, out)?;
        }
        proofs(root, row, out)?;
    }
    Ok(())
}

fn control(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "spec fn invocation_byte_cut_{root}_v36(pc: int) -> bool {{ false"
    );
    for (block, cut) in row.cuts.iter().enumerate() {
        out.budget.charge_work(1)?;
        if cut.is_some() {
            emit!(out, " || pc == {}", add(row.blocks.start, block)?);
        }
    }
    emit!(
        out,
        " }}\nspec fn invocation_byte_follow_{root}_v36(target: MemoryStateV30, fuel: nat) -> InvocationActualBoundaryV36\n decreases fuel\n{{\n if !target.valid || target.pc < 0 || invocation_byte_cut_{root}_v36(target.pc) {{ invocation_actual_boundary_empty_v36(target) }}\n else if fuel == 0 || target.pc < {} || {} <= target.pc {{ invocation_actual_boundary_empty_v36(MemoryStateV30 {{ valid: false, ..target }}) }}\n else {{ let head = byte_block_step_{root}_v30(target, invocation_runtime_little_endian_v36());\n let tail = invocation_byte_follow_{root}_v36(head.state, (fuel - 1) as nat);\n invocation_actual_boundary_join_v36(target.pc, head, tail) }}\n}}\nspec fn invocation_byte_boundary_{root}_v36(target: MemoryStateV30) -> InvocationActualBoundaryV36 {{\n if target.pc < 0 {{ invocation_actual_boundary_empty_v36(target) }}\n else if !target.valid || !invocation_byte_cut_{root}_v36(target.pc) {{ invocation_actual_boundary_empty_v36(MemoryStateV30 {{ valid: false, ..target }}) }}\n else {{ let head = byte_block_step_{root}_v30(target, invocation_runtime_little_endian_v36());\n let tail = invocation_byte_follow_{root}_v36(head.state, {}nat);\n invocation_actual_boundary_join_v36(target.pc, head, tail) }}\n}}\n",
        row.blocks.start,
        row.blocks.end,
        row.blocks.len()
    );
    emit!(
        out,
        "spec fn invocation_paired_source_step_{root}_v36(source: InvocationSourceByteStateV36) -> CfgStepV26<InvocationSourceByteStateV36, InvocationSourceEffectObservationV39> {{\n if source.machine.pc < 0 {{ CfgStepV26 {{ state: source, events: seq![], halted: true }} }} else {{\n let next = invocation_source_block_runtime_{root}_v36(source);\n CfgStepV26 {{ state: next.source, events: invocation_source_observations_v39(next, invocation_runtime_little_endian_v36()), halted: next.source.machine.pc < 0 }} }}\n}}\nspec fn invocation_paired_actual_step_{root}_v36(target: MemoryStateV30) -> CfgStepV26<MemoryStateV30, MemoryOperationObservationV30> {{\n let next = invocation_byte_boundary_{root}_v36(target);\n CfgStepV26 {{ state: next.state, events: invocation_actual_observations_v39(next.observations), halted: next.state.pc < 0 }}\n}}\n"
    );
    emit!(
        out,
        "spec fn invocation_paired_observations_related_{root}_v39(source: Seq<InvocationSourceEffectObservationV39>, target: Seq<MemoryOperationObservationV30>) -> bool {{\n source.len() == target.len() && (forall|i: int| 0 <= i < source.len() ==> invocation_source_byte_map_valid_{root}_v36(source[i].before, target[i].before) && invocation_source_byte_map_valid_{root}_v36(source[i].after, target[i].after) && invocation_observed_effect_related_v39(source[i], target[i], invocation_source_byte_map_{root}_v36(source[i].before, target[i].before), invocation_source_byte_map_{root}_v36(source[i].after, target[i].after)))\n}}\n"
    );
    emit!(
        out,
        "proof fn invocation_paired_observations_append_{root}_v39(a: Seq<InvocationSourceEffectObservationV39>, b: Seq<MemoryOperationObservationV30>, c: Seq<InvocationSourceEffectObservationV39>, d: Seq<MemoryOperationObservationV30>)\n requires invocation_paired_observations_related_{root}_v39(a, b), invocation_paired_observations_related_{root}_v39(c, d),\n ensures invocation_paired_observations_related_{root}_v39(a + c, b + d),\n{{ }}\n"
    );
    let _ = model;
    Ok(())
}

fn actual_value(
    model: &PairedInvocations<'_, '_, '_>,
    binding: &Binding,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    match binding.definition {
        Some(definition) => {
            emit!(out, " let actual = target.values[{definition}]; ");
            let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
            emit_value_type(
                inventory
                    .definitions()
                    .get(definition)
                    .ok_or_else(mismatch)?
                    .ty,
                model.width,
                "actual",
                out,
            )?;
            emit!(
                out,
                " && invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory)"
            );
        }
        None => emit!(out, " original == MemoryValueV30::Unit"),
    }
    Ok(())
}

fn binding(
    model: &PairedInvocations<'_, '_, '_>,
    row: &Binding,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if let SourceValue::Aggregate(index) = row.source {
        return aggregate_binding(model, index, out);
    }
    if let SourceValue::Enum(index) = row.source {
        return enum_binding(model, index, out);
    }
    emit!(out, " && ({{ ");
    match row.source {
        SourceValue::Local(local) => emit!(out, "let original = source.machine.values[{local}]; "),
        SourceValue::Slot { descriptor, bits } => {
            let frame = row.frame;
            let width = if bits == 1 { 1 } else { bits / 8 };
            emit!(
                out,
                "source.slots.contains_key({descriptor}) && {frame} < source.machine.frames.active.len() && ({{ let slot = invocation_source_slot_{descriptor}_v36(); let pointer = source.slots[{descriptor}];\n match pointer.allocation {{ MemoryAllocationV30::Private {{ owner, invocation, site, .. }} => owner == source.machine.frames.active[{frame}].owner && invocation == source.machine.frames.active[{frame}].invocation && owner == slot.owner && site == slot.site, _ => false }}\n && invocation_source_read_enabled_v36(source.machine, pointer, {width}, slot.alignment) && ({{ let original = MemoryValueV30::Scalar(byte_load_v30(source.machine.memory, pointer, {width}, invocation_runtime_little_endian_v36())); invocation_source_byte_value_typed_v36(original, {bits}) && ({{ "
            );
        }
        SourceValue::Aggregate(_) | SourceValue::Enum(_) | SourceValue::ReturnSnapshot => {
            return Err(mismatch());
        }
    }
    actual_value(model, row, out)?;
    if matches!(row.source, SourceValue::Slot { .. }) {
        emit!(out, " }}) }}) }})");
    }
    match (row.source, row.logical) {
        (_, LogicalBinding::Plain) => (),
        (SourceValue::Local(local), LogicalBinding::DescriptorReference(recipe)) => {
            emit!(
                out,
                " && invocation_source_descriptor_reference_current_v51(source, {local}, "
            );
            recipe.emit(out)?;
            emit!(out, ")");
        }
        (SourceValue::Local(local), LogicalBinding::Witness { source_type }) => {
            emit!(
                out,
                " && invocation_source_witness_current_v38(source, {local}, {source_type})"
            );
        }
        (
            SourceValue::Local(local),
            LogicalBinding::Reference {
                source_type,
                origin,
                generation,
                instance,
                block,
                statement,
            },
        ) => {
            emit!(
                out,
                " && invocation_source_reference_current_v38(source, {local}, {source_type}) && ({{ let reference = source.logical.references[{local}]; reference.origin == {origin} && reference.origin_generation == {generation} && reference.borrow_instance == {instance} && reference.borrow_block == {block} && reference.borrow_statement == {statement} }})"
            );
        }
        (
            SourceValue::Slot { .. }
            | SourceValue::Aggregate(_)
            | SourceValue::Enum(_)
            | SourceValue::ReturnSnapshot,
            _,
        ) => {
            return Err(mismatch());
        }
    }
    emit!(out, " }})");
    Ok(())
}

fn enum_binding(
    model: &PairedInvocations<'_, '_, '_>,
    index: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = model.enums.get(index).ok_or_else(mismatch)?;
    emit!(
        out,
        " && (match invocation_source_enum_local_v47(source, {}, {}) {{ Some(value) => {{ ",
        row.local,
        row.source_type.index()
    );
    enum_payload_binding(model, index, out)?;
    emit!(out, " }}, None => false }})");
    Ok(())
}

fn enum_payload_binding(
    model: &PairedInvocations<'_, '_, '_>,
    index: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::enum_bindings::FieldCarrier;
    let row = model.enums.get(index).ok_or_else(mismatch)?;
    if let Some(variant) = row.known_variant {
        emit!(out, "value.variant == {variant} && ");
    }
    emit!(out, "(");
    for variant in &row.variants {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if value.variant == {} {{ target.values[{}] == MemoryValueV30::Scalar({})",
            variant.ordinal,
            row.tag,
            variant.discriminant
        );
        for (field, carrier) in variant.fields.iter().enumerate() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " && (!value.fields.contains_key({field}) || ({{ let original = value.fields[{field}]; "
            );
            match carrier {
                FieldCarrier::Missing => emit!(out, "false"),
                FieldCarrier::Unit => emit!(out, "original == MemoryValueV30::Unit"),
                FieldCarrier::Value(definition) => actual_value(
                    model,
                    &Binding {
                        source: SourceValue::Local(row.local),
                        logical: LogicalBinding::Plain,
                        definition: Some(*definition),
                        frame: 0,
                    },
                    out,
                )?,
                FieldCarrier::Spill {
                    row: spill,
                    width,
                    alignment,
                } => {
                    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
                    let fe2o3_kernel_ir::Type::Pointer(pointer) = inventory
                        .definitions()
                        .get(spill.definition)
                        .ok_or_else(mismatch)?
                        .ty
                    else {
                        return Err(mismatch());
                    };
                    let storage = enum_bindings::spill_pointer_v57(
                        &pointer.pointee,
                        &inventory.owner().module().storage_layouts,
                        out,
                    )?;
                    let pointer_payload = storage.is_some()
                        || matches!(pointer.pointee.as_ref(), fe2o3_kernel_ir::Type::Pointer(_));
                    let operation = spill.operation;
                    emit!(
                        out,
                        "match invocation_enum_spill_read_v49(target, {}, {}, MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, {width}, {alignment}, {pointer_payload}, invocation_runtime_little_endian_v36()) {{ Some(actual) => {{ ",
                        spill.definition,
                        spill.physical_owner,
                        operation.block.function.0,
                        operation.block.block,
                        operation.operation
                    );
                    if let Some(storage) = storage {
                        emit_pointer_value_type(storage.value_space, model.width, "actual", out)?;
                    } else {
                        emit_value_type(&pointer.pointee, model.width, "actual", out)?;
                    }
                    emit!(
                        out,
                        " && invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory) }}, None => false }}"
                    );
                }
            }
            emit!(out, " }}))");
        }
        emit!(out, " }} else ");
    }
    emit!(out, "{{ false }})");
    Ok(())
}

fn aggregate_binding(
    model: &PairedInvocations<'_, '_, '_>,
    index: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let aggregate = model.aggregates.get(index).ok_or_else(mismatch)?;
    for component in &aggregate.components {
        out.budget.charge_work(1)?;
        let leaf = model
            .slots
            .aggregate_leaf(aggregate.source_type, component.leaf, out)?;
        let path = leaf.path(out)?;
        emit!(
            out,
            " && (match invocation_source_aggregate_leaf_v42(source, {}, {}, seq![",
            aggregate.local,
            aggregate.source_type.index()
        );
        for field in path {
            out.budget.charge_work(1)?;
            emit!(out, "{field}int,");
        }
        emit!(out, "]) {{ Some(original) => {{ ");
        actual_value(
            model,
            &Binding {
                source: SourceValue::Local(aggregate.local),
                logical: LogicalBinding::Plain,
                definition: component.definition,
                frame: 0,
            },
            out,
        )?;
        emit!(out, " }}, None => false }})");
    }
    Ok(())
}

fn snapshot_binding(
    model: &PairedInvocations<'_, '_, '_>,
    row: &Binding,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    if let LogicalBinding::DescriptorReference(recipe) = row.logical {
        emit!(
            out,
            "match original {{ InvocationSourceValueV42::Descriptor(snapshot) => invocation_source_descriptor_snapshot_current_v53(source, snapshot, "
        );
        recipe.emit(out)?;
        emit!(out, ") && ({{ let original = snapshot.value; ");
        actual_value(model, row, out)?;
        emit!(out, " }}), _ => false }}");
        return Ok(());
    }
    if let SourceValue::Enum(index) = row.source {
        let binding = model.enums.get(index).ok_or_else(mismatch)?;
        emit!(
            out,
            "match original {{ InvocationSourceValueV42::Enum(value) => value.source_type == {} && invocation_source_enum_snapshot_current_v50(source, value, invocation_runtime_little_endian_v36()) && (",
            binding.source_type.index()
        );
        enum_payload_binding(model, index, out)?;
        emit!(out, "), _ => false }}");
        return Ok(());
    }
    if let SourceValue::Aggregate(index) = row.source {
        let aggregate = model.aggregates.get(index).ok_or_else(mismatch)?;
        emit!(
            out,
            "match original {{ InvocationSourceValueV42::Aggregate(snapshot) => snapshot.source_type == {} && invocation_source_aggregate_complete_v42(snapshot)",
            aggregate.source_type.index()
        );
        for component in &aggregate.components {
            out.budget.charge_work(1)?;
            let leaf = model
                .slots
                .aggregate_leaf(aggregate.source_type, component.leaf, out)?;
            emit!(out, " && ({{ let path = seq![");
            for field in leaf.path(out)? {
                out.budget.charge_work(1)?;
                emit!(out, "{field}int,");
            }
            emit!(
                out,
                "]; snapshot.leaves.contains_key(path) && ({{ let original = snapshot.leaves[path]; "
            );
            actual_value(
                model,
                &Binding {
                    source: SourceValue::Local(aggregate.local),
                    logical: LogicalBinding::Plain,
                    definition: component.definition,
                    frame: row.frame,
                },
                out,
            )?;
            emit!(out, " }}) }})");
        }
        emit!(out, ", _ => false }}");
    } else {
        emit!(
            out,
            "match original {{ InvocationSourceValueV42::Carrier(original) => {{ "
        );
        actual_value(model, row, out)?;
        emit!(out, " }}, _ => false }}");
    }
    Ok(())
}

fn related(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "spec fn invocation_paired_related_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30) -> bool {{\n"
    );
    related_body(model, root, row, true, out)?;
    emit!(out, "}}\n");
    if row.step_hints.is_some() {
        emit!(
            out,
            "spec fn invocation_paired_residual_{root}_v85(source: InvocationSourceByteStateV36, target: MemoryStateV30) -> bool {{\n"
        );
        related_body(model, root, row, false, out)?;
        emit!(out, "}}\n");
    }
    Ok(())
}

fn related_body(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    storage: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        " source.machine.valid && target.valid && source.machine.values.len() == {} && target.values.len() == {}\n && (match source.machine.frames.execution {{ Some(execution) => invocation_runtime_execution_{root}_v37(execution), None => false }})\n",
        model.locals,
        model.definitions
    );
    if storage {
        emit!(
            out,
            " && invocation_source_byte_storage_related_{root}_v36(source, target)\n"
        );
    }
    emit!(
        out,
        " && ({{ let map = invocation_source_byte_map_{root}_v36(source, target);\n if source.machine.pc < 0 {{ (source.machine.pc == -1 && target.pc == -1 && source.machine.frames.active.len() == 0) || (source.machine.pc == -2 && target.pc == -2) }} else\n"
    );
    for (block, cut) in row.cuts.iter().enumerate() {
        out.budget.charge_work(1)?;
        let Some(cut) = cut else {
            continue;
        };
        let instance = model
            .instances
            .get(cut.instance)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        emit!(
            out,
            " if source.machine.pc == {} {{ target.pc == {} && source.machine.frames.active.len() == {}",
            cut.source,
            add(row.blocks.start, block)?,
            instance.owners.len()
        );
        for (frame, owner) in instance.owners.iter().enumerate() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " && source.machine.frames.active[{frame}].owner == {owner}"
            );
        }
        for value in cut.live.iter().chain(&instance.suspended) {
            binding(model, value, out)?;
        }
        emit!(out, " }} else\n");
    }
    emit!(out, " {{ false }} }})\n");
    Ok(())
}

fn observed(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "spec fn invocation_paired_control_values_{root}_v36(before: InvocationSourceByteStateV36, source_result: InvocationSourceBlockResultV36, target_result: InvocationActualBoundaryV36) -> bool {{\n let source = source_result.source; let target = target_result.state;\n let map = invocation_source_byte_map_{root}_v36(source, target);\n target.values.len() == {} && (\n",
        model.definitions
    );
    for cut in row.cuts.iter().flatten() {
        out.budget.charge_work(1)?;
        let instance = model
            .instances
            .get(cut.instance)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        emit!(out, " if before.machine.pc == {} {{ ", cut.source);
        match cut.end {
            End::Ordinary => emit!(
                out,
                " source_result.returned.is_none() && target_result.returned.len() == 0"
            ),
            End::Call(child) => {
                let child = model
                    .instances
                    .get(child)
                    .and_then(Option::as_ref)
                    .ok_or_else(mismatch)?;
                emit!(
                    out,
                    " source_result.returned.is_none() && target_result.returned.len() == 0 && source_result.operands.len() == {}",
                    child.arguments.len()
                );
                for (argument, row) in child.arguments.iter().enumerate() {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        " && ({{ let original = source_result.operands[{argument}].value; "
                    );
                    snapshot_binding(model, row, out)?;
                    emit!(out, " }})");
                }
            }
            End::Return => {
                emit!(
                    out,
                    " match source_result.returned {{ Some(original) => {{ "
                );
                if let Some(returned) = &instance.returned {
                    emit!(out, " target_result.returned.len() == 0 && ({{ ");
                    snapshot_binding(model, returned, out)?;
                    emit!(out, " }})");
                } else {
                    emit!(
                        out,
                        " match original {{ InvocationSourceValueV42::Carrier(original) => if original == MemoryValueV30::Unit {{ target_result.returned.len() == 0 }} else {{ target_result.returned.len() == 1 && invocation_value_related_v36(original, target_result.returned[0], map, source.machine.memory, target.memory) }}, _ => false }}"
                    );
                }
                emit!(out, " }}, None => false }}");
            }
        }
        emit!(out, " }} else\n");
    }
    emit!(out, " {{ false }} )\n}}\n");
    Ok(())
}

fn initial(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let entry = model
        .instances
        .get(row.instances.start)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    emit!(
        out,
        "spec fn invocation_paired_arguments_{root}_v36(arguments: Seq<MemoryValueV30>) -> bool {{ arguments.len() == {}",
        entry.arguments.len()
    );
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    for (argument, binding) in entry.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " && invocation_source_external_argument_v36(arguments[{argument}]) && ({{ let value = arguments[{argument}]; "
        );
        match binding.definition {
            Some(definition) => emit_value_type(
                inventory.definitions()[definition].ty,
                model.width,
                "value",
                out,
            )?,
            None => emit!(out, " value == MemoryValueV30::Unit"),
        }
        emit!(out, " }})");
    }
    emit!(
        out,
        " }}\nspec fn invocation_paired_native_inputs_{root}_v38(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> bool {{\n invocation_runtime_execution_{root}_v37(execution) && invocation_paired_arguments_{root}_v36(arguments) && byte_memory_well_formed_v30(external) && byte_native_view_inputs_v38(external, arguments) && invocation_native_provenance_v39(external, arguments)\n}}\nproof fn invocation_paired_source_ready_{root}_v38(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37)\n requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution),\n ensures invocation_source_initial_runtime_{root}_v36(arguments, external, execution).machine.valid,\n{{\n let entered_memory = ByteMemoryV30 {{ view_contracts: invocation_source_view_contracts_0_v39(invocation_runtime_little_endian_v36()), ..external }};\n invocation_native_initial_memory_invariants_v77(entered_memory, arguments, byte_root_frame_with_execution_v37({}, execution));\n}}\nspec fn invocation_paired_raw_initial_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> MemoryStateV30 {{\n let values = Seq::new({}nat, |i: int| MemoryValueV30::Undefined)",
        row.owner,
        model.definitions
    );
    for parameter in &row.parameters {
        out.budget.charge_work(1)?;
        emit!(
            out,
            ".update({}, arguments[{}])",
            parameter.definition,
            parameter.source
        );
    }
    emit!(
        out,
        ";\n let admitted = invocation_paired_native_inputs_{root}_v38(arguments, external, execution);\n let memory = if admitted {{ ByteMemoryV30 {{ view_contracts: byte_target_view_contracts_1_v38(invocation_runtime_little_endian_v36()), ..external }} }} else {{ external }};\n MemoryStateV30 {{ pc: {}, values, memory, generations: Map::empty(), frames: byte_root_frame_with_execution_v37({}, execution), valid: admitted }}\n}}\nspec fn invocation_paired_ready_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> InvocationActualBoundaryV36 {{ invocation_byte_follow_{root}_v36(invocation_paired_raw_initial_{root}_v36(arguments, external, execution), {}nat) }}\nproof fn invocation_paired_initial_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37)\n requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution),\n ensures invocation_paired_related_{root}_v36(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), invocation_paired_ready_{root}_v36(arguments, external, execution).state),\n invocation_actual_observations_v39(invocation_paired_ready_{root}_v36(arguments, external, execution).observations).len() == 0,\n{{\n invocation_paired_source_ready_{root}_v38(arguments, external, execution);\n let source_memory = ByteMemoryV30 {{ view_contracts: invocation_source_view_contracts_0_v39(invocation_runtime_little_endian_v36()), ..external }};\n let target_memory = ByteMemoryV30 {{ view_contracts: byte_target_view_contracts_1_v38(invocation_runtime_little_endian_v36()), ..external }};\n let frames = byte_root_frame_with_execution_v37({}, execution);\n invocation_native_initial_memory_invariants_v77(source_memory, arguments, frames);\n invocation_native_initial_memory_invariants_v77(target_memory, arguments, frames);\n invocation_native_initial_heaps_related_v77(external, arguments);\n}}\n",
        row.blocks.start,
        row.owner,
        row.blocks.len(),
        row.owner
    );
    Ok(())
}

fn proofs(root: usize, row: &Root, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        r#"spec fn invocation_paired_source_defined_{root}_v36(source: InvocationSourceByteStateV36, fuel: nat) -> bool
 decreases fuel
{{ source.machine.valid && (fuel == 0 || source.machine.pc < 0 || invocation_paired_source_defined_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, (fuel - 1) as nat)) }}
#[verifier::spinoff_prover]
proof fn invocation_paired_source_defined_prefix_{root}_v79(source: InvocationSourceByteStateV36, fuel: nat)
 requires invocation_paired_source_defined_{root}_v36(source, fuel), fuel > 0,
 ensures invocation_paired_source_defined_{root}_v36(source, 1),
 !invocation_paired_source_step_{root}_v36(source).halted ==> invocation_paired_source_defined_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, (fuel - 1) as nat),
{{
 hide(invocation_paired_source_step_{root}_v36);
 hide(invocation_source_block_runtime_{root}_v36);
 hide(invocation_paired_source_defined_{root}_v36);
 reveal_with_fuel(invocation_paired_source_defined_{root}_v36, 2);
 if source.machine.pc < 0 {{
 reveal(invocation_paired_source_step_{root}_v36);
 assert(invocation_paired_source_step_{root}_v36(source).halted);
 }}
}}
proof fn invocation_paired_step_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30)
 requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),
 ensures invocation_paired_related_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),
 invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, invocation_paired_actual_step_{root}_v36(target).events),
 invocation_paired_source_step_{root}_v36(source).halted == invocation_paired_actual_step_{root}_v36(target).halted,
source.machine.pc >= 0 ==> invocation_paired_control_values_{root}_v36(source, invocation_source_block_runtime_{root}_v36(source), invocation_byte_boundary_{root}_v36(target)),
{{
"#
    );
    if row.step_hints.is_some() {
        step::dispatch(root, row, out)?;
    }
    emit!(
        out,
        r#"}}
#[verifier::spinoff_prover]
proof fn invocation_paired_finite_trace_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30, fuel: nat)
 requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, fuel),
 ensures invocation_paired_observations_related_{root}_v39(cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), source, fuel).events, cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), target, fuel).events),
 cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), source, fuel).halted == cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), target, fuel).halted,
 invocation_paired_related_{root}_v36(cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), source, fuel).state, cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), target, fuel).state),
 decreases fuel,
{{
 hide(cfg_trace_v26);
 hide(invocation_paired_related_{root}_v36);
 hide(invocation_paired_observations_related_{root}_v39);
 hide(invocation_paired_source_step_{root}_v36);
 hide(invocation_paired_actual_step_{root}_v36);
 hide(invocation_paired_control_values_{root}_v36);
 hide(invocation_source_block_runtime_{root}_v36);
 hide(invocation_byte_boundary_{root}_v36);
 hide(invocation_paired_source_defined_{root}_v36);
 reveal_with_fuel(cfg_trace_v26, 1);
 if fuel == 0 {{
 assert(invocation_paired_observations_related_{root}_v39(seq![], seq![])) by {{
 reveal(invocation_paired_observations_related_{root}_v39);
 }}
 }} else {{
 invocation_paired_source_defined_prefix_{root}_v79(source, fuel);
 invocation_paired_step_{root}_v36(source, target);
 let original = invocation_paired_source_step_{root}_v36(source); let actual = invocation_paired_actual_step_{root}_v36(target);
 if !original.halted {{ invocation_paired_finite_trace_{root}_v36(original.state, actual.state, (fuel - 1) as nat);
 invocation_paired_observations_append_{root}_v39(original.events, actual.events,
 cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), original.state, (fuel - 1) as nat).events,
 cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), actual.state, (fuel - 1) as nat).events);
 }} }} }}
proof fn invocation_paired_initial_trace_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, fuel: nat)
 requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution), invocation_paired_source_defined_{root}_v36(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel),
 ensures invocation_paired_observations_related_{root}_v39(cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel).events, cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), invocation_paired_ready_{root}_v36(arguments, external, execution).state, fuel).events),
 invocation_paired_related_{root}_v36(cfg_trace_v26(|s: InvocationSourceByteStateV36| invocation_paired_source_step_{root}_v36(s), invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel).state, cfg_trace_v26(|s: MemoryStateV30| invocation_paired_actual_step_{root}_v36(s), invocation_paired_ready_{root}_v36(arguments, external, execution).state, fuel).state),
{{ invocation_paired_initial_{root}_v36(arguments, external, execution); invocation_paired_finite_trace_{root}_v36(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), invocation_paired_ready_{root}_v36(arguments, external, execution).state, fuel); }}
"#
    );
    Ok(())
}

const PAIRED_V36: &str = r#"
struct InvocationActualBoundaryV36 {
    state: MemoryStateV30,
    observations: Seq<MemoryOperationObservationV30>,
    returned: Seq<MemoryValueV30>,
    blocks: Seq<int>,
}
spec fn invocation_actual_boundary_empty_v36(state: MemoryStateV30) -> InvocationActualBoundaryV36 {
    InvocationActualBoundaryV36 { state, observations: seq![], returned: seq![], blocks: seq![] }
}
spec fn invocation_actual_boundary_join_v36(block: int, head: MemoryBlockResultV30,
    tail: InvocationActualBoundaryV36,
) -> InvocationActualBoundaryV36 {
    InvocationActualBoundaryV36 { state: tail.state,
        observations: head.observations + tail.observations,
        returned: if head.state.pc < 0 { head.returned } else { tail.returned },
        blocks: seq![block] + tail.blocks }
}
"#;

fn headers() -> usize {
    size_of::<bool>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 6 * size_of::<Result<()>>()
        + size_of::<Option<usize>>()
}
