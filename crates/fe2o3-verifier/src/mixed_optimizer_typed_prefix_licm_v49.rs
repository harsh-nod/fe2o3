//! LICM operates on the exact prefix micro-cuts, including cuts inside merged
//! blocks. The intermediate state is shared, not related by an assumed encoding.
use super::*;
use crate::mixed_optimizer_refinement_v26::relocation_plan_v28;
use fe2o3_kernel_ir::with_canonical_kir_control_flow_v18 as with_flow;

fn plan_error(error: relocation_plan_v28::Error) -> Error {
    match error {
        relocation_plan_v28::Error::Resource(error) => error.into(),
        relocation_plan_v28::Error::Inventory(error) => error.into(),
        relocation_plan_v28::Error::Flow(error) => error.into(),
        relocation_plan_v28::Error::Mismatch(reason) => Error::Statement(reason),
    }
}

fn relocated_definition(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Licm<'_>,
    original: usize,
    out: &mut Writer<'_, '_>,
) -> Result<(usize, bool)> {
    out.budget.charge_work(4)?;
    let row = &input.definitions()[original];
    let (coordinate, moved) = match row.coordinate {
        Definition::Result { operation, result } => {
            let ordinal = operation_index(input, operation)?;
            let origin = pair.origins().get(ordinal).ok_or_else(mismatch)?;
            if origin.input != operation {
                return Err(mismatch());
            }
            (
                Definition::Result {
                    operation: origin.output,
                    result,
                },
                origin.hoist.is_some(),
            )
        }
        other => (other, false),
    };
    let target = definition_index(output, coordinate)?;
    if row.ty != output.definitions()[target].ty {
        return Err(mismatch());
    }
    Ok((target, moved))
}

