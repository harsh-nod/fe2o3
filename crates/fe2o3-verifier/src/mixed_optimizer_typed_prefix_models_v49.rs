//! Checked-prefix concrete byte interpreters and segment-level obligations.
//! These are proof obligations, not an additional optimizer certificate.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV18 as Flow, with_canonical_kir_control_flow_v18 as with_flow,
};
use std::ops::Range;

// A second live CFG scope would invalidate the first scope's exact storage
// floor. Retain only prepaid dominance bits, then query the first CFG alone.
pub(super) struct Dominance<'a, 'owner> {
    inventory: &'a Inventory<'owner>,
    blocks: Range<usize>,
    bits: Vec<usize>,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
    cleanup: Cell<bool>,
}

pub(super) fn with_dominance<T>(
    inventory: &Inventory<'_>,
    function: usize,
    out: &mut Writer<'_, '_>,
    run: impl FnOnce(&Dominance<'_, '_>, &mut Writer<'_, '_>) -> Result<T>,
) -> Result<T> {
    let floor = out.budget.storage();
    let mut retained = None;
    let mut cleanup = true;
    let result = (|| {
        out.budget.reserve_storage(
            size_of::<Dominance<'_, '_>>() + size_of::<([usize; 8], Result<()>)>(),
        )?;
        let row = inventory.functions().get(function).ok_or_else(mismatch)?;
        let count = row.blocks.len();
        let cells = count.checked_mul(count).ok_or(Resource::Arithmetic)?;
        let words = cells.div_ceil(usize::BITS as usize);
        let mut bits = allocate(words, out)?;
        out.budget.charge_work(words)?;
        bits.fill(0);
        retained = Some(
            out.budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        );
        with_flow(
            inventory.owner(),
            row.coordinate,
            Default::default(),
            out.budget,
            |flow, budget| {
                for (definition, original) in row.blocks.clone().enumerate() {
                    for (use_block, actual) in row.blocks.clone().enumerate() {
                        if flow.dominates(
                            inventory.blocks()[original].coordinate,
                            inventory.blocks()[actual].coordinate,
                            budget,
                        )? {
                            budget.charge_work(1)?;
                            let cell = definition * count + use_block;
                            bits[cell / usize::BITS as usize] |=
                                1usize << (cell % usize::BITS as usize);
                        }
                    }
                }
                Ok::<_, Error>(())
            },
        )?;
        let prepared = Dominance {
            inventory,
            blocks: row.blocks.clone(),
            bits,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
            cleanup: Cell::new(true),
        };
        let result = run(&prepared, out);
        let custody = prepared.check(out);
        cleanup = prepared.cleanup.get();
        custody.and(result)
    })();
    // Only construction owns its full accepted delta. Once a CFG or callback
    // runs, release exactly the table's reservation, never their extra credit.
    let retained = match retained {
        Some(retained) => retained,
        None => out
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    };
    if cleanup {
        out.budget.release_storage(retained)?;
    }
    result
}

