use super::*;

impl RootSeed<'_, '_, '_, '_> {
    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        let Self {
            root,
            source_owner,
            source_pc,
            target_owner,
            target_pc,
            definitions,
            ..
        } = self;
        out.budget.charge_work(12)?;
        write!(out, "// Actual native scalar entry only; ContextIssue/prologue/calls and full-frame preservation remain separate.\nspec fn expanded_scalar_native_inputs_{root}_v300(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> bool {{\n invocation_runtime_execution_{root}_v37(execution) && byte_memory_well_formed_v30(external)\n && byte_native_view_inputs_v38(external, arguments) && invocation_native_provenance_v39(external, arguments)\n && arguments.len() == {}", self.arguments.len()).map_err(|_| out.error())?;
        for (ordinal, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(
                out,
                " && invocation_source_byte_value_typed_v36(arguments[{ordinal}], {})",
                argument.bits
            )
            .map_err(|_| out.error())?;
        }
        write!(out, r#"
}}
spec fn expanded_scalar_raw_initial_{root}_v300(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, little_endian: bool) -> MemoryStateV30 {{
 let values = Seq::new({definitions}nat, |i: int| MemoryValueV30::Undefined)"#).map_err(|_| out.error())?;
        for (ordinal, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(1)?;
            if let Some(definition) = argument.target {
                write!(out, ".update({definition}int, arguments[{ordinal}])")
                    .map_err(|_| out.error())?;
            }
        }
        write!(out, r#";
 let admitted = expanded_scalar_native_inputs_{root}_v300(arguments, external, execution);
 let memory = if admitted {{ ByteMemoryV30 {{ view_contracts: byte_target_view_contracts_1_v38(little_endian), ..external }} }} else {{ external }};
 MemoryStateV30 {{ pc: {target_pc}, values, memory, generations: Map::empty(), frames: byte_root_frame_with_execution_v37({target_owner}, execution), valid: admitted }}
}}
spec fn expanded_scalar_source_entry_{root}_v300(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, little_endian: bool) -> InvocationSourceMicroStateV36 {{
 invocation_source_micro_begin_{root}_0_v36(invocation_source_byte_initial_{root}_v36(arguments, external, execution, little_endian))
}}
spec fn expanded_scalar_target_entry_{root}_v300(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, little_endian: bool) -> MemoryMicroStateV30 {{
 byte_micro_begin_{root}_v30(expanded_scalar_raw_initial_{root}_v300(arguments, external, execution, little_endian))
}}
spec fn expanded_scalar_entry_projections_{root}_v300(s: InvocationSourceMicroStateV36, t: MemoryMicroStateV30, arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, little_endian: bool) -> bool {{
 arguments.len() == {} && invocation_source_active_{root}_0_v36(s.source)
 && invocation_source_byte_state_well_formed_v36(s.source) && s.source.machine.valid
 && s.source.machine.pc == {source_pc} && s.next_statement == 0 && s.observations.len() == 0
 && s.source.slots == Map::empty() && s.source.objects == Map::empty()
 && s.source.machine.memory.live == external.live && s.source.machine.generations == Map::empty()
 && t.state.valid && t.state.pc == {target_pc} && byte_inputs_{root}_v55(t.state, little_endian)
 && t == byte_micro_begin_{root}_v30(t.state) && t.observations.len() == 0
 && t.state.memory.live == external.live && t.state.generations == Map::empty()
 && s.source.machine.frames.active.len() == 1 && s.source.machine.frames.active[0].owner == {source_owner}
 && t.state.frames.active.len() == 1 && t.state.frames.active[0].owner == {target_owner}
 && s.source.machine.frames.active[0].invocation == 0 && t.state.frames.active[0].invocation == 0
 && s.source.machine.frames.execution == t.state.frames.execution
 && s.source.machine.frames.next_invocation == 1 && t.state.frames.next_invocation == 1
"#, self.arguments.len()).map_err(|_| out.error())?;
        for (ordinal, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(2)?;
            write!(out, " && 0 <= {} < s.source.machine.values.len() && s.source.machine.values[{}] == arguments[{ordinal}]\n", argument.local, argument.local).map_err(|_| out.error())?;
            if let Some(definition) = argument.target {
                write!(out, " && 0 <= {definition} < t.state.values.len() && t.state.values[{definition}] == s.source.machine.values[{}]\n", argument.local).map_err(|_| out.error())?;
            }
        }
        write!(out, r#"}}
proof fn expanded_scalar_entry_seed_{root}_v300(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, little_endian: bool)
 requires expanded_scalar_native_inputs_{root}_v300(arguments, external, execution),
 ensures expanded_scalar_entry_projections_{root}_v300(
 expanded_scalar_source_entry_{root}_v300(arguments, external, execution, little_endian),
 expanded_scalar_target_entry_{root}_v300(arguments, external, execution, little_endian),
 arguments, external, little_endian),
{{
 reveal(expanded_scalar_native_inputs_{root}_v300);
 let source_memory = ByteMemoryV30 {{ view_contracts: invocation_source_view_contracts_0_v39(little_endian), ..external }};
 let target_memory = ByteMemoryV30 {{ view_contracts: byte_target_view_contracts_1_v38(little_endian), ..external }};
 invocation_native_initial_memory_invariants_v77(source_memory, arguments, byte_root_frame_with_execution_v37({source_owner}, execution));
 invocation_native_initial_memory_invariants_v77(target_memory, arguments, byte_root_frame_with_execution_v37({target_owner}, execution));
 assert forall|i: int| 0 <= i < arguments.len() implies invocation_source_external_argument_v36(arguments[i]) by {{ }}
 assert forall|logical: InvocationSourceLogicalV38, begin: int, end: int|
 logical.execution_pending == Map::empty() implies
 #[trigger] invocation_source_logical_clear_v38(logical, begin, end).execution_pending == Map::empty() by {{
 assert(invocation_source_logical_clear_v38(logical, begin, end).execution_pending =~= Map::empty());
 }}
 assert forall|logical: InvocationSourceLogicalV38, local: int|
 logical.execution_pending == Map::empty() implies
 #[trigger] invocation_source_logical_write_v38(logical, local).execution_pending == Map::empty() by {{
 assert(invocation_source_logical_write_v38(logical, local).execution_pending =~= Map::empty());
 }}
 let source = invocation_source_byte_initial_{root}_v36(arguments, external, execution, little_endian);
 reveal(invocation_source_byte_initial_{root}_v36);
 reveal(invocation_source_enter_{root}_0_v36);
 assert(source.machine.valid);
 assert(invocation_source_byte_state_well_formed_v36(source));
 reveal(invocation_source_active_{root}_0_v36);
 assert(invocation_source_active_{root}_0_v36(source));
 reveal(expanded_scalar_source_entry_{root}_v300);
 reveal(invocation_source_micro_begin_{root}_0_v36);
 reveal(expanded_scalar_raw_initial_{root}_v300);
 reveal(expanded_scalar_target_entry_{root}_v300);
 reveal(byte_micro_begin_{root}_v30);
 reveal(byte_inputs_{root}_v55);
 reveal(expanded_scalar_entry_projections_{root}_v300);
}}
"#).map_err(|_| out.error())?;
        self.check(out)
    }
}
