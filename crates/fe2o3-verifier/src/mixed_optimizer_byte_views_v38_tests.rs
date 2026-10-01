fn view_spec_body_v38(name: &str) -> &'static str {
    let text = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    text.split_once(&format!("open spec fn {name}"))
        .unwrap()
        .1
        .split("open spec fn ")
        .next()
        .unwrap()
}

#[test]
fn byte_view_all_writes_stamp_destination_epochs_and_preserve_contracts() {
    for (name, address, width) in [
        ("byte_store_v30", "pointer", "width"),
        (
            "byte_vector_store_v30",
            "pointer",
            "lane_width * values.len()",
        ),
        ("byte_pointer_store_v37", "address", "width"),
        ("byte_deinitialize_v37", "address", "width"),
        ("byte_copy_snapshot_v39", "destination", "width"),
    ] {
        let body = view_spec_body_v38(name);
        assert!(
            body.contains(&format!(
                "write_clock: byte_write_clock_v38(previous, {width})"
            )),
            "{name}"
        );
        assert!(
            body.contains(&format!(
                "write_epochs: byte_overwrite_epochs_v38(previous, {address}.byte_offset, {width})"
            )),
            "{name}"
        );
        assert!(
            body.contains("view_contracts: memory.view_contracts"),
            "{name}"
        );
    }
    let copy = view_spec_body_v38("byte_copy_object_v37");
    assert!(copy.contains("let snapshot = memory.live[source.allocation]"));
    assert!(copy.contains(
        "byte_copy_snapshot_v39(memory, snapshot, source.byte_offset, destination, width)"
    ));
    assert!(!copy.contains("snapshot.write_epochs"));
    let snapshot = view_spec_body_v38("byte_copy_snapshot_v39");
    assert!(snapshot.contains("let previous = memory.live[destination.allocation]"));
    assert!(!snapshot.contains("snapshot.write_epochs"));
    let stamp = view_spec_body_v38("byte_overwrite_epochs_v38");
    assert!(stamp.contains("else { previous.write_epochs[i] }"));
    assert!(!stamp.contains("previous.bytes"));
    assert!(
        view_spec_body_v38("byte_write_clock_v38")
            .contains("previous.write_clock + if 0 < width { 1 } else { 0 }")
    );
}

#[test]
fn byte_copy_snapshot_reads_only_the_saved_source_window_before_destination_invalidation() {
    let body = view_spec_body_v38("byte_copy_snapshot_v39");
    for required in [
        "snapshot: MemoryBytesV30, source_offset: int",
        "byte_object_relocations_well_formed_v37(snapshot)",
        "0 <= source_offset && 0 <= width && source_offset + width <= snapshot.bytes.len()",
        "byte_range_live_v30(memory, destination, width)",
        "source_offset + at - destination.byte_offset",
        "snapshot.bytes[copied(i)]",
        "snapshot.initialized[copied(i)]",
        "else { previous.bytes[i] }",
        "else { previous.initialized[i] }",
    ] {
        assert!(body.contains(required), "{required}");
    }
    assert!(!body.contains("memory.live[source.allocation]"));
    assert!(!body.contains("byte_load_v30"));
    assert!(!body.contains("byte_range_initialized_v30"));
}

#[test]
fn byte_copy_snapshot_keeps_complete_cell_containment_and_partial_fragment_policy() {
    let body = view_spec_body_v38("byte_copy_snapshot_v39");
    for required in [
        "destination.byte_offset <= at",
        "at < destination.byte_offset + width",
        "snapshot.relocations.contains_key(copied(at))",
        "copied(at) + snapshot.relocations[copied(at)].width <= source_offset + width",
        "byte_relocations_without_overlap_v37(previous.relocations, destination.byte_offset, width)",
        "retained.contains_key(at) || complete(at)",
        "if complete(at) { snapshot.relocations[copied(at)] } else { retained[at] }",
    ] {
        assert!(body.contains(required), "{required}");
    }
    assert!(!body.contains("source_offset == destination.byte_offset"));
    assert!(!body.contains("source.allocation == destination.allocation"));
    assert!(!body.contains("byte_pointer_load_v37"));
}

