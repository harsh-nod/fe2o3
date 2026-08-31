use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::{Command, Stdio};

use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionSupervisorDeploymentV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
    sealed_static_issuer_runtime_measurement_v1,
};
use rustix::fs::{
    FileType, MemfdFlags, Mode, OFlags, ResolveFlags, fstat, fstatfs, memfd_create, openat2,
};
use rustix::process::{Resource, Rlimit, setrlimit};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;

use super::fault::QualificationFaultHooksV1;
use super::preflight::CompilerExecutionSystemdPreflightV1;
use super::{
    COMPILER_EXECUTION_PROVISIONING_PARENT_PID_ENV_V1,
    COMPILER_EXECUTION_PROVISIONING_TOOL_COMMAND_V1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, QualificationFaultPointV1, changed, invalid, io_error,
    require_no_xattrs, snapshot, std_io_error, verify_directory_children,
};

const COMPOSED_ROOT_STDIN_PATH_V1: &str = "/proc/self/fd/0";
const OVERLAYFS_MAGIC_V1: i64 = 0x794c_7630;
const PROVISIONER_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-provision";
const POLICY_GENERATION_V1: u64 = 1;
const PROVISIONING_OUTPUT_MAX_BYTES_V1: u64 = 64 * 1024;
const CONFIG_DIRECTORY_V1: &str = "etc/fe2o3/compiler-execution";
const ISSUER_POLICY_FILE_V1: &str = "issuer-policy-v1";
const SUPERVISOR_DEPLOYMENT_FILE_V1: &str = "supervisor-deployment-v1";
const ANCHOR_DEPLOYMENT_FILE_V1: &str = "anchor-deployment-v1";
const ANCHOR_PROVISIONING_FILE_V1: &str = "anchor-provisioning-v1";
const ISSUER_SEED_FILE_V1: &str = "issuer-signing-key-seed-v1";
const ANCHOR_SEED_FILE_V1: &str = "anchor-signing-key-seed-v1";
const CONFIG_FILES_V1: &[&str] = &[
    ANCHOR_DEPLOYMENT_FILE_V1,
    ANCHOR_PROVISIONING_FILE_V1,
    ANCHOR_SEED_FILE_V1,
    ISSUER_POLICY_FILE_V1,
    ISSUER_SEED_FILE_V1,
    SUPERVISOR_DEPLOYMENT_FILE_V1,
];
const SUPERVISOR_PATH_V1: &str = "usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor";
const LAUNCHER_PATH_V1: &str = "usr/libexec/fe2o3/fe2o3-static-preexec-launcher";
const ISSUER_PATH_V1: &str = "usr/libexec/fe2o3/fe2o3-compiler-execution-issuer";
const ANCHOR_HELPER_PATH_V1: &str = "usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper";
const ANCHOR_DAEMON_PATH_V1: &str = "usr/libexec/fe2o3/fe2o3-external-anchor-service";
const COMPILER_UID_V1: u32 = 999;
const ANCHOR_UID_V1: u32 = 998;
const ROOT_OWNER_V1: (u32, u32) = (0, 0);
const CONFIG_DIRECTORY_MODE_V1: u32 = 0o755;
const PUBLIC_RECORD_MODE_V1: u32 = 0o444;
const SECRET_SEED_MODE_V1: u32 = 0o400;
const EXECUTABLE_MODE_V1: u32 = 0o555;
const KEY_SEED_BYTES_V1: usize = 32;
const HASH_BUFFER_BYTES_V1: usize = 64 * 1024;

pub(super) struct CompilerExecutionProvisionedQualificationV1 {
    preflight: CompilerExecutionSystemdPreflightV1,
    policy_generation: u64,
}

impl CompilerExecutionProvisionedQualificationV1 {
    pub(super) const fn policy_generation(&self) -> u64 {
        self.policy_generation
    }

    pub(super) fn systemd_version(&self) -> &str {
        self.preflight.systemd_version()
    }

    pub(super) const fn verified_unit_count(&self) -> usize {
        self.preflight.verified_unit_count()
    }

    pub(super) const fn compiler_uid(&self) -> u32 {
        self.preflight.compiler_uid()
    }

    pub(super) const fn compiler_gid(&self) -> u32 {
        self.preflight.compiler_gid()
    }

    pub(super) const fn anchor_uid(&self) -> u32 {
        self.preflight.anchor_uid()
    }

