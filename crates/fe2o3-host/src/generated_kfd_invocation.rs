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

use crate::generated_argument_plan::validate_worker_v3_argument_packing;
use crate::generated_host_contract_v2::CheckedGeneratedHostDispatchV2;
use crate::worker_v3_verification_admission::AdmittedWorkerV3SemanticMachineRefinementV1;
use crate::{
    AdmittedGeneratedHostContractV2, AuthenticatedWorkerV3CapabilityApplicationV1,
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedHostArgumentsV2,
    CompilerGeneratedKernelExpectationRosterV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKernelExpectationV2, CompilerGeneratedKfdArguments,
    GeneratedHostContractErrorV2, GeneratedHostDispatchEvidenceV2, GeneratedHostLaunchGeometryV2,
    GeneratedHostPrepareErrorV2, GeneratedHostRuntimeCoordinatesV2, GeneratedKfdCompletion,
    GeneratedKfdCompletionError, GeneratedKfdPackingObservationV1, GeneratedKfdPrepareError,
    RecoveredWorkerV3AdmissionErrorV1,
};

const DIFFERENTIAL_BINDING_DOMAIN_V1: &[u8] = b"FE2O3/HOST/GENERATED-KFD-DIFFERENTIAL-BINDING/V1\0";
const APPLICATION_EXECUTION_BINDING_DOMAIN_V1: &[u8] =
    b"FE2O3/HOST/WORKER-V3-APPLICATION-EXECUTION-BINDING/V1\0";
const DEVICE_TOPOLOGY_BINDING_DOMAIN_V1: &[u8] = b"FE2O3/HOST/DIRECT-KFD-DEVICE-TOPOLOGY/V1\0";
const GENERATED_HOST_KFD_CONTEXT_DOMAIN_V2: &[u8] = b"FE2O3/HOST/GENERATED-KFD-CONTEXT/V2\0";
const GENERATED_HOST_KFD_STREAM_DOMAIN_V2: &[u8] = b"FE2O3/HOST/GENERATED-KFD-STREAM/V2\0";

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

/// One exact, move-only Worker V3 invocation for the permanent pure-KFD runtime.
///
/// Construction joins authenticated compiler and proof evidence, compiler-generated argument
/// capabilities, the current finalized artifact, physical runtime preparation, and one checked
/// gfx942 device. The private authority cannot be extracted or exchanged with another prepared
/// request or device. The explicit test-only synthetic lane retains non-executable qualification
/// custody; native execution requires the protected production variant.
#[must_use = "a prepared KFD invocation retains output borrows and application custody"]
pub struct GeneratedWorkerV3KfdInvocation<'allocation, K> {
    authority: GeneratedWorkerV3KfdInvocationAuthorityV1<K>,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
    completion: GeneratedKfdCompletion<'allocation>,
    differential: Option<GeneratedWorkerV3KfdDifferentialBindingV1>,
    generated_host: Option<CheckedGeneratedHostDispatchV2<K>>,
}

/// One typed direct-KFD invocation borrowed from authenticated aggregate application custody.
///
/// The application borrow permits sequential launches of distinct generated marker types while
/// preventing carrier, artifact, and currentness custody from being released during a launch.
#[must_use = "a prepared capability invocation retains every allocation borrow until completion"]
pub(crate) struct GeneratedWorkerV3CapabilityKfdInvocation<'application, 'allocation, R, K> {
    authority: GeneratedWorkerV3CapabilityKfdExecutionAuthority<'application, R, K>,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
    completion: GeneratedKfdCompletion<'allocation>,
    generated_host: CheckedGeneratedHostDispatchV2<K>,
}

impl<R, K> fmt::Debug for GeneratedWorkerV3CapabilityKfdInvocation<'_, '_, R, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkerV3CapabilityKfdInvocation")
            .field("kernel_name", &self.prepared.kernel_name())
            .field(
                "dispatch_contract_sha256",
                &self.prepared.dispatch_contract_sha256(),
            )
            .field("device_unique_id", &self.device.observation().unique_id())
            .finish_non_exhaustive()
    }
}

impl<R, K> GeneratedWorkerV3CapabilityKfdInvocation<'_, '_, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
{
    pub(crate) fn kernel_name(&self) -> &str {
        self.prepared.kernel_name()
    }

