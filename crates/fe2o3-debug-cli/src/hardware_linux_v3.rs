//! Linux coordinator for an exact-artifact, cooperative direct-KFD session.

use std::ffi::OsString;
use std::io::{self, BufReader, BufWriter, Write};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::{
    LiveGpuDebugRequestV3, LiveGpuOperationV3, encode_live_gpu_response_line_v3,
    read_live_gpu_request_line_v3,
};
use fe2o3_kfd::{
    KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1, KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1,
    KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1, KfdDebugSessionPlanV1, KfdDebuggerTelemetryEndpointV1,
    KfdLiveDebugSessionV1, KfdTargetDebugSessionNonceV1, KfdTargetDebugTelemetryPayloadV1,
    KfdTargetDebugTelemetryProcessV1, create_kfd_target_debug_telemetry_channel_v1,
};
use fe2o3_kfd_uapi::{KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1, KfdDebugExceptionMaskV1};
use serde::Serialize;

use crate::hardware_linux_v2::{
    CLEANUP_TIMEOUT, STARTUP_TIMEOUT, continue_tracee, detach_then_kill, detach_tracee,
    kill_and_reap, kill_and_reap_result, run_cleanup_actions, set_exit_kill, stop_tracee,
    wait_for_tracee_stop,
};
use crate::hardware_v2::LiveKfdTransportV2;
use crate::live_gpu_backend_v3::LiveKfdBackendV3;
use crate::live_kfd_v3::{LiveKfdContentIdentityV3, LiveKfdSemanticSessionBindingV3};

const READER_POLL: Duration = Duration::from_millis(5);

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct LiveKfdBootstrapErrorV3<'a> {
    schema: &'static str,
    status: &'static str,
    stage: &'a str,
    code: &'a str,
    message: &'a str,
}

enum ReaderMessageV3 {
    Request(LiveGpuDebugRequestV3),
    Eof,
    Error,
}