#[test]
fn byte_view_zero_width_and_nested_guards_are_checked_before_access() {
    assert!(
        view_spec_body_v38("byte_pointer_view_shape_v38")
            .contains("view.guards[i].allocation == pointer.allocation")
    );
    let range = view_spec_body_v38("byte_range_live_v30");
    assert!(range.contains(
        "byte_raw_range_live_v38(memory, pointer.allocation, pointer.byte_offset, width)"
    ));
    assert!(range.contains("&& byte_pointer_view_current_v38(memory, pointer, width)"));
    assert!(!range.contains("width == 0"));
    let view = view_spec_body_v38("byte_pointer_view_current_v38");
    assert!(view.contains("pointer.byte_offset + width <= view.upper"));
    assert!(view.contains("0 <= i < view.guards.len()"));
    assert!(view.contains("byte_tag_guard_current_v38(memory, view.guards[i])"));
    let project = view_spec_body_v38("byte_project_view_v38");
    assert!(project.contains("!byte_range_live_v30(memory, parent, 0)"));
    assert!(project.contains("!byte_range_live_v30(memory, pointer, extent)"));
    assert!(
        view_spec_body_v38("byte_narrow_view_v38")
            .contains("None => Seq::empty(), Some(view) => view.guards")
    );
    assert!(
        view_spec_body_v38("byte_extend_view_guard_v38")
            .contains("guards: view.guards.push(guard)")
    );
}

#[test]
fn byte_view_guards_require_actual_tag_bytes_owner_and_captured_epochs() {
    let guard = view_spec_body_v38("byte_tag_guard_current_v38");
    for required in [
        "guard.owner == memory.view_contracts.owner",
        "memory.view_contracts.rows.contains_key(guard.contract)",
        "memory.live[guard.allocation].initialized[tag + i]",
        "memory.live[guard.allocation].write_epochs[tag + i] == guard.epochs[i]",
        "byte_tag_observation_v39(memory.live[guard.allocation], tag, contract)",
        "byte_tag_observation_selects_v39(contract, observation, guard.variant)",
    ] {
        assert!(guard.contains(required), "{required}");
    }
    let selector = view_spec_body_v38("byte_tag_selects_v38");
    assert!(selector.contains("contract.inhabited[variant]"));
    assert!(selector.contains("MemoryTagEncodingV38::Unsupported => false"));
    assert!(selector.contains(
        "contract.untagged_valid_bits[i].0 <= bits <= contract.untagged_valid_bits[i].1"
    ));
}

#[test]
fn byte_pointer_niche_observation_requires_initialized_null_or_complete_nominal_fragments() {
    let raw = view_spec_body_v38("byte_pointer_tag_observation_v39");
    for required in [
        "!byte_raw_tag_initialized_v39(object, offset, width)",
        "object.bytes[offset + i] == MemoryByteV37::Octet(0)",
        "Some(MemoryTagObservationV39::Null)",
        "!object.relocations.contains_key(offset)",
        "relocation.width != width || relocation.little_endian != little_endian",
        "!byte_pointer_view_shape_v38(relocation.pointer)",
        "object.bytes[offset + i] == byte_relocation_fragment_v37(relocation, i)",
        "Some(MemoryTagObservationV39::Pointer { pointer: relocation.pointer })",
    ] {
        assert!(raw.contains(required), "{required}");
    }
    for forbidden in ["byte_tag_bits", "byte_octet", "byte_range_live", "as int"] {
        assert!(!raw.contains(forbidden), "{forbidden}");
    }
    let initialized = view_spec_body_v38("byte_raw_tag_initialized_v39");
    for required in [
        "ordinary_memory_width_v30(width)",
        "0 <= offset && offset + width <= object.bytes.len()",
        "object.initialized.len() == object.bytes.len()",
        "0 <= i < width ==> object.initialized[offset + i]",
    ] {
        assert!(initialized.contains(required), "{required}");
    }
}

