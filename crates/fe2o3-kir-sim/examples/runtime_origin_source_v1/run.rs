use fe2o3_kernel_ir::{
    AccessMode, ScalarType, VerifiedCanonicalKernelIrV11, VerifiedSimulationBundleV6,
};
use fe2o3_kir_sim::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[path = "collector.rs"]
mod collector;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[path = "topology.rs"]
mod topology;
use collector::Capture;

const BUNDLE_CAP: usize = 256 * 1024;
const REPORT_CAP: usize = 512 * 1024;
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

fn hash(bytes: &[u8]) -> String {
    hash_identity(&<[u8; 32]>::from(Sha256::digest(bytes)))
}
fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: BUNDLE_CAP,
        max_reachable_functions: 8,
        max_reachable_operations: 256,
        max_invocations: 4,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: 4096,
        max_call_depth: 8,
        max_ssa_values: 256,
        max_allocations: 16,
        max_allocation_bytes: 4096,
        max_total_bytes: 16 * 1024,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 8192,
        max_memory_access_records: 256,
    }
}

fn request(rounds: u32) -> Result<SimulationRequestV1, String> {
    let values = [
        0xdeadbeef, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xcafebabe,
    ]
    .map(ScalarBitsV1::u32);
    let buffer = BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &values, TARGET)
        .map_err(display)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        4,
        TARGET,
    )
    .map_err(display)?;
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

fn oracle(rounds: u32) -> u32 {
    let mut value = 0xabcd1234;
    for iteration in 0..(rounds & 3) {
        value = (value ^ (0x0f0f55aa ^ iteration)) & 0xffff;
    }
    value
}

fn check_outputs(
    result: &SimulationExecutionV1,
    request: &SimulationRequestV1,
    expected: u32,
) -> Result<(), String> {
    if result.arguments() != request.arguments
        || result.invocations_executed() != 4
        || result.workgroups_visited() != 1
        || result.scheduled_slots_visited() != 64
        || result.shared_buffers().len() != 1
    {
        return Err("source output ABI/counts".into());
    }
    let buffer = result
        .shared_buffer(BufferBackingIdV1(0))
        .ok_or("source output backing")?;
    check_buffer(buffer, expected)
}

