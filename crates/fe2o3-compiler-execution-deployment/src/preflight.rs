use std::fs::File;
use std::io::Read as _;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt as _;
use std::os::unix::process::CommandExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::process::{Command, Stdio};

use rustix::fs::{FileType, MemfdFlags, Mode, OFlags, ResolveFlags, fstat, memfd_create, openat2};
use rustix::process::{Resource, Rlimit, setrlimit};

use super::fault::QualificationFaultHooksV1;
use super::{
    COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_PARENT_PID_ENV_V1,
    COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_TOOL_COMMAND_V1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, MountedCompilerExecutionQualificationV1,
    QualificationFaultPointV1, changed, io_error, require_no_xattrs, snapshot, std_io_error,
};

const COMPOSED_ROOT_STDIN_PATH_V1: &str = "/proc/self/fd/0";
const OVERLAYFS_MAGIC_V1: i64 = 0x794c_7630;
const SYSTEMD_VERSION_V1: &str = "255.4-1ubuntu8.17";
const SYSTEMD_VERSION_LINE_V1: &str = "systemd 255 (255.4-1ubuntu8.17)";
const SYSTEMD_TOOL_OUTPUT_MAX_BYTES_V1: u64 = 64 * 1024;
const COMPILER_UID_V1: u32 = 999;
const ANCHOR_UID_V1: u32 = 998;
const ACCOUNT_DATABASE_MAX_BYTES_V1: u64 = 16 * 1024;
const VERIFIED_SYSTEMD_UNIT_COUNT_V1: usize = 3;
const EXPECTED_PASSWD_V1: &[u8] = b"root:x:0:0:root:/root:/bin/bash\n\
fe2o3-compiler:x:999:999:fe2o3 compiler-execution supervisor:/var/lib/fe2o3/compiler-execution:/usr/sbin/nologin\n\
fe2o3-anchor:x:998:998:fe2o3 external monotonic anchor:/var/lib/fe2o3/external-anchor:/usr/sbin/nologin\n";
const EXPECTED_GROUP_V1: &[u8] = b"root:x:0:\n\
fe2o3-compiler:x:999:\n\
fe2o3-anchor:x:998:\n";

const VERSION_ARGS_V1: &[&str] = &["/usr/bin/systemd-analyze", "--version"];
const SYSUSERS_ARGS_V1: &[&str] = &["/usr/bin/systemd-sysusers", "--no-pager"];
const TMPFILES_ARGS_V1: &[&str] = &["/usr/bin/systemd-tmpfiles", "--create", "--no-pager"];
const ANALYZE_ARGS_V1: &[&str] = &[
    "/usr/bin/systemd-analyze",
    "--offline=yes",
    "--man=no",
    "--generators=no",
    "--no-pager",
    "verify",
    "fe2o3-qualification.target",
    "fe2o3-compiler-execution.socket",
    "fe2o3-compiler-execution.service",
];

