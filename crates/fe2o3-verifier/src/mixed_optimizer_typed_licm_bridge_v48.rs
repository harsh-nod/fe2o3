//! Concrete byte-state LICM relation over the existing checked operation
//! permutation and relocation DAG. Prefix stuttering is a separate obligation.
use super::*;
use crate::mixed_optimizer_refinement_v26::relocation_plan_v28;
use fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38 as Physical;
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV18 as Flow, FormalIndexWidth,
    with_canonical_kir_control_flow_v18 as with_flow,
};
use std::ops::Range;

fn plan_error(error: relocation_plan_v28::Error) -> Error {
    match error {
        relocation_plan_v28::Error::Resource(error) => error.into(),
        relocation_plan_v28::Error::Inventory(error) => error.into(),
        relocation_plan_v28::Error::Flow(error) => error.into(),
        relocation_plan_v28::Error::Mismatch(reason) => Error::Statement(reason),
    }
}

fn available(
    flow: &mut Flow<'_, '_>,
    definition: Definition,
    block: Block,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(2)?;
    Ok(match definition {
        Definition::FunctionArgument { function, .. } => function == block.function,
        Definition::BlockArgument { block: origin, .. } => {
            origin.function == block.function && flow.dominates(origin, block, out.budget)?
        }
        Definition::Result { operation, .. } => {
            operation.block.function == block.function
                && operation.block != block
                && flow.dominates(operation.block, block, out.budget)?
        }
    })
}

fn output_definition(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Licm<'_>,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    out.budget.charge_work(4)?;
    let original = &input.definitions()[definition];
    let coordinate = match original.coordinate {
        Definition::Result { operation, result } => {
            let ordinal = operation_index(input, operation)?;
            let row = pair.origins().get(ordinal).ok_or_else(mismatch)?;
            if row.input != operation {
                return Err(mismatch());
            }
            Definition::Result {
                operation: row.output,
                result,
            }
        }
        coordinate => coordinate,
    };
    let target = definition_index(output, coordinate)?;
    if output.definitions()[target].ty != original.ty {
        return Err(mismatch());
    }
    Ok(target)
}

fn relation(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Licm<'_>,
    function: usize,
    width: FormalIndexWidth,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &input.functions()[function];
    emit!(
        out,
        "open spec fn typed_licm_related_{function}_v48(before: MemoryStateV30, after: MemoryStateV30, little_endian: bool) -> bool {{\n before.valid && after.valid && before.pc == after.pc\n && before.memory == after.memory && before.generations == after.generations && before.frames == after.frames\n && before.values.len() == {} && after.values.len() == {}\n && byte_state_memory_well_formed_v30(before) && byte_state_memory_well_formed_v30(after)\n && typed_allocation_environment_1_v48(before) == typed_allocation_environment_2_v48(after)\n && (if before.pc == -1 || before.pc == -2 {{ true }} else {{\n",
        input.definitions().len(),
        output.definitions().len()
    );
    // Reuse the established streaming-writer pattern: a single checked flow
    // scope serves every definition/cut in this function.
    let text = std::mem::take(&mut out.text);
    let failure = out.failure.take();
    let (text, failure) = with_flow(
        input.owner(),
        row.coordinate,
        Default::default(),
        out.budget,
        |flow, budget| {
            let mut writer = Writer {
                text,
                budget,
                failure,
            };
            let out = &mut writer;
            for block in row.blocks.clone() {
                out.budget.charge_work(2)?;
                let coordinate = input.blocks()[block].coordinate;
                if coordinate != output.blocks()[block].coordinate {
                    return Err(mismatch());
                }
                emit!(
                    out,
                    " if before.pc == {block} {{ typed_relocation_cut_0_v48({block}, before, after, little_endian)"
                );
                if !flow.is_reachable(coordinate, out.budget)? {
                    emit!(out, " && false }} else\n");
                    continue;
                }
                for definition in row.definitions.clone() {
                    let original = &input.definitions()[definition];
                    let target = output_definition(input, output, pair, definition, out)?;
                    let present = available(flow, original.coordinate, coordinate, out)?;
                    let target_present = available(
                        flow,
                        output.definitions()[target].coordinate,
                        coordinate,
                        out,
                    )?;
                    let moved = match original.coordinate {
                        Definition::Result { operation, .. } => pair.origins()
                            [operation_index(input, operation)?]
                        .hoist
                        .is_some(),
                        _ => false,
                    };
                    if (!moved && present != target_present) || (present && !target_present) {
                        return Err(Error::Statement("typed LICM exact definition availability"));
                    }
                    if !present {
                        continue;
                    }
                    emit!(
                        out,
                        "\n && before.values[{definition}] == after.values[{target}] && ({{ let compared = before.values[{definition}]; "
                    );
                    super::super::byte_function_v30::emit_value_type(
                        original.ty,
                        width,
                        "compared",
                        out,
                    )?;
                    emit!(out, " }})");
                }
                emit!(out, " }} else\n");
            }
            Ok::<_, Error>((writer.text, writer.failure))
        },
    )?;
    out.text = text;
    out.failure = failure;
    emit!(out, " {{ false }} }})\n}}\n");
    Ok(())
}

