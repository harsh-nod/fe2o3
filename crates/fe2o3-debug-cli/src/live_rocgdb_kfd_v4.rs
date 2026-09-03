//! One-shot agent-facing launcher for exact ROCgdb/KFD native correlation.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::{
    LiveGpuContentIdentityV3, OpaqueIdentityV1, RocgdbMiExecutionEventV3, RocgdbMiNativeCapturedV5,
    RocgdbMiNativeCliResponseSchemaV4, RocgdbMiNativeCliResponseSchemaV5,
    RocgdbMiNativeCliResponseV4, RocgdbMiNativeCliResponseV5, RocgdbMiNativeCliResultV4,
    RocgdbMiNativeCliResultV5, RocgdbMiNativeInspectionProbeV5,
    RocgdbMiNativeInspectionUnavailableReasonV5, RocgdbMiNativeInspectionV5, RocgdbMiNativeProbeV4,
    RocgdbMiNativeUnavailableFieldV5, RocgdbMiNativeUnavailableReasonV4,
};
use fe2o3_kfd::{
    DeviceSelector, KfdDebuggerTelemetryEndpointV2, KfdTargetDebugSessionNonceV1,
    KfdTargetDebugTelemetryPayloadV2, KfdTargetDebugTelemetryProcessV1, OpenedKfd,
    create_kfd_target_debug_telemetry_channel_v2, derive_kfd_target_debug_generation_v2,
};
use sha2::{Digest, Sha256};

use crate::rocgdb_mi_v3::{
    RocgdbMiAdapterErrorV3, RocgdbMiAdapterLimitsV3, RocgdbMiNativeSpawnProvisionV4,
    RocgdbMiProcessV3,
};
use crate::rocgdb_mi_v4::{
    RocgdbCodeObjectBindingV4, RocgdbDirectKfdDeviceBindingV4, RocgdbInferiorBindingV4,
    RocgdbMiNativeCorrelationAdapterV4,
};

const USAGE: &str = "fe2o3-debug (live-rocgdb-kfd-v4 | live-rocgdb-kfd-v5) --rocgdb PATH --authorization ID --hsaco PATH --load-base 0xHEX --kernel NAME [--device-unique-id DECIMAL] [--protocol jsonl] [--wave-width 32|64] [--timeout-ms N] -- PROGRAM [ARG...]";
const MAX_PATH_BYTES_V4: usize = 4_096;
const MAX_ARGUMENTS_V4: usize = 256;
const MAX_ARGUMENT_BYTES_V4: usize = 32 * 1_024;
const MAX_HSACO_BYTES_V4: u64 = 1 << 31;