    pub(super) const fn anchor_gid(&self) -> u32 {
        self.preflight.anchor_gid()
    }

    pub(super) fn git_commit(&self) -> &str {
        self.preflight.git_commit()
    }

    pub(super) fn manifest_sha256(&self) -> [u8; 32] {
        self.preflight.manifest_sha256()
    }

    pub(super) fn base_image_sha256(&self) -> [u8; 32] {
        self.preflight.base_image_sha256()
    }

    pub(super) fn inherit_systemd_machine_descriptors(
        &self,
    ) -> Result<(OwnedFd, OwnedFd), DeploymentVerificationErrorV1> {
        let (base, root) = self.preflight.inherit_systemd_machine_descriptors()?;
        self.require_current_provisioned_state(&root)?;
        Ok((base, root))
    }

    pub(super) fn revalidate_systemd_machine_state(
        &self,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        let root = self.preflight.inherit_provisioning_root_descriptor()?;
        self.require_current_provisioned_state(&root)
    }

    pub(super) fn cleanup_with_hooks(
        self,
        hooks: &mut impl QualificationFaultHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        self.preflight.cleanup_with_hooks(hooks)
    }

    fn require_current_provisioned_state(
        &self,
        root: &OwnedFd,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        let generation =
            admit_provisioned_state(root, ROOT_OWNER_V1, COMPILER_UID_V1, ANCHOR_UID_V1)?;
        if generation != self.policy_generation {
            return Err(provisioning_invalid(
                "compiler-execution policy generation changed after provisioning",
            ));
        }
        Ok(())
    }
}

pub(super) fn run_compiler_execution_provisioning_with_hooks_v1(
    preflight: CompilerExecutionSystemdPreflightV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<CompilerExecutionProvisionedQualificationV1, DeploymentVerificationErrorV1> {
    match run_provisioning_inner(&preflight, hooks) {
        Ok(policy_generation) => Ok(CompilerExecutionProvisionedQualificationV1 {
            preflight,
            policy_generation,
        }),
        Err(error) => match preflight.cleanup_with_hooks(hooks) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!(
                    "compiler-execution provisioning failed ({error}); cleanup also failed: {cleanup}"
                ),
            )),
        },
    }
}

