//! Consuming repository-policy join for scalar-add Worker V2 evidence.

use core::fmt;

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InertCompilerHandoffExecutionV2, InertDecodedWorkerExchangeV2,
    InspectedPlironScalarAddV1Hsaco, PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES,
    PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES, PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT,
    PLIRON_SCALAR_ADD_V1_KERNARG_BYTES, PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY,
    PlironScalarAddV1AmdhsaDescriptorIdentity, PlironScalarAddV1InspectionError,
    PlironScalarAddV1MachineIdentity, WorkerMeasurementV1, WorkerProtocolError, WorkerRequestV2,
    WorkerResponseV2, WorkerStageV1, inspect_pliron_scalar_add_v1_hsaco,
};
use fe2o3_pliron_worker_v2::InertScalarAddWorkerRequestV2;
use sha2::{Digest as _, Sha256};

use crate::source::CanonicalSourceObservationV1;

const LINEAGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/OPAQUE-LINEAGE/V1\0";
const OBSERVATION_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/EXECUTION-OBSERVATION/V1\0";
const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PLIRON-SCALAR-ADD-V1/WORKER-RESPONSE/V2\0";
const FINALIZATION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PLIRON-SCALAR-ADD-V1/REPOSITORY-FINALIZATION/V1\0";

/// One exact pin in the checkout-embedded approval profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RepositoryApprovalFieldV1 {
    /// Exact deterministic V2 source handoff identity.
    SourceHandoffIdentity,
    /// Exact deterministic LLVM assembly content identity.
    AssemblyIdentity,
    /// Exact canonical compiler-handoff identity.
    CompilerHandoffIdentity,
    /// Exact compiler symbol-manifest identity.
    SymbolManifestIdentity,
    /// Exact worker executable content identity.
    WorkerExecutable,
    /// Build identity embedded in the approved executable.
    WorkerBuildIdentity,
    /// Pinned upstream LLVM identity embedded in the approved executable.
    LlvmBuildIdentity,
    /// Exact complete HSACO content identity.
    OutputIdentity,
    /// Exact 64-byte AMDHSA descriptor identity.
    DescriptorIdentity,
    /// Exact bound machine-code identity.
    MachineIdentity,
}

/// One field rejected while joining opaque lineage and exact execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ScalarAddLineageFieldV1 {
    /// The execution attempt differs from the opaque request lineage.
    Attempt,
    /// The consumed compiler-handoff identity differs from the lineage.
    TransactionHandoffIdentity,
    /// The execution used a different worker executable.
    WorkerExecutable,
    /// Canonical decoding did not reproduce the sealed request and response.
    SealedExchange,
    /// The response worker build identity differs from the measured executable.
    WorkerBuildIdentity,
    /// The worker did not report complete execution.
    CompletionStage,
    /// The response introduced a provider or secondary response identity.
    ProviderClosure,
    /// The output is absent or not exactly request-bound.
    OutputBinding,
    /// The deterministic structured diagnostic closure changed.
    Diagnostics,
}

/// Failure from the repository-policy scalar-add join.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarAddFinalizationErrorV1 {
    /// An observed worker, toolchain, or artifact field differs from repository policy.
    PolicyMismatch(RepositoryApprovalFieldV1),
    /// Opaque lineage and measured execution do not join exactly.
    Lineage(ScalarAddLineageFieldV1),
    /// Canonical Worker V2 decoding rejected the exchange.
    WorkerProtocol(WorkerProtocolError),
    /// Bounded low-level HSACO inspection rejected the output.
    HsacoInspection(PlironScalarAddV1InspectionError),
}

impl fmt::Display for ScalarAddFinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyMismatch(field) => {
                write!(
                    formatter,
                    "scalar-add observation differs from repository policy at {field:?}"
                )
            }
            Self::Lineage(field) => write!(formatter, "scalar-add lineage changed at {field:?}"),
            Self::WorkerProtocol(error) => write!(formatter, "Worker V2 exchange failed: {error}"),
            Self::HsacoInspection(error) => write!(formatter, "HSACO inspection failed: {error}"),
        }
    }
}

impl std::error::Error for ScalarAddFinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkerProtocol(error) => Some(error),
            Self::HsacoInspection(error) => Some(error),
            Self::PolicyMismatch(_) | Self::Lineage(_) => None,
        }
    }
}

/// Stable identity of the checked-in repository approval manifest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryApprovalIdentityV1([u8; 32]);

