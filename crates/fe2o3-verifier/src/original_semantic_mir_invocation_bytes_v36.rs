//! Source activation and byte correspondence use the shared concrete memory
//! vocabulary. Static slot rows select descriptors, never dynamic identities.
//! These definitions do not replace the mandatory all-operation step relation.
pub(super) const INVOCATION_BYTES_V36: &str = r#"
struct InvocationByteBindingV36 {
    target: MemoryPointerV30,
    extent: int,
    alignment: int,
}

struct InvocationByteMapV36 {
    private: Map<MemoryAllocationV30, InvocationByteBindingV36>,
}

// Generated only from the authenticated allocation/frame visitor. The owner is
// an original MIR declaration; site.function is a canonical KIR coordinate.
struct InvocationSourceSlotV36 {
    root: int,
    instance: int,
    owner: int,
    local: int,
    semantic_type: int,
    source_generation: Option<int>,
    storage_layout: Option<int>,
    site: MemorySourceOperationV30,
    extent: int,
    alignment: int,
}

open spec fn invocation_private_allocation_v36(a: MemoryAllocationV30) -> bool {
    match a {
        MemoryAllocationV30::Private { owner: _, invocation: _, site: _, generation: _ } => true,
        MemoryAllocationV30::External { identity: _, generation: _ } => false,
    }
}

open spec fn invocation_binding_disjoint_v36(
    a: InvocationByteBindingV36, b: InvocationByteBindingV36,
) -> bool {
    a.target.allocation != b.target.allocation
        || a.target.byte_offset + a.extent <= b.target.byte_offset
        || b.target.byte_offset + b.extent <= a.target.byte_offset
}

open spec fn invocation_view_registries_related_v40(
    source: ByteMemoryV30, target: ByteMemoryV30,
) -> bool {
    source.view_contracts == invocation_source_view_contracts_0_v39(invocation_runtime_little_endian_v36())
        && target.view_contracts == byte_target_view_contracts_1_v38(invocation_runtime_little_endian_v36())
        && byte_view_contracts_shape_v38(source.view_contracts)
        && byte_view_contracts_shape_v38(target.view_contracts)
}

open spec fn invocation_byte_heaps_related_v36(
    source: ByteMemoryV30, target: ByteMemoryV30, map: InvocationByteMapV36,
) -> bool {
    byte_memory_well_formed_v30(source) && byte_memory_well_formed_v30(target)
    && invocation_view_registries_related_v40(source, target)
    && (forall|a: MemoryAllocationV30| !invocation_private_allocation_v36(a) ==>
        (source.live.contains_key(a) == target.live.contains_key(a))
        && (source.live.contains_key(a) ==>
            invocation_external_bytes_related_v38(source.live[a], target.live[a], map, source, target)))
    && (forall|a: MemoryAllocationV30| map.private.contains_key(a) <==>
        source.live.contains_key(a) && invocation_private_allocation_v36(a))
    && (forall|a: MemoryAllocationV30| map.private.contains_key(a) ==> {
        let binding = map.private[a];
        invocation_private_allocation_v36(binding.target.allocation)
            && binding.extent == source.live[a].bytes.len()
            && binding.alignment == source.live[a].base_alignment
            && byte_range_aligned_v30(target, binding.target, binding.extent, binding.alignment)
            && (forall|i: int| 0 <= i < binding.extent && source.live[a].initialized[i] ==>
                target.live[binding.target.allocation].initialized[binding.target.byte_offset + i]
                && invocation_byte_token_related_v37(source.live[a].bytes[i],
                    target.live[binding.target.allocation].bytes[binding.target.byte_offset + i], map, source, target))
            && (forall|at: int| source.live[a].relocations.contains_key(at) ==> {
                let source_cell = source.live[a].relocations[at];
                let target_at = binding.target.byte_offset + at;
                target.live[binding.target.allocation].relocations.contains_key(target_at)
                && invocation_relocation_related_v37(source_cell,
                    target.live[binding.target.allocation].relocations[target_at], map, source, target)
            })
    })
    && (forall|a: MemoryAllocationV30, b: MemoryAllocationV30|
        map.private.contains_key(a) && map.private.contains_key(b) && a != b ==>
            invocation_binding_disjoint_v36(map.private[a], map.private[b]))
}

