use std::fs::File;
use std::os::fd::{AsRawFd as _, OwnedFd, RawFd};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1, COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
};
use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, fstat, fstatfs, llistxattr, openat2};
use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_getfd, fcntl_setfd};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use super::client_transaction::{
    CompilerExecutionClientTransactionEvidenceV1, require_client_transaction_report_absent_v1,
    try_admit_client_transaction_report_v1,
};
use super::fault::QualificationFaultHooksV1;
use super::provision::CompilerExecutionProvisionedQualificationV1;
use super::{
    COMPILER_EXECUTION_SYSTEMD_MACHINE_PARENT_PID_ENV_V1,
    COMPILER_EXECUTION_SYSTEMD_MACHINE_TOOL_COMMAND_V1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, QualificationFaultPointV1, changed, invalid, io_error,
    require_no_xattrs, snapshot, std_io_error,
};

const QUALIFICATION_STAGING_PREFIX_V1: &str = ".compiler-execution-qualification-v1-";
const MACHINE_NAME_PREFIX_V1: &str = "fe2o3-q-";
const BASE_STDIN_PATH_V1: &str = "/proc/self/fd/0";
const ROOT_STDOUT_PATH_V1: &str = "/proc/self/fd/1";
const SQUASHFS_MAGIC_V1: i64 = 0x7371_7368;
const OVERLAYFS_MAGIC_V1: i64 = 0x794c_7630;
const PINNED_LOADER_PATH_V1: &str = "usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
const PINNED_NSPAWN_PATH_V1: &str = "usr/bin/systemd-nspawn";
const PINNED_LOADER_BYTE_LEN_V1: i64 = 236_616;
const PINNED_NSPAWN_BYTE_LEN_V1: i64 = 368_400;
const PINNED_LIBRARY_PATHS_V1: &[&str] = &[
    "usr/lib/x86_64-linux-gnu",
    "usr/lib/x86_64-linux-gnu/systemd",
];
const COMPILER_UID_V1: u32 = 999;
const READINESS_TIMEOUT_V1: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT_V1: Duration = Duration::from_secs(30);
const POLL_INTERVAL_V1: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QualificationMachineIdentityV1 {
    machine_name: String,
    uuid: String,
}

