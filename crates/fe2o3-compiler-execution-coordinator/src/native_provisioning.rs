//! Pure, bounded construction of the complete native V3 public record graph.
#![forbid(unsafe_code)]

use std::{error::Error as StdError, fmt, mem::size_of};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as ANCHOR_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as ANCHOR_WORK,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3 as PROVISIONING_STORAGE,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3 as PROVISIONING_WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_STORAGE,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as SUPERVISOR_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
    CompilerExecutionAttestationErrorV3 as PolicyError,
    CompilerExecutionAttestationStorageV3 as NativeStorage,
    CompilerExecutionClientProfileErrorV3 as ProfileError,
    CompilerExecutionClientProfileV3 as Profile,
    CompilerExecutionExternalAnchorDeploymentErrorV3 as AnchorError,
    CompilerExecutionExternalAnchorDeploymentV3 as Anchor,
    CompilerExecutionExternalAnchorProvisioningErrorV3 as ProvisioningError,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentErrorV3 as SupervisorError,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as MAX_ISSUER,
    SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1, sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

const EXECUTABLE_ROLES: [&str; 5] = [
    "protected supervisor",
    "static pre-exec launcher",
    "compiler-execution issuer",
    "external-anchor provisioning helper",
    "external-anchor daemon",
];
const ENTRY_WORK: usize = 8;
// Ten fixed digest/length comparisons, the shared sealed-static runtime hash,
// and fixed metadata/control work. Native constructors charge their own quotas.
const LOCAL_WORK: usize = ENTRY_WORK
    + 10 * size_of::<Measurement>()
    + 32 * SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len()
    + 512;

/// Inert measured inputs, with one fixed slot per executable role.
///
/// These public values are not authenticated measurements or provisioning evidence.
/// The caller measures and pins the executables and public keys independently.
/// The bundle constructor validates role separation and the complete native record graph.
/// The shared measurement and service-identity primitives do not carry a policy family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionProvisioningInputsV3 {
    /// Nonzero deployment generation.
    pub generation: u64,
    /// Dedicated compiler-service UID, distinct from the anchor UID.
    pub compiler_service_uid: u32,
    /// Dedicated compiler-service GID.
    pub compiler_service_gid: u32,
    /// Already validated non-root anchor UID/GID.
    pub external_anchor_service: Service,
    /// Exact protected-supervisor executable measurement.
    pub supervisor: Measurement,
    /// Exact static pre-exec launcher measurement.
    pub launcher: Measurement,
    /// Exact issuer executable measurement.
    pub issuer: Measurement,
    /// Exact external-anchor provisioning-helper measurement.
    pub anchor_helper: Measurement,
    /// Exact external-anchor daemon measurement.
    pub anchor_daemon: Measurement,
    /// Public issuer key; no signing seed is accepted.
    pub issuer_verifying_key: [u8; 32],
    /// Distinct public anchor key; no signing seed is accepted.
    pub anchor_verifying_key: [u8; 32],
}

impl CompilerExecutionProvisioningInputsV3 {
    /// Full borrowed input floor, reserved before calling the bundle constructor.
    pub const fn retained_storage(&self) -> usize {
        size_of::<Self>()
    }
}

/// Full unreserved charge for the returned bundle; borrowed inputs stay prepaid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionProvisioningStorageV3(usize);

impl CompilerExecutionProvisioningStorageV3 {
    /// Reserve before retaining the bundle; release after dropping its full owner.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

use CompilerExecutionProvisioningBundleV3 as Bundle;
use CompilerExecutionProvisioningErrorV3 as Error;
use CompilerExecutionProvisioningInputsV3 as Inputs;
use CompilerExecutionProvisioningStorageV3 as Storage;
type Result<T> = std::result::Result<T, Error>;

/// Move-only, complete native V3 public deployment configuration.
///
/// The client profile owns the single policy; `policy()` borrows that exact owner.
/// Construction grants no signing, descriptor, process, compiler, publication, load,
/// launch or GPU authority and performs no I/O. No older-family owner is admitted.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::CompilerExecutionProvisioningBundleV3 as Bundle;
/// fn duplicate(bundle: Bundle) { let _ = bundle.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::{
///     CompilerExecutionProvisioningBundleV1 as Old,
///     CompilerExecutionProvisioningBundleV3 as Bundle,
/// };
/// fn upgrade(old: Old) -> Bundle { old.into() }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerExecutionProvisioningBundleV3 {
    client_profile: Profile,
    supervisor: Supervisor,
    anchor_deployment: Anchor,
    anchor_provisioning: Provisioning,
    retained: usize,
}