impl RepositoryApprovalIdentityV1 {
    pub(crate) const fn from_manifest_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Returns the SHA-256 digest of the complete checked-in manifest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The sole checkout-embedded policy profile for this qualification slice.
///
/// ```compile_fail
/// use fe2o3_pliron_scalar_add_v1::RepositoryScalarAddProfileV1;
/// let _ = RepositoryScalarAddProfileV1::from_observed_pins();
/// ```
///
/// The type has no public raw-pin constructor, decoder, `From`, or `TryFrom`
/// implementation. Callers can obtain it only through
/// [`crate::repository_profile_v1`].
pub struct RepositoryScalarAddProfileV1 {
    identity: RepositoryApprovalIdentityV1,
    source: CanonicalSourceObservationV1,
    worker_executable: ContentIdentityV1,
    worker_build_identity: String,
    llvm_build_identity: String,
    runtime_implementation: String,
    runtime_version: String,
    runtime_image_sha256: [u8; 32],
    output_identity: ContentIdentityV1,
    descriptor_identity: PlironScalarAddV1AmdhsaDescriptorIdentity,
    machine_identity: PlironScalarAddV1MachineIdentity,
}

impl fmt::Debug for RepositoryScalarAddProfileV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryScalarAddProfileV1")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl RepositoryScalarAddProfileV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_repository_manifest(
        identity: RepositoryApprovalIdentityV1,
        source: CanonicalSourceObservationV1,
        worker_executable: ContentIdentityV1,
        worker_build_identity: String,
        llvm_build_identity: String,
        runtime_implementation: String,
        runtime_version: String,
        runtime_image_sha256: [u8; 32],
        output_identity: ContentIdentityV1,
        descriptor_identity: PlironScalarAddV1AmdhsaDescriptorIdentity,
        machine_identity: PlironScalarAddV1MachineIdentity,
    ) -> Self {
        Self {
            identity,
            source,
            worker_executable,
            worker_build_identity,
            llvm_build_identity,
            runtime_implementation,
            runtime_version,
            runtime_image_sha256,
            output_identity,
            descriptor_identity,
            machine_identity,
        }
    }

    /// Returns the identity of the complete repository manifest.
    pub const fn identity(&self) -> RepositoryApprovalIdentityV1 {
        self.identity
    }

    /// Returns deterministic source identities pinned at compile time in this checkout.
    pub const fn canonical_source(&self) -> CanonicalSourceObservationV1 {
        self.source
    }

    /// Returns the exact approved worker executable identity.
    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.worker_executable
    }

    /// Returns the approved build identity embedded in the worker executable.
    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    /// Returns the approved LLVM identity embedded in the worker executable.
    pub fn llvm_build_identity(&self) -> &str {
        &self.llvm_build_identity
    }

    /// Returns the exact repository-pinned HSA runtime implementation.
    pub fn runtime_implementation(&self) -> &str {
        &self.runtime_implementation
    }

    /// Returns the exact repository-pinned HSA runtime version.
    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    /// Returns the exact repository-pinned loaded ROCr/HIP image digest.
    pub const fn runtime_image_sha256(&self) -> &[u8; 32] {
        &self.runtime_image_sha256
    }

    /// Returns the exact approved HSACO identity.
    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity
    }

    /// Returns the exact approved descriptor identity.
    pub const fn descriptor_identity(&self) -> PlironScalarAddV1AmdhsaDescriptorIdentity {
        self.descriptor_identity
    }

    /// Returns the exact approved machine-code identity.
    pub const fn machine_identity(&self) -> PlironScalarAddV1MachineIdentity {
        self.machine_identity
    }

    /// Checks an observed executable digest and its reported build identities.
    ///
    /// This is exact equality against compile-time pins, not signature or
    /// external provenance authentication.
    pub fn matches_embedded_worker_pins(
        &self,
        executable: ContentIdentityV1,
        worker_build_identity: &str,
        llvm_build_identity: &str,
    ) -> bool {
        executable == self.worker_executable
            && worker_build_identity == self.worker_build_identity
            && llvm_build_identity == self.llvm_build_identity
    }
}

/// Stable identity of the opaque source-to-request lineage consumed here.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarAddLineageIdentityV1([u8; 32]);

impl ScalarAddLineageIdentityV1 {
    #[cfg(test)]
    pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Returns the canonical lineage digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of the measured execution observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarAddObservationIdentityV1([u8; 32]);

impl ScalarAddObservationIdentityV1 {
    #[cfg(test)]
    pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Returns the canonical observation digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Measured worker and artifact facts retained separately from repository policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRepositoryScalarAddV1 {
    identity: ScalarAddObservationIdentityV1,
    worker: WorkerMeasurementV1,
    response_identity: [u8; 32],
    output_identity: ContentIdentityV1,
    descriptor_identity: PlironScalarAddV1AmdhsaDescriptorIdentity,
    machine_identity: PlironScalarAddV1MachineIdentity,
}

impl ObservedRepositoryScalarAddV1 {
    /// Returns the complete observation identity.
    pub const fn identity(&self) -> ScalarAddObservationIdentityV1 {
        self.identity
    }

    /// Returns the measured worker facts; these facts cannot construct policy.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    /// Returns the canonical response identity.
    pub const fn response_identity(&self) -> &[u8; 32] {
        &self.response_identity
    }

