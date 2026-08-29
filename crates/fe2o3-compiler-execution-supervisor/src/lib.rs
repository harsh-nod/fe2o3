#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-compiler-execution-supervisor requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;

use fe2o3_broker_authority_service::sealed_static_issuer_runtime_measurement_v1;
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV1;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV1, ProtectedStaticExecutableMeasurementV1,
    ProtectedStaticExecutableOwnerV1, ProtectedStaticExecutableV1,
};
use fe2o3_runtime_protocol::SealedStaticApplicationErrorV1;
use fe2o3_static_preexec_manifest::StaticPreexecObjectIdentityV1;

mod authority;
#[allow(unsafe_code)]
mod deployment;
mod handoff;
mod launch;
mod listener;
#[allow(unsafe_code)]
mod process;
mod session;

pub use authority::{
    ISSUER_SERVICE_SECUREBITS_V1, IssuerServiceCredentialProfileErrorV1,
    IssuerServiceCredentialProfileV1, ProtectedIssuerSupervisorErrorV1,
    ProtectedIssuerSupervisorV1,
};
pub use deployment::{
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1,
    COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1, COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1, COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1,
    COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1, COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1,
    ProtectedIssuerDeploymentErrorV1, run_inherited_protected_issuer_service_v1,
};
pub use handoff::{AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1};
pub use launch::{PreparedProtectedIssuerLaunchV1, ProtectedIssuerLaunchPreparationErrorV1};
pub use listener::{
    ProtectedIssuerServiceErrorV1, ProtectedIssuerServiceReportV1,
    ProtectedIssuerServiceShutdownV1, ProtectedIssuerServiceV1,
    ProtectedIssuerServiceWorkerCountV1, ProtectedIssuerSessionOutcomeV1,
};
pub use process::{
    ExitedProtectedIssuerV1, LaunchedProtectedIssuerV1, MAX_PROTECTED_ISSUER_PROCESSES_V1,
    ProtectedIssuerLaunchErrorV1, ProtectedIssuerTerminationV1, ReadyProtectedIssuerV1,
    ServingProtectedIssuerV1, validate_current_issuer_service_profile_v1,
};
pub use session::{
    ProtectedIssuerSessionErrorV1, ProtectedIssuerSessionTimeoutErrorV1,
    ProtectedIssuerSessionTimeoutsV1,
};

const MAX_PROVISIONED_EXECUTABLE_BYTES_V1: u64 =
    fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1;

/// Exact trusted-provisioning measurement for one static executable image.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProvisionedStaticExecutableMeasurementV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProvisionedStaticExecutableMeasurementV1 {
    /// Constructs one nonzero bounded executable measurement.
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 || byte_len > MAX_PROVISIONED_EXECUTABLE_BYTES_V1 {
            return Err(IssuerProgramAdmissionErrorV1::InvalidMeasurement);
        }
        Ok(Self { sha256, byte_len })
    }

    fn from_issuer_policy(
        measurement: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        Self::new(measurement.sha256(), measurement.byte_len())
    }

    /// Returns the complete image SHA-256.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact image length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Stable failure categories for protected issuer program admission.
