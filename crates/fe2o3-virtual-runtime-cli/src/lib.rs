#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_kernel_ir::AccessMode;
use fe2o3_kir_sim::{SimulationArgumentV1, SimulationRequestV1};
use fe2o3_kir_sim_cli::{
    AdmittedSimulationInputV1, load_debug_simulation_bundle_v1, load_debug_simulation_input_v1,
};
use fe2o3_runtime_model::IdentityDigestV1;
use fe2o3_runtime_model::TransitionErrorV1;
use fe2o3_virtual_runtime::{
    VirtualArgumentV1, VirtualBufferAccessV1, VirtualBufferHandleV1, VirtualDispatchRequestV1,
    VirtualRunProgressV1, VirtualRuntimeConfigV1, VirtualRuntimeErrorV1, VirtualRuntimeLimitsV1,
    VirtualRuntimeV1, VirtualTargetProfileV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: fe2o3-virtual-runtime (--kir-v7 PATH [--target amdgpu64-target-neutral|gfx942:xnack-|gfx950:xnack-] | --bundle PATH) --request PATH [--repeat 1..256] [--fault early-release]";
const MAX_REPEAT: usize = 256;
const MAX_SNAPSHOT_BYTES_V1: usize = 16 * 1024 * 1024;
const MAX_RESPONSE_BYTES_V1: usize = 48 * 1024 * 1024;
const RESPONSE_FIXED_ALLOWANCE_V1: usize = 16 * 1024;
const RESPONSE_DISPATCH_ALLOWANCE_V1: usize = 768;

#[derive(Debug)]
struct CommandV1 {
    input: InputV1,
    request: PathBuf,
    repeat: usize,
    target: Option<VirtualTargetProfileV1>,
    fault: Option<FaultV1>,
}

#[derive(Debug)]
enum InputV1 {
    Kir(PathBuf),
    Bundle(PathBuf),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultV1 {
    EarlyRelease,
}

#[derive(Serialize)]
struct SuccessV1 {
    schema: &'static str,
    status: &'static str,
    authority: &'static str,
    simulated: bool,
    hardware_observed: bool,
    performance_prediction: bool,
    target: &'static str,
    kir: KirIdentityV1,
    request_sha256: String,
    request_bytes: u64,
    bundle_sha256: Option<String>,
    lifecycle: LifecycleV1,
    dispatches: Vec<DispatchResultV1>,
    buffers: Vec<BufferResultV1>,
}

#[derive(Serialize)]
struct LifecycleV1 {
    schema: &'static str,
    runtime_identity: String,
    module: u64,
    queue: u64,
    allocations: usize,
    host_input_copies: usize,
    serial_dependency_edges: usize,
    completed_dispatches: usize,
    terminal_buffer_state: &'static str,
    terminal_module_state: &'static str,
    terminal_queue_state: &'static str,
}

#[derive(Serialize)]
struct KirIdentityV1 {
    sha256: String,
    canonical_bytes: u64,
}

#[derive(Serialize)]
struct DispatchResultV1 {
    completion: u64,
    depends_on: Option<u64>,
    state: &'static str,
    invocations: u64,
    workgroups: u64,
    scheduled_slots: u64,
    steps: u64,
    schedule: &'static str,
    schedule_transcript_sha256: String,
    schedule_decisions: u64,
    schedule_barrier_releases: u64,
    conflict_state: &'static str,
    race_state: &'static str,
}

#[derive(Serialize)]
struct BufferResultV1 {
    id: u64,
    bytes: String,
    initialized: String,
}

#[derive(Serialize)]
struct ErrorV1<'a> {
    schema: &'static str,
    status: &'static str,
    stage: &'a str,
    code: &'a str,
    message: String,
    authority: &'static str,
    hardware_observed: bool,
    performance_prediction: bool,
}

pub fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(success) => {
            let mut stdout = std::io::stdout().lock();
            if serde_json::to_writer(&mut stdout, &success).is_err()
                || stdout.write_all(b"\n").is_err()
            {
                emit_error(
                    "output",
                    "stdout_write_failed",
                    "could not write complete result",
                );
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            emit_error(&error.stage, &error.code, &error.message);
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct CommandErrorV1 {
    stage: String,
    code: String,
    message: String,
}

fn run(arguments: impl Iterator<Item = OsString>) -> Result<SuccessV1, CommandErrorV1> {
    let command = parse(arguments)?;
    let (input, profile, bundle_sha256) = match command.input {
        InputV1::Kir(path) => {
            let admitted = load_debug_simulation_input_v1(&path, &command.request)
                .map_err(|error| admission_error(error.stage, error.code, error.message))?;
            (
                admitted,
                command
                    .target
                    .unwrap_or(VirtualTargetProfileV1::Amdgpu64TargetNeutral),
                None,
            )
        }
        InputV1::Bundle(path) => {
            if command.target.is_some() {
                return Err(argument_error("--target is invalid with --bundle"));
            }
            let admitted = load_debug_simulation_bundle_v1(&path, &command.request)
                .map_err(|error| admission_error(error.stage, error.code, error.message))?;
            let profile =
                parse_target(admitted.bundle().target()).ok_or_else(|| CommandErrorV1 {
                    stage: "admission".to_owned(),
                    code: "unsupported_bundle_target".to_owned(),
                    message: format!(
                        "unsupported exact simulation bundle target {}",
                        admitted.bundle().target()
                    ),
                })?;
            let identity = admitted
                .input()
                .simulation_bundle_identity()
                .map(|identity| hex(&identity));
            let (input, _) = admitted.into_parts();
            (input, profile, identity)
        }
    };
    execute(input, profile, bundle_sha256, command.repeat, command.fault)
}

fn execute(
    input: AdmittedSimulationInputV1,
    profile: VirtualTargetProfileV1,
    bundle_sha256: Option<String>,
    repeat: usize,
    fault: Option<FaultV1>,
) -> Result<SuccessV1, CommandErrorV1> {
    let kir_identity = *input.module.identity();
    let request_sha256 = input.request_sha256;
    let request_bytes = input.request_bytes();
    let runtime_identity = derive_runtime_identity(&input, repeat, fault);
    let mut simulation_limits = input.simulation_limits;
    simulation_limits.max_allocation_bytes = simulation_limits.max_allocation_bytes.min(16 << 20);
    simulation_limits.max_total_bytes = simulation_limits.max_total_bytes.min(64 << 20);
    simulation_limits.max_resident_bytes = simulation_limits.max_resident_bytes.min(256 << 20);
    let mut runtime = VirtualRuntimeV1::new(VirtualRuntimeConfigV1 {
        runtime_identity,
        target: profile,
        runtime_limits: VirtualRuntimeLimitsV1 {
            max_user_allocations: 3_968,
            max_total_user_bytes: MAX_SNAPSHOT_BYTES_V1,
            max_modules: 1,
            max_queues: 1,
            max_dispatches: MAX_REPEAT,
            max_dependencies_per_dispatch: 1,
            max_schedule_decisions: 1 << 20,
        },
        simulation_limits,
    })
    .map_err(runtime_error)?;
    let (template, buffers) =
        import_request(&mut runtime, input.request, profile).map_err(runtime_error)?;
    let module = runtime
        .register_module(input.module)
        .map_err(runtime_error)?;
    let queue = runtime.create_queue(256).map_err(runtime_error)?;
    let mut completions = Vec::new();
    for index in 0..repeat {
        let dependency = completions.last().copied();
        let mut request = template.clone();
        request.dependencies = dependency.into_iter().collect();
        let completion = runtime
            .submit(queue, module, request)
            .map_err(runtime_error)?;
        completions.push(completion);
        if index == 0 && fault == Some(FaultV1::EarlyRelease) {
            let buffer = buffers.first().copied().ok_or_else(|| CommandErrorV1 {
                stage: "fault_injection".to_owned(),
                code: "fault_not_applicable".to_owned(),
                message: "early-release fault requires at least one virtual buffer".to_owned(),
            })?;
            return match runtime.release_buffer(buffer) {
                Err(error) => Err(runtime_error(error)),
                Ok(()) => Err(CommandErrorV1 {
                    stage: "fault_injection".to_owned(),
                    code: "fault_not_observed".to_owned(),
                    message: "early release unexpectedly succeeded".to_owned(),
                }),
            };
        }
        let progress = runtime.run_next().map_err(runtime_error)?;
        if !matches!(progress, VirtualRunProgressV1::Completed { completion: observed, .. } if observed == completion)
        {
            return Err(CommandErrorV1 {
                stage: "execution".to_owned(),
                code: "unexpected_scheduler_state".to_owned(),
                message: format!("serial dispatch {index} did not complete deterministically"),
            });
        }
    }
    let mut dispatches = Vec::new();
    for (index, completion) in completions.iter().copied().enumerate() {
        let summary = runtime
            .completion_summary(completion)
            .map_err(runtime_error)?
            .ok_or_else(|| CommandErrorV1 {
                stage: "execution".to_owned(),
                code: "missing_completion_summary".to_owned(),
                message: format!(
                    "completion {} has no successful summary",
                    completion.ordinal()
                ),
            })?;
        dispatches.push(DispatchResultV1 {
            completion: completion.ordinal(),
            depends_on: index
                .checked_sub(1)
                .map(|prior| completions[prior].ordinal()),
            state: "completed",
            invocations: summary.invocations_executed,
            workgroups: summary.workgroups_visited,
            scheduled_slots: summary.scheduled_slots_visited,
            steps: summary.steps_executed,
            schedule: schedule_name(summary.schedule),
            schedule_transcript_sha256: hex(&summary.schedule_transcript_identity),
            schedule_decisions: summary.schedule_decisions,
            schedule_barrier_releases: summary.schedule_barrier_releases,
            conflict_state: match summary.conflict_state {
                fe2o3_virtual_runtime::VirtualConflictStateV1::NoneObserved => "none_observed",
                fe2o3_virtual_runtime::VirtualConflictStateV1::Observed => "observed",
                fe2o3_virtual_runtime::VirtualConflictStateV1::Incomplete => "incomplete",
            },
            race_state: match summary.race_state {
                fe2o3_virtual_runtime::VirtualRaceStateV1::NoneObserved => "none_observed",
                fe2o3_virtual_runtime::VirtualRaceStateV1::Observed => "observed",
                fe2o3_virtual_runtime::VirtualRaceStateV1::Incomplete => "incomplete",
            },
        });
    }
    let mut result_buffers = Vec::new();
    enforce_response_bound(
        &buffers
            .iter()
            .map(|buffer| {
                runtime
                    .buffer_snapshot(*buffer)
                    .map(|snapshot| snapshot.bytes.len())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(runtime_error)?,
        completions.len(),
    )?;
    for buffer in &buffers {
        let snapshot = runtime.buffer_snapshot(*buffer).map_err(runtime_error)?;
        result_buffers.push(BufferResultV1 {
            id: buffer.ordinal(),
            bytes: hex(snapshot.bytes),
            initialized: initialization_hex(snapshot.initialized),
        });
    }
    for buffer in buffers.iter().copied() {
        runtime.release_buffer(buffer).map_err(runtime_error)?;
    }
    runtime.release_module(module).map_err(runtime_error)?;
    runtime.release_queue(queue).map_err(runtime_error)?;
    Ok(SuccessV1 {
        schema: "fe2o3-virtual-runtime-result-v1",
        status: "ok",
        authority: "observation_only",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        target: profile.label(),
        kir: KirIdentityV1 {
            sha256: hex(kir_identity.digest()),
            canonical_bytes: kir_identity.canonical_length(),
        },
        request_sha256: hex(&request_sha256),
        request_bytes,
        bundle_sha256,
        lifecycle: LifecycleV1 {
            schema: "fe2o3-virtual-runtime-lifecycle-v1",
            runtime_identity: hex(runtime_identity.as_bytes()),
            module: module.ordinal(),
            queue: queue.ordinal(),
            allocations: buffers.len(),
            host_input_copies: buffers.len(),
            serial_dependency_edges: repeat.saturating_sub(1),
            completed_dispatches: dispatches.len(),
            terminal_buffer_state: "released",
            terminal_module_state: "released",
            terminal_queue_state: "released",
        },
        dispatches,
        buffers: result_buffers,
    })
}

fn import_request(
    runtime: &mut VirtualRuntimeV1,
    request: SimulationRequestV1,
    profile: VirtualTargetProfileV1,
) -> Result<(VirtualDispatchRequestV1, Vec<VirtualBufferHandleV1>), VirtualRuntimeErrorV1> {
    let mut shared = Vec::new();
    let mut buffers = Vec::new();
    for backing in request.shared_buffers {
        let handle = import_buffer(runtime, &backing.buffer)?;
        shared.push((backing.id, handle));
        buffers.push(handle);
    }
    let mut arguments = Vec::new();
    for argument in request.arguments {
        match argument {
            SimulationArgumentV1::Scalar(value) => arguments.push(VirtualArgumentV1::Scalar(value)),
            SimulationArgumentV1::Buffer(buffer) => {
                let handle = import_buffer(runtime, &buffer)?;
                let elements = buffer
                    .element_count(profile.simulation_target())
                    .map_err(|error| VirtualRuntimeErrorV1::SimulatorBuffer(error.to_string()))?;
                arguments.push(VirtualArgumentV1::Buffer {
                    buffer: handle,
                    element: buffer.element(),
                    access: buffer.access(),
                    alignment: buffer.alignment(),
                    byte_offset: 0,
                    elements,
                });
                buffers.push(handle);
            }
            SimulationArgumentV1::BufferView(view) => {
                let handle = shared
                    .iter()
                    .find(|(id, _)| *id == view.backing())
                    .map(|(_, handle)| *handle)
                    .ok_or_else(|| {
                        VirtualRuntimeErrorV1::SimulatorBuffer(format!(
                            "request view references missing backing {}",
                            view.backing().0
                        ))
                    })?;
                arguments.push(VirtualArgumentV1::Buffer {
                    buffer: handle,
                    element: view.element(),
                    access: view.access(),
                    alignment: view.alignment(),
                    byte_offset: view.byte_offset(),
                    elements: view.elements(),
                });
            }
        }
    }
    Ok((
        VirtualDispatchRequestV1 {
            kernel: request.kernel,
            grid: request.grid.0,
            workgroup: request.workgroup.0,
            arguments,
            dependencies: Vec::new(),
        },
        buffers,
    ))
}

fn import_buffer(
    runtime: &mut VirtualRuntimeV1,
    buffer: &fe2o3_kir_sim::BufferArgumentV1,
) -> Result<VirtualBufferHandleV1, VirtualRuntimeErrorV1> {
    let access = match buffer.access() {
        AccessMode::ReadOnly => VirtualBufferAccessV1::ReadOnly,
        AccessMode::WriteOnly | AccessMode::ReadWrite => VirtualBufferAccessV1::ReadWrite,
    };
    let handle = runtime.allocate_buffer(buffer.bytes().len(), access)?;
    runtime.copy_from_host_with_initialization(handle, 0, buffer.bytes(), buffer.initialized())?;
    Ok(handle)
}

fn parse(arguments: impl Iterator<Item = OsString>) -> Result<CommandV1, CommandErrorV1> {
    let mut arguments = arguments.peekable();
    let mut kir = None;
    let mut bundle = None;
    let mut request = None;
    let mut repeat = None;
    let mut target = None;
    let mut fault = None;
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or_else(|| argument_error(USAGE))?;
        if argument == "--kir-v7" {
            assign_once(&mut kir, PathBuf::from(value), "--kir-v7")?;
        } else if argument == "--bundle" {
            assign_once(&mut bundle, PathBuf::from(value), "--bundle")?;
        } else if argument == "--request" {
            assign_once(&mut request, PathBuf::from(value), "--request")?;
        } else if argument == "--repeat" {
            let text = value
                .to_str()
                .ok_or_else(|| argument_error("--repeat must be UTF-8"))?;
            let count = text
                .parse::<usize>()
                .map_err(|_| argument_error("--repeat must be an integer"))?;
            if count == 0 || count > MAX_REPEAT {
                return Err(argument_error("--repeat must be between 1 and 256"));
            }
            assign_once(&mut repeat, count, "--repeat")?;
        } else if argument == "--target" {
            let text = value
                .to_str()
                .ok_or_else(|| argument_error("--target must be UTF-8"))?;
            let profile =
                parse_target(text).ok_or_else(|| argument_error("unsupported --target profile"))?;
            assign_once(&mut target, profile, "--target")?;
        } else if argument == "--fault" {
            let text = value
                .to_str()
                .ok_or_else(|| argument_error("--fault must be UTF-8"))?;
            let selected = match text {
                "early-release" => FaultV1::EarlyRelease,
                _ => return Err(argument_error("unsupported --fault mode")),
            };
            assign_once(&mut fault, selected, "--fault")?;
        } else {
            return Err(argument_error(USAGE));
        }
    }
    let input = match (kir, bundle) {
        (Some(path), None) => InputV1::Kir(path),
        (None, Some(path)) => InputV1::Bundle(path),
        _ => return Err(argument_error(USAGE)),
    };
    Ok(CommandV1 {
        input,
        request: request.ok_or_else(|| argument_error(USAGE))?,
        repeat: repeat.unwrap_or(1),
        target,
        fault,
    })
}

fn assign_once<T>(
    slot: &mut Option<T>,
    value: T,
    name: &'static str,
) -> Result<(), CommandErrorV1> {
    if slot.replace(value).is_some() {
        return Err(argument_error(&format!("duplicate {name}")));
    }
    Ok(())
}

fn parse_target(value: &str) -> Option<VirtualTargetProfileV1> {
    match value {
        "amdgpu64-target-neutral" => Some(VirtualTargetProfileV1::Amdgpu64TargetNeutral),
        "gfx942:xnack-" => Some(VirtualTargetProfileV1::Gfx942XnackMinus),
        "gfx950:xnack-" => Some(VirtualTargetProfileV1::Gfx950XnackMinus),
        _ => None,
    }
}

fn derive_runtime_identity(
    input: &AdmittedSimulationInputV1,
    repeat: usize,
    fault: Option<FaultV1>,
) -> IdentityDigestV1 {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/VIRTUAL-RUNTIME/CLI-SESSION/V1\0");
    digest.update(input.module.identity().digest());
    digest.update(input.module.identity().canonical_length().to_le_bytes());
    digest.update(input.request_sha256);
    digest.update(input.request_bytes().to_le_bytes());
    digest.update((repeat as u64).to_le_bytes());
    digest.update([match fault {
        None => 0,
        Some(FaultV1::EarlyRelease) => 1,
    }]);
    IdentityDigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn schedule_name(value: fe2o3_kir_sim::SimulationScheduleIdentityV1) -> &'static str {
    match value {
        fe2o3_kir_sim::SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => {
            "workgroup_major_local_zyx_serial_v1"
        }
        fe2o3_kir_sim::SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => {
            "workgroup_major_local_zyx_cooperative_v1"
        }
        fe2o3_kir_sim::SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1 => {
            "workgroup_major_seeded_runnable_cooperative_v1"
        }
    }
}

fn runtime_error(error: VirtualRuntimeErrorV1) -> CommandErrorV1 {
    let code = match &error {
        VirtualRuntimeErrorV1::Simulation { .. } => "simulation_failed",
        VirtualRuntimeErrorV1::ForeignHandle { .. } => "foreign_handle",
        VirtualRuntimeErrorV1::UninitializedHostRead { .. } => "uninitialized_host_read",
        VirtualRuntimeErrorV1::Model(TransitionErrorV1::ResourceInUse(_)) => "resource_in_use",
        VirtualRuntimeErrorV1::Model(_) => "runtime_model_rejected",
        VirtualRuntimeErrorV1::ExactTargetMismatch { .. } => "exact_target_mismatch",
        VirtualRuntimeErrorV1::CapacityExceeded(_) => "resource_limit",
        _ => "runtime_misuse",
    };
    CommandErrorV1 {
        stage: "virtual_runtime".to_owned(),
        code: code.to_owned(),
        message: error.to_string(),
    }
}

fn admission_error(stage: String, code: String, message: String) -> CommandErrorV1 {
    CommandErrorV1 {
        stage,
        code,
        message,
    }
}

fn argument_error(message: &str) -> CommandErrorV1 {
    CommandErrorV1 {
        stage: "arguments".to_owned(),
        code: "invalid_command_line".to_owned(),
        message: message.to_owned(),
    }
}

fn emit_error(stage: &str, code: &str, message: &str) {
    let error = ErrorV1 {
        schema: "fe2o3-virtual-runtime-error-v1",
        status: "error",
        stage,
        code,
        message: message.to_owned(),
        authority: "observation_only",
        hardware_observed: false,
        performance_prediction: false,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &error);
    let _ = stderr.write_all(b"\n");
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn initialization_hex(initialized: &[bool]) -> String {
    let mut bytes = vec![0_u8; initialized.len().div_ceil(8)];
    for (index, initialized) in initialized.iter().copied().enumerate() {
        if initialized {
            bytes[index / 8] |= 1 << (index % 8);
        }
    }
    hex(&bytes)
}

fn enforce_response_bound(
    buffer_lengths: &[usize],
    dispatches: usize,
) -> Result<(), CommandErrorV1> {
    let snapshot_bytes = buffer_lengths
        .iter()
        .try_fold(0_usize, |total, length| total.checked_add(*length));
    let Some(snapshot_bytes) = snapshot_bytes else {
        return Err(output_limit_error());
    };
    if snapshot_bytes > MAX_SNAPSHOT_BYTES_V1 {
        return Err(output_limit_error());
    }
    let encoded_buffers = buffer_lengths.iter().try_fold(0_usize, |total, length| {
        let initialized = length.div_ceil(8);
        total
            .checked_add(length.checked_mul(2)?)
            .and_then(|total| total.checked_add(initialized.checked_mul(2)?))
            .and_then(|total| total.checked_add(192))
    });
    let encoded_dispatches = dispatches.checked_mul(RESPONSE_DISPATCH_ALLOWANCE_V1);
    let encoded = encoded_buffers
        .and_then(|bytes| bytes.checked_add(encoded_dispatches?))
        .and_then(|bytes| bytes.checked_add(RESPONSE_FIXED_ALLOWANCE_V1));
    if encoded.is_none_or(|bytes| bytes > MAX_RESPONSE_BYTES_V1) {
        return Err(output_limit_error());
    }
    Ok(())
}

fn output_limit_error() -> CommandErrorV1 {
    CommandErrorV1 {
        stage: "output".to_owned(),
        code: "response_limit".to_owned(),
        message: format!(
            "virtual runtime snapshots exceed the {} byte snapshot or {} byte encoded response bound",
            MAX_SNAPSHOT_BYTES_V1, MAX_RESPONSE_BYTES_V1
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_estimator_rejects_oversized_snapshot_before_encoding() {
        let error = enforce_response_bound(&[MAX_SNAPSHOT_BYTES_V1, 1], 1).unwrap_err();
        assert_eq!(error.stage, "output");
        assert_eq!(error.code, "response_limit");
    }

    #[test]
    fn response_estimator_accepts_the_admission_ceiling() {
        enforce_response_bound(&[MAX_SNAPSHOT_BYTES_V1], MAX_REPEAT).unwrap();
    }
}