impl<'b, 'a, 'owner, 'rows, R: ByteAllocationResolverV30>
    PrefixSegmentsV49<'b, 'a, 'owner, 'rows, R>
{
    pub(super) fn relocated_segments(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<Vec<Option<SegmentV49>>> {
        self.check(out)?;
        let input = self.bridge.prefix.output();
        let output = self.bridge.output;
        if input.blocks().len() != output.blocks().len()
            || input.functions().len() != output.functions().len()
        {
            return Err(mismatch());
        }
        out.budget.reserve_storage(
            size_of::<Vec<Option<SegmentV49>>>()
                + self
                    .segments
                    .len()
                    .checked_mul(size_of::<Option<SegmentV49>>())
                    .ok_or(Resource::Arithmetic)?,
        )?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(self.segments.len())
            .map_err(|_| Resource::Allocation)?;
        out.budget.reserve_storage(
            segments
                .capacity()
                .checked_sub(self.segments.len())
                .ok_or(Resource::Accounting)?
                .checked_mul(size_of::<Option<SegmentV49>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let scratch = out.budget.storage();
        let built = (|| {
            let mut gaps = allocate(input.operations().len(), out)?;
            for (block, source) in input.blocks().iter().enumerate() {
                self.bridge.charge(5, out)?;
                let target = &output.blocks()[block];
                if source.coordinate != target.coordinate {
                    return Err(mismatch());
                }
                let mut next = target.operations.end;
                for operation in source.operations.clone().rev() {
                    self.bridge.charge(5, out)?;
                    let origin = &self.bridge.licm.origins()[operation];
                    if origin.input != input.operations()[operation].coordinate {
                        return Err(mismatch());
                    }
                    if origin.hoist.is_none() {
                        let actual = operation_index(output, origin.output)?;
                        if !target.operations.contains(&actual) || actual >= next {
                            return Err(mismatch());
                        }
                        next = actual;
                    }
                    gaps[operation] = next;
                }
            }
            for (original, segment) in self.segments.iter().enumerate() {
                self.bridge.charge(7, out)?;
                let Some(segment) = segment else {
                    segments.push(None);
                    continue;
                };
                let input_block = &input.blocks()[segment.output_block];
                let output_block = &output.blocks()[segment.output_block];
                let gap = |position: usize| -> Result<usize> {
                    if position == input_block.operations.end {
                        Ok(output_block.operations.end)
                    } else if input_block.operations.contains(&position) {
                        gaps.get(position).copied().ok_or_else(mismatch)
                    } else {
                        Err(mismatch())
                    }
                };
                let start = if self.heads[segment.output_block] == original {
                    output_block.operations.start
                } else {
                    gap(segment.start)?
                };
                let end = if segment.next_original.is_none() {
                    output_block.operations.end
                } else {
                    gap(segment.end)?
                };
                if start > end
                    || start < output_block.operations.start
                    || end > output_block.operations.end
                {
                    return Err(mismatch());
                }
                segments.push(Some(SegmentV49 {
                    start,
                    end,
                    ..*segment
                }));
            }
            for (original, segment) in segments.iter().enumerate() {
                self.bridge.charge(3, out)?;
                let Some(segment) = segment else {
                    continue;
                };
                if let Some(next) = segment.next_original {
                    let next = segments.get(next).copied().flatten().ok_or_else(mismatch)?;
                    if next.output_block != segment.output_block || next.start != segment.end {
                        return Err(mismatch());
                    }
                } else if segment.end != output.blocks()[segment.output_block].operations.end {
                    return Err(mismatch());
                }
                if self.heads[segment.output_block] == original
                    && segment.start != output.blocks()[segment.output_block].operations.start
                {
                    return Err(mismatch());
                }
            }
            drop(gaps);
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(scratch)
                .ok_or(Resource::Accounting)?,
        )?;
        built?;
        self.check(out)?;
        Ok(segments)
    }

    fn emit_gap_relation(
        &self,
        relocated: &[Option<SegmentV49>],
        function: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let input = self.bridge.prefix.output();
        let output = self.bridge.output;
        let row = &input.functions()[function];
        emit!(
            out,
            "spec fn typed_licm_arguments_{function}_v102(before: MemoryStateV30, after: MemoryStateV30) -> bool {{ true"
        );
        for definition in row.definitions.clone() {
            self.bridge.charge(1, out)?;
            if !matches!(
                input.definitions()[definition].coordinate,
                Definition::FunctionArgument { .. }
            ) {
                continue;
            }
            let (target, moved) =
                relocated_definition(input, output, self.bridge.licm, definition, out)?;
            if moved {
                return Err(mismatch());
            }
            emit!(
                out,
                "\n && before.values[{definition}] == after.values[{target}] && "
            );
            super::super::super::byte_function_v30::emit_value_type(
                output.definitions()[target].ty,
                width,
                format_args!("after.values[{target}]"),
                out,
            )?;
        }
        emit!(out, "\n}}\n");
        emit!(
            out,
            "spec fn typed_licm_cursor_related_{function}_v49(before_cursor: TypedPrefixCursorV49, after_cursor: TypedPrefixCursorV49, little_endian: bool) -> bool {{\n let before = before_cursor.micro.state; let after = after_cursor.micro.state;\n typed_prefix_cursor_valid_{function}_v49(before_cursor, little_endian) && typed_relocated_cursor_valid_{function}_v49(after_cursor, little_endian)\n && before_cursor.segment == after_cursor.segment && before.pc == after.pc\n && before.memory == after.memory && before.generations == after.generations && before.frames == after.frames\n && typed_allocation_environment_1_v48(before) == typed_allocation_environment_2_v48(after)\n && (if before.pc == -1 || before.pc == -2 {{ true }} else {{ typed_licm_arguments_{function}_v102(before, after) && (\n"
        );
        super::models::with_dominance(output, function, out, |output_flow, out| {
            let text = std::mem::take(&mut out.text);
            let failure = out.failure.take();
            let (text, failure) = with_flow(
                input.owner(),
                row.coordinate,
                Default::default(),
                out.budget,
                |input_flow, budget| {
                    let mut writer = Writer {
                        text,
                        budget,
                        failure,
                    };
                    let out = &mut writer;
                    for (original, segment) in self.segments.iter().enumerate() {
                        let Some(segment) = segment else {
                            continue;
                        };
                        if !row.blocks.contains(&segment.output_block) {
                            continue;
                        }
                        let actual = relocated[original].ok_or_else(mismatch)?;
                        emit!(out, " if before_cursor.segment == {original} {{ true");
                        for definition in row.definitions.clone() {
                            let (target, moved) = relocated_definition(
                                input,
                                output,
                                self.bridge.licm,
                                definition,
                                out,
                            )?;
                            let present = super::models::available(
                                input,
                                input_flow,
                                input.definitions()[definition].coordinate,
                                segment.output_block,
                                segment.start,
                                out,
                            )?;
                            let target_present = output_flow.available(
                                output.definitions()[target].coordinate,
                                actual.output_block,
                                actual.start,
                                out,
                            )?;
                            if (!moved && present != target_present) || (present && !target_present)
                            {
                                return Err(mismatch());
                            }
                            // Retain both availability checks before sharing the
                            // argument predicate across all nonterminal segments.
                            if matches!(
                                input.definitions()[definition].coordinate,
                                Definition::FunctionArgument { .. }
                            ) {
                                if !present || !target_present || moved {
                                    return Err(mismatch());
                                }
                                continue;
                            }
                            if present {
                                emit!(
                                    out,
                                    "\n && before.values[{definition}] == after.values[{target}]"
                                );
                            }
                            if moved && target_present {
                                emit!(
                                    out,
                                    "\n && typed_relocation_result_0_v48({definition}, before, little_endian) == Some(after.values[{target}])"
                                );
                            }
                            if target_present {
                                emit!(out, " && ");
                                super::super::super::byte_function_v30::emit_value_type(
                                    output.definitions()[target].ty,
                                    width,
                                    format_args!("after.values[{target}]"),
                                    out,
                                )?;
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

    fn emit_composed_laws(&self, function: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        let input = self.bridge.prefix.output();
        let row = &input.functions()[function];
        emit!(
            out,
            "spec fn typed_relocated_actual_step_{function}_v49(cursor: TypedPrefixCursorV49, little_endian: bool) -> CfgStepV26<TypedPrefixCursorV49, TypedPrefixEventV49> {{ if !cursor.micro.state.valid || cursor.micro.state.pc < 0 {{ CfgStepV26 {{ state: cursor, events: seq![], halted: true }} }} else {{ let result = typed_relocated_step_{function}_v49(cursor, little_endian); CfgStepV26 {{ state: result.cursor, events: seq![typed_relocated_event_v49(result)], halted: !result.cursor.micro.state.valid || result.cursor.micro.state.pc < 0 }} }} }}\n"
        );
        for (original, segment) in self.segments.iter().enumerate() {
            let Some(segment) = segment else {
                continue;
            };
            if !row.blocks.contains(&segment.output_block) {
                continue;
            }
            self.bridge.charge(3, out)?;
            emit!(
                out,
                "proof fn typed_licm_segment_{original}_v49(before: TypedPrefixCursorV49, after: TypedPrefixCursorV49, little_endian: bool)\n requires typed_licm_cursor_related_{function}_v49(before, after, little_endian), before.segment == {original}, typed_prefix_step_{function}_v49(before, little_endian).cursor.micro.state.valid,\n ensures typed_licm_cursor_related_{function}_v49(typed_prefix_step_{function}_v49(before, little_endian).cursor, typed_relocated_step_{function}_v49(after, little_endian).cursor, little_endian),\n typed_prefix_actual_event_v49(typed_prefix_step_{function}_v49(before, little_endian)).observations.is_some(),\n typed_relocated_event_v49(typed_relocated_step_{function}_v49(after, little_endian)).observations.is_some(),\n typed_prefix_actual_event_v49(typed_prefix_step_{function}_v49(before, little_endian)) == typed_relocated_event_v49(typed_relocated_step_{function}_v49(after, little_endian)),\n{{ let left = typed_prefix_step_{function}_v49(before, little_endian); let right = typed_relocated_step_{function}_v49(after, little_endian);\n assert(typed_licm_cursor_related_{function}_v49(left.cursor, right.cursor, little_endian));\n assert(typed_prefix_actual_event_v49(left).observations.is_some()); assert(typed_relocated_event_v49(right).observations.is_some());\n assert(typed_prefix_actual_event_v49(left) == typed_relocated_event_v49(right));\n}}\n"
            );
        }
        emit!(
            out,
            "spec fn typed_composed_related_{function}_v49(original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49, little_endian: bool) -> bool {{ typed_prefix_related_{function}_v49(original, middle, little_endian) && typed_licm_cursor_related_{function}_v49(middle, actual, little_endian) }}\nproof fn typed_composed_step_{function}_v49(original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49, little_endian: bool)\n requires typed_composed_related_{function}_v49(original, middle, actual, little_endian), typed_prefix_input_defined_{function}_v49(original, little_endian, 1),\n ensures typed_composed_related_{function}_v49(typed_prefix_source_step_{function}_v49(original, little_endian).state, typed_prefix_actual_step_{function}_v49(middle, little_endian).state, typed_relocated_actual_step_{function}_v49(actual, little_endian).state, little_endian),\n typed_prefix_source_step_{function}_v49(original, little_endian).events == typed_relocated_actual_step_{function}_v49(actual, little_endian).events,\n typed_prefix_source_step_{function}_v49(original, little_endian).halted == typed_relocated_actual_step_{function}_v49(actual, little_endian).halted,\n{{ typed_prefix_step_relation_{function}_v49(original, middle, little_endian);\n if original.pc >= 0 {{\n"
        );
        for (original, segment) in self.segments.iter().enumerate() {
            if segment.is_some_and(|s| row.blocks.contains(&s.output_block)) {
                self.bridge.charge(1, out)?;
                emit!(
                    out,
                    " if middle.segment == {original} {{ typed_licm_segment_{original}_v49(middle, actual, little_endian); }} else\n"
                );
            }
        }
        emit!(
            out,
            " {{ assert(false); }} }} }}\nproof fn typed_composed_trace_{function}_v49(original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49, little_endian: bool, fuel: nat)\n requires typed_composed_related_{function}_v49(original, middle, actual, little_endian), typed_prefix_input_defined_{function}_v49(original, little_endian, fuel),\n ensures cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), original, fuel).events == cfg_trace_v26(|c| typed_relocated_actual_step_{function}_v49(c, little_endian), actual, fuel).events,\n cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), original, fuel).halted == cfg_trace_v26(|c| typed_relocated_actual_step_{function}_v49(c, little_endian), actual, fuel).halted,\n typed_composed_related_{function}_v49(cfg_trace_v26(|s| typed_prefix_source_step_{function}_v49(s, little_endian), original, fuel).state, cfg_trace_v26(|c| typed_prefix_actual_step_{function}_v49(c, little_endian), middle, fuel).state, cfg_trace_v26(|c| typed_relocated_actual_step_{function}_v49(c, little_endian), actual, fuel).state, little_endian),\n decreases fuel,\n{{ if fuel > 0 {{ typed_composed_step_{function}_v49(original, middle, actual, little_endian); let left = typed_prefix_source_step_{function}_v49(original, little_endian); let center = typed_prefix_actual_step_{function}_v49(middle, little_endian); let right = typed_relocated_actual_step_{function}_v49(actual, little_endian); if !left.halted {{ typed_composed_trace_{function}_v49(left.state, center.state, right.state, little_endian, (fuel - 1) as nat); }} }} }}\n"
        );
        self.check(out)
    }

    fn emit_composed_entry(
        &self,
        function: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let original = self.bridge.prefix.input();
        let middle = self.bridge.prefix.output();
        let output = self.bridge.output;
        let original_function = self.bridge.prefix.rows().functions[function].input.0 as usize;
        let original_row = &original.functions()[original_function];
        let middle_row = &middle.functions()[function];
        let entry = output.functions()[function].blocks.start;
        emit!(
            out,
            "spec fn typed_relocated_entry_{function}_v49(before: MemoryStateV30) -> TypedPrefixCursorV49 {{\n let values = Seq::new({}, |i: int| MemoryValueV30::Undefined);\n",
            output.definitions().len()
        );
        for definition in middle_row.definitions.clone() {
            if !matches!(
                middle.definitions()[definition].coordinate,
                Definition::FunctionArgument { .. }
            ) {
                continue;
            }
            let (target, moved) =
                relocated_definition(middle, output, self.bridge.licm, definition, out)?;
            if moved {
                return Err(mismatch());
            }
            emit!(
                out,
                " let values = values.update({target}, if {definition} < before.values.len() {{ before.values[{definition}] }} else {{ MemoryValueV30::Undefined }});\n"
            );
        }
        emit!(
            out,
            " typed_relocated_begin_{function}_v49(MemoryStateV30 {{ pc: {entry}, values, ..before }})\n}}\nproof fn typed_composed_entry_relation_{function}_v49(original: MemoryStateV30, little_endian: bool)\n requires typed_prefix_native_shape_{function}_v49(original), byte_state_memory_well_formed_v30(original),"
        );
        for definition in original_row.definitions.clone() {
            if matches!(
                original.definitions()[definition].coordinate,
                Definition::FunctionArgument { .. }
            ) {
                self.bridge.charge(1, out)?;
                emit!(out, "\n {{ let compared = original.values[{definition}]; ");
                super::super::super::byte_function_v30::emit_value_type(
                    original.definitions()[definition].ty,
                    width,
                    "compared",
                    out,
                )?;
                emit!(out, " }},");
            }
        }
        emit!(
            out,
            "\n ensures typed_composed_related_{function}_v49(original, typed_prefix_entry_{function}_v49(original), typed_relocated_entry_{function}_v49(typed_prefix_entry_{function}_v49(original).micro.state), little_endian),\n{{ typed_prefix_entry_relation_{function}_v49(original, little_endian); let middle = typed_prefix_entry_{function}_v49(original); let actual = typed_relocated_entry_{function}_v49(middle.micro.state);\n assert(typed_licm_cursor_related_{function}_v49(middle, actual, little_endian));\n}}\n"
        );
        self.check(out)
    }

    pub(super) fn emit_licm_composition(
        &self,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let floor = out.budget.storage();
        let result = (|| {
            out.budget.reserve_storage(size_of::<(
                [usize; 10],
                [&(); 12],
                [Result<()>; 2],
                Writer<'_, '_>,
                [std::fmt::Arguments<'_>; 2],
            )>())?;
            let relocated = self.relocated_segments(out)?;
            let expressions =
                relocation_plan_v28::build(self.bridge.licm, out.budget).map_err(plan_error)?;
            expressions.emit_typed_expressions_v48(
                self.bridge.licm,
                self.bridge.prefix.output(),
                self.bridge.output,
                0,
                self.bridge.prefix.input().functions().len(),
                out,
            )?;
            self.emit_stage_cursors(AllocationSideV48::Relocated, &relocated, out)?;
            emit!(
                out,
                "spec fn typed_relocated_event_v49(result: TypedPrefixStepV49) -> TypedPrefixEventV49 {{ TypedPrefixEventV49 {{ observations: typed_observations_2_v48(result.observations), returned: result.returned, terminal: if !result.cursor.micro.state.valid {{ -3 }} else if result.cursor.micro.state.pc == -2 {{ -2 }} else if result.cursor.micro.state.pc == -1 {{ -1 }} else {{ 0 }} }} }}\n"
            );
            for (function, row) in self.bridge.prefix.output().functions().iter().enumerate() {
                self.bridge.charge(1, out)?;
                if row.function.body.is_none() {
                    continue;
                }
                self.emit_gap_relation(&relocated, function, width, out)?;
                self.emit_composed_laws(function, out)?;
                self.emit_composed_entry(function, width, out)?;
            }
            drop(relocated);
            expressions.discard(out.budget).map_err(plan_error)?;
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        if let Err(Error::Resource(error)) = &result {
            self.bridge.failure.set(Some(*error));
        }
        result?;
        self.check(out)
    }
}