// target.frames are physical allocation owners, not V36's separate logical
// source-cut frame annotations. A logical return does not pop this target stack.
open spec fn invocation_byte_states_related_v36(
    source: MemoryStateV30, target: MemoryStateV30, map: InvocationByteMapV36,
) -> bool {
    source.valid && target.valid
        && byte_frame_runtime_well_formed_v30(source.frames)
        && byte_frame_runtime_well_formed_v30(target.frames)
        && source.frames.execution == target.frames.execution
        && byte_private_frames_live_v30(source.memory, source.frames)
        && byte_private_frames_live_v30(target.memory, target.frames)
        && private_generation_counters_valid_v30(source.generations, source.memory)
        && private_generation_counters_valid_v30(target.generations, target.memory)
        && invocation_byte_heaps_related_v36(source.memory, target.memory, map)
}

open spec fn invocation_pointer_coordinates_related_v40(
    source: MemoryPointerV30, target: MemoryPointerV30, map: InvocationByteMapV36,
) -> bool {
    if invocation_private_allocation_v36(source.allocation) {
        map.private.contains_key(source.allocation)
            && 0 <= source.byte_offset <= map.private[source.allocation].extent
            && target.allocation == map.private[source.allocation].target.allocation
            && target.byte_offset == map.private[source.allocation].target.byte_offset + source.byte_offset
    } else { source.allocation == target.allocation && source.byte_offset == target.byte_offset }
}

open spec fn invocation_pointer_related_v36(
    source: MemoryPointerV30, target: MemoryPointerV30, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    invocation_pointer_coordinates_related_v40(source, target, map)
        && invocation_pointer_views_related_v40(source, target, map, source_memory, target_memory)
}

// The generated predicate is closed over the original owner-backed endpoint
// census. Numeric type/layout equality, equal bytes and equal epochs are not
// source/target correspondence evidence.
open spec fn invocation_tag_guards_related_v40(
    source: MemoryTagGuardV38, target: MemoryTagGuardV38, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    invocation_view_registries_related_v40(source_memory, target_memory)
        && source.owner == source_memory.view_contracts.owner
        && target.owner == target_memory.view_contracts.owner
        && source_memory.view_contracts.rows.contains_key(source.contract)
        && target_memory.view_contracts.rows.contains_key(target.contract)
        && invocation_source_target_tag_pair_0_1_v40(source.contract, target.contract)
        && source.variant == target.variant
        && source.epochs.len() == source_memory.view_contracts.rows[source.contract].tag_width
        && target.epochs.len() == target_memory.view_contracts.rows[target.contract].tag_width
        && invocation_pointer_coordinates_related_v40(
            MemoryPointerV30 { allocation: source.allocation, byte_offset: source.object_start, view: None },
            MemoryPointerV30 { allocation: target.allocation, byte_offset: target.object_start, view: None }, map)
        && (byte_tag_guard_current_v38(source_memory, source)
            == byte_tag_guard_current_v38(target_memory, target))
}

// Every enclosing guard is retained in both directions. Repeated equivalent
// guards may differ in count, but no independent obligation can disappear.
// Epochs are compared only to their own execution's current heap.
open spec fn invocation_pointer_views_related_v40(
    source: MemoryPointerV30, target: MemoryPointerV30, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    byte_pointer_view_shape_v38(source) && byte_pointer_view_shape_v38(target)
        && match (source.view, target.view) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                let offset = target.byte_offset - source.byte_offset;
                b.lower == a.lower + offset && b.upper == a.upper + offset
                    && (forall|i: int| 0 <= i < a.guards.len() ==>
                        exists|j: int| 0 <= j < b.guards.len()
                            && invocation_tag_guards_related_v40(a.guards[i], b.guards[j], map,
                                source_memory, target_memory))
                    && (forall|j: int| 0 <= j < b.guards.len() ==>
                        exists|i: int| 0 <= i < a.guards.len()
                            && invocation_tag_guards_related_v40(a.guards[i], b.guards[j], map,
                                source_memory, target_memory))
            },
            _ => false,
        }
}

open spec fn invocation_external_bytes_related_v38(
    source: MemoryBytesV30, target: MemoryBytesV30, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    source.base_alignment == target.base_alignment
        && source.bytes.len() == target.bytes.len()
        && source.initialized == target.initialized
        && (forall|i: int| 0 <= i < source.bytes.len() && source.initialized[i] ==>
            invocation_byte_token_related_v37(source.bytes[i], target.bytes[i], map, source_memory, target_memory))
        && source.relocations.dom() == target.relocations.dom()
        && (forall|i: int| source.relocations.contains_key(i) ==>
            invocation_relocation_related_v37(source.relocations[i], target.relocations[i], map, source_memory, target_memory))
}