fn block_law(
    function: usize,
    block: usize,
    input_namespace: usize,
    output_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    emit!(
        out,
        "proof fn typed_licm_block_{block}_v48(before: MemoryStateV30, after: MemoryStateV30, little_endian: bool)\n requires typed_licm_related_{function}_v48(before, after, little_endian), before.pc == {block}, byte_block_{input_namespace}_{block}_v30(before, little_endian).state.valid,\n ensures typed_licm_related_{function}_v48(byte_block_{input_namespace}_{block}_v30(before, little_endian).state, byte_block_{output_namespace}_{block}_v30(after, little_endian).state, little_endian),\n typed_licm_block_event_1_v48(byte_block_{input_namespace}_{block}_v30(before, little_endian)).observations.is_some(),\n typed_licm_block_event_2_v48(byte_block_{output_namespace}_{block}_v30(after, little_endian)).observations.is_some(),\n typed_licm_block_event_1_v48(byte_block_{input_namespace}_{block}_v30(before, little_endian)) == typed_licm_block_event_2_v48(byte_block_{output_namespace}_{block}_v30(after, little_endian)),\n{{\n let left = byte_block_{input_namespace}_{block}_v30(before, little_endian);\n let right = byte_block_{output_namespace}_{block}_v30(after, little_endian);\n assert(typed_licm_related_{function}_v48(left.state, right.state, little_endian));\n assert(typed_licm_block_event_1_v48(left).observations.is_some());\n assert(typed_licm_block_event_2_v48(right).observations.is_some());\n assert(typed_licm_block_event_1_v48(left) == typed_licm_block_event_2_v48(right));\n}}\n"
    );
    Ok(())
}

fn function_laws(
    input: &Inventory<'_>,
    function: usize,
    input_namespace: usize,
    output_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(6)?;
    for (side, namespace) in [(1, input_namespace), (2, output_namespace)] {
        emit!(
            out,
            "open spec fn typed_licm_step_{side}_{function}_v48(state: MemoryStateV30, little_endian: bool) -> CfgStepV26<MemoryStateV30, TypedLicmBlockEventV48> {{\n if !state.valid || state.pc < 0 {{ CfgStepV26 {{ state, events: seq![], halted: true }} }} else {{ let result = byte_block_step_{namespace}_v30(state, little_endian); CfgStepV26 {{ state: result.state, events: seq![typed_licm_block_event_{side}_v48(result)], halted: !result.state.valid || result.state.pc < 0 }} }}\n}}\n"
        );
    }
    emit!(
        out,
        "open spec fn typed_licm_input_defined_{function}_v48(state: MemoryStateV30, little_endian: bool, fuel: nat) -> bool\n decreases fuel,\n{{ state.valid && (fuel == 0 || state.pc < 0 || typed_licm_input_defined_{function}_v48(typed_licm_step_1_{function}_v48(state, little_endian).state, little_endian, (fuel - 1) as nat)) }}\nproof fn typed_licm_step_{function}_v48(before: MemoryStateV30, after: MemoryStateV30, little_endian: bool)\n requires typed_licm_related_{function}_v48(before, after, little_endian), typed_licm_input_defined_{function}_v48(before, little_endian, 1),\n ensures typed_licm_related_{function}_v48(typed_licm_step_1_{function}_v48(before, little_endian).state, typed_licm_step_2_{function}_v48(after, little_endian).state, little_endian),\n typed_licm_step_1_{function}_v48(before, little_endian).events == typed_licm_step_2_{function}_v48(after, little_endian).events,\n typed_licm_step_1_{function}_v48(before, little_endian).halted == typed_licm_step_2_{function}_v48(after, little_endian).halted,\n{{ if before.pc >= 0 {{\n"
    );
    for block in input.functions()[function].blocks.clone() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " if before.pc == {block} {{ typed_licm_block_{block}_v48(before, after, little_endian); }} else\n"
        );
    }
    emit!(
        out,
        " {{ assert(false); }}\n }}\n}}\nproof fn typed_licm_trace_{function}_v48(before: MemoryStateV30, after: MemoryStateV30, little_endian: bool, fuel: nat)\n requires typed_licm_related_{function}_v48(before, after, little_endian), typed_licm_input_defined_{function}_v48(before, little_endian, fuel),\n ensures cfg_trace_v26(|s| typed_licm_step_1_{function}_v48(s, little_endian), before, fuel).events == cfg_trace_v26(|s| typed_licm_step_2_{function}_v48(s, little_endian), after, fuel).events,\n cfg_trace_v26(|s| typed_licm_step_1_{function}_v48(s, little_endian), before, fuel).halted == cfg_trace_v26(|s| typed_licm_step_2_{function}_v48(s, little_endian), after, fuel).halted,\n typed_licm_related_{function}_v48(cfg_trace_v26(|s| typed_licm_step_1_{function}_v48(s, little_endian), before, fuel).state, cfg_trace_v26(|s| typed_licm_step_2_{function}_v48(s, little_endian), after, fuel).state, little_endian),\n decreases fuel,\n{{ if fuel > 0 {{\n typed_licm_step_{function}_v48(before, after, little_endian);\n let left = typed_licm_step_1_{function}_v48(before, little_endian);\n let right = typed_licm_step_2_{function}_v48(after, little_endian);\n if !left.halted {{ typed_licm_trace_{function}_v48(left.state, right.state, little_endian, (fuel - 1) as nat); }}\n }}\n}}\n"
    );
    Ok(())
}

