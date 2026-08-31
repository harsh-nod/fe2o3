use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileExt;
use std::time::Duration;

use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1, CompilerExecutionPolicyCapabilityV1,
    CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSupervisorDeploymentCapabilityV1,
};
use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
};
use fe2o3_compiler_execution_supervisor::{
    IssuerServiceCredentialProfileV1, ProvisionedProtectedIssuerServiceInputsV1,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV1;
use fe2o3_protected_service_spawn::require_exact_root_identity_v1;
use rustix::fs::{FileType, OFlags};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::lifecycle::CompilerExecutionLifecycleLeaseV1;
use crate::{
    CompilerExecutionCoordinatorErrorV1, CompilerExecutionSupervisorProgramSourcesV1,
    CompilerExecutionSupervisorTrustV1, PreparedCompilerExecutionSupervisorV1,
    RootManagedCompilerExecutionServiceV1,
};

/// System-manager-owned production listener.
pub const COMPILER_EXECUTION_COORDINATOR_LISTENER_FD_V1: RawFd = 3;
/// Existing protected-supervisor service-owned mode-0700 state root.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1: RawFd = 4;
/// Existing external-anchor service-owned mode-0700 state root.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1: RawFd = 5;
/// Root-provisioned static protected-supervisor image.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1: RawFd = 6;
/// Root-provisioned static issuer pre-exec launcher image.
pub const COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1: RawFd = 7;
/// Root-provisioned static compiler-execution issuer image.
pub const COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1: RawFd = 8;
/// Root-provisioned static external-anchor helper image.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1: RawFd = 9;
/// Root-provisioned static external-anchor daemon image.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1: RawFd = 10;
/// Canonical protected-supervisor deployment record.
pub const COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1: RawFd = 11;
/// Canonical compiler-execution issuer policy record.
pub const COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1: RawFd = 12;
/// Canonical external-anchor deployment record.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1: RawFd = 13;
/// Canonical external-anchor provisioning record.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1: RawFd = 14;
/// Root-owned raw issuer signing-key seed.
pub const COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1: RawFd = 15;
/// Root-owned raw external-anchor signing-key seed.
pub const COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1: RawFd = 16;

const ROOT_ID_V1: u32 = 0;
const EXECUTABLE_MODE_V1: u32 = 0o555;
const PUBLIC_RECORD_MODE_V1: u32 = 0o444;
const SECRET_SEED_MODE_V1: u32 = 0o400;
const KEY_SEED_BYTES_V1: usize = 32;

const DESCRIPTORS_V1: [RawFd; 14] = [
    COMPILER_EXECUTION_COORDINATOR_LISTENER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1,
    COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1,
];

/// Move-only root-admitted inputs for the complete anchor plus supervisor deployment.
///
/// This is the only public composition that consumes the fixed production descriptor set. It
/// exposes no descriptor, key, signing operation, compiler authority, or partial service launch.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::InheritedCompilerExecutionDeploymentV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<InheritedCompilerExecutionDeploymentV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_coordinator::InheritedCompilerExecutionDeploymentV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<InheritedCompilerExecutionDeploymentV1>();
/// ```
pub struct InheritedCompilerExecutionDeploymentV1 {
    programs: CompilerExecutionSupervisorProgramSourcesV1,
    trust: CompilerExecutionSupervisorTrustV1,
    service_inputs: ProvisionedProtectedIssuerServiceInputsV1,
    anchor: PreparedExternalAnchorOccurrenceV1,
    supervisor_lifecycle: CompilerExecutionServiceLifecycleLeaseV1,
    lifecycle: CompilerExecutionLifecycleLeaseV1,
}

impl std::fmt::Debug for InheritedCompilerExecutionDeploymentV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InheritedCompilerExecutionDeploymentV1")
            .field("authority", &"complete-root-deployment-only")
            .finish_non_exhaustive()
    }
}

impl InheritedCompilerExecutionDeploymentV1 {
    /// Takes and admits the exact inherited production descriptor set under UID/GID 0.
    pub fn admit() -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        require_exact_root_identity_v1().map_err(CompilerExecutionCoordinatorErrorV1::Spawn)?;
        require_single_threaded_entrypoint()?;
        let [
            listener,
            supervisor_root,
            anchor_root,
            supervisor,
            launcher,
            issuer,
            anchor_helper,
            anchor_daemon,
            supervisor_deployment,
            policy,
            anchor_deployment,
            anchor_provisioning,
            issuer_key_seed,
            anchor_key_seed,
        ] = take_inherited_descriptors()?;

