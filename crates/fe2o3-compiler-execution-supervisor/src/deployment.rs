//! Descriptor-only deployed service entrypoint.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV1, ProtectedServiceAdmissionErrorV1,
};
use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1, CompilerExecutionPolicyCapabilityV1,
    CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSupervisorDeploymentCapabilityV1,
};

use crate::{
    AdmittedIssuerProgramV1, IssuerProgramAdmissionErrorV1, IssuerServiceCredentialProfileErrorV1,
    IssuerServiceCredentialProfileV1, ProtectedIssuerLaunchErrorV1, ProtectedIssuerServiceErrorV1,
    ProtectedIssuerServiceReportV1, ProtectedIssuerServiceV1, ProtectedIssuerServiceWorkerCountV1,
    ProtectedIssuerSessionTimeoutErrorV1, ProtectedIssuerSessionTimeoutsV1,
    ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1,
    ProvisionedStaticExecutableMeasurementV1, validate_current_issuer_service_profile_v1,
};

/// Inherited production listener at the sole fixed supervisor pathname.
pub const COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1: RawFd = 3;
/// Dedicated service-owned durable issuer root.
pub const COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1: RawFd = 4;
/// Trusted static pre-exec launcher source image.
pub const COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1: RawFd = 5;
/// Trusted sealed-static compiler-execution issuer source image.
pub const COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1: RawFd = 6;
/// Immutable caller-pinned issuer policy capability.
pub const COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1: RawFd = 7;
/// Service-owned sealed issuer signing-key capability.
pub const COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1: RawFd = 8;
/// Connected endpoint owned by the independently administered external-anchor service.
pub const COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1: RawFd = 9;
/// Pidfd retaining the exact live external-anchor service process.
pub const COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1: RawFd = 10;

const PRIVATE_DESCRIPTOR_FLOOR_V1: RawFd = 256;
const CLOSE_RANGE_CLOEXEC: i32 = 1 << 2;
const WORKER_COUNT_V1: usize = 4;
const HANDOFF_TIMEOUT_V1: Duration = Duration::from_secs(30);
const LAUNCH_TIMEOUT_V1: Duration = Duration::from_secs(30);
const READINESS_TIMEOUT_V1: Duration = Duration::from_secs(30);
const PUBLICATION_TIMEOUT_V1: Duration = Duration::from_secs(30);
const SESSION_TIMEOUT_V1: Duration = Duration::from_secs(300);

const _: () = assert!(COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1 > libc::STDERR_FILENO);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1 < COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1 < COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1 < COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1 < COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1 < COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1
        < COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1
        < COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1
        < COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1
);
const _: () = assert!(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1 < PRIVATE_DESCRIPTOR_FLOOR_V1);

