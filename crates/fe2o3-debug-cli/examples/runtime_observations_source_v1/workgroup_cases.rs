//! Unchanged ordinary Rust LDS reduction; never a synthetic Alloca source claim.
use super::common::*;
use fe2o3_kernel_ir::*;
use fe2o3_kir_debugger::*;
use fe2o3_kir_sim::*;
use serde_json::{Value, json};

fn request() -> Result<SimulationRequestV1, String> {
    let mut words = vec![ScalarBitsV1::u32(0xdeadbeef)];
    words.extend(vec![ScalarBitsV1::u32(0xa5a5a5a5); 128]);
    words.push(ScalarBitsV1::u32(0xcafebabe));
    let buffer =
        BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &words, TARGET).map_err(fail)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        128,
        TARGET,
    )
    .map_err(fail)?;
    Ok(SimulationRequestV1::new(
        "workgroup_reduce_u32",
        [128, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(2)),
            SimulationArgumentV1::BufferView(view),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer,
    }]))
}
#[cfg(test)]
mod request_tests {
    use super::*;
    #[test]
    fn exact_workgroup_buffer_view_exposes_all_payload_words_and_excludes_guards() {
        let request = request().unwrap();
        assert_eq!(request.grid.0, [128, 1, 1]);
        assert_eq!(request.workgroup.0, [64, 1, 1]);
        let SimulationArgumentV1::BufferView(view) = &request.arguments[1] else {
            panic!("view expected");
        };
        assert_eq!(view.backing(), BufferBackingIdV1(0));
        assert_eq!(view.alignment(), 4);
        assert_eq!(view.byte_offset(), 4);
        assert_eq!(view.elements(), 128);
        assert_eq!(request.shared_buffers.len(), 1);
        let buffer = &request.shared_buffers[0].buffer;
        assert_eq!(buffer.bytes().len(), 520);
        assert_eq!(&buffer.bytes()[..4], &0xdeadbeef_u32.to_le_bytes());
        assert_eq!(&buffer.bytes()[516..], &0xcafebabe_u32.to_le_bytes());
        assert!(buffer.bytes()[4..516].iter().all(|byte| *byte == 0xa5));
        assert_eq!(buffer.initialized(), [true; 520]);
        assert_eq!(
            request.arguments[0],
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(2))
        );
    }
}
fn check_output(
    execution: &SimulationExecutionV1,
    request: &SimulationRequestV1,
) -> Result<(), String> {
    demand(
        execution.arguments() == request.arguments
            && execution.invocations_executed() == 128
            && execution.workgroups_visited() == 2
            && execution.scheduled_slots_visited() == 128
            && execution.shared_buffers().len() == 1,
        "workgroup ABI/counts",
    )?;
    let buffer = execution
        .shared_buffer(BufferBackingIdV1(0))
        .ok_or("workgroup output")?;
    let mut words = vec![0xdeadbeef_u32];
    words.extend(vec![128; 128]);
    words.push(0xcafebabe);
    let expected = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    demand(
        buffer.bytes() == expected && buffer.initialized() == vec![true; 520],
        "workgroup outputs/all initialization/both guards",
    )
}
fn topology(module: &Module) -> Result<Value, String> {
    demand(
        module.kernels.len() == 1
            && module.kernels[0].id.as_str() == "workgroup_reduce_u32"
            && !module.functions.is_empty()
            && module.functions.len() <= 8,
        "workgroup source kernel/function roster",
    )?;
    let mut operations = 0;
    let mut declarations = Vec::new();
    for (function_index, function) in module.functions.iter().enumerate() {
        if let Some(body) = &function.body {
            for (block_index, block) in body.blocks.iter().enumerate() {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    operations += 1;
                    demand(
                        !matches!(operation.kind, OperationKind::Alloca { .. }),
                        "ordinary source cannot substitute synthetic private allocation",
                    )?;
                    if let OperationKind::WorkgroupMemory(memory) = &operation.kind {
                        demand(
                            memory.element == Type::Scalar(ScalarType::U32)
                                && memory.extent == WorkgroupMemoryExtent::Static(64)
                                && memory.alignment == 4,
                            "exact source LDS shape",
                        )?;
                        declarations.push(
                            json!({"runtime_site":[function_index,block.id.0,operation_index],
                            "authoring_coordinate":[function_index,block_index,operation_index]}),
                        );
                    }
                }
            }
        }
    }
    demand(
        operations > 0 && operations <= 512 && declarations.len() == 1,
        "one actual source LDS declaration",
    )?;
    Ok(
        json!({"operations":operations,"lds":declarations[0],"element":"U32","elements":64,"alignment":4,"private_alloca":false}),
    )
}
fn find_first_snapshot(
    owner: &DebugObservedTranscriptV1,
    allocation: u64,
) -> Result<usize, String> {
    owner
        .legacy()
        .records()
        .iter()
        .enumerate()
        .find_map(|(index, record)| {
            if let SimulationDebugRecordKindV1::Checkpoint {
                memory: SimulationDebugCollectionV1::Captured(memory),
                ..
            } = &record.kind
            {
                memory
                    .iter()
                    .any(|value| value.allocation == allocation)
                    .then_some(index)
            } else {
                None
            }
        })
        .ok_or("actual allocation snapshot absent".into())
}
fn sample(
    owner: &DebugObservedTranscriptV1,
    index: usize,
    allocation: u64,
) -> Result<Value, String> {
    let state = owner.allocations_at(index).map_err(debug)?;
    let descriptor = state.descriptor(allocation, &mut work()?).map_err(debug)?;
    let bytes = memory(owner, index, allocation)?;
    let record = &owner.legacy().records()[index];
    demand(
        bytes.address_space == descriptor.address_space()
            && bytes.access == descriptor.access()
            && bytes.alignment == descriptor.alignment()
            && bytes.bytes.len() as u64 == descriptor.byte_len()
            && bytes.initialized.len() == bytes.bytes.len(),
        "snapshot/allocator descriptor join",
    )?;
    Ok(
        json!({"record":index,"capture_instance":owner.capture_instance().to_string(),
        "invocation":invocation(record.invocation),"site":site(record.site),
        "through_sequence":state.through_sequence().to_string(),"identity":identity(descriptor.identity()),
        "scope":allocation_scope(descriptor.scope()),"bytes":bytes.bytes,"initialized":bytes.initialized}),
    )
}
fn lifecycle(owner: DebugObservedTranscriptV1) -> Result<Value, String> {
    let last = owner.legacy().records().len() - 1;
    let observed = owner.allocations_at(last).map_err(debug)?;
    let prefix = observed.transitions();
    let creates = prefix
        .iter()
        .filter(|row| {
            row.descriptor().address_space() == AddressSpace::Workgroup
                && matches!(
                    row.kind(),
                    SimulationAllocationTransitionKindV1::Create { .. }
                )
        })
        .copied()
        .collect::<Vec<_>>();
    demand(
        creates.len() == 2,
        "two actual LDS creations before current record",
    )?;
    let a = creates[0].descriptor();
    let b = creates[1].descriptor();
    let first = a.identity();
    let second = b.identity();
    demand(
        first.allocation() != second.allocation()
            && first.storage_slot() == second.storage_slot()
            && first.generation() == 1
            && second.generation() == 2
            && matches!(
                creates[0].kind(),
                SimulationAllocationTransitionKindV1::Create {
                    previous_allocation: None
                }
            )
            && matches!(creates[1].kind(),SimulationAllocationTransitionKindV1::Create {previous_allocation:Some(value)}
            if value==first.allocation()),
        "allocator-owned actual slot reuse chain",
    )?;
    let released = prefix
        .iter()
        .find(|row| {
            row.descriptor().identity() == first
                && matches!(row.kind(), SimulationAllocationTransitionKindV1::Release)
        })
        .ok_or("first LDS release absent")?;
    demand(
        creates[0].sequence() < released.sequence()
            && released.sequence() < creates[1].sequence()
            && released.descriptor() == a,
        "release must precede exact compatible replacement",
    )?;
    for (group, descriptor) in [(0, a), (1, b)] {
        demand(
            descriptor.byte_len() == 256
                && descriptor.alignment() == 4
                && descriptor.access() == AccessMode::ReadWrite
                && matches!(descriptor.scope(),SimulationAllocationScopeV1::Workgroup {coordinate,size,count,launch}
                if coordinate==[group,0,0] && size==[64,1,1] && count==[2,1,1] && launch==[128,1,1]),
            "exact allocator workgroup scope",
        )?;
    }
    let first_index = find_first_snapshot(&owner, first.allocation())?;
    let second_index = find_first_snapshot(&owner, second.allocation())?;
    let first_snapshot = sample(&owner, first_index, first.allocation())?;
    let second_snapshot = sample(&owner, second_index, second.allocation())?;
    for (index, allocation) in [
        (first_index, first.allocation()),
        (second_index, second.allocation()),
    ] {
        let snapshot = memory(&owner, index, allocation)?;
        demand(
            snapshot.bytes == [0; 256] && snapshot.initialized == [false; 256],
            "new/reused source LDS bytes and initialization must be reset",
        )?;
    }
    demand(
        owner
            .allocations_at(second_index)
            .map_err(debug)?
            .descriptor(first.allocation(), &mut work()?)
            == Err(RuntimeAllocationMissingV1::NotLive),
        "old allocation rebound to replacement",
    )?;
    demand(
        owner
            .allocations_at(first_index)
            .map_err(debug)?
            .descriptor(second.allocation(), &mut work()?)
            == Err(RuntimeAllocationMissingV1::NotLive),
        "future incarnation leaked into older snapshot",
    )?;
    demand(
        memory(&owner, second_index, first.allocation()).is_err(),
        "old allocation bytes visible at replacement checkpoint",
    )?;
    let transitions = prefix.iter().copied().map(transition).collect::<Vec<_>>();
    let usage = usage(&owner);
    let mut session = owner.into_session();
    let mut replay = work()?;
    for (index, identity) in [
        (first_index, first),
        (second_index, second),
        (first_index, first),
        (second_index, second),
    ] {
        session.seek_record(index, &mut replay).map_err(debug)?;
        let selected = session
            .current_allocations()
            .map_err(debug)?
            .descriptor(identity.allocation(), &mut replay)
            .map_err(debug)?;
        demand(
            selected.identity() == identity,
            "historical seek/repeat changed allocator identity",
        )?;
        let legacy = session
            .legacy()
            .current()
            .ok_or("selected resource record")?;
        demand(legacy.ordinal == index as u64, "historical record ordinal")?;
    }
    Ok(
        json!({"transitions_at_last_record":transitions,"first":first_snapshot,"second":second_snapshot,
        "old_allocation_refused_at_second":true,"future_allocation_refused_at_first":true,
        "historical_seek_repeat_checks":4,"replay_work_used":1_000_000_000-replay.remaining(),"usage":usage,
        "terminal_release_claimed":false}),
    )
}
pub(super) fn observe(bytes: Vec<u8>) -> Result<Value, String> {
    let digest = hash(&bytes);
    let bundle = VerifiedSimulationBundleV5::from_canonical_bytes(bytes).map_err(fail)?;
    bundle.revalidate().map_err(fail)?;
    demand(
        bundle.target() == "gfx942:xnack-" && bundle.kernel_count() == 1,
        "workgroup bundle target/kernel",
    )?;
    let canonical =
        VerifiedCanonicalKernelIrV10::from_canonical_bytes(bundle.canonical_kir_v10().to_vec())
            .map_err(fail)?;
    let admitted = AdmittedSimulationModuleV1::admit_v10(canonical, limits(true)).map_err(fail)?;
    let topology = topology(admitted.module())?;
    let mut cases = Vec::new();
    for seeded in [false, true] {
        let request = request()?;
        let (off, off_owner) = capture(&admitted, &request, true, seeded, false)?;
        let (on, owner) = capture(&admitted, &request, true, seeded, true)?;
        demand(
            on == off && owner.legacy() == off_owner.legacy(),
            "reuse changed source reduction or legacy records",
        )?;
        check_output(&off, &request)?;
        check_output(&on, &request)?;
        let observations = lifecycle(owner)?;
        let output = on
            .shared_buffer(BufferBackingIdV1(0))
            .ok_or("workgroup output")?;
        cases.push(json!({"schedule":if seeded{"seeded_71"}else{"canonical"},"grid":[128,1,1],"workgroup":[64,1,1],
            "input":2,"expected_word":128,"output_bytes":output.bytes(),"initialized":output.initialized(),
            "reuse_off_execution_equal":true,"reuse_off_legacy_equal":true,"records":off_owner.legacy().records().len(),
            "observations":observations}));
    }
    Ok(
        json!({"bundle_sha256":digest,"bundle_identity":hex(bundle.identity().as_bytes()),"target":bundle.target(),
        "canonical_kir_digest":hex(bundle.canonical_kir_v10_digest()),"canonical_kir_sha256":hash(bundle.canonical_kir_v10()),
        "canonical_kir_bytes":bundle.canonical_kir_v10().len(),"topology":topology,"cases":cases,
        "reuse_on_runs":2,"reuse_off_runs":2,"source_authenticated":false,"hardware_observed":false,"compiler_resume_authority":false}),
    )
}