        let lifecycle =
            CompilerExecutionLifecycleLeaseV1::admit_service_from_root(&supervisor_root)?;
        let supervisor_lifecycle = CompilerExecutionServiceLifecycleLeaseV1::open(&supervisor_root)
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;
        let anchor_lifecycle = CompilerExecutionServiceLifecycleLeaseV1::open(&anchor_root)
            .map_err(CompilerExecutionCoordinatorErrorV1::ServiceLifecycle)?;

        let supervisor_deployment =
            decode_supervisor_deployment(File::from(supervisor_deployment))?;
        let deployment_capability =
            CompilerExecutionSupervisorDeploymentCapabilityV1::create(supervisor_deployment)
                .map_err(CompilerExecutionCoordinatorErrorV1::DeploymentCapability)?;
        let credentials = IssuerServiceCredentialProfileV1::new(
            deployment_capability.deployment().service_uid(),
            deployment_capability.deployment().service_gid(),
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Credentials)?;
        let service_inputs = ProvisionedProtectedIssuerServiceInputsV1::admit(
            listener,
            File::from(supervisor_root),
            credentials,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::ServiceInputs)?;

        let supervisor = admit_executable(File::from(supervisor), "supervisor executable")?;
        let launcher = admit_executable(File::from(launcher), "issuer pre-exec launcher")?;
        let issuer = admit_executable(File::from(issuer), "compiler-execution issuer")?;
        let anchor_helper = admit_executable(File::from(anchor_helper), "external-anchor helper")?;
        let anchor_daemon = admit_executable(File::from(anchor_daemon), "external-anchor daemon")?;

        let policy = decode_policy(File::from(policy))?;
        let anchor_deployment = decode_anchor_deployment(File::from(anchor_deployment))?;
        let anchor_provisioning = decode_anchor_provisioning(File::from(anchor_provisioning))?;

        let mut seed = read_seed(File::from(issuer_key_seed), "issuer signing-key seed")?;
        let issuer_key =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &policy)
                .map_err(CompilerExecutionCoordinatorErrorV1::KeyTemplate)?;
        let mut seed = read_seed(
            File::from(anchor_key_seed),
            "external-anchor signing-key seed",
        )?;
        let anchor_key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed,
            &anchor_deployment,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::ExternalAnchorKeyTemplate)?;

        let policy_capability = CompilerExecutionPolicyCapabilityV1::create(policy)
            .map_err(CompilerExecutionCoordinatorErrorV1::PolicyCapability)?;
        let trust = CompilerExecutionSupervisorTrustV1::new(
            deployment_capability,
            policy_capability,
            issuer_key,
        )?;
        let anchor = PreparedExternalAnchorOccurrenceV1::prepare(
            anchor_helper,
            anchor_daemon,
            File::from(anchor_root),
            anchor_lifecycle,
            anchor_deployment,
            anchor_provisioning,
            anchor_key,
        )
        .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        Ok(Self {
            programs: CompilerExecutionSupervisorProgramSourcesV1::new(
                supervisor, launcher, issuer,
            ),
            trust,
            service_inputs,
            anchor,
            supervisor_lifecycle,
            lifecycle,
        })
    }

    /// Launches the anchor first and then the exact bound supervisor under one timeout per stage.
    pub fn launch(
        self,
        timeout: Duration,
    ) -> Result<RootManagedCompilerExecutionServiceV1, CompilerExecutionCoordinatorErrorV1> {
        let anchor = self
            .anchor
            .launch(timeout)
            .map_err(CompilerExecutionCoordinatorErrorV1::Anchor)?;
        PreparedCompilerExecutionSupervisorV1::prepare(
            self.programs,
            self.trust,
            self.service_inputs,
            self.supervisor_lifecycle,
            self.lifecycle,
            anchor,
        )?
        .launch(timeout)
    }
}

fn require_single_threaded_entrypoint() -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let tasks = std::fs::read_dir("/proc/self/task").map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role: "coordinator process",
            reason: "cannot inspect process thread set",
        }
    })?;
    if tasks.take(2).count() != 1 {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role: "coordinator process",
            reason: "inherited descriptors must be admitted before creating threads",
        });
    }
    Ok(())
}