const PREFLIGHT_COMMANDS_V1: [SystemdPreflightCommandV1; 4] = [
    SystemdPreflightCommandV1 {
        stage: SystemdPreflightStageV1::Version,
        arguments: VERSION_ARGS_V1,
    },
    SystemdPreflightCommandV1 {
        stage: SystemdPreflightStageV1::Sysusers,
        arguments: SYSUSERS_ARGS_V1,
    },
    SystemdPreflightCommandV1 {
        stage: SystemdPreflightStageV1::Tmpfiles,
        arguments: TMPFILES_ARGS_V1,
    },
    SystemdPreflightCommandV1 {
        stage: SystemdPreflightStageV1::UnitVerify,
        arguments: ANALYZE_ARGS_V1,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SystemdPreflightStageV1 {
    Version,
    Sysusers,
    Tmpfiles,
    UnitVerify,
}

impl SystemdPreflightStageV1 {
    const fn canonical_name(self) -> &'static str {
        match self {
            Self::Version => "systemd-version",
            Self::Sysusers => "systemd-sysusers",
            Self::Tmpfiles => "systemd-tmpfiles",
            Self::UnitVerify => "systemd-analyze-verify",
        }
    }

    const fn complete_fault_point(self) -> QualificationFaultPointV1 {
        match self {
            Self::Version => QualificationFaultPointV1::SystemdVersionComplete,
            Self::Sysusers => QualificationFaultPointV1::SystemdSysusersComplete,
            Self::Tmpfiles => QualificationFaultPointV1::SystemdTmpfilesComplete,
            Self::UnitVerify => QualificationFaultPointV1::SystemdUnitVerifyComplete,
        }
    }

    const fn revalidated_fault_point(self) -> QualificationFaultPointV1 {
        match self {
            Self::Version => QualificationFaultPointV1::SystemdVersionRevalidated,
            Self::Sysusers => QualificationFaultPointV1::SystemdSysusersRevalidated,
            Self::Tmpfiles => QualificationFaultPointV1::SystemdTmpfilesRevalidated,
            Self::UnitVerify => QualificationFaultPointV1::SystemdUnitVerifyRevalidated,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SystemdPreflightCommandV1 {
    stage: SystemdPreflightStageV1,
    arguments: &'static [&'static str],
}

trait SystemdPreflightCommandRunnerV1 {
    fn run(
        &mut self,
        root: &OwnedFd,
        command: SystemdPreflightCommandV1,
    ) -> Result<Vec<u8>, DeploymentVerificationErrorV1>;
}

struct ProductionSystemdPreflightCommandRunnerV1;

impl SystemdPreflightCommandRunnerV1 for ProductionSystemdPreflightCommandRunnerV1 {
    fn run(
        &mut self,
        root: &OwnedFd,
        command: SystemdPreflightCommandV1,
    ) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
        let inherited_root = rustix::io::dup(root).map_err(|source| {
            io_error(
                "duplicate composed root for systemd preflight helper",
                source,
            )
        })?;
        let output = memfd_create("fe2o3-systemd-preflight-output-v1", MemfdFlags::CLOEXEC)
            .map_err(|source| io_error("create systemd preflight output", source))?;
        let child_output = rustix::io::dup(&output)
            .map_err(|source| io_error("duplicate systemd preflight output", source))?;
        let status = Command::new("/proc/self/exe")
            .arg(COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_TOOL_COMMAND_V1)
            .arg(command.stage.canonical_name())
            .env_clear()
            .env(
                COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_PARENT_PID_ENV_V1,
                std::process::id().to_string(),
            )
            .stdin(Stdio::from(inherited_root))
            .stdout(Stdio::from(child_output))
            .stderr(Stdio::null())
            .status()
            .map_err(|source| std_io_error("execute systemd preflight command", source))?;
        if !status.success() {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
                format!(
                    "{} failed with exit_code={:?} signal={:?}",
                    command.stage.canonical_name(),
                    status.code(),
                    status.signal()
                ),
            ));
        }
        read_bounded_tool_output(File::from(output))
    }
}

fn read_bounded_tool_output(file: File) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let byte_len = file
        .metadata()
        .map_err(|source| std_io_error("inspect systemd preflight output", source))?
        .len();
    if byte_len > SYSTEMD_TOOL_OUTPUT_MAX_BYTES_V1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd preflight output exceeds the fixed bound",
        ));
    }
    let mut bytes = vec![0_u8; usize::try_from(byte_len).expect("bounded output fits usize")];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|source| std_io_error("read systemd preflight output", source))?;
    if file
        .metadata()
        .map_err(|source| std_io_error("reinspect systemd preflight output", source))?
        .len()
        != byte_len
    {
        return Err(changed("systemd preflight output changed while reading"));
    }
    Ok(bytes)
}