    /// Returns the observed complete HSACO identity.
    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.output_identity
    }

    /// Returns the observed descriptor identity.
    pub const fn descriptor_identity(&self) -> PlironScalarAddV1AmdhsaDescriptorIdentity {
        self.descriptor_identity
    }

    /// Returns the observed machine-code identity.
    pub const fn machine_identity(&self) -> PlironScalarAddV1MachineIdentity {
        self.machine_identity
    }
}

/// Stable identity of one repository-policy-bound inert finalization receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedRepositoryScalarAddV1Identity([u8; 32]);

impl FinalizedRepositoryScalarAddV1Identity {
    #[cfg(test)]
    pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    /// Returns the canonical finalization digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert repository-policy-bound scalar-add finalization receipt.
///
/// ```compile_fail
/// use fe2o3_pliron_scalar_add_v1::FinalizedRepositoryScalarAddV1;
/// fn clone_receipt(receipt: FinalizedRepositoryScalarAddV1) {
///     let _ = receipt.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron_scalar_add_v1::FinalizedRepositoryScalarAddV1;
/// fn expose_bytes(receipt: &FinalizedRepositoryScalarAddV1) {
///     let _ = receipt.bytes();
/// }
/// ```
///
/// This type is not `Clone`, exposes no artifact bytes or callback, and grants
/// no publication, load, or launch authority. The crate's one concrete runtime
/// transition consumes it and obtains bytes only from the retained execution.
pub struct FinalizedRepositoryScalarAddV1 {
    identity: FinalizedRepositoryScalarAddV1Identity,
    lineage_identity: ScalarAddLineageIdentityV1,
    policy: RepositoryScalarAddProfileV1,
    observation: ObservedRepositoryScalarAddV1,
    #[allow(dead_code)]
    pub(crate) lineage: InertScalarAddWorkerRequestV2,
    #[allow(dead_code)]
    pub(crate) execution: InertCompilerHandoffExecutionV2,
}

impl fmt::Debug for FinalizedRepositoryScalarAddV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalizedRepositoryScalarAddV1")
            .field("identity", &self.identity)
            .field("lineage", &self.lineage_identity)
            .field("policy", &self.policy.identity())
            .field("observation", &self.observation.identity())
            .finish_non_exhaustive()
    }
}

impl FinalizedRepositoryScalarAddV1 {
    /// Returns the complete finalization identity.
    pub const fn identity(&self) -> FinalizedRepositoryScalarAddV1Identity {
        self.identity
    }

    /// Returns the identity derived only from the consumed opaque lineage.
    pub const fn lineage_identity(&self) -> ScalarAddLineageIdentityV1 {
        self.lineage_identity
    }

    /// Returns the repository policy retained separately from observation.
    pub const fn approval_policy(&self) -> &RepositoryScalarAddProfileV1 {
        &self.policy
    }

    /// Returns the measured facts retained separately from policy.
    pub const fn observation(&self) -> &ObservedRepositoryScalarAddV1 {
        &self.observation
    }

    /// Returns the caller-populated kernarg prefix size.
    pub const fn explicit_kernarg_bytes(&self) -> u64 {
        PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES
    }

    /// Returns the runtime-populated COV6 kernarg suffix size.
    pub const fn implicit_kernarg_bytes(&self) -> u64 {
        PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES
    }

    /// Returns the complete COV6 kernarg segment size.
    pub const fn total_kernarg_bytes(&self) -> u64 {
        PLIRON_SCALAR_ADD_V1_KERNARG_BYTES
    }

    /// Returns the kernarg segment alignment.
    pub const fn kernarg_alignment(&self) -> u64 {
        PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT
    }