fn take_inherited_descriptors() -> Result<[OwnedFd; 14], CompilerExecutionCoordinatorErrorV1> {
    for descriptor in DESCRIPTORS_V1 {
        // SAFETY: F_GETFD accepts an integer descriptor and does not dereference process memory.
        let inherited_flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if inherited_flags < 0 {
            return Err(CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor,
                operation: "inspect inherited descriptor flags",
                source: std::io::Error::last_os_error(),
            });
        }
        if inherited_flags != 0 {
            return Err(CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor,
                operation: "require inheritable input descriptor",
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "descriptor was already close-on-exec",
                ),
            });
        }
    }
    // SAFETY: the complete preflight above proved every fixed descriptor valid; this one-shot
    // entrypoint contract transfers exclusive ownership before any thread can close or reuse one.
    let descriptors = DESCRIPTORS_V1.map(|descriptor| unsafe { OwnedFd::from_raw_fd(descriptor) });
    for descriptor in &descriptors {
        rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::CLOEXEC).map_err(|source| {
            CompilerExecutionCoordinatorErrorV1::InheritedDescriptor {
                descriptor: descriptor.as_raw_fd(),
                operation: "protect inherited descriptor",
                source: source.into(),
            }
        })?;
    }
    Ok(descriptors)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootFileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    pub(crate) byte_len: u64,
}

fn admit_executable(
    executable: File,
    role: &'static str,
) -> Result<File, CompilerExecutionCoordinatorErrorV1> {
    validate_root_file(&executable, role, EXECUTABLE_MODE_V1, None)?;
    Ok(executable)
}

fn decode_supervisor_deployment(
    file: File,
) -> Result<CompilerExecutionSupervisorDeploymentV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1>(
        &file,
        "supervisor deployment",
    )?;
    CompilerExecutionSupervisorDeploymentV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "supervisor deployment",
            reason: error.to_string(),
        }
    })
}

fn decode_policy(
    file: File,
) -> Result<CompilerExecutionIssuerPolicyV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes =
        read_stable_record::<COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1>(&file, "issuer policy")?;
    CompilerExecutionIssuerPolicyV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "issuer policy",
            reason: error.to_string(),
        }
    })
}

fn decode_anchor_deployment(
    file: File,
) -> Result<CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1>(
        &file,
        "external-anchor deployment",
    )?;
    CompilerExecutionExternalAnchorDeploymentV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "external-anchor deployment",
            reason: error.to_string(),
        }
    })
}

fn decode_anchor_provisioning(
    file: File,
) -> Result<CompilerExecutionExternalAnchorProvisioningV1, CompilerExecutionCoordinatorErrorV1> {
    let bytes = read_stable_record::<COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1>(
        &file,
        "external-anchor provisioning",
    )?;
    CompilerExecutionExternalAnchorProvisioningV1::decode(&bytes).map_err(|error| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedRecord {
            role: "external-anchor provisioning",
            reason: error.to_string(),
        }
    })
}

fn read_stable_record<const N: usize>(
    file: &File,
    role: &'static str,
) -> Result<[u8; N], CompilerExecutionCoordinatorErrorV1> {
    let before = validate_root_file(file, role, PUBLIC_RECORD_MODE_V1, Some(N))?;
    let first = read_exact_at::<N>(file, role)?;
    let second = read_exact_at::<N>(file, role)?;
    let after = validate_root_file(file, role, PUBLIC_RECORD_MODE_V1, Some(N))?;
    if before != after || first != second {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "record changed during admission",
        });
    }
    Ok(first)
}

fn read_seed(
    file: File,
    role: &'static str,
) -> Result<[u8; KEY_SEED_BYTES_V1], CompilerExecutionCoordinatorErrorV1> {
    let before = validate_root_file(&file, role, SECRET_SEED_MODE_V1, Some(KEY_SEED_BYTES_V1))?;
    let mut first = read_seed_copy(&file, role)?;
    let mut second = read_seed_copy(&file, role)?;
    let after = validate_root_file(&file, role, SECRET_SEED_MODE_V1, Some(KEY_SEED_BYTES_V1))?;
    if before != after || !bool::from(first.ct_eq(&second)) {
        first.zeroize();
        second.zeroize();
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "key seed changed during admission",
        });
    }
    second.zeroize();
    Ok(first)
}

