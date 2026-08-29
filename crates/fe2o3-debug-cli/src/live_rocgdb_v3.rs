//! Bounded JSONL coordinator for the structured ROCgdb MI substrate.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use fe2o3_debug_protocol::*;
use sha2::{Digest, Sha256};

use crate::rocgdb_mi_v3::{RocgdbMiAdapterErrorV3, RocgdbMiAdapterLimitsV3, RocgdbMiProcessV3};

const BOOTSTRAP_REQUEST_ID_V3: u64 = u64::MAX;
const MAX_BOOTSTRAP_PATH_BYTES_V3: usize = 4_096;
const MAX_BOOTSTRAP_ARGUMENTS_V3: usize = 256;
const MAX_BOOTSTRAP_ARGUMENT_BYTES_V3: usize = 32 * 1_024;
const USAGE: &str = "fe2o3-debug live-rocgdb --rocgdb PATH --authorization ID [--protocol jsonl] [--wave-width 32|64] [--timeout-ms N] (--attach PID | -- PROGRAM [ARG...])";

#[derive(Debug)]
enum BootstrapModeV3 {
    Launch {
        program: PathBuf,
        arguments: Vec<OsString>,
    },
    Attach {
        process: u32,
    },
}

#[derive(Debug)]
struct OptionsV3 {
    rocgdb: PathBuf,
    authorization: OpaqueIdentityV1,
    wave_width: u16,
    timeout: Duration,
    mode: BootstrapModeV3,
}