impl Bundle {
    /// Fixed metadata growth above the four full native owner charges.
    const GROWTH_STORAGE: usize = size_of::<(Self, Storage)>()
        - size_of::<Profile>()
        - size_of::<Supervisor>()
        - size_of::<Anchor>()
        - size_of::<Provisioning>();
    // Covers aggregate construction/move/result staging, measurements, SHA state,
    // receipts and bounded diagnostics. Nested native scratch is charged separately.
    const OUTER_STORAGE: usize = 4 * size_of::<(Self, Storage)>()
        + 2 * size_of::<Inputs>()
        + 5 * size_of::<Measurement>()
        + 5 * size_of::<NativeStorage>()
        + 8 * size_of::<Error>()
        + 2 * size_of::<sha2::Sha256>()
        + SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len()
        + 1024;

    /// Complete successful logical work, including every nested native operation.
    /// This is not an instruction or elapsed-time bound.
    pub const WORK: usize =
        LOCAL_WORK + POLICY_WORK + SUPERVISOR_WORK + ANCHOR_WORK + PROVISIONING_WORK + PROFILE_WORK;

    /// Conservative additional logical peak above the complete prepaid input floor.
    /// Each native envelope covers its returned owner as well as scratch, so their
    /// sum covers earlier retained records at every sequential construction step.
    /// This is not a heap-allocation, generated-stack or process-RSS bound.
    pub const SCRATCH: usize = Self::OUTER_STORAGE
        + POLICY_STORAGE
        + SUPERVISOR_STORAGE
        + ANCHOR_STORAGE
        + PROVISIONING_STORAGE
        + PROFILE_STORAGE;

    /// Builds matching native records from borrowed, fully prepaid measured inputs.
    ///
    /// Charges entry work before checking the input floor, then prepays local work
    /// and staging before inspecting inputs. All native calls share the original
    /// ledger. Every intermediate full owner (or consuming delta) is reserved before
    /// reuse. Success, error and unwind restore entry storage while preserving work,
    /// peak and first-denial history. The returned receipt is the FULL bundle charge.
    pub fn new(inputs: &Inputs, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(
            inputs.retained_storage(),
            ENTRY_WORK,
            LOCAL_WORK,
            Self::OUTER_STORAGE,
            |budget| {
                // Policy framing accepts larger inert measurements, but the root
                // runner stages the issuer under the launcher's fixed image cap.
                if inputs.issuer.byte_len() > MAX_ISSUER {
                    return Err(Error::IssuerImageTooLarge);
                }
                let measurements = [
                    inputs.supervisor,
                    inputs.launcher,
                    inputs.issuer,
                    inputs.anchor_helper,
                    inputs.anchor_daemon,
                ];
                for first in 0..measurements.len() {
                    for second in first + 1..measurements.len() {
                        if measurements[first] == measurements[second] {
                            return Err(Error::AliasedExecutableMeasurements {
                                first: EXECUTABLE_ROLES[first],
                                second: EXECUTABLE_ROLES[second],
                            });
                        }
                    }
                }
                let (policy, charge) = Policy::new(
                    inputs.generation,
                    inputs.issuer,
                    sealed_static_issuer_runtime_measurement_v1(),
                    inputs.issuer_verifying_key,
                    inputs.anchor_verifying_key,
                    budget,
                )
                .map_err(Error::Policy)?;
                budget.reserve_storage(charge.additional_storage())?;
                let (supervisor, charge) = Supervisor::new(
                    inputs.compiler_service_uid,
                    inputs.compiler_service_gid,
                    inputs.external_anchor_service,
                    inputs.supervisor,
                    inputs.launcher,
                    &policy,
                    budget,
                )
                .map_err(Error::SupervisorDeployment)?;
                budget.reserve_storage(charge.additional_storage())?;
                let (anchor_deployment, charge) =
                    Anchor::new(&supervisor, &policy, inputs.anchor_daemon, budget)
                        .map_err(Error::ExternalAnchorDeployment)?;
                budget.reserve_storage(charge.additional_storage())?;
                let (anchor_provisioning, charge) =
                    Provisioning::new(&anchor_deployment, inputs.anchor_helper, budget)
                        .map_err(Error::ExternalAnchorProvisioning)?;
                budget.reserve_storage(charge.additional_storage())?;

                // Move the prepaid policy into the profile after its final borrow.
                // Its existing reservation transfers; only the native delta is new.
                let (client_profile, charge) = Profile::new(
                    supervisor.service_uid(),
                    supervisor.service_gid(),
                    supervisor.external_anchor_service(),
                    policy,
                    budget,
                )
                .map_err(Error::ClientProfile)?;
                budget.reserve_storage(charge.additional_storage())?;
                let retained = client_profile
                    .retained_storage()
                    .checked_add(supervisor.retained_storage())
                    .and_then(|n| n.checked_add(anchor_deployment.retained_storage()))
                    .and_then(|n| n.checked_add(anchor_provisioning.retained_storage()))
                    .and_then(|n| n.checked_add(Self::GROWTH_STORAGE))
                    .ok_or(Resource::Arithmetic)?;
                Ok((
                    Self {
                        client_profile,
                        supervisor,
                        anchor_deployment,
                        anchor_provisioning,
                        retained,
                    },
                    Storage(retained),
                ))
            },
        )
    }