    /// Returns the only admitted grid dimensions.
    pub const fn grid_dimensions(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    /// Returns the only admitted workgroup dimensions.
    pub const fn workgroup_dimensions(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    /// Returns the only admitted dynamic LDS byte count.
    pub const fn dynamic_lds_bytes(&self) -> u32 {
        0
    }

    /// This receipt grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// This receipt grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This receipt grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Confirms that the retained execution is the attempt admitted by lineage.
    pub fn retains_exact_execution(&self) -> bool {
        self.execution.attempt() == self.lineage.attempt()
            && self.execution.handoff_identity() == self.lineage.transaction_handoff_identity()
            && self.execution.worker_executable() == self.lineage.worker_measurement().executable()
    }

    pub(crate) fn into_runtime_authority(self) -> RepositoryRuntimeAuthorityV1 {
        RepositoryRuntimeAuthorityV1 {
            identity: self.identity,
            lineage_identity: self.lineage_identity,
            policy: self.policy,
            observation: self.observation,
            _lineage: self.lineage,
            execution: self.execution,
        }
    }
}

pub(crate) struct RepositoryRuntimeAuthorityV1 {
    pub(crate) identity: FinalizedRepositoryScalarAddV1Identity,
    pub(crate) lineage_identity: ScalarAddLineageIdentityV1,
    pub(crate) policy: RepositoryScalarAddProfileV1,
    pub(crate) observation: ObservedRepositoryScalarAddV1,
    pub(crate) _lineage: InertScalarAddWorkerRequestV2,
    pub(crate) execution: InertCompilerHandoffExecutionV2,
}

/// Consumes opaque Pliron lineage, one exact Worker V2 execution, and checkout policy.
///
/// Callers cannot supply source, compiler-handoff, request, response, or output
/// identities independently. The checked-in policy is a distinct capability;
/// observed identities cannot construct it.
pub fn finalize_repository_scalar_add_v1(
    lineage: InertScalarAddWorkerRequestV2,
    execution: InertCompilerHandoffExecutionV2,
    policy: RepositoryScalarAddProfileV1,
) -> Result<FinalizedRepositoryScalarAddV1, ScalarAddFinalizationErrorV1> {
    validate_source_policy(&policy, &lineage)?;
    validate_worker_policy(&policy, lineage.worker_measurement())?;
    validate_execution_lineage(&lineage, &execution)?;

    let exchange = InertDecodedWorkerExchangeV2::decode(
        lineage.sealed_request().canonical_bytes(),
        execution.response().canonical_bytes(),
    )
    .map_err(ScalarAddFinalizationErrorV1::WorkerProtocol)?;
    validate_exchange_identity(&lineage, &execution, &exchange)?;
    let output = validate_response(
        lineage.worker_measurement(),
        exchange.request(),
        exchange.response(),
    )?;
    let inspected = inspect_pliron_scalar_add_v1_hsaco(output.bytes())
        .map_err(ScalarAddFinalizationErrorV1::HsacoInspection)?;
    validate_artifact_policy(&policy, &inspected)?;

    let lineage_identity = calculate_lineage_identity(&lineage);
    let response_identity = domain_hash(
        RESPONSE_IDENTITY_DOMAIN,
        execution.response().canonical_bytes(),
    );
    let observation = observed_worker(lineage.worker_measurement(), response_identity, &inspected);
    let identity = calculate_finalization_identity(
        lineage_identity,
        policy.identity(),
        observation.identity(),
    );
    Ok(FinalizedRepositoryScalarAddV1 {
        identity,
        lineage_identity,
        policy,
        observation,
        lineage,
        execution,
    })
}

#[derive(Clone, Copy)]
struct SourcePolicyFactsV1 {
    handoff: [u8; 32],
    assembly: ContentIdentityV1,
    compiler_handoff: ContentIdentityV1,
    symbol_manifest: ContentIdentityV1,
}

fn validate_source_policy(
    policy: &RepositoryScalarAddProfileV1,
    lineage: &InertScalarAddWorkerRequestV2,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    let compiler = lineage.compiler_handoff_identity();
    let symbols = lineage.manifest_identity();
    validate_source_policy_facts(
        policy,
        SourcePolicyFactsV1 {
            handoff: *lineage.source_identity().as_bytes(),
            assembly: ContentIdentityV1::from_parts(
                *lineage.assembly_sha256().as_bytes(),
                lineage.assembly_len(),
            ),
            compiler_handoff: ContentIdentityV1::from_parts(
                *compiler.sha256(),
                compiler.byte_len(),
            ),
            symbol_manifest: ContentIdentityV1::from_parts(*symbols.sha256(), symbols.byte_len()),
        },
    )
}

fn validate_source_policy_facts(
    policy: &RepositoryScalarAddProfileV1,
    observed: SourcePolicyFactsV1,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    let source = policy.source;
    let mismatch = if observed.handoff != *source.v2_handoff_identity() {
        Some(RepositoryApprovalFieldV1::SourceHandoffIdentity)
    } else if observed.assembly != source.assembly_identity() {
        Some(RepositoryApprovalFieldV1::AssemblyIdentity)
    } else if observed.compiler_handoff != source.compiler_handoff_identity() {
        Some(RepositoryApprovalFieldV1::CompilerHandoffIdentity)
    } else if observed.symbol_manifest != source.symbol_manifest_identity() {
        Some(RepositoryApprovalFieldV1::SymbolManifestIdentity)
    } else {
        None
    };
    mismatch.map_or(Ok(()), |field| {
        Err(ScalarAddFinalizationErrorV1::PolicyMismatch(field))
    })
}

fn validate_worker_policy(
    policy: &RepositoryScalarAddProfileV1,
    observed: &WorkerMeasurementV1,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    validate_policy_facts(
        policy,
        PolicyFactsV1 {
            worker_executable: observed.executable(),
            worker_build_identity: observed.worker_build_identity(),
            llvm_build_identity: observed.llvm_build_identity(),
            output_identity: policy.output_identity,
            descriptor_identity: policy.descriptor_identity,
            machine_identity: policy.machine_identity,
        },
    )
}

fn validate_artifact_policy(
    policy: &RepositoryScalarAddProfileV1,
    inspected: &InspectedPlironScalarAddV1Hsaco,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    validate_policy_facts(
        policy,
        PolicyFactsV1 {
            worker_executable: policy.worker_executable,
            worker_build_identity: &policy.worker_build_identity,
            llvm_build_identity: &policy.llvm_build_identity,
            output_identity: inspected.output_identity(),
            descriptor_identity: inspected.descriptor_identity(),
            machine_identity: inspected.machine_identity(),
        },
    )
}

#[derive(Clone, Copy)]
struct PolicyFactsV1<'a> {
    worker_executable: ContentIdentityV1,
    worker_build_identity: &'a str,
    llvm_build_identity: &'a str,
    output_identity: ContentIdentityV1,
    descriptor_identity: PlironScalarAddV1AmdhsaDescriptorIdentity,
    machine_identity: PlironScalarAddV1MachineIdentity,
}

fn validate_policy_facts(
    policy: &RepositoryScalarAddProfileV1,
    observed: PolicyFactsV1<'_>,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    let mismatch = if observed.worker_executable != policy.worker_executable {
        Some(RepositoryApprovalFieldV1::WorkerExecutable)
    } else if observed.worker_build_identity != policy.worker_build_identity {
        Some(RepositoryApprovalFieldV1::WorkerBuildIdentity)
    } else if observed.llvm_build_identity != policy.llvm_build_identity {
        Some(RepositoryApprovalFieldV1::LlvmBuildIdentity)
    } else if observed.output_identity != policy.output_identity {
        Some(RepositoryApprovalFieldV1::OutputIdentity)
    } else if observed.descriptor_identity != policy.descriptor_identity {
        Some(RepositoryApprovalFieldV1::DescriptorIdentity)
    } else if observed.machine_identity != policy.machine_identity {
        Some(RepositoryApprovalFieldV1::MachineIdentity)
    } else {
        None
    };
    mismatch.map_or(Ok(()), |field| {
        Err(ScalarAddFinalizationErrorV1::PolicyMismatch(field))
    })
}

