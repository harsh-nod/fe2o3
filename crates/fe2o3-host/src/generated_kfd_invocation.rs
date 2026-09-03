use std::{error::Error, fmt};

use fe2o3_amd_target::{AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, DeviceBindingError};
use fe2o3_runtime::{
    Gfx942AuthorizedRuntimeDispatchResultV1, Gfx942AuthorizedRuntimeExecutionErrorV1,
    Gfx942RuntimePreparationErrorV1, PreparedGfx942RuntimeDispatchV1,
    WorkerV3Gfx942ExecutionAuthorityV1, execute_authorized_gfx942_runtime_dispatch_v1,
    prepare_gfx942_runtime_dispatch_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKfdArguments, GeneratedKfdCompletion, GeneratedKfdCompletionError,
    GeneratedKfdPackingObservationV1, GeneratedKfdPrepareError, RecoveredWorkerV3AdmissionErrorV1,
};

const DIFFERENTIAL_BINDING_DOMAIN_V1: &[u8] = b"FE2O3/HOST/GENERATED-KFD-DIFFERENTIAL-BINDING/V1\0";
const DEVICE_TOPOLOGY_BINDING_DOMAIN_V1: &[u8] = b"FE2O3/HOST/DIRECT-KFD-DEVICE-TOPOLOGY/V1\0";

/// Stable schema for the generated-host/direct-KFD observation boundary.
pub const GENERATED_KFD_DIFFERENTIAL_OBSERVATION_SCHEMA_V1: &str =
    "fe2o3-generated-worker-v3-kfd-differential-observation-v1";
/// Runtime contract traversed before a physical differential observation can be minted.
pub const GENERATED_WORKER_V3_DIRECT_KFD_RUNTIME_CONTRACT_V1: &str =
    "fe2o3-generated-worker-v3-authorized-direct-kfd-runtime-v1";

/// Whether this exact generated invocation can mint a sealed differential observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedWorkerV3KfdDifferentialAvailabilityV1 {
    SealedObservationAvailable,
    ProtectedProductionEvidenceUnavailable,
}

/// One exact, move-only Worker V3 invocation ready for the permanent pure-KFD runtime.
///
/// Construction joins authenticated compiler and proof evidence, compiler-generated argument
/// capabilities, the current finalized artifact, physical runtime preparation, and one checked
/// gfx942 device. The private authority cannot be extracted or exchanged with another prepared
/// request or device.
#[must_use = "a prepared KFD invocation retains output borrows and execution authority"]
pub struct GeneratedWorkerV3KfdInvocation<'allocation, K> {
    authority: GeneratedWorkerV3KfdExecutionAuthority<K>,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
    completion: GeneratedKfdCompletion<'allocation>,
    differential: Option<GeneratedWorkerV3KfdDifferentialBindingV1>,
}

impl<K> fmt::Debug for GeneratedWorkerV3KfdInvocation<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkerV3KfdInvocation")
            .field("kernel_name", &self.prepared.kernel_name())
            .field(
                "dispatch_contract_sha256",
                &self.prepared.dispatch_contract_sha256(),
            )
            .field("device_unique_id", &self.device.observation().unique_id())
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedWorkerV3KfdInvocation<'_, K> {
    pub fn kernel_name(&self) -> &str {
        self.prepared.kernel_name()
    }

    pub const fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.prepared.dispatch_contract_sha256()
    }

    pub const fn device_unique_id(&self) -> u64 {
        self.authority.device_unique_id
    }

    /// Returns the exact generated-host binding when protected production evidence was retained.
    ///
    /// Synthetic verifier fixtures deliberately return `None` and cannot enter differential
    /// hardware qualification.
    pub const fn differential_binding(&self) -> Option<&GeneratedWorkerV3KfdDifferentialBindingV1> {
        self.differential.as_ref()
    }

    /// Reports the exact generated-host availability boundary without launching the device.
    pub const fn differential_availability(
        &self,
    ) -> GeneratedWorkerV3KfdDifferentialAvailabilityV1 {
        if self.differential.is_some() {
            GeneratedWorkerV3KfdDifferentialAvailabilityV1::SealedObservationAvailable
        } else {
            GeneratedWorkerV3KfdDifferentialAvailabilityV1::ProtectedProductionEvidenceUnavailable
        }
    }

    /// Consumes the complete authority, waits for KFD quiescence, validates runtime effects, and
    /// writes completed mutable buffers back through their retained exclusive Rust borrows.
    pub fn execute(
        self,
    ) -> Result<Gfx942AuthorizedRuntimeDispatchResultV1, GeneratedWorkerV3KfdExecutionError> {
        let Self {
            authority,
            device,
            prepared,
            completion,
            differential: _,
        } = self;
        let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)
            .map_err(GeneratedWorkerV3KfdExecutionError::Runtime)?;
        completion
            .apply(result)
            .map_err(GeneratedWorkerV3KfdExecutionError::Completion)
    }

    /// Executes through the normal authorized Worker V3 KFD path and returns a sealed observation.
    ///
    /// The observation cannot be caller-constructed. Its presence proves only that the existing
    /// generated-host and direct-KFD completion boundary was traversed; comparison with simulator
    /// output and any semantic conclusion remain the differential harness's responsibility.
    pub fn execute_for_differential(
        self,
    ) -> Result<GeneratedWorkerV3KfdDifferentialObservationV1, GeneratedWorkerV3KfdExecutionError>
    {
        let Self {
            authority,
            device,
            prepared,
            completion,
            differential,
        } = self;
        let binding = differential
            .ok_or(GeneratedWorkerV3KfdExecutionError::DifferentialEvidenceUnavailable)?;
        let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)
            .map_err(GeneratedWorkerV3KfdExecutionError::Runtime)?;
        let result = completion
            .apply(result)
            .map_err(GeneratedWorkerV3KfdExecutionError::Completion)?;
        Ok(GeneratedWorkerV3KfdDifferentialObservationV1 { binding, result })
    }
}