// This is component metadata irrelevance with the two interpretation heaps
// fixed. Renaming a live heap's epochs also requires transporting its guards.
proof fn invocation_external_epoch_renaming_v38(
    source: MemoryBytesV30, target: MemoryBytesV30, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
    clock: int, epochs: Seq<int>,
)
    ensures invocation_external_bytes_related_v38(source, target, map, source_memory, target_memory)
        == invocation_external_bytes_related_v38(source,
            MemoryBytesV30 { write_clock: clock, write_epochs: epochs, ..target }, map, source_memory, target_memory),
{ }

open spec fn invocation_byte_token_related_v37(
    source: MemoryByteV37, target: MemoryByteV37, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    match (source, target) {
        (MemoryByteV37::Octet(a), MemoryByteV37::Octet(b)) => a == b,
        (MemoryByteV37::PointerFragment { pointer: a, width: aw, ordinal: ai },
         MemoryByteV37::PointerFragment { pointer: b, width: bw, ordinal: bi }) =>
            aw == bw && ai == bi && invocation_pointer_related_v36(a, b, map, source_memory, target_memory),
        _ => false,
    }
}

open spec fn invocation_relocation_related_v37(
    source: MemoryRelocationV37, target: MemoryRelocationV37, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    source.width == target.width && source.little_endian == target.little_endian
        && invocation_pointer_related_v36(source.pointer, target.pointer, map, source_memory, target_memory)
}

open spec fn invocation_value_related_v36(
    source: MemoryValueV30, target: MemoryValueV30, map: InvocationByteMapV36,
    source_memory: ByteMemoryV30, target_memory: ByteMemoryV30,
) -> bool {
    match (source, target) {
        (MemoryValueV30::Unit, MemoryValueV30::Unit) => true,
        (MemoryValueV30::Scalar(a), MemoryValueV30::Scalar(b)) => a == b,
        (MemoryValueV30::Vector(a), MemoryValueV30::Vector(b)) => a == b,
        (MemoryValueV30::Pointer(a), MemoryValueV30::Pointer(b)) =>
            invocation_pointer_related_v36(a, b, map, source_memory, target_memory),
        (MemoryValueV30::Slice(a), MemoryValueV30::Slice(b)) =>
            0 <= a.length && a.length == b.length
                && invocation_pointer_related_v36(a.pointer, b.pointer, map, source_memory, target_memory),
        _ => false,
    }
}

// The source read check is mandatory even if physical target storage contains
// initialized bytes from an earlier logical invocation or lifetime generation.
open spec fn invocation_source_read_enabled_v36(
    source: MemoryStateV30, pointer: MemoryPointerV30, width: int, alignment: int,
) -> bool {
    source.valid && byte_memory_well_formed_v30(source.memory) && ordinary_memory_width_v30(width)
        && byte_range_aligned_v30(source.memory, pointer, width, alignment)
        && byte_scalar_range_initialized_v37(source.memory, pointer, width)
}

// This scalar operation requires ordinary octets. Stored pointer cells use a
// separate typed operation; nominal fragments never become integer bits here.
open spec fn invocation_source_store_enabled_v36(
    source: MemoryStateV30, pointer: MemoryPointerV30, width: int, alignment: int,
    value: MemoryValueV30,
) -> bool {
    source.valid && byte_memory_well_formed_v30(source.memory) && ordinary_memory_width_v30(width)
        && byte_range_aligned_v30(source.memory, pointer, width, alignment)
        && match value {
            MemoryValueV30::Scalar(bits) => 0 <= bits < memory_value_modulus_v30(width),
            _ => false,
        }
}

open spec fn invocation_source_store_v36(
    source: MemoryStateV30, pointer: MemoryPointerV30, width: int, alignment: int,
    value: MemoryValueV30, little_endian: bool,
) -> MemoryStateV30 {
    if !invocation_source_store_enabled_v36(source, pointer, width, alignment, value) {
        invocation_source_refused_v36(source)
    } else {
        match value {
            MemoryValueV30::Scalar(bits) => MemoryStateV30 { pc: source.pc, values: source.values,
                memory: byte_store_v30(source.memory, pointer, width, bits, little_endian),
                generations: source.generations, frames: source.frames, valid: true },
            _ => invocation_source_refused_v36(source),
        }
    }
}