/// Enters the composed root inherited on stdin and replaces this helper with one pinned tool.
///
/// This is a narrow hidden boundary for the static qualification image. The caller must first
/// bind this process to its exact parent with parent-death `SIGKILL`. The function requires root,
/// one task, an OverlayFS directory on stdin, and one canonical preflight stage. Success does not
/// return because the helper is replaced by the selected binary from the admitted base.
pub fn execute_compiler_execution_systemd_preflight_tool_v1(
    stage: &str,
) -> Result<std::convert::Infallible, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "systemd preflight helper requires effective UID 0",
        ));
    }
    if super::host::process_thread_count()? != 1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "systemd preflight helper requires one task",
        ));
    }
    let command = PREFLIGHT_COMMANDS_V1
        .iter()
        .find(|command| command.stage.canonical_name() == stage)
        .ok_or_else(|| {
            super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
                "systemd preflight helper stage is not canonical",
            )
        })?;
    let root = File::open(COMPOSED_ROOT_STDIN_PATH_V1)
        .map_err(|source| std_io_error("open inherited composed-root stdin", source))?;
    let null_stdin = File::open("/dev/null")
        .map_err(|source| std_io_error("open null input before entering composed root", source))?;
    let root_stat =
        fstat(&root).map_err(|source| io_error("inspect inherited composed-root stdin", source))?;
    let root_fs = rustix::fs::fstatfs(&root)
        .map_err(|source| io_error("inspect inherited composed-root filesystem", source))?;
    if FileType::from_raw_mode(root_stat.st_mode) != FileType::Directory
        || root_stat.st_mode & 0o7777 != 0o755
        || (root_stat.st_uid, root_stat.st_gid) != (0, 0)
        || root_fs.f_type != OVERLAYFS_MAGIC_V1
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd preflight helper stdin is not the exact composed OverlayFS root",
        ));
    }
    setrlimit(
        Resource::Fsize,
        Rlimit {
            current: Some(SYSTEMD_TOOL_OUTPUT_MAX_BYTES_V1),
            maximum: Some(SYSTEMD_TOOL_OUTPUT_MAX_BYTES_V1),
        },
    )
    .map_err(|source| io_error("bound systemd preflight output", source))?;
    rustix::process::chroot(COMPOSED_ROOT_STDIN_PATH_V1)
        .map_err(|source| io_error("enter composed root for systemd preflight", source))?;
    std::env::set_current_dir("/")
        .map_err(|source| std_io_error("enter composed-root working directory", source))?;
    let error = Command::new(command.arguments[0])
        .args(&command.arguments[1..])
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("SYSTEMD_COLORS", "0")
        .env("SYSTEMD_LOG_LEVEL", "warning")
        .env("SYSTEMD_PAGER", "cat")
        .stdin(Stdio::from(null_stdin))
        .exec();
    Err(std_io_error(
        "replace systemd preflight helper with pinned tool",
        error,
    ))
}

pub(super) struct CompilerExecutionSystemdPreflightV1 {
    mounted: MountedCompilerExecutionQualificationV1,
    systemd_version: String,
}

impl CompilerExecutionSystemdPreflightV1 {
    pub(super) fn systemd_version(&self) -> &str {
        &self.systemd_version
    }

    pub(super) const fn verified_unit_count(&self) -> usize {
        VERIFIED_SYSTEMD_UNIT_COUNT_V1
    }

    pub(super) const fn compiler_uid(&self) -> u32 {
        COMPILER_UID_V1
    }

    pub(super) const fn compiler_gid(&self) -> u32 {
        COMPILER_UID_V1
    }

    pub(super) const fn anchor_uid(&self) -> u32 {
        ANCHOR_UID_V1
    }

    pub(super) const fn anchor_gid(&self) -> u32 {
        ANCHOR_UID_V1
    }

    pub(super) fn git_commit(&self) -> &str {
        self.mounted.git_commit()
    }

    pub(super) fn manifest_sha256(&self) -> [u8; 32] {
        self.mounted.manifest_sha256()
    }

    pub(super) fn base_image_sha256(&self) -> [u8; 32] {
        self.mounted.base_image_sha256()
    }

    pub(super) fn inherit_systemd_machine_descriptors(
        &self,
    ) -> Result<(OwnedFd, OwnedFd), DeploymentVerificationErrorV1> {
        self.mounted.inherit_systemd_machine_descriptors()
    }

    pub(super) fn inherit_provisioning_root_descriptor(
        &self,
    ) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
        self.mounted.inherit_systemd_preflight_root_descriptor()
    }

    pub(super) fn revalidate_systemd_machine_state(
        &self,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        self.mounted.revalidate_systemd_preflight_state()
    }

    pub(super) fn cleanup_with_hooks(
        self,
        hooks: &mut impl QualificationFaultHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        self.mounted.cleanup_with_hooks(hooks)
    }
}

pub(super) fn run_compiler_execution_systemd_preflight_with_hooks_v1(
    mounted: MountedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<CompilerExecutionSystemdPreflightV1, DeploymentVerificationErrorV1> {
    match run_systemd_preflight_inner(&mounted, hooks) {
        Ok(systemd_version) => Ok(CompilerExecutionSystemdPreflightV1 {
            mounted,
            systemd_version,
        }),
        Err(error) => match mounted.cleanup_with_hooks(hooks) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("systemd preflight failed ({error}); cleanup also failed: {cleanup}"),
            )),
        },
    }
}