fn validate_execution_lineage(
    lineage: &InertScalarAddWorkerRequestV2,
    execution: &InertCompilerHandoffExecutionV2,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    validate_lineage_facts(LineageFactsV1 {
        attempt: execution.attempt() == lineage.attempt(),
        transaction_handoff_identity: execution.handoff_identity()
            == lineage.transaction_handoff_identity(),
        worker_executable: execution.worker_executable()
            == lineage.worker_measurement().executable(),
        sealed_exchange: true,
        worker_build_identity: true,
        completion_stage: true,
        provider_closure: true,
        output_binding: true,
        diagnostics: true,
    })
}

fn validate_exchange_identity(
    lineage: &InertScalarAddWorkerRequestV2,
    execution: &InertCompilerHandoffExecutionV2,
    exchange: &InertDecodedWorkerExchangeV2,
) -> Result<(), ScalarAddFinalizationErrorV1> {
    validate_lineage_facts(LineageFactsV1 {
        attempt: true,
        transaction_handoff_identity: true,
        worker_executable: true,
        sealed_exchange: exchange.request() == lineage.sealed_request()
            && exchange.response() == execution.response(),
        worker_build_identity: true,
        completion_stage: true,
        provider_closure: true,
        output_binding: true,
        diagnostics: true,
    })
}

fn validate_response<'a>(
    observed: &WorkerMeasurementV1,
    request: &WorkerRequestV2,
    response: &'a WorkerResponseV2,
) -> Result<&'a fe2o3_hsaco_finalize::WorkerOutputV2, ScalarAddFinalizationErrorV1> {
    let output = response.output();
    validate_lineage_facts(LineageFactsV1 {
        attempt: true,
        transaction_handoff_identity: true,
        worker_executable: true,
        sealed_exchange: response.binds_request(request),
        worker_build_identity: response.worker_build_identity() == observed.worker_build_identity(),
        completion_stage: response.stage() == WorkerStageV1::Complete,
        provider_closure: response.device_library_provider().is_none()
            && response.response_identity().is_none(),
        output_binding: output.is_some_and(|output| {
            output.request_identity() == request.identity()
                && output.compiler_envelope_identity() == request.compiler_envelope_identity()
                && output.identity().matches(output.bytes())
                && output.identity().byte_len() == request.output_constraints().max_bytes()
        }),
        diagnostics: output.is_some_and(|output| {
            response.diagnostics()
                == exact_diagnostics(request.compiler_module().bytes(), output.bytes()).as_slice()
        }),
    })?;
    output.ok_or(ScalarAddFinalizationErrorV1::Lineage(
        ScalarAddLineageFieldV1::OutputBinding,
    ))
}