fn read_seed_copy(
    file: &File,
    role: &'static str,
) -> Result<[u8; KEY_SEED_BYTES_V1], CompilerExecutionCoordinatorErrorV1> {
    let mut bytes = [0_u8; KEY_SEED_BYTES_V1];
    if file.read_exact_at(&mut bytes, 0).is_err() {
        bytes.zeroize();
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot read exact key-seed bytes",
        });
    }
    Ok(bytes)
}

fn read_exact_at<const N: usize>(
    file: &File,
    role: &'static str,
) -> Result<[u8; N], CompilerExecutionCoordinatorErrorV1> {
    let mut bytes = [0_u8; N];
    file.read_exact_at(&mut bytes, 0).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot read exact bytes",
        }
    })?;
    Ok(bytes)
}

fn validate_root_file(
    file: &File,
    role: &'static str,
    expected_mode: u32,
    expected_length: Option<usize>,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    validate_provisioned_file(
        file,
        role,
        expected_mode,
        expected_length,
        ROOT_ID_V1,
        ROOT_ID_V1,
    )
}

pub(crate) fn validate_provisioned_file(
    file: &File,
    role: &'static str,
    expected_mode: u32,
    expected_length: Option<usize>,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect descriptor flags",
        }
    })?;
    let status = rustix::fs::fcntl_getfl(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect status flags",
        }
    })?;
    let stat = rustix::fs::fstat(file).map_err(|_| {
        CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "cannot inspect object metadata",
        }
    })?;
    let snapshot = RootFileSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
        byte_len: stat.st_size.try_into().unwrap_or(u64::MAX),
    };
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.mode & 0o7777 != expected_mode
        || snapshot.uid != expected_uid
        || snapshot.gid != expected_gid
        || snapshot.links != 1
        || expected_length.is_some_and(|length| snapshot.byte_len != length as u64)
        || expected_length.is_none() && snapshot.byte_len == 0
    {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role,
            reason: "type, access, owner, group, mode, links, or length is not exact",
        });
    }
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(file, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                    role,
                    reason: "file has a forbidden capability or POSIX ACL",
                });
            }
            Err(_) => {
                return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                    role,
                    reason: "cannot inspect capability or POSIX ACL metadata",
                });
            }
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn inherited_descriptor_contract_is_dense_unique_and_fixed() {
        assert_eq!(
            DESCRIPTORS_V1,
            std::array::from_fn(|index| 3 + index as i32)
        );
    }

    #[test]
    fn inherited_deployment_is_move_only_and_descriptor_free() {
        fn assert_send<T: Send>() {}
        assert_send::<InheritedCompilerExecutionDeploymentV1>();
        assert!(!std::mem::needs_drop::<RootFileSnapshotV1>());
    }

    #[test]
    fn provisioned_file_policy_accepts_only_exact_immutable_shape() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("record");
        fs::write(&path, [0x5a; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let file = File::open(&path).unwrap();
        validate_provisioned_file(
            &file,
            "test record",
            PUBLIC_RECORD_MODE_V1,
            Some(32),
            uid,
            gid,
        )
        .unwrap();

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();

        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(31),
                uid,
                gid,
            )
            .is_err()
        );
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                different_id(uid),
                gid,
            )
            .is_err()
        );
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                different_id(gid),
            )
            .is_err()
        );

        fs::hard_link(&path, fixture.path().join("record-link")).unwrap();
        assert!(
            validate_provisioned_file(
                &file,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );
    }

    #[test]
    fn provisioned_file_policy_rejects_writable_and_empty_executables() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("writable-record");
        let mut writable = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        writable.write_all(&[0x5a; 32]).unwrap();
        writable.flush().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(PUBLIC_RECORD_MODE_V1)).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        assert!(
            validate_provisioned_file(
                &writable,
                "test record",
                PUBLIC_RECORD_MODE_V1,
                Some(32),
                uid,
                gid,
            )
            .is_err()
        );

        let empty = fixture.path().join("empty-executable");
        fs::write(&empty, []).unwrap();
        fs::set_permissions(&empty, fs::Permissions::from_mode(EXECUTABLE_MODE_V1)).unwrap();
        assert!(
            validate_provisioned_file(
                &File::open(empty).unwrap(),
                "test executable",
                EXECUTABLE_MODE_V1,
                None,
                uid,
                gid,
            )
            .is_err()
        );
    }

    fn different_id(id: u32) -> u32 {
        if id == u32::MAX - 1 {
            u32::MAX - 2
        } else {
            id + 1
        }
    }
}