    /// Executes through the internal authorized KFD boundary and releases borrows only after
    /// checked device completion and post-completion publication-currentness revalidation.
    pub(crate) fn execute(
        self,
    ) -> Result<Gfx942AuthorizedRuntimeDispatchResultV1, GeneratedWorkerV3KfdExecutionError> {
        let Self {
            authority,
            device,
            prepared,
            completion,
            generated_host: _generated_host,
        } = self;
        let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)
            .map_err(GeneratedWorkerV3KfdExecutionError::Runtime)?;
        completion
            .apply(result)
            .map_err(GeneratedWorkerV3KfdExecutionError::Completion)
    }
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
        self.device.observation().unique_id()
    }

    /// Returns the exact production application binding retained by this invocation.
    ///
    /// The explicitly feature-gated synthetic verifier lane returns `None`; it can exercise
    /// bounded preparation but cannot manufacture production application or native-execution
    /// custody.
    pub fn application_execution_binding(
        &self,
    ) -> Option<&WorkerV3ApplicationExecutionBindingV1<K>> {
        self.authority.application_binding()
    }

    /// Returns the exact generated-host binding when protected production evidence was retained.
    ///
    /// Synthetic verifier fixtures deliberately return `None` and cannot enter differential
    /// hardware qualification.
    pub const fn differential_binding(&self) -> Option<&GeneratedWorkerV3KfdDifferentialBindingV1> {
        self.differential.as_ref()
    }

    /// Returns the exact V2 runtime coordinates retained by side-by-side host preparation.
    pub fn generated_host_runtime_coordinates(&self) -> Option<GeneratedHostRuntimeCoordinatesV2> {
        self.generated_host
            .as_ref()
            .map(|checked| checked.runtime())
    }

    /// Returns the exact capability association retained by side-by-side host preparation.
    pub fn generated_host_capability_association_identity(&self) -> Option<([u8; 32], u64)> {
        self.generated_host
            .as_ref()
            .map(|checked| checked.capability_association_identity())
    }

    /// Returns the exact V13 static-association identity retained through KFD completion.
    pub fn generated_host_v13_static_association_identity(&self) -> Option<[u8; 32]> {
        self.generated_host
            .as_ref()
            .map(|checked| checked.static_association_identity())
    }

    /// Returns the exact V13 final-graph subject retained through KFD completion.
    pub fn generated_host_v13_subject(&self) -> Option<crate::GeneratedHostExecutionSubjectV2> {
        self.generated_host
            .as_ref()
            .map(|checked| checked.subject())
    }

    /// Returns the exact artifact and kernel ordinal retained by V2 preparation.
    pub fn generated_host_artifact_binding(
        &self,
    ) -> Option<(
        fe2o3_kernel_descriptor::CanonicalCodeObjectDigest,
        usize,
        &str,
    )> {
        self.generated_host.as_ref().map(|checked| {
            (
                checked.artifact_identity(),
                checked.kernel_ordinal(),
                checked.entry_name(),
            )
        })
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
            generated_host: _generated_host,
        } = self;
        let authority = authority
            .into_production()
            .map_err(|_| GeneratedWorkerV3KfdExecutionError::ProductionAuthorityUnavailable)?;
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
            generated_host: _generated_host,
        } = self;
        let binding = differential
            .ok_or(GeneratedWorkerV3KfdExecutionError::DifferentialEvidenceUnavailable)?;
        let authority = authority
            .into_production()
            .map_err(|_| GeneratedWorkerV3KfdExecutionError::ProductionAuthorityUnavailable)?;
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
    compiler_capability_association_sha256: [u8; 32],
    compiler_capability_association_bytes: u64,
    production_capability_result_sha256: [u8; 32],
    production_capability_result_bytes: u64,
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
    pub const fn compiler_capability_association_identity(&self) -> (&[u8; 32], u64) {
        (
            &self.compiler_capability_association_sha256,
            self.compiler_capability_association_bytes,
        )
    }
    pub const fn production_capability_result_identity(&self) -> (&[u8; 32], u64) {
        (
            &self.production_capability_result_sha256,
            self.production_capability_result_bytes,
        )
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

/// Exact move-only application binding retained by a production Worker V3 invocation.
///
/// This owner is constructed only after the protected verifier decision, compiler-generated
/// argument packing, bounded runtime preparation, and checked device have all been joined. It
/// retains the authenticated executable and therefore the exact current-publication token; a
/// currentness check never relies on a copied digest. Public code can inspect this owner only
/// through a prepared invocation and cannot extract it or construct runtime authority from it.
///
/// This is the application side of the release transition. It owns the unique admitted
/// semantic-to-machine receipt in addition to the authenticated decision and current-publication
/// token. The binding does not produce that proof; no concrete production proof backend is
/// shipped today.
///
/// ```compile_fail
/// use fe2o3_host::WorkerV3ApplicationExecutionBindingV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<WorkerV3ApplicationExecutionBindingV1<()>>();
/// ```
#[must_use = "dropping the binding abandons exact Worker V3 application custody"]
pub struct WorkerV3ApplicationExecutionBindingV1<K> {
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    semantic_machine_refinement: AdmittedWorkerV3SemanticMachineRefinementV1,
    coordinates: WorkerV3ApplicationExecutionCoordinatesV1,
    packing: GeneratedKfdPackingObservationV1,
}

impl<K> fmt::Debug for WorkerV3ApplicationExecutionBindingV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3ApplicationExecutionBindingV1")
            .field("identity", &self.coordinates.identity)
            .field("kernel_id", &self.coordinates.kernel_id)
            .field("export_name", &self.coordinates.export_name)
            .field(
                "finalized_hsaco_sha256",
                &self.coordinates.finalized_hsaco_sha256,
            )
            .field("target", &self.coordinates.target)
            .field(
                "dispatch_contract_sha256",
                &self.coordinates.dispatch_contract_sha256,
            )
            .field("device_unique_id", &self.coordinates.device_unique_id)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3ApplicationExecutionBindingV1<K> {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.coordinates.identity
    }

    pub const fn kernel_id(&self) -> &[u8; 32] {
        &self.coordinates.kernel_id
    }

    pub const fn logical_name(&self) -> &'static str {
        self.coordinates.logical_name
    }

    pub const fn export_name(&self) -> &'static str {
        self.coordinates.export_name
    }

    pub const fn kernel_binding_identity(&self) -> &[u8; 32] {
        &self.coordinates.kernel_binding_identity
    }

    pub const fn generated_host_contract_identity(&self) -> &[u8; 32] {
        &self.coordinates.generated_host_contract_identity
    }

    pub const fn finalized_hsaco_sha256(&self) -> &[u8; 32] {
        &self.coordinates.finalized_hsaco_sha256
    }

    pub const fn finalized_hsaco_bytes(&self) -> u64 {
        self.coordinates.finalized_hsaco_bytes
    }

    pub fn target(&self) -> &str {
        &self.coordinates.target
    }

    pub const fn code_object_version(&self) -> u16 {
        self.coordinates.code_object_version
    }

    pub const fn proof_executable_binding_sha256(&self) -> &[u8; 32] {
        &self.coordinates.proof_executable_binding_sha256
    }

    pub const fn rust_type_layout_contract_sha256(&self) -> &[u8; 32] {
        &self.coordinates.rust_type_layout_contract_sha256
    }

    pub const fn rust_effect_contract_sha256(&self) -> &[u8; 32] {
        &self.coordinates.rust_effect_contract_sha256
    }

    /// Returns the unique semantic-to-machine receipt identity consumed into this binding.
    pub const fn semantic_machine_refinement_receipt_identity(&self) -> &[u8; 32] {
        self.semantic_machine_refinement.receipt().identity()
    }

    /// Reports ownership of the unique receipt consumed by this application occurrence.
    pub const fn retains_semantic_machine_refinement_receipt(&self) -> bool {
        true
    }

    /// Returns the exact side-by-side capability association retained through completion.
    ///
    /// `None` is possible only in the explicitly feature-gated legacy integration-test lane.
    pub fn compiler_capability_association_identity(&self) -> Option<([u8; 32], u64)> {
        self.semantic_machine_refinement
            .compiler_capability_association_identity()
    }

    /// Returns the exact completed V5 result retained through application execution.
    pub fn production_capability_result_identity(&self) -> Option<([u8; 32], u64)> {
        self.semantic_machine_refinement
            .production_capability_result_identity()
    }

    pub const fn dispatch_contract_sha256(&self) -> &[u8; 32] {
        &self.coordinates.dispatch_contract_sha256
    }

    pub const fn grid(&self) -> [u32; 3] {
        self.coordinates.grid
    }

    pub const fn workgroup(&self) -> [u16; 3] {
        self.coordinates.workgroup
    }

    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.coordinates.dynamic_group_segment_bytes
    }

    pub const fn timeout_milliseconds(&self) -> u32 {
        self.coordinates.timeout_milliseconds
    }

    pub const fn device_unique_id(&self) -> u64 {
        self.coordinates.device_unique_id
    }

    pub const fn device_topology_identity(&self) -> &[u8; 32] {
        &self.coordinates.device_topology_identity
    }

    pub const fn packing(&self) -> &GeneratedKfdPackingObservationV1 {
        &self.packing
    }

    /// Rechecks the retained exact current-publication token rather than reacquiring by path.
    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.authenticated.revalidate_currentness()
    }

    /// Reports custody of the exact refinement established by the consumed proof receipt.
    ///
    /// This does not claim a proof producer exists: no concrete production proof backend or
    /// authenticated proof artifact ships today, so a production binding remains unreachable to
    /// the repository alone. Nor does it extend the receipt beyond its exact KIR, LLVM, selected
    /// ISA, machine-effect, artifact, and currentness coordinates.
    pub const fn establishes_semantic_or_machine_refinement(&self) -> bool {
        true
    }

    /// The binding has no public unchecked load, queue, or launch operation.
    pub const fn grants_unchecked_execution_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkerV3ApplicationExecutionCoordinatesV1 {
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
    compiler_execution_carriage_identity: [u8; 32],
    compiler_execution_policy_identity: [u8; 32],
    compiler_execution_issuer_journal_identity: [u8; 32],
    compiler_execution_occurrence_identity: [u8; 32],
    compiler_execution_receipt_identity: [u8; 32],
    compiler_execution_publication_identity: [u8; 32],
    compiler_execution_acknowledgment_identity: [u8; 32],
    compiler_execution_worker_ledger_record_identity: [u8; 32],
    compiler_execution_sequence: u64,
    compiler_execution_prior_rollback_anchor: [u8; 32],
    compiler_execution_current_rollback_anchor: [u8; 32],
    compiler_current_record_verification_identity: [u8; 32],
    compiler_current_record_attestation_identity: [u8; 32],
    protected_compiler_policy_verification_identity: [u8; 32],
    protected_worker_ledger_verification_identity: [u8; 32],
    external_rollback_verification_identity: [u8; 32],
    finalizer_derivation_identity: [u8; 32],
    proof_binding_sha256: [u8; 32],
    proof_binding_bytes: u64,
    production_kir_v8_sha256: [u8; 32],
    production_kir_v8_bytes: u64,
    target_binding_sha256: [u8; 32],
    target_binding_bytes: u64,
    data_layout_sha256: [u8; 32],
    data_layout_bytes: u64,
    semantic_to_llvm_sha256: [u8; 32],
    semantic_to_llvm_bytes: u64,
    final_llvm_sha256: [u8; 32],
    final_llvm_bytes: u64,
    final_module_commitment_sha256: [u8; 32],
    final_module_commitment_bytes: u64,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_bytes: u64,
    target: String,
    code_object_version: u16,
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    semantic_machine_refinement_receipt_identity: [u8; 32],
    compiler_capability_association_sha256: [u8; 32],
    compiler_capability_association_bytes: u64,
    production_capability_result_sha256: [u8; 32],
    production_capability_result_bytes: u64,
    safety_properties: u8,
    dispatch_contract_sha256: [u8; 32],
    grid: [u32; 3],
    workgroup: [u16; 3],
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
    device_unique_id: u64,
    device_topology_identity: [u8; 32],
    packing_identity: [u8; 32],
}