impl Dominance<'_, '_> {
    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.slot != std::ptr::from_ref(out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            self.failure.set(Some(Resource::Accounting));
            self.cleanup.set(false);
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    pub(super) fn available(
        &self,
        definition: Definition,
        block: usize,
        gap: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.check(out)?;
        out.budget.charge_work(3).map_err(|error| {
            self.failure.set(Some(error));
            Error::Resource(error)
        })?;
        if !self.blocks.contains(&block) {
            return Err(mismatch());
        }
        let coordinate = self.inventory.blocks()[block].coordinate;
        let origin = match definition {
            Definition::FunctionArgument { function, .. } => {
                return Ok(function == coordinate.function);
            }
            Definition::BlockArgument { block: origin, .. } => origin,
            Definition::Result { operation, .. } => {
                if operation.block == coordinate {
                    return Ok(operation_index(self.inventory, operation)? < gap);
                }
                operation.block
            }
        };
        if origin.function != coordinate.function {
            return Ok(false);
        }
        let origin = block_index(self.inventory, origin)?;
        if !self.blocks.contains(&origin) {
            return Err(mismatch());
        }
        let cell = (origin - self.blocks.start) * self.blocks.len() + block - self.blocks.start;
        Ok(self.bits[cell / usize::BITS as usize] & (1usize << (cell % usize::BITS as usize)) != 0)
    }
}

pub(super) fn available(
    inventory: &Inventory<'_>,
    flow: &mut Flow<'_, '_>,
    definition: Definition,
    block: usize,
    gap: usize,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(3)?;
    let coordinate = inventory.blocks()[block].coordinate;
    Ok(match definition {
        Definition::FunctionArgument { function, .. } => function == coordinate.function,
        Definition::BlockArgument { block: origin, .. } => {
            origin.function == coordinate.function
                && flow.dominates(origin, coordinate, out.budget)?
        }
        Definition::Result { operation, .. } => {
            operation.block.function == coordinate.function
                && if operation.block == coordinate {
                    operation_index(inventory, operation)? < gap
                } else {
                    flow.dominates(operation.block, coordinate, out.budget)?
                }
        }
    })
}

impl<'b, 'a, 'owner, 'rows, R: ByteAllocationResolverV30>
    PrefixSegmentsV49<'b, 'a, 'owner, 'rows, R>
{
    fn emit_relation(
        &self,
        function: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let input = self.bridge.prefix.input();
        let output = self.bridge.prefix.output();
        let association = &self.bridge.prefix.rows().functions[function];
        let original_function = association.input.0 as usize;
        let input_row = &input.functions()[original_function];
        let output_row = &output.functions()[function];
        emit!(
            out,
            "open spec fn typed_prefix_related_{function}_v49(before: MemoryStateV30, cursor: TypedPrefixCursorV49, little_endian: bool) -> bool {{\n let after = cursor.micro.state;\n before.valid && after.valid && before.values.len() == {} && after.values.len() == {}\n && before.memory == after.memory && before.generations == after.generations && before.frames == after.frames\n && byte_state_memory_well_formed_v30(before) && typed_prefix_cursor_valid_{function}_v49(cursor, little_endian)\n && typed_allocation_environment_0_v48(before) == typed_allocation_environment_1_v48(after)\n && (if before.pc == -1 || before.pc == -2 {{ cursor.segment == before.pc && after.pc == before.pc }} else {{ before.pc == cursor.segment && (\n",
            input.definitions().len(),
            output.definitions().len()
        );
        with_dominance(output, function, out, |actual_flow, out| {
            let text = std::mem::take(&mut out.text);
            let failure = out.failure.take();
            let (text, failure) = with_flow(
                input.owner(),
                association.input,
                Default::default(),
                out.budget,
                |original_flow, budget| {
                    let mut writer = Writer {
                        text,
                        budget,
                        failure,
                    };
                    let out = &mut writer;
                    for original_block in input_row.blocks.clone() {
                        let Some(segment) = self.segment(original_block, out)? else {
                            continue;
                        };
                        if !output_row.blocks.contains(&segment.output_block) {
                            return Err(mismatch());
                        }
                        emit!(out, " if before.pc == {original_block} {{ true");
                        // Every available target register has its checked original
                        // anchor, or is an independently executed actual constant.
                        for target in output_row.definitions.clone() {
                            if !actual_flow.available(
                                output.definitions()[target].coordinate,
                                segment.output_block,
                                segment.start,
                                out,
                            )? {
                                continue;
                            }
                            let anchor = self.anchors[target];
                            if anchor != NONE {
                                emit!(
                                    out,
                                    "\n && before.values[{anchor}] == after.values[{target}]"
                                );
                            } else {
                                let Definition::Result { operation, .. } =
                                    output.definitions()[target].coordinate
                                else {
                                    return Err(mismatch());
                                };
                                let ordinal = operation_index(output, operation)?;
                                let block = block_index(output, operation.block)?;
                                let namespace = input.functions().len() + function;
                                emit!(
                                    out,
                                    "\n && ({{ let constant = byte_operation_{namespace}_{ordinal}_v30(MemoryStateV30 {{ pc: {block}, ..after }}, little_endian); constant.state.valid && constant.state.values[{target}] == after.values[{target}] && constant.observation.effect == MemoryOperationEffectV30::Pure && constant.state.memory == after.memory && constant.state.generations == after.generations && constant.state.frames == after.frames }})"
                                );
                            }
                            emit!(out, " && ({{ let compared = after.values[{target}]; ");
                            super::super::super::byte_function_v30::emit_value_type(
                                output.definitions()[target].ty,
                                width,
                                "compared",
                                out,
                            )?;
                            emit!(out, " }})");
                        }
                        for original in input_row.definitions.clone() {
                            if !available(
                                input,
                                original_flow,
                                input.definitions()[original].coordinate,
                                original_block,
                                input.blocks()[original_block].operations.start,
                                out,
                            )? {
                                continue;
                            }
                            for descendant in descendants(self.bridge.prefix.rows(), original)? {
                                self.bridge.charge(2, out)?;
                                let target = definition_index(output, descendant.output)?;
                                if actual_flow.available(
                                    descendant.output,
                                    segment.output_block,
                                    segment.start,
                                    out,
                                )? {
                                    emit!(
                                        out,
                                        "\n && before.values[{original}] == after.values[{target}]"
                                    );
                                }
                            }
                        }
                        emit!(out, " }} else\n");
                    }
                    Ok::<_, Error>((writer.text, writer.failure))
                },
            )?;
            out.text = text;
            out.failure = failure;
            Ok(())
        })?;
        emit!(out, " {{ false }}) }})\n}}\n");
        self.check(out)
    }

    fn emit_entry(
        &self,
        function: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let input = self.bridge.prefix.input();
        let output = self.bridge.prefix.output();
        let original_function = self.bridge.prefix.rows().functions[function].input.0 as usize;
        let original_row = &input.functions()[original_function];
        let target_row = &output.functions()[function];
        let original_entry = original_row.blocks.start;
        let target_entry = target_row.blocks.start;
        self.bridge.charge(4, out)?;
        if original_row.blocks.is_empty()
            || target_row.blocks.is_empty()
            || self.heads[target_entry] != original_entry
        {
            return Err(mismatch());
        }
        emit!(
            out,
            "open spec fn typed_prefix_entry_{function}_v49(before: MemoryStateV30) -> TypedPrefixCursorV49 {{\n let values = Seq::new({}, |i: int| MemoryValueV30::Undefined);\n",
            output.definitions().len()
        );
        for target in target_row.definitions.clone() {
            self.bridge.charge(3, out)?;
            let Definition::FunctionArgument {
                function: target_function,
                argument,
            } = output.definitions()[target].coordinate
            else {
                continue;
            };
            let original = self.anchors[target];
            if original == NONE
                || target_function != target_row.coordinate
                || input.definitions()[original].coordinate
                    != (Definition::FunctionArgument {
                        function: original_row.coordinate,
                        argument,
                    })
            {
                return Err(mismatch());
            }
            emit!(
                out,
                " let values = values.update({target}, if {original} < before.values.len() {{ before.values[{original}] }} else {{ MemoryValueV30::Undefined }});\n"
            );
        }
        emit!(
            out,
            " typed_prefix_begin_{function}_v49(MemoryStateV30 {{ pc: {target_entry}, values, ..before }})\n}}\nopen spec fn typed_prefix_native_shape_{function}_v49(before: MemoryStateV30) -> bool {{ before.valid && before.pc == {original_entry} && before.values.len() == {}",
            input.definitions().len()
        );
        for (original, row) in input.definitions().iter().enumerate() {
            self.bridge.charge(1, out)?;
            if !matches!(row.coordinate, Definition::FunctionArgument { function, .. } if function == original_row.coordinate)
            {
                emit!(
                    out,
                    " && before.values[{original}] == MemoryValueV30::Undefined"
                );
            }
        }
        emit!(
            out,
            " }}\nproof fn typed_prefix_entry_relation_{function}_v49(before: MemoryStateV30, little_endian: bool)\n requires typed_prefix_native_shape_{function}_v49(before), byte_state_memory_well_formed_v30(before),"
        );
        for original in original_row.definitions.clone() {
            let row = &input.definitions()[original];
            if matches!(row.coordinate, Definition::FunctionArgument { .. }) {
                self.bridge.charge(1, out)?;
                emit!(out, "\n {{ let compared = before.values[{original}]; ");
                // Entry uses the same concrete type interpretation as the bodies.
                // The caller supplies no replacement value or runtime semantics.
                super::super::super::byte_function_v30::emit_value_type(
                    row.ty, width, "compared", out,
                )?;
                emit!(out, " }},");
            }
        }
        emit!(
            out,
            "\n ensures typed_prefix_related_{function}_v49(before, typed_prefix_entry_{function}_v49(before), little_endian),\n{{ assert(typed_prefix_related_{function}_v49(before, typed_prefix_entry_{function}_v49(before), little_endian)); }}\n"
        );
        self.check(out)
    }

    fn emit_laws(&self, function: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        let input = self.bridge.prefix.input();
        let original_function = self.bridge.prefix.rows().functions[function].input.0 as usize;
        let row = &input.functions()[original_function];
        emit!(
            out,
            "open spec fn typed_prefix_source_step_{function}_v49(state: MemoryStateV30, little_endian: bool) -> CfgStepV26<MemoryStateV30, TypedPrefixEventV49> {{ if !state.valid || state.pc < 0 {{ CfgStepV26 {{ state, events: seq![], halted: true }} }} else {{ let result = byte_block_step_{original_function}_v30(state, little_endian); CfgStepV26 {{ state: result.state, events: seq![typed_prefix_source_event_v49(result)], halted: !result.state.valid || result.state.pc < 0 }} }} }}\nopen spec fn typed_prefix_actual_step_{function}_v49(cursor: TypedPrefixCursorV49, little_endian: bool) -> CfgStepV26<TypedPrefixCursorV49, TypedPrefixEventV49> {{ if !cursor.micro.state.valid || cursor.micro.state.pc < 0 {{ CfgStepV26 {{ state: cursor, events: seq![], halted: true }} }} else {{ let result = typed_prefix_step_{function}_v49(cursor, little_endian); CfgStepV26 {{ state: result.cursor, events: seq![typed_prefix_actual_event_v49(result)], halted: !result.cursor.micro.state.valid || result.cursor.micro.state.pc < 0 }} }} }}\n"
        );
        for original in row.blocks.clone() {
            let Some(_) = self.segment(original, out)? else {
                continue;
            };
            self.bridge.charge(3, out)?;
            emit!(
                out,
                "proof fn typed_prefix_segment_{original}_v49(before: MemoryStateV30, cursor: TypedPrefixCursorV49, little_endian: bool)\n requires typed_prefix_related_{function}_v49(before, cursor, little_endian), before.pc == {original}, byte_block_{original_function}_{original}_v30(before, little_endian).state.valid,\n ensures typed_prefix_related_{function}_v49(byte_block_{original_function}_{original}_v30(before, little_endian).state, typed_prefix_step_{function}_v49(cursor, little_endian).cursor, little_endian),\n typed_prefix_source_event_v49(byte_block_{original_function}_{original}_v30(before, little_endian)).observations.is_some(),\n typed_prefix_actual_event_v49(typed_prefix_step_{function}_v49(cursor, little_endian)).observations.is_some(),\n typed_prefix_source_event_v49(byte_block_{original_function}_{original}_v30(before, little_endian)) == typed_prefix_actual_event_v49(typed_prefix_step_{function}_v49(cursor, little_endian)),\n{{ let original = byte_block_{original_function}_{original}_v30(before, little_endian); let actual = typed_prefix_step_{function}_v49(cursor, little_endian);\n assert(typed_prefix_related_{function}_v49(original.state, actual.cursor, little_endian));\n assert(typed_prefix_source_event_v49(original).observations.is_some());\n assert(typed_prefix_actual_event_v49(actual).observations.is_some());\n assert(typed_prefix_source_event_v49(original) == typed_prefix_actual_event_v49(actual));\n}}\n"
            );
        }
        emit!(
            out,
            "open spec fn typed_prefix_input_defined_{function}_v49(state: MemoryStateV30, little_endian: bool, fuel: nat) -> bool\n decreases fuel,\n{{ state.valid && (fuel == 0 || state.pc < 0 || typed_prefix_input_defined_{function}_v49(typed_prefix_source_step_{function}_v49(state, little_endian).state, little_endian, (fuel - 1) as nat)) }}\nproof fn typed_prefix_step_relation_{function}_v49(before: MemoryStateV30, cursor: TypedPrefixCursorV49, little_endian: bool)\n requires typed_prefix_related_{function}_v49(before, cursor, little_endian), typed_prefix_input_defined_{function}_v49(before, little_endian, 1),\n ensures typed_prefix_related_{function}_v49(typed_prefix_source_step_{function}_v49(before, little_endian).state, typed_prefix_actual_step_{function}_v49(cursor, little_endian).state, little_endian),\n typed_prefix_source_step_{function}_v49(before, little_endian).events == typed_prefix_actual_step_{function}_v49(cursor, little_endian).events,\n typed_prefix_source_step_{function}_v49(before, little_endian).halted == typed_prefix_actual_step_{function}_v49(cursor, little_endian).halted,\n{{ if before.pc >= 0 {{\n"
        );
        for original in row.blocks.clone() {
            if self.segment(original, out)?.is_some() {
                emit!(
                    out,
                    " if before.pc == {original} {{ typed_prefix_segment_{original}_v49(before, cursor, little_endian); }} else\n"
                );
            }
        }
        emit!(
            out,
            " {{ assert(false); }} }} }}\nproof fn typed_prefix_trace_{function}_v49(before: MemoryStateV30, cursor: TypedPrefixCursorV49, little_endian: bool, fuel: nat)\n requires typed_prefix_related_{function}_v49(before, cursor, little_endian), typed_prefix_input_defined_{function}_v49(before, little_endian, fuel),\n ensures cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), before, fuel).events == cfg_trace_v26(|c| typed_prefix_actual_step_{function}_v49(c, little_endian), cursor, fuel).events,\n cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), before, fuel).halted == cfg_trace_v26(|c| typed_prefix_actual_step_{function}_v49(c, little_endian), cursor, fuel).halted,\n typed_prefix_related_{function}_v49(cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), before, fuel).state, cfg_trace_v26(|c| typed_prefix_actual_step_{function}_v49(c, little_endian), cursor, fuel).state, little_endian),\n decreases fuel,\n{{ if fuel > 0 {{ typed_prefix_step_relation_{function}_v49(before, cursor, little_endian); let original = typed_prefix_source_step_{function}_v49(before, little_endian); let actual = typed_prefix_actual_step_{function}_v49(cursor, little_endian); if !original.halted {{ typed_prefix_trace_{function}_v49(original.state, actual.state, little_endian, (fuel - 1) as nat); }} }} }}\n"
        );
        self.check(out)
    }
}

impl<'a, 'owner, 'rows, R: ByteAllocationResolverV30> AllocationBridgeV48<'a, 'owner, 'rows, R> {
    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_typed_prefix_v49(
        &self,
        input_physical: &Physical<'_, 'owner>,
        output_physical: &Physical<'_, 'owner>,
        input_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        output_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_typed_prefix_inner_v49(
            input_physical,
            output_physical,
            input_contracts,
            output_contracts,
            None,
            None,
            None,
            width,
            registry_namespace,
            out,
        )
    }

    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_typed_prefix_licm_v49(
        &self,
        input_physical: &Physical<'_, 'owner>,
        middle_physical: &Physical<'_, 'owner>,
        output_physical: &Physical<'_, 'owner>,
        input_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        middle_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        output_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.emit_typed_prefix_inner_v49(
            input_physical,
            middle_physical,
            input_contracts,
            middle_contracts,
            Some((output_physical, output_contracts)),
            None,
            None,
            width,
            registry_namespace,
            out,
        )
    }

    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_typed_source_prefix_licm_v49(
        &self,
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        input_physical: &Physical<'_, 'owner>,
        middle_physical: &Physical<'_, 'owner>,
        output_physical: &Physical<'_, 'owner>,
        input_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        middle_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        output_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let inventory = relation.inventory(out.budget)?;
        self.check(out)?;
        if !std::ptr::eq(inventory.owner(), self.prefix.input().owner()) {
            return Err(mismatch());
        }
        self.emit_typed_prefix_inner_v49(
            input_physical,
            middle_physical,
            input_contracts,
            middle_contracts,
            Some((output_physical, output_contracts)),
            Some(relation),
            None,
            width,
            registry_namespace,
            out,
        )
    }

    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_typed_source_chain_v49(
        &self,
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        original_physical: &Physical<'_, 'owner>,
        prefix_physical: &Physical<'_, 'owner>,
        relocated_physical: &Physical<'_, 'owner>,
        original_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        prefix_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        relocated_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        forwarded: &Inventory<'owner>,
        forwarding: &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
        forwarded_physical: &Physical<'_, 'owner>,
        forwarded_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let source = relation.inventory(out.budget)?;
        self.check(out)?;
        self.charge(3, out)?;
        if !std::ptr::eq(source.owner(), self.prefix.input().owner())
            || !self.output.belongs_to(forwarding.input())
            || !forwarded.belongs_to(forwarding.output())
        {
            return Err(mismatch());
        }
        self.emit_typed_prefix_inner_v49(
            original_physical,
            prefix_physical,
            original_contracts,
            prefix_contracts,
            Some((relocated_physical, relocated_contracts)),
            Some(relation),
            Some((
                forwarded,
                forwarding,
                forwarded_physical,
                forwarded_contracts,
            )),
            width,
            registry_namespace,
            out,
        )
    }