fn run_systemd_preflight_inner(
    mounted: &MountedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<String, DeploymentVerificationErrorV1> {
    let root = mounted.inherit_composed_root_descriptor()?;
    let systemd_version = execute_preflight_commands(
        &root,
        &mut ProductionSystemdPreflightCommandRunnerV1,
        || mounted.revalidate_systemd_preflight_state(),
        hooks,
    )?;
    validate_account_databases(
        &read_exact_root_file(&root, "etc/passwd", 0o644)?,
        &read_exact_root_file(&root, "etc/group", 0o644)?,
    )?;
    validate_tmpfiles_projection(&root)?;
    hooks.checkpoint(QualificationFaultPointV1::SystemdPostconditionsAdmitted)?;
    mounted.revalidate_systemd_preflight_state()?;
    hooks.checkpoint(QualificationFaultPointV1::InstalledLowerRevalidated)?;
    Ok(systemd_version)
}

fn execute_preflight_commands(
    root: &OwnedFd,
    runner: &mut impl SystemdPreflightCommandRunnerV1,
    mut revalidate: impl FnMut() -> Result<(), DeploymentVerificationErrorV1>,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<String, DeploymentVerificationErrorV1> {
    let mut systemd_version = None;
    for command in PREFLIGHT_COMMANDS_V1 {
        let output = runner.run(root, command)?;
        match command.stage {
            SystemdPreflightStageV1::Version => {
                systemd_version = Some(admit_systemd_version(&output)?);
            }
            _ if !output.is_empty() => {
                return Err(super::invalid(
                    DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
                    format!(
                        "{} emitted unexpected standard output",
                        command.stage.canonical_name()
                    ),
                ));
            }
            _ => {}
        }
        hooks.checkpoint(command.stage.complete_fault_point())?;
        revalidate()?;
        hooks.checkpoint(command.stage.revalidated_fault_point())?;
    }
    systemd_version.ok_or_else(|| {
        super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd version evidence is missing",
        )
    })
}

fn admit_systemd_version(output: &[u8]) -> Result<String, DeploymentVerificationErrorV1> {
    let output = std::str::from_utf8(output).map_err(|_| {
        super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd version output is not UTF-8",
        )
    })?;
    if !output.ends_with('\n') || output.lines().next() != Some(SYSTEMD_VERSION_LINE_V1) {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd version does not match the pinned qualification base",
        ));
    }
    Ok(SYSTEMD_VERSION_V1.to_owned())
}

fn read_exact_root_file(
    root: &OwnedFd,
    path: &str,
    expected_mode: u32,
) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let descriptor = open_root_object(root, path, false)?;
    let before = snapshot(
        &fstat(&descriptor)
            .map_err(|source| io_error("inspect systemd account database", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::RegularFile
        || before.mode & 0o7777 != expected_mode
        || (before.uid, before.gid) != (0, 0)
        || before.links != 1
        || before.byte_len > ACCOUNT_DATABASE_MAX_BYTES_V1
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            format!("composed-root {path} metadata is not canonical"),
        ));
    }
    require_no_xattrs(&descriptor, "systemd account database")?;
    let mut file = File::from(descriptor);
    let mut bytes = vec![0_u8; usize::try_from(before.byte_len).expect("bounded file fits usize")];
    file.read_exact(&mut bytes)
        .map_err(|source| std_io_error("read systemd account database", source))?;
    let after = snapshot(
        &fstat(&file).map_err(|source| io_error("reinspect systemd account database", source))?,
    );
    if before != after {
        return Err(changed("systemd account database changed while reading"));
    }
    Ok(bytes)
}

fn validate_account_databases(
    passwd: &[u8],
    group: &[u8],
) -> Result<(), DeploymentVerificationErrorV1> {
    if passwd != EXPECTED_PASSWD_V1 || group != EXPECTED_GROUP_V1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            "systemd-sysusers did not produce the exact V1 account databases",
        ));
    }
    Ok(())
}