impl<'a, 'owner, 'rows, R: ByteAllocationResolverV30> AllocationBridgeV48<'a, 'owner, 'rows, R> {
    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_typed_licm_v48(
        &self,
        input_physical: &Physical<'_, 'owner>,
        output_physical: &Physical<'_, 'owner>,
        input_contracts: &super::super::target_view_contracts_v38::TargetByteViewContractsV38<
            '_,
            'owner,
        >,
        output_contracts: &super::super::target_view_contracts_v38::TargetByteViewContractsV38<
            '_,
            'owner,
        >,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let input = self.prefix.output();
        let output = self.output;
        input_contracts.check_owner_width_v39(input.owner(), width, out)?;
        output_contracts.check_owner_width_v39(output.owner(), width, out)?;
        if input.functions().len() != output.functions().len()
            || input.blocks().len() != output.blocks().len()
            || !input_physical.is_for(input)
            || !output_physical.is_for(output)
        {
            return Err(mismatch());
        }
        let floor = out.budget.storage();
        let result: Result<()> = (|| {
            out.budget.reserve_storage(size_of::<(
                [usize; 8],
                [&(); 12],
                [Range<usize>; 2],
                [Result<()>; 2],
                Writer<'_, '_>,
            )>())?;
            let expressions =
                relocation_plan_v28::build(self.licm, out.budget).map_err(plan_error)?;
            let before_allocations = self.view(AllocationSideV48::Prefix, out)?;
            let after_allocations = self.view(AllocationSideV48::Relocated, out)?;
            emit!(out, "mod typed_licm_v48 {{\n use super::*;\n");
            self.emit_transport([0, 1, 2], out)?;
            emit!(
                out,
                "struct TypedLicmBlockEventV48 {{ observations: Option<Seq<TypedMemoryObservationV48>>, returned: Seq<MemoryValueV30>, terminal: int }}\n"
            );
            for side in [1, 2] {
                emit!(
                    out,
                    "open spec fn typed_licm_block_event_{side}_v48(result: MemoryBlockResultV30) -> TypedLicmBlockEventV48 {{ TypedLicmBlockEventV48 {{ observations: typed_observations_{side}_v48(result.observations), returned: if result.state.valid {{ result.returned }} else {{ seq![] }}, terminal: if !result.state.valid {{ -3 }} else if result.state.pc == -2 {{ -2 }} else if result.state.pc == -1 {{ -1 }} else {{ 0 }} }} }}\n"
                );
            }
            let output_base = input.functions().len();
            for (function, row) in input.functions().iter().enumerate() {
                out.budget.charge_work(2)?;
                if row.function.body.is_none() {
                    continue;
                }
                let function_floor = out.budget.storage();
                let before = super::super::byte_function_v30::ByteFunctionV30::derive(
                    input,
                    input_physical,
                    row.coordinate,
                    super::super::byte_function_v30::ByteInterpretationContextV39::classified(
                        width,
                        input_contracts,
                        registry_namespace,
                    ),
                    &before_allocations,
                    out,
                )?;
                let after = super::super::byte_function_v30::ByteFunctionV30::derive(
                    output,
                    output_physical,
                    row.coordinate,
                    super::super::byte_function_v30::ByteInterpretationContextV39::classified(
                        width,
                        output_contracts,
                        registry_namespace,
                    ),
                    &after_allocations,
                    out,
                )?;
                before.emit(function, out)?;
                after.emit(
                    output_base
                        .checked_add(function)
                        .ok_or(Resource::Arithmetic)?,
                    out,
                )?;
                drop((before, after));
                out.budget.release_storage(
                    out.budget
                        .storage()
                        .checked_sub(function_floor)
                        .ok_or(Resource::Accounting)?,
                )?;
            }
            expressions.emit_typed_expressions_v48(self.licm, input, output, 0, 0, out)?;
            for (function, row) in input.functions().iter().enumerate() {
                out.budget.charge_work(1)?;
                if row.function.body.is_none() {
                    continue;
                }
                let output_namespace = output_base
                    .checked_add(function)
                    .ok_or(Resource::Arithmetic)?;
                relation(input, output, self.licm, function, width, out)?;
                for block in row.blocks.clone() {
                    block_law(function, block, function, output_namespace, out)?;
                }
                function_laws(input, function, function, output_namespace, out)?;
            }
            emit!(out, "}}\n");
            drop((before_allocations, after_allocations));
            expressions.discard(out.budget).map_err(plan_error)?;
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        result?;
        self.check(out)
    }
}