fn run_provisioning_inner(
    preflight: &CompilerExecutionSystemdPreflightV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<u64, DeploymentVerificationErrorV1> {
    let root = preflight.inherit_provisioning_root_descriptor()?;
    run_production_provisioner(&root)?;
    hooks.checkpoint(QualificationFaultPointV1::CompilerExecutionProvisioningComplete)?;
    preflight.revalidate_systemd_machine_state()?;
    hooks.checkpoint(QualificationFaultPointV1::CompilerExecutionProvisioningRevalidated)?;
    let generation = admit_provisioned_state(&root, ROOT_OWNER_V1, COMPILER_UID_V1, ANCHOR_UID_V1)?;
    preflight.revalidate_systemd_machine_state()?;
    hooks.checkpoint(QualificationFaultPointV1::CompilerExecutionProvisioningAdmitted)?;
    Ok(generation)
}

fn run_production_provisioner(root: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
    let inherited_root = rustix::io::dup(root)
        .map_err(|source| io_error("duplicate composed root for provisioning helper", source))?;
    let stdout = memfd_create("fe2o3-provisioning-stdout-v1", MemfdFlags::CLOEXEC)
        .map_err(|source| io_error("create provisioning stdout", source))?;
    let stderr = memfd_create("fe2o3-provisioning-stderr-v1", MemfdFlags::CLOEXEC)
        .map_err(|source| io_error("create provisioning stderr", source))?;
    let child_stdout = rustix::io::dup(&stdout)
        .map_err(|source| io_error("duplicate provisioning stdout", source))?;
    let child_stderr = rustix::io::dup(&stderr)
        .map_err(|source| io_error("duplicate provisioning stderr", source))?;
    let status = Command::new("/proc/self/exe")
        .arg(COMPILER_EXECUTION_PROVISIONING_TOOL_COMMAND_V1)
        .env_clear()
        .env(
            COMPILER_EXECUTION_PROVISIONING_PARENT_PID_ENV_V1,
            std::process::id().to_string(),
        )
        .stdin(Stdio::from(inherited_root))
        .stdout(Stdio::from(child_stdout))
        .stderr(Stdio::from(child_stderr))
        .status()
        .map_err(|source| std_io_error("execute compiler-execution provisioner", source))?;
    if !status.success() {
        return Err(provisioning_invalid(format!(
            "production provisioner failed with exit_code={:?} signal={:?}",
            status.code(),
            status.signal()
        )));
    }
    require_empty_bounded_output(File::from(stdout), "standard output")?;
    require_empty_bounded_output(File::from(stderr), "standard error")
}

fn require_empty_bounded_output(
    file: File,
    stream: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let byte_len = file
        .metadata()
        .map_err(|source| std_io_error("inspect provisioning output", source))?
        .len();
    if byte_len > PROVISIONING_OUTPUT_MAX_BYTES_V1 {
        return Err(provisioning_invalid(format!(
            "production provisioner {stream} exceeds the fixed bound"
        )));
    }
    if byte_len != 0 {
        return Err(provisioning_invalid(format!(
            "production provisioner emitted unexpected {stream}"
        )));
    }
    Ok(())
}

/// Enters the inherited composed root and replaces this helper with the production provisioner.
///
/// The qualification binary binds this hidden one-task root helper to its exact parent before
/// entry. Success does not return because the helper is replaced by the admitted static image.
pub fn execute_compiler_execution_provisioning_tool_v1()
-> Result<std::convert::Infallible, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "compiler-execution provisioning helper requires effective UID 0",
        ));
    }
    if super::host::process_thread_count()? != 1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "compiler-execution provisioning helper requires one task",
        ));
    }
    let root = File::open(COMPOSED_ROOT_STDIN_PATH_V1)
        .map_err(|source| std_io_error("open inherited provisioning root", source))?;
    let null_stdin = File::open("/dev/null")
        .map_err(|source| std_io_error("open null input before provisioning chroot", source))?;
    validate_composed_root(&root)?;
    setrlimit(
        Resource::Fsize,
        Rlimit {
            current: Some(PROVISIONING_OUTPUT_MAX_BYTES_V1),
            maximum: Some(PROVISIONING_OUTPUT_MAX_BYTES_V1),
        },
    )
    .map_err(|source| io_error("bound provisioning file output", source))?;
    rustix::process::chroot(COMPOSED_ROOT_STDIN_PATH_V1)
        .map_err(|source| io_error("enter composed root for provisioning", source))?;
    std::env::set_current_dir("/")
        .map_err(|source| std_io_error("enter provisioning root working directory", source))?;
    let error = Command::new(PROVISIONER_PATH_V1)
        .arg(POLICY_GENERATION_V1.to_string())
        .env_clear()
        .stdin(Stdio::from(null_stdin))
        .exec();
    Err(std_io_error(
        "replace provisioning helper with production provisioner",
        error,
    ))
}