open spec fn invocation_read_related_v36(
    source: MemoryStateV30, target: MemoryStateV30, map: InvocationByteMapV36,
    source_pointer: MemoryPointerV30, target_pointer: MemoryPointerV30,
    width: int, alignment: int,
) -> bool {
    invocation_source_read_enabled_v36(source, source_pointer, width, alignment)
        && invocation_byte_states_related_v36(source, target, map)
        && invocation_pointer_related_v36(source_pointer, target_pointer, map, source.memory, target.memory)
        && byte_range_aligned_v30(target.memory, target_pointer, width, alignment)
        && byte_scalar_range_initialized_v37(target.memory, target_pointer, width)
}

open spec fn invocation_source_refused_v36(source: MemoryStateV30) -> MemoryStateV30 {
    MemoryStateV30 { pc: source.pc, values: source.values, memory: source.memory,
        generations: source.generations, frames: source.frames, valid: false }
}

struct InvocationByteActivationV36 {
    source: MemoryStateV30,
    map: InvocationByteMapV36,
    value: MemoryValueV30,
}

// Relational witness only, not the independent source interpreter. A failed
// target binding refuses the relation; it cannot redefine source execution.
// Successful source projection must equal the target-free source activation.
// Target bytes are never reset and give no source initialization credit.
open spec fn invocation_activation_witness_v36(
    source: MemoryStateV30, target: MemoryStateV30, map: InvocationByteMapV36,
    slot: InvocationSourceSlotV36, binding: InvocationByteBindingV36,
    root: int, instance: int,
) -> InvocationByteActivationV36 {
    if !invocation_byte_states_related_v36(source, target, map)
        || source.frames.active.len() == 0 || slot.root != root || slot.instance != instance
        || slot.owner < 0 || slot.extent < 0 || slot.alignment <= 0
        || slot.site.function < 0 || slot.site.block < 0 || slot.site.operation < 0 {
        InvocationByteActivationV36 { source: invocation_source_refused_v36(source), map,
            value: MemoryValueV30::Undefined }
    } else {
        let frame = source.frames.active.last();
        let site = MemoryPrivateSiteV30 { owner: frame.owner, invocation: frame.invocation, site: slot.site };
        let generation = private_generation_v30(source.generations, site);
        let allocation = private_allocation_v30(site, generation);
        if frame.owner != slot.owner || generation < 0
            || source.memory.live.contains_key(allocation) || map.private.contains_key(allocation)
            || binding.extent != slot.extent || binding.alignment != slot.alignment {
            InvocationByteActivationV36 { source: invocation_source_refused_v36(source), map,
                value: MemoryValueV30::Undefined }
        } else {
            let memory = byte_allocate_v30(source.memory, allocation, slot.extent, slot.alignment);
            let next_map = InvocationByteMapV36 { private: map.private.insert(allocation, binding) };
            let next = MemoryStateV30 { pc: source.pc, values: source.values, memory,
                generations: source.generations.insert(site, generation + 1), frames: source.frames,
                valid: invocation_byte_heaps_related_v36(memory, target.memory, next_map) };
            InvocationByteActivationV36 { source: next, map: next_map,
                value: MemoryValueV30::Pointer(MemoryPointerV30 { allocation, byte_offset: 0, view: None }) }
        }
    }
}

open spec fn invocation_value_names_allocation_v36(value: MemoryValueV30, allocation: MemoryAllocationV30) -> bool {
    match value {
        MemoryValueV30::Pointer(pointer) => pointer.allocation == allocation,
        MemoryValueV30::Slice(slice) => slice.pointer.allocation == allocation,
        _ => false,
    }
}

open spec fn invocation_memory_names_allocation_v37(
    memory: ByteMemoryV30, allocation: MemoryAllocationV30,
) -> bool {
    exists|object: MemoryAllocationV30, at: int| object != allocation
        && memory.live.contains_key(object)
        && ((memory.live[object].relocations.contains_key(at)
            && memory.live[object].relocations[at].pointer.allocation == allocation)
            || (0 <= at < memory.live[object].bytes.len()
                && memory.live[object].initialized[at]
                && match memory.live[object].bytes[at] {
                    MemoryByteV37::PointerFragment { pointer, .. } => pointer.allocation == allocation,
                    _ => false,
                }))
}