/// Runs the sole deployed protected issuer supervisor from fixed inherited descriptors.
///
/// The process accepts no arguments or environment. Trusted provisioning must establish the
/// complete locked service profile before exec and install every descriptor named by this module.
/// The function consumes and closes every inherited fixed descriptor after retaining private
/// close-on-exec custody, marks every unrelated descriptor close-on-exec, validates the deployment
/// manifest against the exact policy, launcher, service credentials, and external-anchor peer, and
/// then enters only [`ProtectedIssuerServiceV1::run`].
pub fn run_inherited_protected_issuer_service_v1()
-> Result<ProtectedIssuerServiceReportV1, ProtectedIssuerDeploymentErrorV1> {
    require_descriptor_only_invocation_v1()?;

    let deployment = CompilerExecutionSupervisorDeploymentCapabilityV1::from_inherited()
        .map_err(ProtectedIssuerDeploymentErrorV1::DeploymentCapability)?;
    close_inherited(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1)?;
    let manifest = deployment.deployment();
    let credentials =
        IssuerServiceCredentialProfileV1::new(manifest.service_uid(), manifest.service_gid())
            .map_err(ProtectedIssuerDeploymentErrorV1::Credentials)?;
    validate_current_issuer_service_profile_v1(credentials)
        .map_err(ProtectedIssuerDeploymentErrorV1::ProcessProfile)?;

    let policy = CompilerExecutionPolicyCapabilityV1::from_inherited_at(
        COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1,
    )
    .map_err(ProtectedIssuerDeploymentErrorV1::PolicyCapability)?;
    close_inherited(COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1)?;
    if !deployment.deployment().matches_policy(policy.policy()) {
        return Err(ProtectedIssuerDeploymentErrorV1::PolicyMismatch);
    }
    let signing_key_template = File::from(take_inherited(
        COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1,
    )?);
    let signing_key =
        CompilerExecutionSigningKeyCapabilityV1::reissue_root_template_for_current_service(
            signing_key_template,
            deployment.deployment(),
            policy.policy(),
        )
        .map_err(ProtectedIssuerDeploymentErrorV1::SigningKeyCapability)?;

    let listener = take_inherited(COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1)?;
    let root = File::from(take_inherited(COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1)?);
    let launcher = File::from(take_inherited(
        COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1,
    )?);
    let issuer = File::from(take_inherited(COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1)?);
    let external_anchor_peer =
        take_inherited(COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1)?;
    let external_anchor_pidfd =
        take_inherited(COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1)?;
    protect_unrelated_descriptors_v1()?;

    let launcher_measurement = ProvisionedStaticExecutableMeasurementV1::new(
        manifest.launcher().sha256(),
        manifest.launcher().byte_len(),
    )
    .map_err(ProtectedIssuerDeploymentErrorV1::LauncherMeasurement)?;
    let program =
        AdmittedIssuerProgramV1::provision(launcher, launcher_measurement, issuer, policy)
            .map_err(ProtectedIssuerDeploymentErrorV1::Program)?;
    let external_anchor = ProtectedExternalAnchorServiceAdmissionV1::admit(
        external_anchor_peer,
        external_anchor_pidfd,
        manifest.external_anchor_service(),
    )
    .map_err(ProtectedIssuerDeploymentErrorV1::ExternalAnchor)?;
    let supervisor =
        ProtectedIssuerSupervisorV1::bind(program, credentials, root, signing_key, external_anchor)
            .map_err(ProtectedIssuerDeploymentErrorV1::Supervisor)?;
    let timeouts = ProtectedIssuerSessionTimeoutsV1::new(
        HANDOFF_TIMEOUT_V1,
        LAUNCH_TIMEOUT_V1,
        READINESS_TIMEOUT_V1,
        PUBLICATION_TIMEOUT_V1,
        SESSION_TIMEOUT_V1,
    )
    .map_err(ProtectedIssuerDeploymentErrorV1::Timeouts)?;
    let service = ProtectedIssuerServiceV1::bind(supervisor, listener, timeouts)
        .map_err(ProtectedIssuerDeploymentErrorV1::Service)?;
    let workers = ProtectedIssuerServiceWorkerCountV1::new(WORKER_COUNT_V1)
        .map_err(ProtectedIssuerDeploymentErrorV1::Service)?;
    service
        .run(workers, |_| {})
        .map_err(ProtectedIssuerDeploymentErrorV1::Service)
}

fn require_descriptor_only_invocation_v1() -> Result<(), ProtectedIssuerDeploymentErrorV1> {
    if std::env::args_os().count() != 1 || std::env::vars_os().next().is_some() {
        return Err(ProtectedIssuerDeploymentErrorV1::RuntimeConfiguration);
    }
    Ok(())
}

fn take_inherited(descriptor: RawFd) -> Result<OwnedFd, ProtectedIssuerDeploymentErrorV1> {
    require_inherited(descriptor)?;
    // SAFETY: F_DUPFD_CLOEXEC atomically creates one new owned descriptor or reports an error.
    let retained = unsafe {
        libc::fcntl(
            descriptor,
            libc::F_DUPFD_CLOEXEC,
            PRIVATE_DESCRIPTOR_FLOOR_V1,
        )
    };
    if retained < 0 {
        return Err(ProtectedIssuerDeploymentErrorV1::Descriptor {
            operation: "retain inherited supervisor descriptor",
            source: io::Error::last_os_error(),
        });
    }
    // SAFETY: successful F_DUPFD_CLOEXEC returned one newly owned descriptor.
    let retained = unsafe { OwnedFd::from_raw_fd(retained) };
    close_inherited(descriptor)?;
    Ok(retained)
}

