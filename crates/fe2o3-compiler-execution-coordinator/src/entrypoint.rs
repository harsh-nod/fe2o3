use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::time::Duration;

use rustix::net::{
    AddressFamily, SendFlags, SocketAddrUnix, SocketFlags, SocketType, sendto, socket_with,
};

use crate::{
    CompilerExecutionCoordinatorErrorV1, InheritedCompilerExecutionDeploymentV1,
    RootManagedCompilerExecutionServiceV1,
};

const LAUNCH_TIMEOUT_V1: Duration = Duration::from_secs(120);
const CONTINUITY_INTERVAL_V1: Duration = Duration::from_secs(1);
const ACTIVATION_DESCRIPTOR_COUNT_V1: &str = "14";
const ACTIVATION_DESCRIPTOR_NAMES_V1: &str = "runtime-root:supervisor-root:anchor-root:supervisor:launcher:issuer:anchor-helper:anchor-daemon:supervisor-deployment:issuer-policy:anchor-deployment:anchor-provisioning:issuer-key-seed:anchor-key-seed";
const UNIX_SOCKET_PATH_MAX_BYTES_V1: usize = 107;
const SYSTEMD_READY_RECORD_V1: &[u8] = b"READY=1";

/// Runs the sole system-manager-activated root coordinator until graceful termination.
///
/// The caller must supply no arguments beyond `argv[0]`. Systemd activation metadata must bind the
/// exact current PID, 14 descriptors, and role names. The environment is cleared before any
/// authority input is admitted. `SIGTERM` and `SIGINT` are synchronously consumed while the
/// coordinator revalidates service continuity once per second.
pub fn run_inherited_compiler_execution_coordinator_v1()
-> Result<(), CompilerExecutionCoordinatorErrorV1> {
    validate_arguments()?;
    let pid = rustix::process::getpid().as_raw_pid();
    let readiness = validate_activation_environment(
        pid,
        std::env::var_os("LISTEN_PID").as_deref(),
        std::env::var_os("LISTEN_FDS").as_deref(),
        std::env::var_os("LISTEN_FDNAMES").as_deref(),
        std::env::var_os("NOTIFY_SOCKET").as_deref(),
    )?;
    clear_environment()?;
    let signals = BlockedTerminationSignalsV1::install()?;
    let service = InheritedCompilerExecutionDeploymentV1::admit()?.launch(LAUNCH_TIMEOUT_V1)?;
    readiness.publish()?;
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
    notify_socket: Option<&OsStr>,
) -> Result<SystemdReadinessPlanV1, CompilerExecutionCoordinatorErrorV1> {
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
    SystemdReadinessPlanV1::parse(pid, notify_socket)
}

#[derive(Debug, Eq, PartialEq)]
struct SystemdReadinessPlanV1 {
    main_pid: i32,
    target: SystemdNotifyTargetV1,
}

