//! Fixed-descriptor deployed external-anchor entrypoint.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};

use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
    CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV1, LifecycleLeaseErrorV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileErrorV1, ProtectedServiceCredentialProfileV1,
    ProtectedServiceNamespaceSetV1, ProtectedServiceProcessProfileV1,
    ProtectedServiceProfileErrorV1, require_owned_sigchld_v1,
};
pub use fe2o3_protected_static_executable::ProtectedStaticExecutableErrorV1 as ExternalAnchorExecutableErrorV1;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1, ProtectedStaticExecutableOwnerV1,
    ProtectedStaticExecutableV1,
};

use crate::{
    DurableExternalAnchorV1, ExternalAnchorDaemonErrorV1, ExternalAnchorServiceErrorV1,
    ExternalAnchorServiceReportV1, serve_connected_peer_v1,
};

/// Connected unnamed nonblocking `SOCK_SEQPACKET` endpoint supplied to the anchor daemon.
pub const EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1: RawFd = 3;
/// Existing private mode-0700 durable state directory supplied to the anchor daemon.
pub const EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1: RawFd = 4;
/// Independent shared lifecycle lease retained until the external-anchor daemon exits.
pub const EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1: RawFd = 5;

const PRIVATE_ROOT_FD_V1: RawFd = 256;
const PRIVATE_PEER_FD_V1: RawFd = 257;
const PRIVATE_LIFECYCLE_FD_V1: RawFd = 258;
const PRIVATE_LIFECYCLE_PARENT_FD_V1: RawFd = 259;
const CLOSE_RANGE_CEILING_BEFORE_PRIVATE_V1: u32 = PRIVATE_ROOT_FD_V1 as u32 - 1;
const CLOSE_RANGE_FLOOR_AFTER_PRIVATE_V1: u32 = PRIVATE_LIFECYCLE_PARENT_FD_V1 as u32 + 1;

const _: () = assert!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1 == 221);
const _: () = assert!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1 == 222);
const _: () = assert!(PRIVATE_ROOT_FD_V1 > COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1);
const _: () = assert!(PRIVATE_PEER_FD_V1 == PRIVATE_ROOT_FD_V1 + 1);
const _: () = assert!(PRIVATE_LIFECYCLE_FD_V1 == PRIVATE_PEER_FD_V1 + 1);
const _: () = assert!(PRIVATE_LIFECYCLE_PARENT_FD_V1 == PRIVATE_LIFECYCLE_FD_V1 + 1);

struct AdmittedExternalAnchorProfileV1 {
    process: Option<ProtectedServiceProcessProfileV1>,
    namespaces: Option<ProtectedServiceNamespaceSetV1>,
}

struct RetainedExternalAnchorExecutableV1 {
    executable: Option<ProtectedStaticExecutableV1>,
}

