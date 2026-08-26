#[cfg(target_os = "linux")]
mod application_descriptor_handoff;
mod argument_alias;
mod artifact_binding;
mod generated_argument_plan;
mod generated_worker_v3_dispatch;
mod hsa_executable_lifecycle;
mod prepared_launch;
#[cfg(target_os = "linux")]
mod production_application;
mod published_direct_link;
mod published_hsaco_inspection;
mod recovered_worker_v3_admission;
#[cfg(any(test, feature = "hardware-test-hooks"))]
mod test_currentness_retry;
mod tile_interop;
mod worker_v3_verification_admission;

#[cfg(feature = "hardware-test-hooks")]
#[doc(hidden)]
pub mod __hardware_test {
    use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};

    use crate::{
        AllocationProvenance, ArgumentAccess, ArgumentAccessMode, GeneratedSliceArgumentPairV1,
        ObservedContext,
    };

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
#[doc(hidden)]
pub use application_descriptor_handoff::consume_inherited_worker_v3_application_handoff_v1;
#[cfg(target_os = "linux")]
pub use application_descriptor_handoff::{
    ApplicationDescriptorHandoffErrorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
};
pub use argument_alias::{
    AliasAdmissionError, AllocationIdentity, AllocationProvenance, ArgumentAccess,
    ArgumentAccessMode, ArgumentAliasAdmission, ArgumentAliasValidator, AtomicAccess,
    AtomicOperation, AtomicOrdering, AtomicScope, CheckedByteRegion, ConflictSource,
    InvalidAtomicOrdering, RegionError,
};
#[doc(hidden)]
pub use argument_alias::{
    GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice, GeneratedSliceArgumentPairV1,
};
pub use artifact_binding::{
    ARTIFACT_KERNEL_IDENTITY_VERSION, ArtifactBindingError, ArtifactKernelIdentityV1,
    ArtifactLaunchContractError, ArtifactRevalidationError, ValidatedArtifactSelectionV1,
};
#[doc(hidden)]
pub use artifact_binding::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    CompilerGeneratedSemanticWitnessErrorV1, ValidatedCompilerGeneratedSemanticWitnessV1,
    semantic_witness_from_backend_v1, validate_compiler_generated_semantic_witness_v1,
};
pub use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};
#[doc(hidden)]
pub use generated_argument_plan::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentFieldProperty,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedPackingComponentKindV1,
    GeneratedPackingComponentV1,
};
#[doc(hidden)]
pub use generated_worker_v3_dispatch::{
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedWorkerV3ArgumentBindingV1,
    GeneratedWorkerV3ArgumentErrorV1, GeneratedWorkerV3PrepareErrorV1,
    GeneratedWorkerV3PreparedInvocationV1,
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
pub use prepared_launch::{
    ArgumentAdmittedLaunch, CheckedDimensions, DeviceIdentity, KernelBrand, LaunchAxis,
    LaunchDimension, ObservedContext, PrepareLaunchError, PreparedGeometry, PreparedLaunch,
    PreparedResources, UntrustedKernelDeclaration, UntrustedLaunchRequest,
};
#[cfg(target_os = "linux")]
pub use production_application::{
    ProductionWorkerV3ApplicationLoadErrorV1, load_inherited_worker_v3_application_v1,
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
pub use recovered_worker_v3_admission::{
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3PinnedDescriptorV1,
    WorkerV3HostLineageIdentityV1, admit_recovered_worker_v3_descriptor_v1,
};
pub use tile_interop::{
    GFX942_XOR4_BF16_TILE_COLUMNS_V1, GFX942_XOR4_BF16_TILE_ELEMENTS_V1,
    GFX942_XOR4_BF16_TILE_ROWS_V1, GFX942_XOR4_BF16_TILE_WAVE_LANES_V1, Gfx942TileInteropErrorV1,
    Gfx942Xor4Bf16TileAllocationV1, Gfx942Xor4Bf16TileLeaseV1,
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

    pub use crate::{
        CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
        CompilerGeneratedKernelProfileV1, CompilerGeneratedSemanticWitnessErrorV1,
        CompilerGeneratedWorkerV3ArgumentsV1, GeneratedArgumentFieldProperty,
        GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
        GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedPackingComponentKindV1,
        GeneratedPackingComponentV1, GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice,
        GeneratedSliceArgumentPairV1, GeneratedWorkerV3ArgumentBindingV1,
        GeneratedWorkerV3ArgumentErrorV1, GeneratedWorkerV3PrepareErrorV1,
        GeneratedWorkerV3PreparedInvocationV1, ValidatedCompilerGeneratedSemanticWitnessV1,
        semantic_witness_from_backend_v1, validate_compiler_generated_semantic_witness_v1,
    };
    pub use fe2o3_artifacts::{
        AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability, Name,
        PointerWidth, RustDisjointIndexSpaceV1, ScalarType,
    };
}