#[derive(Clone, Copy)]
struct LineageFactsV1 {
    attempt: bool,
    transaction_handoff_identity: bool,
    worker_executable: bool,
    sealed_exchange: bool,
    worker_build_identity: bool,
    completion_stage: bool,
    provider_closure: bool,
    output_binding: bool,
    diagnostics: bool,
}

fn validate_lineage_facts(facts: LineageFactsV1) -> Result<(), ScalarAddFinalizationErrorV1> {
    let mismatch = if !facts.attempt {
        Some(ScalarAddLineageFieldV1::Attempt)
    } else if !facts.transaction_handoff_identity {
        Some(ScalarAddLineageFieldV1::TransactionHandoffIdentity)
    } else if !facts.worker_executable {
        Some(ScalarAddLineageFieldV1::WorkerExecutable)
    } else if !facts.sealed_exchange {
        Some(ScalarAddLineageFieldV1::SealedExchange)
    } else if !facts.worker_build_identity {
        Some(ScalarAddLineageFieldV1::WorkerBuildIdentity)
    } else if !facts.completion_stage {
        Some(ScalarAddLineageFieldV1::CompletionStage)
    } else if !facts.provider_closure {
        Some(ScalarAddLineageFieldV1::ProviderClosure)
    } else if !facts.output_binding {
        Some(ScalarAddLineageFieldV1::OutputBinding)
    } else if !facts.diagnostics {
        Some(ScalarAddLineageFieldV1::Diagnostics)
    } else {
        None
    };
    mismatch.map_or(Ok(()), |field| {
        Err(ScalarAddFinalizationErrorV1::Lineage(field))
    })
}

fn observed_worker(
    worker: &WorkerMeasurementV1,
    response_identity: [u8; 32],
    inspected: &InspectedPlironScalarAddV1Hsaco,
) -> ObservedRepositoryScalarAddV1 {
    let identity = calculate_observation_identity(
        worker,
        response_identity,
        inspected.output_identity(),
        inspected.descriptor_identity(),
        inspected.machine_identity(),
    );
    ObservedRepositoryScalarAddV1 {
        identity,
        worker: worker.clone(),
        response_identity,
        output_identity: inspected.output_identity(),
        descriptor_identity: inspected.descriptor_identity(),
        machine_identity: inspected.machine_identity(),
    }
}

fn exact_diagnostics(module: &[u8], output: &[u8]) -> Vec<String> {
    let module_sha = hex(Sha256::digest(module).into());
    let output_sha = hex(Sha256::digest(output).into());
    let mut diagnostics = vec![
        "post_link.check=exports status=ok symbols=[scalar_add,scalar_add.kd]".to_owned(),
        "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-".to_owned(),
        format!(
            "post_link.check=pliron_scalar_add_v1_profile status=ok kernel=scalar_add required_workgroup=absent max_flat_workgroup_size=64 wavefront_size=64 kernarg_size=280 explicit_kernarg_size=24 hidden_kernarg_size=256 kernarg_align=8 group_size=0 private_size=0 sgpr_spills=0 vgpr_spills=0 dynamic_stack=false machine_calls=0 machine_branches=0 machine_atomics=0 machine_scratch=0 relocations=0 dynamic_dependencies=0 llvm_build_identity={PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY} input_ir_sha256={module_sha} raw_hsaco_sha256={output_sha}"
        ),
        "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c"
            .to_owned(),
        "post_link.check=unresolved status=ok symbols=[]".to_owned(),
        "post_link.kernel name=scalar_add symbol=scalar_add.kd kernarg_size=280 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=absent".to_owned(),
    ];
    diagnostics.sort();
    diagnostics
}

fn calculate_lineage_identity(
    lineage: &InertScalarAddWorkerRequestV2,
) -> ScalarAddLineageIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(LINEAGE_IDENTITY_DOMAIN);
    digest.update(lineage.source_identity().as_bytes());
    digest.update(lineage.assembly_sha256().as_bytes());
    digest.update(lineage.assembly_len().to_le_bytes());
    digest.update(lineage.compiler_handoff_identity().sha256());
    digest.update(lineage.compiler_handoff_identity().byte_len().to_le_bytes());
    digest.update(lineage.transaction_handoff_identity().as_bytes());
    digest.update(lineage.manifest_identity().sha256());
    digest.update(lineage.manifest_identity().byte_len().to_le_bytes());
    digest.update(lineage.plan_identity().as_bytes());
    let attempt = lineage.attempt();
    digest.update(attempt.generation().to_le_bytes());
    digest.update(attempt.session().as_bytes());
    digest.update(attempt.invocation().as_bytes());
    hash_content(&mut digest, lineage.worker_measurement().executable());
    hash_text(
        &mut digest,
        lineage.worker_measurement().worker_build_identity(),
    );
    hash_text(
        &mut digest,
        lineage.worker_measurement().llvm_build_identity(),
    );
    digest.update(lineage.request_id());
    digest.update(lineage.request_identity());
    ScalarAddLineageIdentityV1(digest.finalize().into())
}

