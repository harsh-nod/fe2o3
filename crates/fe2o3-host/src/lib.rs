#[cfg(target_os = "linux")]
mod application_descriptor_handoff;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod argument_alias;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod artifact_binding;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod authenticated_service_queue;
#[cfg(target_os = "linux")]
mod compiler_execution_current_record_audit;
mod compiler_generated_contract;
mod generated_argument_borrow;
mod generated_argument_plan;
mod generated_kfd_arguments;
mod generated_kfd_invocation;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod generated_worker_v3_dispatch;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod hsa_executable_lifecycle;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod prepared_launch;
#[cfg(target_os = "linux")]
mod production_application;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod published_direct_link;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod published_hsaco_inspection;
mod recovered_worker_v3_admission;
#[cfg(all(
    feature = "qualification-legacy-hip-hsa",
    any(test, feature = "hardware-test-hooks")
))]
mod test_currentness_retry;
#[cfg(feature = "qualification-legacy-hip-hsa")]
mod tile_interop;
mod worker_v3_verification_admission;

#[cfg(feature = "hardware-test-hooks")]
#[doc(hidden)]
pub mod __hardware_test {
    use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};

    use crate::{
        AllocationProvenance, ArgumentAccess, ArgumentAccessMode,
        CompilerGeneratedKernelExpectationRosterEntryV1,
        CompilerGeneratedKernelExpectationRosterV1, GeneratedSliceArgumentPairV1, ObservedContext,
    };

    /// Inert one-entry roster for Cargo's strict inherited-handoff integration fixture.
    pub struct ApplicationHandoffVecAddRosterFixtureV1;

    impl CompilerGeneratedKernelExpectationRosterV1 for ApplicationHandoffVecAddRosterFixtureV1 {
        const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] =
            &[CompilerGeneratedKernelExpectationRosterEntryV1::from_parts(
                "vecadd", "vecadd", [0xa1; 32], [0xb2; 32],
            )];
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