struct InvocationByteLifetimeV36 {
    source: MemoryStateV30,
    map: InvocationByteMapV36,
}

// retained indexes must be the complete source live-value/returned-value census
// supplied by the same invocation consumer, not a caller-chosen subset.
open spec fn invocation_lifetime_witness_v36(
    source: MemoryStateV30, map: InvocationByteMapV36, allocation: MemoryAllocationV30,
    retained: Seq<int>,
) -> InvocationByteLifetimeV36 {
    let allowed = source.valid && source.frames.active.len() > 0
        && byte_allocation_in_frame_v30(allocation, source.frames.active.last())
        && invocation_private_allocation_v36(allocation)
        && source.memory.live.contains_key(allocation) && map.private.contains_key(allocation)
        && (forall|i: int| 0 <= i < retained.len() ==>
            0 <= retained[i] < source.values.len()
            && !invocation_value_names_allocation_v36(source.values[retained[i]], allocation))
        && !invocation_memory_names_allocation_v37(source.memory, allocation);
    if !allowed {
        InvocationByteLifetimeV36 { source: invocation_source_refused_v36(source), map }
    } else {
        InvocationByteLifetimeV36 { source: MemoryStateV30 { pc: source.pc, values: source.values,
            memory: byte_end_lifetime_v30(source.memory, allocation), generations: source.generations,
            frames: source.frames, valid: true },
            map: InvocationByteMapV36 { private: map.private.remove(allocation) } }
    }
}

// No target memory parameter is returned or changed. Popping the source frame
// cannot authorize deallocation of hoisted or reused physical target storage.
open spec fn invocation_frame_end_witness_v36(
    source: MemoryStateV30, map: InvocationByteMapV36, retained: Seq<int>,
) -> InvocationByteLifetimeV36 {
    if !source.valid || source.frames.active.len() == 0 {
        InvocationByteLifetimeV36 { source: invocation_source_refused_v36(source), map }
    } else {
        let frame = source.frames.active.last();
        let allowed = (forall|i: int| 0 <= i < retained.len() ==>
            0 <= retained[i] < source.values.len()
            && forall|allocation: MemoryAllocationV30| byte_allocation_in_frame_v30(allocation, frame) ==>
                !invocation_value_names_allocation_v36(source.values[retained[i]], allocation))
            && !invocation_source_memory_escapes_frame_v37(source.memory, frame);
        if !allowed {
            InvocationByteLifetimeV36 { source: invocation_source_refused_v36(source), map }
        } else {
            InvocationByteLifetimeV36 { source: MemoryStateV30 { pc: source.pc, values: source.values,
                memory: byte_end_frame_v30(source.memory, frame), generations: source.generations,
                frames: byte_pop_frame_v30(source.frames), valid: true },
                map: InvocationByteMapV36 { private: Map::new(
                    |allocation: MemoryAllocationV30| map.private.contains_key(allocation)
                        && !byte_allocation_in_frame_v30(allocation, frame),
                    |allocation: MemoryAllocationV30| map.private[allocation],
                ) } }
        }
    }
}
"#;