impl RetainedExternalAnchorExecutableV1 {
    fn admit(
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, ExternalAnchorExecutableErrorV1> {
        let (measurement, owner) = executable_contract(deployment)?;
        ProtectedStaticExecutableV1::admit_running(measurement, owner, "external-anchor service")
            .map(|executable| Self {
                executable: Some(executable),
            })
    }

    fn revalidate(&self) -> Result<(), ExternalAnchorExecutableErrorV1> {
        if let Some(executable) = &self.executable {
            executable.revalidate()?;
        }
        Ok(())
    }

    #[cfg(test)]
    const fn for_test() -> Self {
        Self { executable: None }
    }
}

fn executable_contract(
    deployment: &CompilerExecutionExternalAnchorDeploymentV1,
) -> Result<
    (
        ProtectedStaticExecutableMeasurementV1,
        ProtectedStaticExecutableOwnerV1,
    ),
    ExternalAnchorExecutableErrorV1,
> {
    let executable = deployment.executable();
    let service = deployment.service();
    Ok((
        ProtectedStaticExecutableMeasurementV1::new(
            executable.sha256(),
            executable.byte_len(),
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        )?,
        ProtectedStaticExecutableOwnerV1::new(service.uid(), service.gid())?,
    ))
}

impl AdmittedExternalAnchorProfileV1 {
    fn admit(
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedServiceProfileErrorV1> {
        let process = ProtectedServiceProcessProfileV1::capture(credentials)?;
        require_owned_sigchld_v1()?;
        let namespaces = ProtectedServiceNamespaceSetV1::capture_self()?;
        namespaces.revalidate_self()?;
        Ok(Self {
            process: Some(process),
            namespaces: Some(namespaces),
        })
    }

    #[cfg(test)]
    const fn for_test() -> Self {
        Self {
            process: None,
            namespaces: None,
        }
    }

    fn revalidate(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        if let Some(process) = &self.process {
            process.revalidate_current()?;
        }
        if let Some(namespaces) = &self.namespaces {
            namespaces.revalidate_self()?;
        }
        if self.process.is_some() != self.namespaces.is_some() {
            return Err(ProtectedServiceProfileErrorV1::InvalidState(
                "partial external-anchor profile admission",
            ));
        }
        Ok(())
    }
}

/// Runs the sole descriptor-only external-anchor service occurrence.
///
/// The deployment manifest is public and admitted first. The complete locked process profile is
/// then admitted before FD 222 is inspected or any private key bytes are read. Production opens an
/// already initialized durable root; this entrypoint can never create or reset genesis state.
pub fn run_inherited_external_anchor_service_v1()
-> Result<ExternalAnchorServiceReportV1, ExternalAnchorEntrypointErrorV1> {
    require_descriptor_only_invocation_v1()?;
    run_inherited_with_profile_v1(AdmittedExternalAnchorProfileV1::admit)
}

fn run_inherited_with_profile_v1(
    admit_profile: impl FnOnce(
        ProtectedServiceCredentialProfileV1,
    ) -> Result<
        AdmittedExternalAnchorProfileV1,
        ProtectedServiceProfileErrorV1,
    >,
) -> Result<ExternalAnchorServiceReportV1, ExternalAnchorEntrypointErrorV1> {
    run_inherited_with_admission_v1(
        admit_profile,
        RetainedExternalAnchorExecutableV1::admit,
        |file, root| {
            CompilerExecutionServiceLifecycleLeaseV1::admit_with_parent_at(
                file,
                root,
                PRIVATE_LIFECYCLE_PARENT_FD_V1,
            )
        },
    )
}

fn run_inherited_with_admission_v1(
    admit_profile: impl FnOnce(
        ProtectedServiceCredentialProfileV1,
    ) -> Result<
        AdmittedExternalAnchorProfileV1,
        ProtectedServiceProfileErrorV1,
    >,
    admit_executable: impl FnOnce(
        &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<
        RetainedExternalAnchorExecutableV1,
        ExternalAnchorExecutableErrorV1,
    >,
    admit_lifecycle: impl FnOnce(
        File,
        &OwnedFd,
    ) -> Result<
        CompilerExecutionServiceLifecycleLeaseV1,
        LifecycleLeaseErrorV1,
    >,
) -> Result<ExternalAnchorServiceReportV1, ExternalAnchorEntrypointErrorV1> {
    let deployment = CompilerExecutionExternalAnchorDeploymentCapabilityV1::from_inherited()
        .map_err(ExternalAnchorEntrypointErrorV1::DeploymentCapability)?;
    close_inherited(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1)?;
    let manifest = deployment.deployment().clone();
    let service = manifest.service();
    let credentials = ProtectedServiceCredentialProfileV1::new(service.uid(), service.gid())
        .map_err(ExternalAnchorEntrypointErrorV1::Credentials)?;
    let profile = admit_profile(credentials).map_err(ExternalAnchorEntrypointErrorV1::Profile)?;
    profile
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Profile)?;
    deployment
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::DeploymentCapability)?;
    let executable =
        admit_executable(&manifest).map_err(ExternalAnchorEntrypointErrorV1::Executable)?;
    executable
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Executable)?;

    let key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_inherited(&manifest)
        .map_err(ExternalAnchorEntrypointErrorV1::SigningKeyCapability)?;
    close_inherited(COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1)?;
    key.revalidate(&manifest)
        .map_err(ExternalAnchorEntrypointErrorV1::SigningKeyCapability)?;