/// Exact pre-dispatch identity axes retained from generated packing and protected Worker V3.
///
/// This value is descriptive and grants no load, launch, compiler, proof, or parity authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedWorkerV3KfdDifferentialBindingV1 {
    identity: [u8; 32],
    kernel_id: [u8; 32],
    logical_name: &'static str,
    export_name: &'static str,
    kernel_binding_identity: [u8; 32],
    generated_host_contract_identity: [u8; 32],
    direct_kfd_runtime_contract: &'static str,
    worker_challenge_identity: [u8; 32],
    worker_lineage_identity: [u8; 32],
    compiler_execution_subject_identity: [u8; 32],
    compiler_execution_receipt_identity: [u8; 32],
    finalizer_derivation_identity: [u8; 32],
    production_kir_v8_sha256: [u8; 32],
    production_kir_v8_bytes: u64,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_bytes: u64,
    target: String,
    dispatch_contract_sha256: [u8; 32],
    grid: [u32; 3],
    workgroup: [u16; 3],
    dynamic_group_segment_bytes: u32,
    device_unique_id: u64,
    device_topology_identity: [u8; 32],
    packing: GeneratedKfdPackingObservationV1,
}

impl GeneratedWorkerV3KfdDifferentialBindingV1 {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn kernel_id(&self) -> &[u8; 32] {
        &self.kernel_id
    }
    pub const fn logical_name(&self) -> &'static str {
        self.logical_name
    }
    pub const fn export_name(&self) -> &'static str {
        self.export_name
    }
    pub const fn kernel_binding_identity(&self) -> &[u8; 32] {
        &self.kernel_binding_identity
    }
    pub const fn generated_host_contract_identity(&self) -> &[u8; 32] {
        &self.generated_host_contract_identity
    }
    pub const fn direct_kfd_runtime_contract(&self) -> &'static str {
        self.direct_kfd_runtime_contract
    }
    pub const fn worker_challenge_identity(&self) -> &[u8; 32] {
        &self.worker_challenge_identity
    }
    pub const fn worker_lineage_identity(&self) -> &[u8; 32] {
        &self.worker_lineage_identity
    }
    pub const fn compiler_execution_subject_identity(&self) -> &[u8; 32] {
        &self.compiler_execution_subject_identity
    }
    pub const fn compiler_execution_receipt_identity(&self) -> &[u8; 32] {
        &self.compiler_execution_receipt_identity
    }
    pub const fn finalizer_derivation_identity(&self) -> &[u8; 32] {
        &self.finalizer_derivation_identity
    }
    pub const fn production_kir_v8_sha256(&self) -> &[u8; 32] {
        &self.production_kir_v8_sha256
    }
    pub const fn production_kir_v8_bytes(&self) -> u64 {
        self.production_kir_v8_bytes
    }
    pub const fn finalized_hsaco_sha256(&self) -> &[u8; 32] {
        &self.finalized_hsaco_sha256
    }
    pub const fn finalized_hsaco_bytes(&self) -> u64 {
        self.finalized_hsaco_bytes
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub const fn dispatch_contract_sha256(&self) -> &[u8; 32] {
        &self.dispatch_contract_sha256
    }
    pub const fn grid(&self) -> [u32; 3] {
        self.grid
    }
    pub const fn workgroup(&self) -> [u16; 3] {
        self.workgroup
    }
    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.dynamic_group_segment_bytes
    }
    pub const fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }
    pub const fn device_topology_identity(&self) -> &[u8; 32] {
        &self.device_topology_identity
    }
    pub const fn packing(&self) -> &GeneratedKfdPackingObservationV1 {
        &self.packing
    }
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Successful output minted only after authorized direct-KFD completion and generated writeback.
#[must_use]
pub struct GeneratedWorkerV3KfdDifferentialObservationV1 {
    binding: GeneratedWorkerV3KfdDifferentialBindingV1,
    result: Gfx942AuthorizedRuntimeDispatchResultV1,
}