impl QualificationMachineIdentityV1 {
    pub(super) fn from_staging_name(
        staging_name: &str,
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        let suffix = staging_name
            .strip_prefix(QUALIFICATION_STAGING_PREFIX_V1)
            .filter(|suffix| {
                suffix.len() == 32
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
            .ok_or_else(|| {
                invalid(
                    DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                    "qualification staging identity cannot name one isolated machine",
                )
            })?;
        let uuid = format!(
            "{}-{}-{}-{}-{}",
            &suffix[..8],
            &suffix[8..12],
            &suffix[12..16],
            &suffix[16..20],
            &suffix[20..]
        );
        Ok(Self {
            machine_name: format!("{MACHINE_NAME_PREFIX_V1}{suffix}"),
            uuid,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PinnedSystemdNspawnPlanV1 {
    program: String,
    arguments: Vec<String>,
    environment: Vec<(&'static str, &'static str)>,
}

impl PinnedSystemdNspawnPlanV1 {
    pub(super) fn program(&self) -> &str {
        &self.program
    }

    pub(super) fn arguments(&self) -> &[String] {
        &self.arguments
    }

    pub(super) fn environment(&self) -> &[(&'static str, &'static str)] {
        &self.environment
    }
}

pub(super) fn pinned_systemd_nspawn_plan_v1(
    base_descriptor: RawFd,
    root_descriptor: RawFd,
    identity: &QualificationMachineIdentityV1,
) -> Result<PinnedSystemdNspawnPlanV1, DeploymentVerificationErrorV1> {
    if base_descriptor < 3 || root_descriptor < 3 || base_descriptor == root_descriptor {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "systemd machine requires distinct inherited base and root descriptors",
        ));
    }
    let base = format!("/proc/self/fd/{base_descriptor}");
    let root = format!("/proc/self/fd/{root_descriptor}");
    let program = format!("{base}/{PINNED_LOADER_PATH_V1}");
    let library_path = PINNED_LIBRARY_PATHS_V1
        .iter()
        .map(|path| format!("{base}/{path}"))
        .collect::<Vec<_>>()
        .join(":");
    let arguments = vec![
        "--library-path".to_owned(),
        library_path,
        format!("{base}/{PINNED_NSPAWN_PATH_V1}"),
        "--quiet".to_owned(),
        "--no-pager".to_owned(),
        "--settings=no".to_owned(),
        format!("--directory={root}"),
        "--boot".to_owned(),
        "--register=no".to_owned(),
        "--keep-unit".to_owned(),
        "--private-users=no".to_owned(),
        "--private-network".to_owned(),
        "--volatile=no".to_owned(),
        "--link-journal=no".to_owned(),
        "--resolv-conf=off".to_owned(),
        "--timezone=off".to_owned(),
        "--console=pipe".to_owned(),
        "--notify-ready=yes".to_owned(),
        "--kill-signal=SIGRTMIN+3".to_owned(),
        "--bind=+/run/fe2o3:/run/fe2o3:norbind,noidmap".to_owned(),
        format!("--machine={}", identity.machine_name),
        "--hostname=fe2o3-qualification".to_owned(),
        format!("--uuid={}", identity.uuid),
        "--".to_owned(),
        "--unit=fe2o3-qualification.target".to_owned(),
    ];
    Ok(PinnedSystemdNspawnPlanV1 {
        program,
        arguments,
        environment: vec![
            ("LANG", "C"),
            ("LC_ALL", "C"),
            ("TZ", "UTC"),
            ("SYSTEMD_COLORS", "0"),
            ("SYSTEMD_LOG_LEVEL", "warning"),
            ("SYSTEMD_PAGER", "cat"),
        ],
    })
}

/// Replaces the hidden helper with the pinned loader and `systemd-nspawn` from the base.
///
/// The qualification worker passes the retained SquashFS base on stdin and the composed
/// OverlayFS root on stdout. The binary entrypoint must bind this process to its exact parent
/// before calling this function. Success does not return.
pub fn execute_compiler_execution_systemd_machine_tool_v1(
    staging_name: &str,
) -> Result<std::convert::Infallible, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "systemd machine helper requires effective UID 0",
        ));
    }
    if super::host::process_thread_count()? != 1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "systemd machine helper requires one task",
        ));
    }
    let identity = QualificationMachineIdentityV1::from_staging_name(staging_name)?;
    let base = File::open(BASE_STDIN_PATH_V1)
        .map_err(|source| std_io_error("open inherited pinned-base stdin", source))?;
    let root = File::open(ROOT_STDOUT_PATH_V1)
        .map_err(|source| std_io_error("open inherited composed-root stdout", source))?;
    validate_machine_root(&base, SQUASHFS_MAGIC_V1, "pinned SquashFS base")?;
    validate_machine_root(&root, OVERLAYFS_MAGIC_V1, "composed OverlayFS root")?;
    validate_pinned_executable(&base, PINNED_LOADER_PATH_V1, PINNED_LOADER_BYTE_LEN_V1)?;
    validate_pinned_executable(&base, PINNED_NSPAWN_PATH_V1, PINNED_NSPAWN_BYTE_LEN_V1)?;

    let inherited_base = inherit_exec_descriptor(&base, 10)?;
    let inherited_root = inherit_exec_descriptor(&root, 11)?;
    let plan = pinned_systemd_nspawn_plan_v1(
        inherited_base.as_raw_fd(),
        inherited_root.as_raw_fd(),
        &identity,
    )?;
    let error = Command::new(plan.program())
        .args(plan.arguments())
        .env_clear()
        .envs(plan.environment().iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .exec();
    Err(std_io_error(
        "replace systemd machine helper with pinned systemd-nspawn",
        error,
    ))
}

