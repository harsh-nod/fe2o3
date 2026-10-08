//! Coupled actual interpreters, conditional only on the two authenticated
//! prestates. This neither establishes root reachability nor a full frame map.
use super::*;

impl CheckedTargetSegment<'_, '_, '_, '_> {
    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(self.program, self.target, out)?;
        out.budget.charge_work(24)?;
        let Self {
            root,
            instance,
            block,
            statement,
            source_pc,
            destination,
            source_operands: [source_left, source_right],
            target_block,
            target_operation,
            target_prefix,
            checked,
            ..
        } = self;
        let [target_left, target_right] = checked.operands;
        let [value, overflow] = checked.results;
        // Formatting the optional next coordinate directly avoids an allocation.
        write!(out, "// Retained single Checked Add segment; partial projections, no initialization or complete-frame claim.\nspec fn checked_target_next_{root}_{instance}_{block}_{statement}_v298() -> int {{ ")
            .map_err(|_| out.error())?;
        match self.target_next {
            Some(next) => write!(out, "{next}"),
            None => write!(out, "-1"),
        }
        .map_err(|_| out.error())?;
        write!(out, r#" }}
proof fn checked_target_actual_step_{root}_{instance}_{block}_{statement}_v298(
 t: MemoryMicroStateV30, left: int, right: int, little_endian: bool,
)
 requires t.state.valid, byte_inputs_{root}_v55(t.state, little_endian),
 t.state.pc == {target_block}, t.next_operation == {target_operation},
 t.observations.len() == {target_prefix},
 0 <= {target_left} < t.state.values.len(), 0 <= {target_right} < t.state.values.len(),
 0 <= {value} < t.state.values.len(), 0 <= {overflow} < t.state.values.len(),
 t.state.values[{target_left}] == MemoryValueV30::Scalar(left),
 t.state.values[{target_right}] == MemoryValueV30::Scalar(right),
 0 <= left < 4294967296, 0 <= right < 4294967296,
 ensures ({{ let n = byte_micro_step_{root}_v30(t, little_endian);
 n.next.state.valid && n.next.state.pc == t.state.pc
 && n.next.state.values == t.state.values.update({value}int, MemoryValueV30::Scalar((left + right) % 4294967296))
     .update({overflow}int, MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }}))
 && n.next.state.memory == t.state.memory && n.next.state.frames == t.state.frames
 && n.next.state.generations == t.state.generations
 && n.next.next_operation == checked_target_next_{root}_{instance}_{block}_{statement}_v298()
 && n.next.observations.len() == t.observations.len() + 1
 && n.next.observations.take(t.observations.len() as int) == t.observations
 && n.observation.before == t.state && n.observation.after == n.next.state
 && n.observation.valid_before && n.observation.valid_after
 && n.observation.effect == MemoryOperationEffectV30::Pure }}),
{{
 reveal(byte_micro_step_{root}_v30);
 reveal(byte_operation_{root}_{target_operation}_v30);
 reveal(memory_value_modulus_v30);
 reveal(byte_result_v55);
 reveal(byte_micro_result_v55);
 let n = byte_micro_step_{root}_v30(t, little_endian);
 assert(n.next.observations.take(t.observations.len() as int) =~= t.observations);
}}
spec fn checked_actual_segment_inputs_{root}_{instance}_{block}_{statement}_v298(
 s: InvocationSourceMicroStateV36, t: MemoryMicroStateV30,
 left: int, right: int, little_endian: bool,
) -> bool {{
 invocation_source_active_{root}_{instance}_v36(s.source)
 && invocation_source_byte_state_well_formed_v36(s.source) && s.source.machine.valid
 && s.source.machine.pc == {source_pc} && s.next_statement == {statement}
 && s.next_statement == s.observations.len()
 && 0 <= {destination} < s.source.machine.values.len() && !s.source.objects.contains_key({destination})
 && 0 <= {source_left} < s.source.machine.values.len() && 0 <= {source_right} < s.source.machine.values.len()
 && s.source.machine.values[{source_left}] == MemoryValueV30::Scalar(left)
 && s.source.machine.values[{source_right}] == MemoryValueV30::Scalar(right)
 && 0 <= left < 4294967296 && 0 <= right < 4294967296
 && t.state.valid && byte_inputs_{root}_v55(t.state, little_endian)
 && t.state.pc == {target_block} && t.next_operation == {target_operation}
 && t.observations.len() == {target_prefix}
 && 0 <= {target_left} < t.state.values.len() && 0 <= {target_right} < t.state.values.len()
 && 0 <= {value} < t.state.values.len() && 0 <= {overflow} < t.state.values.len()
 && t.state.values[{target_left}] == s.source.machine.values[{source_left}]
 && t.state.values[{target_right}] == s.source.machine.values[{source_right}]
}}
spec fn checked_actual_segment_results_{root}_{instance}_{block}_{statement}_v298(
 before_source: InvocationSourceMicroStateV36, before_target: MemoryMicroStateV30,
 after_source: InvocationSourceMicroStateV36, after_target: MemoryMicroResultV30,
 left: int, right: int,
) -> bool {{
 after_source.source.machine.valid && invocation_source_active_{root}_{instance}_v36(after_source.source)
 && invocation_source_byte_state_well_formed_v36(after_source.source)
 && after_source.source.machine.pc == before_source.source.machine.pc
 && after_source.next_statement == before_source.next_statement + 1
 && after_source.observations.len() == before_source.observations.len() + 1
 && after_source.observations.take(before_source.observations.len() as int) == before_source.observations
 && after_source.source.machine.memory == before_source.source.machine.memory
 && after_source.source.machine.frames == before_source.source.machine.frames
 && after_source.source.machine.generations == before_source.source.machine.generations
 && after_source.source.slots == before_source.source.slots && after_source.source.objects == before_source.source.objects
 && checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296(before_source.source, after_source.source, left, right)
 && after_source.source.logical.aggregates.contains_key({destination})
 && after_target.next.state.valid && after_target.next.state.pc == before_target.state.pc
 && after_target.next.state.values.len() == before_target.state.values.len()
 && after_target.next.state.values[{value}] == after_source.source.logical.aggregates[{destination}].leaves[seq![0int]]
 && after_target.next.state.values[{overflow}] == after_source.source.logical.aggregates[{destination}].leaves[seq![1int]]
 && after_target.next.state.values[{value}] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && after_target.next.state.values[{overflow}] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }})
 && (forall|i: int| 0 <= i < before_target.state.values.len() && i != {value} && i != {overflow}
     ==> #[trigger] after_target.next.state.values[i] == before_target.state.values[i])
 && after_target.next.state.memory == before_target.state.memory && after_target.next.state.frames == before_target.state.frames
 && after_target.next.state.generations == before_target.state.generations
 && after_target.next.next_operation == checked_target_next_{root}_{instance}_{block}_{statement}_v298()
 && after_target.next.observations.len() == before_target.observations.len() + 1
 && after_target.next.observations.take(before_target.observations.len() as int) == before_target.observations
 && after_target.observation.before == before_target.state && after_target.observation.after == after_target.next.state
 && after_target.observation.effect == MemoryOperationEffectV30::Pure
}}
proof fn checked_source_projection_{root}_{instance}_{block}_{statement}_v331(
 s: InvocationSourceMicroStateV36, left: int, right: int, little_endian: bool,
)
 requires invocation_source_active_{root}_{instance}_v36(s.source)
 && invocation_source_byte_state_well_formed_v36(s.source) && s.source.machine.valid
 && s.source.machine.pc == {source_pc} && s.next_statement == {statement}
 && s.next_statement == s.observations.len()
 && 0 <= {destination} < s.source.machine.values.len() && !s.source.objects.contains_key({destination})
 && 0 <= {source_left} < s.source.machine.values.len() && 0 <= {source_right} < s.source.machine.values.len()
 && s.source.machine.values[{source_left}] == MemoryValueV30::Scalar(left)
 && s.source.machine.values[{source_right}] == MemoryValueV30::Scalar(right)
 && 0 <= left < 4294967296 && 0 <= right < 4294967296,
 ensures ({{ let a = invocation_source_micro_step_{root}_{instance}_v36(s, little_endian);
 a.source.machine.valid && invocation_source_active_{root}_{instance}_v36(a.source)
 && invocation_source_byte_state_well_formed_v36(a.source)
 && a.source.machine.pc == s.source.machine.pc
 && a.next_statement == s.next_statement + 1
 && a.observations.len() == s.observations.len() + 1
 && a.observations.take(s.observations.len() as int) == s.observations
 && a.source.machine.memory == s.source.machine.memory
 && a.source.machine.frames == s.source.machine.frames
 && a.source.machine.generations == s.source.machine.generations
 && a.source.slots == s.source.slots && a.source.objects == s.source.objects
 && a.source.logical.aggregates.contains_key({destination})
 && a.source.logical.aggregates[{destination}].leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && a.source.logical.aggregates[{destination}].leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }})
 && checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296(s.source, a.source, left, right) }}),
{{
 hide(invocation_source_micro_step_{root}_{instance}_v36);
 hide(invocation_source_byte_state_well_formed_v36);
 hide(invocation_source_active_{root}_{instance}_v36);
 hide(checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296);
 checked_add_actual_micro_step_{root}_{instance}_{block}_{statement}_v293(s, left, right, little_endian);
 checked_add_actual_demanded_step_{root}_{instance}_{block}_{statement}_v296(s, left, right, little_endian);
}}
proof fn checked_target_projection_{root}_{instance}_{block}_{statement}_v331(
 t: MemoryMicroStateV30, left: int, right: int, little_endian: bool,
)
 requires t.state.valid, byte_inputs_{root}_v55(t.state, little_endian),
 t.state.pc == {target_block}, t.next_operation == {target_operation},
 t.observations.len() == {target_prefix},
 0 <= {target_left} < t.state.values.len(), 0 <= {target_right} < t.state.values.len(),
 0 <= {value} < t.state.values.len(), 0 <= {overflow} < t.state.values.len(),
 t.state.values[{target_left}] == MemoryValueV30::Scalar(left),
 t.state.values[{target_right}] == MemoryValueV30::Scalar(right),
 0 <= left < 4294967296, 0 <= right < 4294967296,
 ensures ({{ let n = byte_micro_step_{root}_v30(t, little_endian);
 n.next.state.valid && n.next.state.pc == t.state.pc
 && n.next.state.values.len() == t.state.values.len()
 && n.next.state.values[{value}] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && n.next.state.values[{overflow}] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }})
 && (forall|i: int| 0 <= i < t.state.values.len() && i != {value} && i != {overflow}
     ==> #[trigger] n.next.state.values[i] == t.state.values[i])
 && n.next.state.memory == t.state.memory && n.next.state.frames == t.state.frames
 && n.next.state.generations == t.state.generations
 && n.next.next_operation == checked_target_next_{root}_{instance}_{block}_{statement}_v298()
 && n.next.observations.len() == t.observations.len() + 1
 && n.next.observations.take(t.observations.len() as int) == t.observations
 && n.observation.before == t.state && n.observation.after == n.next.state
 && n.observation.effect == MemoryOperationEffectV30::Pure }}),
{{
 hide(byte_micro_step_{root}_v30);
 hide(byte_inputs_{root}_v55);
 hide(checked_target_next_{root}_{instance}_{block}_{statement}_v298);
 checked_target_actual_step_{root}_{instance}_{block}_{statement}_v298(t, left, right, little_endian);
 let n = byte_micro_step_{root}_v30(t, little_endian);
 assert({value}int != {overflow}int);
 assert(0 <= {value}int < t.state.values.len() && 0 <= {overflow}int < t.state.values.len());
 assert(n.next.state.values == t.state.values.update({value}int, MemoryValueV30::Scalar((left + right) % 4294967296))
     .update({overflow}int, MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }})));
 assert(n.next.state.values.len() == t.state.values.len());
 assert(n.next.state.values[{value}] == MemoryValueV30::Scalar((left + right) % 4294967296));
 assert(n.next.state.values[{overflow}] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }}));
 assert forall|i: int| 0 <= i < t.state.values.len() && i != {value} && i != {overflow}
     implies #[trigger] n.next.state.values[i] == t.state.values[i] by {{ }}
}}
proof fn checked_actual_source_target_step_{root}_{instance}_{block}_{statement}_v298(
 s: InvocationSourceMicroStateV36, t: MemoryMicroStateV30,
 left: int, right: int, little_endian: bool,
)
 requires checked_actual_segment_inputs_{root}_{instance}_{block}_{statement}_v298(s, t, left, right, little_endian),
 ensures checked_actual_segment_results_{root}_{instance}_{block}_{statement}_v298(s, t,
     invocation_source_micro_step_{root}_{instance}_v36(s, little_endian),
     byte_micro_step_{root}_v30(t, little_endian), left, right),
{{
 hide(invocation_source_micro_step_{root}_{instance}_v36);
 hide(byte_micro_step_{root}_v30);
 hide(invocation_source_byte_state_well_formed_v36);
 hide(invocation_source_active_{root}_{instance}_v36);
 hide(byte_inputs_{root}_v55);
 hide(checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296);
 hide(checked_target_next_{root}_{instance}_{block}_{statement}_v298);
 reveal(checked_actual_segment_inputs_{root}_{instance}_{block}_{statement}_v298);
 checked_source_projection_{root}_{instance}_{block}_{statement}_v331(s, left, right, little_endian);
 checked_target_projection_{root}_{instance}_{block}_{statement}_v331(t, left, right, little_endian);
 reveal(checked_actual_segment_results_{root}_{instance}_{block}_{statement}_v298);
}}
proof fn checked_actual_source_prefix_target_step_{root}_{instance}_{block}_{statement}_v298(
 s: InvocationSourceMicroStateV36, fuel: nat, t: MemoryMicroStateV30,
 left: int, right: int, little_endian: bool,
)
 requires checked_actual_segment_inputs_{root}_{instance}_{block}_{statement}_v298(
     invocation_source_micro_run_{root}_{instance}_v36(s, fuel, little_endian), t, left, right, little_endian),
 ensures checked_actual_segment_results_{root}_{instance}_{block}_{statement}_v298(
     invocation_source_micro_run_{root}_{instance}_v36(s, fuel, little_endian), t,
     invocation_source_micro_run_{root}_{instance}_v36(s, fuel + 1, little_endian),
     byte_micro_step_{root}_v30(t, little_endian), left, right),
{{
 hide(invocation_source_micro_step_{root}_{instance}_v36);
 let p = invocation_source_micro_run_{root}_{instance}_v36(s, fuel, little_endian);
 checked_actual_source_target_step_{root}_{instance}_{block}_{statement}_v298(p, t, left, right, little_endian);
 invocation_source_micro_run_composes_{root}_{instance}_v292(s, fuel, 1, little_endian);
 reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, 2);
 assert(invocation_source_micro_run_{root}_{instance}_v36(p, 1, little_endian)
     == invocation_source_micro_step_{root}_{instance}_v36(p, little_endian));
}}
"#).map_err(|_| out.error())?;
        self.check(self.program, self.target, out)
    }
}