pub(super) const CLASSIFIED_VIEW_LAWS_V40: &str =
    include_str!("original_semantic_mir_classified_view_laws_v40.vrs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_mir_classified_views_use_exact_registries_and_owner_derived_pairs() {
        let text = INVOCATION_BYTES_V36;
        for required in [
            "source.view_contracts == invocation_source_view_contracts_0_v39(invocation_runtime_little_endian_v36())",
            "target.view_contracts == byte_target_view_contracts_1_v38(invocation_runtime_little_endian_v36())",
            "source.owner == source_memory.view_contracts.owner",
            "target.owner == target_memory.view_contracts.owner",
            "invocation_source_target_tag_pair_0_1_v40(source.contract, target.contract)",
            "source.epochs.len() == source_memory.view_contracts.rows[source.contract].tag_width",
            "target.epochs.len() == target_memory.view_contracts.rows[target.contract].tag_width",
            "byte_offset: source.object_start, view: None",
            "byte_offset: target.object_start, view: None",
        ] {
            assert!(text.contains(required), "{required}");
        }
        assert!(!text.contains("source.contract == target.contract"));
        assert!(!text.contains("source.epochs == target.epochs"));
        assert!(!text.contains("source.live[a] == target.live[a]"));
    }

    #[test]
    fn original_mir_classified_views_keep_both_nested_obligation_sets_and_event_heaps() {
        let text = INVOCATION_BYTES_V36;
        for required in [
            "forall|i: int| 0 <= i < a.guards.len() ==>",
            "exists|j: int| 0 <= j < b.guards.len()",
            "forall|j: int| 0 <= j < b.guards.len() ==>",
            "exists|i: int| 0 <= i < a.guards.len()",
            "byte_tag_guard_current_v38(source_memory, source)",
            "byte_tag_guard_current_v38(target_memory, target)",
            "invocation_pointer_related_v36(source_pointer, target_pointer, map, source.memory, target.memory)",
        ] {
            assert!(text.contains(required), "{required}");
        }
        let laws = CLASSIFIED_VIEW_LAWS_V40;
        assert_eq!(laws.matches("proof fn ").count(), 8);
        assert!(laws.contains("invocation_guard_pair_never_refreshes_stale_source_v40"));
        assert!(laws.contains("invocation_guard_pair_uses_side_local_epoch_status_v40"));
        assert!(laws.contains("invocation_zero_width_keeps_stale_guard_refusal_v40"));
        assert!(!laws.contains("assume("));
        assert!(!laws.contains("external_body"));
    }

    #[test]
    fn original_mir_external_heap_relation_ignores_epoch_numbers_but_translates_pointer_cells() {
        let text = INVOCATION_BYTES_V36;
        assert!(!text.contains("source.live[a] == target.live[a]"));
        let relation = text
            .split("open spec fn invocation_external_bytes_related_v38")
            .nth(1)
            .unwrap()
            .split("proof fn invocation_external_epoch_renaming_v38")
            .next()
            .unwrap();
        assert!(relation.contains("source.initialized == target.initialized"));
        assert!(
            relation.contains(
                "invocation_byte_token_related_v37(source.bytes[i], target.bytes[i], map, source_memory, target_memory)"
            )
        );
        assert!(relation.contains(
            "invocation_relocation_related_v37(source.relocations[i], target.relocations[i], map, source_memory, target_memory)"
        ));
        assert!(!relation.contains("write_epochs"));
        assert!(!relation.contains("write_clock"));
        assert!(text.contains(
            "invocation_source_target_tag_pair_0_1_v40(source.contract, target.contract)"
        ));
        assert!(text.contains("byte_tag_guard_current_v38(source_memory, source)\n            == byte_tag_guard_current_v38(target_memory, target)"));
        assert!(!text.contains("source.epochs == target.epochs"));
        assert!(text.contains("b.lower == a.lower + offset && b.upper == a.upper + offset"));
    }

    #[test]
    fn original_mir_byte_relation_translates_stored_provenance_not_literal_private_ids() {
        let text = INVOCATION_BYTES_V36;
        assert!(text.contains("invocation_byte_token_related_v37(source.live[a].bytes[i]"));
        assert!(text.contains("invocation_relocation_related_v37(source_cell"));
        assert!(text.contains("aw == bw && ai == bi && invocation_pointer_related_v36(a, b, map, source_memory, target_memory)"));
        assert!(text.contains(
            "source.width == target.width && source.little_endian == target.little_endian"
        ));
        assert!(
            text.contains("invocation_pointer_related_v36(source.pointer, target.pointer, map, source_memory, target_memory)")
        );
        assert!(text.contains("byte_scalar_range_initialized_v37(source.memory, pointer, width)"));
        assert!(
            text.contains(
                "byte_scalar_range_initialized_v37(target.memory, target_pointer, width)"
            )
        );
    }

    #[test]
    fn original_mir_byte_lifetime_witness_checks_memory_even_without_live_ssa_values() {
        let text = INVOCATION_BYTES_V36;
        assert!(text.contains("object != allocation"));
        assert!(
            text.contains("memory.live[object].relocations[at].pointer.allocation == allocation")
        );
        assert!(text.contains("!invocation_value_names_allocation_v36(source.values[retained[i]], allocation))\n        && !invocation_memory_names_allocation_v37(source.memory, allocation)"));
        assert!(text.contains("!invocation_source_memory_escapes_frame_v37(source.memory, frame)"));
    }
}
