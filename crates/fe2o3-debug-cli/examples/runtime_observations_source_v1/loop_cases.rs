//! Ordinary loop/helper source acceptance, using only the sealed public owner.
use super::common::*;
use super::topology;
use fe2o3_kernel_ir::{
    AccessMode, ScalarType, VerifiedCanonicalKernelIrV11, VerifiedSimulationBundleV6,
};
use fe2o3_kir_debugger::*;
use fe2o3_kir_sim::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn request(rounds: u32) -> Result<SimulationRequestV1, String> {
    let words = [
        0xdeadbeef, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xcafebabe,
    ]
    .map(ScalarBitsV1::u32);
    let buffer =
        BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &words, TARGET).map_err(fail)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        4,
        TARGET,
    )
    .map_err(fail)?;
    Ok(SimulationRequestV1::new(
        "loop_helper",
        [4, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(view),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0xabcd1234)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0x0f0f55aa)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(rounds)),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer,
    }]))
}
fn expected(rounds: u32) -> u32 {
    let mut value = 0xabcd1234;
    for iteration in 0..(rounds & 3) {
        value = (value ^ (0x0f0f55aa ^ iteration)) & 0xffff;
    }
    value
}
fn check_output(
    execution: &SimulationExecutionV1,
    request: &SimulationRequestV1,
    rounds: u32,
) -> Result<(), String> {
    demand(
        execution.arguments() == request.arguments
            && execution.invocations_executed() == 4
            && execution.workgroups_visited() == 1
            && execution.scheduled_slots_visited() == 64
            && execution.shared_buffers().len() == 1,
        "loop ABI/counts",
    )?;
    let buffer = execution
        .shared_buffer(BufferBackingIdV1(0))
        .ok_or("loop shared output")?;
    let word = expected(rounds);
    let bytes = [0xdeadbeef, word, word, word, word, 0xcafebabe]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    demand(
        buffer.bytes() == bytes && buffer.initialized() == [true; 24],
        "loop output/all initialization/both guards",
    )
}
fn exact_invocation(value: SimulationInvocationV1) -> bool {
    value.global[0] < 4
        && value.global[1..] == [0, 0]
        && value.workgroup == [0, 0, 0]
        && value.local == [value.global[0] as u32, 0, 0]
        && value.workgroup_size == [64, 1, 1]
        && value.workgroup_count == [1, 1, 1]
        && value.launch_extent == [4, 1, 1]
}
fn check_frames(
    owner: &DebugObservedTranscriptV1,
    topology: &topology::Topology,
    rounds: u32,
) -> Result<(Value, Vec<usize>), String> {
    let mut helpers = BTreeMap::<SimulationInvocationV1, BTreeSet<u64>>::new();
    let mut call_counts = BTreeMap::<SimulationInvocationV1, usize>::new();
    let mut helper_first = Vec::new();
    let mut witnesses = Vec::new();
    let mut writes = BTreeSet::new();
    let mut pairs = RuntimeOriginScanWorkV1::new(1_000_000).map_err(fail)?;
    let mut checked_helper_operations = 0;
    for (index, record) in owner.legacy().records().iter().enumerate() {
        demand(
            record.ordinal == index as u64 && exact_invocation(record.invocation),
            "loop full invocation/record join",
        )?;
        let origin = owner.origin_at(index).map_err(debug)?;
        demand(
            origin.invocation() == record.invocation && origin.site() == record.site,
            "loop exact origin",
        )?;
        if let SimulationDebugRecordKindV1::Memory {
            access,
            byte_offset,
            byte_len,
            value,
            ..
        } = &record.kind
        {
            demand(
                *access == SimulationDebugMemoryAccessV1::WriteCommitted
                    && *byte_len == 4
                    && *byte_offset == 4 + 4 * record.invocation.global[0] as usize
                    && matches!(value, SimulationDebugValueV1::Scalar(bits)
                    if bits.ty() == ScalarType::U32 && bits.bits() == u128::from(expected(rounds)))
                    && writes.insert(record.invocation),
                "loop committed output write",
            )?;
            demand(
                owner.frames_at(index).unwrap_err()
                    == RuntimeFrameMissingV1::RuntimeUnavailable(
                        SimulationDebugFrameOriginUnavailableV1::NotCheckpoint,
                    ),
                "memory record reused old frame roster",
            )?;
            continue;
        }
        let frames = owner.frames_at(index).map_err(debug)?;
        demand(
            frames.len()
                == if record.site.function_ordinal == topology.helper {
                    2
                } else {
                    1
                },
            "exact source stack shape",
        )?;
        if phase(record) != Some(SimulationDebugCheckpointPhaseV1::BeforeOperation) {
            continue;
        }
        if site(record.site) == json!(topology.call) {
            demand(
                origin.activation() == 1,
                "caller must be actual root activation",
            )?;
            *call_counts.entry(record.invocation).or_default() += 1;
            let after = owner.paired_after(index, &mut pairs).map_err(debug)?;
            let returned = owner.frames_at(after).map_err(debug)?;
            let completed = owner.origin_at(after).map_err(debug)?;
            demand(
                returned.len() == 1
                    && completed.activation() == origin.activation()
                    && completed.attempt() == origin.attempt()
                    && completed.site() == origin.site(),
                "actual caller return attempt",
            )?;
        }
        if record.site.function_ordinal != topology.helper {
            continue;
        }
        let child = frames.get(1).ok_or("source child frame")?;
        let caller = frames.get(0).ok_or("source caller frame")?;
        demand(
            child.activation() == origin.activation() && child.activation() != 1,
            "child operation activation",
        )?;
        let SimulationDebugFrameParentV1::Caller {
            activation,
            attempt,
            call_site,
        } = child.parent()
        else {
            return Err("source helper missing actual caller".into());
        };
        demand(
            activation == caller.activation()
                && activation == 1
                && site(call_site) == json!(topology.call)
                && matches!(caller.operation_state(),SimulationDebugFrameOperationV1::Suspended { attempt: pending,site }
                if pending == attempt && site == call_site),
            "source suspended caller custody",
        )?;
        let after = owner.paired_after(index, &mut pairs).map_err(debug)?;
        let completed = owner.frames_at(after).map_err(debug)?;
        demand(
            completed.get(0).ok_or("after caller")?.legacy().values == caller.legacy().values,
            "caller SSA changed during child-local operation",
        )?;
        demand(
            completed.get(1).ok_or("after child")?.legacy().values != child.legacy().values,
            "source child operation produced no distinct retained SSA",
        )?;
        checked_helper_operations += 1;
        if helpers
            .entry(record.invocation)
            .or_default()
            .insert(child.activation())
        {
            helper_first.push(index);
            witnesses.push(
                json!({"before":checkpoint(owner,index)?,"after":checkpoint(owner,after)?,
                "caller_ssa_unchanged":true,"child_ssa_changed":true}),
            );
        }
    }
    demand(writes.len() == 4, "four source output writes")?;
    for lane in 0..4 {
        let invocation = SimulationInvocationV1 {
            global: [lane, 0, 0],
            workgroup: [0, 0, 0],
            local: [lane as u32, 0, 0],
            workgroup_size: [64, 1, 1],
            workgroup_count: [1, 1, 1],
            launch_extent: [4, 1, 1],
        };
        demand(
            helpers.get(&invocation).map_or(0, BTreeSet::len) == rounds as usize
                && call_counts.get(&invocation).copied().unwrap_or(0) == rounds as usize,
            "exact per-invocation helper count",
        )?;
    }
    demand(
        checked_helper_operations == rounds as usize * 4 * 3
            && helper_first.len() == rounds as usize * 4,
        "exact helper operation/activation count",
    )?;
    Ok((
        json!({"helper_activations":helper_first.len(),"call_attempts":call_counts.values().sum::<usize>(),
        "checked_helper_operations":checked_helper_operations,"writes":writes.len(),"witnesses":witnesses,
        "pair_scan_work":1_000_000-pairs.remaining()}),
        helper_first,
    ))
}
fn navigation(owner: DebugObservedTranscriptV1, firsts: &[usize]) -> Result<Value, String> {
    let Some(&first) = firsts.first() else {
        return Ok(json!({"checks":0,"reason":"no helper activation for zero rounds"}));
    };
    let baseline = checkpoint(&owner, first)?;
    let focus = owner.legacy().records()[first].invocation;
    let activation = owner.origin_at(first).map_err(debug)?.activation();
    let after = owner
        .paired_after(
            first,
            &mut RuntimeOriginScanWorkV1::new(65_536).map_err(fail)?,
        )
        .map_err(debug)?;
    let completed = checkpoint(&owner, after)?;
    let later = firsts
        .iter()
        .copied()
        .find(|index| owner.legacy().records()[*index].invocation == focus && *index != first);
    let mut session = owner.into_session();
    let mut work = work()?;
    session.seek_record(first, &mut work).map_err(debug)?;
    demand(
        current_checkpoint(&session)? == baseline,
        "initial selected activation/value roster",
    )?;
    session
        .step_over(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
        .map_err(debug)?;
    demand(
        current_checkpoint(&session)? == completed,
        "forward exact attempt/value roster",
    )?;
    session
        .step_over(RuntimeNavigationDirectionV1::Reverse, focus, &mut work)
        .map_err(debug)?;
    demand(
        current_checkpoint(&session)? == baseline,
        "reverse restores historical activation/value roster",
    )?;
    session
        .step_over(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
        .map_err(debug)?;
    demand(
        current_checkpoint(&session)? == completed,
        "repeat must not mint identities",
    )?;
    session.seek_record(first, &mut work).map_err(debug)?;
    session
        .step_out(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
        .map_err(debug)?;
    demand(
        session
            .current_frames()
            .map_err(debug)?
            .by_activation(activation)
            .is_none(),
        "retired helper selection persisted",
    )?;
    let returned = current_checkpoint(&session)?;
    session.seek_record(first, &mut work).map_err(debug)?;
    session
        .step_out(RuntimeNavigationDirectionV1::Reverse, focus, &mut work)
        .map_err(debug)?;
    let before_call = current_checkpoint(&session)?;
    if let Some(later) = later {
        session.seek_record(later, &mut work).map_err(debug)?;
        demand(
            session
                .current_frames()
                .map_err(debug)?
                .by_activation(activation)
                .is_none(),
            "old activation followed reused depth",
        )?;
        session.seek_record(first, &mut work).map_err(debug)?;
        demand(
            current_checkpoint(&session)? == baseline,
            "historical selection not restored",
        )?;
    }
    Ok(
        json!({"checks":if later.is_some(){8}else{6},"first":baseline,"completed":completed,
        "caller_before":before_call,"caller_after":returned,"old_activation_absent_at_later_call":later.is_some(),
        "later_record":later,"replay_work_used":1_000_000_000-work.remaining()}),
    )
}
pub(super) fn observe(bytes: Vec<u8>) -> Result<Value, String> {
    let digest = hash(&bytes);
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes).map_err(fail)?;
    bundle.revalidate().map_err(fail)?;
    demand(
        bundle.target() == "gfx942:xnack-" && bundle.kernel_count() == 1,
        "loop bundle target/kernel",
    )?;
    let (canonical, module) = VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
        bundle.canonical_kir_v11().to_vec(),
    )
    .map_err(fail)?;
    let topology = topology::select(&module)?;
    drop(module);
    let admitted = AdmittedSimulationModuleV1::admit_v11(canonical, limits(false)).map_err(fail)?;
    let mut cases = Vec::new();
    let mut total = 0;
    for rounds in [0, 1, 3] {
        for seeded in [false, true] {
            let request = request(rounds)?;
            let (off, off_owner) = capture(&admitted, &request, false, seeded, false)?;
            let (on, owner) = capture(&admitted, &request, false, seeded, true)?;
            demand(
                on == off && owner.legacy() == off_owner.legacy(),
                "reuse/observation changed loop execution or legacy records",
            )?;
            demand(
                owner.capture_instance() != off_owner.capture_instance(),
                "capture owner collision",
            )?;
            check_output(&off, &request, rounds)?;
            check_output(&on, &request, rounds)?;
            let (observations, firsts) = check_frames(&owner, &topology, rounds)?;
            total += firsts.len();
            let usage = usage(&owner);
            let navigation = navigation(owner, &firsts)?;
            let buffer = on
                .shared_buffer(BufferBackingIdV1(0))
                .ok_or("loop output")?;
            cases.push(json!({"rounds":rounds,"schedule":if seeded{"seeded_71"}else{"canonical"},
                "expected_word":expected(rounds),"output_bytes":buffer.bytes(),"initialized":buffer.initialized(),
                "reuse_off_execution_equal":true,"reuse_off_legacy_equal":true,
                "records":off_owner.legacy().records().len(),"observations":observations,"navigation":navigation,"usage":usage}));
        }
    }
    demand(total == 32, "source total helper activations")?;
    Ok(
        json!({"bundle_sha256":digest,"bundle_identity":hex(bundle.identity().as_bytes()),"target":bundle.target(),
        "canonical_kir_digest":hex(bundle.canonical_kir_v11_digest()),"canonical_kir_sha256":hash(bundle.canonical_kir_v11()),
        "canonical_kir_bytes":bundle.canonical_kir_v11().len(),"topology":topology.json(),
        "source_authenticated":false,"compiler_resume_authority":false,"hardware_observed":false,
        "cases":cases,"reuse_on_runs":6,"reuse_off_runs":6,"helper_activations":total}),
    )
}
