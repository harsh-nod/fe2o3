// Temporary diagnostic only. It neither queries nor modifies a proof ledger.
fn source_private_spill_probe_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    physical: &fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
) {
    use std::io::Write as _;
    if std::env::var_os("FE2O3_TRACE_PRIVATE_SPILLS_V25").as_deref()
        != Some(std::ffi::OsStr::new("1"))
    {
        return;
    }
    let mut out = std::io::stderr().lock();
    let owner = &original.source.owner.inner;
    let source = owner.source.owner.source_semantic();
    let roots = &owner.pending.roots;
    let _ = writeln!(
        out,
        "PRIVATE_SPILL_V25 BEGIN source_owner={:p} input_owner={:p} output_owner={:p} roots={} input_ops={} output_ops={} definitions={} authority=0",
        &owner.source.owner,
        original.inventory.owner(),
        output.owner(),
        roots.len(),
        original.inventory.operations().len(),
        output.operations().len(),
        output.definitions().len()
    );
    let mut slots_left = 256;
    let mut instances_left = 256;
    let mut sidecars_left = 128;
    let mut anchors_left = 512;
    for (root, row) in roots.iter().take(16).enumerate() {
        let _ = writeln!(
            out,
            "PRIVATE_SPILL_V25 ROOT root={root} function={} slots={} instances={} sidecars={}",
            row.function_ordinal,
            row.source_slots.slots.len(),
            row.source_slots.instances.len(),
            row.sidecars.rows.len()
        );
        let slots = row.source_slots.slots.len().min(slots_left);
        let instances = row.source_slots.instances.len().min(instances_left);
        let sidecars = row.sidecars.rows.len().min(sidecars_left);
        slots_left -= slots;
        instances_left -= instances;
        sidecars_left -= sidecars;
        for (slot, backing) in row.source_slots.slots.iter().take(slots).enumerate() {
            let layout = source
                .types()
                .get(backing.origin.semantic_type.index() as usize)
                .map(|ty| (ty.layout().size_bytes(), ty.layout().alignment_bytes()));
            let _ = writeln!(
                out,
                "PRIVATE_SPILL_V25 SLOT root={root} slot={slot} instance={} identity={:?} source={:?} type={} layout={layout:?} representation={:?} pointer={} allocation={:?}",
                backing.instance.index(),
                backing.origin.identity,
                backing.origin.source,
                backing.origin.semantic_type.index(),
                backing.representation,
                backing.origin.pointer.0,
                backing.allocation
            );
        }
        for (index, instance) in row
            .source_slots
            .instances
            .iter()
            .take(instances)
            .enumerate()
        {
            let _ = writeln!(
                out,
                "PRIVATE_SPILL_V25 INSTANCE root={root} row={index} instance={} function={} slots={:?}",
                instance.instance.index(),
                instance.function.index(),
                instance.slots
            );
        }
        for (instance, sidecar) in row.sidecars.rows.iter().take(sidecars).enumerate() {
            if let Some(anchors) = &sidecar.scoped_memory_anchors {
                let shown = anchors.rows.len().min(anchors_left);
                anchors_left -= shown;
                for (anchor, value) in anchors.rows.iter().take(shown).enumerate() {
                    let _ = writeln!(
                        out,
                        "PRIVATE_SPILL_V25 ANCHOR root={root} sidecar={instance} source_instance={:?} anchor={anchor} block={} operation={} frame={:?} kind={:?}",
                        sidecar.source_call_instance.map(|id| id.index()),
                        value.block.0,
                        value.position,
                        value.source,
                        value.kind
                    );
                }
                let _ = writeln!(
                    out,
                    "PRIVATE_SPILL_V25 ANCHORS_END root={root} sidecar={instance} total={} shown={} truncated={}",
                    anchors.rows.len(),
                    shown,
                    u8::from(anchors.rows.len() > shown)
                );
            }
        }
        let _ = writeln!(
            out,
            "PRIVATE_SPILL_V25 ROOT_END root={root} slots_truncated={} instances_truncated={} sidecars_truncated={}",
            u8::from(row.source_slots.slots.len() > slots),
            u8::from(row.source_slots.instances.len() > instances),
            u8::from(row.sidecars.rows.len() > sidecars)
        );
    }
    source_private_spill_inventory_probe_v25(&mut out, "input", original.inventory, None);
    source_private_spill_inventory_probe_v25(&mut out, "output", output, Some(physical));
    for (definition, row) in output.definitions().iter().take(512).enumerate() {
        if let Some(address) = physical.address(definition) {
            let _ = writeln!(
                out,
                "PRIVATE_SPILL_V25 ADDRESS definition={definition} coordinate={:?} value={:?} allocation={} start={} length={} offset={} stride={} alignment={}",
                row.coordinate,
                row.value,
                address.allocation(),
                address.start(),
                address.length(),
                address.offset(),
                address.stride(),
                address.alignment()
            );
        }
    }
    let _ = writeln!(
        out,
        "PRIVATE_SPILL_V25 END roots_truncated={} definitions_truncated={} authority=0",
        u8::from(roots.len() > 16),
        u8::from(output.definitions().len() > 512)
    );
}

fn source_private_spill_inventory_probe_v25(
    out: &mut impl std::io::Write,
    side: &str,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    physical: Option<&fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>>,
) {
    for (ordinal, row) in inventory.operations().iter().take(512).enumerate() {
        let (kind, pointer, value) = match &row.operation.kind {
            OperationKind::Alloca { .. } => ("alloca", None, None),
            OperationKind::Load { pointer, .. } => ("load", Some(*pointer), None),
            OperationKind::Store { pointer, value, .. } => ("store", Some(*pointer), Some(*value)),
            OperationKind::GuardedLoad { pointer, .. } => ("guarded_load", Some(*pointer), None),
            OperationKind::GuardedStore { pointer, value, .. } => {
                ("guarded_store", Some(*pointer), Some(*value))
            }
            OperationKind::Storage(_) => ("storage", None, None),
            _ => continue,
        };
        let _ = writeln!(
            out,
            "PRIVATE_SPILL_V25 OP side={side} ordinal={ordinal} coordinate={:?} kind={kind} pointer={pointer:?} rhs={value:?} results={} result0={:?} physical={:?} writer={:?}",
            row.coordinate,
            row.operation.results.len(),
            row.operation.results.first().map(|value| value.id),
            physical.map(|proof| proof.operation(ordinal)),
            physical.and_then(|proof| proof.latest_stores().get(ordinal).copied().flatten())
        );
    }
    let _ = writeln!(
        out,
        "PRIVATE_SPILL_V25 OPS_END side={side} total={} scanned={} truncated={}",
        inventory.operations().len(),
        inventory.operations().len().min(512),
        u8::from(inventory.operations().len() > 512)
    );
}
