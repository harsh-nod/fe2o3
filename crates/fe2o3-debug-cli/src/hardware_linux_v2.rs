//! Linux process coordinator for the hardware V2 debugger.
//!
//! The main task owns ptrace, the pidfd-backed KFD session, and every ioctl.
//! The bounded reader task may only decode and hand off one inert request.

use std::ffi::{OsStr, OsString};
use std::io::{self, BufReader, BufWriter, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::{
    HardwareDebugOperationV2, HardwareProtocolCodecErrorV2, encode_hardware_response_line_v2,
    read_hardware_request_line_v2,
};
use fe2o3_kfd::{KfdDebugSessionPlanV1, KfdLiveDebugSessionV1};
use fe2o3_kfd_uapi::{KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1, KfdDebugExceptionMaskV1};
use serde::Serialize;

use crate::hardware_v2::{HardwareBackendV2, LiveKfdTransportV2};

pub(crate) const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct HardwareBootstrapErrorV2<'a> {
    schema: &'static str,
    status: &'static str,
    stage: &'a str,
    code: &'a str,
    message: &'a str,
}

enum ReaderMessageV2 {
    Request(fe2o3_debug_protocol::HardwareDebugRequestV2),
    Eof,
    Error(HardwareProtocolCodecErrorV2),
}

pub(crate) fn run(arguments: impl Iterator<Item = OsString>) -> ExitCode {
    let argv: Vec<_> = arguments.collect();
    if argv
        .first()
        .is_none_or(|argument| argument != OsStr::new("--"))
        || argv.len() < 2
    {
        bootstrap_error(
            "arguments",
            "invalid_command_line",
            "hardware launch requires: fe2o3-debug hardware -- PROGRAM [ARG...]",
        );
        return ExitCode::FAILURE;
    }

    let mut child = match launch_tracee(&argv[1], &argv[2..]) {
        Ok(child) => child,
        Err(message) => {
            bootstrap_error("launch", "target_launch_failed", &message);
            return ExitCode::FAILURE;
        }
    };
    if let Err(message) = wait_for_tracee_stop(&mut child, Instant::now() + STARTUP_TIMEOUT) {
        bootstrap_error("launch", "target_exec_stop_failed", &message);
        kill_and_reap(&mut child);
        return ExitCode::FAILURE;
    }
    if let Err(message) = set_exit_kill(&child) {
        bootstrap_error("launch", "ptrace_exit_kill_failed", &message);
        detach_then_kill(&mut child);
        return ExitCode::FAILURE;
    }

    let plan = match KfdDebugSessionPlanV1::new(
        child.id(),
        std::process::id(),
        KfdDebugExceptionMaskV1::ALL,
        KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            bootstrap_error("session", "debug_plan_rejected", &error.to_string());
            detach_then_kill(&mut child);
            return ExitCode::FAILURE;
        }
    };
    let session = match KfdLiveDebugSessionV1::attach(plan) {
        Ok(session) => session,
        Err(_error) => {
            bootstrap_error(
                "session",
                "kfd_debug_attach_failed",
                "KFD debug session admission failed",
            );
            detach_then_kill(&mut child);
            return ExitCode::FAILURE;
        }
    };
    if let Err(message) = continue_tracee(&child) {
        bootstrap_error("launch", "target_continue_failed", &message);
        let _ = session.finish();
        detach_then_kill(&mut child);
        return ExitCode::FAILURE;
    }

    let mut backend = HardwareBackendV2::new(LiveKfdTransportV2::new(session));
    let receiver = spawn_reader(backend.limits());
    let run_result = run_requests(&receiver, &mut backend);
    let cleanup_result = cleanup_backend_and_child(backend, &mut child);
    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => ExitCode::SUCCESS,
        (Err(message), cleanup) => {
            bootstrap_error("stream", "hardware_protocol_stream_failed", &message);
            if let Err(cleanup_message) = cleanup {
                bootstrap_error("cleanup", "target_cleanup_failed", &cleanup_message);
            }
            ExitCode::FAILURE
        }
        (Ok(()), Err(message)) => {
            bootstrap_error("cleanup", "target_cleanup_failed", &message);
            ExitCode::FAILURE
        }
    }
}