fn validate_tmpfiles_projection(root: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
    for (path, mode, owner) in [
        ("run/fe2o3", 0o755, (0, 0)),
        ("var/lib/fe2o3", 0o755, (0, 0)),
        (
            "var/lib/fe2o3/compiler-execution",
            0o700,
            (COMPILER_UID_V1, COMPILER_UID_V1),
        ),
        (
            "var/lib/fe2o3/external-anchor",
            0o700,
            (ANCHOR_UID_V1, ANCHOR_UID_V1),
        ),
        ("etc/fe2o3", 0o755, (0, 0)),
        ("etc/fe2o3/compiler-execution", 0o755, (0, 0)),
    ] {
        validate_root_object(root, path, FileType::Directory, mode, owner, None)?;
    }
    validate_root_object(
        root,
        "var/lib/fe2o3/compiler-execution-lifecycle-v1",
        FileType::RegularFile,
        0o400,
        (0, 0),
        Some(0),
    )
}

fn validate_root_object(
    root: &OwnedFd,
    path: &str,
    expected_type: FileType,
    expected_mode: u32,
    expected_owner: (u32, u32),
    expected_size: Option<u64>,
) -> Result<(), DeploymentVerificationErrorV1> {
    let descriptor = open_root_object(root, path, expected_type == FileType::Directory)?;
    let observed = snapshot(
        &fstat(&descriptor)
            .map_err(|source| io_error("inspect systemd tmpfiles object", source))?,
    );
    if FileType::from_raw_mode(observed.mode) != expected_type
        || observed.mode & 0o7777 != expected_mode
        || (observed.uid, observed.gid) != expected_owner
        || observed.links == 0
        || expected_size.is_some_and(|size| observed.byte_len != size)
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            format!("systemd-tmpfiles object {path} is not canonical"),
        ));
    }
    require_no_xattrs(&descriptor, "systemd tmpfiles object")
}