fn validate_machine_root(
    root: &File,
    expected_magic: i64,
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let stat = fstat(root).map_err(|source| io_error("inspect inherited machine root", source))?;
    let filesystem =
        fstatfs(root).map_err(|source| io_error("inspect inherited machine filesystem", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != 0o755
        || (stat.st_uid, stat.st_gid) != (0, 0)
        || filesystem.f_type != expected_magic
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            format!("systemd machine did not inherit the exact {role}"),
        ));
    }
    Ok(())
}

fn validate_pinned_executable(
    base: &File,
    path: &str,
    expected_byte_len: i64,
) -> Result<(), DeploymentVerificationErrorV1> {
    let executable = openat2(
        base,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map_err(|source| io_error("open pinned systemd machine executable", source))?;
    let stat = fstat(&executable)
        .map_err(|source| io_error("inspect pinned systemd machine executable", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != 0o755
        || (stat.st_uid, stat.st_gid) != (0, 0)
        || stat.st_nlink != 1
        || stat.st_size != expected_byte_len
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            format!("pinned systemd machine executable {path} has noncanonical metadata"),
        ));
    }
    require_no_xattrs(&executable, "pinned systemd machine executable")
}

fn inherit_exec_descriptor(
    original: &File,
    minimum: RawFd,
) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
    let inherited = fcntl_dupfd_cloexec(original, minimum)
        .map_err(|source| io_error("duplicate inherited systemd machine descriptor", source))?;
    fcntl_setfd(&inherited, FdFlags::empty())
        .map_err(|source| io_error("make systemd machine descriptor exec-inheritable", source))?;
    let flags = fcntl_getfd(&inherited)
        .map_err(|source| io_error("reinspect systemd machine descriptor flags", source))?;
    let original_stat =
        fstat(original).map_err(|source| io_error("inspect systemd machine descriptor", source))?;
    let inherited_stat = fstat(&inherited)
        .map_err(|source| io_error("inspect inherited systemd machine descriptor", source))?;
    if inherited.as_raw_fd() < minimum
        || !flags.is_empty()
        || (original_stat.st_dev, original_stat.st_ino)
            != (inherited_stat.st_dev, inherited_stat.st_ino)
    {
        return Err(changed(
            "systemd machine descriptor lost exact exec-inheritable custody",
        ));
    }
    Ok(inherited)
}