fn require_inherited(descriptor: RawFd) -> Result<(), ProtectedIssuerDeploymentErrorV1> {
    // SAFETY: F_GETFD consumes only the scalar descriptor and reports invalid inputs via errno.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(ProtectedIssuerDeploymentErrorV1::Descriptor {
            operation: "inspect inherited supervisor descriptor",
            source: io::Error::last_os_error(),
        });
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(ProtectedIssuerDeploymentErrorV1::UnexpectedCloseOnExec(
            descriptor,
        ));
    }
    Ok(())
}

fn close_inherited(descriptor: RawFd) -> Result<(), ProtectedIssuerDeploymentErrorV1> {
    // SAFETY: each caller closes one inherited fixed descriptor exactly once after private
    // close-on-exec custody has been retained.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(ProtectedIssuerDeploymentErrorV1::Descriptor {
            operation: "close inherited supervisor descriptor",
            source: io::Error::last_os_error(),
        });
    }
    Ok(())
}

fn protect_unrelated_descriptors_v1() -> Result<(), ProtectedIssuerDeploymentErrorV1> {
    // SAFETY: close_range with CLOEXEC changes descriptor flags only and does not dereference user
    // memory. Private retained descriptors are already close-on-exec; this protects all extras.
    if unsafe { libc::close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC) } != 0 {
        return Err(ProtectedIssuerDeploymentErrorV1::Descriptor {
            operation: "protect unrelated supervisor descriptors",
            source: io::Error::last_os_error(),
        });
    }
    Ok(())
}

/// Stable deployed-supervisor entrypoint failures.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerDeploymentErrorV1 {
    /// Arguments or environment attempted to configure the descriptor-only service.
    RuntimeConfiguration,
    /// A fixed descriptor could not be inspected, retained, protected, or closed.
    Descriptor {
        /// Exact bounded descriptor operation.
        operation: &'static str,
        /// Kernel or filesystem failure.
        source: io::Error,
    },
    /// One inherited fixed descriptor was unexpectedly close-on-exec.
    UnexpectedCloseOnExec(RawFd),
    /// The sealed deployment capability was invalid.
    DeploymentCapability(String),
    /// The sealed issuer-policy capability was invalid.
    PolicyCapability(String),
    /// The deployment manifest names a different issuer policy.
    PolicyMismatch,
    /// The sealed signing-key capability was invalid or mismatched.
    SigningKeyCapability(String),
    /// The deployment service credentials were invalid.
    Credentials(IssuerServiceCredentialProfileErrorV1),
    /// The current process does not have the exact locked service profile.
    ProcessProfile(ProtectedIssuerLaunchErrorV1),
    /// The trusted launcher measurement was invalid.
    LauncherMeasurement(IssuerProgramAdmissionErrorV1),
    /// Static launcher or issuer program admission failed.
    Program(IssuerProgramAdmissionErrorV1),
    /// External-anchor endpoint or pidfd admission failed.
    ExternalAnchor(ProtectedServiceAdmissionErrorV1),
    /// Complete supervisor authority binding failed.
    Supervisor(ProtectedIssuerSupervisorErrorV1),
    /// The fixed session timeout policy was invalid.
    Timeouts(ProtectedIssuerSessionTimeoutErrorV1),
    /// Listener admission or the fixed worker service failed.
    Service(ProtectedIssuerServiceErrorV1),
}