impl GeneratedWorkerV3KfdDifferentialObservationV1 {
    pub const fn binding(&self) -> &GeneratedWorkerV3KfdDifferentialBindingV1 {
        &self.binding
    }
    pub const fn result(&self) -> &Gfx942AuthorizedRuntimeDispatchResultV1 {
        &self.result
    }
    pub fn into_runtime_result(self) -> Gfx942AuthorizedRuntimeDispatchResultV1 {
        self.result
    }
    pub const fn hardware_observed(&self) -> bool {
        true
    }
    pub const fn grants_parity_authority(&self) -> bool {
        false
    }
}

struct GeneratedWorkerV3KfdExecutionAuthority<K> {
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    kernel_name: &'static str,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
}

// SAFETY: this private implementation is constructed only by
// `prepare_generated_kfd_invocation`. That transition retains the exact authenticated Worker V3
// decision and its current-publication token, admits only compiler-generated argument capabilities,
// prepares the runtime request from the token's exact HSACO bytes, validates the selected kernel
// and artifact identities, and retains the same checked KFD device whose identity is named here.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3Gfx942ExecutionAuthorityV1
    for GeneratedWorkerV3KfdExecutionAuthority<K>
{
    type CurrentnessError = RecoveredWorkerV3AdmissionErrorV1;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    fn kernel_name(&self) -> &str {
        self.kernel_name
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.authenticated.revalidate_currentness()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Joins this authenticated executable with one generated host-memory invocation and checked
    /// gfx942 device. No caller-created digest or raw pointer enters the transition.
    pub fn prepare_generated_kfd_invocation<'allocation, Arguments>(
        self,
        arguments: Arguments,
        mut device: CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Result<GeneratedWorkerV3KfdInvocation<'allocation, K>, GeneratedWorkerV3KfdInvocationError>
    where
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
    {
        self.revalidate_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        validate_gfx942_target(&self)?;

        let packed = self
            .prepare_generated_kfd_arguments_with_current(
                self.current_publication_token(),
                arguments,
            )
            .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
        let mut differential = differential_binding::<K>(
            &self,
            &device,
            &packed,
            geometry,
            dynamic_group_segment_bytes,
        );
        let (inputs, completion) =
            packed.into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds);
        let prepared = prepare_gfx942_runtime_dispatch_v1(
            self.current_publication_token().exact_artifact_bytes(),
            K::EXPORT_NAME,
            inputs,
        )
        .map_err(GeneratedWorkerV3KfdInvocationError::RuntimePreparation)?;
        validate_runtime_binding(&self, &prepared)?;
        if let Some(binding) = differential.as_mut() {
            binding.dispatch_contract_sha256 = prepared.dispatch_contract_sha256();
            binding.identity = differential_binding_identity(binding);
        }

        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        self.admission()
            .revalidate_retained_currentness_token(self.current_publication_token())
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;

        let verification = self.verification();
        let authority = GeneratedWorkerV3KfdExecutionAuthority {
            finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
            finalized_hsaco_length: verification.finalized_hsaco_length(),
            kernel_name: K::EXPORT_NAME,
            dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
            device_unique_id: device.observation().unique_id(),
            authenticated: self,
        };
        Ok(GeneratedWorkerV3KfdInvocation {
            authority,
            device,
            prepared,
            completion,
            differential,
        })
    }
}