#[derive(Debug)]
struct OptionsV4 {
    output: OutputVersion,
    rocgdb: PathBuf,
    authorization: OpaqueIdentityV1,
    hsaco: PathBuf,
    load_base: u64,
    kernel: OsString,
    device_unique_id: Option<u64>,
    wave_width: u16,
    timeout: Duration,
    program: PathBuf,
    arguments: Vec<OsString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputVersion {
    V4,
    V5,
}

pub(crate) fn run(arguments: Vec<OsString>) -> ExitCode {
    let options = match parse_options(arguments) {
        Ok(options) => options,
        Err(message) => {
            super::write_bootstrap_error("arguments", "invalid_native_v4_command_line", &message);
            return ExitCode::FAILURE;
        }
    };
    let output = options.output;
    let mut inspection_probe = RocgdbMiNativeInspectionProbeV5::default();
    let mut inspection = None;
    let response = match run_inner(options, &mut inspection_probe, &mut inspection) {
        Ok(response) => response,
        Err(reason) => unavailable(
            RocgdbMiNativeProbeV4 {
                structured_mi_commands: false,
                direct_kfd_device_admitted: false,
                cooperative_v2_declaration: false,
                cooperative_v2_publication: false,
            },
            reason,
        ),
    };
    match output {
        OutputVersion::V4 => write_response(response),
        OutputVersion::V5 => write_response_v5(response_v5(response, inspection_probe, inspection)),
    }
}

fn run_inner(
    options: OptionsV4,
    inspection_probe: &mut RocgdbMiNativeInspectionProbeV5,
    inspection: &mut Option<RocgdbMiNativeInspectionV5>,
) -> Result<RocgdbMiNativeCliResponseV4, RocgdbMiNativeUnavailableReasonV4> {
    let session_domain = match options.output {
        OutputVersion::V4 => b"fe2o3-live-rocgdb-kfd-v4\0".as_slice(),
        OutputVersion::V5 => b"fe2o3-live-rocgdb-kfd-v5\0".as_slice(),
    };
    let session = random_identity(session_domain, options.authorization)
        .ok_or(RocgdbMiNativeUnavailableReasonV4::RocgdbSpawnFailed)?;
    let nonce = random_nonce().ok_or(RocgdbMiNativeUnavailableReasonV4::RocgdbSpawnFailed)?;
    let debugger_process = KfdTargetDebugTelemetryProcessV1::capture(std::process::id())
        .map_err(|_| RocgdbMiNativeUnavailableReasonV4::CooperativeTelemetryUnavailable)?;
    let (debugger_endpoint, target_endpoint) = create_kfd_target_debug_telemetry_channel_v2()
        .map_err(|_| RocgdbMiNativeUnavailableReasonV4::CooperativeTelemetryUnavailable)?;
    let mut process = RocgdbMiProcessV3::spawn_native_v4(
        &options.rocgdb,
        session,
        options.authorization,
        options.wave_width,
        RocgdbMiAdapterLimitsV3::default(),
        RocgdbMiNativeSpawnProvisionV4 {
            target_endpoint: &target_endpoint,
            nonce,
            debugger_pid: debugger_process.pid(),
        },
    )
    .map_err(|_| RocgdbMiNativeUnavailableReasonV4::RocgdbSpawnFailed)?;
    drop(target_endpoint);
    let mut probe = RocgdbMiNativeProbeV4 {
        structured_mi_commands: false,
        direct_kfd_device_admitted: false,
        cooperative_v2_declaration: false,
        cooperative_v2_publication: false,
    };
    probe.structured_mi_commands = match process.native_v4_commands_available(options.timeout) {
        Ok(available) => available,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::StructuredCommandsUnavailable,
            ));
        }
    };
    if !probe.structured_mi_commands {
        return Ok(unavailable(
            probe,
            RocgdbMiNativeUnavailableReasonV4::StructuredCommandsUnavailable,
        ));
    }
    if options.output == OutputVersion::V5 {
        *inspection_probe = match process.native_v5_inspection_commands(options.timeout) {
            Ok(probe) => probe,
            Err(_) => {
                return Ok(unavailable(
                    probe,
                    RocgdbMiNativeUnavailableReasonV4::StructuredCommandsUnavailable,
                ));
            }
        };
    }

    let topology = match fe2o3_kfd::topology::discover_default_topology() {
        Ok(topology) => topology,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::DirectKfdDeviceUnavailable,
            ));
        }
    };
    let devices = topology.topology().gpu_nodes();
    let selected_unique_id = match (options.device_unique_id, devices) {
        (Some(unique_id), devices)
            if devices.iter().any(|device| device.unique_id() == unique_id) =>
        {
            unique_id
        }
        (None, [device]) => device.unique_id(),
        _ => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::DirectKfdDeviceUnavailable,
            ));
        }
    };
    let admitted = match OpenedKfd::open_default().and_then(OpenedKfd::admit_uapi) {
        Ok(admitted) => admitted,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::DirectKfdDeviceUnavailable,
            ));
        }
    };
    let device =
        match admitted.bind_gfx942_xnack_minus(DeviceSelector::UniqueId(selected_unique_id)) {
            Ok(device) => device,
            Err(_) => {
                return Ok(unavailable(
                    probe,
                    RocgdbMiNativeUnavailableReasonV4::DirectKfdDeviceUnavailable,
                ));
            }
        };
    let direct_kfd = RocgdbDirectKfdDeviceBindingV4::from_checked_device(&device);
    probe.direct_kfd_device_admitted = true;

    let hsaco = match read_bounded(&options.hsaco) {
        Some(hsaco) => hsaco,
        None => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let digest = match OpaqueIdentityV1::new(Sha256::digest(&hsaco).into()) {
        Ok(digest) => digest,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let artifact = LiveGpuContentIdentityV3 {
        digest,
        canonical_bytes: hsaco.len() as u64,
    };
    let kernel_name = options
        .kernel
        .to_str()
        .expect("the argument parser requires a UTF-8 kernel name");
    let inspected = match fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&hsaco) {
        Ok(inspected) => inspected,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let selected_index = match inspected
        .inspection()
        .kernels()
        .iter()
        .position(|kernel| kernel.name() == kernel_name)
    {
        Some(index) => index,
        None => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let selected = match inspected.bindings().get(selected_index).copied() {
        Some(binding) if binding.kernel_index() == selected_index => binding,
        _ => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let code = match RocgdbCodeObjectBindingV4::new(
        artifact,
        options.load_base,
        selected.entry_address(),
        selected.entry_size(),
    ) {
        Ok(code) => code,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };

    let target_pid = match process.launch_native_v4(
        &options.program,
        &options.arguments,
        options.kernel.as_bytes(),
        options.timeout,
    ) {
        Ok(pid) => pid,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::TargetLaunchFailed,
            ));
        }
    };
    let target = match KfdTargetDebugTelemetryProcessV1::capture(target_pid) {
        Ok(target) => target,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::TargetExitedBeforePublication,
            ));
        }
    };
    let expected_process = match target.correlation_identity_v2() {
        Ok(identity) => identity,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CooperativeTelemetryUnavailable,
            ));
        }
    };
    let expected_generation = derive_kfd_target_debug_generation_v2(nonce);
    let mut telemetry =
        match KfdDebuggerTelemetryEndpointV2::admit(debugger_endpoint, nonce, target) {
            Ok(telemetry) => telemetry,
            Err(_) => {
                return Ok(unavailable(
                    probe,
                    RocgdbMiNativeUnavailableReasonV4::CooperativeTelemetryUnavailable,
                ));
            }
        };

    let deadline = Instant::now() + options.timeout;
    let mut declaration = None;
    let mut publication = None;
    let mut stopped = false;
    let mut mi_terminal = false;
    let mut telemetry_terminal = false;
    while Instant::now() < deadline {
        let mut telemetry_progress = false;
        match telemetry.try_receive() {
            Ok(Some(record)) => {
                telemetry_progress = true;
                telemetry_terminal |= observe_native_telemetry_v4(
                    record.payload(),
                    &mut probe,
                    &mut declaration,
                    &mut publication,
                );
            }
            Ok(None) => {}
            Err(_) => {
                return Ok(unavailable(
                    probe,
                    RocgdbMiNativeUnavailableReasonV4::CooperativeTelemetryUnavailable,
                ));
            }
        }
        if stopped
            && process.native_v4_stop_is_current()
            && declaration.is_some()
            && publication.is_some()
            || telemetry_terminal
            || mi_terminal && publication.is_some()
        {
            break;
        }
        if !mi_terminal {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let poll = remaining.min(Duration::from_millis(10));
            if poll.is_zero() {
                break;
            }
            match process.next_event(poll) {
                Ok(RocgdbMiExecutionEventV3::Stopped { .. }) => stopped = true,
                Ok(RocgdbMiExecutionEventV3::Unavailable { .. })
                    if process.native_v4_stop_is_current() =>
                {
                    stopped = true
                }
                Ok(RocgdbMiExecutionEventV3::Exited { .. })
                | Ok(RocgdbMiExecutionEventV3::Unavailable { .. }) => mi_terminal = true,
                Ok(RocgdbMiExecutionEventV3::Running { .. }) => stopped = false,
                Err(RocgdbMiAdapterErrorV3::Timeout) => {}
                Err(_) => mi_terminal = true,
            }
        } else if !telemetry_progress {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    stopped &= process.native_v4_stop_is_current();
    let reason = native_collection_unavailable_reason_v4(stopped, &declaration, &publication);
    if let Some(reason) = reason {
        return Ok(unavailable(probe, reason));
    }
    let declaration = declaration.expect("collection prerequisite requires a declaration");
    let publication = publication.expect("collection prerequisite requires publication");
    let mut correlation = RocgdbMiNativeCorrelationAdapterV4::new(session);
    if process
        .collect_native_hierarchy_v4(&mut correlation, options.timeout)
        .is_err()
    {
        return Ok(unavailable(
            probe,
            RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
        ));
    }
    let process_instance = match OpaqueIdentityV1::new(*expected_process.as_bytes()) {
        Ok(identity) => identity,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let inferior = match RocgdbInferiorBindingV4::new(process_instance, expected_generation) {
        Ok(inferior) => inferior,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    let stopped_state = match correlation.correlate_telemetry(
        &declaration,
        &publication,
        direct_kfd,
        inferior,
        code,
    ) {
        Ok(stopped) => stopped,
        Err(_) => {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            ));
        }
    };
    if options.output == OutputVersion::V5 {
        let (raw_thread, scope) = match correlation.inspection_scope_v5(&stopped_state) {
            Ok(authority) => authority,
            Err(_) => {
                return Ok(unavailable(
                    probe,
                    RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
                ));
            }
        };
        let registers = if inspection_probe.register_names && inspection_probe.register_values {
            match process.inspect_native_registers_v5(&raw_thread, scope, options.timeout) {
                Ok((value, evidence_identity)) => RocgdbMiNativeCapturedV5::Captured {
                    evidence_identity,
                    value,
                },
                Err(_) => RocgdbMiNativeCapturedV5::Unavailable {
                    reason: RocgdbMiNativeInspectionUnavailableReasonV5::BackendRejected,
                },
            }
        } else {
            RocgdbMiNativeCapturedV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable,
            }
        };
        if !process.native_v4_stop_is_current() {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
            ));
        }
        let locals = if inspection_probe.simple_locals {
            match process.inspect_native_locals_v5(&raw_thread, scope, options.timeout) {
                Ok((value, evidence_identity)) => RocgdbMiNativeCapturedV5::Captured {
                    evidence_identity,
                    value,
                },
                Err(_) => RocgdbMiNativeCapturedV5::Unavailable {
                    reason: RocgdbMiNativeInspectionUnavailableReasonV5::BackendRejected,
                },
            }
        } else {
            RocgdbMiNativeCapturedV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable,
            }
        };
        if !process.native_v4_stop_is_current() {
            return Ok(unavailable(
                probe,
                RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
            ));
        }
        *inspection = Some(RocgdbMiNativeInspectionV5 {
            association_identity: stopped_state.association_identity,
            scope,
            registers,
            locals,
            source: RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAuthenticatedSourceMap,
            },
            isa: RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresArtifactRelativeInstructionBinding,
            },
            memory: RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAllocationRelativeAuthority,
            },
        });
    }
    Ok(RocgdbMiNativeCliResponseV4 {
        schema: RocgdbMiNativeCliResponseSchemaV4::V4,
        result: RocgdbMiNativeCliResultV4::Available {
            probe,
            stopped_state: Box::new(stopped_state),
        },
    })
}