    /// Borrows the exact native issuer policy owned by the client profile.
    pub const fn policy(&self) -> &Policy {
        self.client_profile.policy()
    }
    /// Borrows the complete native client profile.
    pub const fn client_profile(&self) -> &Profile {
        &self.client_profile
    }
    /// Borrows the native protected-supervisor deployment.
    pub const fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }
    /// Borrows the native external-anchor deployment.
    pub const fn anchor_deployment(&self) -> &Anchor {
        &self.anchor_deployment
    }
    /// Borrows the native external-anchor provisioning record.
    pub const fn anchor_provisioning(&self) -> &Provisioning {
        &self.anchor_provisioning
    }
    /// Full charge, including all native owners, nested policy and metadata.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
}

/// Bounded construction refusal; shared primitives do not admit older policy families.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionProvisioningErrorV3 {
    /// Original ledger refusal or checked resource arithmetic failure.
    Resource(Resource),
    /// The issuer exceeds the native root runner's fixed executable ceiling.
    IssuerImageTooLarge,
    /// Two roles have the same exact digest and byte length.
    AliasedExecutableMeasurements {
        /// First conflicting role.
        first: &'static str,
        /// Second conflicting role.
        second: &'static str,
    },
    /// Native issuer-policy construction refused.
    Policy(PolicyError),
    /// Native client-profile construction refused.
    ClientProfile(ProfileError),
    /// Native supervisor construction refused.
    SupervisorDeployment(SupervisorError),
    /// Native external-anchor deployment construction refused.
    ExternalAnchorDeployment(AnchorError),
    /// Native external-anchor provisioning construction refused.
    ExternalAnchorProvisioning(ProvisioningError),
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::IssuerImageTooLarge => f.write_str("issuer exceeds the native executable limit"),
            Self::AliasedExecutableMeasurements { first, second } => {
                write!(f, "{first} and {second} have the same measurement")
            }
            Self::Policy(error) => write!(f, "invalid native issuer policy: {error}"),
            Self::ClientProfile(error) => write!(f, "invalid native client profile: {error}"),
            Self::SupervisorDeployment(error) => {
                write!(f, "invalid native supervisor deployment: {error}")
            }
            Self::ExternalAnchorDeployment(error) => {
                write!(f, "invalid native external-anchor deployment: {error}")
            }
            Self::ExternalAnchorProvisioning(error) => {
                write!(f, "invalid native external-anchor provisioning: {error}")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::AliasedExecutableMeasurements { .. } | Self::IssuerImageTooLarge => None,
            Self::Policy(error) => Some(error),
            Self::ClientProfile(error) => Some(error),
            Self::SupervisorDeployment(error) => Some(error),
            Self::ExternalAnchorDeployment(error) => Some(error),
            Self::ExternalAnchorProvisioning(error) => Some(error),
        }
    }
}

#[cfg(test)]
#[path = "native_provisioning_tests.rs"]
mod tests;