#[cfg(target_os = "linux")]
pub use application_descriptor_handoff::{
    ApplicationDescriptorHandoffErrorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
};
#[cfg(target_os = "linux")]
#[doc(hidden)]
pub use application_descriptor_handoff::{
    consume_inherited_worker_v3_application_handoff_v1,
    consume_inherited_worker_v3_application_roster_handoff_v1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
#[doc(hidden)]
pub use argument_alias::GeneratedSliceArgumentPairV1;
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use argument_alias::{
    AliasAdmissionError, AllocationIdentity, AllocationProvenance, ArgumentAccess,
    ArgumentAccessMode, ArgumentAliasAdmission, ArgumentAliasValidator, AtomicAccess,
    AtomicOperation, AtomicOrdering, AtomicScope, CheckedByteRegion, ConflictSource,
    InvalidAtomicOrdering, RegionError,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
#[doc(hidden)]
pub use argument_alias::{
    GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice, GeneratedWriteDeviceSlice,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use artifact_binding::{
    ARTIFACT_KERNEL_IDENTITY_VERSION, ArtifactBindingError, ArtifactKernelIdentityV1,
    ArtifactLaunchContractError, ArtifactRevalidationError, ValidatedArtifactSelectionV1,
};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use authenticated_service_queue::{
    AuthenticatedQuarantinedServiceQueueV1, AuthenticatedServiceCompletedQueueSessionV1,
    AuthenticatedServiceCurrentnessFailureV1, AuthenticatedServicePublishedQueueSessionV1,
    AuthenticatedServiceQueueBindFailureV1, AuthenticatedServiceQueueCreateFailureV1,
    AuthenticatedServiceQueueDataUpdateFailureV1, AuthenticatedServiceQueueHostDataUpdateV1,
    AuthenticatedServiceQueueOperationFailureV1, AuthenticatedServiceQueuePartitionedDataUpdateV1,
    AuthenticatedServiceQueuePollV1, AuthenticatedServiceQueuePollWithProgressV1,
    AuthenticatedServiceQueueReleaseFailureV1, AuthenticatedServiceQueueReleaseV1,
    AuthenticatedServiceQueueRetainedBindFailureV1,
    AuthenticatedServiceQueueRetainedRolloverFailureV1, AuthenticatedServiceQueueRolloverFailureV1,
    AuthenticatedServiceQueueRolloverSuccessV1, AuthenticatedServiceQueueSessionV1,
    AuthenticatedServiceQueueSubmitFailureV1, AuthenticatedServiceQueueUnboundSessionV1,
    AuthenticatedServiceRecycledQueueSessionV1, AuthenticatedServiceTerminalProgramCustodyV1,
    AuthenticatedWorkerV3ProgramLookupErrorV1, AuthenticatedWorkerV3ProgramMaterializationErrorV1,
    AuthenticatedWorkerV3ProgramSetAdmissionErrorV1,
    AuthenticatedWorkerV3ProgramSetAppendFailureV1,
    AuthenticatedWorkerV3ProgramSetInitialFailureV1, AuthenticatedWorkerV3ProgramSetV1,
};
#[cfg(target_os = "linux")]
pub use compiler_execution_current_record_audit::{
    InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    WORKER_V3_COMPILER_CURRENT_RECORD_AUDIT_TIMEOUT_V1, WorkerV3CompilerCurrentRecordAuditErrorV1,
    WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordEvidenceViewV1,
};
#[doc(hidden)]
pub use compiler_generated_contract::{
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    CompilerGeneratedSemanticWitnessErrorV1, ValidatedCompilerGeneratedSemanticWitnessV1,
    semantic_witness_from_backend_v1, validate_compiler_generated_semantic_witness_v1,
};
pub use fe2o3_aql::{AqlDispatchGeometryV1, AqlGeometryError};
#[cfg(target_os = "linux")]
pub use fe2o3_compiler_execution_client::CompilerExecutionCurrentRecordChallengeV1;
pub use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};
pub use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, DeviceBindingError, DeviceSelector, KfdAdapterError,
    KfdWithAdmittedUapi, OpenedKfd,
};
#[doc(hidden)]
pub use generated_argument_plan::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentFieldProperty,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedPackingComponentKindV1,
    GeneratedPackingComponentV1,
};
#[doc(hidden)]
pub use generated_kfd_arguments::{
    CompilerGeneratedKfdArguments, GeneratedKfdArgumentBinding, GeneratedKfdArgumentError,
    GeneratedKfdCompletion, GeneratedKfdCompletionError, GeneratedKfdPackedArguments,
    GeneratedKfdPrepareError, GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice,
    GeneratedKfdSliceBinding, GeneratedKfdWriteSlice,
};
pub use generated_kfd_invocation::{
    GeneratedWorkerV3KfdExecutionError, GeneratedWorkerV3KfdInvocation,
    GeneratedWorkerV3KfdInvocationError,
};
#[doc(hidden)]
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use generated_worker_v3_dispatch::{
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedWorkerV3ArgumentBindingV1,
    GeneratedWorkerV3ArgumentErrorV1, GeneratedWorkerV3PrepareErrorV1,
    GeneratedWorkerV3PreparedInvocationV1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
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
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use prepared_launch::{
    ArgumentAdmittedLaunch, CheckedDimensions, DeviceIdentity, KernelBrand, LaunchAxis,
    LaunchDimension, ObservedContext, PrepareLaunchError, PreparedGeometry, PreparedLaunch,
    PreparedResources, UntrustedKernelDeclaration, UntrustedLaunchRequest,
};
#[cfg(all(target_os = "linux", feature = "qualification-legacy-hip-hsa"))]
pub use production_application::{
    ProductionWorkerV3ApplicationLoadErrorV1, load_inherited_worker_v3_application_v1,
};
#[cfg(target_os = "linux")]
pub use production_application::{
    ProductionWorkerV3KfdApplicationErrorV1, ProductionWorkerV3KfdPreparationErrorV1,
    prepare_inherited_worker_v3_kfd_application_v1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use published_direct_link::{
    PublishedDirectLinkAdmissionError, ValidatedPublishedDirectLinkSelectionV1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use published_hsaco_inspection::{
    AMDHSA_KERNEL_IDENTITY_RULE_V1, CurrentPendingPublishedDirectLinkLoadAdmissionV1,
    InspectedPublishedDirectLinkPhysicalLayoutV1, MissingPublishedDirectLinkLoadPrerequisiteV1,
    PendingPublishedDirectLinkLoadAdmissionV1, PhysicalMetadataValueV1,
    PublishedKernelPhysicalLayoutV1, PublishedLoadAdmissionError,
    PublishedPhysicalArgumentLayoutV1, PublishedPhysicalHiddenArgumentLayoutV1,
    PublishedPhysicalLaunchLayoutV1, PublishedPhysicalLayoutInspectionError,
};
pub use recovered_worker_v3_admission::{
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3EntrypointV1,
    RecoveredWorkerV3PinnedDescriptorV1, RecoveredWorkerV3PinnedRosterV1,
    WorkerV3HostLineageIdentityV1, admit_recovered_worker_v3_descriptor_v1,
    admit_recovered_worker_v3_roster_v1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub use tile_interop::{
    GFX942_XOR4_BF16_TILE_COLUMNS_V1, GFX942_XOR4_BF16_TILE_ELEMENTS_V1,
    GFX942_XOR4_BF16_TILE_ROWS_V1, GFX942_XOR4_BF16_TILE_WAVE_LANES_V1, Gfx942TileInteropErrorV1,
    Gfx942Xor4Bf16TileAllocationV1, Gfx942Xor4Bf16TileLeaseV1,
};

pub use worker_v3_verification_admission::{
    AuthenticatedWorkerV3ExecutableV1, AuthenticatedWorkerV3RosterEntryV1,
    AuthenticatedWorkerV3RosterV1, WorkerV3AuditorV1, WorkerV3CompilerExecutionEvidenceErrorV1,
    WorkerV3CompilerExecutionVerificationV1, WorkerV3ProtectedRosterEntryEvidenceV1,
    WorkerV3ProtectedRosterVerificationEvidenceV1, WorkerV3ProtectedRosterVerifierAdapterV1,
    WorkerV3ProtectedRosterVerifierBackendV1, WorkerV3ProtectedVerificationEvidenceV1,
    WorkerV3ProtectedVerifierAdapterV1, WorkerV3ProtectedVerifierBackendV1,
    WorkerV3RosterEntryErrorV1, WorkerV3RosterLoadEnvelopeEvidenceViewV1,
    WorkerV3RosterVerificationAuthenticationErrorV1,
    WorkerV3RosterVerificationAuthenticationFailureV1,
    WorkerV3RosterVerificationChallengeIdentityV1, WorkerV3RosterVerificationDecisionErrorV1,
    WorkerV3RosterVerificationDecisionV1, WorkerV3RosterVerificationRequestV1,
    WorkerV3SafetyPropertiesV1, WorkerV3SafetyPropertyV1, WorkerV3VerificationAuditErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerificationChallengeIdentityV1,
    WorkerV3VerificationDecisionErrorV1, WorkerV3VerificationDecisionV1,
    WorkerV3VerificationRequestV1, WorkerV3VerificationRosterIdentityV1, WorkerV3VerifierV1,
    audit_recovered_worker_v3_verification_v1,
};
#[cfg(feature = "worker-v3-verifier-test-support")]
#[doc(hidden)]
pub use worker_v3_verification_admission::{
    WorkerV3SyntheticVerifierAdapterV1, WorkerV3SyntheticVerifierV1,
};

/// Compiler-generated host bindings. This is an unstable implementation SPI,
/// not an application extension point.
#[doc(hidden)]
pub mod __generated {
    #[cfg(all(target_os = "linux", feature = "qualification-legacy-hip-hsa"))]
    pub use crate::production_application::load_admitted_worker_v3_application_v1;
    #[cfg(target_os = "linux")]
    pub use crate::production_application::prepare_admitted_worker_v3_kfd_application_v1;

    pub use crate::{
        CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationRosterEntryV1,
        CompilerGeneratedKernelExpectationRosterV1, CompilerGeneratedKernelExpectationV1,
        CompilerGeneratedKernelProfileV1, CompilerGeneratedKfdArguments,
        CompilerGeneratedSemanticWitnessErrorV1, GeneratedArgumentFieldProperty,
        GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
        GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedKfdArgumentBinding,
        GeneratedKfdArgumentError, GeneratedKfdCompletion, GeneratedKfdCompletionError,
        GeneratedKfdPackedArguments, GeneratedKfdPrepareError, GeneratedKfdReadSlice,
        GeneratedKfdReadWriteSlice, GeneratedKfdSliceBinding, GeneratedKfdWriteSlice,
        GeneratedPackingComponentKindV1, GeneratedPackingComponentV1,
        GeneratedWorkerV3KfdExecutionError, GeneratedWorkerV3KfdInvocation,
        GeneratedWorkerV3KfdInvocationError, ValidatedCompilerGeneratedSemanticWitnessV1,
        semantic_witness_from_backend_v1, validate_compiler_generated_semantic_witness_v1,
    };
    #[cfg(feature = "qualification-legacy-hip-hsa")]
    pub use crate::{
        CompilerGeneratedWorkerV3ArgumentsV1, GeneratedReadDeviceSlice,
        GeneratedReadWriteDeviceSlice, GeneratedSliceArgumentPairV1,
        GeneratedWorkerV3ArgumentBindingV1, GeneratedWorkerV3ArgumentErrorV1,
        GeneratedWorkerV3PrepareErrorV1, GeneratedWorkerV3PreparedInvocationV1,
        GeneratedWriteDeviceSlice,
    };
    pub use fe2o3_artifacts::{
        AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability, Name,
        PointerWidth, RustDisjointIndexSpaceV1, ScalarType,
    };
}
