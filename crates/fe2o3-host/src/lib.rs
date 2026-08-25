#[cfg(target_os = "linux")]
mod application_descriptor_handoff;
mod argument_alias;
mod artifact_binding;
mod cooperative_launch;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod flash_attention_v1_lifecycle;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_alpha_zeta_cov6;
mod generated_argument_plan;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_flash_attention_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_lds_gemm;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_lds_gemm_lifecycle;
#[cfg(test)]
mod generated_lds_gemm_lifecycle_tests;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_moe_expert_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_moe_expert_v2;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_moe_top2_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_row_softmax_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_scalar_gemm_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_vecadd;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_wave64_collectives_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_wave64_collectives_v1_lifecycle;
#[cfg(test)]
mod generated_wave64_collectives_v1_lifecycle_tests;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_worker_v2_vecadd;
mod generated_worker_v3_dispatch;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_workgroup_lds_reduction_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_workgroup_scoped_atomic_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod generated_workgroup_sync_v1_lifecycle;
#[cfg(test)]
mod generated_workgroup_sync_v1_lifecycle_tests;
mod gfx942_ocml;
mod hsa_executable_lifecycle;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod launch_kernel_v2_bridge;
mod loaded_kernel;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod moe_expert_v1_denial;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod moe_routing_expert_bridge_v1;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod moe_routing_expert_bridge_v2;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod moe_top2_v1_lifecycle;
mod prepared_launch;
#[cfg(target_os = "linux")]
mod production_application;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod protected_row_softmax_v1_lifecycle;
mod published_direct_link;
mod published_hsaco_inspection;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod recovered_worker_v2_admission;
mod recovered_worker_v3_admission;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod row_softmax_protected_admission;
mod tile_interop;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod worker_v2_bundle_admission;
mod worker_v3_verification_admission;

#[cfg(feature = "hardware-test-hooks")]
#[doc(hidden)]
pub mod __hardware_test {
    #[cfg(feature = "qualification-oracles-test-only")]
    use fe2o3_artifact_transaction::DurableCurrentLinkPublicationTokenV1;
    use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};

    use crate::{
        AllocationProvenance, ArgumentAccess, ArgumentAccessMode, GeneratedSliceArgumentPairV1,
        ObservedContext,
    };

    #[cfg(feature = "qualification-oracles-test-only")]
    pub use crate::worker_v2_bundle_admission::tests::{
        TestDirectory, TestPublicationTurnover,
        admitted_alpha_zeta_cov6_hardware_for_lifecycle_test, admitted_hardware_for_lifecycle_test,
        begin_test_publication_turnover,
    };

    #[cfg(feature = "qualification-oracles-test-only")]
    pub fn acquire_retained_currentness_token<K>(
        authenticated: &crate::AuthenticatedWorkerV2ExecutableV1<K>,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, crate::FinalizedWorkerV2BundleAdmissionError>
    where
        K: crate::CompilerGeneratedKernelExpectationV1,
    {
        authenticated.acquire_retained_currentness_token()
    }

    #[cfg(feature = "qualification-oracles-test-only")]
    pub fn load_with_retained_currentness<K, A>(
        authorized: crate::AuthorizedHsaLoadV1<K, A>,
        current: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<crate::LoadedHsaExecutableV1<K, A>, crate::HsaExecutableLoadError<A::Error>>
    where
        A: crate::ReviewedHsaExecutableLifecycleAdapterV1,
    {
        authorized.load_with_retained_currentness(current)
    }

    /// Constructs inert device facts for a descriptor-handoff integration fixture.
    pub fn application_handoff_observed_context_fixture_v1(target: &str) -> ObservedContext {
        ObservedContext::for_test(0xf3_02, 0, target, 1_024, 65_536)
    }

    /// Constructs one shared-`f32` argument pair for an envelope integration test.
    ///
    /// # Safety
    ///
    /// Either `address..address + length * 4` must denote one live device allocation owned by
    /// `owner` in `observed` for the returned value's lifetime, or every value derived from this
    /// fixture must remain inside an inert test path whose adapter cannot submit work or access
    /// device memory. The latter case must not be passed to a live HSA adapter.
    pub unsafe fn generated_shared_f32_argument_pair_fixture_v1<'allocation, Owner: ?Sized>(
        observed: &ObservedContext,
        owner: &'allocation Owner,
        plan: &crate::GeneratedArgumentPackingPlanV1,
        argument_index: usize,
        address: usize,
        length: usize,
    ) -> GeneratedSliceArgumentPairV1<'allocation> {
        let byte_length = length.checked_mul(size_of::<f32>()).unwrap();
        // SAFETY: upheld by this test-only function's live-allocation-or-inert-adapter contract.
        let provenance = unsafe {
            AllocationProvenance::from_raw_parts(observed, owner, address as *mut u8, byte_length)
        }
        .unwrap();
        let access = ArgumentAccess::new(
            provenance.region(0, byte_length).unwrap(),
            ArgumentAccessMode::SharedRead,
        );
        // SAFETY: the test caller supplies the retained allocation or confines the value to an
        // inert adapter; the validated plan still checks index, physical width, space, and effect.
        let input = unsafe {
            plan.slice(
                argument_index,
                address as *const (),
                u64::try_from(length).unwrap(),
                PointerWidth::Bits64,
                AddressSpace::Global,
                Access::ReadOnly,
            )
        }
        .unwrap();
        GeneratedSliceArgumentPairV1::new(input, access)
    }
}