#[test]
fn byte_pointer_niche_selection_keeps_scalar_bits_and_reference_validity_separate() {
    let reader = view_spec_body_v38("byte_tag_observation_v39");
    assert!(reader.contains("MemoryTagEncodingV38::PointerNullNiche { .. } =>"));
    assert!(reader.contains(
        "byte_pointer_tag_observation_v39(object, offset, contract.tag_width, contract.little_endian)"
    ));
    assert!(reader.contains("byte_token_well_formed_v37(object.bytes[offset + i])"));
    let selector = view_spec_body_v38("byte_tag_observation_selects_v39");
    for required in [
        "contract.inhabited[variant]",
        "MemoryTagObservationV39::Pointer { pointer }",
        "variant == nonnull",
        "MemoryTagObservationV39::Null",
        "variant == null",
        "byte_tag_selects_v38(contract, bits, variant)",
    ] {
        assert!(selector.contains(required), "{required}");
    }
    assert!(
        view_spec_body_v38("byte_tag_selects_v38")
            .contains("MemoryTagEncodingV38::PointerNullNiche { .. } => false")
    );
    assert!(!selector.contains("memory.live"));
    assert!(!selector.contains("byte_range_live"));
}

#[test]
fn byte_view_actual_dispatch_refuses_supplied_contract_registries() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(38, out)
        })
        .0
        .unwrap();
        assert_eq!(
            text.matches("!byte_native_view_inputs_v38(s.memory, s.values)")
                .count(),
            3
        );
        assert_eq!(
            text.matches("!byte_native_view_inputs_v38(done.memory, done.values)")
                .count(),
            1
        );
        assert!(
            view_spec_body_v38("byte_default_view_contracts_v38")
                .contains("memory.view_contracts == byte_empty_view_contracts_v38()")
        );
        assert!(
            view_spec_body_v38("byte_empty_view_contracts_v38")
                .contains("MemoryViewContractsV38 { owner: 0, rows: Map::empty() }")
        );
    });
}

#[test]
fn byte_view_unused_values_and_objects_remain_in_the_entry_census() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("unused-guarded-input-census");
    module.functions.push(KirFunction::internal_helper(
        "unused",
        Signature::new(vec![pointer(), pointer()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(38, out)
        })
        .0
        .unwrap();
        assert!(text.contains("done.values.len() != 2"));
        assert!(text.contains("!byte_native_view_inputs_v38(done.memory, done.values)"));
        assert!(!text.contains("MemoryOperationEffectV30::Read"));
        assert!(
            view_spec_body_v38("byte_native_view_inputs_v38")
                .contains("memory.live.contains_key(allocation) ==>")
        );
    });
}

#[test]
fn byte_view_address_formation_preserves_views_instead_of_rebuilding_naked_pointers() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(38, out)
        })
        .0
        .unwrap();
        assert_eq!(
            text.matches("byte_offset: p.byte_offset + i * 4, view: p.view")
                .count(),
            2
        );
        assert!(!text.contains("view: None"));
    });
    assert!(view_spec_body_v38("byte_vector_load_v30").contains("view: pointer.view"));
}

#[test]
fn byte_view_native_inputs_check_initialized_fragments_but_not_uninitialized_history() {
    let native = view_spec_body_v38("byte_native_view_inputs_v38");
    assert!(native.contains("byte_default_view_contracts_v38(memory)"));
    assert!(native.contains("byte_value_has_no_guards_v38(values[i])"));
    assert!(native.contains("0 <= i < object.bytes.len() && object.initialized[i] ==>"));
    assert!(native.contains("MemoryByteV37::PointerFragment { pointer, .. }"));
    assert!(native.contains("object.relocations.contains_key(at) ==>"));
    assert!(native.contains("byte_pointer_has_no_guards_v38(object.relocations[at].pointer)"));
}