    let root = take_inherited_at(
        EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1,
        PRIVATE_ROOT_FD_V1,
        "durable root",
    )?;
    let peer = take_inherited_at(
        EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1,
        PRIVATE_PEER_FD_V1,
        "connected peer",
    )?;
    let lifecycle = admit_lifecycle(
        File::from(take_inherited_at(
            EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1,
            PRIVATE_LIFECYCLE_FD_V1,
            "lifecycle lease",
        )?),
        &root,
    )
    .map_err(ExternalAnchorEntrypointErrorV1::Lifecycle)?;
    profile
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Profile)?;
    deployment
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::DeploymentCapability)?;
    executable
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Executable)?;
    let signing_key = key
        .into_signing_key(&manifest)
        .map_err(ExternalAnchorEntrypointErrorV1::SigningKeyCapability)?;
    drop(deployment);
    drop(executable);
    close_unrelated_descriptors_v1()?;
    profile
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Profile)?;
    lifecycle
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Lifecycle)?;

    let mut anchor = DurableExternalAnchorV1::open(root, signing_key)
        .map_err(ExternalAnchorEntrypointErrorV1::Anchor)?;
    if anchor.verifying_key_bytes() != *manifest.verifying_key() {
        return Err(ExternalAnchorEntrypointErrorV1::SigningKeyMismatch);
    }
    profile
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Profile)?;
    lifecycle
        .revalidate()
        .map_err(ExternalAnchorEntrypointErrorV1::Lifecycle)?;
    serve_connected_peer_v1(&mut anchor, peer).map_err(ExternalAnchorEntrypointErrorV1::Daemon)
}

fn require_descriptor_only_invocation_v1() -> Result<(), ExternalAnchorEntrypointErrorV1> {
    if std::env::args_os().count() != 1 || std::env::vars_os().next().is_some() {
        return Err(ExternalAnchorEntrypointErrorV1::RuntimeConfiguration);
    }
    Ok(())
}

fn take_inherited_at(
    source: RawFd,
    target: RawFd,
    label: &'static str,
) -> Result<OwnedFd, ExternalAnchorEntrypointErrorV1> {
    require_inherited(source, label)?;
    require_unused(target, label)?;
    // SAFETY: F_DUPFD_CLOEXEC atomically creates one new owned descriptor or reports an error.
    let retained = unsafe { libc::fcntl(source, libc::F_DUPFD_CLOEXEC, target) };
    if retained < 0 {
        return Err(descriptor_error(
            "retain inherited external-anchor descriptor",
        ));
    }
    if retained != target {
        // SAFETY: successful fcntl returned one newly owned descriptor not represented elsewhere.
        let _ = unsafe { libc::close(retained) };
        return Err(ExternalAnchorEntrypointErrorV1::PrivateDescriptorBusy {
            descriptor: target,
            label,
        });
    }
    // SAFETY: successful F_DUPFD_CLOEXEC returned one newly owned descriptor.
    let retained = unsafe { OwnedFd::from_raw_fd(retained) };
    close_inherited(source)?;
    Ok(retained)
}

fn require_inherited(
    descriptor: RawFd,
    label: &'static str,
) -> Result<(), ExternalAnchorEntrypointErrorV1> {
    // SAFETY: F_GETFD consumes only the scalar descriptor and reports invalid inputs via errno.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_error(
            "inspect inherited external-anchor descriptor",
        ));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(ExternalAnchorEntrypointErrorV1::UnexpectedCloseOnExec { descriptor, label });
    }
    Ok(())
}

fn require_unused(
    descriptor: RawFd,
    label: &'static str,
) -> Result<(), ExternalAnchorEntrypointErrorV1> {
    // SAFETY: F_GETFD consumes only the scalar descriptor and reports an unused descriptor via
    // EBADF.
    if unsafe { libc::fcntl(descriptor, libc::F_GETFD) } >= 0 {
        return Err(ExternalAnchorEntrypointErrorV1::PrivateDescriptorBusy { descriptor, label });
    }
    let source = io::Error::last_os_error();
    if source.raw_os_error() != Some(libc::EBADF) {
        return Err(ExternalAnchorEntrypointErrorV1::Descriptor {
            operation: "inspect private external-anchor descriptor",
            source,
        });
    }
    Ok(())
}