fn response_v5(
    response: RocgdbMiNativeCliResponseV4,
    inspection_probe: RocgdbMiNativeInspectionProbeV5,
    inspection: Option<RocgdbMiNativeInspectionV5>,
) -> RocgdbMiNativeCliResponseV5 {
    let result = match response.result {
        RocgdbMiNativeCliResultV4::Available {
            probe,
            stopped_state,
        } => match inspection {
            Some(inspection) => RocgdbMiNativeCliResultV5::Available {
                probe,
                inspection_probe,
                stopped_state,
                inspection: Box::new(inspection),
            },
            None => RocgdbMiNativeCliResultV5::Unavailable {
                probe,
                inspection_probe,
                reason: RocgdbMiNativeUnavailableReasonV4::CorrelationRejected,
            },
        },
        RocgdbMiNativeCliResultV4::Unavailable { probe, reason } => {
            RocgdbMiNativeCliResultV5::Unavailable {
                probe,
                inspection_probe,
                reason,
            }
        }
    };
    RocgdbMiNativeCliResponseV5 {
        schema: RocgdbMiNativeCliResponseSchemaV5::V5,
        result,
    }
}

fn observe_native_telemetry_v4(
    payload: &KfdTargetDebugTelemetryPayloadV2,
    probe: &mut RocgdbMiNativeProbeV4,
    declaration: &mut Option<KfdTargetDebugTelemetryPayloadV2>,
    publication: &mut Option<KfdTargetDebugTelemetryPayloadV2>,
) -> bool {
    match payload {
        KfdTargetDebugTelemetryPayloadV2::DispatchDeclared { .. } => {
            probe.cooperative_v2_declaration = true;
            *declaration = Some(payload.clone());
            false
        }
        KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished { .. } => {
            probe.cooperative_v2_publication = true;
            *publication = Some(payload.clone());
            false
        }
        KfdTargetDebugTelemetryPayloadV2::SessionEnded { .. } => true,
        _ => true,
    }
}