fn check_buffer(buffer: &BufferArgumentV1, expected: u32) -> Result<(), String> {
    let wanted = [
        0xdeadbeef, expected, expected, expected, expected, 0xcafebabe,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    if buffer.bytes() != wanted || buffer.initialized() != [true; 24] {
        return Err("source output/canary/initialization mismatch".into());
    }
    Ok(())
}

fn observe(bytes: Vec<u8>) -> Result<Value, String> {
    let bundle_hash = hash(&bytes);
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes).map_err(display)?;
    bundle.revalidate().map_err(display)?;
    if bundle.target() != "gfx942:xnack-" || bundle.kernel_count() != 1 {
        return Err("source bundle target/kernel profile".into());
    }
    let (owner, module) = VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
        bundle.canonical_kir_v11().to_vec(),
    )
    .map_err(display)?;
    // No simulation occurs until this actual retained topology gate succeeds.
    let topology = topology::select(&module)?;
    drop(module);
    let admitted = AdmittedSimulationModuleV1::admit_v11(owner, limits()).map_err(display)?;
    let capture = SimulationDebugCaptureLimitsV1::new(4, 32, 1, 24).map_err(display)?;
    let mut cases = Vec::new();
    let mut helper_activations = 0_u64;
    for rounds in [0, 1, 3] {
        for seeded in [false, true] {
            let request = request(rounds)?;
            let schedule = if seeded {
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 71,
                    max_decisions: 4096,
                }
            } else {
                SimulationScheduleRequestV1::RecordCanonical {
                    max_decisions: 4096,
                }
            };
            let mut contextual = Capture::new(true)?;
            let observed = admitted
                .simulate_debugged_scheduled_with_sink(
                    &request,
                    TARGET,
                    limits(),
                    schedule,
                    capture,
                    &mut contextual,
                )
                .map_err(display)?;
            contextual.ensure()?;
            let mut legacy = Capture::new(false)?;
            let baseline = admitted
                .simulate_debugged_scheduled_with_sink(
                    &request,
                    TARGET,
                    limits(),
                    schedule,
                    capture,
                    &mut legacy,
                )
                .map_err(display)?;
            contextual.same_legacy(&legacy)?;
            if observed != baseline {
                return Err("source observation changed execution".into());
            }
            let expected = oracle(rounds);
            check_outputs(&observed, &request, expected)?;
            let observation = contextual.validate(&topology, rounds, expected)?;
            helper_activations += observation["helper_activations"]
                .as_u64()
                .ok_or("helper count")?;
            cases.push(json!({"rounds":rounds,"schedule":if seeded {"seeded_71"} else {"canonical"},
                "expected_word":expected,"invocations":observed.invocations_executed(),
                "steps":observed.steps_executed(),"output_bytes":observed.shared_buffers()[0].buffer.bytes(),
                "observation":observation,"full_execution_equal":true,
                "compact_legacy_records_equal":true}));
        }
    }
    if helper_activations != 32 {
        return Err("source aggregate measured helper count".into());
    }
    Ok(
        json!({"schema":"task-runtime-origin-source-observer-v1","status":"passed",
        "bundle_sha256":bundle_hash,"bundle_identity":hash_identity(bundle.identity().as_bytes()),
        "canonical_kir_sha256":hash(bundle.canonical_kir_v11()),
        "canonical_kir_digest":hash_identity(bundle.canonical_kir_v11_digest()),
        "canonical_kir_bytes":bundle.canonical_kir_v11().len(),
        "production_kir_identity":format!("{:?}",bundle.production_kir_identity()),
        "kernel_abi_identity":hash_identity(bundle.kernel_abi_identity()),
        "semantic_mir_identity":hash_identity(&bundle.semantic_mir_identity()),
        "target":bundle.target(),"topology":topology.json(),"cases":cases,
        "contextual_runs":6,"opt_out_runs":6,"helper_activations":helper_activations,
        "source_authenticated":false,"hardware_observed":false,"compiler_resume_authority":false,
        "scope":"exact retained ordinary-source CPU cases; no per-frame or serialized debugger identity"}),
    )
}

fn hash_identity(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn arguments(args: Vec<String>) -> Result<(String, String), String> {
    if args.len() != 2
        || !Path::new(&args[0]).is_absolute()
        || args[0].len() > 4096
        || args[0].chars().any(char::is_control)
        || args[1].len() != 64
        || !args[1]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(
            "usage: observe_runtime_origin_source_v1 ABSOLUTE_BUNDLE EXPECTED_SHA256".into(),
        );
    }
    Ok((args[0].clone(), args[1].clone()))
}

fn execute() -> Result<(), String> {
    let (path, expected_hash) = arguments(std::env::args().skip(1).collect())?;
    if !std::fs::symlink_metadata(&path).map_err(display)?.is_file() {
        return Err("bundle must be a regular non-symlink input".into());
    }
    let file = File::open(path).map_err(display)?;
    if !file.metadata().map_err(display)?.is_file()
        || file.metadata().map_err(display)?.len() > BUNDLE_CAP as u64
    {
        return Err("bundle input cap/type".into());
    }
    let mut bytes = Vec::new();
    file.take(BUNDLE_CAP as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(display)?;
    if bytes.len() > BUNDLE_CAP || hash(&bytes) != expected_hash {
        return Err("bundle exact byte hash/cap".into());
    }
    let report = serde_json::to_vec(&observe(bytes)?).map_err(display)?;
    if report.len() >= REPORT_CAP {
        return Err("observer cumulative report cap".into());
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&report).map_err(display)?;
    stdout.write_all(b"\n").map_err(display)
}

pub(super) fn main() -> std::process::ExitCode {
    match execute() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "runtime-origin acceptance refused: {}",
                error.chars().take(2048).collect::<String>()
            );
            std::process::ExitCode::FAILURE
        }
    }
}