pub(crate) fn run(
    mut admitted: LiveKfdSemanticSessionBindingV3,
    target_arguments: Vec<OsString>,
    protocol_binding: fe2o3_debug_protocol::LiveGpuArtifactBindingV3,
) -> ExitCode {
    let expected_host = admitted.observed_host().content();
    let debugger_process = match KfdTargetDebugTelemetryProcessV1::capture(std::process::id()) {
        Ok(process) => process,
        Err(_) => {
            bootstrap_error(
                "launch",
                "debugger_process_binding_failed",
                "could not bind the debugger process instance",
            );
            return ExitCode::FAILURE;
        }
    };
    let nonce = match generate_nonce() {
        Ok(nonce) => nonce,
        Err(()) => {
            bootstrap_error(
                "launch",
                "session_nonce_failed",
                "could not generate a live KFD session nonce",
            );
            return ExitCode::FAILURE;
        }
    };
    let (debugger_endpoint, target_endpoint) = match create_kfd_target_debug_telemetry_channel_v1()
    {
        Ok(channel) => channel,
        Err(_) => {
            bootstrap_error(
                "launch",
                "telemetry_channel_failed",
                "could not create the bounded target telemetry channel",
            );
            return ExitCode::FAILURE;
        }
    };
    let mut child = match launch_tracee(
        admitted.host_executable_fd(),
        &target_arguments,
        &target_endpoint,
        nonce,
        debugger_process.pid(),
    ) {
        Ok(child) => child,
        Err(()) => {
            bootstrap_error(
                "launch",
                "target_launch_failed",
                "could not launch the exact admitted target image",
            );
            return ExitCode::FAILURE;
        }
    };
    drop(target_endpoint);
    if wait_for_tracee_stop(&mut child, Instant::now() + STARTUP_TIMEOUT).is_err() {
        bootstrap_error(
            "launch",
            "target_exec_stop_failed",
            "the target did not reach its owned exec stop",
        );
        kill_and_reap(&mut child);
        return ExitCode::FAILURE;
    }
    if admitted.record_host_exec_sigtrap_v3().is_err() {
        bootstrap_error(
            "binding",
            "host_exec_binding_failed",
            "the launched host descriptor changed across exec",
        );
        detach_then_kill(&mut child);
        return ExitCode::FAILURE;
    }
    if set_exit_kill(&child).is_err() {
        bootstrap_error(
            "launch",
            "ptrace_exit_kill_failed",
            "could not establish target exit-on-debugger-loss",
        );
        detach_then_kill(&mut child);
        return ExitCode::FAILURE;
    }
    let target_process = match KfdTargetDebugTelemetryProcessV1::capture(child.id()) {
        Ok(process) => process,
        Err(_) => {
            bootstrap_error(
                "launch",
                "target_process_binding_failed",
                "could not bind the launched target process instance",
            );
            detach_then_kill(&mut child);
            return ExitCode::FAILURE;
        }
    };
    let telemetry =
        match KfdDebuggerTelemetryEndpointV1::admit(debugger_endpoint, nonce, target_process) {
            Ok(endpoint) => endpoint,
            Err(_) => {
                bootstrap_error(
                    "session",
                    "telemetry_admission_failed",
                    "target telemetry endpoint admission failed",
                );
                detach_then_kill(&mut child);
                return ExitCode::FAILURE;
            }
        };
    let plan = match KfdDebugSessionPlanV1::new(
        child.id(),
        std::process::id(),
        KfdDebugExceptionMaskV1::ALL,
        KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1,
    ) {
        Ok(plan) => plan,
        Err(_) => {
            bootstrap_error(
                "session",
                "debug_plan_rejected",
                "the direct KFD debug plan was rejected",
            );
            detach_then_kill(&mut child);
            return ExitCode::FAILURE;
        }
    };
    let session = match KfdLiveDebugSessionV1::attach(plan) {
        Ok(session) => session,
        Err(_) => {
            bootstrap_error(
                "session",
                "kfd_debug_attach_failed",
                "direct KFD debug-session admission failed",
            );
            detach_then_kill(&mut child);
            return ExitCode::FAILURE;
        }
    };
    if continue_tracee(&child).is_err() {
        bootstrap_error(
            "launch",
            "target_continue_failed",
            "could not continue the admitted target",
        );
        let _ = session.finish();
        detach_then_kill(&mut child);
        return ExitCode::FAILURE;
    }

    let mut backend = LiveKfdBackendV3::new(LiveKfdTransportV2::new(session), protocol_binding);
    let receiver = spawn_reader(backend.limits());
    let run_result = run_requests(&receiver, &mut backend, telemetry, expected_host);
    let cleanup_result = cleanup_backend_and_child(backend, &mut child);
    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => ExitCode::SUCCESS,
        (Err(()), cleanup) => {
            bootstrap_error(
                "stream",
                "live_kfd_protocol_failed",
                "the bounded live KFD protocol session failed",
            );
            if cleanup.is_err() {
                bootstrap_error(
                    "cleanup",
                    "target_cleanup_failed",
                    "live KFD target cleanup was incomplete",
                );
            }
            ExitCode::FAILURE
        }
        (Ok(()), Err(())) => {
            bootstrap_error(
                "cleanup",
                "target_cleanup_failed",
                "live KFD target cleanup was incomplete",
            );
            ExitCode::FAILURE
        }
    }
}

fn launch_tracee(
    executable: BorrowedFd<'_>,
    arguments: &[OsString],
    target_endpoint: &OwnedFd,
    nonce: KfdTargetDebugSessionNonceV1,
    debugger_pid: u32,
) -> Result<Child, ()> {
    let executable_path = format!("/proc/self/fd/{}", executable.as_raw_fd());
    let inherited_telemetry_fd = target_endpoint.as_raw_fd();
    let mut command = Command::new(executable_path);
    command
        .args(arguments)
        .env(
            KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1,
            inherited_telemetry_fd.to_string(),
        )
        .env(
            KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1,
            lower_hex(nonce.as_bytes()),
        )
        .env(
            KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1,
            debugger_pid.to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // SAFETY: the hook performs only pointer-free async-signal-safe fcntl and
    // ptrace syscalls. Clearing CLOEXEC deliberately provisions this one
    // telemetry socket; every other retained descriptor keeps its policy.
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(inherited_telemetry_fd, libc::F_GETFD);
            if flags < 0
                || libc::fcntl(
                    inherited_telemetry_fd,
                    libc::F_SETFD,
                    flags & !libc::FD_CLOEXEC,
                ) < 0
            {
                return Err(io::Error::last_os_error());
            }
            let result = libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            );
            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }
    command.spawn().map_err(|_| ())
}

fn spawn_reader(
    limits: fe2o3_debug_protocol::LiveGpuProtocolLimitsV3,
) -> Receiver<ReaderMessageV3> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        loop {
            let message = match read_live_gpu_request_line_v3(&mut reader, limits) {
                Ok(Some(request)) => ReaderMessageV3::Request(request),
                Ok(None) => ReaderMessageV3::Eof,
                Err(_error) => ReaderMessageV3::Error,
            };
            let terminal = !matches!(message, ReaderMessageV3::Request(_));
            if sender.send(message).is_err() || terminal {
                break;
            }
        }
    });
    receiver
}