fn native_collection_unavailable_reason_v4(
    stopped: bool,
    declaration: &Option<KfdTargetDebugTelemetryPayloadV2>,
    publication: &Option<KfdTargetDebugTelemetryPayloadV2>,
) -> Option<RocgdbMiNativeUnavailableReasonV4> {
    if declaration.is_none() || publication.is_none() {
        Some(RocgdbMiNativeUnavailableReasonV4::NativePublicationNotObserved)
    } else if !stopped {
        Some(RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable)
    } else {
        None
    }
}

fn unavailable(
    probe: RocgdbMiNativeProbeV4,
    reason: RocgdbMiNativeUnavailableReasonV4,
) -> RocgdbMiNativeCliResponseV4 {
    RocgdbMiNativeCliResponseV4 {
        schema: RocgdbMiNativeCliResponseSchemaV4::V4,
        result: RocgdbMiNativeCliResultV4::Unavailable { probe, reason },
    }
}

fn write_response(response: RocgdbMiNativeCliResponseV4) -> ExitCode {
    if response.validate().is_err() {
        return ExitCode::FAILURE;
    }
    let mut bytes = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(_) => return ExitCode::FAILURE,
    };
    bytes.push(b'\n');
    if std::io::stdout().lock().write_all(&bytes).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn write_response_v5(response: RocgdbMiNativeCliResponseV5) -> ExitCode {
    if response.validate().is_err() {
        return ExitCode::FAILURE;
    }
    let mut bytes = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(_) => return ExitCode::FAILURE,
    };
    bytes.push(b'\n');
    if std::io::stdout().lock().write_all(&bytes).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn read_bounded(path: &PathBuf) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length == 0 || length > MAX_HSACO_BYTES_V4 {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(length).ok()?);
    file.take(MAX_HSACO_BYTES_V4 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? == length).then_some(bytes)
}