fn validate_composed_root(root: &File) -> Result<(), DeploymentVerificationErrorV1> {
    let stat =
        fstat(root).map_err(|source| io_error("inspect inherited provisioning root", source))?;
    let filesystem = fstatfs(root)
        .map_err(|source| io_error("inspect inherited provisioning filesystem", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != 0o755
        || (stat.st_uid, stat.st_gid) != ROOT_OWNER_V1
        || filesystem.f_type != OVERLAYFS_MAGIC_V1
    {
        return Err(provisioning_invalid(
            "provisioning helper did not inherit the exact composed OverlayFS root",
        ));
    }
    Ok(())
}

fn admit_provisioned_state(
    root: &OwnedFd,
    root_owner: (u32, u32),
    compiler_uid: u32,
    anchor_uid: u32,
) -> Result<u64, DeploymentVerificationErrorV1> {
    let config = open_root_object(root, CONFIG_DIRECTORY_V1, true)?;
    let config_before = snapshot(&fstat(&config).map_err(|source| {
        io_error("inspect compiler-execution configuration directory", source)
    })?);
    if FileType::from_raw_mode(config_before.mode) != FileType::Directory
        || config_before.mode & 0o7777 != CONFIG_DIRECTORY_MODE_V1
        || (config_before.uid, config_before.gid) != root_owner
        || config_before.links == 0
    {
        return Err(provisioning_invalid(
            "compiler-execution configuration directory is not canonical",
        ));
    }
    require_no_xattrs(&config, "compiler-execution configuration directory")?;
    let config = File::from(config);
    verify_directory_children(
        &config,
        CONFIG_FILES_V1,
        "compiler-execution configuration directory",
    )?;

    let policy = CompilerExecutionIssuerPolicyV1::decode(&read_fixed_file::<
        COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    >(
        &config,
        ISSUER_POLICY_FILE_V1,
        PUBLIC_RECORD_MODE_V1,
        root_owner,
    )?)
    .map_err(|error| provisioning_invalid(format!("issuer policy is not canonical: {error}")))?;
    let supervisor = CompilerExecutionSupervisorDeploymentV1::decode(&read_fixed_file::<
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    >(
        &config,
        SUPERVISOR_DEPLOYMENT_FILE_V1,
        PUBLIC_RECORD_MODE_V1,
        root_owner,
    )?)
    .map_err(|error| {
        provisioning_invalid(format!("supervisor deployment is not canonical: {error}"))
    })?;
    let anchor = CompilerExecutionExternalAnchorDeploymentV1::decode(&read_fixed_file::<
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    >(
        &config,
        ANCHOR_DEPLOYMENT_FILE_V1,
        PUBLIC_RECORD_MODE_V1,
        root_owner,
    )?)
    .map_err(|error| {
        provisioning_invalid(format!(
            "external-anchor deployment is not canonical: {error}"
        ))
    })?;
    let anchor_provisioning =
        CompilerExecutionExternalAnchorProvisioningV1::decode(&read_fixed_file::<
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
        >(
            &config,
            ANCHOR_PROVISIONING_FILE_V1,
            PUBLIC_RECORD_MODE_V1,
            root_owner,
        )?)
        .map_err(|error| {
            provisioning_invalid(format!(
                "external-anchor provisioning is not canonical: {error}"
            ))
        })?;

    let anchor_service = supervisor.external_anchor_service();
    if policy.generation() != POLICY_GENERATION_V1
        || policy.runtime() != sealed_static_issuer_runtime_measurement_v1()
        || supervisor.service_uid() != compiler_uid
        || supervisor.service_gid() != compiler_uid
        || anchor_service.uid() != anchor_uid
        || anchor_service.gid() != anchor_uid
        || !supervisor.matches_policy(&policy)
        || !anchor.matches_supervisor_and_policy(&supervisor, &policy)
        || !anchor_provisioning.matches_deployment(&anchor)
    {
        return Err(provisioning_invalid(
            "provisioned record graph does not bind the exact generation, runtime, or service identities",
        ));
    }

    let supervisor_measurement = measure_static_image(
        root,
        SUPERVISOR_PATH_V1,
        "protected supervisor",
        MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
        root_owner,
    )?;
    let launcher_measurement = measure_static_image(
        root,
        LAUNCHER_PATH_V1,
        "static pre-exec launcher",
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
        root_owner,
    )?;
    let issuer_measurement = measure_static_image(
        root,
        ISSUER_PATH_V1,
        "compiler-execution issuer",
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
        root_owner,
    )?;
    let anchor_helper_measurement = measure_static_image(
        root,
        ANCHOR_HELPER_PATH_V1,
        "external-anchor provisioning helper",
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        root_owner,
    )?;
    let anchor_daemon_measurement = measure_static_image(
        root,
        ANCHOR_DAEMON_PATH_V1,
        "external-anchor daemon",
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        root_owner,
    )?;
    if supervisor.executable() != supervisor_measurement
        || supervisor.launcher() != launcher_measurement
        || policy.executable() != issuer_measurement
        || anchor.executable() != anchor_daemon_measurement
        || anchor_provisioning.helper() != anchor_helper_measurement
    {
        return Err(provisioning_invalid(
            "provisioned records do not measure the exact installed static images",
        ));
    }

    let issuer_seed = SecretSeedV1(read_fixed_file::<KEY_SEED_BYTES_V1>(
        &config,
        ISSUER_SEED_FILE_V1,
        SECRET_SEED_MODE_V1,
        root_owner,
    )?);
    let anchor_seed = SecretSeedV1(read_fixed_file::<KEY_SEED_BYTES_V1>(
        &config,
        ANCHOR_SEED_FILE_V1,
        SECRET_SEED_MODE_V1,
        root_owner,
    )?);
    let issuer_key = SigningKey::from_bytes(issuer_seed.as_bytes())
        .verifying_key()
        .to_bytes();
    let anchor_key = SigningKey::from_bytes(anchor_seed.as_bytes())
        .verifying_key()
        .to_bytes();
    if issuer_key != *policy.verifying_key()
        || anchor_key != *policy.external_anchor_verifying_key()
    {
        return Err(provisioning_invalid(
            "provisioned key seeds do not derive the policy verifying keys",
        ));
    }
    revalidate_config_directory(root, &config, config_before)?;
    Ok(policy.generation())
}

#[cfg(test)]
fn config_path(name: &str) -> String {
    format!("{CONFIG_DIRECTORY_V1}/{name}")
}

fn read_fixed_file<const N: usize>(
    directory: &File,
    path: &str,
    expected_mode: u32,
    expected_owner: (u32, u32),
) -> Result<[u8; N], DeploymentVerificationErrorV1> {
    let descriptor = open_root_object(directory.as_fd(), path, false)?;
    let before = snapshot(
        &fstat(&descriptor).map_err(|source| io_error("inspect provisioned file", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::RegularFile
        || before.mode & 0o7777 != expected_mode
        || (before.uid, before.gid) != expected_owner
        || before.links != 1
        || before.byte_len != N as u64
    {
        return Err(provisioning_invalid(format!(
            "provisioned file {path} metadata is not canonical"
        )));
    }
    require_no_xattrs(&descriptor, "provisioned compiler-execution file")?;
    let mut file = File::from(descriptor);
    let mut bytes = [0_u8; N];
    file.read_exact(&mut bytes)
        .map_err(|source| std_io_error("read provisioned compiler-execution file", source))?;
    let after =
        snapshot(&fstat(&file).map_err(|source| io_error("reinspect provisioned file", source))?);
    if before != after {
        bytes.zeroize();
        return Err(changed(format!(
            "provisioned file {path} changed while reading"
        )));
    }
    Ok(bytes)
}

fn measure_static_image(
    root: &OwnedFd,
    path: &str,
    role: &'static str,
    max_bytes: u64,
    expected_owner: (u32, u32),
) -> Result<CompilerExecutionIssuerMeasurementV1, DeploymentVerificationErrorV1> {
    let descriptor = open_root_object(root, path, false)?;
    let before = snapshot(
        &fstat(&descriptor)
            .map_err(|source| io_error("inspect provisioned static image", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::RegularFile
        || before.mode & 0o7777 != EXECUTABLE_MODE_V1
        || (before.uid, before.gid) != expected_owner
        || before.links != 1
        || before.byte_len == 0
        || before.byte_len > max_bytes
    {
        return Err(provisioning_invalid(format!(
            "provisioned {role} metadata is not canonical"
        )));
    }
    require_no_xattrs(&descriptor, "provisioned static image")?;
    let mut file = File::from(descriptor);
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES_V1];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| std_io_error("measure provisioned static image", source))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).expect("bounded read length fits u64"))
            .ok_or_else(|| provisioning_invalid("static image length overflowed"))?;
        if total > before.byte_len {
            return Err(changed(format!("provisioned {role} grew while measuring")));
        }
        digest.update(&buffer[..count]);
    }
    let after = snapshot(
        &fstat(&file).map_err(|source| io_error("reinspect provisioned static image", source))?,
    );
    if before != after || total != before.byte_len {
        return Err(changed(format!(
            "provisioned {role} changed while measuring"
        )));
    }
    CompilerExecutionIssuerMeasurementV1::new(digest.finalize().into(), total)
        .map_err(|_| provisioning_invalid(format!("provisioned {role} measurement is invalid")))
}

fn revalidate_config_directory(
    root: &OwnedFd,
    retained: &File,
    expected: super::ObjectSnapshotV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let retained_after = snapshot(&fstat(retained).map_err(|source| {
        io_error(
            "reinspect retained compiler-execution configuration directory",
            source,
        )
    })?);
    if retained_after != expected {
        return Err(changed(
            "compiler-execution configuration directory changed during admission",
        ));
    }
    require_no_xattrs(retained, "compiler-execution configuration directory")?;
    verify_directory_children(
        retained,
        CONFIG_FILES_V1,
        "compiler-execution configuration directory",
    )?;

    let reopened = open_root_object(root.as_fd(), CONFIG_DIRECTORY_V1, true)?;
    let reopened_snapshot = snapshot(&fstat(&reopened).map_err(|source| {
        io_error(
            "reinspect canonical compiler-execution configuration directory",
            source,
        )
    })?);
    if reopened_snapshot != expected {
        return Err(changed(
            "compiler-execution configuration directory pathname changed during admission",
        ));
    }
    require_no_xattrs(&reopened, "compiler-execution configuration directory")?;
    verify_directory_children(
        &File::from(reopened),
        CONFIG_FILES_V1,
        "compiler-execution configuration directory",
    )
}

fn open_root_object(
    root: impl std::os::fd::AsFd,
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
    .map_err(|source| io_error("open composed-root provisioning object", source))
}

fn provisioning_invalid(message: impl Into<String>) -> DeploymentVerificationErrorV1 {
    invalid(
        DeploymentVerificationErrorKindV1::InvalidQualificationProvisioning,
        message,
    )
}

struct SecretSeedV1([u8; KEY_SEED_BYTES_V1]);

impl SecretSeedV1 {
    fn as_bytes(&self) -> &[u8; KEY_SEED_BYTES_V1] {
        &self.0
    }
}

impl Drop for SecretSeedV1 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1;

    use super::*;

    #[test]
    fn exact_provisioned_state_binds_generation_keys_records_identities_and_images() {
        let fixture = provisioned_fixture();
        assert_eq!(
            admit_provisioned_state(&fixture.root, owner(), COMPILER_UID_V1, ANCHOR_UID_V1)
                .unwrap(),
            POLICY_GENERATION_V1
        );
    }

    #[test]
    fn provisioned_state_rejects_record_seed_image_and_inventory_substitution() {
        for mutation in [
            FixtureMutationV1::Policy,
            FixtureMutationV1::IssuerSeed,
            FixtureMutationV1::SupervisorImage,
            FixtureMutationV1::ExtraInventory,
        ] {
            let fixture = provisioned_fixture();
            mutation.apply(&fixture);
            assert!(
                admit_provisioned_state(&fixture.root, owner(), COMPILER_UID_V1, ANCHOR_UID_V1)
                    .is_err(),
                "mutation {mutation:?} was admitted"
            );
        }
    }

    #[test]
    fn retained_configuration_directory_must_remain_at_the_canonical_path() {
        let fixture = provisioned_fixture();
        let retained =
            File::from(open_root_object(&fixture.root, CONFIG_DIRECTORY_V1, true).unwrap());
        let canonical = fixture.path(CONFIG_DIRECTORY_V1);
        let displaced = fixture.path("etc/fe2o3/compiler-execution-displaced");
        fs::rename(&canonical, &displaced).unwrap();
        let expected = snapshot(&fstat(&retained).unwrap());
        fs::create_dir(&canonical).unwrap();
        fs::set_permissions(
            &canonical,
            fs::Permissions::from_mode(CONFIG_DIRECTORY_MODE_V1),
        )
        .unwrap();

        assert!(revalidate_config_directory(&fixture.root, &retained, expected).is_err());
    }

    #[test]
    fn successful_provisioner_output_must_be_silent() {
        let empty = tempfile::tempfile().unwrap();
        require_empty_bounded_output(empty, "standard output").unwrap();
        let mut nonempty = tempfile::tempfile().unwrap();
        nonempty.write_all(b"unexpected").unwrap();
        assert_eq!(
            require_empty_bounded_output(nonempty, "standard error")
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationProvisioning
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum FixtureMutationV1 {
        Policy,
        IssuerSeed,
        SupervisorImage,
        ExtraInventory,
    }

    impl FixtureMutationV1 {
        fn apply(self, fixture: &ProvisionedFixtureV1) {
            match self {
                Self::Policy => flip_first_byte(
                    &fixture.path(&config_path(ISSUER_POLICY_FILE_V1)),
                    PUBLIC_RECORD_MODE_V1,
                ),
                Self::IssuerSeed => flip_first_byte(
                    &fixture.path(&config_path(ISSUER_SEED_FILE_V1)),
                    SECRET_SEED_MODE_V1,
                ),
                Self::SupervisorImage => {
                    flip_first_byte(&fixture.path(SUPERVISOR_PATH_V1), EXECUTABLE_MODE_V1)
                }
                Self::ExtraInventory => write_file(
                    &fixture.path(&config_path("unexpected")),
                    b"unexpected",
                    PUBLIC_RECORD_MODE_V1,
                ),
            }
        }
    }

    struct ProvisionedFixtureV1 {
        _temporary: tempfile::TempDir,
        root: OwnedFd,
    }

    impl ProvisionedFixtureV1 {
        fn path(&self, relative: &str) -> std::path::PathBuf {
            self._temporary.path().join(relative)
        }
    }

    fn provisioned_fixture() -> ProvisionedFixtureV1 {
        let temporary = tempfile::tempdir().unwrap();
        let config = temporary.path().join(CONFIG_DIRECTORY_V1);
        let images = temporary.path().join("usr/libexec/fe2o3");
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&images).unwrap();
        fs::set_permissions(
            &config,
            fs::Permissions::from_mode(CONFIG_DIRECTORY_MODE_V1),
        )
        .unwrap();

        for (path, bytes) in [
            (SUPERVISOR_PATH_V1, b"supervisor-v1".as_slice()),
            (LAUNCHER_PATH_V1, b"launcher-v1".as_slice()),
            (ISSUER_PATH_V1, b"issuer-v1".as_slice()),
            (ANCHOR_HELPER_PATH_V1, b"anchor-helper-v1".as_slice()),
            (ANCHOR_DAEMON_PATH_V1, b"anchor-daemon-v1".as_slice()),
        ] {
            write_file(&temporary.path().join(path), bytes, EXECUTABLE_MODE_V1);
        }

        let root: OwnedFd = File::open(temporary.path()).unwrap().into();
        let root_owner = owner();
        let supervisor_measurement = measure_static_image(
            &root,
            SUPERVISOR_PATH_V1,
            "protected supervisor",
            MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
            root_owner,
        )
        .unwrap();
        let launcher_measurement = measure_static_image(
            &root,
            LAUNCHER_PATH_V1,
            "static pre-exec launcher",
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
            root_owner,
        )
        .unwrap();
        let issuer_measurement = measure_static_image(
            &root,
            ISSUER_PATH_V1,
            "compiler-execution issuer",
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
            root_owner,
        )
        .unwrap();
        let anchor_helper_measurement = measure_static_image(
            &root,
            ANCHOR_HELPER_PATH_V1,
            "external-anchor provisioning helper",
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
            root_owner,
        )
        .unwrap();
        let anchor_daemon_measurement = measure_static_image(
            &root,
            ANCHOR_DAEMON_PATH_V1,
            "external-anchor daemon",
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
            root_owner,
        )
        .unwrap();

        let issuer_seed = [0x31; KEY_SEED_BYTES_V1];
        let anchor_seed = [0x52; KEY_SEED_BYTES_V1];
        let policy = CompilerExecutionIssuerPolicyV1::new(
            POLICY_GENERATION_V1,
            issuer_measurement,
            sealed_static_issuer_runtime_measurement_v1(),
            SigningKey::from_bytes(&issuer_seed)
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&anchor_seed)
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            COMPILER_UID_V1,
            COMPILER_UID_V1,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(ANCHOR_UID_V1, ANCHOR_UID_V1)
                .unwrap(),
            supervisor_measurement,
            launcher_measurement,
            &policy,
        )
        .unwrap();
        let anchor = CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            anchor_daemon_measurement,
        )
        .unwrap();
        let anchor_provisioning =
            CompilerExecutionExternalAnchorProvisioningV1::new(&anchor, anchor_helper_measurement)
                .unwrap();

        for (name, bytes, mode) in [
            (
                ISSUER_POLICY_FILE_V1,
                policy.canonical_bytes().as_slice(),
                PUBLIC_RECORD_MODE_V1,
            ),
            (
                SUPERVISOR_DEPLOYMENT_FILE_V1,
                supervisor.canonical_bytes().as_slice(),
                PUBLIC_RECORD_MODE_V1,
            ),
            (
                ANCHOR_DEPLOYMENT_FILE_V1,
                anchor.canonical_bytes().as_slice(),
                PUBLIC_RECORD_MODE_V1,
            ),
            (
                ANCHOR_PROVISIONING_FILE_V1,
                anchor_provisioning.canonical_bytes().as_slice(),
                PUBLIC_RECORD_MODE_V1,
            ),
            (
                ISSUER_SEED_FILE_V1,
                issuer_seed.as_slice(),
                SECRET_SEED_MODE_V1,
            ),
            (
                ANCHOR_SEED_FILE_V1,
                anchor_seed.as_slice(),
                SECRET_SEED_MODE_V1,
            ),
        ] {
            write_file(&config.join(name), bytes, mode);
        }

        ProvisionedFixtureV1 {
            _temporary: temporary,
            root,
        }
    }

    fn write_file(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn flip_first_byte(path: &Path, final_mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = fs::read(path).unwrap();
        bytes[0] ^= 0x80;
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(final_mode)).unwrap();
    }

    fn owner() -> (u32, u32) {
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
    }
}