fn calculate_observation_identity(
    worker: &WorkerMeasurementV1,
    response_identity: [u8; 32],
    output_identity: ContentIdentityV1,
    descriptor_identity: PlironScalarAddV1AmdhsaDescriptorIdentity,
    machine_identity: PlironScalarAddV1MachineIdentity,
) -> ScalarAddObservationIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(OBSERVATION_IDENTITY_DOMAIN);
    hash_content(&mut digest, worker.executable());
    hash_text(&mut digest, worker.worker_build_identity());
    hash_text(&mut digest, worker.llvm_build_identity());
    digest.update(response_identity);
    hash_content(&mut digest, output_identity);
    digest.update(descriptor_identity.as_bytes());
    digest.update(machine_identity.as_bytes());
    ScalarAddObservationIdentityV1(digest.finalize().into())
}

fn calculate_finalization_identity(
    lineage: ScalarAddLineageIdentityV1,
    policy: RepositoryApprovalIdentityV1,
    observation: ScalarAddObservationIdentityV1,
) -> FinalizedRepositoryScalarAddV1Identity {
    let mut digest = Sha256::new();
    digest.update(FINALIZATION_IDENTITY_DOMAIN);
    digest.update(lineage.as_bytes());
    digest.update(policy.as_bytes());
    digest.update(observation.as_bytes());
    FinalizedRepositoryScalarAddV1Identity(digest.finalize().into())
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn hash_content(digest: &mut Sha256, identity: ContentIdentityV1) {
    digest.update(identity.sha256());
    digest.update(identity.byte_len().to_le_bytes());
}

fn hash_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
}