fn close_inherited(descriptor: RawFd) -> Result<(), ExternalAnchorEntrypointErrorV1> {
    // SAFETY: each caller closes one inherited fixed descriptor exactly once after private
    // close-on-exec custody has been retained.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(descriptor_error(
            "close inherited external-anchor descriptor",
        ));
    }
    Ok(())
}

fn close_unrelated_descriptors_v1() -> Result<(), ExternalAnchorEntrypointErrorV1> {
    // SAFETY: close_range closes scalar descriptor ranges and does not dereference user memory.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            0,
            CLOSE_RANGE_CEILING_BEFORE_PRIVATE_V1,
            0,
        )
    } != 0
    {
        return Err(descriptor_error(
            "close low unrelated external-anchor descriptors",
        ));
    }
    // SAFETY: the range starts above the two exact private descriptors retained by this process.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            CLOSE_RANGE_FLOOR_AFTER_PRIVATE_V1,
            u32::MAX,
            0,
        )
    } != 0
    {
        return Err(descriptor_error(
            "close high unrelated external-anchor descriptors",
        ));
    }
    Ok(())
}

fn descriptor_error(operation: &'static str) -> ExternalAnchorEntrypointErrorV1 {
    ExternalAnchorEntrypointErrorV1::Descriptor {
        operation,
        source: io::Error::last_os_error(),
    }
}

/// Stable failure entering or running the fixed-descriptor external-anchor service.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExternalAnchorEntrypointErrorV1 {
    RuntimeConfiguration,
    DeploymentCapability(String),
    Credentials(ProtectedServiceCredentialProfileErrorV1),
    Profile(ProtectedServiceProfileErrorV1),
    Lifecycle(LifecycleLeaseErrorV1),
    Executable(ExternalAnchorExecutableErrorV1),
    SigningKeyCapability(String),
    UnexpectedCloseOnExec {
        descriptor: RawFd,
        label: &'static str,
    },
    PrivateDescriptorBusy {
        descriptor: RawFd,
        label: &'static str,
    },
    Descriptor {
        operation: &'static str,
        source: io::Error,
    },
    SigningKeyMismatch,
    Anchor(ExternalAnchorServiceErrorV1),
    Daemon(ExternalAnchorDaemonErrorV1),
}

impl fmt::Display for ExternalAnchorEntrypointErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeConfiguration => formatter.write_str(
                "external-anchor service accepts no arguments or environment configuration",
            ),
            Self::DeploymentCapability(error) => {
                write!(
                    formatter,
                    "invalid external-anchor deployment capability: {error}"
                )
            }
            Self::Credentials(error) => write!(formatter, "invalid anchor credentials: {error}"),
            Self::Profile(error) => write!(formatter, "invalid anchor process profile: {error}"),
            Self::Lifecycle(error) => write!(formatter, "invalid anchor lifecycle: {error}"),
            Self::Executable(error) => {
                write!(formatter, "invalid anchor executable image: {error}")
            }
            Self::SigningKeyCapability(error) => {
                write!(
                    formatter,
                    "invalid external-anchor signing-key capability: {error}"
                )
            }
            Self::UnexpectedCloseOnExec { descriptor, label } => write!(
                formatter,
                "inherited external-anchor {label} descriptor {descriptor} is close-on-exec"
            ),
            Self::PrivateDescriptorBusy { descriptor, label } => write!(
                formatter,
                "private external-anchor {label} descriptor {descriptor} is already in use"
            ),
            Self::Descriptor { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::SigningKeyMismatch => {
                formatter.write_str("opened anchor key differs from its deployment manifest")
            }
            Self::Anchor(error) => write!(formatter, "cannot open durable anchor state: {error}"),
            Self::Daemon(error) => write!(formatter, "external-anchor daemon failed: {error}"),
        }
    }
}

impl Error for ExternalAnchorEntrypointErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Credentials(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Executable(error) => Some(error),
            Self::Descriptor { source, .. } => Some(source),
            Self::Anchor(error) => Some(error),
            Self::Daemon(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::fd::{AsRawFd, IntoRawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV1,
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };
    use rustix::fs::{OFlags, SealFlags};
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
    use sha2::{Digest, Sha256};

    use super::*;

    fn manifest(
        seed: u8,
        executable: CompilerExecutionIssuerMeasurementV1,
    ) -> fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV1 {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 2).unwrap(),
            SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw().max(1),
            rustix::process::getegid().as_raw().max(1),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            service.uid().wrapping_add(1).max(1),
            service.gid().wrapping_add(1).max(1),
            service,
            CompilerExecutionIssuerMeasurementV1::new([3; 32], 3).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([4; 32], 4).unwrap(),
            &policy,
        )
        .unwrap();
        CompilerExecutionExternalAnchorDeploymentV1::new(&supervisor, &policy, executable).unwrap()
    }

    fn fake_executable() -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new([5; 32], 5).unwrap()
    }

    #[test]
    fn executable_admission_rejects_measurement_substitution() {
        let _guard = crate::ENTRYPOINT_TEST_LOCK.lock().unwrap();
        let (image, measurement) = sealed_test_executable();
        let mut substituted_digest = measurement.sha256();
        substituted_digest[0] ^= 1;
        let substituted =
            CompilerExecutionIssuerMeasurementV1::new(substituted_digest, measurement.byte_len())
                .unwrap();

        let deployment = manifest(17, substituted);
        let (measurement, owner) = executable_contract(&deployment).unwrap();
        assert!(matches!(
            ProtectedStaticExecutableV1::admit_sealed(
                image,
                measurement,
                owner,
                "external-anchor service",
            ),
            Err(ExternalAnchorExecutableErrorV1::MeasurementMismatch(
                "external-anchor service"
            ))
        ));
    }

    #[test]
    fn descriptor_contract_is_fixed_and_disjoint() {
        assert_eq!(EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1, 3);
        assert_eq!(EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1, 4);
        assert_eq!(EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1, 5);
        assert_eq!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1, 221);
        assert_eq!(COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1, 222);
        assert_eq!(PRIVATE_PEER_FD_V1, PRIVATE_ROOT_FD_V1 + 1);
        assert_eq!(PRIVATE_LIFECYCLE_FD_V1, PRIVATE_PEER_FD_V1 + 1);
        assert_eq!(PRIVATE_LIFECYCLE_PARENT_FD_V1, PRIVATE_LIFECYCLE_FD_V1 + 1);
    }

    #[test]
    fn descriptor_only_contract_rejects_test_process_configuration() {
        assert!(matches!(
            require_descriptor_only_invocation_v1(),
            Err(ExternalAnchorEntrypointErrorV1::RuntimeConfiguration)
        ));
    }

    #[test]
    fn process_profile_rejects_before_secret_descriptor_is_inspected() {
        let _guard = crate::ENTRYPOINT_TEST_LOCK.lock().unwrap();
        let capability = CompilerExecutionExternalAnchorDeploymentCapabilityV1::create(manifest(
            17,
            fake_executable(),
        ))
        .unwrap();
        let installed = rustix::io::fcntl_dupfd_cloexec(
            capability.try_clone_for_transfer().unwrap(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
        )
        .unwrap();
        assert_eq!(
            installed.as_raw_fd(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1
        );
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        let _ = installed.into_raw_fd();
        // SAFETY: F_GETFD only observes the scalar fixed descriptor.
        assert!(
            unsafe {
                libc::fcntl(
                    COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
                    libc::F_GETFD,
                )
            } < 0
        );

        let result = run_inherited_with_profile_v1(|_| {
            Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "hostile profile rejected",
            ))
        });
        assert!(matches!(
            result,
            Err(ExternalAnchorEntrypointErrorV1::Profile(
                ProtectedServiceProfileErrorV1::ProcessProfile("hostile profile rejected")
            ))
        ));
    }

    #[test]
    fn core_opens_existing_state_and_serves_one_exact_exchange() {
        let _guard = crate::ENTRYPOINT_TEST_LOCK.lock().unwrap();
        let (executable, executable_measurement) = sealed_test_executable();
        let deployment = manifest(17, executable_measurement);
        let deployment_capability =
            CompilerExecutionExternalAnchorDeploymentCapabilityV1::create(deployment.clone())
                .unwrap();
        let mut seed = [17; 32];
        let key_capability =
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
                &mut seed,
                &deployment,
            )
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let state_root = directory.path().join("external-anchor");
        fs::create_dir(&state_root).unwrap();
        fs::set_permissions(&state_root, fs::Permissions::from_mode(0o700)).unwrap();
        let lifecycle_path = directory.path().join(
            std::path::Path::new(
                fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
            )
            .file_name()
            .unwrap(),
        );
        fs::write(&lifecycle_path, []).unwrap();
        fs::set_permissions(&lifecycle_path, fs::Permissions::from_mode(0o400)).unwrap();
        let root = File::open(&state_root).unwrap();
        DurableExternalAnchorV1::initialize(
            root.try_clone().unwrap().into(),
            SigningKey::from_bytes(&[17; 32]),
        )
        .unwrap();
        let (service_peer, client_peer) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();

        let mut child = spawn_entrypoint_helper(
            executable,
            deployment_capability.try_clone_for_transfer().unwrap(),
            key_capability.try_clone_for_transfer().unwrap(),
            root,
            File::open(lifecycle_path).unwrap(),
            service_peer,
        );

        let pinned = fe2o3_external_anchor_protocol::PinnedAnchorKeyV1::from_bytes(
            *deployment.verifying_key(),
        )
        .unwrap();
        let pending = fe2o3_external_anchor_protocol::AnchoredStateV1::from_local_state(
            0,
            fe2o3_external_anchor_protocol::HashChainHeadV1::from_bytes([0; 32]),
        )
        .prepare(
            fe2o3_external_anchor_protocol::TransactionDigestV1::from_bytes([9; 32]),
            &pinned,
        )
        .unwrap()
        .begin_advance(
            fe2o3_external_anchor_protocol::CallerNonceV1::from_bytes([8; 32]),
            &pinned,
        )
        .unwrap();
        let challenge_bytes = pending.challenge().as_bytes();
        assert_eq!(
            rustix::net::send(
                &client_peer,
                challenge_bytes,
                rustix::net::SendFlags::empty()
            )
            .unwrap(),
            challenge_bytes.len()
        );
        let mut response = [0_u8; fe2o3_external_anchor_protocol::ANCHOR_OBSERVATION_WIRE_LEN_V1];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match rustix::net::recv(&client_peer, &mut response, rustix::net::RecvFlags::empty()) {
                Ok((count, message_length)) => {
                    if count == 0 {
                        let output = child.wait_with_output().unwrap();
                        panic!(
                            "anchor closed before its observation; child status: {}; child stderr: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    assert_eq!(count, response.len());
                    assert_eq!(message_length, response.len());
                    break;
                }
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let output = child.wait_with_output().unwrap();
                        panic!(
                            "timed out waiting for anchor observation; child status: {}; child stderr: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    std::thread::yield_now();
                }
                Err(error) => {
                    let output = child.wait_with_output().unwrap();
                    panic!(
                        "receive anchor observation: {error}; child status: {}; child stderr: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }
        assert!(matches!(
            pending.verify(&response).unwrap(),
            fe2o3_external_anchor_protocol::AnchorDecisionV1::Commit(_)
        ));
        drop(client_peer);
        assert!(child.wait_with_output().unwrap().status.success());
    }

    fn spawn_entrypoint_helper(
        executable: File,
        deployment: File,
        key: File,
        root: File,
        lifecycle: File,
        peer: OwnedFd,
    ) -> std::process::Child {
        use std::os::unix::process::CommandExt;

        let executable = duplicate_at(&executable, 400);
        let program = format!("/proc/self/fd/{}", executable.as_raw_fd());
        let sources = [
            duplicate_high(&peer),
            duplicate_high(&root),
            duplicate_high(&lifecycle),
            duplicate_high(&deployment),
            duplicate_high(&key),
        ];
        let mut command = std::process::Command::new(program);
        command
            .arg("--exact")
            .arg("entrypoint::tests::entrypoint_subprocess_helper")
            .arg("--nocapture")
            .env_clear()
            .env("FE2O3_ANCHOR_ENTRYPOINT_TEST_HELPER", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        // SAFETY: each source is an owned high descriptor retained by the callback. dup2 and
        // F_SETFD are async-signal-safe descriptor operations before exec.
        unsafe {
            command.pre_exec(move || {
                let _keep_executable_open_through_exec = &executable;
                for (source, target) in sources.iter().zip([
                    EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1,
                    EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1,
                    EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1,
                    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
                    COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
                ]) {
                    if libc::dup2(source.as_raw_fd(), target) != target
                        || libc::fcntl(target, libc::F_SETFD, 0) != 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        command.spawn().unwrap()
    }

    fn duplicate_high(descriptor: &impl std::os::fd::AsFd) -> OwnedFd {
        rustix::io::fcntl_dupfd_cloexec(descriptor, 300).unwrap()
    }

    fn duplicate_at(descriptor: &impl std::os::fd::AsFd, target: RawFd) -> OwnedFd {
        let duplicated = rustix::io::fcntl_dupfd_cloexec(descriptor, target).unwrap();
        assert_eq!(duplicated.as_raw_fd(), target);
        duplicated
    }

    fn sealed_test_executable() -> (File, CompilerExecutionIssuerMeasurementV1) {
        let bytes = fs::read(std::env::current_exe().unwrap()).unwrap();
        let length = bytes.len() as u64;
        let image = rustix::fs::memfd_create(
            c"fe2o3-external-anchor-test-executable",
            rustix::fs::MemfdFlags::CLOEXEC
                | rustix::fs::MemfdFlags::ALLOW_SEALING
                | rustix::fs::MemfdFlags::EXEC,
        )
        .map(File::from)
        .unwrap();
        let mut writer = image.try_clone().unwrap();
        writer.write_all(&bytes).unwrap();
        writer.flush().unwrap();
        drop(writer);
        rustix::fs::fchmod(
            &image,
            rustix::fs::Mode::RUSR
                | rustix::fs::Mode::RGRP
                | rustix::fs::Mode::ROTH
                | rustix::fs::Mode::XUSR
                | rustix::fs::Mode::XGRP
                | rustix::fs::Mode::XOTH,
        )
        .unwrap();
        rustix::fs::fcntl_add_seals(
            &image,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::EXEC,
        )
        .unwrap();
        rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL).unwrap();
        let path = PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
        let read_only = rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .unwrap();
        drop(image);
        (
            read_only,
            CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(&bytes).into(), length)
                .unwrap(),
        )
    }

    #[test]
    fn entrypoint_subprocess_helper() {
        if std::env::var_os("FE2O3_ANCHOR_ENTRYPOINT_TEST_HELPER").is_none() {
            return;
        }
        let result = run_inherited_with_admission_v1(
            |_| Ok(AdmittedExternalAnchorProfileV1::for_test()),
            |_| Ok(RetainedExternalAnchorExecutableV1::for_test()),
            |file, root| {
                CompilerExecutionServiceLifecycleLeaseV1::admit_non_authoritative_same_owner_test_with_parent_at(
                    file,
                    root,
                    PRIVATE_LIFECYCLE_PARENT_FD_V1,
                )
            },
        );
        std::process::exit(match result {
            Ok(report) if report.exchanges() == 1 => 0,
            Ok(report) => {
                eprintln!("unexpected external-anchor report: {report:?}");
                1
            }
            Err(error) => {
                eprintln!("external-anchor entrypoint failed: {error:?}");
                1
            }
        });
    }
}