fn run_requests(
    receiver: &Receiver<ReaderMessageV3>,
    backend: &mut LiveKfdBackendV3<LiveKfdTransportV2>,
    mut telemetry: KfdDebuggerTelemetryEndpointV1,
    expected_host: LiveKfdContentIdentityV3,
) -> Result<(), ()> {
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    loop {
        let request = match receiver.recv_timeout(READER_POLL) {
            Ok(ReaderMessageV3::Request(request)) => request,
            Ok(ReaderMessageV3::Eof) => return Ok(()),
            Ok(ReaderMessageV3::Error) => return Err(()),
            Err(RecvTimeoutError::Timeout) => {
                pump_observations(backend, &mut telemetry, expected_host)?;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return Err(()),
        };
        pump_observations(backend, &mut telemetry, expected_host)?;
        let terminate = request.operation() == LiveGpuOperationV3::Terminate;
        let response = backend.handle(request);
        let line = encode_live_gpu_response_line_v3(&response, backend.limits()).map_err(|_| ())?;
        writer
            .write_all(&line)
            .and_then(|()| writer.flush())
            .map_err(|_| ())?;
        if terminate {
            return Ok(());
        }
    }
}

fn pump_observations(
    backend: &mut LiveKfdBackendV3<LiveKfdTransportV2>,
    telemetry: &mut KfdDebuggerTelemetryEndpointV1,
    expected_host: LiveKfdContentIdentityV3,
) -> Result<(), ()> {
    backend.pump_async_observations().map_err(|_| ())?;
    if telemetry.is_finished() {
        return Ok(());
    }
    for _ in 0..64 {
        let Some(record) = telemetry.try_receive().map_err(|_| ())? else {
            return Ok(());
        };
        if record.sequence() == 0 {
            let KfdTargetDebugTelemetryPayloadV1::SessionStarted { executable, .. } =
                record.payload()
            else {
                return Err(());
            };
            if executable.digest().as_bytes() != &expected_host.sha256()
                || executable.byte_length() != expected_host.length()
            {
                return Err(());
            }
        }
        backend.apply_target_telemetry(&record).map_err(|_| ())?;
        if telemetry.is_finished() {
            return Ok(());
        }
    }
    Ok(())
}

fn cleanup_backend_and_child(
    backend: LiveKfdBackendV3<LiveKfdTransportV2>,
    child: &mut Child,
) -> Result<(), ()> {
    let (stopped, stop_failed) = match stop_tracee(child, Instant::now() + CLEANUP_TIMEOUT) {
        Ok(stopped) => (stopped, false),
        Err(_) => (false, true),
    };
    let child = std::cell::RefCell::new(child);
    let cleanup_failed = run_cleanup_actions(
        || {
            backend
                .into_transport()
                .finish()
                .map_err(|_| "KFD finish".to_owned())
        },
        || {
            if stopped {
                detach_tracee(&child.borrow())
            } else {
                Ok(())
            }
        },
        || kill_and_reap_result(&mut child.borrow_mut()),
    )
    .is_err();
    if stop_failed || cleanup_failed {
        Err(())
    } else {
        Ok(())
    }
}

fn generate_nonce() -> Result<KfdTargetDebugSessionNonceV1, ()> {
    let mut bytes = [0_u8; 32];
    let mut filled = 0;
    while filled < bytes.len() {
        // SAFETY: the remaining initialized byte slice is writable for its
        // exact length. getrandom has no descriptor or pathname authority.
        let result = unsafe {
            libc::getrandom(
                bytes[filled..].as_mut_ptr().cast::<libc::c_void>(),
                bytes.len() - filled,
                0,
            )
        };
        if result > 0 {
            filled += usize::try_from(result).map_err(|_| ())?;
        } else if result < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        } else {
            return Err(());
        }
    }
    KfdTargetDebugSessionNonceV1::from_bytes(bytes).map_err(|_| ())
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn bootstrap_error(stage: &str, code: &str, message: &str) {
    let error = LiveKfdBootstrapErrorV3 {
        schema: "fe2o3-live-kfd-bootstrap-error-v3",
        status: "error",
        stage,
        code,
        message,
    };
    let _ = serde_json::to_writer(io::stderr().lock(), &error);
    let _ = writeln!(io::stderr().lock());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_hex_is_fixed_lowercase_and_pid_free() {
        let text = lower_hex(&[0xab; 32]);
        assert_eq!(text, "ab".repeat(32));
        assert!(!text.contains(&std::process::id().to_string()));
    }
}