impl fmt::Display for ProtectedIssuerDeploymentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeConfiguration => formatter
                .write_str("protected issuer supervisor accepts no arguments or environment"),
            Self::Descriptor { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::UnexpectedCloseOnExec(descriptor) => write!(
                formatter,
                "inherited supervisor descriptor {descriptor} is unexpectedly close-on-exec"
            ),
            Self::DeploymentCapability(error) => {
                write!(
                    formatter,
                    "supervisor deployment capability failed: {error}"
                )
            }
            Self::PolicyCapability(error) => {
                write!(
                    formatter,
                    "supervisor issuer-policy capability failed: {error}"
                )
            }
            Self::PolicyMismatch => {
                formatter.write_str("supervisor deployment names another issuer policy")
            }
            Self::SigningKeyCapability(error) => {
                write!(
                    formatter,
                    "supervisor signing-key capability failed: {error}"
                )
            }
            Self::Credentials(error) => write!(formatter, "supervisor credentials failed: {error}"),
            Self::ProcessProfile(error) => {
                write!(formatter, "supervisor process profile failed: {error}")
            }
            Self::LauncherMeasurement(error) => {
                write!(formatter, "supervisor launcher measurement failed: {error}")
            }
            Self::Program(error) => {
                write!(formatter, "supervisor program admission failed: {error}")
            }
            Self::ExternalAnchor(error) => {
                write!(
                    formatter,
                    "supervisor external-anchor admission failed: {error}"
                )
            }
            Self::Supervisor(error) => write!(formatter, "supervisor authority failed: {error}"),
            Self::Timeouts(error) => write!(formatter, "supervisor timeouts failed: {error}"),
            Self::Service(error) => write!(formatter, "supervisor service failed: {error}"),
        }
    }
}

impl Error for ProtectedIssuerDeploymentErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor { source, .. } => Some(source),
            Self::Credentials(error) => Some(error),
            Self::ProcessProfile(error) => Some(error),
            Self::LauncherMeasurement(error) | Self::Program(error) => Some(error),
            Self::ExternalAnchor(error) => Some(error),
            Self::Supervisor(error) => Some(error),
            Self::Timeouts(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::RuntimeConfiguration
            | Self::UnexpectedCloseOnExec(_)
            | Self::DeploymentCapability(_)
            | Self::PolicyCapability(_)
            | Self::PolicyMismatch
            | Self::SigningKeyCapability(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, IntoRawFd};

    use rustix::pipe::{PipeFlags, pipe_with};

    use super::*;

    #[test]
    fn descriptor_contract_is_ordered_and_outside_static_child_staging() {
        assert_eq!(COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1, 3);
        assert_eq!(COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1, 10);
        assert_eq!(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1, 220);
        assert_eq!(WORKER_COUNT_V1, 4);
    }

    #[test]
    fn inherited_descriptor_is_privately_retained_and_source_is_closed() {
        let (reader, _writer) = pipe_with(PipeFlags::empty()).unwrap();
        // SAFETY: F_DUPFD returns a new descriptor or a negative error result.
        let source = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_DUPFD, 500) };
        assert!(source >= 500);
        drop(reader);
        // SAFETY: successful F_DUPFD returned one newly owned descriptor.
        let inherited = unsafe { OwnedFd::from_raw_fd(source) };
        let source = inherited.into_raw_fd();
        let retained = take_inherited(source).unwrap();
        assert!(retained.as_raw_fd() >= PRIVATE_DESCRIPTOR_FLOOR_V1);
        assert!(
            rustix::io::fcntl_getfd(&retained)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        // SAFETY: F_GETFD only inspects the scalar descriptor.
        assert_eq!(unsafe { libc::fcntl(source, libc::F_GETFD) }, -1);
    }

    #[test]
    fn close_on_exec_inherited_descriptor_fails_closed() {
        let (reader, _writer) = pipe_with(PipeFlags::CLOEXEC).unwrap();
        assert!(matches!(
            require_inherited(reader.as_raw_fd()),
            Err(ProtectedIssuerDeploymentErrorV1::UnexpectedCloseOnExec(_))
        ));
    }

    #[test]
    fn absent_inherited_descriptor_fails_closed() {
        assert!(matches!(
            require_inherited(i32::MAX),
            Err(ProtectedIssuerDeploymentErrorV1::Descriptor {
                operation: "inspect inherited supervisor descriptor",
                ..
            })
        ));
    }
}
