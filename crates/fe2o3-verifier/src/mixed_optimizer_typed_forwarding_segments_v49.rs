//! The final forwarding relation shares the exact LICM cursor and source cuts.
//! Only authenticated private reads disappear from the source effect quotient.
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::store_consensus_plan_v46::MemoryPlan;
use fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18 as Pair;

const SOURCE_PROJECTION: &str = r#"
proof fn forwarding_source_projection_identity_v49(observations: Seq<MemoryOperationObservationV30>)
    requires typed_source_observations_valid_v49(observations),
    ensures invocation_actual_observations_v39(forwarding_project_observations_v46(observations))
        == invocation_actual_observations_v39(observations),
    decreases observations.len(),
{
    if observations.len() > 0 {
        let head = observations[0];
        if forwarding_removed_read_v46(head) {
            assert(invocation_project_effect_v39(head.effect).is_none());
        }
        forwarding_source_projection_identity_v49(observations.drop_first());
    }
}
proof fn forwarding_source_projection_v49(a: Seq<MemoryOperationObservationV30>, b: Seq<MemoryOperationObservationV30>)
    requires typed_source_observations_valid_v49(a), typed_source_observations_valid_v49(b),
        forwarding_project_observations_v46(a) == forwarding_project_observations_v46(b),
    ensures typed_source_mapped_observations_v49(a) == typed_source_mapped_observations_v49(b),
{
    forwarding_source_projection_identity_v49(a);
    forwarding_source_projection_identity_v49(b);
}
"#;

