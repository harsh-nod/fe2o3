use std::io;
use std::io::ErrorKind;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::Duration;

const EXECUTABLE_BUSY_RETRIES: usize = 8;
const EXECUTABLE_BUSY_INITIAL_DELAY: Duration = Duration::from_millis(1);

/// Retries only the transient Linux fork/exec writer-alias failure.
///
/// The operation must do no work before attempting exec and must be safe to repeat when exec
/// returns `ETXTBSY`, which guarantees that no child image started.
pub(crate) fn retry_transient_executable_busy<T>(
    operation: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    retry_transient_executable_busy_with_policy(
        operation,
        EXECUTABLE_BUSY_RETRIES,
        EXECUTABLE_BUSY_INITIAL_DELAY,
    )
}

fn retry_transient_executable_busy_with_policy<T>(
    mut operation: impl FnMut() -> io::Result<T>,
    retries: usize,
    mut retry_delay: Duration,
) -> io::Result<T> {
    for attempt in 0..=retries {
        match operation() {
            Err(error) if error.kind() == ErrorKind::ExecutableFileBusy && attempt < retries => {
                std::thread::sleep(retry_delay);
                retry_delay *= 2;
            }
            result => return result,
        }
    }
    unreachable!("bounded executable-busy retry loop always returns")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
}

pub(crate) fn status(command: &mut Command) -> io::Result<ExitStatus> {
    let mut child = spawn(command)?;
    child.wait()
}

/// Runs a command whose stdout and stderr are intentionally captured.
///
/// `Command` does not expose whether its stdio was previously configured, so this helper has an
/// explicit contract instead of pretending to preserve every `Command::output` configuration.
/// Callers that configured stdout or stderr must use `output_with_configured_stdio`.
pub(crate) fn capture_output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    output_with_configured_stdio(command)
}

/// Runs a command after the caller has configured its desired stdout and stderr behavior.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn output_with_configured_stdio(command: &mut Command) -> io::Result<Output> {
    spawn(command)?.wait_with_output()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_busy_retry_is_bounded_to_the_exact_transient_error() {
        let mut calls = 0;
        let value = retry_transient_executable_busy_with_policy(
            || {
                calls += 1;
                if calls < 3 {
                    Err(ErrorKind::ExecutableFileBusy.into())
                } else {
                    Ok(17)
                }
            },
            2,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(value, 17);
        assert_eq!(calls, 3);

        let mut fatal_calls = 0;
        let error = retry_transient_executable_busy_with_policy(
            || {
                fatal_calls += 1;
                if fatal_calls < 3 {
                    Err::<(), _>(ErrorKind::ExecutableFileBusy.into())
                } else {
                    Err::<(), _>(ErrorKind::PermissionDenied.into())
                }
            },
            2,
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
        assert_eq!(fatal_calls, 3);

        let mut exhausted_calls = 0;
        let error = retry_transient_executable_busy_with_policy(
            || {
                exhausted_calls += 1;
                Err::<(), _>(ErrorKind::ExecutableFileBusy.into())
            },
            2,
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ExecutableFileBusy);
        assert_eq!(exhausted_calls, 3);
    }

    #[test]
    fn configured_output_preserves_explicit_stdout_and_stderr() {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("printf stdout; printf stderr >&2")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let output = output_with_configured_stdio(&mut command).unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn capture_output_collects_stdout_and_stderr() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("printf stdout; printf stderr >&2");

        let output = capture_output(&mut command).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"stdout");
        assert_eq!(output.stderr, b"stderr");
    }

    #[test]
    fn capture_output_command_remains_reusable() {
        let mut command = Command::new("/bin/printf");
        command.arg("reused");

        for _ in 0..2 {
            let output = capture_output(&mut command).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"reused");
            assert!(output.stderr.is_empty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn capture_output_propagates_pre_exec_failure() {
        use std::os::unix::process::CommandExt;

        let mut command = Command::new("/bin/true");
        // SAFETY: the callback returns one fixed error without accessing inherited process state.
        unsafe {
            command.pre_exec(|| Err(io::Error::from_raw_os_error(libc::EPERM)));
        }

        let error = capture_output(&mut command).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
    }
}