fn hex(bytes: [u8; 32]) -> String {
    use fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    const LLVM: &str = PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY;
    const BUILD: &str = "fe2o3-worker-v1-sha256-test";

    fn fixture_policy() -> RepositoryScalarAddProfileV1 {
        RepositoryScalarAddProfileV1::from_repository_manifest(
            RepositoryApprovalIdentityV1::from_manifest_digest([7; 32]),
            crate::canonical_source_observation_v1().unwrap(),
            ContentIdentityV1::from_parts([1; 32], 101),
            BUILD.to_owned(),
            LLVM.to_owned(),
            "ROCr HSA".to_owned(),
            "1.18".to_owned(),
            [8; 32],
            ContentIdentityV1::from_parts([2; 32], 202),
            PlironScalarAddV1AmdhsaDescriptorIdentity::from_bytes([3; 32]),
            PlironScalarAddV1MachineIdentity::from_bytes([4; 32]),
        )
    }

    fn matching_source_facts(policy: &RepositoryScalarAddProfileV1) -> SourcePolicyFactsV1 {
        SourcePolicyFactsV1 {
            handoff: *policy.source.v2_handoff_identity(),
            assembly: policy.source.assembly_identity(),
            compiler_handoff: policy.source.compiler_handoff_identity(),
            symbol_manifest: policy.source.symbol_manifest_identity(),
        }
    }

    #[test]
    fn every_deterministic_source_substitution_has_an_exact_category() {
        let policy = fixture_policy();
        let cases = [
            RepositoryApprovalFieldV1::SourceHandoffIdentity,
            RepositoryApprovalFieldV1::AssemblyIdentity,
            RepositoryApprovalFieldV1::CompilerHandoffIdentity,
            RepositoryApprovalFieldV1::SymbolManifestIdentity,
        ];
        for expected in cases {
            let mut facts = matching_source_facts(&policy);
            match expected {
                RepositoryApprovalFieldV1::SourceHandoffIdentity => facts.handoff[0] ^= 1,
                RepositoryApprovalFieldV1::AssemblyIdentity => {
                    facts.assembly = ContentIdentityV1::from_parts([9; 32], 1);
                }
                RepositoryApprovalFieldV1::CompilerHandoffIdentity => {
                    facts.compiler_handoff = ContentIdentityV1::from_parts([9; 32], 1);
                }
                RepositoryApprovalFieldV1::SymbolManifestIdentity => {
                    facts.symbol_manifest = ContentIdentityV1::from_parts([9; 32], 1);
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_source_policy_facts(&policy, facts),
                Err(ScalarAddFinalizationErrorV1::PolicyMismatch(field)) if field == expected
            ));
        }
    }

    fn matching_policy_facts<'a>(policy: &'a RepositoryScalarAddProfileV1) -> PolicyFactsV1<'a> {
        PolicyFactsV1 {
            worker_executable: policy.worker_executable,
            worker_build_identity: &policy.worker_build_identity,
            llvm_build_identity: &policy.llvm_build_identity,
            output_identity: policy.output_identity,
            descriptor_identity: policy.descriptor_identity,
            machine_identity: policy.machine_identity,
        }
    }

    #[test]
    fn every_policy_substitution_has_an_exact_category() {
        let policy = fixture_policy();
        let cases = [
            RepositoryApprovalFieldV1::WorkerExecutable,
            RepositoryApprovalFieldV1::WorkerBuildIdentity,
            RepositoryApprovalFieldV1::LlvmBuildIdentity,
            RepositoryApprovalFieldV1::OutputIdentity,
            RepositoryApprovalFieldV1::DescriptorIdentity,
            RepositoryApprovalFieldV1::MachineIdentity,
        ];
        for expected in cases {
            let mut facts = matching_policy_facts(&policy);
            match expected {
                RepositoryApprovalFieldV1::SourceHandoffIdentity
                | RepositoryApprovalFieldV1::AssemblyIdentity
                | RepositoryApprovalFieldV1::CompilerHandoffIdentity
                | RepositoryApprovalFieldV1::SymbolManifestIdentity => unreachable!(),
                RepositoryApprovalFieldV1::WorkerExecutable => {
                    facts.worker_executable = ContentIdentityV1::from_parts([9; 32], 101);
                }
                RepositoryApprovalFieldV1::WorkerBuildIdentity => {
                    facts.worker_build_identity = "substituted-worker-build";
                }
                RepositoryApprovalFieldV1::LlvmBuildIdentity => {
                    facts.llvm_build_identity = "substituted-llvm-build";
                }
                RepositoryApprovalFieldV1::OutputIdentity => {
                    facts.output_identity = ContentIdentityV1::from_parts([9; 32], 202);
                }
                RepositoryApprovalFieldV1::DescriptorIdentity => {
                    facts.descriptor_identity =
                        PlironScalarAddV1AmdhsaDescriptorIdentity::from_bytes([9; 32]);
                }
                RepositoryApprovalFieldV1::MachineIdentity => {
                    facts.machine_identity = PlironScalarAddV1MachineIdentity::from_bytes([9; 32]);
                }
            }
            assert!(matches!(
                validate_policy_facts(&policy, facts),
                Err(ScalarAddFinalizationErrorV1::PolicyMismatch(field)) if field == expected
            ));
        }
    }

    #[test]
    fn every_lineage_substitution_has_an_exact_category() {
        let cases = [
            ScalarAddLineageFieldV1::Attempt,
            ScalarAddLineageFieldV1::TransactionHandoffIdentity,
            ScalarAddLineageFieldV1::WorkerExecutable,
            ScalarAddLineageFieldV1::SealedExchange,
            ScalarAddLineageFieldV1::WorkerBuildIdentity,
            ScalarAddLineageFieldV1::CompletionStage,
            ScalarAddLineageFieldV1::ProviderClosure,
            ScalarAddLineageFieldV1::OutputBinding,
            ScalarAddLineageFieldV1::Diagnostics,
        ];
        for expected in cases {
            let mut facts = LineageFactsV1 {
                attempt: true,
                transaction_handoff_identity: true,
                worker_executable: true,
                sealed_exchange: true,
                worker_build_identity: true,
                completion_stage: true,
                provider_closure: true,
                output_binding: true,
                diagnostics: true,
            };
            match expected {
                ScalarAddLineageFieldV1::Attempt => facts.attempt = false,
                ScalarAddLineageFieldV1::TransactionHandoffIdentity => {
                    facts.transaction_handoff_identity = false;
                }
                ScalarAddLineageFieldV1::WorkerExecutable => facts.worker_executable = false,
                ScalarAddLineageFieldV1::SealedExchange => facts.sealed_exchange = false,
                ScalarAddLineageFieldV1::WorkerBuildIdentity => {
                    facts.worker_build_identity = false;
                }
                ScalarAddLineageFieldV1::CompletionStage => facts.completion_stage = false,
                ScalarAddLineageFieldV1::ProviderClosure => facts.provider_closure = false,
                ScalarAddLineageFieldV1::OutputBinding => facts.output_binding = false,
                ScalarAddLineageFieldV1::Diagnostics => facts.diagnostics = false,
            }
            assert!(matches!(
                validate_lineage_facts(facts),
                Err(ScalarAddFinalizationErrorV1::Lineage(field)) if field == expected
            ));
        }
    }

    #[test]
    fn matching_synthetic_policy_and_lineage_are_accepted() {
        let policy = fixture_policy();
        assert!(validate_source_policy_facts(&policy, matching_source_facts(&policy)).is_ok());
        assert!(validate_policy_facts(&policy, matching_policy_facts(&policy)).is_ok());
        assert!(
            validate_lineage_facts(LineageFactsV1 {
                attempt: true,
                transaction_handoff_identity: true,
                worker_executable: true,
                sealed_exchange: true,
                worker_build_identity: true,
                completion_stage: true,
                provider_closure: true,
                output_binding: true,
                diagnostics: true,
            })
            .is_ok()
        );
    }
}