#[cfg(all(
    target_os = "linux",
    any(test, feature = "qualification-oracles-test-only")
))]
pub use application_descriptor_handoff::WorkerV2ApplicationDescriptorHandoffErrorV1;
#[cfg(target_os = "linux")]
pub use application_descriptor_handoff::WorkerV3ApplicationDescriptorHandoffErrorV1;
#[cfg(all(
    target_os = "linux",
    any(test, feature = "qualification-oracles-test-only")
))]
pub use application_descriptor_handoff::consume_inherited_worker_v2_application_handoff_v1;
#[cfg(target_os = "linux")]
#[doc(hidden)]
pub use application_descriptor_handoff::consume_inherited_worker_v3_application_handoff_v1;
pub use argument_alias::{
    AliasAdmissionError, AllocationIdentity, AllocationProvenance, ArgumentAccess,
    ArgumentAccessMode, ArgumentAliasAdmission, ArgumentAliasValidator, AtomicAccess,
    AtomicOperation, AtomicOrdering, AtomicScope, CheckedByteRegion, ConflictSource,
    InvalidAtomicOrdering, RegionError,
};
#[doc(hidden)]
pub use argument_alias::{
    GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice, GeneratedSliceArgumentPairV1,
    GeneratedWriteDeviceSlice,
};
pub use artifact_binding::{
    ARTIFACT_KERNEL_IDENTITY_VERSION, ArtifactBindingError, ArtifactKernelIdentityV1,
    ArtifactLaunchContractError, ArtifactRevalidationError, ValidatedArtifactSelectionV1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use artifact_binding::{
    AuthenticatedKernelArtifactV1, CompilerGeneratedKernelContractV1,
    GeneratedArtifactAuthenticationError, GeneratedKernelBindingV1,
};
#[doc(hidden)]
pub use artifact_binding::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    CompilerGeneratedSemanticWitnessErrorV1, GeneratedKernelProfileError,
    GeneratedMarkerBindingError, ValidatedCompilerGeneratedSemanticWitnessV1,
    semantic_witness_from_backend_v1, validate_compiler_generated_semantic_witness_v1,
};
pub use cooperative_launch::{
    CooperativeAdmissionError, CooperativeLaunchAdmission, CooperativeLaunchError,
    CooperativeResidencyAdmission,
};
pub use fe2o3_core::{KernelParams, LaunchConfig};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use fe2o3_hsaco_finalize::WorkgroupSyncProfileKindV1;
pub use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use flash_attention_v1_lifecycle::{
    CompletedFlashAttentionV1, FlashAttentionV1DispatchErrorV1, FlashAttentionV1JoinErrorV1,
    FlashAttentionV1KernelResourceObservationV1, FlashAttentionV1LoadErrorV1,
    FlashAttentionV1UnloadIdentityV1, JoinedFlashAttentionV1, LoadedFlashAttentionV1,
    ReviewedFlashAttentionV1RuntimeAdapterV1, UnloadedFlashAttentionV1, join_flash_attention_v1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_alpha_zeta_cov6::{
    AlphaZetaCov6DispatchIdentityV1, AlphaZetaCov6KernelRoleV1, AlphaZetaCov6ProfileError,
    CompilerGeneratedAlphaZetaCov6ArgumentsV1, GeneratedAlphaZetaCov6ArgumentBindingV1,
    GeneratedAlphaZetaCov6ArgumentError, GeneratedAlphaZetaCov6CompletionV1,
    GeneratedAlphaZetaCov6GeometryError, GeneratedAlphaZetaCov6PhysicalKernargError,
    GeneratedAlphaZetaCov6PrepareError, GeneratedAlphaZetaCov6PrepareResultV1,
    GeneratedAlphaZetaCov6PreparedInvocationV1,
};
#[doc(hidden)]
pub use generated_argument_plan::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentFieldProperty,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedPackingComponentKindV1,
    GeneratedPackingComponentV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_flash_attention_v1::{
    FlashAttentionV1BufferAccessV1, FlashAttentionV1BufferRoleV1,
    GeneratedFlashAttentionV1HostAdapterErrorV1, GeneratedFlashAttentionV1HostAdapterV1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_lds_gemm::{
    GeneratedLdsGemmSlice1HostAdapterErrorV1, GeneratedLdsGemmSlice1HostAdapterV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_lds_gemm_lifecycle::{
    CompletedExactLdsGemmSlice1V1, ExactLdsGemmKernelResourceObservationV1,
    ExactLdsGemmSlice1DispatchErrorV1, ExactLdsGemmSlice1JoinErrorV1,
    ExactLdsGemmSlice1LoadErrorV1, ExactLdsGemmUnloadIdentityV1, JoinedExactLdsGemmSlice1V1,
    LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1, UnloadedExactLdsGemmSlice1V1,
    join_exact_lds_gemm_slice1_v1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_moe_expert_v1::{
    GeneratedMoeExpertV1HostAdapterErrorV1, GeneratedMoeExpertV1HostAdapterV1,
    MoeExpertCompactCopyV1, MoeExpertCompactPackPlanErrorV1, MoeExpertCompactPackPlanV1,
    MoeExpertV1BufferAccessV1, MoeExpertV1BufferRoleV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_moe_expert_v2::{
    GeneratedMoeExpertV2HostAdapterErrorV2, GeneratedMoeExpertV2HostAdapterV2,
    MoeExpertV2BufferAccessV2, MoeExpertV2BufferRoleV2,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_moe_top2_v1::{
    GeneratedMoeTop2V1HostAdapterErrorV1, GeneratedMoeTop2V1HostAdapterV1, MoeTop2V1BufferAccessV1,
    MoeTop2V1BufferRoleV1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_row_softmax_v1::{
    GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1, GeneratedProtectedRowSoftmaxV1HostAdapterV1,
    ProtectedRowSoftmaxV1BufferRoleV1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_scalar_gemm_v1::{
    CompilerGeneratedScalarGemmV1Arguments, GeneratedScalarGemmV1ArgumentBinding,
    GeneratedScalarGemmV1Completion, GeneratedScalarGemmV1PrepareError,
    GeneratedScalarGemmV1PrepareResult, GeneratedScalarGemmV1PreparedInvocation,
    ScalarGemmV1ArgumentError, ScalarGemmV1DispatchIdentity, ScalarGemmV1GeometryError,
    ScalarGemmV1PhysicalKernargError, ScalarGemmV1ProfileError,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_scalar_gemm_v1::{
    GeneratedScalarGemmV1ReadDeviceSlice, GeneratedScalarGemmV1ReadWriteDeviceSlice,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_vecadd::{
    GeneratedVecAddKernelV1, GeneratedVecAddLoadError, GeneratedVecAddPrepareError,
    GeneratedVecAddPreparedV1, GeneratedVecAddProfileError,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_wave64_collectives_v1::{
    GeneratedWave64CollectivesV1HostAdapterErrorV1, GeneratedWave64CollectivesV1HostAdapterV1,
    Wave64CollectivesBufferRoleV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_wave64_collectives_v1_lifecycle::{
    CompletedWave64CollectivesV1, JoinedWave64CollectivesV1, LoadedWave64CollectivesV1,
    ReviewedWave64CollectivesRuntimeAdapterV1, UnloadedWave64CollectivesV1,
    Wave64CollectivesDispatchErrorV1, Wave64CollectivesJoinErrorV1,
    Wave64CollectivesKernelResourceObservationV1, Wave64CollectivesLifecycleIdentityV1,
    Wave64CollectivesLoadErrorV1, Wave64CollectivesUnloadIdentityV1, join_wave64_collectives_v1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_worker_v2_vecadd::{
    GeneratedWorkerV2VecAddBindError, GeneratedWorkerV2VecAddCompletionV1,
    GeneratedWorkerV2VecAddExecutorV1, GeneratedWorkerV2VecAddPrepareError,
    GeneratedWorkerV2VecAddPreparedV1,
};
#[doc(hidden)]
pub use generated_worker_v3_dispatch::{
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedWorkerV3ArgumentBindingV1,
    GeneratedWorkerV3ArgumentErrorV1, GeneratedWorkerV3PrepareErrorV1,
    GeneratedWorkerV3PreparedInvocationV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_workgroup_lds_reduction_v1::{
    GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1,
    GeneratedWorkgroupLdsReductionV1HostAdapterV1, WorkgroupLdsReductionBufferRoleV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_workgroup_scoped_atomic_v1::{
    GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1,
    GeneratedWorkgroupScopedAtomicV1HostAdapterV1, WorkgroupScopedAtomicBufferRoleV1,
    WorkgroupScopedAtomicEffectV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use generated_workgroup_sync_v1_lifecycle::{
    CompletedWorkgroupLdsReductionV1, CompletedWorkgroupScopedAtomicV1,
    JoinedWorkgroupLdsReductionV1, JoinedWorkgroupScopedAtomicV1, LoadedWorkgroupLdsReductionV1,
    LoadedWorkgroupScopedAtomicV1, ReviewedWorkgroupSyncRuntimeAdapterV1,
    UnloadedWorkgroupLdsReductionV1, UnloadedWorkgroupScopedAtomicV1, WorkgroupSyncDispatchErrorV1,
    WorkgroupSyncImplicitKernargObservationV1, WorkgroupSyncJoinErrorV1,
    WorkgroupSyncKernelResourceObservationV1, WorkgroupSyncLifecycleIdentityV1,
    WorkgroupSyncLoadErrorV1, WorkgroupSyncUnloadIdentityV1, join_workgroup_lds_reduction_v1,
    join_workgroup_scoped_atomic_v1,
};
pub use gfx942_ocml::{
    GFX942_OCML_SIN_F32_CODE_OBJECT_VERSION_V1, GFX942_OCML_SIN_F32_DEVICE_ABI_V1,
    GFX942_OCML_SIN_F32_IMPORT_SYMBOL_V1, GFX942_OCML_SIN_F32_KERNEL_ABI_V1,
    GFX942_OCML_SIN_F32_KERNEL_SYMBOL_V1, GFX942_OCML_SIN_F32_MAX_ELEMENTS_V1,
    GFX942_OCML_SIN_F32_MAX_HSACO_BYTES_V1, GFX942_OCML_SIN_F32_TARGET_V1,
    GFX942_OCML_SIN_F32_WORKGROUP_SIZE_V1, Gfx942OcmlArtifactIdentityV1, Gfx942OcmlSinErrorV1,
    Gfx942OcmlSinF32KernelV1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use hsa_executable_lifecycle::{
    AuthenticatedWorkerV2ExecutableV1, AuthorizedHsaLoadV1, HsaDispatchError,
    HsaExecutableLoadError, HsaGeneratedDispatchError, HsaKernelLaunchAuthorizationV1,
    HsaLoadAuthorizationError, InertLoadedWorkerV2KernelSelectionV1, LoadedHsaExecutableV1,
    WorkerV2ExecutableAuthenticationError, WorkerV2PrerequisiteAuthenticatorV1,
    WorkerV2PrerequisiteDecisionV1, WorkerV2PrerequisiteError, WorkerV2PrerequisiteRequestV1,
    WorkerV2RequiredProfileError, WorkerV2SafetyPropertiesV1, WorkerV2SafetyPropertyV1,
};
pub use hsa_executable_lifecycle::{
    AuthorizedWorkerV3HsaLoadV1, HsaAgentIdentityV1, HsaCodeObjectLoadObservationV1,
    HsaCompletedDispatchV1, HsaCompletedWorkerV3DispatchV1, HsaDispatchObservationV1,
    HsaEnvironmentMismatch, HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1,
    HsaExecutableUnloadError, HsaImplicitKernargInitializationObservationV1,
    HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchAuthorizationError,
    HsaLaunchGeometryV1, HsaObservationError, HsaPhysicalDeviceIdentityV1, HsaRuntimeIdentityV1,
    HsaUnloadObservationV1, LoadedWorkerV3HsaExecutableV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1, UnloadedHsaExecutableV1, WorkerV3GeneratedDispatchErrorV1,
    WorkerV3HsaExecutableLoadErrorV1, WorkerV3HsaLoadAuthorizationErrorV1,
};
#[doc(hidden)]
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use launch_kernel_v2_bridge::{
    CurrentRecoveredLaunchKernelMetadataV2, Gfx942DynamicLdsProjectionV2, Gfx942ImplicitAbiKindV2,
    Gfx942ImplicitAbiParameterV2, Gfx942OccupancyMetadataStatusV2,
    Gfx942PhysicalKernelSignatureIdentityV2, Gfx942PhysicalKernelSignatureV2,
    Gfx942PhysicalLaunchProjectionV2, Gfx942PhysicalResourceProjectionV2,
    LaunchKernelMetadataBridgeErrorV2, OccupancyDependentLaunchAdmissionErrorV2,
    bind_current_recovered_launch_kernel_metadata_v2,
};
#[doc(hidden)]
pub use loaded_kernel::{GeneratedAdmittedLaunch, LoadedKernelLoadError};
pub use loaded_kernel::{
    LoadedArgumentAdmittedLaunch, LoadedKernel, LoadedKernelMatchError, LoadedLaunchError,
    LoadedPreparedLaunch,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use moe_expert_v1_denial::{MoeExpertExecutionDeniedV1, deny_moe_expert_execution_v1};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use moe_routing_expert_bridge_v1::{
    CheckedMoeHostObservedRoutingOutputV1, MoeHostObservedRoutingExpertBridgeV1,
    MoeRoutingExpertBridgeUploadErrorV1, MoeRoutingOutputCandidateV1,
    MoeRoutingOutputConsistencyErrorV1, check_host_observed_moe_routing_output_v1,
    upload_checked_moe_routing_expert_bridge_v1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use moe_routing_expert_bridge_v2::{
    CheckedMoeCompletedRoutingExpertInputsV2, CheckedMoeCompletedRoutingReadbackV2,
    MoeCompletedRoutingExpertBridgeV2, MoeCompletedRoutingExpertUploadErrorV2,
    MoeCompletedRoutingExpertUploadRoleV2, MoeExpertInputCandidateV2, MoeExpertInputJoinErrorV2,
    MoeExpertWeightArtifactBindingV2, MoeRoutingCompletionReadbackErrorV2,
    MoeRoutingCompletionReadbackProvenanceV2, MoeRoutingExpertBatchIdentityV2,
    MoeRoutingOutputCandidateV2, bind_completed_moe_routing_expert_inputs_v2,
    check_completed_moe_routing_readback_v2, upload_completed_moe_routing_expert_bridge_v2,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use moe_top2_v1_lifecycle::{
    CompletedMoeTop2V1, JoinedMoeTop2V1, LoadedMoeTop2V1, MoeTop2V1DispatchErrorV1,
    MoeTop2V1JoinErrorV1, MoeTop2V1KernelResourceObservationV1, MoeTop2V1LoadErrorV1,
    MoeTop2V1UnloadIdentityV1, ReviewedMoeTop2V1RuntimeAdapterV1, UnloadedMoeTop2V1,
    join_moe_top2_v1,
};
pub use prepared_launch::{
    ArgumentAdmittedLaunch, CheckedDimensions, DeviceIdentity, KernelBrand, LaunchAxis,
    LaunchDimension, ObservedContext, PrepareLaunchError, PreparedGeometry, PreparedLaunch,
    PreparedResources, UntrustedKernelDeclaration, UntrustedLaunchRequest,
};
#[cfg(target_os = "linux")]
pub use production_application::{
    ProductionWorkerV3ApplicationLoadErrorV1, load_inherited_worker_v3_application_v1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use protected_row_softmax_v1_lifecycle::{
    CompletedProtectedRowSoftmaxV1, JoinedProtectedRowSoftmaxV1, LoadedProtectedRowSoftmaxV1,
    ProtectedRowSoftmaxV1DispatchErrorV1, ProtectedRowSoftmaxV1JoinErrorV1,
    ProtectedRowSoftmaxV1KernelResourceObservationV1, ProtectedRowSoftmaxV1LoadErrorV1,
    ProtectedRowSoftmaxV1UnloadIdentityV1, ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
    UnloadedProtectedRowSoftmaxV1, join_protected_row_softmax_v1,
};
pub use published_direct_link::{
    PublishedDirectLinkAdmissionError, ValidatedPublishedDirectLinkSelectionV1,
};
pub use published_hsaco_inspection::{
    AMDHSA_KERNEL_IDENTITY_RULE_V1, CurrentPendingPublishedDirectLinkLoadAdmissionV1,
    InspectedPublishedDirectLinkPhysicalLayoutV1, MissingPublishedDirectLinkLoadPrerequisiteV1,
    PendingPublishedDirectLinkLoadAdmissionV1, PhysicalMetadataValueV1,
    PublishedKernelPhysicalLayoutV1, PublishedLoadAdmissionError,
    PublishedPhysicalArgumentLayoutV1, PublishedPhysicalHiddenArgumentLayoutV1,
    PublishedPhysicalLaunchLayoutV1, PublishedPhysicalLayoutInspectionError,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use recovered_worker_v2_admission::{
    RecoveredWorkerV2AdmissionError, RecoveredWorkerV2PinnedDescriptorV1,
    RecoveredWorkerV2SynchronousHsaDispatchError, RecoveredWorkerV2SynchronousHsaHandoffError,
    RecoveredWorkerV2SynchronousHsaHandoffV1, RecoveredWorkerV2SynchronousHsaPrepareError,
    RecoveredWorkerV2SynchronousHsaPrepareResultV1,
    RecoveredWorkerV2SynchronousHsaPreparedInvocationV1,
    RecoveredWorkerV2SynchronousHsaScalarGemmV1PrepareError,
    RecoveredWorkerV2SynchronousHsaScalarGemmV1PrepareResultV1,
    RecoveredWorkerV2SynchronousHsaScalarGemmV1PreparedInvocationV1,
    RecoveredWorkerV2SynchronousHsaUnloadError,
};
pub use recovered_worker_v3_admission::{
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3PinnedDescriptorV1,
    WorkerV3HostLineageIdentityV1, admit_recovered_worker_v3_descriptor_v1,
};
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use row_softmax_protected_admission::{
    ProtectedRowSoftmaxV1HostAdmissionErrorV1, ProtectedRowSoftmaxV1HostTokenIdentityV1,
    ProtectedRowSoftmaxV1HostTokenV1, prepare_protected_row_softmax_v1_host_token_v1,
};

pub use tile_interop::{
    GFX942_XOR4_BF16_TILE_COLUMNS_V1, GFX942_XOR4_BF16_TILE_ELEMENTS_V1,
    GFX942_XOR4_BF16_TILE_ROWS_V1, GFX942_XOR4_BF16_TILE_WAVE_LANES_V1, Gfx942TileInteropErrorV1,
    Gfx942Xor4Bf16TileAllocationV1, Gfx942Xor4Bf16TileLeaseV1,
};

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use worker_v2_bundle_admission::{
    AdmittedFinalizedWorkerV2BundleV1, AdmittedWorkerV2TypedKernelV1,
    CurrentFinalizedWorkerV2BundleAdmissionV1, FinalizedWorkerV2BundleAdmissionError,
    MissingFinalizedWorkerV2LoadPrerequisiteV1,
    WORKER_V2_FULL_LINEAGE_PREREQUISITE_CHALLENGE_VERSION_V2,
    WorkerV2FullLineagePrerequisiteChallengeIdentityV2, WorkerV2TypedKernelSelectionError,
};
pub use worker_v3_verification_admission::{
    AuthenticatedWorkerV3ExecutableV1, WorkerV3AuditorV1, WorkerV3SafetyPropertiesV1,
    WorkerV3SafetyPropertyV1, WorkerV3VerificationAuditErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerificationChallengeIdentityV1,
    WorkerV3VerificationDecisionErrorV1, WorkerV3VerificationDecisionV1,
    WorkerV3VerificationRequestV1, WorkerV3VerifierV1, audit_recovered_worker_v3_verification_v1,
};

/// Compiler-generated host bindings. This is an unstable implementation SPI,
/// not an application extension point.
#[doc(hidden)]
pub mod __generated {
    #[cfg(target_os = "linux")]
    pub use crate::production_application::load_admitted_worker_v3_application_v1;

    #[cfg(any(test, feature = "qualification-oracles-test-only"))]
    pub use crate::{
        AlphaZetaCov6DispatchIdentityV1, AlphaZetaCov6KernelRoleV1, AlphaZetaCov6ProfileError,
        CompilerGeneratedAlphaZetaCov6ArgumentsV1, CompilerGeneratedScalarGemmV1Arguments,
        GeneratedAlphaZetaCov6ArgumentBindingV1, GeneratedAlphaZetaCov6ArgumentError,
        GeneratedAlphaZetaCov6CompletionV1, GeneratedAlphaZetaCov6GeometryError,
        GeneratedAlphaZetaCov6PhysicalKernargError, GeneratedAlphaZetaCov6PrepareError,
        GeneratedAlphaZetaCov6PrepareResultV1, GeneratedAlphaZetaCov6PreparedInvocationV1,
        GeneratedLdsGemmSlice1HostAdapterErrorV1, GeneratedLdsGemmSlice1HostAdapterV1,
        GeneratedScalarGemmV1ArgumentBinding, GeneratedScalarGemmV1Completion,
        GeneratedScalarGemmV1PrepareError, GeneratedScalarGemmV1PrepareResult,
        GeneratedScalarGemmV1PreparedInvocation, GeneratedScalarGemmV1ReadDeviceSlice,
        GeneratedScalarGemmV1ReadWriteDeviceSlice, GeneratedWorkerV2VecAddBindError,
        GeneratedWorkerV2VecAddCompletionV1, GeneratedWorkerV2VecAddExecutorV1,
        GeneratedWorkerV2VecAddPrepareError, GeneratedWorkerV2VecAddPreparedV1,
        ScalarGemmV1ArgumentError, ScalarGemmV1DispatchIdentity, ScalarGemmV1GeometryError,
        ScalarGemmV1PhysicalKernargError, ScalarGemmV1ProfileError,
    };
    #[cfg(any(test, feature = "qualification-oracles-test-only"))]
    pub use crate::{
        AuthenticatedKernelArtifactV1, CompilerGeneratedKernelContractV1,
        GeneratedArtifactAuthenticationError, GeneratedKernelBindingV1, GeneratedVecAddKernelV1,
        GeneratedVecAddLoadError, GeneratedVecAddPrepareError, GeneratedVecAddPreparedV1,
        GeneratedVecAddProfileError,
    };
    pub use crate::{
        CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
        CompilerGeneratedKernelProfileV1, CompilerGeneratedSemanticWitnessErrorV1,
        CompilerGeneratedWorkerV3ArgumentsV1, GeneratedAdmittedLaunch,
        GeneratedArgumentFieldProperty, GeneratedArgumentLayoutError, GeneratedArgumentPackError,
        GeneratedArgumentPackingError, GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1,
        GeneratedKernelProfileError, GeneratedMarkerBindingError, GeneratedPackingComponentKindV1,
        GeneratedPackingComponentV1, GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice,
        GeneratedSliceArgumentPairV1, GeneratedWorkerV3ArgumentBindingV1,
        GeneratedWorkerV3ArgumentErrorV1, GeneratedWorkerV3PrepareErrorV1,
        GeneratedWorkerV3PreparedInvocationV1, GeneratedWriteDeviceSlice, LoadedKernelLoadError,
        ValidatedCompilerGeneratedSemanticWitnessV1, semantic_witness_from_backend_v1,
        validate_compiler_generated_semantic_witness_v1,
    };
    pub use fe2o3_artifacts::{
        AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability, Name,
        PointerWidth, RustDisjointIndexSpaceV1, ScalarType,
    };

    /// Constructs the exact immutable slice promised by a generated backend
    /// accessor pair.
    ///
    /// # Safety
    ///
    /// `pointer` must be non-null, correctly aligned, and point to one live,
    /// immutable allocation containing exactly `length` initialized bytes.
    /// That allocation must remain live and immutable for the entire program.
    /// `length` must not exceed `isize::MAX`, and the range must not wrap the
    /// address space. Only compiler-generated unsafe trait implementations may
    /// call this function with values returned by the trusted backend object.
    #[cfg(any(test, feature = "qualification-oracles-test-only"))]
    pub unsafe fn artifact_bytes_from_backend_v1(
        pointer: *const u8,
        length: usize,
    ) -> &'static [u8] {
        if pointer.is_null()
            || length == 0
            || length > isize::MAX as usize
            || pointer.addr().checked_add(length).is_none()
        {
            return &[];
        }

        // SAFETY: the caller establishes the single-allocation, initialization,
        // immutability, range, and static-lifetime requirements above.
        unsafe { core::slice::from_raw_parts(pointer, length) }
    }
}

/// Loads and launches a GPU kernel using raw, caller-described ABI arguments.
///
/// # Safety
///
/// The caller must ensure that the named function's ABI exactly matches the
/// argument kinds, order, and Rust types supplied here. Every device pointer
/// must be valid for the kernel's accesses and remain alive until the stream
/// has completed the launch. The supplied module must remain loaded until that
/// completion; a temporary module expression does not satisfy this requirement.
/// Mutable arguments must satisfy the kernel's aliasing and synchronization
/// requirements, and the launch configuration must satisfy the kernel's grid,
/// block, and shared-memory requirements.
///
/// An unguarded launch does not compile:
///
/// ```compile_fail,E0133
/// use fe2o3_core::{GpuModule, LaunchConfig, Result, Stream};
/// use fe2o3_host::launch;
/// use std::sync::Arc;
///
/// fn unguarded(module: &Arc<GpuModule>, stream: &Stream) -> Result<()> {
///     launch! {
///         kernel: example,
///         stream: stream,
///         module: module,
///         config: LaunchConfig::for_num_elems(1),
///         args: []
///     }
/// }
/// ```
#[macro_export]
macro_rules! launch {
    (
        kernel: $kernel:ident,
        stream: $stream:expr,
        module: $module:expr,
        config: $config:expr,
        args: [$($kind:ident($value:expr)),* $(,)?]
    ) => {{
        let __fe2o3_function = ($module).load_function(stringify!($kernel))?;
        let mut __fe2o3_params = ::fe2o3_core::KernelParams::new();
        $(
            $crate::__push_kernel_arg!(__fe2o3_params, $kind($value));
        )*
        ::fe2o3_core::launch_kernel_on_stream(
            &__fe2o3_function,
            $config,
            &$stream,
            &mut __fe2o3_params,
        )
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __push_kernel_arg {
    ($params:ident, scalar($value:expr)) => {{
        $params.push($value);
    }};
    ($params:ident, raw($value:expr)) => {{
        $params.push($value);
    }};
    ($params:ident, buffer($value:expr)) => {{
        $params.push(($value).as_device_ptr());
    }};
    ($params:ident, slice($value:expr)) => {{
        $params.push(($value).as_device_ptr());
        $params.push(($value).len());
    }};
    ($params:ident, slice_mut($value:expr)) => {{
        $params.push(($value).as_device_ptr());
        $params.push(($value).len());
    }};
}

#[cfg(test)]
mod tests {
    use fe2o3_core::KernelParams;

    #[derive(Clone, Copy)]
    struct FakeBuffer {
        ptr: usize,
        len: usize,
    }

    impl FakeBuffer {
        fn as_device_ptr(&self) -> usize {
            self.ptr
        }

        fn len(&self) -> usize {
            self.len
        }
    }

    #[test]
    fn argument_kinds_preserve_abi_field_counts() {
        let buffer = FakeBuffer {
            ptr: 0x1000,
            len: 8,
        };
        let mut params = KernelParams::new();

        crate::__push_kernel_arg!(params, scalar(1.0_f32));
        crate::__push_kernel_arg!(params, raw(7_u32));
        crate::__push_kernel_arg!(params, buffer(buffer));
        crate::__push_kernel_arg!(params, slice(buffer));
        crate::__push_kernel_arg!(params, slice_mut(buffer));

        assert_eq!(params.len(), 7);
    }
}