fn differential_binding<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &AuthenticatedWorkerV3ExecutableV1<K>,
    device: &CheckedGfx942XnackMinusDevice,
    packed: &crate::GeneratedKfdPackedArguments<'_>,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Option<GeneratedWorkerV3KfdDifferentialBindingV1> {
    let verification = authenticated.verification();
    let proof = verification.validated_compiler_proof_inputs()?;
    let production_kir = proof.kernel_ir().identity();
    let compiler_subject = authenticated
        .admission()
        .compiler_execution_subject()
        .identity();
    let compiler_execution = verification.compiler_execution();
    let finalizer = verification.finalizer_derivation().identity();
    let observation = device.observation();
    let process = device.process_incarnation();
    let correlation = device.projection().correlation();
    let mut device_hasher = Sha256::new();
    device_hasher.update(DEVICE_TOPOLOGY_BINDING_DOMAIN_V1);
    device_hasher.update(fe2o3_kfd::DEVICE_ADMISSION_PROFILE_SHA256_BYTES_V1);
    device_hasher.update(observation.topology_node_id().to_le_bytes());
    device_hasher.update(observation.kfd_gpu_id().to_le_bytes());
    device_hasher.update(observation.unique_id().to_le_bytes());
    device_hasher.update(correlation.identity().gpu_unique_id.to_le_bytes());
    device_hasher.update(correlation.drm_schema_identity().as_bytes());
    device_hasher.update(process.pid().to_le_bytes());
    device_hasher.update(process.start_time_ticks().to_le_bytes());
    device_hasher.update(process.mount_namespace_device().to_le_bytes());
    device_hasher.update(process.mount_namespace_inode().to_le_bytes());
    let device_topology_identity = device_hasher.finalize().into();

    let mut binding = GeneratedWorkerV3KfdDifferentialBindingV1 {
        identity: [0; 32],
        kernel_id: *packed.kernel_id().as_bytes(),
        logical_name: K::LOGICAL_NAME,
        export_name: K::EXPORT_NAME,
        kernel_binding_identity: K::KERNEL_BINDING_ID_V1,
        generated_host_contract_identity: K::PROFILE.generated_host_contract_identity(),
        direct_kfd_runtime_contract: GENERATED_WORKER_V3_DIRECT_KFD_RUNTIME_CONTRACT_V1,
        worker_challenge_identity: *verification.challenge_identity().as_bytes(),
        worker_lineage_identity: *verification.lineage_identity().as_bytes(),
        compiler_execution_subject_identity: *compiler_subject.sha256(),
        compiler_execution_receipt_identity: compiler_execution.receipt_sha256(),
        finalizer_derivation_identity: *finalizer.as_bytes(),
        production_kir_v8_sha256: *production_kir.digest(),
        production_kir_v8_bytes: production_kir.canonical_length(),
        finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
        finalized_hsaco_bytes: verification.finalized_hsaco_length(),
        target: authenticated.target().to_string(),
        dispatch_contract_sha256: [0; 32],
        grid: geometry.grid(),
        workgroup: geometry.workgroup(),
        dynamic_group_segment_bytes,
        device_unique_id: observation.unique_id(),
        device_topology_identity,
        packing: packed.packing_observation().clone(),
    };
    binding.identity = differential_binding_identity(&binding);
    Some(binding)
}

fn differential_binding_identity(binding: &GeneratedWorkerV3KfdDifferentialBindingV1) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DIFFERENTIAL_BINDING_DOMAIN_V1);
    hasher.update(binding.kernel_id);
    hash_bytes(&mut hasher, binding.logical_name.as_bytes());
    hash_bytes(&mut hasher, binding.export_name.as_bytes());
    hasher.update(binding.kernel_binding_identity);
    hasher.update(binding.generated_host_contract_identity);
    hash_bytes(&mut hasher, binding.direct_kfd_runtime_contract.as_bytes());
    hasher.update(binding.worker_challenge_identity);
    hasher.update(binding.worker_lineage_identity);
    hasher.update(binding.compiler_execution_subject_identity);
    hasher.update(binding.compiler_execution_receipt_identity);
    hasher.update(binding.finalizer_derivation_identity);
    hasher.update(binding.production_kir_v8_sha256);
    hasher.update(binding.production_kir_v8_bytes.to_le_bytes());
    hasher.update(binding.finalized_hsaco_sha256);
    hasher.update(binding.finalized_hsaco_bytes.to_le_bytes());
    hash_bytes(&mut hasher, binding.target.as_bytes());
    hasher.update(binding.dispatch_contract_sha256);
    for value in binding.grid {
        hasher.update(value.to_le_bytes());
    }
    for value in binding.workgroup {
        hasher.update(value.to_le_bytes());
    }
    hasher.update(binding.dynamic_group_segment_bytes.to_le_bytes());
    hasher.update(binding.device_unique_id.to_le_bytes());
    hasher.update(binding.device_topology_identity);
    hasher.update(binding.packing.identity());
    hasher.finalize().into()
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(
        u64::try_from(bytes.len())
            .expect("Rust slice length fits the canonical u64 length field")
            .to_le_bytes(),
    );
    hasher.update(bytes);
}