fn launch_tracee(program: &OsStr, arguments: &[OsString]) -> Result<Child, String> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // SAFETY: the child-side hook makes one pointer-free, async-signal-safe
    // ptrace syscall. A successful exec produces the owned SIGTRAP stop.
    unsafe {
        command.pre_exec(|| {
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
    command
        .spawn()
        .map_err(|error| format!("could not launch exact argv target: {error}"))
}

fn spawn_reader(
    limits: fe2o3_debug_protocol::HardwareProtocolLimitsV2,
) -> Receiver<ReaderMessageV2> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        loop {
            let message = match read_hardware_request_line_v2(&mut reader, limits) {
                Ok(Some(request)) => ReaderMessageV2::Request(request),
                Ok(None) => ReaderMessageV2::Eof,
                Err(error) => ReaderMessageV2::Error(error),
            };
            let terminal = !matches!(message, ReaderMessageV2::Request(_));
            if sender.send(message).is_err() || terminal {
                break;
            }
        }
    });
    receiver
}

fn run_requests(
    receiver: &Receiver<ReaderMessageV2>,
    backend: &mut HardwareBackendV2<LiveKfdTransportV2>,
) -> Result<(), String> {
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    loop {
        let request = match receiver.recv_timeout(Duration::from_millis(5)) {
            Ok(ReaderMessageV2::Request(request)) => request,
            Ok(ReaderMessageV2::Eof) => return Ok(()),
            Ok(ReaderMessageV2::Error(error)) => return Err(error.to_string()),
            Err(RecvTimeoutError::Timeout) => {
                backend
                    .pump_async_observations()
                    .map_err(|error| error.operation.to_owned())?;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("bounded stdin reader terminated unexpectedly".to_owned());
            }
        };
        let terminate = request.operation() == HardwareDebugOperationV2::Terminate;
        let response = backend.handle(request);
        let line = encode_hardware_response_line_v2(&response, backend.limits())
            .map_err(|error| error.to_string())?;
        writer
            .write_all(&line)
            .and_then(|()| writer.flush())
            .map_err(|error| format!("hardware response write failed: {error}"))?;
        if terminate {
            return Ok(());
        }
    }
}

fn cleanup_backend_and_child(
    backend: HardwareBackendV2<LiveKfdTransportV2>,
    child: &mut Child,
) -> Result<(), String> {
    let mut errors = Vec::new();
    let stopped = match stop_tracee(child, Instant::now() + CLEANUP_TIMEOUT) {
        Ok(stopped) => stopped,
        Err(error) => {
            errors.push(error);
            false
        }
    };
    let child = std::cell::RefCell::new(child);
    if let Err(error) = run_cleanup_actions(
        || {
            backend
                .into_transport()
                .finish()
                .map_err(|error| format!("KFD debug finish failed: {error}"))
        },
        || {
            if stopped {
                detach_tracee(&child.borrow())
            } else {
                Ok(())
            }
        },
        || kill_and_reap_result(&mut child.borrow_mut()),
    ) {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) fn run_cleanup_actions(
    finish_kfd: impl FnOnce() -> Result<(), String>,
    detach_ptrace: impl FnOnce() -> Result<(), String>,
    kill_and_reap_target: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    // This is the production cleanup contract. Later actions remain mandatory
    // after an earlier diagnostic failure.
    finish_kfd().unwrap_or_else(|error| errors.push(error));
    detach_ptrace().unwrap_or_else(|error| errors.push(error));
    kill_and_reap_target().unwrap_or_else(|error| errors.push(error));
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) fn wait_for_tracee_stop(child: &mut Child, deadline: Instant) -> Result<(), String> {
    loop {
        let mut status = 0;
        // SAFETY: status is live, the PID belongs to Child, and WNOHANG keeps
        // the explicit deadline enforceable.
        let result = unsafe {
            libc::waitpid(
                child.id() as libc::pid_t,
                &mut status,
                libc::WNOHANG | libc::WUNTRACED,
            )
        };
        if result == child.id() as libc::pid_t {
            return if libc::WIFSTOPPED(status) {
                Ok(())
            } else {
                Err("target exited before its ptrace exec stop".to_owned())
            };
        }
        if result < 0 {
            return Err(format!("waitpid failed: {}", io::Error::last_os_error()));
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for target ptrace stop".to_owned());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn continue_tracee(child: &Child) -> Result<(), String> {
    // SAFETY: the coordinator owns a ptrace-stopped child and supplies no
    // address or data pointer.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_CONT,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "PTRACE_CONT failed: {}",
            io::Error::last_os_error()
        ))
    }
}