impl<'b, 'a, 'owner, 'rows, R: ByteAllocationResolverV30>
    PrefixSegmentsV49<'b, 'a, 'owner, 'rows, R>
{
    pub(in crate::mixed_optimizer_refinement_v26::semantics) fn emit_forwarding_composition(
        &self,
        output: &Inventory<'_>,
        pair: &Pair<'_>,
        plan: &MemoryPlan,
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let original = relation.inventory(out.budget)?;
        self.bridge.charge(5, out)?;
        if !std::ptr::eq(original.owner(), self.bridge.prefix.input().owner())
            || !self.bridge.output.belongs_to(pair.input())
            || !output.belongs_to(pair.output())
            || output.blocks().len() != self.bridge.output.blocks().len()
            || output.definitions().len() != self.bridge.output.definitions().len()
        {
            return Err(mismatch());
        }
        let floor = out.budget.storage();
        let result = (|| {
            out.budget
                .reserve_storage(size_of::<([usize; 10], [&(); 8], [Result<()>; 2])>())?;
            let relocated = self.relocated_segments(out)?;
            emit!(out, "{SOURCE_PROJECTION}\n");
            self.emit_named_stage_cursors("forwarded", output, &relocated, 1, 2, out)?;
            for (function, row) in output.functions().iter().enumerate() {
                self.bridge.charge(1, out)?;
                if row.function.body.is_some() {
                    self.emit_forwarding_function(function, &relocated, plan, out)?;
                }
            }
            let source = relation.source(out.budget)?;
            let roots = source.root_count(out.budget)?;
            let mut root_by_function = allocate(original.functions().len(), out)?;
            for root in 0..roots {
                let (_, function) = source.root(root, out.budget)?;
                self.bridge.charge(2, out)?;
                let entry = root_by_function.get_mut(function).ok_or_else(mismatch)?;
                if *entry != NONE {
                    return Err(mismatch());
                }
                *entry = root;
            }
            for (function, row) in self.bridge.prefix.rows().functions.iter().enumerate() {
                self.bridge.charge(2, out)?;
                let original = row.input.0 as usize;
                let root = root_by_function.get_mut(original).ok_or_else(mismatch)?;
                if *root == NONE {
                    continue;
                }
                self.emit_final_source_root(*root, original, function, out)?;
                *root = NONE;
            }
            self.bridge.charge(root_by_function.len(), out)?;
            if root_by_function.iter().any(|root| *root != NONE) {
                return Err(mismatch());
            }
            drop((relocated, root_by_function));
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

    fn emit_forwarding_function(
        &self,
        function: usize,
        segments: &[Option<SegmentV49>],
        plan: &MemoryPlan,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let input = self.bridge.output;
        let row = &input.functions()[function];
        let left_namespace = function.checked_mul(2).ok_or(Resource::Arithmetic)?;
        let right_namespace = left_namespace.checked_add(1).ok_or(Resource::Arithmetic)?;
        let shared_namespace = self
            .bridge
            .prefix
            .input()
            .functions()
            .len()
            .checked_add(self.bridge.prefix.output().functions().len())
            .and_then(|n| n.checked_add(function))
            .ok_or(Resource::Arithmetic)?;
        emit!(
            out,
            "spec fn forwarding_cursor_related_{function}_v49(left: TypedPrefixCursorV49, right: TypedPrefixCursorV49, little_endian: bool) -> bool {{\n left.segment == right.segment && left.micro.next_operation == right.micro.next_operation\n && left.micro.state.valid && right.micro.state.valid && forwarding_states_related_v46(left.micro.state, right.micro.state)\n && typed_relocated_cursor_valid_{function}_v49(left, little_endian) && typed_forwarded_cursor_valid_{function}_v49(right, little_endian)\n && forwarding_project_observations_v46(left.micro.observations) == forwarding_project_observations_v46(right.micro.observations)\n && (left.micro.state.pc < 0 || (\n"
        );
        for (original, segment) in segments.iter().enumerate() {
            let Some(segment) = segment else {
                continue;
            };
            if !row.blocks.contains(&segment.output_block) {
                continue;
            }
            self.bridge.charge(4, out)?;
            let (label, coordinate) =
                if segment.start < input.blocks()[segment.output_block].operations.end {
                    ("before", segment.start)
                } else {
                    ("exit", segment.output_block)
                };
            emit!(
                out,
                " if left.segment == {original} {{ forwarding_{label}_fact_v46({coordinate}, left.micro.state, little_endian) }} else\n"
            );
        }
        emit!(out, " {{ false }}))\n}}\n");
        for ordinal in row.operations.clone() {
            self.bridge.charge(1, out)?;
            emit!(
                out,
                "proof fn forwarding_shared_operation_{ordinal}_v49(s: MemoryStateV30, little_endian: bool)\n ensures super::byte_operation_{shared_namespace}_{ordinal}_v30(s, little_endian) == byte_operation_{left_namespace}_{ordinal}_v30(s, little_endian),\n{{ }}\n"
            );
        }
        for (original, segment) in segments.iter().enumerate() {
            let Some(segment) = segment else {
                continue;
            };
            if !row.blocks.contains(&segment.output_block) {
                continue;
            }
            self.bridge.charge(6, out)?;
            emit!(
                out,
                "proof fn forwarding_segment_{original}_v49(left: TypedPrefixCursorV49, right: TypedPrefixCursorV49, little_endian: bool)\n requires forwarding_cursor_related_{function}_v49(left, right, little_endian), left.segment == {original}, typed_relocated_step_{function}_v49(left, little_endian).cursor.micro.state.valid,\n ensures forwarding_cursor_related_{function}_v49(typed_relocated_step_{function}_v49(left, little_endian).cursor, typed_forwarded_step_{function}_v49(right, little_endian).cursor, little_endian),\n forwarding_project_observations_v46(typed_relocated_step_{function}_v49(left, little_endian).observations) == forwarding_project_observations_v46(typed_forwarded_step_{function}_v49(right, little_endian).observations),\n typed_relocated_step_{function}_v49(left, little_endian).returned == typed_forwarded_step_{function}_v49(right, little_endian).returned,\n{{ let nl = left.micro.state; let nr = right.micro.state;\n let nobs = left.micro.observations; let robs = right.micro.observations;\n"
            );
            for ordinal in segment.start..segment.end {
                self.bridge.charge(2, out)?;
                emit!(
                    out,
                    " forwarding_shared_operation_{ordinal}_v49(nl, little_endian);\n forwarding_operation_{ordinal}_v46(nl, nr, little_endian);\n let nl_result = byte_operation_{left_namespace}_{ordinal}_v30(nl, little_endian);\n let nr_result = byte_operation_{right_namespace}_{ordinal}_v30(nr, little_endian);\n let nobs = nobs.push(nl_result.observation); let robs = robs.push(nr_result.observation);\n let nl = nl_result.state; let nr = nr_result.state;\n"
                );
            }
            if segment.next_original.is_none() {
                let block = segment.output_block;
                emit!(
                    out,
                    " assert(super::byte_control_{shared_namespace}_{block}_v30(nl, nobs) == byte_control_{left_namespace}_{block}_v30(nl, nobs));\n let next_l = byte_control_{left_namespace}_{block}_v30(nl, nobs);\n let next_r = byte_control_{right_namespace}_{block}_v30(nr, robs);\n assert(forwarding_states_related_v46(next_l.state, next_r.state));\n if next_l.state.valid && next_l.state.pc >= 0 {{ assert(forwarding_entry_fact_v46(next_l.state.pc, next_l.state, little_endian)); }}\n"
                );
            }
            emit!(out, "}}\n");
        }
        let entry = row.blocks.start;
        self.bridge.charge(1, out)?;
        if plan.entry.get(entry).copied() != Some(NONE) {
            return Err(mismatch());
        }
        emit!(
            out,
            "proof fn forwarding_cursor_step_{function}_v49(left: TypedPrefixCursorV49, right: TypedPrefixCursorV49, little_endian: bool)\n requires forwarding_cursor_related_{function}_v49(left, right, little_endian), typed_relocated_step_{function}_v49(left, little_endian).cursor.micro.state.valid,\n ensures forwarding_cursor_related_{function}_v49(typed_relocated_step_{function}_v49(left, little_endian).cursor, typed_forwarded_step_{function}_v49(right, little_endian).cursor, little_endian),\n forwarding_project_observations_v46(typed_relocated_step_{function}_v49(left, little_endian).observations) == forwarding_project_observations_v46(typed_forwarded_step_{function}_v49(right, little_endian).observations),\n typed_relocated_step_{function}_v49(left, little_endian).returned == typed_forwarded_step_{function}_v49(right, little_endian).returned,\n{{ if left.micro.state.pc >= 0 {{\n"
        );
        for (original, segment) in segments.iter().enumerate() {
            if segment.is_some_and(|segment| row.blocks.contains(&segment.output_block)) {
                self.bridge.charge(1, out)?;
                emit!(
                    out,
                    " if left.segment == {original} {{ forwarding_segment_{original}_v49(left, right, little_endian); }} else\n"
                );
            }
        }
        emit!(out, " {{ assert(false); }} }} }}\n");
        Ok(())
    }

    fn emit_final_source_root(
        &self,
        root: usize,
        original: usize,
        function: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let range = self.bridge.prefix.input().functions()[original]
            .blocks
            .clone();
        let start = range.start;
        let end = range.end;
        let bound = range.len();
        self.bridge.charge(12, out)?;
        emit!(
            out,
            r#"spec fn typed_source_forwarded_follow_{root}_v49(cursor: TypedPrefixCursorV49, fuel: nat) -> TypedSourceBoundaryV49
 decreases fuel
{{ if !cursor.micro.state.valid || cursor.micro.state.pc < 0 || invocation_byte_cut_{root}_v36(cursor.segment) {{ typed_source_boundary_empty_v49(cursor) }}
 else if fuel == 0 || cursor.segment < {start} || {end} <= cursor.segment {{ typed_source_boundary_empty_v49(typed_prefix_refuse_v49(cursor).cursor) }}
 else {{ let head = typed_forwarded_step_{function}_v49(cursor, invocation_runtime_little_endian_v36());
 typed_source_boundary_join_v49(cursor.segment, head, typed_source_forwarded_follow_{root}_v49(head.cursor, (fuel - 1) as nat)) }} }}
spec fn typed_source_forwarded_boundary_{root}_v49(cursor: TypedPrefixCursorV49) -> TypedSourceBoundaryV49 {{
 if cursor.micro.state.pc < 0 {{ typed_source_boundary_empty_v49(cursor) }}
 else if !cursor.micro.state.valid || !invocation_byte_cut_{root}_v36(cursor.segment) {{ typed_source_boundary_empty_v49(typed_prefix_refuse_v49(cursor).cursor) }}
 else {{ let head = typed_forwarded_step_{function}_v49(cursor, invocation_runtime_little_endian_v36());
 typed_source_boundary_join_v49(cursor.segment, head, typed_source_forwarded_follow_{root}_v49(head.cursor, {bound}nat)) }} }}
proof fn forwarding_source_follow_{root}_v49(left: TypedPrefixCursorV49, right: TypedPrefixCursorV49, fuel: nat)
 requires forwarding_cursor_related_{function}_v49(left, right, invocation_runtime_little_endian_v36()), typed_source_relocated_follow_{root}_v49(left, fuel).cursor.micro.state.valid,
 ensures forwarding_cursor_related_{function}_v49(typed_source_relocated_follow_{root}_v49(left, fuel).cursor, typed_source_forwarded_follow_{root}_v49(right, fuel).cursor, invocation_runtime_little_endian_v36()),
 forwarding_project_observations_v46(typed_source_relocated_follow_{root}_v49(left, fuel).observations) == forwarding_project_observations_v46(typed_source_forwarded_follow_{root}_v49(right, fuel).observations),
 typed_source_observations_valid_v49(typed_source_relocated_follow_{root}_v49(left, fuel).observations), typed_source_observations_valid_v49(typed_source_forwarded_follow_{root}_v49(right, fuel).observations),
 typed_source_relocated_follow_{root}_v49(left, fuel).returned == typed_source_forwarded_follow_{root}_v49(right, fuel).returned,
 decreases fuel,
{{ if left.micro.state.pc >= 0 && !invocation_byte_cut_{root}_v36(left.segment) {{
 assert(fuel > 0);
 forwarding_cursor_step_{function}_v49(left, right, invocation_runtime_little_endian_v36());
 let nl = typed_relocated_step_{function}_v49(left, invocation_runtime_little_endian_v36());
 let nr = typed_forwarded_step_{function}_v49(right, invocation_runtime_little_endian_v36());
 forwarding_source_follow_{root}_v49(nl.cursor, nr.cursor, (fuel - 1) as nat);
 forwarding_projection_append_v46(nl.observations, typed_source_relocated_follow_{root}_v49(nl.cursor, (fuel - 1) as nat).observations);
 forwarding_projection_append_v46(nr.observations, typed_source_forwarded_follow_{root}_v49(nr.cursor, (fuel - 1) as nat).observations);
}} }}
proof fn forwarding_source_boundary_{root}_v49(left: TypedPrefixCursorV49, right: TypedPrefixCursorV49)
 requires forwarding_cursor_related_{function}_v49(left, right, invocation_runtime_little_endian_v36()), typed_source_relocated_boundary_{root}_v49(left).cursor.micro.state.valid,
 ensures forwarding_cursor_related_{function}_v49(typed_source_relocated_boundary_{root}_v49(left).cursor, typed_source_forwarded_boundary_{root}_v49(right).cursor, invocation_runtime_little_endian_v36()),
 forwarding_project_observations_v46(typed_source_relocated_boundary_{root}_v49(left).observations) == forwarding_project_observations_v46(typed_source_forwarded_boundary_{root}_v49(right).observations),
 typed_source_observations_valid_v49(typed_source_relocated_boundary_{root}_v49(left).observations), typed_source_observations_valid_v49(typed_source_forwarded_boundary_{root}_v49(right).observations),
 typed_source_relocated_boundary_{root}_v49(left).returned == typed_source_forwarded_boundary_{root}_v49(right).returned,
{{ if left.micro.state.pc >= 0 {{
 assert(invocation_byte_cut_{root}_v36(left.segment));
 forwarding_cursor_step_{function}_v49(left, right, invocation_runtime_little_endian_v36());
 forwarding_source_follow_{root}_v49(typed_relocated_step_{function}_v49(left, invocation_runtime_little_endian_v36()).cursor, typed_forwarded_step_{function}_v49(right, invocation_runtime_little_endian_v36()).cursor, {bound}nat);
}} }}
spec fn typed_final_related_{root}_v49(source: InvocationSourceByteStateV36, original: MemoryStateV30, prefix: TypedPrefixCursorV49, relocated: TypedPrefixCursorV49, actual: TypedPrefixCursorV49) -> bool {{
 typed_source_related_{root}_v49(source, original, prefix, relocated) && forwarding_cursor_related_{function}_v49(relocated, actual, invocation_runtime_little_endian_v36()) }}
proof fn typed_final_source_step_{root}_v49(source: InvocationSourceByteStateV36, original: MemoryStateV30, prefix: TypedPrefixCursorV49, relocated: TypedPrefixCursorV49, actual: TypedPrefixCursorV49)
 requires typed_final_related_{root}_v49(source, original, prefix, relocated, actual), invocation_paired_source_defined_{root}_v36(source, 1),
 ensures typed_final_related_{root}_v49(invocation_paired_source_step_{root}_v36(source).state, invocation_byte_boundary_{root}_v36(original).state, typed_source_prefix_boundary_{root}_v49(prefix).cursor, typed_source_relocated_boundary_{root}_v49(relocated).cursor, typed_source_forwarded_boundary_{root}_v49(actual).cursor),
 invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, typed_source_mapped_observations_v49(typed_source_forwarded_boundary_{root}_v49(actual).observations)),
 invocation_paired_source_step_{root}_v36(source).halted == (typed_source_forwarded_boundary_{root}_v49(actual).cursor.micro.state.pc < 0),
 invocation_byte_boundary_{root}_v36(original).returned == typed_source_forwarded_boundary_{root}_v49(actual).returned,
{{ typed_source_step_{root}_v49(source, original, prefix, relocated);
 forwarding_source_boundary_{root}_v49(relocated, actual);
 forwarding_source_projection_v49(typed_source_relocated_boundary_{root}_v49(relocated).observations, typed_source_forwarded_boundary_{root}_v49(actual).observations);
}}
spec fn typed_final_actual_step_{root}_v49(cursor: TypedPrefixCursorV49) -> CfgStepV26<TypedPrefixCursorV49, MemoryOperationObservationV30> {{
 let next = typed_source_forwarded_boundary_{root}_v49(cursor);
 CfgStepV26 {{ state: next.cursor, events: typed_source_mapped_observations_v49(next.observations), halted: next.cursor.micro.state.pc < 0 }} }}
spec fn typed_final_actual_initial_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> TypedSourceBoundaryV49 {{
 let original = invocation_paired_raw_initial_{root}_v36(arguments, external, execution);
 let prefix = typed_prefix_entry_{function}_v49(original);
 let relocated = typed_relocated_entry_{function}_v49(prefix.micro.state);
 typed_source_forwarded_follow_{root}_v49(typed_forwarded_begin_{function}_v49(relocated.micro.state), {bound}nat) }}
proof fn typed_final_source_initial_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37)
 requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution),
 ensures typed_final_related_{root}_v49(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), invocation_paired_ready_{root}_v36(arguments, external, execution).state, typed_source_middle_ready_{root}_v49(arguments, external, execution).cursor, typed_source_actual_ready_{root}_v49(arguments, external, execution).cursor, typed_final_actual_initial_{root}_v49(arguments, external, execution).cursor),
 typed_source_mapped_observations_v49(typed_final_actual_initial_{root}_v49(arguments, external, execution).observations).len() == 0,
{{ typed_source_initial_{root}_v49(arguments, external, execution);
 let original = invocation_paired_raw_initial_{root}_v36(arguments, external, execution);
 let prefix = typed_prefix_entry_{function}_v49(original);
 let relocated = typed_relocated_entry_{function}_v49(prefix.micro.state);
 let actual = typed_forwarded_begin_{function}_v49(relocated.micro.state);
 assert(forwarding_state_invariant_v46(relocated.micro.state));
 assert(forwarding_cursor_related_{function}_v49(relocated, actual, invocation_runtime_little_endian_v36()));
 forwarding_source_follow_{root}_v49(relocated, actual, {bound}nat);
 typed_source_observation_transport_{root}_v49(invocation_paired_ready_{root}_v36(arguments, external, execution).observations, typed_source_actual_ready_{root}_v49(arguments, external, execution).observations);
 forwarding_source_projection_v49(typed_source_actual_ready_{root}_v49(arguments, external, execution).observations, typed_final_actual_initial_{root}_v49(arguments, external, execution).observations);
}}
proof fn typed_final_source_trace_{root}_v49(source: InvocationSourceByteStateV36, original: MemoryStateV30, prefix: TypedPrefixCursorV49, relocated: TypedPrefixCursorV49, actual: TypedPrefixCursorV49, fuel: nat)
 requires typed_final_related_{root}_v49(source, original, prefix, relocated, actual), invocation_paired_source_defined_{root}_v36(source, fuel),
 ensures invocation_paired_observations_related_{root}_v39(cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), source, fuel).events, cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), actual, fuel).events),
 cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), source, fuel).halted == cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), actual, fuel).halted,
 typed_final_related_{root}_v49(cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), source, fuel).state, cfg_trace_v26(|s| invocation_paired_actual_step_{root}_v36(s), original, fuel).state, cfg_trace_v26(|s| typed_source_prefix_cut_step_{root}_v49(s), prefix, fuel).state, cfg_trace_v26(|s| typed_source_relocated_cut_step_{root}_v49(s), relocated, fuel).state, cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), actual, fuel).state),
 decreases fuel,
{{ if fuel > 0 {{
 typed_final_source_step_{root}_v49(source, original, prefix, relocated, actual);
 let next_source = invocation_paired_source_step_{root}_v36(source);
 let next_actual = typed_final_actual_step_{root}_v49(actual);
 if !next_source.halted {{
 typed_final_source_trace_{root}_v49(next_source.state, invocation_byte_boundary_{root}_v36(original).state, typed_source_prefix_boundary_{root}_v49(prefix).cursor, typed_source_relocated_boundary_{root}_v49(relocated).cursor, next_actual.state, (fuel - 1) as nat);
 invocation_paired_observations_append_{root}_v39(next_source.events, next_actual.events, cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), next_source.state, (fuel - 1) as nat).events, cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), next_actual.state, (fuel - 1) as nat).events);
 }} }} }}