#[cfg_attr(
    not(feature = "worker-v3-verifier-test-support"),
    allow(
        dead_code,
        reason = "qualification custody is constructed only by the explicit verifier test feature"
    )
)]
enum ProductionExecutionCustodyV1<P, Q> {
    Production(P),
    Qualification(Q),
}

impl<P, Q> ProductionExecutionCustodyV1<P, Q> {
    fn into_production(self) -> Result<P, Q> {
        match self {
            Self::Production(production) => Ok(production),
            Self::Qualification(qualification) => Err(qualification),
        }
    }
}

struct GeneratedWorkerV3KfdInvocationAuthorityV1<K> {
    custody: ProductionExecutionCustodyV1<
        GeneratedWorkerV3KfdExecutionAuthority<K>,
        Box<AuthenticatedWorkerV3ExecutableV1<K>>,
    >,
}

impl<K> GeneratedWorkerV3KfdInvocationAuthorityV1<K> {
    fn application_binding(&self) -> Option<&WorkerV3ApplicationExecutionBindingV1<K>> {
        match &self.custody {
            ProductionExecutionCustodyV1::Production(authority) => Some(&authority.binding),
            ProductionExecutionCustodyV1::Qualification(_) => None,
        }
    }

    fn into_production(
        self,
    ) -> Result<GeneratedWorkerV3KfdExecutionAuthority<K>, Box<AuthenticatedWorkerV3ExecutableV1<K>>>
    {
        self.custody.into_production()
    }
}

struct GeneratedWorkerV3KfdExecutionAuthority<K> {
    binding: Box<WorkerV3ApplicationExecutionBindingV1<K>>,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
}

struct GeneratedWorkerV3CapabilityKfdExecutionAuthority<'application, R, K> {
    application: &'application AuthenticatedWorkerV3CapabilityApplicationV1<R>,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
    _marker: std::marker::PhantomData<fn() -> K>,
}

// SAFETY: construction is private to `prepare_capability_direct_kfd_invocation_v2`, which checks
// the exact authenticated roster entry, V5-derived generated host contract, inspected descriptor,
// generated argument packing, target, device, geometry, dynamic memory facts, and runtime request.
// The authority borrows aggregate custody through completion and revalidates it on both sides of
// the native dispatch.
unsafe impl<R, K> WorkerV3Gfx942ExecutionAuthorityV1
    for GeneratedWorkerV3CapabilityKfdExecutionAuthority<'_, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
{
    type CurrentnessError = RecoveredWorkerV3AdmissionErrorV1;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    fn kernel_name(&self) -> &str {
        K::EXPORT_NAME
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.application.revalidate_currentness()
    }
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
        self.binding
            .authenticated
            .verification()
            .finalized_hsaco_sha256()
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.binding
            .authenticated
            .verification()
            .finalized_hsaco_length()
    }

    fn kernel_name(&self) -> &str {
        K::EXPORT_NAME
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.binding.authenticated.revalidate_currentness()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Joins this authenticated executable with one generated host-memory invocation and checked
    /// gfx942 device. No caller-created digest or raw pointer enters the transition.
    #[doc(hidden)]
    pub fn prepare_generated_kfd_invocation<'allocation, Arguments>(
        mut self,
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
        let differential = differential_binding::<K>(
            &self,
            &device,
            &packed,
            geometry,
            dynamic_group_segment_bytes,
        );
        let packed_kernel_id = packed.kernel_id();
        let packing = packed.packing_observation().clone();
        let (inputs, completion) =
            packed.into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds);
        let prepared = prepare_gfx942_runtime_dispatch_v1(
            self.current_publication_token().exact_artifact_bytes(),
            K::EXPORT_NAME,
            inputs,
        )
        .map_err(GeneratedWorkerV3KfdInvocationError::RuntimePreparation)?;
        validate_runtime_binding(&self, &prepared)?;
        let mut differential = differential;
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

        let application_admission = application_execution_admission::<K>(
            &mut self,
            &device,
            packed_kernel_id,
            &packing,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
            prepared.dispatch_contract_sha256(),
        );
        let authority = match application_admission {
            Some(application) => GeneratedWorkerV3KfdInvocationAuthorityV1 {
                custody: ProductionExecutionCustodyV1::Production(
                    GeneratedWorkerV3KfdExecutionAuthority {
                        binding: Box::new(WorkerV3ApplicationExecutionBindingV1 {
                            authenticated: self,
                            semantic_machine_refinement: application.refinement,
                            coordinates: application.coordinates,
                            packing,
                        }),
                        dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
                        device_unique_id: device.observation().unique_id(),
                    },
                ),
            },
            None => {
                #[cfg(feature = "worker-v3-verifier-test-support")]
                {
                    GeneratedWorkerV3KfdInvocationAuthorityV1 {
                        custody: ProductionExecutionCustodyV1::Qualification(Box::new(self)),
                    }
                }
                #[cfg(not(feature = "worker-v3-verifier-test-support"))]
                {
                    return Err(
                        GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable,
                    );
                }
            }
        };
        Ok(GeneratedWorkerV3KfdInvocation {
            authority,
            device,
            prepared,
            completion,
            differential,
            generated_host: None,
        })
    }
}

impl<K: CompilerGeneratedKernelExpectationV2> AdmittedGeneratedHostContractV2<K> {
    /// Derives the exact logical direct-KFD context and stream selected for one checked device and
    /// launch. The physical-device observation and topology custody are independent of caller
    /// dispatch evidence.
    #[doc(hidden)]
    pub fn direct_kfd_runtime_coordinates_v2(
        &self,
        device: &CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
    ) -> Result<GeneratedHostRuntimeCoordinatesV2, GeneratedHostContractErrorV2> {
        let target = self.subject().target_model_identity();
        let context = generated_host_kfd_context_identity(target, device);
        let stream =
            generated_host_kfd_stream_identity::<K>(context, geometry, dynamic_group_segment_bytes);
        GeneratedHostRuntimeCoordinatesV2::new(target, context, stream)
    }