fn open_root_object(
    root: &OwnedFd,
    path: &str,
    directory: bool,
) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    if directory {
        flags |= OFlags::DIRECTORY;
    }
    openat2(
        root,
        path,
        flags,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map_err(|source| io_error("open composed-root systemd object", source))
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd as _;

    use super::*;

    #[test]
    fn command_plan_is_exact_and_runs_one_revalidation_per_success() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = File::open(temporary.path()).unwrap();
        let root = rustix::io::dup(directory.as_fd()).unwrap();
        let mut runner = RecordingRunnerV1::default();
        let mut revalidations = 0;
        let mut hooks = RecordingFaultHooksV1::default();

        execute_preflight_commands(
            &root,
            &mut runner,
            || {
                revalidations += 1;
                Ok(())
            },
            &mut hooks,
        )
        .unwrap();

        assert_eq!(runner.observed, PREFLIGHT_COMMANDS_V1);
        assert_eq!(revalidations, PREFLIGHT_COMMANDS_V1.len());
        assert_eq!(
            hooks.observed,
            vec![
                QualificationFaultPointV1::SystemdVersionComplete,
                QualificationFaultPointV1::SystemdVersionRevalidated,
                QualificationFaultPointV1::SystemdSysusersComplete,
                QualificationFaultPointV1::SystemdSysusersRevalidated,
                QualificationFaultPointV1::SystemdTmpfilesComplete,
                QualificationFaultPointV1::SystemdTmpfilesRevalidated,
                QualificationFaultPointV1::SystemdUnitVerifyComplete,
                QualificationFaultPointV1::SystemdUnitVerifyRevalidated,
            ]
        );
        assert_eq!(ANALYZE_ARGS_V1.len(), 9);
        assert_eq!(VERIFIED_SYSTEMD_UNIT_COUNT_V1, 3);
    }

    #[test]
    fn failed_command_stops_before_revalidation_or_later_mutation() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = File::open(temporary.path()).unwrap();
        let root = rustix::io::dup(directory.as_fd()).unwrap();
        let mut runner = RecordingRunnerV1 {
            fail_at: Some(SystemdPreflightStageV1::Tmpfiles),
            ..RecordingRunnerV1::default()
        };
        let mut revalidations = 0;
        let mut hooks = RecordingFaultHooksV1::default();

        assert_eq!(
            execute_preflight_commands(
                &root,
                &mut runner,
                || {
                    revalidations += 1;
                    Ok(())
                },
                &mut hooks
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight
        );
        assert_eq!(runner.observed, PREFLIGHT_COMMANDS_V1[..=2].to_vec());
        assert_eq!(revalidations, 2);
        assert_eq!(hooks.observed.len(), 4);
    }

    #[test]
    fn injected_completion_stops_before_revalidation_and_later_commands() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = File::open(temporary.path()).unwrap();
        let root = rustix::io::dup(directory.as_fd()).unwrap();
        let mut runner = RecordingRunnerV1::default();
        let mut revalidations = 0;
        let mut hooks = crate::fault::InjectQualificationFaultV1::new(
            QualificationFaultPointV1::SystemdTmpfilesComplete,
        );

        assert_eq!(
            execute_preflight_commands(
                &root,
                &mut runner,
                || {
                    revalidations += 1;
                    Ok(())
                },
                &mut hooks
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InjectedFailure
        );
        assert_eq!(runner.observed, PREFLIGHT_COMMANDS_V1[..=2].to_vec());
        assert_eq!(revalidations, 2);
        assert!(hooks.fired());
    }

    #[test]
    fn injected_revalidation_stops_after_exact_revalidation() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = File::open(temporary.path()).unwrap();
        let root = rustix::io::dup(directory.as_fd()).unwrap();
        let mut runner = RecordingRunnerV1::default();
        let mut revalidations = 0;
        let mut hooks = crate::fault::InjectQualificationFaultV1::new(
            QualificationFaultPointV1::SystemdSysusersRevalidated,
        );

        assert_eq!(
            execute_preflight_commands(
                &root,
                &mut runner,
                || {
                    revalidations += 1;
                    Ok(())
                },
                &mut hooks
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InjectedFailure
        );
        assert_eq!(runner.observed, PREFLIGHT_COMMANDS_V1[..=1].to_vec());
        assert_eq!(revalidations, 2);
        assert!(hooks.fired());
    }

    #[test]
    fn account_database_admission_is_byte_exact() {
        validate_account_databases(EXPECTED_PASSWD_V1, EXPECTED_GROUP_V1).unwrap();
        let mut substituted = EXPECTED_PASSWD_V1.to_vec();
        substituted[0] = b'R';
        assert_eq!(
            validate_account_databases(&substituted, EXPECTED_GROUP_V1)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight
        );
    }

    #[test]
    fn version_admission_requires_the_exact_pinned_first_line() {
        assert_eq!(
            admit_systemd_version(b"systemd 255 (255.4-1ubuntu8.17)\n+PAM +AUDIT +SECCOMP\n")
                .unwrap(),
            SYSTEMD_VERSION_V1
        );
        assert_eq!(
            admit_systemd_version(b"systemd 255\n").unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight
        );
    }

    #[test]
    fn oversized_tool_output_is_rejected_before_allocation() {
        let output = memfd_create("oversized-preflight-output", MemfdFlags::CLOEXEC).unwrap();
        let file = File::from(output);
        file.set_len(SYSTEMD_TOOL_OUTPUT_MAX_BYTES_V1 + 1).unwrap();
        assert_eq!(
            read_bounded_tool_output(file).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight
        );
    }

    #[derive(Default)]
    struct RecordingRunnerV1 {
        observed: Vec<SystemdPreflightCommandV1>,
        fail_at: Option<SystemdPreflightStageV1>,
    }

    #[derive(Default)]
    struct RecordingFaultHooksV1 {
        observed: Vec<QualificationFaultPointV1>,
    }

    impl QualificationFaultHooksV1 for RecordingFaultHooksV1 {
        fn checkpoint(
            &mut self,
            point: QualificationFaultPointV1,
        ) -> Result<(), DeploymentVerificationErrorV1> {
            self.observed.push(point);
            Ok(())
        }
    }

    impl SystemdPreflightCommandRunnerV1 for RecordingRunnerV1 {
        fn run(
            &mut self,
            _root: &OwnedFd,
            command: SystemdPreflightCommandV1,
        ) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
            self.observed.push(command);
            if self.fail_at == Some(command.stage) {
                return Err(super::super::invalid(
                    DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
                    "injected systemd preflight command failure",
                ));
            }
            Ok(match command.stage {
                SystemdPreflightStageV1::Version => {
                    format!("{SYSTEMD_VERSION_LINE_V1}\n+PAM +AUDIT +SECCOMP\n").into_bytes()
                }
                _ => Vec::new(),
            })
        }
    }
}
