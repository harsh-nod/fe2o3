//! Conditional source-conservation proof bodies over the original interpreter.

use super::*;
use std::fmt::Write as _;

pub(super) const SHARED: &str =
    include_str!("original_semantic_mir_source_constructor_laws_v81.vrs");

pub(super) fn emit(root: usize, fuels: &[usize], out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.reserve_storage(
        8 * size_of::<usize>() + 4 * size_of::<&()>() + 2 * size_of::<Result<()>>(),
    )?;
    write!(out, r#"#[verifier::spinoff_prover]
proof fn invocation_paired_source_heap_ready_{root}_v79(
    source: InvocationSourceByteStateV36, target: MemoryStateV30,
)
    requires invocation_paired_related_{root}_v36(source, target),
    ensures forall|allocation: MemoryAllocationV30| #[trigger] source.machine.memory.live.contains_key(allocation)
        ==> !invocation_private_allocation_v36(allocation),
{{
    hide(byte_memory_well_formed_v30);
    hide(byte_frame_runtime_well_formed_v30);
    hide(byte_execution_well_formed_v37);
    hide(byte_private_frames_live_v30);
    hide(private_generation_counters_valid_v30);
    hide(invocation_source_byte_state_well_formed_v36);
    hide(invocation_byte_heaps_related_v36);
    invocation_empty_private_map_has_no_private_source_v78(source.machine.memory, target.memory,
        invocation_source_byte_map_{root}_v36(source, target));
}}

#[verifier::spinoff_prover]
proof fn invocation_paired_source_defined_step_valid_{root}_v79(source: InvocationSourceByteStateV36)
    requires invocation_paired_source_defined_{root}_v36(source, 1),
    ensures invocation_paired_source_step_{root}_v36(source).state.machine.valid,
{{
    hide(invocation_paired_source_step_{root}_v36);
    hide(invocation_source_block_runtime_{root}_v36);
    hide(invocation_paired_source_defined_{root}_v36);
    reveal_with_fuel(invocation_paired_source_defined_{root}_v36, 2);
    if source.machine.pc < 0 {{
        reveal(invocation_paired_source_step_{root}_v36);
    }}
}}

#[verifier::spinoff_prover]
proof fn invocation_paired_source_preserved_{root}_v77(source: InvocationSourceByteStateV36, target: MemoryStateV30)
 requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),
 ensures invocation_paired_source_step_{root}_v36(source).state.machine.valid,
 invocation_paired_source_step_{root}_v36(source).state.machine.memory == source.machine.memory,
 invocation_paired_source_step_{root}_v36(source).state.machine.generations == source.machine.generations,
 invocation_paired_source_step_{root}_v36(source).state.machine.values.len() == source.machine.values.len(),
{{
 hide(invocation_source_observations_v39);
 hide(invocation_paired_related_{root}_v36);
 hide(invocation_paired_source_defined_{root}_v36);
 hide(invocation_source_byte_put_local_v36);
 hide(invocation_source_value_evaluate_v42);
 hide(invocation_source_byte_step_v36);
 hide(invocation_source_return_v36);
 hide(byte_memory_well_formed_v30);
 hide(byte_frame_runtime_well_formed_v30);
 hide(byte_execution_well_formed_v37);
 hide(byte_private_frames_live_v30);
 hide(private_generation_counters_valid_v30);
 hide(invocation_source_byte_state_well_formed_v36);
 hide(invocation_byte_heaps_related_v36);
 hide(byte_end_frame_v30);

 assert forall|s: InvocationSourceByteStateV36, local: int, value: MemoryValueV30|
  #![trigger invocation_source_byte_put_local_v36(s, local, value)]
  invocation_source_byte_put_local_v36(s, local, value).machine.memory == s.machine.memory &&
  invocation_source_byte_put_local_v36(s, local, value).machine.generations == s.machine.generations &&
  invocation_source_byte_put_local_v36(s, local, value).machine.frames == s.machine.frames &&
  invocation_source_byte_put_local_v36(s, local, value).machine.values.len() == s.machine.values.len() by {{
  invocation_source_put_local_preserves_heap_v78(s, local, value);
 }}
 assert forall|s: InvocationSourceByteStateV36, local: int, moved: bool, bits: int, root: int, instance: int, little_endian: bool|
  #![trigger invocation_source_value_evaluate_v42(s, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local, moved }}, bits }}, root, instance, little_endian)]
  invocation_source_value_evaluate_v42(s, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local, moved }}, bits }}, root, instance, little_endian).source.machine.memory == s.machine.memory &&
  invocation_source_value_evaluate_v42(s, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local, moved }}, bits }}, root, instance, little_endian).source.machine.generations == s.machine.generations &&
  invocation_source_value_evaluate_v42(s, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local, moved }}, bits }}, root, instance, little_endian).source.machine.frames == s.machine.frames &&
  invocation_source_value_evaluate_v42(s, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local, moved }}, bits }}, root, instance, little_endian).source.machine.values.len() == s.machine.values.len() by {{
  invocation_source_scalar_local_preserves_heap_v78(s, local, moved, bits, root, instance, little_endian);
 }}
 assert forall|s: InvocationSourceByteStateV36, local: int, value: int, bits: int, root: int, instance: int, little_endian: bool|
  #![trigger invocation_source_byte_step_v36(s, InvocationSourceByteEventV36::Transfer {{ destination: InvocationSourceByteDestinationV36::Local(local), value: InvocationSourceByteValueV36::Constant(value), bits }}, root, instance, little_endian)]
  invocation_source_byte_step_v36(s, InvocationSourceByteEventV36::Transfer {{ destination: InvocationSourceByteDestinationV36::Local(local), value: InvocationSourceByteValueV36::Constant(value), bits }}, root, instance, little_endian).machine.memory == s.machine.memory &&
  invocation_source_byte_step_v36(s, InvocationSourceByteEventV36::Transfer {{ destination: InvocationSourceByteDestinationV36::Local(local), value: InvocationSourceByteValueV36::Constant(value), bits }}, root, instance, little_endian).machine.generations == s.machine.generations &&
  invocation_source_byte_step_v36(s, InvocationSourceByteEventV36::Transfer {{ destination: InvocationSourceByteDestinationV36::Local(local), value: InvocationSourceByteValueV36::Constant(value), bits }}, root, instance, little_endian).machine.frames == s.machine.frames &&
  invocation_source_byte_step_v36(s, InvocationSourceByteEventV36::Transfer {{ destination: InvocationSourceByteDestinationV36::Local(local), value: InvocationSourceByteValueV36::Constant(value), bits }}, root, instance, little_endian).machine.values.len() == s.machine.values.len() by {{
  invocation_source_constant_transfer_preserves_heap_v78(s, local, value, bits, root, instance, little_endian);
 }}
 assert forall|s: InvocationSourceByteStateV36, begin: int, end: int, value: MemoryValueV30, destination: Option<InvocationSourceReturnDestinationV42>, continuation: int, little_endian: bool|
  #![trigger invocation_source_return_v36(s, begin, end, InvocationSourceValueV42::Carrier(value), destination, continuation, little_endian)]
  (forall|allocation: MemoryAllocationV30| #[trigger] s.machine.memory.live.contains_key(allocation) ==> !invocation_private_allocation_v36(allocation)) && (match destination {{ None => true, Some(destination) => destination.component.is_none() && destination.memory.is_none() && destination.descriptor.is_none() }}) implies invocation_source_return_v36(s, begin, end, InvocationSourceValueV42::Carrier(value), destination, continuation, little_endian).source.machine.memory == s.machine.memory &&
  invocation_source_return_v36(s, begin, end, InvocationSourceValueV42::Carrier(value), destination, continuation, little_endian).source.machine.generations == s.machine.generations &&
  invocation_source_return_v36(s, begin, end, InvocationSourceValueV42::Carrier(value), destination, continuation, little_endian).source.machine.values.len() == s.machine.values.len() by {{
  invocation_source_plain_return_preserves_heap_v78(s, begin, end, value, destination, continuation, little_endian);
 }}

"#).map_err(|_| out.error())?;
    for (instance, fuel) in fuels.iter().copied().enumerate() {
        out.budget.charge_work(1)?;
        if fuel != 0 {
            write!(
                out,
                " reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {fuel});\n"
            )
            .map_err(|_| out.error())?;
        }
    }
    write!(
        out,
        r#" invocation_paired_source_heap_ready_{root}_v79(source, target);
 invocation_paired_source_defined_step_valid_{root}_v79(source);
}}
"#
    )
    .map_err(|_| out.error())?;
    Ok(())
}