    /// Checks the complete V2 host contract before entering the existing real direct-KFD prepared
    /// invocation boundary. The returned invocation retains both the original KFD authority and the
    /// inert checked-host receipt until synchronous completion.
    #[allow(clippy::too_many_arguments)]
    #[doc(hidden)]
    pub fn prepare_direct_kfd_invocation_v2<'allocation, Arguments>(
        &self,
        authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
        evidence: GeneratedHostDispatchEvidenceV2,
        arguments: Arguments,
        device: CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Result<GeneratedWorkerV3KfdInvocation<'allocation, K>, GeneratedWorkerV3KfdInvocationError>
    where
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>
            + CompilerGeneratedHostArgumentsV2<'allocation, K>
            + 'allocation,
    {
        if !authenticated
            .verification()
            .retains_protected_application_execution_evidence()
        {
            return Err(
                GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable,
            );
        }
        let capability_source = authenticated
            .verification()
            .compiler_capability_source()
            .ok_or(GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable)?;
        self.validate_sealed_v13_source_v2(capability_source)
            .map_err(GeneratedWorkerV3KfdInvocationError::HostContractCoordinates)?;

        let observed = self
            .direct_kfd_runtime_coordinates_v2(&device, geometry, dynamic_group_segment_bytes)
            .map_err(GeneratedWorkerV3KfdInvocationError::HostContractCoordinates)?;
        let host_geometry = GeneratedHostLaunchGeometryV2::new(
            geometry.grid(),
            geometry.workgroup().map(u32::from),
            dynamic_group_segment_bytes,
        );
        let checked = self
            .check_dispatch_v2(evidence, observed, host_geometry, &arguments)
            .map_err(GeneratedWorkerV3KfdInvocationError::HostContract)?;

        let authenticated_plan = validate_worker_v3_argument_packing(
            authenticated.admission().descriptor_table(),
            authenticated.admission().descriptor(),
            K::generated_host_contract_v2()
                .map_err(GeneratedKfdPrepareError::GeneratedLayout)
                .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?
                .arguments(),
        )
        .map_err(GeneratedKfdPrepareError::PackingPlan)
        .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
        if &authenticated_plan != checked.packing_plan() {
            return Err(GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution);
        }
        let authenticated_table = authenticated.admission().descriptor_table();
        let authenticated_descriptor = authenticated.admission().descriptor();
        if authenticated_table.canonical_code_object_digest() != checked.artifact_identity()
            || fe2o3_kernel_descriptor::DeviceDescriptorTableDigest::calculate(authenticated_table)
                .map_err(|_| GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution)?
                != checked.descriptor_table_identity()
            || fe2o3_kernel_descriptor::KernelDescriptorDigest::calculate(authenticated_descriptor)
                != checked.descriptor_identity()
            || authenticated_descriptor.entry_name().as_str() != checked.entry_name()
            || authenticated_table.kernels().get(checked.kernel_ordinal())
                != Some(authenticated_descriptor)
        {
            return Err(GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution);
        }

        let mut invocation = authenticated.prepare_generated_kfd_invocation(
            arguments,
            device,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
        )?;
        invocation.generated_host = Some(checked);
        Ok(invocation)
    }