proof fn typed_final_native_source_trace_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, fuel: nat)
 requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution), invocation_paired_source_defined_{root}_v36(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel),
 ensures invocation_paired_observations_related_{root}_v39(cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel).events, cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), typed_final_actual_initial_{root}_v49(arguments, external, execution).cursor, fuel).events),
 cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel).halted == cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), typed_final_actual_initial_{root}_v49(arguments, external, execution).cursor, fuel).halted,
 typed_final_related_{root}_v49(cfg_trace_v26(|s| invocation_paired_source_step_{root}_v36(s), invocation_source_initial_runtime_{root}_v36(arguments, external, execution), fuel).state, cfg_trace_v26(|s| invocation_paired_actual_step_{root}_v36(s), invocation_paired_ready_{root}_v36(arguments, external, execution).state, fuel).state, cfg_trace_v26(|s| typed_source_prefix_cut_step_{root}_v49(s), typed_source_middle_ready_{root}_v49(arguments, external, execution).cursor, fuel).state, cfg_trace_v26(|s| typed_source_relocated_cut_step_{root}_v49(s), typed_source_actual_ready_{root}_v49(arguments, external, execution).cursor, fuel).state, cfg_trace_v26(|s| typed_final_actual_step_{root}_v49(s), typed_final_actual_initial_{root}_v49(arguments, external, execution).cursor, fuel).state),
{{ typed_final_source_initial_{root}_v49(arguments, external, execution);
 typed_final_source_trace_{root}_v49(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), invocation_paired_ready_{root}_v36(arguments, external, execution).state, typed_source_middle_ready_{root}_v49(arguments, external, execution).cursor, typed_source_actual_ready_{root}_v49(arguments, external, execution).cursor, typed_final_actual_initial_{root}_v49(arguments, external, execution).cursor, fuel);
}}
"#
        );
        self.check(out)
    }
}