fn validate_gfx942_target<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &AuthenticatedWorkerV3ExecutableV1<K>,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    let expected = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
        .expect("the canonical production gfx942 target is valid");
    let artifact = authenticated.target();
    if artifact != expected {
        return Err(GeneratedWorkerV3KfdInvocationError::TargetMismatch { artifact });
    }
    Ok(())
}

fn validate_runtime_binding<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &AuthenticatedWorkerV3ExecutableV1<K>,
    prepared: &PreparedGfx942RuntimeDispatchV1,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    let verification = authenticated.verification();
    validate_runtime_identity_fields(
        verification.finalized_hsaco_sha256(),
        verification.finalized_hsaco_length(),
        K::EXPORT_NAME,
        prepared.identity().object_sha256(),
        prepared.finalized_hsaco_length(),
        prepared.kernel_name(),
    )
}

fn validate_runtime_identity_fields(
    expected_sha256: [u8; 32],
    expected_length: u64,
    expected_kernel_name: &str,
    actual_sha256: [u8; 32],
    actual_length: u64,
    actual_kernel_name: &str,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    if actual_sha256 != expected_sha256 {
        return Err(GeneratedWorkerV3KfdInvocationError::ArtifactIdentityMismatch);
    }
    if actual_length != expected_length {
        return Err(GeneratedWorkerV3KfdInvocationError::ArtifactLengthMismatch);
    }
    if actual_kernel_name != expected_kernel_name {
        return Err(GeneratedWorkerV3KfdInvocationError::KernelNameMismatch);
    }
    Ok(())
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3KfdInvocationError {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    DeviceCurrentness(DeviceBindingError),
    TargetMismatch { artifact: AmdTargetId },
    Arguments(GeneratedKfdPrepareError),
    RuntimePreparation(Gfx942RuntimePreparationErrorV1),
    ArtifactIdentityMismatch,
    ArtifactLengthMismatch,
    KernelNameMismatch,
}

impl fmt::Display for GeneratedWorkerV3KfdInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => {
                write!(formatter, "Worker V3 publication is not current: {error}")
            }
            Self::DeviceCurrentness(error) => {
                write!(formatter, "checked KFD device is not current: {error}")
            }
            Self::TargetMismatch { artifact } => write!(
                formatter,
                "pure-KFD invocation requires {PRODUCTION_GFX942_DEVICE_TARGET_V1}; artifact is {artifact}"
            ),
            Self::Arguments(error) => write!(formatter, "generated KFD arguments failed: {error}"),
            Self::RuntimePreparation(error) => {
                write!(formatter, "pure-KFD runtime preparation failed: {error}")
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("runtime prepared a different finalized HSACO")
            }
            Self::ArtifactLengthMismatch => {
                formatter.write_str("runtime prepared a different finalized HSACO length")
            }
            Self::KernelNameMismatch => {
                formatter.write_str("runtime prepared a different kernel entry")
            }
        }
    }
}

impl Error for GeneratedWorkerV3KfdInvocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::DeviceCurrentness(error) => Some(error),
            Self::Arguments(error) => Some(error),
            Self::RuntimePreparation(error) => Some(error),
            Self::TargetMismatch { .. }
            | Self::ArtifactIdentityMismatch
            | Self::ArtifactLengthMismatch
            | Self::KernelNameMismatch => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3KfdExecutionError {
    Runtime(Gfx942AuthorizedRuntimeExecutionErrorV1<RecoveredWorkerV3AdmissionErrorV1>),
    Completion(GeneratedKfdCompletionError),
    DifferentialEvidenceUnavailable,
}

impl fmt::Display for GeneratedWorkerV3KfdExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "authorized KFD execution failed: {error}"),
            Self::Completion(error) => {
                write!(formatter, "generated KFD completion failed: {error}")
            }
            Self::DifferentialEvidenceUnavailable => formatter.write_str(
                "protected production Worker V3 evidence is unavailable for differential observation",
            ),
        }
    }
}

impl Error for GeneratedWorkerV3KfdExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Completion(error) => Some(error),
            Self::DifferentialEvidenceUnavailable => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_binding_requires_exact_artifact_length_and_kernel() {
        let expected_sha256 = [0x41; 32];
        assert!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_096,
                "kernel_v1",
            )
            .is_ok()
        );

        let mut changed_sha256 = expected_sha256;
        changed_sha256[0] ^= 1;
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                changed_sha256,
                4_096,
                "kernel_v1",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::ArtifactIdentityMismatch)
        ));
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_097,
                "kernel_v1",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::ArtifactLengthMismatch)
        ));
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_096,
                "other",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::KernelNameMismatch)
        ));
    }
}
