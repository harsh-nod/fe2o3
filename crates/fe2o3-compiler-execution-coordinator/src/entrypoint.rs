use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::time::Duration;

use crate::{
    CompilerExecutionCoordinatorErrorV1, InheritedCompilerExecutionDeploymentV1,
    RootManagedCompilerExecutionServiceV1,
};

const LAUNCH_TIMEOUT_V1: Duration = Duration::from_secs(120);
const CONTINUITY_INTERVAL_V1: Duration = Duration::from_secs(1);
const ACTIVATION_DESCRIPTOR_COUNT_V1: &str = "14";
const ACTIVATION_DESCRIPTOR_NAMES_V1: &str = "compiler-execution-listener:supervisor-root:anchor-root:supervisor:launcher:issuer:anchor-helper:anchor-daemon:supervisor-deployment:issuer-policy:anchor-deployment:anchor-provisioning:issuer-key-seed:anchor-key-seed";

/// Runs the sole system-manager-activated root coordinator until graceful termination.
///
/// The caller must supply no arguments beyond `argv[0]`. Systemd activation metadata must bind the
/// exact current PID, 14 descriptors, and role names. The environment is cleared before any
/// authority input is admitted. `SIGTERM` and `SIGINT` are synchronously consumed while the
/// coordinator revalidates service continuity once per second.
pub fn run_inherited_compiler_execution_coordinator_v1()
-> Result<(), CompilerExecutionCoordinatorErrorV1> {
    validate_arguments()?;
    validate_activation_environment(
        rustix::process::getpid().as_raw_pid(),
        std::env::var_os("LISTEN_PID").as_deref(),
        std::env::var_os("LISTEN_FDS").as_deref(),
        std::env::var_os("LISTEN_FDNAMES").as_deref(),
    )?;
    clear_environment()?;
    let signals = BlockedTerminationSignalsV1::install()?;
    let service = InheritedCompilerExecutionDeploymentV1::admit()?.launch(LAUNCH_TIMEOUT_V1)?;
    monitor_service(service, &signals)
}

fn validate_arguments() -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    if std::env::args_os().count() != 1 {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "arguments are forbidden",
        ));
    }
    Ok(())
}

fn validate_activation_environment(
    pid: i32,
    listen_pid: Option<&OsStr>,
    listen_fds: Option<&OsStr>,
    listen_fdnames: Option<&OsStr>,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let expected_pid = pid.to_string();
    if pid <= 0 || listen_pid != Some(OsStr::new(&expected_pid)) {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "LISTEN_PID does not name this process",
        ));
    }
    if listen_fds != Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)) {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "LISTEN_FDS is not exactly 14",
        ));
    }
    if listen_fdnames != Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)) {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "LISTEN_FDNAMES does not match the fixed role order",
        ));
    }
    Ok(())
}

fn clear_environment() -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    // SAFETY: activation is required to be single-threaded and no Rust environment access follows.
    if unsafe { libc::clearenv() } != 0 {
        return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "cannot clear process environment",
        ));
    }
    Ok(())
}

struct BlockedTerminationSignalsV1 {
    set: libc::sigset_t,
}

impl BlockedTerminationSignalsV1 {
    fn install() -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        let mut set = MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY: each libc call receives initialized scalar arguments and a valid sigset pointer.
        let status = unsafe {
            if libc::sigemptyset(set.as_mut_ptr()) != 0 {
                -1
            } else {
                let mut set = set.assume_init();
                if libc::sigaddset(&mut set, libc::SIGTERM) != 0
                    || libc::sigaddset(&mut set, libc::SIGINT) != 0
                {
                    -1
                } else {
                    let status = libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
                    if status == 0 {
                        return Ok(Self { set });
                    }
                    status
                }
            }
        };
        let error = if status > 0 {
            io::Error::from_raw_os_error(status)
        } else {
            io::Error::last_os_error()
        };
        Err(CompilerExecutionCoordinatorErrorV1::Signal(error))
    }

    fn wait_interval(&self) -> Result<Option<i32>, CompilerExecutionCoordinatorErrorV1> {
        let timeout = libc::timespec {
            tv_sec: i64::try_from(CONTINUITY_INTERVAL_V1.as_secs()).expect("one second fits"),
            tv_nsec: i64::from(CONTINUITY_INTERVAL_V1.subsec_nanos()),
        };
        // SAFETY: the installed set remains initialized and blocked; timeout is a valid timespec.
        let signal = unsafe { libc::sigtimedwait(&self.set, std::ptr::null_mut(), &timeout) };
        if signal == libc::SIGTERM || signal == libc::SIGINT {
            return Ok(Some(signal));
        }
        if signal < 0 {
            let error = io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(libc::EAGAIN) | Some(libc::EINTR)) {
                return Ok(None);
            }
            return Err(CompilerExecutionCoordinatorErrorV1::Signal(error));
        }
        Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "unexpected signal escaped the blocked termination set",
        ))
    }
}

fn monitor_service(
    service: RootManagedCompilerExecutionServiceV1,
    signals: &BlockedTerminationSignalsV1,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    loop {
        if signals.wait_interval()?.is_some() {
            return service.shutdown();
        }
        service.validate_continuity()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_environment_requires_exact_pid_count_and_names() {
        let pid = 1234;
        let pid_text = pid.to_string();
        assert!(
            validate_activation_environment(
                pid,
                Some(OsStr::new(&pid_text)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
            )
            .is_ok()
        );
        for (listen_pid, listen_fds, names) in [
            (
                Some(OsStr::new("1235")),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
            ),
            (
                Some(OsStr::new(&pid_text)),
                Some(OsStr::new("13")),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
            ),
            (
                Some(OsStr::new(&pid_text)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new("compiler-execution-listener:substituted")),
            ),
            (None, None, None),
        ] {
            assert!(validate_activation_environment(pid, listen_pid, listen_fds, names).is_err());
        }
        assert!(
            validate_activation_environment(
                0,
                Some(OsStr::new("0")),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
            )
            .is_err()
        );
    }
}