pub(crate) fn run(arguments: Vec<OsString>) -> ExitCode {
    let options = match parse_options(arguments) {
        Ok(options) => options,
        Err(message) => {
            super::write_bootstrap_error("arguments", "invalid_live_rocgdb_command_line", &message);
            return ExitCode::FAILURE;
        }
    };
    let session_identity = match random_session_identity(options.authorization) {
        Ok(identity) => identity,
        Err(()) => {
            super::write_bootstrap_error(
                "session",
                "session_identity_unavailable",
                "could not create a private live ROCgdb session identity",
            );
            return ExitCode::FAILURE;
        }
    };
    let mut process = match RocgdbMiProcessV3::spawn(
        &options.rocgdb,
        session_identity,
        options.authorization,
        options.wave_width,
        RocgdbMiAdapterLimitsV3::default(),
    ) {
        Ok(process) => process,
        Err(_) => {
            super::write_bootstrap_error(
                "backend",
                "rocgdb_spawn_failed",
                "the exact ROCgdb argument could not be started in structured MI mode",
            );
            return ExitCode::FAILURE;
        }
    };
    let authorization = RocgdbMiControlAuthorizationV3 {
        authorization_identity: options.authorization,
        expected_revision: 0,
    };
    let bootstrap = match options.mode {
        BootstrapModeV3::Launch { program, arguments } => process.launch_target(
            RocgdbMiControlRequestV3::Launch {
                request_id: BOOTSTRAP_REQUEST_ID_V3,
                authorization,
            },
            &program,
            &arguments,
            options.timeout,
        ),
        BootstrapModeV3::Attach { process: target } => process.attach_target(
            RocgdbMiControlRequestV3::Attach {
                request_id: BOOTSTRAP_REQUEST_ID_V3,
                authorization,
            },
            target,
            options.timeout,
        ),
    };
    let bootstrap = match bootstrap {
        Ok(bootstrap)
            if matches!(
                bootstrap.outcome,
                RocgdbMiControlOutcomeV3::Applied {
                    effect: RocgdbMiControlEffectV3::Committed,
                    ..
                }
            ) =>
        {
            bootstrap
        }
        Ok(_) | Err(_) => {
            super::write_bootstrap_error(
                "backend",
                "rocgdb_bootstrap_failed",
                "ROCgdb did not commit the authorized launch or attach",
            );
            return ExitCode::FAILURE;
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let result = run_jsonl(
        &mut process,
        options.authorization,
        bootstrap,
        options.timeout,
        &mut reader,
        &mut writer,
    );
    if result.is_err() {
        super::write_bootstrap_error(
            "protocol",
            "live_rocgdb_stream_failed",
            "the bounded ROCgdb JSONL stream could not be completed",
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_jsonl<R: io::BufRead, W: Write>(
    process: &mut RocgdbMiProcessV3,
    authorization_identity: OpaqueIdentityV1,
    bootstrap: RocgdbMiControlResultV3,
    timeout: Duration,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), ()> {
    let mut request_ids = BTreeSet::new();
    let mut requests = 0_u64;
    loop {
        let request = match read_rocgdb_mi_cli_request_line_v3(reader) {
            Ok(Some(request)) => request,
            Ok(None) => {
                let _ = process.shutdown(timeout);
                return Ok(());
            }
            Err(_) => {
                let response = error_response(
                    process.adapter().revision(),
                    None,
                    RocgdbMiCliErrorCodeV3::InvalidRequest,
                    RocgdbMiControlEffectV3::None,
                    true,
                );
                write_response(writer, &response)?;
                let _ = process.shutdown(timeout);
                return Ok(());
            }
        };
        let request_id = request.request_id();
        requests = requests.saturating_add(1);
        if requests > MAX_ROCGDB_MI_CLI_REQUESTS_V3 {
            let response = error_response(
                process.adapter().revision(),
                Some(request_id),
                RocgdbMiCliErrorCodeV3::CommandBudgetExhausted,
                RocgdbMiControlEffectV3::None,
                true,
            );
            write_response(writer, &response)?;
            let _ = process.shutdown(timeout);
            return Ok(());
        }
        if !request_ids.insert(request_id) {
            write_response(
                writer,
                &error_response(
                    process.adapter().revision(),
                    Some(request_id),
                    RocgdbMiCliErrorCodeV3::DuplicateRequestId,
                    RocgdbMiControlEffectV3::None,
                    false,
                ),
            )?;
            continue;
        }
        let response = handle_request(process, authorization_identity, bootstrap, timeout, request);
        let terminate = matches!(
            &response,
            RocgdbMiCliResponseV3::Ok {
                result,
                ..
            } if matches!(result.as_ref(), RocgdbMiCliResultV3::Terminated { .. })
        ) || matches!(
            &response,
            RocgdbMiCliResponseV3::Error { terminal: true, .. }
        );
        write_response(writer, &response)?;
        if terminate {
            return Ok(());
        }
    }
}

fn handle_request(
    process: &mut RocgdbMiProcessV3,
    authorization_identity: OpaqueIdentityV1,
    bootstrap: RocgdbMiControlResultV3,
    timeout: Duration,
    request: RocgdbMiCliRequestV3,
) -> RocgdbMiCliResponseV3 {
    let request_id = request.request_id();
    let result = match request {
        RocgdbMiCliRequestV3::GetSession { .. } => Ok(RocgdbMiCliResultV3::Session {
            session_identity: process.adapter().session_identity(),
            bootstrap,
        }),
        RocgdbMiCliRequestV3::DiscoverCapabilities { .. } => process
            .discover_capabilities(timeout)
            .map(|mi| RocgdbMiCliResultV3::Capabilities {
                capabilities: RocgdbMiCliCapabilitiesV3 {
                    mi,
                    generic_stopped_scopes: unsupported_generic_scopes(),
                },
            }),
        RocgdbMiCliRequestV3::NextEvent {
            wait_milliseconds, ..
        } => process
            .next_event(Duration::from_millis(wait_milliseconds))
            .map(|event| RocgdbMiCliResultV3::Event { event }),
        RocgdbMiCliRequestV3::AdmitGpuThreads {
            thread_ordinals, ..
        } => process
            .admit_gpu_threads(&thread_ordinals, timeout)
            .map(|admissions| RocgdbMiCliResultV3::GpuThreadsAdmitted { admissions }),
        RocgdbMiCliRequestV3::AdmitCodeObject {
            content,
            load_base,
            byte_len,
            kernel_entry,
            ..
        } => process
            .adapter_mut()
            .bind_code_object(
                content,
                load_base.parse().expect("validated native input"),
                byte_len,
                kernel_entry.parse().expect("validated native input"),
            )
            .map(|()| RocgdbMiCliResultV3::BindingAdmitted {
                admission: RocgdbMiCliBindingAdmissionV3::CodeObject { content },
            }),
        RocgdbMiCliRequestV3::AdmitAllocation {
            allocation,
            base,
            byte_len,
            space,
            ..
        } => process
            .adapter_mut()
            .bind_allocation(
                allocation,
                base.parse().expect("validated native input"),
                byte_len,
                space,
            )
            .map(|()| RocgdbMiCliResultV3::BindingAdmitted {
                admission: RocgdbMiCliBindingAdmissionV3::Allocation { allocation },
            }),
        RocgdbMiCliRequestV3::AdmitSourceLine {
            source, path, line, ..
        } => process
            .adapter_mut()
            .bind_source_line(OsStr::new(&path), line, source)
            .map(|()| RocgdbMiCliResultV3::BindingAdmitted {
                admission: RocgdbMiCliBindingAdmissionV3::SourceLine { source },
            }),
        RocgdbMiCliRequestV3::InspectRegisters { scope, .. } => process
            .inspect_registers(scope, timeout)
            .map(|snapshot| RocgdbMiCliResultV3::Registers { snapshot }),
        RocgdbMiCliRequestV3::InspectValues { scope, .. } => process
            .inspect_values(scope, timeout)
            .map(|snapshot| RocgdbMiCliResultV3::Values { snapshot }),
        RocgdbMiCliRequestV3::EvaluateExpression {
            scope,
            value_identity,
            name,
            expression,
            ..
        } => process
            .evaluate_expression(scope, value_identity, &name, &expression, timeout)
            .map(|value| RocgdbMiCliResultV3::EvaluatedValue { scope, value }),
        RocgdbMiCliRequestV3::ReadMemory { request, .. } => process
            .read_memory(request, timeout)
            .map(|memory| RocgdbMiCliResultV3::Memory { memory }),
        RocgdbMiCliRequestV3::Control { control, .. } => process
            .control(control, timeout)
            .map(|control| RocgdbMiCliResultV3::Control { control }),
        RocgdbMiCliRequestV3::Terminate { authorization, .. } => {
            if authorization.authorization_identity != authorization_identity {
                return error_response(
                    process.adapter().revision(),
                    Some(request_id),
                    RocgdbMiCliErrorCodeV3::AuthorizationMismatch,
                    RocgdbMiControlEffectV3::None,
                    false,
                );
            }
            if authorization.expected_revision != process.adapter().revision() {
                return error_response(
                    process.adapter().revision(),
                    Some(request_id),
                    RocgdbMiCliErrorCodeV3::StaleRevision,
                    RocgdbMiControlEffectV3::None,
                    false,
                );
            }
            match process.shutdown(timeout) {
                Ok(revision) => Ok(RocgdbMiCliResultV3::Terminated {
                    revision,
                    effect: RocgdbMiControlEffectV3::Committed,
                }),
                Err(error) => {
                    return error_response(
                        process.adapter().revision(),
                        Some(request_id),
                        error_code(error),
                        RocgdbMiControlEffectV3::Indeterminate,
                        true,
                    );
                }
            }
        }
    };
    match result {
        Ok(result) => RocgdbMiCliResponseV3::Ok {
            schema: RocgdbMiCliResponseSchemaV3::V3,
            request_id,
            revision: process.adapter().revision(),
            result: Box::new(result),
        },
        Err(error) => {
            let code = error_code(error);
            let terminal = matches!(
                code,
                RocgdbMiCliErrorCodeV3::BackendDisconnected
                    | RocgdbMiCliErrorCodeV3::CommandBudgetExhausted
                    | RocgdbMiCliErrorCodeV3::Timeout
            );
            error_response(
                process.adapter().revision(),
                Some(request_id),
                code,
                RocgdbMiControlEffectV3::None,
                terminal,
            )
        }
    }
}

fn unsupported_generic_scopes() -> Vec<LiveGpuCapabilityV3> {
    [
        LiveGpuCapabilityNameV3::StoppedDispatch,
        LiveGpuCapabilityNameV3::StoppedWorkgroups,
        LiveGpuCapabilityNameV3::StoppedWaves,
        LiveGpuCapabilityNameV3::StoppedLanes,
    ]
    .into_iter()
    .map(|name| LiveGpuCapabilityV3 {
        backend: LiveGpuBackendV3::RocgdbMi,
        name,
        availability: LiveGpuCapabilityAvailabilityV3::Unavailable,
        unavailable_reason: Some(LiveGpuUnavailableReasonV3::Unsupported),
    })
    .collect()
}

fn error_response(
    revision: u64,
    request_id: Option<u64>,
    code: RocgdbMiCliErrorCodeV3,
    effect: RocgdbMiControlEffectV3,
    terminal: bool,
) -> RocgdbMiCliResponseV3 {
    RocgdbMiCliResponseV3::Error {
        schema: RocgdbMiCliResponseSchemaV3::V3,
        request_id,
        revision,
        code,
        effect,
        terminal,
    }
}

fn error_code(error: RocgdbMiAdapterErrorV3) -> RocgdbMiCliErrorCodeV3 {
    match error {
        RocgdbMiAdapterErrorV3::StaleRevision => RocgdbMiCliErrorCodeV3::StaleRevision,
        RocgdbMiAdapterErrorV3::SessionNotStopped => RocgdbMiCliErrorCodeV3::SessionNotStopped,
        RocgdbMiAdapterErrorV3::UnknownThread
        | RocgdbMiAdapterErrorV3::UnknownAllocation
        | RocgdbMiAdapterErrorV3::UnknownCodeObject
        | RocgdbMiAdapterErrorV3::UnknownSource => RocgdbMiCliErrorCodeV3::UnknownLogicalIdentity,
        RocgdbMiAdapterErrorV3::DuplicateBinding
        | RocgdbMiAdapterErrorV3::InvalidField(_)
        | RocgdbMiAdapterErrorV3::InvalidWaveWidth
        | RocgdbMiAdapterErrorV3::IdentityCollision => RocgdbMiCliErrorCodeV3::InvalidBinding,
        RocgdbMiAdapterErrorV3::Timeout => RocgdbMiCliErrorCodeV3::Timeout,
        RocgdbMiAdapterErrorV3::ProcessIo
        | RocgdbMiAdapterErrorV3::ProcessExited
        | RocgdbMiAdapterErrorV3::ProcessSpawn => RocgdbMiCliErrorCodeV3::BackendDisconnected,
        RocgdbMiAdapterErrorV3::ResponseBudgetExhausted
        | RocgdbMiAdapterErrorV3::EventBudgetExhausted
        | RocgdbMiAdapterErrorV3::CountOutOfRange(_)
        | RocgdbMiAdapterErrorV3::LimitOutOfRange(_) => {
            RocgdbMiCliErrorCodeV3::CommandBudgetExhausted
        }
        RocgdbMiAdapterErrorV3::BackendRejected
        | RocgdbMiAdapterErrorV3::ProtocolRecordRejected
        | RocgdbMiAdapterErrorV3::InvalidMiRecord
        | RocgdbMiAdapterErrorV3::UnexpectedMiRecord
        | RocgdbMiAdapterErrorV3::MissingField(_)
        | RocgdbMiAdapterErrorV3::InvalidTimeout
        | RocgdbMiAdapterErrorV3::InvalidCommand => RocgdbMiCliErrorCodeV3::BackendRejected,
    }
}

fn write_response(writer: &mut impl Write, response: &RocgdbMiCliResponseV3) -> Result<(), ()> {
    let line = encode_rocgdb_mi_cli_response_line_v3(response).map_err(|_| ())?;
    writer.write_all(&line).map_err(|_| ())?;
    writer.flush().map_err(|_| ())
}

fn parse_options(arguments: Vec<OsString>) -> Result<OptionsV3, String> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some(OsStr::new("live-rocgdb")) {
        return Err(USAGE.to_owned());
    }
    let mut rocgdb = None;
    let mut authorization = None;
    let mut attach = None;
    let mut launch = None;
    let mut protocol_seen = false;
    let mut wave_width_seen = false;
    let mut timeout_seen = false;
    let mut wave_width = 64_u16;
    let mut timeout = Duration::from_secs(5);
    while let Some(option) = arguments.next() {
        if option == OsStr::new("--") {
            let program = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("-- requires PROGRAM; {USAGE}"))?;
            if !program.is_absolute()
                || program.as_os_str().as_bytes().len() > MAX_BOOTSTRAP_PATH_BYTES_V3
            {
                return Err(format!("PROGRAM must be a bounded absolute path; {USAGE}"));
            }
            let launch_arguments: Vec<_> = arguments.collect();
            let argument_bytes = launch_arguments
                .iter()
                .try_fold(0_usize, |total, argument| {
                    total.checked_add(argument.as_bytes().len())
                });
            if launch_arguments.len() > MAX_BOOTSTRAP_ARGUMENTS_V3
                || argument_bytes.is_none_or(|bytes| bytes > MAX_BOOTSTRAP_ARGUMENT_BYTES_V3)
            {
                return Err(format!(
                    "PROGRAM arguments exceed the startup bound; {USAGE}"
                ));
            }
            launch = Some(BootstrapModeV3::Launch {
                program,
                arguments: launch_arguments,
            });
            break;
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("an option requires a value; {USAGE}"))?;
        if option == OsStr::new("--rocgdb") {
            set_once(&mut rocgdb, PathBuf::from(value), "--rocgdb")?;
        } else if option == OsStr::new("--authorization") {
            let text = value
                .to_str()
                .ok_or_else(|| format!("--authorization must be UTF-8; {USAGE}"))?;
            let quoted = serde_json::to_string(text)
                .map_err(|_| format!("invalid --authorization; {USAGE}"))?;
            let identity = serde_json::from_str(&quoted)
                .map_err(|_| format!("--authorization must be 64 lowercase hex digits; {USAGE}"))?;
            set_once(&mut authorization, identity, "--authorization")?;
        } else if option == OsStr::new("--attach") {
            let text = value
                .to_str()
                .ok_or_else(|| format!("--attach must be decimal; {USAGE}"))?;
            if text.is_empty()
                || (text.len() > 1 && text.starts_with('0'))
                || !text.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(format!("--attach must be canonical decimal; {USAGE}"));
            }
            let process = text
                .parse::<u32>()
                .ok()
                .filter(|process| *process > 0 && i32::try_from(*process).is_ok())
                .ok_or_else(|| format!("--attach is out of range; {USAGE}"))?;
            set_once(&mut attach, process, "--attach")?;
        } else if option == OsStr::new("--protocol") {
            if protocol_seen || value != OsStr::new("jsonl") {
                return Err(format!(
                    "--protocol must appear at most once and equal jsonl; {USAGE}"
                ));
            }
            protocol_seen = true;
        } else if option == OsStr::new("--wave-width") {
            if wave_width_seen {
                return Err(format!("--wave-width may appear only once; {USAGE}"));
            }
            wave_width_seen = true;
            wave_width = match value.to_str() {
                Some("32") => 32,
                Some("64") => 64,
                _ => return Err(format!("--wave-width must be 32 or 64; {USAGE}")),
            };
        } else if option == OsStr::new("--timeout-ms") {
            if timeout_seen {
                return Err(format!("--timeout-ms may appear only once; {USAGE}"));
            }
            timeout_seen = true;
            let milliseconds = value
                .to_str()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0 && *value <= MAX_ROCGDB_MI_CLI_WAIT_MILLISECONDS_V3)
                .ok_or_else(|| format!("--timeout-ms must be 1..=60000; {USAGE}"))?;
            timeout = Duration::from_millis(milliseconds);
        } else {
            return Err(format!("unknown option; {USAGE}"));
        }
    }
    let mode = match (attach, launch) {
        (Some(process), None) => BootstrapModeV3::Attach { process },
        (None, Some(launch)) => launch,
        _ => {
            return Err(format!(
                "exactly one of --attach or -- PROGRAM is required; {USAGE}"
            ));
        }
    };
    let rocgdb = rocgdb.ok_or_else(|| format!("--rocgdb is required; {USAGE}"))?;
    if !rocgdb.is_absolute() || rocgdb.as_os_str().as_bytes().len() > MAX_BOOTSTRAP_PATH_BYTES_V3 {
        return Err(format!("--rocgdb must be a bounded absolute path; {USAGE}"));
    }
    Ok(OptionsV3 {
        rocgdb,
        authorization: authorization
            .ok_or_else(|| format!("--authorization is required; {USAGE}"))?,
        wave_width,
        timeout,
        mode,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err(format!("{name} may appear only once; {USAGE}"))
    } else {
        Ok(())
    }
}

fn random_session_identity(authorization: OpaqueIdentityV1) -> Result<OpaqueIdentityV1, ()> {
    let mut random = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|_| ())?;
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-live-rocgdb-session-v3\0");
    digest.update(random);
    digest.update(authorization.as_bytes());
    OpaqueIdentityV1::new(digest.finalize().into()).map_err(|_| ())
}