#[derive(Debug)]
#[non_exhaustive]
pub enum IssuerProgramAdmissionErrorV1 {
    /// The trusted expected measurement is empty, zero, or exceeds the fixed bound.
    InvalidMeasurement,
    /// A provisioned source descriptor has the wrong type, mode, access, or descriptor flags.
    InvalidSource(&'static str),
    /// A provisioned source carries a file capability.
    SourceFileCapability(&'static str),
    /// A provisioned source changed while it was copied.
    SourceChanged(&'static str),
    /// Complete source bytes do not match trusted provisioning.
    MeasurementMismatch(&'static str),
    /// An image is outside the loader-independent static ELF profile.
    InvalidStaticImage {
        /// Image role being admitted.
        role: &'static str,
        /// Exact static-image validation failure.
        source: SealedStaticApplicationErrorV1,
    },
    /// The sealed caller policy is invalid or changed.
    Policy(String),
    /// The caller policy does not name the fixed sealed-static issuer runtime closure.
    RuntimePolicyMismatch,
    /// The retained issuer image no longer agrees with its sealed caller policy.
    PolicyImageMismatch,
    /// A newly created sealed executable has an invalid kernel-visible property.
    InvalidSealedImage(&'static str),
    /// A bounded operating-system operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem error.
        source: io::Error,
    },
}

impl fmt::Display for IssuerProgramAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMeasurement => formatter.write_str("invalid provisioned measurement"),
            Self::InvalidSource(role) => write!(formatter, "invalid {role} source descriptor"),
            Self::SourceFileCapability(role) => {
                write!(formatter, "{role} source has a file capability")
            }
            Self::SourceChanged(role) => {
                write!(formatter, "{role} source changed during admission")
            }
            Self::MeasurementMismatch(role) => {
                write!(
                    formatter,
                    "{role} source does not match trusted provisioning"
                )
            }
            Self::InvalidStaticImage { role, source } => {
                write!(formatter, "{role} source is not sealed-static: {source}")
            }
            Self::Policy(error) => write!(formatter, "invalid compiler-execution policy: {error}"),
            Self::RuntimePolicyMismatch => {
                formatter.write_str("compiler-execution policy names a different runtime closure")
            }
            Self::PolicyImageMismatch => formatter
                .write_str("sealed compiler-execution policy and issuer image do not agree"),
            Self::InvalidSealedImage(role) => {
                write!(formatter, "new sealed {role} image has an invalid property")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for IssuerProgramAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidStaticImage { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Move-only authenticated launcher, issuer, and caller-policy custody.
///
/// This value is produced only during trusted service provisioning. Per-launch
/// requests cannot select either executable or its expected measurement.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<AdmittedIssuerProgramV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<AdmittedIssuerProgramV1>();
/// ```
pub struct AdmittedIssuerProgramV1 {
    launcher: PinnedSealedStaticExecutableV1,
    issuer: PinnedSealedStaticExecutableV1,
    policy: CompilerExecutionPolicyCapabilityV1,
}

impl fmt::Debug for AdmittedIssuerProgramV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedIssuerProgramV1")
            .field("authority", &"none")
            .field("launcher", &self.launcher.measurement)
            .field("issuer", &self.issuer.measurement)
            .field("policy", self.policy.policy())
            .finish_non_exhaustive()
    }
}

impl AdmittedIssuerProgramV1 {
    /// Authenticates and seals the complete executable chain before authority binding.
    ///
    /// `launcher_expected` must come from trusted service release provisioning,
    /// never from a compiler launch request. The issuer expectation and runtime
    /// closure come from `policy`.
    pub fn provision(
        launcher_source: File,
        launcher_expected: ProvisionedStaticExecutableMeasurementV1,
        issuer_source: File,
        policy: CompilerExecutionPolicyCapabilityV1,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        policy
            .revalidate()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if policy.policy().runtime() != sealed_static_issuer_runtime_measurement_v1() {
            return Err(IssuerProgramAdmissionErrorV1::RuntimePolicyMismatch);
        }
        let launcher = PinnedSealedStaticExecutableV1::admit(
            launcher_source,
            launcher_expected,
            "static launcher",
        )?;
        let issuer = PinnedSealedStaticExecutableV1::admit(
            issuer_source,
            ProvisionedStaticExecutableMeasurementV1::from_issuer_policy(
                policy.policy().executable(),
            )?,
            "compiler issuer",
        )?;
        if launcher.same_object_key(&issuer) {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(
                "aliased launcher and issuer",
            ));
        }
        let admitted = Self {
            launcher,
            issuer,
            policy,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Revalidates both sealed images and the exact caller policy.
    pub fn revalidate(&self) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.policy
            .revalidate()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if self.policy.policy().runtime() != sealed_static_issuer_runtime_measurement_v1() {
            return Err(IssuerProgramAdmissionErrorV1::RuntimePolicyMismatch);
        }
        if ProvisionedStaticExecutableMeasurementV1::from_issuer_policy(
            self.policy.policy().executable(),
        )? != self.issuer.measurement
        {
            return Err(IssuerProgramAdmissionErrorV1::PolicyImageMismatch);
        }
        self.launcher.revalidate()?;
        self.issuer.revalidate()?;
        if self.launcher.same_object_key(&self.issuer) {
            return Err(IssuerProgramAdmissionErrorV1::InvalidSealedImage(
                "aliased launcher and issuer",
            ));
        }
        Ok(())
    }

    /// Returns the authenticated launcher content measurement without exposing custody.
    pub const fn launcher_measurement(&self) -> ProvisionedStaticExecutableMeasurementV1 {
        self.launcher.measurement
    }

    /// Returns the caller-pinned issuer policy without exposing its sealed descriptor.
    pub const fn policy(
        &self,
    ) -> &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV1 {
        self.policy.policy()
    }

    /// Returns the inert launcher object identity used by the static pre-exec manifest.
    pub fn launcher_object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        self.launcher.object_identity()
    }

    /// Returns the inert issuer object identity used by the static pre-exec manifest.
    pub fn issuer_object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        self.issuer.object_identity()
    }

    pub(crate) fn try_clone_launcher_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.launcher.try_clone_for_launch()
    }

    pub(crate) fn try_clone_issuer_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.issuer.try_clone_for_launch()
    }