pub(super) fn boot_and_stop_systemd_machine_v1(
    provisioned: &CompilerExecutionProvisionedQualificationV1,
    staging_name: &str,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<CompilerExecutionClientTransactionEvidenceV1, DeploymentVerificationErrorV1> {
    QualificationMachineIdentityV1::from_staging_name(staging_name)?;
    let (base, root) = provisioned.inherit_systemd_machine_descriptors()?;
    let mut machine = RunningSystemdMachineV1::spawn(&base, &root, staging_name)?;
    hooks.checkpoint(QualificationFaultPointV1::SystemdMachineSpawned)?;
    let socket = await_machine_socket(&mut machine, &root, READINESS_TIMEOUT_V1)?;
    hooks.checkpoint(QualificationFaultPointV1::SupervisorSocketMetadataAdmitted)?;
    let transaction =
        await_client_transaction(&mut machine, &root, provisioned, READINESS_TIMEOUT_V1)?;
    hooks.checkpoint(QualificationFaultPointV1::ClientTransactionComplete)?;
    socket.revalidate(&root)?;
    transaction.revalidate(&root)?;
    provisioned.revalidate_systemd_machine_state()?;
    hooks.checkpoint(QualificationFaultPointV1::ClientTransactionRevalidated)?;
    hooks.checkpoint(QualificationFaultPointV1::SystemdMachineReady)?;
    machine.stop(SHUTDOWN_TIMEOUT_V1)?;
    hooks.checkpoint(QualificationFaultPointV1::SystemdMachineStopped)?;
    require_machine_socket_absent(&root)?;
    require_client_transaction_report_absent_v1(&root)?;
    provisioned.revalidate_systemd_machine_state()?;
    hooks.checkpoint(QualificationFaultPointV1::PostBootLowerRevalidated)?;
    Ok(transaction)
}

struct RunningSystemdMachineV1 {
    child: Option<Child>,
    pidfd: OwnedFd,
}

impl RunningSystemdMachineV1 {
    fn spawn(
        base: &OwnedFd,
        root: &OwnedFd,
        staging_name: &str,
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        let child_base = rustix::io::dup(base)
            .map_err(|source| io_error("duplicate machine base for helper", source))?;
        let child_root = rustix::io::dup(root)
            .map_err(|source| io_error("duplicate machine root for helper", source))?;
        let mut child = Command::new("/proc/self/exe")
            .arg(COMPILER_EXECUTION_SYSTEMD_MACHINE_TOOL_COMMAND_V1)
            .arg(staging_name)
            .env_clear()
            .env(
                COMPILER_EXECUTION_SYSTEMD_MACHINE_PARENT_PID_ENV_V1,
                std::process::id().to_string(),
            )
            .stdin(Stdio::from(child_base))
            .stdout(Stdio::from(child_root))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| std_io_error("spawn pinned systemd machine helper", source))?;
        let pid = Pid::from_child(&child);
        let child_group = rustix::process::getpgid(Some(pid));
        if child_group != Ok(rustix::process::getpgrp()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(match child_group {
                Ok(_) => invalid(
                    DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                    "systemd machine helper escaped the qualification worker process group",
                ),
                Err(source) => io_error("inspect systemd machine helper process group", source),
            });
        }
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io_error("open exact systemd machine pidfd", source));
            }
        };
        Ok(Self {
            child: Some(child),
            pidfd,
        })
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, DeploymentVerificationErrorV1> {
        self.child
            .as_mut()
            .expect("running systemd machine retains one child")
            .try_wait()
            .map_err(|source| std_io_error("poll systemd machine helper", source))
    }

    fn stop(&mut self, timeout: Duration) -> Result<(), DeploymentVerificationErrorV1> {
        pidfd_send_signal(&self.pidfd, Signal::TERM)
            .map_err(|source| io_error("request systemd machine shutdown", source))?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_wait()? {
                self.child.take();
                if status.success() {
                    return Ok(());
                }
                return Err(machine_exit_error("during shutdown", status));
            }
            if Instant::now() >= deadline {
                self.force_stop_and_reap();
                return Err(invalid(
                    DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                    "systemd machine did not stop within the fixed timeout",
                ));
            }
            std::thread::sleep(POLL_INTERVAL_V1);
        }
    }

    fn force_stop_and_reap(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = pidfd_send_signal(&self.pidfd, Signal::TERM);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL_V1),
                _ => break,
            }
        }
        let _ = pidfd_send_signal(&self.pidfd, Signal::KILL);
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for RunningSystemdMachineV1 {
    fn drop(&mut self) {
        self.force_stop_and_reap();
    }
}

fn await_machine_socket(
    machine: &mut RunningSystemdMachineV1,
    root: &OwnedFd,
    timeout: Duration,
) -> Result<MachineSocketReadinessV1, DeploymentVerificationErrorV1> {
    let deadline = Instant::now() + timeout;
    let policy = MachineSocketPolicyV1::production();
    loop {
        if let Some(readiness) = try_admit_machine_socket(root, policy)? {
            return Ok(readiness);
        }
        if let Some(status) = machine.try_wait()? {
            machine.child.take();
            return Err(machine_exit_error("before readiness", status));
        }
        if Instant::now() >= deadline {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                "systemd machine did not publish the exact supervisor socket within the fixed timeout",
            ));
        }
        std::thread::sleep(POLL_INTERVAL_V1);
    }
}