    fn emit_typed_prefix_inner_v49(
        &self,
        input_physical: &Physical<'_, 'owner>,
        output_physical: &Physical<'_, 'owner>,
        input_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        output_contracts: &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        relocated: Option<(
            &Physical<'_, 'owner>,
            &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        )>,
        source: Option<&fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>>,
        forwarded: Option<(
            &Inventory<'owner>,
            &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
            &Physical<'_, 'owner>,
            &super::super::super::target_view_contracts_v38::TargetByteViewContractsV38<'_, 'owner>,
        )>,
        width: FormalIndexWidth,
        registry_namespace: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let input = self.prefix.input();
        let output = self.prefix.output();
        input_contracts.check_owner_width_v39(input.owner(), width, out)?;
        output_contracts.check_owner_width_v39(output.owner(), width, out)?;
        if !input_physical.is_for(input) || !output_physical.is_for(output) {
            return Err(mismatch());
        }
        if let Some((physical, contracts)) = relocated {
            contracts.check_owner_width_v39(self.output.owner(), width, out)?;
            if !physical.is_for(self.output) {
                return Err(mismatch());
            }
        }
        let floor = out.budget.storage();
        let result = (|| {
            out.budget.reserve_storage(size_of::<(
                [usize; 8],
                [&(); 12],
                [Range<usize>; 2],
                [Result<()>; 2],
                Writer<'_, '_>,
            )>())?;
            let segments = PrefixSegmentsV49::derive(self, out)?;
            let original_allocations = self.view(AllocationSideV48::Original, out)?;
            let prefix_allocations = self.view(AllocationSideV48::Prefix, out)?;
            let relocated_allocations = self.view(AllocationSideV48::Relocated, out)?;
            emit!(out, "mod typed_prefix_v49 {{\n use super::*;\n");
            self.emit_transport([0, 1, 2], out)?;
            for (function, association) in self.prefix.rows().functions.iter().enumerate() {
                self.charge(4, out)?;
                let original_function = association.input.0 as usize;
                let original = input
                    .functions()
                    .get(original_function)
                    .ok_or_else(mismatch)?;
                let actual = output.functions().get(function).ok_or_else(mismatch)?;
                if actual.coordinate != association.output
                    || original.coordinate != association.input
                    || actual.function.body.is_some() != original.function.body.is_some()
                {
                    return Err(mismatch());
                }
                if original.function.body.is_none() {
                    continue;
                }
                let function_floor = out.budget.storage();
                let before = super::super::super::byte_function_v30::ByteFunctionV30::derive(
                    input, input_physical, association.input,
                    super::super::super::byte_function_v30::ByteInterpretationContextV39::classified(width, input_contracts, registry_namespace),
                    &original_allocations, out,
                )?;
                let after = super::super::super::byte_function_v30::ByteFunctionV30::derive(
                    output, output_physical, association.output,
                    super::super::super::byte_function_v30::ByteInterpretationContextV39::classified(width, output_contracts, registry_namespace),
                    &prefix_allocations, out,
                )?;
                before.emit(original_function, out)?;
                after.emit(
                    input
                        .functions()
                        .len()
                        .checked_add(function)
                        .ok_or(Resource::Arithmetic)?,
                    out,
                )?;
                drop((before, after));
                if let Some((physical, contracts)) = relocated {
                    let model = super::super::super::byte_function_v30::ByteFunctionV30::derive(
                        self.output, physical, association.output,
                        super::super::super::byte_function_v30::ByteInterpretationContextV39::classified(width, contracts, registry_namespace),
                        &relocated_allocations, out,
                    )?;
                    model.emit(
                        input
                            .functions()
                            .len()
                            .checked_add(output.functions().len())
                            .and_then(|n| n.checked_add(function))
                            .ok_or(Resource::Arithmetic)?,
                        out,
                    )?;
                    drop(model);
                }
                out.budget.release_storage(
                    out.budget
                        .storage()
                        .checked_sub(function_floor)
                        .ok_or(Resource::Accounting)?,
                )?;
            }
            segments.emit_cursors(out)?;
            emit!(
                out,
                "struct TypedPrefixEventV49 {{ observations: Option<Seq<TypedMemoryObservationV48>>, returned: Seq<MemoryValueV30>, terminal: int }}\nopen spec fn typed_prefix_source_event_v49(result: MemoryBlockResultV30) -> TypedPrefixEventV49 {{ TypedPrefixEventV49 {{ observations: typed_observations_0_v48(result.observations), returned: result.returned, terminal: if !result.state.valid {{ -3 }} else if result.state.pc == -2 {{ -2 }} else if result.state.pc == -1 {{ -1 }} else {{ 0 }} }} }}\nopen spec fn typed_prefix_actual_event_v49(result: TypedPrefixStepV49) -> TypedPrefixEventV49 {{ TypedPrefixEventV49 {{ observations: typed_observations_1_v48(result.observations), returned: result.returned, terminal: if !result.cursor.micro.state.valid {{ -3 }} else if result.cursor.micro.state.pc == -2 {{ -2 }} else if result.cursor.micro.state.pc == -1 {{ -1 }} else {{ 0 }} }} }}\n"
            );
            for (function, row) in output.functions().iter().enumerate() {
                self.charge(1, out)?;
                if row.function.body.is_none() {
                    continue;
                }
                segments.emit_relation(function, width, out)?;
                segments.emit_entry(function, width, out)?;
                segments.emit_laws(function, out)?;
            }
            if relocated.is_some() {
                segments.emit_licm_composition(width, out)?;
            }
            if let Some(relation) = source {
                if relocated.is_none() {
                    return Err(mismatch());
                }
                segments.emit_source_composition(relation, out)?;
            }
            if let Some((final_inventory, forwarding, final_physical, final_contracts)) = forwarded
            {
                let relation = source.ok_or_else(mismatch)?;
                let (physical, contracts) = relocated.ok_or_else(mismatch)?;
                super::super::super::store_consensus_typed_v46::generate_segmented(
                    self.output,
                    final_inventory,
                    forwarding,
                    physical,
                    final_physical,
                    contracts,
                    final_contracts,
                    width,
                    registry_namespace,
                    &relocated_allocations,
                    &segments,
                    relation,
                    out,
                )?;
            }
            emit!(out, "}}\n");
            drop((
                segments,
                original_allocations,
                prefix_allocations,
                relocated_allocations,
            ));
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result?;
        self.check(out)
    }
}