pub(crate) fn set_exit_kill(child: &Child) -> Result<(), String> {
    // SAFETY: the coordinator owns the ptrace-stopped child. EXITKILL ensures
    // an unexpected debugger death cannot orphan the launched target.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_SETOPTIONS,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            libc::PTRACE_O_EXITKILL as usize as *mut libc::c_void,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "PTRACE_SETOPTIONS(EXITKILL) failed: {}",
            io::Error::last_os_error()
        ))
    }
}

pub(crate) fn stop_tracee(child: &mut Child, deadline: Instant) -> Result<bool, String> {
    if child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(false);
    }
    // SAFETY: the PID belongs to the retained Child and SIGSTOP has no pointer
    // operand.
    if unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGSTOP) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        return Err(format!("SIGSTOP failed: {error}"));
    }
    wait_for_tracee_stop(child, deadline).map(|()| true)
}

pub(crate) fn detach_tracee(child: &Child) -> Result<(), String> {
    // SAFETY: the coordinator owns a stopped tracee. SIGCONT clears both the
    // ptrace stop and any process-wide group-stop while detaching.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            libc::SIGCONT as usize as *mut libc::c_void,
        )
    };
    if result != 0 {
        return Err(format!(
            "PTRACE_DETACH failed: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: the retained Child still pins the PID. ESRCH means it exited.
    let resumed = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGCONT) };
    if resumed == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!("SIGCONT failed: {}", io::Error::last_os_error()))
    }
}

pub(crate) fn detach_then_kill(child: &mut Child) {
    let _ = detach_tracee(child);
    kill_and_reap(child);
}

pub(crate) fn kill_and_reap(child: &mut Child) {
    let _ = kill_and_reap_result(child);
}

pub(crate) fn kill_and_reap_result(child: &mut Child) -> Result<(), String> {
    match child.try_wait() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(error) => return Err(format!("target status failed: {error}")),
    }
    child
        .kill()
        .map_err(|error| format!("target kill failed: {error}"))?;
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => return Err(format!("target reap failed: {error}")),
        }
        if Instant::now() >= deadline {
            return Err("timed out reaping killed target".to_owned());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn bootstrap_error(stage: &str, code: &str, message: &str) {
    let error = HardwareBootstrapErrorV2 {
        schema: "fe2o3-hardware-debug-bootstrap-error-v2",
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
    use std::cell::RefCell;

    use super::run_cleanup_actions;

    #[test]
    fn cleanup_is_kfd_then_detach_then_bounded_reap_even_after_error() {
        let calls = RefCell::new(Vec::new());
        let result = run_cleanup_actions(
            || {
                calls.borrow_mut().push("kfd_finish");
                Err("finish failed".to_owned())
            },
            || {
                calls.borrow_mut().push("ptrace_detach");
                Ok(())
            },
            || {
                calls.borrow_mut().push("kill_reap");
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(
            calls.into_inner(),
            ["kfd_finish", "ptrace_detach", "kill_reap"]
        );
    }
}