#[derive(Debug)]
struct MachineSocketReadinessV1 {
    socket: OwnedFd,
    snapshot: super::ObjectSnapshotV1,
    policy: MachineSocketPolicyV1<'static>,
}

impl MachineSocketReadinessV1 {
    fn revalidate(&self, root: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_machine_socket(root, &self.socket, self.snapshot, self.policy)
    }
}

#[derive(Clone, Copy, Debug)]
struct MachineSocketPolicyV1<'a> {
    relative_path: &'a str,
    socket_owner: (u32, u32),
}

impl MachineSocketPolicyV1<'static> {
    fn production() -> Self {
        Self {
            relative_path: COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1
                .strip_prefix('/')
                .expect("the fixed production listener path is absolute"),
            socket_owner: (0, COMPILER_UID_V1),
        }
    }
}

fn try_admit_machine_socket(
    root: &OwnedFd,
    policy: MachineSocketPolicyV1<'_>,
) -> Result<Option<MachineSocketReadinessV1>, DeploymentVerificationErrorV1> {
    let socket = match open_machine_socket(root, policy.relative_path) {
        Ok(socket) => socket,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(source) => return Err(io_error("open systemd machine readiness socket", source)),
    };
    let before = validate_machine_socket_metadata(root, &socket, policy)?;
    revalidate_machine_socket(root, &socket, before, policy)?;
    Ok(Some(MachineSocketReadinessV1 {
        socket,
        snapshot: before,
        policy: MachineSocketPolicyV1::production(),
    }))
}

fn await_client_transaction(
    machine: &mut RunningSystemdMachineV1,
    root: &OwnedFd,
    provisioned: &CompilerExecutionProvisionedQualificationV1,
    timeout: Duration,
) -> Result<CompilerExecutionClientTransactionEvidenceV1, DeploymentVerificationErrorV1> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(transaction) = try_admit_client_transaction_report_v1(
            root,
            provisioned.client_profile(),
            (
                provisioned.qualification_client_uid(),
                provisioned.qualification_client_gid(),
            ),
        )? {
            return Ok(transaction);
        }
        if let Some(status) = machine.try_wait()? {
            machine.child.take();
            return Err(machine_exit_error(
                "before client transaction completion",
                status,
            ));
        }
        if Instant::now() >= deadline {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                "systemd machine did not complete the canonical non-root client transaction within the fixed timeout",
            ));
        }
        std::thread::sleep(POLL_INTERVAL_V1);
    }
}

fn open_machine_socket(root: &OwnedFd, relative_path: &str) -> Result<OwnedFd, rustix::io::Errno> {
    openat2(
        root,
        relative_path,
        OFlags::PATH | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
}

fn validate_machine_socket_metadata(
    root: &OwnedFd,
    socket: &OwnedFd,
    policy: MachineSocketPolicyV1<'_>,
) -> Result<super::ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let before = snapshot(
        &fstat(socket)
            .map_err(|source| io_error("inspect systemd machine readiness socket", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::Socket
        || before.mode & 0o7777 != COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1
        || (before.uid, before.gid) != policy.socket_owner
        || before.links != 1
        || before.byte_len != 0
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "systemd machine readiness socket metadata is not canonical",
        ));
    }
    let path = format!(
        "/proc/self/fd/{}/{}",
        root.as_raw_fd(),
        policy.relative_path
    );
    let mut attributes = [0_u8; 1];
    match llistxattr(path, &mut attributes) {
        Ok(0) => {}
        Ok(_) | Err(rustix::io::Errno::RANGE) => {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::ForbiddenAttributes,
                "systemd machine readiness socket carries an extended attribute",
            ));
        }
        Err(source) => {
            return Err(io_error(
                "inspect systemd machine readiness socket attributes",
                source,
            ));
        }
    }
    Ok(before)
}