    pub(crate) fn try_clone_policy_for_launch(
        &self,
    ) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.policy
            .try_clone_for_transfer()
            .map_err(IssuerProgramAdmissionErrorV1::Policy)
    }

    pub(crate) fn revalidate_launcher_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.launcher.revalidate_clone(image)
    }

    pub(crate) fn revalidate_issuer_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.issuer.revalidate_clone(image)
    }

    pub(crate) fn revalidate_policy_clone(
        &self,
        image: &File,
    ) -> Result<(), IssuerProgramAdmissionErrorV1> {
        let transferred =
            image
                .try_clone()
                .map_err(|source| IssuerProgramAdmissionErrorV1::Io {
                    operation: "clone protected issuer policy for revalidation",
                    source,
                })?;
        let observed = CompilerExecutionPolicyCapabilityV1::from_file(transferred)
            .map_err(IssuerProgramAdmissionErrorV1::Policy)?;
        if observed.policy() != self.policy.policy() {
            return Err(IssuerProgramAdmissionErrorV1::PolicyImageMismatch);
        }
        Ok(())
    }
}

struct PinnedSealedStaticExecutableV1 {
    executable: ProtectedStaticExecutableV1,
    measurement: ProvisionedStaticExecutableMeasurementV1,
    role: &'static str,
}

impl PinnedSealedStaticExecutableV1 {
    fn same_object_key(&self, other: &Self) -> bool {
        let this = self.executable.object_identity();
        let other = other.executable.object_identity();
        this.device() == other.device() && this.inode() == other.inode()
    }

    fn admit(
        source: File,
        expected: ProvisionedStaticExecutableMeasurementV1,
        role: &'static str,
    ) -> Result<Self, IssuerProgramAdmissionErrorV1> {
        let executable = ProtectedStaticExecutableV1::seal_source_for_owner(
            source,
            protected_measurement(expected, role)?,
            ProtectedStaticExecutableOwnerV1::current(),
            role,
        )
        .map_err(|error| map_protected_executable_error(error, role))?;
        let admitted = Self {
            executable,
            measurement: expected,
            role,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    fn revalidate(&self) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.executable
            .revalidate()
            .map_err(|error| map_protected_executable_error(error, self.role))
    }

    fn object_identity(&self) -> StaticPreexecObjectIdentityV1 {
        let object = self.executable.object_identity();
        StaticPreexecObjectIdentityV1::new(
            object.device(),
            object.inode(),
            object.byte_len(),
            object.mode(),
        )
    }

    fn try_clone_for_launch(&self) -> Result<File, IssuerProgramAdmissionErrorV1> {
        self.executable
            .try_clone_for_exec()
            .map_err(|error| map_protected_executable_error(error, self.role))
    }

    fn revalidate_clone(&self, image: &File) -> Result<(), IssuerProgramAdmissionErrorV1> {
        self.executable
            .revalidate_exec_clone(image)
            .map_err(|error| map_protected_executable_error(error, self.role))
    }
}

fn protected_measurement(
    expected: ProvisionedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<ProtectedStaticExecutableMeasurementV1, IssuerProgramAdmissionErrorV1> {
    ProtectedStaticExecutableMeasurementV1::new(
        expected.sha256,
        expected.byte_len,
        MAX_PROVISIONED_EXECUTABLE_BYTES_V1,
    )
    .map_err(|error| match error {
        ProtectedStaticExecutableErrorV1::InvalidMeasurement => {
            IssuerProgramAdmissionErrorV1::InvalidMeasurement
        }
        error => map_protected_executable_error(error, role),
    })
}

fn map_protected_executable_error(
    error: ProtectedStaticExecutableErrorV1,
    fallback_role: &'static str,
) -> IssuerProgramAdmissionErrorV1 {
    match error {
        ProtectedStaticExecutableErrorV1::InvalidMeasurement
        | ProtectedStaticExecutableErrorV1::InvalidOwner => {
            IssuerProgramAdmissionErrorV1::InvalidMeasurement
        }
        ProtectedStaticExecutableErrorV1::OwnerTransition(role)
        | ProtectedStaticExecutableErrorV1::InvalidSource(role) => {
            IssuerProgramAdmissionErrorV1::InvalidSource(role)
        }
        ProtectedStaticExecutableErrorV1::SourceFileCapability(role) => {
            IssuerProgramAdmissionErrorV1::SourceFileCapability(role)
        }
        ProtectedStaticExecutableErrorV1::SourceChanged(role) => {
            IssuerProgramAdmissionErrorV1::SourceChanged(role)
        }
        ProtectedStaticExecutableErrorV1::MeasurementMismatch(role) => {
            IssuerProgramAdmissionErrorV1::MeasurementMismatch(role)
        }
        ProtectedStaticExecutableErrorV1::InvalidStaticImage { role, source } => {
            IssuerProgramAdmissionErrorV1::InvalidStaticImage { role, source }
        }
        ProtectedStaticExecutableErrorV1::InvalidSealedImage(role)
        | ProtectedStaticExecutableErrorV1::SealedFileCapability(role)
        | ProtectedStaticExecutableErrorV1::Changed(role) => {
            IssuerProgramAdmissionErrorV1::InvalidSealedImage(role)
        }
        ProtectedStaticExecutableErrorV1::Io { operation, source } => {
            IssuerProgramAdmissionErrorV1::Io { operation, source }
        }
        _ => IssuerProgramAdmissionErrorV1::InvalidSealedImage(fallback_role),
    }
}

#[cfg(test)]
mod tests;