fn random_nonce() -> Option<KfdTargetDebugSessionNonceV1> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    KfdTargetDebugSessionNonceV1::from_bytes(bytes).ok()
}

fn random_identity(domain: &[u8], authorization: OpaqueIdentityV1) -> Option<OpaqueIdentityV1> {
    let mut random = [0_u8; 32];
    File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut random)
        .ok()?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(random);
    digest.update(authorization.as_bytes());
    OpaqueIdentityV1::new(digest.finalize().into()).ok()
}

fn parse_options(arguments: Vec<OsString>) -> Result<OptionsV4, String> {
    let mut arguments = arguments.into_iter();
    let output = match arguments.next().as_deref() {
        Some(value) if value == OsStr::new("live-rocgdb-kfd-v4") => OutputVersion::V4,
        Some(value) if value == OsStr::new("live-rocgdb-kfd-v5") => OutputVersion::V5,
        _ => return Err(USAGE.to_owned()),
    };
    let mut rocgdb = None;
    let mut authorization = None;
    let mut hsaco = None;
    let mut load_base = None;
    let mut kernel = None;
    let mut device_unique_id = None;
    let mut wave_width = 64;
    let mut timeout = Duration::from_secs(10);
    let mut protocol_seen = false;
    let mut program = None;
    let mut program_arguments = Vec::new();
    while let Some(option) = arguments.next() {
        if option == OsStr::new("--") {
            program = arguments.next().map(PathBuf::from);
            program_arguments.extend(arguments);
            break;
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("option {option:?} requires a value; {USAGE}"))?;
        match option.to_str() {
            Some("--rocgdb") => set_once(&mut rocgdb, PathBuf::from(value), "--rocgdb")?,
            Some("--authorization") => {
                let text = value
                    .to_str()
                    .ok_or_else(|| format!("invalid --authorization; {USAGE}"))?;
                let quoted = serde_json::to_string(text).map_err(|_| USAGE.to_owned())?;
                let identity = serde_json::from_str(&quoted)
                    .map_err(|_| format!("invalid --authorization; {USAGE}"))?;
                set_once(&mut authorization, identity, "--authorization")?;
            }
            Some("--hsaco") => set_once(&mut hsaco, PathBuf::from(value), "--hsaco")?,
            Some("--load-base") => set_once(&mut load_base, parse_hex(&value)?, "--load-base")?,
            Some("--kernel") => {
                if value.to_str().is_none()
                    || value.as_bytes().is_empty()
                    || value.as_bytes().len() > 4_096
                {
                    return Err(format!("invalid --kernel; {USAGE}"));
                }
                set_once(&mut kernel, value, "--kernel")?;
            }
            Some("--device-unique-id") => {
                let text = value
                    .to_str()
                    .ok_or_else(|| format!("invalid --device-unique-id; {USAGE}"))?;
                if text.is_empty()
                    || (text.len() > 1 && text.starts_with('0'))
                    || !text.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(format!("invalid --device-unique-id; {USAGE}"));
                }
                let unique_id = text
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value != 0)
                    .ok_or_else(|| format!("invalid --device-unique-id; {USAGE}"))?;
                set_once(&mut device_unique_id, unique_id, "--device-unique-id")?;
            }
            Some("--wave-width") => {
                wave_width = match value.to_str() {
                    Some("32") => 32,
                    Some("64") => 64,
                    _ => return Err(format!("invalid --wave-width; {USAGE}")),
                }
            }
            Some("--timeout-ms") => {
                let milliseconds = value
                    .to_str()
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| *value > 0 && *value <= 60_000)
                    .ok_or_else(|| format!("invalid --timeout-ms; {USAGE}"))?;
                timeout = Duration::from_millis(milliseconds);
            }
            Some("--protocol") if !protocol_seen && value == OsStr::new("jsonl") => {
                protocol_seen = true;
            }
            _ => return Err(format!("unknown or repeated option; {USAGE}")),
        }
    }
    let absolute = |path: &PathBuf| {
        path.is_absolute() && path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES_V4
    };
    let rocgdb = rocgdb.filter(absolute).ok_or_else(|| USAGE.to_owned())?;
    let hsaco = hsaco.filter(absolute).ok_or_else(|| USAGE.to_owned())?;
    let program = program.filter(absolute).ok_or_else(|| USAGE.to_owned())?;
    if program_arguments.len() > MAX_ARGUMENTS_V4
        || program_arguments
            .iter()
            .map(|value| value.as_bytes().len())
            .sum::<usize>()
            > MAX_ARGUMENT_BYTES_V4
    {
        return Err(format!("target arguments exceed bounds; {USAGE}"));
    }
    Ok(OptionsV4 {
        output,
        rocgdb,
        authorization: authorization.ok_or_else(|| USAGE.to_owned())?,
        hsaco,
        load_base: load_base.ok_or_else(|| USAGE.to_owned())?,
        kernel: kernel.ok_or_else(|| USAGE.to_owned())?,
        device_unique_id,
        wave_width,
        timeout,
        program,
        arguments: program_arguments,
    })
}