    /// Checks and prepares one marker selected by type from an authenticated aggregate roster.
    ///
    /// The application can be borrowed again after this invocation completes, which supports
    /// multiple kernels without a source name, profile, or ordinal selector at the API boundary.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_capability_direct_kfd_invocation_v2<
        'application,
        'allocation,
        R,
        Arguments,
    >(
        &self,
        authenticated: &'application AuthenticatedWorkerV3CapabilityApplicationV1<R>,
        arguments: Arguments,
        mut device: CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Result<
        GeneratedWorkerV3CapabilityKfdInvocation<'application, 'allocation, R, K>,
        GeneratedWorkerV3KfdInvocationError,
    >
    where
        R: CompilerGeneratedKernelExpectationRosterV1,
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>
            + CompilerGeneratedHostArgumentsV2<'allocation, K>
            + 'allocation,
    {
        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        let expected = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
            .expect("the canonical production gfx942 target is valid");
        let artifact_target = authenticated.roster().target();
        if artifact_target != expected {
            return Err(GeneratedWorkerV3KfdInvocationError::TargetMismatch {
                artifact: artifact_target,
            });
        }

        let observed = self
            .direct_kfd_runtime_coordinates_v2(&device, geometry, dynamic_group_segment_bytes)
            .map_err(GeneratedWorkerV3KfdInvocationError::HostContractCoordinates)?;
        let (checked, packed) = prepare_capability_target_arguments_v2(
            self,
            authenticated,
            arguments,
            observed,
            geometry,
            dynamic_group_segment_bytes,
        )?;
        let (inputs, completion) =
            packed.into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds);
        let prepared = prepare_gfx942_runtime_dispatch_v1(
            authenticated.roster().exact_current_hsaco_bytes(),
            K::EXPORT_NAME,
            inputs,
        )
        .map_err(GeneratedWorkerV3KfdInvocationError::RuntimePreparation)?;
        let verification = authenticated.roster().verification();
        validate_runtime_identity_fields(
            verification.finalized_hsaco_sha256(),
            verification.finalized_hsaco_length(),
            K::EXPORT_NAME,
            prepared.identity().object_sha256(),
            prepared.finalized_hsaco_length(),
            prepared.kernel_name(),
        )?;
        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        authenticated
            .revalidate_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;

        Ok(GeneratedWorkerV3CapabilityKfdInvocation {
            authority: GeneratedWorkerV3CapabilityKfdExecutionAuthority {
                application: authenticated,
                finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
                finalized_hsaco_length: verification.finalized_hsaco_length(),
                dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
                device_unique_id: device.observation().unique_id(),
                _marker: std::marker::PhantomData,
            },
            device,
            prepared,
            completion,
            generated_host: checked,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_capability_target_arguments_v2<'allocation, R, K, Arguments>(
    contract: &AdmittedGeneratedHostContractV2<K>,
    authenticated: &AuthenticatedWorkerV3CapabilityApplicationV1<R>,
    arguments: Arguments,
    observed: GeneratedHostRuntimeCoordinatesV2,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Result<
    (
        CheckedGeneratedHostDispatchV2<K>,
        crate::GeneratedKfdPackedArguments<'allocation>,
    ),
    GeneratedWorkerV3KfdInvocationError,
>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV2,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>
        + CompilerGeneratedHostArgumentsV2<'allocation, K>
        + 'allocation,
{
    authenticated
        .revalidate_currentness()
        .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
    let entry = authenticated
        .roster()
        .entry::<K>()
        .map_err(|_| GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution)?;
    let table = authenticated.roster().descriptor_table();
    let descriptor = entry.descriptor();
    let evidence = contract.bind_dispatch_evidence_v2(observed);
    let host_geometry = GeneratedHostLaunchGeometryV2::new(
        geometry.grid(),
        geometry.workgroup().map(u32::from),
        dynamic_group_segment_bytes,
    );
    let checked = contract
        .check_dispatch_v2(evidence, observed, host_geometry, &arguments)
        .map_err(GeneratedWorkerV3KfdInvocationError::HostContract)?;
    let generated = Arguments::generated_argument_layout()
        .map_err(GeneratedKfdPrepareError::GeneratedLayout)
        .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
    let plan = validate_worker_v3_argument_packing(table, descriptor, &generated)
        .map_err(GeneratedKfdPrepareError::PackingPlan)
        .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
    if &plan != checked.packing_plan()
        || table.canonical_code_object_digest() != checked.artifact_identity()
        || fe2o3_kernel_descriptor::DeviceDescriptorTableDigest::calculate(table)
            .map_err(|_| GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution)?
            != checked.descriptor_table_identity()
        || fe2o3_kernel_descriptor::KernelDescriptorDigest::calculate(descriptor)
            != checked.descriptor_identity()
        || descriptor.entry_name().as_str() != checked.entry_name()
        || table.kernels().get(checked.kernel_ordinal()) != Some(descriptor)
    {
        return Err(GeneratedWorkerV3KfdInvocationError::HostDescriptorSubstitution);
    }
    let packed = arguments
        .bind_kfd_arguments(&plan)
        .map_err(GeneratedKfdPrepareError::Bind)
        .and_then(|binding| binding.pack(&plan).map_err(GeneratedKfdPrepareError::Bind))
        .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
    if packed.kernel_id() != descriptor.kernel_id()
        || packed.explicit_kernarg().len()
            != usize::try_from(descriptor.abi_layout().explicit_argument_size())
                .unwrap_or(usize::MAX)
        || packed.alignment() != descriptor.abi_layout().kernarg_segment_alignment()
    {
        return Err(GeneratedWorkerV3KfdInvocationError::Arguments(
            GeneratedKfdPrepareError::PackedSubstitution,
        ));
    }
    authenticated
        .revalidate_currentness()
        .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
    Ok((checked, packed))
}

#[allow(clippy::too_many_arguments)]
fn application_execution_admission<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &mut AuthenticatedWorkerV3ExecutableV1<K>,
    device: &CheckedGfx942XnackMinusDevice,
    packed_kernel_id: crate::KernelId,
    packing: &GeneratedKfdPackingObservationV1,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
    dispatch_contract_sha256: [u8; 32],
) -> Option<WorkerV3ApplicationExecutionAdmissionV1> {
    let refinement = authenticated.take_application_semantic_machine_refinement()?;
    let compiler_capability_association = refinement.compiler_capability_association_identity();
    let production_capability_result = refinement.production_capability_result_identity();
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let (compiler_capability_association_sha256, compiler_capability_association_bytes) =
        compiler_capability_association?;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let (compiler_capability_association_sha256, compiler_capability_association_bytes) =
        compiler_capability_association.unwrap_or(([0; 32], 0));
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let (production_capability_result_sha256, production_capability_result_bytes) =
        production_capability_result?;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let (production_capability_result_sha256, production_capability_result_bytes) =
        production_capability_result.unwrap_or(([0; 32], 0));
    let verification = authenticated.verification();
    let proof = verification.validated_compiler_proof_inputs()?;
    let target_lineage = verification.validated_compiler_target_lineage()?;
    let proof_binding = proof.receipt_identity();
    let production_kir = proof.kernel_ir().identity();
    let target_binding = target_lineage.target_binding_receipt_identity();
    let data_layout = target_lineage.data_layout_receipt_identity();
    let semantic_to_llvm = target_lineage.semantic_to_llvm_receipt_identity();
    let final_llvm = target_lineage.final_llvm_identity();
    let final_module_commitment = target_lineage.final_compiler_module_commitment_identity();
    let compiler_subject = authenticated
        .admission()
        .compiler_execution_subject()
        .identity();
    let compiler_execution = verification.compiler_execution();
    let mut coordinates = WorkerV3ApplicationExecutionCoordinatesV1 {
        identity: [0; 32],
        kernel_id: *packed_kernel_id.as_bytes(),
        logical_name: K::LOGICAL_NAME,
        export_name: K::EXPORT_NAME,
        kernel_binding_identity: K::KERNEL_BINDING_ID_V1,
        generated_host_contract_identity: K::PROFILE.generated_host_contract_identity(),
        direct_kfd_runtime_contract: GENERATED_WORKER_V3_DIRECT_KFD_RUNTIME_CONTRACT_V1,
        worker_challenge_identity: *verification.challenge_identity().as_bytes(),
        worker_lineage_identity: *verification.lineage_identity().as_bytes(),
        compiler_execution_subject_identity: *compiler_subject.sha256(),
        compiler_execution_carriage_identity: compiler_execution.carriage_sha256(),
        compiler_execution_policy_identity: compiler_execution.policy_sha256(),
        compiler_execution_issuer_journal_identity: compiler_execution.issuer_journal_sha256(),
        compiler_execution_occurrence_identity: compiler_execution.compiler_occurrence_sha256(),
        compiler_execution_receipt_identity: compiler_execution.receipt_sha256(),
        compiler_execution_publication_identity: compiler_execution.publication_sha256(),
        compiler_execution_acknowledgment_identity: compiler_execution.acknowledgment_sha256(),
        compiler_execution_worker_ledger_record_identity: compiler_execution
            .worker_ledger_record_sha256(),
        compiler_execution_sequence: compiler_execution.sequence(),
        compiler_execution_prior_rollback_anchor: compiler_execution.prior_rollback_anchor(),
        compiler_execution_current_rollback_anchor: compiler_execution.current_rollback_anchor(),
        compiler_current_record_verification_identity: compiler_execution
            .current_record_verification_sha256(),
        compiler_current_record_attestation_identity: compiler_execution
            .current_record_attestation_sha256(),
        protected_compiler_policy_verification_identity: compiler_execution
            .protected_policy_verification_sha256(),
        protected_worker_ledger_verification_identity: compiler_execution
            .protected_worker_ledger_verification_sha256(),
        external_rollback_verification_identity: compiler_execution
            .external_rollback_verification_sha256(),
        finalizer_derivation_identity: *verification.finalizer_derivation().identity().as_bytes(),
        proof_binding_sha256: *proof_binding.sha256(),
        proof_binding_bytes: proof_binding.byte_len(),
        production_kir_v8_sha256: *production_kir.digest(),
        production_kir_v8_bytes: production_kir.canonical_length(),
        target_binding_sha256: target_binding.sha256(),
        target_binding_bytes: target_binding.byte_len(),
        data_layout_sha256: data_layout.sha256(),
        data_layout_bytes: data_layout.byte_len(),
        semantic_to_llvm_sha256: semantic_to_llvm.sha256(),
        semantic_to_llvm_bytes: semantic_to_llvm.byte_len(),
        final_llvm_sha256: final_llvm.sha256(),
        final_llvm_bytes: final_llvm.byte_len(),
        final_module_commitment_sha256: final_module_commitment.sha256(),
        final_module_commitment_bytes: final_module_commitment.byte_len(),
        finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
        finalized_hsaco_bytes: verification.finalized_hsaco_length(),
        target: authenticated.target().to_string(),
        code_object_version: u16::from(verification.code_object_version().number()),
        verifier_measurement_sha256: verification.verifier_measurement_sha256(),
        verification_transcript_sha256: verification.verification_transcript_sha256(),
        proof_executable_binding_sha256: verification.proof_executable_binding_sha256(),
        rust_type_layout_contract_sha256: verification.rust_type_layout_contract_sha256(),
        rust_effect_contract_sha256: verification.rust_effect_contract_sha256(),
        semantic_machine_refinement_receipt_identity: *refinement.receipt().identity(),
        compiler_capability_association_sha256,
        compiler_capability_association_bytes,
        production_capability_result_sha256,
        production_capability_result_bytes,
        safety_properties: verification.safety_properties().bits(),
        dispatch_contract_sha256,
        grid: geometry.grid(),
        workgroup: geometry.workgroup(),
        dynamic_group_segment_bytes,
        timeout_milliseconds,
        device_unique_id: device.observation().unique_id(),
        device_topology_identity: device_topology_identity(device),
        packing_identity: *packing.identity(),
    };
    coordinates.identity = application_execution_binding_identity(&coordinates);
    Some(WorkerV3ApplicationExecutionAdmissionV1::new(
        coordinates,
        refinement,
    ))
}

struct WorkerV3ApplicationExecutionAdmissionV1 {
    coordinates: WorkerV3ApplicationExecutionCoordinatesV1,
    refinement: AdmittedWorkerV3SemanticMachineRefinementV1,
}

impl WorkerV3ApplicationExecutionAdmissionV1 {
    fn new(
        coordinates: WorkerV3ApplicationExecutionCoordinatesV1,
        refinement: AdmittedWorkerV3SemanticMachineRefinementV1,
    ) -> Self {
        debug_assert_eq!(
            coordinates.semantic_machine_refinement_receipt_identity,
            *refinement.receipt().identity()
        );
        Self {
            coordinates,
            refinement,
        }
    }
}

fn application_execution_binding_identity(
    coordinates: &WorkerV3ApplicationExecutionCoordinatesV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(APPLICATION_EXECUTION_BINDING_DOMAIN_V1);
    hasher.update(coordinates.kernel_id);
    hash_bytes(&mut hasher, coordinates.logical_name.as_bytes());
    hash_bytes(&mut hasher, coordinates.export_name.as_bytes());
    hasher.update(coordinates.kernel_binding_identity);
    hasher.update(coordinates.generated_host_contract_identity);
    hash_bytes(
        &mut hasher,
        coordinates.direct_kfd_runtime_contract.as_bytes(),
    );
    hasher.update(coordinates.worker_challenge_identity);
    hasher.update(coordinates.worker_lineage_identity);
    hasher.update(coordinates.compiler_execution_subject_identity);
    hasher.update(coordinates.compiler_execution_carriage_identity);
    hasher.update(coordinates.compiler_execution_policy_identity);
    hasher.update(coordinates.compiler_execution_issuer_journal_identity);
    hasher.update(coordinates.compiler_execution_occurrence_identity);
    hasher.update(coordinates.compiler_execution_receipt_identity);
    hasher.update(coordinates.compiler_execution_publication_identity);
    hasher.update(coordinates.compiler_execution_acknowledgment_identity);
    hasher.update(coordinates.compiler_execution_worker_ledger_record_identity);
    hasher.update(coordinates.compiler_execution_sequence.to_le_bytes());
    hasher.update(coordinates.compiler_execution_prior_rollback_anchor);
    hasher.update(coordinates.compiler_execution_current_rollback_anchor);
    hasher.update(coordinates.compiler_current_record_verification_identity);
    hasher.update(coordinates.compiler_current_record_attestation_identity);
    hasher.update(coordinates.protected_compiler_policy_verification_identity);
    hasher.update(coordinates.protected_worker_ledger_verification_identity);
    hasher.update(coordinates.external_rollback_verification_identity);
    hasher.update(coordinates.finalizer_derivation_identity);
    hash_identity(
        &mut hasher,
        coordinates.proof_binding_sha256,
        coordinates.proof_binding_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.production_kir_v8_sha256,
        coordinates.production_kir_v8_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.target_binding_sha256,
        coordinates.target_binding_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.data_layout_sha256,
        coordinates.data_layout_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.semantic_to_llvm_sha256,
        coordinates.semantic_to_llvm_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.final_llvm_sha256,
        coordinates.final_llvm_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.final_module_commitment_sha256,
        coordinates.final_module_commitment_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.finalized_hsaco_sha256,
        coordinates.finalized_hsaco_bytes,
    );
    hash_bytes(&mut hasher, coordinates.target.as_bytes());
    hasher.update(coordinates.code_object_version.to_le_bytes());
    hasher.update(coordinates.verifier_measurement_sha256);
    hasher.update(coordinates.verification_transcript_sha256);
    hasher.update(coordinates.proof_executable_binding_sha256);
    hasher.update(coordinates.rust_type_layout_contract_sha256);
    hasher.update(coordinates.rust_effect_contract_sha256);
    hasher.update(coordinates.semantic_machine_refinement_receipt_identity);
    hash_identity(
        &mut hasher,
        coordinates.compiler_capability_association_sha256,
        coordinates.compiler_capability_association_bytes,
    );
    hash_identity(
        &mut hasher,
        coordinates.production_capability_result_sha256,
        coordinates.production_capability_result_bytes,
    );
    hasher.update([coordinates.safety_properties]);
    hasher.update(coordinates.dispatch_contract_sha256);
    for value in coordinates.grid {
        hasher.update(value.to_le_bytes());
    }
    for value in coordinates.workgroup {
        hasher.update(value.to_le_bytes());
    }
    hasher.update(coordinates.dynamic_group_segment_bytes.to_le_bytes());
    hasher.update(coordinates.timeout_milliseconds.to_le_bytes());
    hasher.update(coordinates.device_unique_id.to_le_bytes());
    hasher.update(coordinates.device_topology_identity);
    hasher.update(coordinates.packing_identity);
    hasher.finalize().into()
}

fn hash_identity(hasher: &mut Sha256, sha256: [u8; 32], byte_length: u64) {
    hasher.update(sha256);
    hasher.update(byte_length.to_le_bytes());
}

fn device_topology_identity(device: &CheckedGfx942XnackMinusDevice) -> [u8; 32] {
    let observation = device.observation();
    let process = device.process_incarnation();
    let correlation = device.projection().correlation();
    let mut hasher = Sha256::new();
    hasher.update(DEVICE_TOPOLOGY_BINDING_DOMAIN_V1);
    hasher.update(fe2o3_kfd::DEVICE_ADMISSION_PROFILE_SHA256_BYTES_V1);
    hasher.update(observation.topology_node_id().to_le_bytes());
    hasher.update(observation.kfd_gpu_id().to_le_bytes());
    hasher.update(observation.unique_id().to_le_bytes());
    hasher.update(correlation.identity().gpu_unique_id.to_le_bytes());
    hasher.update(correlation.drm_schema_identity().as_bytes());
    hasher.update(process.pid().to_le_bytes());
    hasher.update(process.start_time_ticks().to_le_bytes());
    hasher.update(process.mount_namespace_device().to_le_bytes());
    hasher.update(process.mount_namespace_inode().to_le_bytes());
    hasher.finalize().into()
}

fn generated_host_kfd_context_identity(
    target_model_identity: [u8; 32],
    device: &CheckedGfx942XnackMinusDevice,
) -> [u8; 32] {
    generated_host_kfd_context_identity_from_parts(
        target_model_identity,
        device.observation().unique_id(),
        device_topology_identity(device),
    )
}

fn generated_host_kfd_context_identity_from_parts(
    target_model_identity: [u8; 32],
    device_unique_id: u64,
    device_topology_identity: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(GENERATED_HOST_KFD_CONTEXT_DOMAIN_V2);
    hasher.update(target_model_identity);
    hasher.update(device_unique_id.to_le_bytes());
    hasher.update(device_topology_identity);
    hasher.finalize().into()
}

fn generated_host_kfd_stream_identity<K: CompilerGeneratedKernelExpectationV2>(
    context_identity: [u8; 32],
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> [u8; 32] {
    generated_host_kfd_stream_identity_from_parts(
        context_identity,
        K::KERNEL_BINDING_ID_V1,
        geometry.grid(),
        geometry.workgroup(),
        dynamic_group_segment_bytes,
    )
}

fn generated_host_kfd_stream_identity_from_parts(
    context_identity: [u8; 32],
    kernel_binding_identity: [u8; 32],
    grid: [u32; 3],
    workgroup: [u16; 3],
    dynamic_group_segment_bytes: u32,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(GENERATED_HOST_KFD_STREAM_DOMAIN_V2);
    hasher.update(context_identity);
    hasher.update(kernel_binding_identity);
    for value in grid {
        hasher.update(value.to_le_bytes());
    }
    for value in workgroup {
        hasher.update(value.to_le_bytes());
    }
    hasher.update(dynamic_group_segment_bytes.to_le_bytes());
    hasher.finalize().into()
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
    let compiler_capability = verification.compiler_capability_source();
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let compiler_capability = compiler_capability?;
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let compiler_capability_identity = compiler_capability.capability().association().identity();
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let compiler_capability_identity =
        compiler_capability.map(|source| source.capability().association().identity());
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let (compiler_capability_association_sha256, compiler_capability_association_bytes) = (
        compiler_capability_identity.sha256(),
        compiler_capability_identity.byte_len(),
    );
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let (compiler_capability_association_sha256, compiler_capability_association_bytes) =
        compiler_capability_identity
            .map(|identity| (identity.sha256(), identity.byte_len()))
            .unwrap_or(([0; 32], 0));
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let production_capability_result_identity = compiler_capability.production_result_identity();
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let production_capability_result_identity =
        compiler_capability.map(|source| source.production_result_identity());
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let (production_capability_result_sha256, production_capability_result_bytes) = (
        production_capability_result_identity.sha256(),
        production_capability_result_identity.byte_len(),
    );
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let (production_capability_result_sha256, production_capability_result_bytes) =
        production_capability_result_identity
            .map(|identity| (identity.sha256(), identity.byte_len()))
            .unwrap_or(([0; 32], 0));
    let compiler_subject = authenticated
        .admission()
        .compiler_execution_subject()
        .identity();
    let compiler_execution = verification.compiler_execution();
    let finalizer = verification.finalizer_derivation().identity();
    let observation = device.observation();

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
        compiler_capability_association_sha256,
        compiler_capability_association_bytes,
        production_capability_result_sha256,
        production_capability_result_bytes,
        finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
        finalized_hsaco_bytes: verification.finalized_hsaco_length(),
        target: authenticated.target().to_string(),
        dispatch_contract_sha256: [0; 32],
        grid: geometry.grid(),
        workgroup: geometry.workgroup(),
        dynamic_group_segment_bytes,
        device_unique_id: observation.unique_id(),
        device_topology_identity: device_topology_identity(device),
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
    hash_identity(
        &mut hasher,
        binding.compiler_capability_association_sha256,
        binding.compiler_capability_association_bytes,
    );
    hash_identity(
        &mut hasher,
        binding.production_capability_result_sha256,
        binding.production_capability_result_bytes,
    );
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
    HostContractCoordinates(GeneratedHostContractErrorV2),
    HostContract(GeneratedHostPrepareErrorV2),
    HostDescriptorSubstitution,
    ProtectedProductionEvidenceUnavailable,
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
            Self::HostContractCoordinates(error) => {
                write!(formatter, "direct-KFD host coordinates are invalid: {error}")
            }
            Self::HostContract(error) => {
                write!(formatter, "direct-KFD host contract rejected: {error}")
            }
            Self::HostDescriptorSubstitution => formatter.write_str(
                "direct-KFD authenticated descriptor disagrees with the checked host contract",
            ),
            Self::ProtectedProductionEvidenceUnavailable => formatter.write_str(
                "an admitted Worker V3 semantic-to-machine refinement receipt is unavailable for application release",
            ),
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
            Self::HostContractCoordinates(error) => Some(error),
            Self::HostContract(error) => Some(error),
            Self::TargetMismatch { .. }
            | Self::HostDescriptorSubstitution
            | Self::ProtectedProductionEvidenceUnavailable
            | Self::ArtifactIdentityMismatch
            | Self::ArtifactLengthMismatch
            | Self::KernelNameMismatch => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3KfdExecutionError {
    ProductionAuthorityUnavailable,
    Runtime(Gfx942AuthorizedRuntimeExecutionErrorV1<RecoveredWorkerV3AdmissionErrorV1>),
    Completion(GeneratedKfdCompletionError),
    DifferentialEvidenceUnavailable,
}

impl fmt::Display for GeneratedWorkerV3KfdExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProductionAuthorityUnavailable => formatter.write_str(
                "protected production Worker V3 execution authority is unavailable",
            ),
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
            Self::ProductionAuthorityUnavailable | Self::DifferentialEvidenceUnavailable => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_v3_verification_admission::admitted_semantic_machine_refinement_for_test_v1;

    fn application_coordinates_fixture() -> WorkerV3ApplicationExecutionCoordinatesV1 {
        WorkerV3ApplicationExecutionCoordinatesV1 {
            identity: [0; 32],
            kernel_id: [1; 32],
            logical_name: "logical_v1",
            export_name: "export_v1",
            kernel_binding_identity: [2; 32],
            generated_host_contract_identity: [3; 32],
            direct_kfd_runtime_contract: "runtime_v1",
            worker_challenge_identity: [4; 32],
            worker_lineage_identity: [5; 32],
            compiler_execution_subject_identity: [6; 32],
            compiler_execution_carriage_identity: [7; 32],
            compiler_execution_policy_identity: [8; 32],
            compiler_execution_issuer_journal_identity: [9; 32],
            compiler_execution_occurrence_identity: [10; 32],
            compiler_execution_receipt_identity: [11; 32],
            compiler_execution_publication_identity: [12; 32],
            compiler_execution_acknowledgment_identity: [13; 32],
            compiler_execution_worker_ledger_record_identity: [14; 32],
            compiler_execution_sequence: 15,
            compiler_execution_prior_rollback_anchor: [16; 32],
            compiler_execution_current_rollback_anchor: [17; 32],
            compiler_current_record_verification_identity: [18; 32],
            compiler_current_record_attestation_identity: [19; 32],
            protected_compiler_policy_verification_identity: [20; 32],
            protected_worker_ledger_verification_identity: [21; 32],
            external_rollback_verification_identity: [22; 32],
            finalizer_derivation_identity: [23; 32],
            proof_binding_sha256: [24; 32],
            proof_binding_bytes: 25,
            production_kir_v8_sha256: [26; 32],
            production_kir_v8_bytes: 27,
            target_binding_sha256: [28; 32],
            target_binding_bytes: 29,
            data_layout_sha256: [30; 32],
            data_layout_bytes: 31,
            semantic_to_llvm_sha256: [32; 32],
            semantic_to_llvm_bytes: 33,
            final_llvm_sha256: [34; 32],
            final_llvm_bytes: 35,
            final_module_commitment_sha256: [36; 32],
            final_module_commitment_bytes: 37,
            finalized_hsaco_sha256: [38; 32],
            finalized_hsaco_bytes: 39,
            target: "gfx942:xnack-".to_string(),
            code_object_version: 6,
            verifier_measurement_sha256: [40; 32],
            verification_transcript_sha256: [41; 32],
            proof_executable_binding_sha256: [42; 32],
            rust_type_layout_contract_sha256: [43; 32],
            rust_effect_contract_sha256: [44; 32],
            semantic_machine_refinement_receipt_identity: [49; 32],
            compiler_capability_association_sha256: [50; 32],
            compiler_capability_association_bytes: 51,
            production_capability_result_sha256: [52; 32],
            production_capability_result_bytes: 53,
            safety_properties: u8::MAX,
            dispatch_contract_sha256: [45; 32],
            grid: [64, 2, 1],
            workgroup: [32, 1, 1],
            dynamic_group_segment_bytes: 128,
            timeout_milliseconds: 30_000,
            device_unique_id: 46,
            device_topology_identity: [47; 32],
            packing_identity: [48; 32],
        }
    }

    #[test]
    fn qualification_custody_cannot_cross_the_production_execution_gate() {
        let production = ProductionExecutionCustodyV1::<u32, u32>::Production(17);
        assert_eq!(production.into_production(), Ok(17));

        let qualification = ProductionExecutionCustodyV1::<u32, u32>::Qualification(29);
        assert_eq!(qualification.into_production(), Err(29));
    }

    #[test]
    fn direct_kfd_application_owner_retains_the_consumed_refinement_receipt() {
        let refinement = admitted_semantic_machine_refinement_for_test_v1();
        let expected = *refinement.receipt().identity();
        let mut coordinates = application_coordinates_fixture();
        coordinates.semantic_machine_refinement_receipt_identity = expected;
        coordinates.identity = application_execution_binding_identity(&coordinates);

        let admitted = WorkerV3ApplicationExecutionAdmissionV1::new(coordinates, refinement);

        assert_eq!(admitted.refinement.receipt().identity(), &expected);
        assert_eq!(
            admitted
                .coordinates
                .semantic_machine_refinement_receipt_identity,
            expected
        );
    }

    #[test]
    fn application_execution_identity_fails_closed_for_every_bound_coordinate() {
        let baseline = application_coordinates_fixture();
        let expected = application_execution_binding_identity(&baseline);
        assert_ne!(expected, [0; 32]);
        assert_eq!(expected, application_execution_binding_identity(&baseline));

        macro_rules! assert_substitution_changes_identity {
            ($field:ident, $value:expr) => {{
                let mut substituted = baseline.clone();
                substituted.$field = $value;
                assert_ne!(
                    expected,
                    application_execution_binding_identity(&substituted),
                    "{} substitution was not bound",
                    stringify!($field)
                );
            }};
        }

        assert_substitution_changes_identity!(kernel_id, [34; 32]);
        assert_substitution_changes_identity!(logical_name, "other_logical");
        assert_substitution_changes_identity!(export_name, "other_export");
        assert_substitution_changes_identity!(kernel_binding_identity, [35; 32]);
        assert_substitution_changes_identity!(generated_host_contract_identity, [36; 32]);
        assert_substitution_changes_identity!(direct_kfd_runtime_contract, "other_runtime");
        assert_substitution_changes_identity!(worker_challenge_identity, [37; 32]);
        assert_substitution_changes_identity!(worker_lineage_identity, [38; 32]);
        assert_substitution_changes_identity!(compiler_execution_subject_identity, [39; 32]);
        assert_substitution_changes_identity!(compiler_execution_carriage_identity, [40; 32]);
        assert_substitution_changes_identity!(compiler_execution_policy_identity, [41; 32]);
        assert_substitution_changes_identity!(compiler_execution_issuer_journal_identity, [42; 32]);
        assert_substitution_changes_identity!(compiler_execution_occurrence_identity, [43; 32]);
        assert_substitution_changes_identity!(compiler_execution_receipt_identity, [40; 32]);
        assert_substitution_changes_identity!(compiler_execution_publication_identity, [44; 32]);
        assert_substitution_changes_identity!(compiler_execution_acknowledgment_identity, [45; 32]);
        assert_substitution_changes_identity!(
            compiler_execution_worker_ledger_record_identity,
            [46; 32]
        );
        assert_substitution_changes_identity!(compiler_execution_sequence, 47);
        assert_substitution_changes_identity!(compiler_execution_prior_rollback_anchor, [48; 32]);
        assert_substitution_changes_identity!(compiler_execution_current_rollback_anchor, [49; 32]);
        assert_substitution_changes_identity!(
            compiler_current_record_verification_identity,
            [50; 32]
        );
        assert_substitution_changes_identity!(
            compiler_current_record_attestation_identity,
            [51; 32]
        );
        assert_substitution_changes_identity!(
            protected_compiler_policy_verification_identity,
            [52; 32]
        );
        assert_substitution_changes_identity!(
            protected_worker_ledger_verification_identity,
            [53; 32]
        );
        assert_substitution_changes_identity!(external_rollback_verification_identity, [54; 32]);
        assert_substitution_changes_identity!(finalizer_derivation_identity, [41; 32]);
        assert_substitution_changes_identity!(proof_binding_sha256, [42; 32]);
        assert_substitution_changes_identity!(proof_binding_bytes, 43);
        assert_substitution_changes_identity!(production_kir_v8_sha256, [44; 32]);
        assert_substitution_changes_identity!(production_kir_v8_bytes, 45);
        assert_substitution_changes_identity!(target_binding_sha256, [46; 32]);
        assert_substitution_changes_identity!(target_binding_bytes, 47);
        assert_substitution_changes_identity!(data_layout_sha256, [48; 32]);
        assert_substitution_changes_identity!(data_layout_bytes, 49);
        assert_substitution_changes_identity!(semantic_to_llvm_sha256, [50; 32]);
        assert_substitution_changes_identity!(semantic_to_llvm_bytes, 51);
        assert_substitution_changes_identity!(final_llvm_sha256, [52; 32]);
        assert_substitution_changes_identity!(final_llvm_bytes, 53);
        assert_substitution_changes_identity!(final_module_commitment_sha256, [54; 32]);
        assert_substitution_changes_identity!(final_module_commitment_bytes, 55);
        assert_substitution_changes_identity!(finalized_hsaco_sha256, [56; 32]);
        assert_substitution_changes_identity!(finalized_hsaco_bytes, 57);
        assert_substitution_changes_identity!(target, "gfx942:xnack+".to_string());
        assert_substitution_changes_identity!(code_object_version, 5);
        assert_substitution_changes_identity!(verifier_measurement_sha256, [58; 32]);
        assert_substitution_changes_identity!(verification_transcript_sha256, [59; 32]);
        assert_substitution_changes_identity!(proof_executable_binding_sha256, [60; 32]);
        assert_substitution_changes_identity!(rust_type_layout_contract_sha256, [61; 32]);
        assert_substitution_changes_identity!(rust_effect_contract_sha256, [62; 32]);
        assert_substitution_changes_identity!(
            semantic_machine_refinement_receipt_identity,
            [67; 32]
        );
        assert_substitution_changes_identity!(compiler_capability_association_sha256, [68; 32]);
        assert_substitution_changes_identity!(compiler_capability_association_bytes, 69);
        assert_substitution_changes_identity!(production_capability_result_sha256, [70; 32]);
        assert_substitution_changes_identity!(production_capability_result_bytes, 71);
        assert_substitution_changes_identity!(safety_properties, 0x7f);
        assert_substitution_changes_identity!(dispatch_contract_sha256, [63; 32]);
        assert_substitution_changes_identity!(grid, [65, 2, 1]);
        assert_substitution_changes_identity!(workgroup, [16, 1, 1]);
        assert_substitution_changes_identity!(dynamic_group_segment_bytes, 129);
        assert_substitution_changes_identity!(timeout_milliseconds, 29_999);
        assert_substitution_changes_identity!(device_unique_id, 64);
        assert_substitution_changes_identity!(device_topology_identity, [65; 32]);
        assert_substitution_changes_identity!(packing_identity, [66; 32]);
    }

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

    #[test]
    fn generated_host_kfd_coordinates_bind_target_device_kernel_stream_and_launch() {
        let context = generated_host_kfd_context_identity_from_parts([1; 32], 2, [3; 32]);
        assert_eq!(
            context,
            generated_host_kfd_context_identity_from_parts([1; 32], 2, [3; 32])
        );
        assert_ne!(
            context,
            generated_host_kfd_context_identity_from_parts([4; 32], 2, [3; 32])
        );
        assert_ne!(
            context,
            generated_host_kfd_context_identity_from_parts([1; 32], 5, [3; 32])
        );
        assert_ne!(
            context,
            generated_host_kfd_context_identity_from_parts([1; 32], 2, [6; 32])
        );

        let stream = generated_host_kfd_stream_identity_from_parts(
            context,
            [7; 32],
            [256, 1, 1],
            [64, 1, 1],
            128,
        );
        assert_eq!(
            stream,
            generated_host_kfd_stream_identity_from_parts(
                context,
                [7; 32],
                [256, 1, 1],
                [64, 1, 1],
                128,
            )
        );
        for substituted in [
            generated_host_kfd_stream_identity_from_parts(
                [8; 32],
                [7; 32],
                [256, 1, 1],
                [64, 1, 1],
                128,
            ),
            generated_host_kfd_stream_identity_from_parts(
                context,
                [9; 32],
                [256, 1, 1],
                [64, 1, 1],
                128,
            ),
            generated_host_kfd_stream_identity_from_parts(
                context,
                [7; 32],
                [257, 1, 1],
                [64, 1, 1],
                128,
            ),
            generated_host_kfd_stream_identity_from_parts(
                context,
                [7; 32],
                [256, 1, 1],
                [32, 1, 1],
                128,
            ),
            generated_host_kfd_stream_identity_from_parts(
                context,
                [7; 32],
                [256, 1, 1],
                [64, 1, 1],
                129,
            ),
        ] {
            assert_ne!(stream, substituted);
        }
    }
}