impl SystemdReadinessPlanV1 {
    fn parse(
        main_pid: i32,
        notify_socket: Option<&OsStr>,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        if main_pid <= 0 {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "NotifyAccess=main requires a positive coordinator PID",
            ));
        }
        let bytes = notify_socket.ok_or(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
            "NOTIFY_SOCKET is required",
        ))?;
        let bytes = bytes.as_bytes();
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "NOTIFY_SOCKET is not an exact Unix socket address",
            ));
        }
        let (abstract_namespace, address) = if bytes[0] == b'@' {
            (true, &bytes[1..])
        } else if bytes[0] == b'/' {
            (false, bytes)
        } else {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "NOTIFY_SOCKET is neither absolute nor abstract",
            ));
        };
        if address.is_empty() || address.len() > UNIX_SOCKET_PATH_MAX_BYTES_V1 {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "NOTIFY_SOCKET exceeds the exact AF_UNIX address bound",
            ));
        }
        let mut bounded = [0_u8; UNIX_SOCKET_PATH_MAX_BYTES_V1];
        bounded[..address.len()].copy_from_slice(address);
        Ok(Self {
            main_pid,
            target: SystemdNotifyTargetV1 {
                bytes: bounded,
                length: address.len(),
                abstract_namespace,
            },
        })
    }

    fn publish(self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        if rustix::process::getpid().as_raw_pid() != self.main_pid {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "NotifyAccess=main sender PID changed before readiness",
            ));
        }
        let address = self.target.socket_address()?;
        let socket = socket_with(
            AddressFamily::UNIX,
            SocketType::DGRAM,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "create systemd readiness socket",
            source: source.into(),
        })?;
        let sent = sendto(
            &socket,
            SYSTEMD_READY_RECORD_V1,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            &address,
        )
        .map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "publish systemd readiness",
            source: source.into(),
        })?;
        if sent != SYSTEMD_READY_RECORD_V1.len() {
            return Err(CompilerExecutionCoordinatorErrorV1::InvalidActivation(
                "systemd readiness datagram was not exact",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SystemdNotifyTargetV1 {
    bytes: [u8; UNIX_SOCKET_PATH_MAX_BYTES_V1],
    length: usize,
    abstract_namespace: bool,
}

impl SystemdNotifyTargetV1 {
    fn socket_address(&self) -> Result<SocketAddrUnix, CompilerExecutionCoordinatorErrorV1> {
        let bytes = &self.bytes[..self.length];
        let result = if self.abstract_namespace {
            SocketAddrUnix::new_abstract_name(bytes)
        } else {
            SocketAddrUnix::new(OsStr::from_bytes(bytes))
        };
        result.map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "construct systemd readiness address",
            source: source.into(),
        })
    }
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
    use std::os::unix::net::UnixDatagram;

    use super::*;

    const TEST_NOTIFY_SOCKET_V1: &str = "/run/systemd/notify";

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
                Some(OsStr::new(TEST_NOTIFY_SOCKET_V1)),
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
            assert!(
                validate_activation_environment(
                    pid,
                    listen_pid,
                    listen_fds,
                    names,
                    Some(OsStr::new(TEST_NOTIFY_SOCKET_V1)),
                )
                .is_err()
            );
        }
        assert!(
            validate_activation_environment(
                0,
                Some(OsStr::new("0")),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
                Some(OsStr::new(TEST_NOTIFY_SOCKET_V1)),
            )
            .is_err()
        );
        assert!(
            validate_activation_environment(
                pid,
                Some(OsStr::new(&pid_text)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_NAMES_V1)),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn activation_rejects_the_retired_listener_role() {
        let pid = 1234;
        let old_names = ACTIVATION_DESCRIPTOR_NAMES_V1.replacen(
            "runtime-root",
            "compiler-execution-listener",
            1,
        );
        assert!(
            validate_activation_environment(
                pid,
                Some(OsStr::new("1234")),
                Some(OsStr::new(ACTIVATION_DESCRIPTOR_COUNT_V1)),
                Some(OsStr::new(&old_names)),
                Some(OsStr::new(TEST_NOTIFY_SOCKET_V1)),
            )
            .is_err()
        );
    }

    #[test]
    fn readiness_plan_parses_only_bounded_filesystem_or_abstract_addresses() {
        let pid = rustix::process::getpid().as_raw_pid();
        let filesystem =
            SystemdReadinessPlanV1::parse(pid, Some(OsStr::new("/run/systemd/notify"))).unwrap();
        assert_eq!(filesystem.main_pid, pid);
        assert!(!filesystem.target.abstract_namespace);
        assert_eq!(
            &filesystem.target.bytes[..filesystem.target.length],
            b"/run/systemd/notify"
        );
        let abstract_target =
            SystemdReadinessPlanV1::parse(pid, Some(OsStr::from_bytes(b"@fe2o3-notify"))).unwrap();
        assert!(abstract_target.target.abstract_namespace);
        assert_eq!(
            &abstract_target.target.bytes[..abstract_target.target.length],
            b"fe2o3-notify"
        );

        let oversized = vec![b'a'; UNIX_SOCKET_PATH_MAX_BYTES_V1 + 1];
        let mut oversized_path = vec![b'/'];
        oversized_path.extend_from_slice(&oversized);
        for invalid in [
            &b""[..],
            &b"relative"[..],
            &b"@"[..],
            &b"/bad\0path"[..],
            oversized_path.as_slice(),
        ] {
            assert!(SystemdReadinessPlanV1::parse(pid, Some(OsStr::from_bytes(invalid))).is_err());
        }
    }

    #[test]
    fn readiness_plan_publishes_one_exact_record_from_the_main_pid() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("notify.sock");
        let receiver = UnixDatagram::bind(&path).unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let pid = rustix::process::getpid().as_raw_pid();
        SystemdReadinessPlanV1::parse(pid, Some(path.as_os_str()))
            .unwrap()
            .publish()
            .unwrap();

        let mut bytes = [0_u8; 32];
        let received = receiver.recv(&mut bytes).unwrap();
        assert_eq!(&bytes[..received], SYSTEMD_READY_RECORD_V1);
    }

    #[test]
    fn readiness_plan_rejects_a_notify_access_main_pid_change() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("notify.sock");
        let _receiver = UnixDatagram::bind(&path).unwrap();
        let pid = rustix::process::getpid().as_raw_pid();
        assert!(
            SystemdReadinessPlanV1::parse(pid + 1, Some(path.as_os_str()))
                .unwrap()
                .publish()
                .is_err()
        );
    }
}