fn revalidate_machine_socket(
    root: &OwnedFd,
    retained: &OwnedFd,
    before: super::ObjectSnapshotV1,
    policy: MachineSocketPolicyV1<'_>,
) -> Result<(), DeploymentVerificationErrorV1> {
    let retained_after = snapshot(
        &fstat(retained)
            .map_err(|source| io_error("reinspect retained machine readiness socket", source))?,
    );
    if retained_after != before {
        return Err(changed(
            "systemd machine readiness socket changed during metadata admission",
        ));
    }
    validate_machine_socket_metadata(root, retained, policy)?;
    let reopened = open_machine_socket(root, policy.relative_path)
        .map_err(|source| io_error("reopen systemd machine readiness socket", source))?;
    let reopened_snapshot = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect systemd machine readiness socket", source))?,
    );
    if reopened_snapshot != before {
        return Err(changed(
            "systemd machine readiness socket pathname changed during metadata admission",
        ));
    }
    Ok(())
}

fn require_machine_socket_absent(root: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
    match open_machine_socket(root, MachineSocketPolicyV1::production().relative_path) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "systemd machine readiness socket remained after shutdown",
        )),
        Err(source) => Err(io_error(
            "verify systemd machine readiness socket removal",
            source,
        )),
    }
}

fn machine_exit_error(stage: &'static str, status: ExitStatus) -> DeploymentVerificationErrorV1 {
    invalid(
        DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
        format!(
            "systemd machine exited {stage}: exit_code={:?} signal={:?}",
            status.code(),
            status.signal()
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsFd as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::PathBuf;

    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, bind, listen, socket_with,
    };

    use super::*;

    #[test]
    fn staging_identity_derives_one_machine_name_and_uuid() {
        let identity = QualificationMachineIdentityV1::from_staging_name(
            ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        assert_eq!(
            identity.machine_name,
            "fe2o3-q-0123456789abcdef0123456789abcdef"
        );
        assert_eq!(identity.uuid, "01234567-89ab-cdef-0123-456789abcdef");
        for invalid_name in [
            "compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef",
            ".compiler-execution-qualification-v1-0123456789abcdef",
            ".compiler-execution-qualification-v1-0123456789ABCDEF0123456789ABCDEF",
            ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdeg",
        ] {
            assert_eq!(
                QualificationMachineIdentityV1::from_staging_name(invalid_name)
                    .unwrap_err()
                    .kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation
            );
        }
    }

    #[test]
    fn nspawn_plan_executes_only_pinned_base_bytes_against_composed_root() {
        let identity = QualificationMachineIdentityV1::from_staging_name(
            ".compiler-execution-qualification-v1-fedcba9876543210fedcba9876543210",
        )
        .unwrap();
        let plan = pinned_systemd_nspawn_plan_v1(10, 11, &identity).unwrap();
        assert_eq!(
            plan.program(),
            "/proc/self/fd/10/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"
        );
        assert_eq!(
            plan.arguments(),
            [
                "--library-path",
                "/proc/self/fd/10/usr/lib/x86_64-linux-gnu:/proc/self/fd/10/usr/lib/x86_64-linux-gnu/systemd",
                "/proc/self/fd/10/usr/bin/systemd-nspawn",
                "--quiet",
                "--no-pager",
                "--settings=no",
                "--directory=/proc/self/fd/11",
                "--boot",
                "--register=no",
                "--keep-unit",
                "--private-users=no",
                "--private-network",
                "--volatile=no",
                "--link-journal=no",
                "--resolv-conf=off",
                "--timezone=off",
                "--console=pipe",
                "--notify-ready=yes",
                "--kill-signal=SIGRTMIN+3",
                "--bind=+/run/fe2o3:/run/fe2o3:norbind,noidmap",
                "--machine=fe2o3-q-fedcba9876543210fedcba9876543210",
                "--hostname=fe2o3-qualification",
                "--uuid=fedcba98-7654-3210-fedc-ba9876543210",
                "--",
                "--unit=fe2o3-qualification.target",
            ]
        );
        assert_eq!(
            plan.environment(),
            [
                ("LANG", "C"),
                ("LC_ALL", "C"),
                ("TZ", "UTC"),
                ("SYSTEMD_COLORS", "0"),
                ("SYSTEMD_LOG_LEVEL", "warning"),
                ("SYSTEMD_PAGER", "cat"),
            ]
        );
    }

    #[test]
    fn nspawn_plan_rejects_stdio_and_aliased_descriptors() {
        let identity = QualificationMachineIdentityV1::from_staging_name(
            ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        for (base, root) in [(0, 11), (10, 2), (10, 10)] {
            assert_eq!(
                pinned_systemd_nspawn_plan_v1(base, root, &identity)
                    .unwrap_err()
                    .kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation
            );
        }
    }

    #[test]
    fn inherited_exec_descriptor_is_exact_non_cloexec_and_above_minimum() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = File::open(temporary.path()).unwrap();
        let inherited = inherit_exec_descriptor(&directory, 20).unwrap();
        assert!(inherited.as_raw_fd() >= 20);
        assert!(fcntl_getfd(&inherited).unwrap().is_empty());
        let expected = fstat(directory.as_fd()).unwrap();
        let observed = fstat(&inherited).unwrap();
        assert_eq!(
            (expected.st_dev, expected.st_ino),
            (observed.st_dev, observed.st_ino)
        );
    }

    #[test]
    fn readiness_admits_and_retains_exact_socket_metadata_without_connecting() {
        let fixture = ListenerFixtureV1::new();
        let readiness = try_admit_machine_socket(&fixture.root, fixture.policy())
            .unwrap()
            .expect("the listening socket metadata is immediately available");
        assert_eq!(
            FileType::from_raw_mode(fstat(&readiness.socket).unwrap().st_mode),
            FileType::Socket
        );
    }

    #[test]
    fn readiness_absence_is_retryable_without_accepting_other_objects() {
        let temporary = tempfile::tempdir().unwrap();
        let root: OwnedFd = File::open(temporary.path()).unwrap().into();
        let missing = temporary.path().join("listener.sock");
        let policy = policy();
        assert!(try_admit_machine_socket(&root, policy).unwrap().is_none());

        fs::write(&missing, []).unwrap();
        fs::set_permissions(
            &missing,
            fs::Permissions::from_mode(COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1),
        )
        .unwrap();
        assert_eq!(
            try_admit_machine_socket(&root, policy).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot
        );
    }

    #[test]
    fn readiness_rejects_wrong_mode() {
        let wrong_mode = ListenerFixtureV1::new();
        fs::set_permissions(&wrong_mode.path, fs::Permissions::from_mode(0o666)).unwrap();
        assert_eq!(
            try_admit_machine_socket(&wrong_mode.root, wrong_mode.policy())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot
        );
    }

    struct ListenerFixtureV1 {
        _temporary: tempfile::TempDir,
        root: OwnedFd,
        _listener: OwnedFd,
        path: PathBuf,
    }

    impl ListenerFixtureV1 {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let path = temporary.path().join("listener.sock");
            let listener = socket_with(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .unwrap();
            let address = SocketAddrUnix::new(&path).unwrap();
            bind(&listener, &address).unwrap();
            listen(&listener, 4).unwrap();
            fs::set_permissions(
                &path,
                fs::Permissions::from_mode(COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1),
            )
            .unwrap();
            let root: OwnedFd = File::open(temporary.path()).unwrap().into();
            Self {
                _temporary: temporary,
                root,
                _listener: listener,
                path,
            }
        }

        fn policy(&self) -> MachineSocketPolicyV1<'_> {
            policy()
        }
    }

    fn policy() -> MachineSocketPolicyV1<'static> {
        MachineSocketPolicyV1 {
            relative_path: "listener.sock",
            socket_owner: (
                rustix::process::geteuid().as_raw(),
                rustix::process::getegid().as_raw(),
            ),
        }
    }
}