fn parse_hex(value: &OsStr) -> Result<u64, String> {
    let text = value.to_str().ok_or_else(|| USAGE.to_owned())?;
    let digits = text.strip_prefix("0x").ok_or_else(|| USAGE.to_owned())?;
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || !digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(USAGE.to_owned());
    }
    u64::from_str_radix(digits, 16).map_err(|_| USAGE.to_owned())
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err(format!("{name} may appear only once; {USAGE}"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telemetry_digest(seed: u8) -> fe2o3_kfd::KfdTargetDebugTelemetryDigestV1 {
        fe2o3_kfd::KfdTargetDebugTelemetryDigestV1::from_bytes([seed; 32]).unwrap()
    }

    #[test]
    fn native_v4_options_are_bounded_and_exact() {
        let base = [
            "live-rocgdb-kfd-v4",
            "--rocgdb",
            "/usr/bin/rocgdb",
            "--authorization",
            "0101010101010101010101010101010101010101010101010101010101010101",
            "--hsaco",
            "/tmp/kernel.hsaco",
            "--load-base",
            "0x1000",
            "--kernel",
            "kernel",
            "--",
            "/bin/true",
        ];
        assert!(parse_options(base.into_iter().map(OsString::from).collect()).is_ok());
        let mut v5 = base;
        v5[0] = "live-rocgdb-kfd-v5";
        assert_eq!(
            parse_options(v5.into_iter().map(OsString::from).collect())
                .unwrap()
                .output,
            OutputVersion::V5
        );
        for changed in ["0x01000", "1000", "0xG"] {
            let mut args = base.map(OsString::from);
            args[8] = OsString::from(changed);
            assert!(parse_options(args.into_iter().collect()).is_err());
        }
    }

    #[test]
    fn late_failure_preserves_already_observed_probe_history() {
        let response = unavailable(
            RocgdbMiNativeProbeV4 {
                structured_mi_commands: true,
                direct_kfd_device_admitted: false,
                cooperative_v2_declaration: false,
                cooperative_v2_publication: false,
            },
            RocgdbMiNativeUnavailableReasonV4::DirectKfdDeviceUnavailable,
        );
        response.validate().unwrap();
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("\"structured_mi_commands\":true"));
        assert!(encoded.contains("\"direct_kfd_device_admitted\":false"));
        assert!(encoded.contains("\"reason\":\"direct_kfd_device_unavailable\""));
    }

    #[test]
    fn v5_unavailable_keeps_registry_discovery_separate_from_observation() {
        let response = response_v5(
            unavailable(
                RocgdbMiNativeProbeV4 {
                    structured_mi_commands: true,
                    direct_kfd_device_admitted: true,
                    cooperative_v2_declaration: true,
                    cooperative_v2_publication: true,
                },
                RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
            ),
            RocgdbMiNativeInspectionProbeV5 {
                register_names: true,
                register_values: true,
                simple_locals: true,
                disassembly: true,
                memory_bytes: true,
            },
            None,
        );
        response.validate().unwrap();
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("\"register_values\":true"));
        assert!(encoded.contains("\"status\":\"unavailable\""));
        assert!(encoded.contains("\"reason\":\"gpu_stopped_state_unavailable\""));
        assert!(!encoded.contains("\"inspection\":"));
    }

    #[test]
    fn publication_is_preserved_when_rocgdb_never_reports_a_stop() {
        let mut probe = RocgdbMiNativeProbeV4 {
            structured_mi_commands: true,
            direct_kfd_device_admitted: true,
            cooperative_v2_declaration: false,
            cooperative_v2_publication: false,
        };
        let mut declaration = None;
        let mut publication = None;
        let declared = KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance: telemetry_digest(1),
            executable: fe2o3_kfd::KfdTargetDebugArtifactIdentityV1::new(telemetry_digest(2), 64)
                .unwrap(),
            artifact: fe2o3_kfd::KfdTargetDebugArtifactIdentityV1::new(telemetry_digest(3), 128)
                .unwrap(),
            dispatch: telemetry_digest(4),
            kernel: telemetry_digest(5),
            logical_queue: telemetry_digest(6),
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_memory_bytes: 0,
            generation: 7,
        };
        let published = KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            process_instance: telemetry_digest(1),
            queue_occurrence: telemetry_digest(7),
            dispatch: telemetry_digest(4),
            artifact: telemetry_digest(3),
            generation: 7,
            target_kfd_gpu_id_observation: 35_090,
            target_kfd_queue_id_observation: 9,
            target_aql_packet_id_observation: 41,
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
        };
        assert!(!observe_native_telemetry_v4(
            &declared,
            &mut probe,
            &mut declaration,
            &mut publication,
        ));
        assert!(!observe_native_telemetry_v4(
            &published,
            &mut probe,
            &mut declaration,
            &mut publication,
        ));
        assert_eq!(
            native_collection_unavailable_reason_v4(false, &declaration, &publication),
            Some(RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable)
        );
        assert!(probe.cooperative_v2_declaration);
        assert!(probe.cooperative_v2_publication);
        let response = unavailable(
            probe,
            RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
        );
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            "{\"schema\":\"fe2o3-rocgdb-kfd-native-response-v4\",\"result\":{\"status\":\"unavailable\",\"probe\":{\"structured_mi_commands\":true,\"direct_kfd_device_admitted\":true,\"cooperative_v2_declaration\":true,\"cooperative_v2_publication\":true},\"reason\":\"gpu_stopped_state_unavailable\"}}"
        );
    }
}
