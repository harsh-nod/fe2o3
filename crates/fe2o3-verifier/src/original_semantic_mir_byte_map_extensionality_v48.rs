//! Source effect maps depend on exact allocation carriers and memory, not on
//! unrelated SSA registers. This lemma does not create an execution state.
use super::*;

impl SourceByteBindings<'_, '_, '_> {
    pub(in super::super) fn emit_carrier_extensionality_v48(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            if out.budget.storage() < self.required {
                return Err(Resource::Accounting.into());
            }
            out.budget.reserve_storage(8 * size_of::<usize>() + 8 * size_of::<&()>())?;
            for (root, range) in self.roots.iter().enumerate() {
                out.budget.charge_work(4)?;
                write!(
                    out,
                    "spec fn invocation_source_byte_carriers_equal_{root}_v48(a: MemoryStateV30, b: MemoryStateV30) -> bool {{ a.values.len() == {} && b.values.len() == {} && a.memory == b.memory",
                    self.definitions, self.definitions
                )
                .map_err(|_| out.error())?;
                for row in &self.rows[range.clone()] {
                    out.budget.charge_work(1)?;
                    let definition = row.definition;
                    write!(out, " && a.values[{definition}] == b.values[{definition}]")
                        .map_err(|_| out.error())?;
                }
                write!(out, " }}\n").map_err(|_| out.error())?;
                out.budget.charge_work(8)?;
                write!(out, "spec fn invocation_source_observation_carriers_equal_{root}_v49(a: MemoryOperationObservationV30, b: MemoryOperationObservationV30) -> bool {{\n invocation_source_byte_carriers_equal_{root}_v48(a.before, b.before) && invocation_source_byte_carriers_equal_{root}_v48(a.after, b.after)\n && a.before.generations == b.before.generations && a.after.generations == b.after.generations\n && a.before.frames == b.before.frames && a.after.frames == b.after.frames\n && a.effect == b.effect && a.before.valid == b.before.valid && a.after.valid == b.after.valid\n && byte_observation_snapshots_valid_v39(a) && byte_observation_snapshots_valid_v39(b)\n && (match a.effect {{ MemoryOperationEffectV30::Trap => a.before.pc >= 0 && b.before.pc >= 0 && a.after == (MemoryStateV30 {{ pc: -2, ..a.before }}) && b.after == (MemoryStateV30 {{ pc: -2, ..b.before }}),\n MemoryOperationEffectV30::Refused | MemoryOperationEffectV30::Pure | MemoryOperationEffectV30::Allocate {{ .. }} | MemoryOperationEffectV30::Read {{ .. }} | MemoryOperationEffectV30::Write {{ .. }} | MemoryOperationEffectV30::Copy {{ .. }} | MemoryOperationEffectV30::TagRead {{ .. }} => true }})\n}}\n")
                    .map_err(|_| out.error())?;
                write!(out, "proof fn invocation_source_observation_extensionality_{root}_v49(source: InvocationSourceEffectObservationV39, a: MemoryOperationObservationV30, b: MemoryOperationObservationV30)\n requires invocation_source_observation_carriers_equal_{root}_v49(a, b),\n ensures invocation_source_byte_map_valid_{root}_v36(source.before, a.before) == invocation_source_byte_map_valid_{root}_v36(source.before, b.before),\n invocation_source_byte_map_valid_{root}_v36(source.after, a.after) == invocation_source_byte_map_valid_{root}_v36(source.after, b.after),\n invocation_observed_effect_related_v39(source, a, invocation_source_byte_map_{root}_v36(source.before, a.before), invocation_source_byte_map_{root}_v36(source.after, a.after)) == invocation_observed_effect_related_v39(source, b, invocation_source_byte_map_{root}_v36(source.before, b.before), invocation_source_byte_map_{root}_v36(source.after, b.after)),\n{{ invocation_source_byte_map_extensionality_{root}_v48(source.before, a.before, b.before);\n invocation_source_byte_map_extensionality_{root}_v48(source.after, a.after, b.after);\n assert(byte_state_memory_well_formed_v30(a.before) == byte_state_memory_well_formed_v30(b.before));\n assert(byte_state_memory_well_formed_v30(a.after) == byte_state_memory_well_formed_v30(b.after));\n}}\n")
                    .map_err(|_| out.error())?;
                write!(out, "proof fn invocation_source_observations_extensionality_{root}_v49(source: Seq<InvocationSourceEffectObservationV39>, a: Seq<MemoryOperationObservationV30>, b: Seq<MemoryOperationObservationV30>)\n requires a.len() == b.len(), forall|i: int| 0 <= i < a.len() ==> invocation_source_observation_carriers_equal_{root}_v49(a[i], b[i]),\n ensures invocation_paired_observations_related_{root}_v39(source, a) == invocation_paired_observations_related_{root}_v39(source, b),\n{{ if source.len() == a.len() {{ assert forall|i: int| 0 <= i < source.len() implies\n (invocation_source_byte_map_valid_{root}_v36(source[i].before, a[i].before) && invocation_source_byte_map_valid_{root}_v36(source[i].after, a[i].after) && invocation_observed_effect_related_v39(source[i], a[i], invocation_source_byte_map_{root}_v36(source[i].before, a[i].before), invocation_source_byte_map_{root}_v36(source[i].after, a[i].after)))\n == (invocation_source_byte_map_valid_{root}_v36(source[i].before, b[i].before) && invocation_source_byte_map_valid_{root}_v36(source[i].after, b[i].after) && invocation_observed_effect_related_v39(source[i], b[i], invocation_source_byte_map_{root}_v36(source[i].before, b[i].before), invocation_source_byte_map_{root}_v36(source[i].after, b[i].after))) by {{ invocation_source_observation_extensionality_{root}_v49(source[i], a[i], b[i]); }} }} }}\n")
                    .map_err(|_| out.error())?;
                write!(
                    out,
                    "proof fn invocation_source_byte_map_extensionality_{root}_v48(source: InvocationSourceByteStateV36, a: MemoryStateV30, b: MemoryStateV30)\n requires invocation_source_byte_carriers_equal_{root}_v48(a, b),\n ensures invocation_source_byte_map_{root}_v36(source, a) == invocation_source_byte_map_{root}_v36(source, b),\n invocation_source_byte_map_valid_{root}_v36(source, a) == invocation_source_byte_map_valid_{root}_v36(source, b),\n{{\n assert(invocation_source_byte_map_{root}_v36(source, a).private =~= invocation_source_byte_map_{root}_v36(source, b).private);\n}}\n"
                )
                .map_err(|_| out.error())?;
            }
            Ok(())
        })
    }
}
