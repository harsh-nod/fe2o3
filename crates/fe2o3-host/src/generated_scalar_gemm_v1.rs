use crate::argument_alias::GeneratedDeviceSliceMetadata;
use crate::generated_argument_plan::{GeneratedArgumentInputV1, GeneratedPackingComponentKindV1};
use crate::{
    ArgumentAccess, ArgumentAccessMode, GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1,
    GeneratedSliceArgumentPairV1, ObservedContext, RegionError,
};
use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};
use fe2o3_core::{
    DeviceBuffer, DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut, DevicePtr,
};
use std::marker::PhantomData;

/// Generated shared `f32` device-slice capability for Scalar GEMM V1.
///
/// Unlike the general generated slice capability, this exact profile permits
/// an empty allocation because `k == 0` and no-dispatch shapes are valid. The
/// capability retains the original allocation borrow and exposes no pointer.
#[doc(hidden)]
pub struct GeneratedScalarGemmV1ReadDeviceSlice<'allocation> {
    pointer: DevicePtr<f32>,
    len: usize,
    metadata: GeneratedDeviceSliceMetadata,
    retained: PhantomData<&'allocation f32>,
}

impl<'allocation> GeneratedScalarGemmV1ReadDeviceSlice<'allocation> {
    pub fn new(
        observed: &ObservedContext,
        buffer: &'allocation DeviceBuffer<f32>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region_allow_empty(observed, buffer)?;
        Ok(Self {
            pointer: buffer.region_device_ptr(),
            len: buffer.region_len(),
            metadata,
            retained: PhantomData,
        })
    }

    /// Consumes one checked shared subregion as the exact Scalar GEMM slice.
    pub fn from_view(
        observed: &ObservedContext,
        view: DeviceBufferView<'allocation, f32>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region_allow_empty(observed, &view)?;
        Ok(Self {
            pointer: view.region_device_ptr(),
            len: view.region_len(),
            metadata,
            retained: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[doc(hidden)]
    pub fn bind_input_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedArgumentInputV1<'allocation>, GeneratedArgumentPackError> {
        // SAFETY: this compiler-only wrapper retains the exact DeviceBuffer
        // borrow and provenance corresponding to this pointer and length.
        let len = u64::try_from(self.len).map_err(|_| {
            GeneratedArgumentPackError::IntegerWidthOverflow {
                argument_index,
                component: GeneratedPackingComponentKindV1::SliceLength,
                value: u64::MAX,
                pointer_width: PointerWidth::Bits64,
            }
        })?;
        unsafe {
            plan.slice(
                argument_index,
                self.pointer.as_raw().cast_const().cast(),
                len,
                PointerWidth::Bits64,
                AddressSpace::Global,
                Access::ReadOnly,
            )
        }
    }

    #[doc(hidden)]
    pub fn argument_access_v1(&self) -> ArgumentAccess<'allocation> {
        ArgumentAccess::new(
            self.metadata.checked_region(),
            ArgumentAccessMode::SharedRead,
        )
    }

    #[doc(hidden)]
    pub fn bind_argument_pair(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError> {
        Ok(GeneratedSliceArgumentPairV1::new(
            self.bind_input_v1(plan, argument_index)?,
            self.argument_access_v1(),
        ))
    }
}

/// Generated exclusive initialized `f32` device-slice capability for Scalar
/// GEMM V1, including its valid empty-output state.
#[doc(hidden)]
pub struct GeneratedScalarGemmV1ReadWriteDeviceSlice<'allocation> {
    pointer: DevicePtr<f32>,
    len: usize,
    metadata: GeneratedDeviceSliceMetadata,
    exclusive: PhantomData<&'allocation mut f32>,
}

impl<'allocation> GeneratedScalarGemmV1ReadWriteDeviceSlice<'allocation> {
    pub fn new(
        observed: &ObservedContext,
        buffer: &'allocation mut DeviceBuffer<f32>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region_allow_empty(observed, buffer)?;
        Ok(Self {
            pointer: buffer.region_device_ptr(),
            len: buffer.region_len(),
            metadata,
            exclusive: PhantomData,
        })
    }

    /// Consumes one checked exclusive subregion as the exact initialized
    /// Scalar GEMM output slice.
    pub fn from_view_mut(
        observed: &ObservedContext,
        view: DeviceBufferViewMut<'allocation, f32>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region_allow_empty(observed, &view)?;
        Ok(Self {
            pointer: view.region_device_ptr(),
            len: view.region_len(),
            metadata,
            exclusive: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[doc(hidden)]
    pub fn bind_input_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedArgumentInputV1<'allocation>, GeneratedArgumentPackError> {
        // SAFETY: this compiler-only wrapper retains the exact exclusive
        // DeviceBuffer borrow and provenance through synchronous completion.
        let len = u64::try_from(self.len).map_err(|_| {
            GeneratedArgumentPackError::IntegerWidthOverflow {
                argument_index,
                component: GeneratedPackingComponentKindV1::SliceLength,
                value: u64::MAX,
                pointer_width: PointerWidth::Bits64,
            }
        })?;
        unsafe {
            plan.slice(
                argument_index,
                self.pointer.as_raw().cast_const().cast(),
                len,
                PointerWidth::Bits64,
                AddressSpace::Global,
                Access::ReadWrite,
            )
        }
    }

    #[doc(hidden)]
    pub fn argument_access_v1(&self) -> ArgumentAccess<'allocation> {
        ArgumentAccess::new(
            self.metadata.checked_region(),
            ArgumentAccessMode::ExclusiveReadWrite,
        )
    }

    #[doc(hidden)]
    pub fn bind_argument_pair(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError> {
        Ok(GeneratedSliceArgumentPairV1::new(
            self.bind_input_v1(plan, argument_index)?,
            self.argument_access_v1(),
        ))
    }
}

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
mod worker_v2 {
    use super::*;
    use crate::argument_alias::{InFlightRegionRegistration, admit_and_register};
    use crate::generated_argument_plan::{GeneratedDeviceScalarV1, GeneratedPackedArgumentsV1};
    use crate::hsa_executable_lifecycle::{
        AuthenticatedLoadedWorkerV2KernelSelectionV1, HsaCompletedDispatchV1,
        HsaCompletedSelectedWorkerV2DispatchV1, HsaExecutableLoadError, HsaGeneratedDispatchError,
        HsaLaunchAuthorizationError, LoadedHsaExecutableV1,
        ResolvedLoadedWorkerV2KernelSelectionV1, WorkerV2ExecutableAuthenticationError,
        WorkerV2PrerequisiteAuthenticatorV1, validate_launch_geometry_contract,
    };
    use crate::worker_v2_bundle_admission::WorkerV2TypedKernelSelectionError;
    use crate::{
        AliasAdmissionError, ArgumentAliasAdmission, ArtifactKernelIdentityV1,
        CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
        CompilerGeneratedKernelProfileV1, GeneratedArgumentLayoutError,
        GeneratedArgumentPackingError, HsaLaunchGeometryV1, PhysicalMetadataValueV1,
        ReviewedHsaExecutableLifecycleAdapterV1, ReviewedHsaImplicitKernargAdapterV1,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AliasClass, ArgumentOwnership, BlockSize, Mutability, ScalarType,
        TargetIdentity,
    };
    #[cfg(any(test, feature = "hardware-test-hooks"))]
    use fe2o3_artifacts::{AbiLayout, Dimensions, LaunchContract, Name};
    use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
    use std::fmt;
    use std::ptr::NonNull;

    const SCALAR_GEMM_V1_NAME: &str = "scalar_gemm_v1";
    const SCALAR_GEMM_V1_TARGET: &str = "gfx942:xnack-";
    const SCALAR_GEMM_V1_BLOCK_X: u32 = 256;
    const SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES: usize = 64;
    const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
    const SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES: usize = 320;
    const HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;

    /// Exact generated source/contract identity for Scalar GEMM V1.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[doc(hidden)]
    pub struct ScalarGemmV1DispatchIdentity {
        kernel_binding_id: [u8; 32],
        generated_host_contract_identity: [u8; 32],
    }

    impl ScalarGemmV1DispatchIdentity {
        pub const fn new(
            kernel_binding_id: [u8; 32],
            generated_host_contract_identity: [u8; 32],
        ) -> Self {
            Self {
                kernel_binding_id,
                generated_host_contract_identity,
            }
        }
    }

    /// Opaque binding of all six exact Scalar GEMM V1 arguments.
    #[doc(hidden)]
    pub struct GeneratedScalarGemmV1ArgumentBinding<'allocation> {
        inputs: Vec<GeneratedArgumentInputV1<'allocation>>,
        accesses: Vec<ArgumentAccess<'allocation>>,
        dimensions: [u32; 3],
    }

    impl<'allocation> GeneratedScalarGemmV1ArgumentBinding<'allocation> {
        /// # Safety
        ///
        /// `inputs` must contain exactly `a`, `b`, `c`, `m`, `n`, and `k` once in
        /// their source positions. `accesses` must come from those same retained
        /// slice capabilities in `a`, `b`, `c` order. `dimensions` must repeat the
        /// exact scalar values bound at positions 3, 4, and 5.
        #[doc(hidden)]
        pub unsafe fn from_compiler_generated_parts_v1(
            inputs: Vec<GeneratedArgumentInputV1<'allocation>>,
            accesses: Vec<ArgumentAccess<'allocation>>,
            dimensions: [u32; 3],
        ) -> Self {
            Self {
                inputs,
                accesses,
                dimensions,
            }
        }
    }

    /// Compiler-generated bridge for the exact Scalar GEMM V1 signature.
    ///
    /// # Safety
    ///
    /// An implementation must be emitted from the same independently collected
    /// Rust source and signature as `K`'s semantic witness. It must bind all six
    /// arguments without substitution and retain all referenced allocations until
    /// synchronous completion. A false implementation can authorize native GPU
    /// memory access.
    #[doc(hidden)]
    pub unsafe trait CompilerGeneratedScalarGemmV1Arguments<
        'allocation,
        K: CompilerGeneratedKernelExpectationV1,
    >
    {
        fn dispatch_identity_v1() -> ScalarGemmV1DispatchIdentity;

        fn generated_argument_layout_v1()
        -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

        fn bind_arguments_v1(
            &self,
            plan: &GeneratedArgumentPackingPlanV1,
        ) -> Result<GeneratedScalarGemmV1ArgumentBinding<'allocation>, GeneratedArgumentPackError>;
    }

    /// Linear prepared invocation for exactly one Scalar GEMM V1 operation.
    #[must_use = "a prepared Scalar GEMM V1 invocation does no work until dispatched"]
    #[doc(hidden)]
    pub struct GeneratedScalarGemmV1PreparedInvocation<
        'loaded,
        'allocation,
        P,
        K,
        A: ReviewedHsaImplicitKernargAdapterV1,
        Arguments,
    > {
        resolved: ResolvedLoadedWorkerV2KernelSelectionV1<'loaded, P, K, A>,
        geometry: Option<HsaLaunchGeometryV1>,
        kernarg: ScalarGemmAlignedKernarg,
        arguments: Arguments,
        admission: Option<ArgumentAliasAdmission<'allocation>>,
        registration: Option<InFlightRegionRegistration<'allocation>>,
    }

    #[doc(hidden)]
    pub type GeneratedScalarGemmV1PrepareResult<
        'loaded,
        'allocation,
        P,
        K,
        A,
        Arguments,
        PrerequisiteError,
    > = Result<
        GeneratedScalarGemmV1PreparedInvocation<'loaded, 'allocation, P, K, A, Arguments>,
        GeneratedScalarGemmV1PrepareError<
            PrerequisiteError,
            <A as ReviewedHsaExecutableLifecycleAdapterV1>::Error,
        >,
    >;

    impl<P, K, A, Arguments> GeneratedScalarGemmV1PreparedInvocation<'_, '_, P, K, A, Arguments>
    where
        A: ReviewedHsaImplicitKernargAdapterV1,
    {
        pub const fn geometry(&self) -> Option<HsaLaunchGeometryV1> {
            self.geometry
        }

        pub const fn explicit_byte_len(&self) -> usize {
            SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES
        }

        pub fn physical_kernarg_byte_len(&self) -> usize {
            self.kernarg.len()
        }

        pub fn physical_kernarg_alignment(&self) -> usize {
            self.kernarg.alignment()
        }

        /// Dispatches once, or returns an authenticated no-dispatch completion for
        /// a zero output extent. Consuming `self` makes a second safe launch
        /// impossible and retains every capability until the operation is done.
        pub fn dispatch(
            self,
        ) -> Result<GeneratedScalarGemmV1Completion<K>, HsaGeneratedDispatchError<A::Error>>
        {
            let Self {
                resolved,
                geometry,
                mut kernarg,
                arguments,
                admission,
                registration,
            } = self;
            let retained = (&arguments, &admission, &registration);
            let completion = if let Some(geometry) = geometry {
                // SAFETY: preparation authenticated the selected profile, packed
                // exact retained arguments, admitted aliases, and allocated the
                // complete aligned COV6 kernarg. The reviewed adapter is
                // synchronous and initializes only the canonical implicit suffix.
                let completed = unsafe {
                    resolved.dispatch_generated_and_wait(
                        geometry,
                        kernarg.bytes_mut(),
                        SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES,
                        SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES,
                        COV6_IMPLICIT_KERNARG_BYTES,
                    )
                }?;
                GeneratedScalarGemmV1Completion {
                    state: ScalarGemmCompletionState::Dispatched(completed),
                }
            } else {
                let artifact = resolved.artifact_identity().clone();
                GeneratedScalarGemmV1Completion {
                    state: ScalarGemmCompletionState::NoDispatch {
                        artifact,
                        marker: PhantomData,
                    },
                }
            };
            let _ = retained;
            Ok(completion)
        }
    }

    // Keep dispatch completion inline so returning from a completed GPU dispatch
    // does not introduce a new fallible allocation boundary.
    #[allow(clippy::large_enum_variant)]
    enum ScalarGemmCompletionState<K> {
        NoDispatch {
            artifact: ArtifactKernelIdentityV1,
            marker: PhantomData<fn() -> K>,
        },
        Dispatched(HsaCompletedSelectedWorkerV2DispatchV1<K>),
    }

    /// Completion of one exact Scalar GEMM V1 operation.
    #[doc(hidden)]
    pub struct GeneratedScalarGemmV1Completion<K> {
        state: ScalarGemmCompletionState<K>,
    }

    impl<K> GeneratedScalarGemmV1Completion<K> {
        pub const fn was_dispatched(&self) -> bool {
            matches!(self.state, ScalarGemmCompletionState::Dispatched(_))
        }

        pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
            match &self.state {
                ScalarGemmCompletionState::NoDispatch { artifact, .. } => artifact,
                ScalarGemmCompletionState::Dispatched(completed) => completed.artifact_identity(),
            }
        }

        pub const fn completed_dispatch(&self) -> Option<&HsaCompletedDispatchV1> {
            match &self.state {
                ScalarGemmCompletionState::NoDispatch { .. } => None,
                ScalarGemmCompletionState::Dispatched(completed) => {
                    Some(completed.completed_dispatch())
                }
            }
        }
    }

    impl<P, A> LoadedHsaExecutableV1<P, A>
    where
        A: ReviewedHsaImplicitKernargAdapterV1,
    {
        /// Authenticates and prepares exactly one generated Scalar GEMM V1 call.
        #[doc(hidden)]
        pub fn prepare_generated_scalar_gemm_v1<'loaded, 'allocation, K, Authenticator, Arguments>(
            &'loaded mut self,
            observed: &ObservedContext,
            authenticator: &mut Authenticator,
            arguments: Arguments,
        ) -> GeneratedScalarGemmV1PrepareResult<
            'loaded,
            'allocation,
            P,
            K,
            A,
            Arguments,
            Authenticator::Error,
        >
        where
            K: CompilerGeneratedKernelExpectationV1,
            Authenticator: WorkerV2PrerequisiteAuthenticatorV1<K>,
            Arguments: CompilerGeneratedScalarGemmV1Arguments<'allocation, K>,
        {
            if !matches!(
                K::PROFILE,
                CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 { .. }
            ) {
                return Err(GeneratedScalarGemmV1PrepareError::UnsupportedProfile);
            }
            validate_context_device(self, observed)
                .map_err(|()| GeneratedScalarGemmV1PrepareError::ContextDeviceMismatch)?;

            let selected_identity = self
                .select_typed_kernel::<K>()
                .map_err(GeneratedScalarGemmV1PrepareError::Selection)?
                .artifact_identity()
                .clone();
            let authenticated: AuthenticatedLoadedWorkerV2KernelSelectionV1<K> = self
                .authenticate_typed_kernel_once::<K, _>(&selected_identity, authenticator)
                .map_err(GeneratedScalarGemmV1PrepareError::Authentication)?;
            let resolved = authenticated
                .resolve(self)
                .map_err(GeneratedScalarGemmV1PrepareError::Resolution)?;

            validate_profile::<P, K, A>(&resolved, Arguments::dispatch_identity_v1())
                .map_err(GeneratedScalarGemmV1PrepareError::Profile)?;
            let generated = Arguments::generated_argument_layout_v1()
                .map_err(GeneratedScalarGemmV1PrepareError::GeneratedLayout)?;
            // SAFETY: only the unsafe compiler-generated implementation can supply
            // this independently generated exact layout for K.
            let plan = unsafe { resolved.validate_argument_packing(&generated) }
                .map_err(GeneratedScalarGemmV1PrepareError::PackingPlan)?;
            let binding = arguments
                .bind_arguments_v1(&plan)
                .map_err(GeneratedScalarGemmV1PrepareError::Bind)?;
            let (packed, admission, registration, output_extent) =
                validate_pack_and_admit(&plan, binding, observed)
                    .map_err(GeneratedScalarGemmV1PrepareError::Arguments)?;
            let geometry = scalar_gemm_geometry(
                resolved.artifact_identity(),
                resolved.physical_kernel().launch(),
                output_extent,
            )
            .map_err(GeneratedScalarGemmV1PrepareError::Geometry)?;
            let kernarg = prepare_physical_kernarg(&resolved, &plan, &packed)
                .map_err(GeneratedScalarGemmV1PrepareError::PhysicalKernarg)?;

            Ok(GeneratedScalarGemmV1PreparedInvocation {
                resolved,
                geometry,
                kernarg,
                arguments,
                admission,
                registration,
            })
        }
    }

    fn validate_context_device<P, A: ReviewedHsaExecutableLifecycleAdapterV1>(
        loaded: &LoadedHsaExecutableV1<P, A>,
        observed: &ObservedContext,
    ) -> Result<(), ()> {
        let physical = loaded.environment().physical_device();
        if observed.device().ordinal() != physical.hip_ordinal()
            || observed.device().target_id() != physical.target()
            || observed.device().target() != SCALAR_GEMM_V1_TARGET
        {
            return Err(());
        }
        Ok(())
    }

    fn validate_profile<
        P,
        K: CompilerGeneratedKernelExpectationV1,
        A: ReviewedHsaExecutableLifecycleAdapterV1,
    >(
        resolved: &ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>,
        generated: ScalarGemmV1DispatchIdentity,
    ) -> Result<(), ScalarGemmV1ProfileError> {
        let artifact = resolved.artifact_identity();
        let expected_contract = match K::PROFILE {
            CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
                generated_host_contract_identity,
            } => generated_host_contract_identity,
            _ => return Err(ScalarGemmV1ProfileError::UnsupportedGeneratedProfile),
        };
        if generated.kernel_binding_id != K::KERNEL_BINDING_ID_V1
            || generated.generated_host_contract_identity != expected_contract
        {
            return Err(ScalarGemmV1ProfileError::GeneratedIdentitySubstitution);
        }
        if K::LOGICAL_NAME != SCALAR_GEMM_V1_NAME
            || K::EXPORT_NAME != SCALAR_GEMM_V1_NAME
            || artifact.name().as_str() != SCALAR_GEMM_V1_NAME
            || artifact.symbol().as_str() != SCALAR_GEMM_V1_NAME
            || resolved.physical_kernel().export_symbol() != SCALAR_GEMM_V1_NAME
        {
            return Err(ScalarGemmV1ProfileError::KernelIdentitySubstitution);
        }
        validate_target(artifact.target())?;
        validate_abi(artifact.abi())?;
        validate_launch(artifact.launch())?;
        Ok(())
    }

    fn validate_target(target: &TargetIdentity) -> Result<(), ScalarGemmV1ProfileError> {
        if target.triple().as_str() != "amdgcn-amd-amdhsa"
            || target.architecture().as_str() != SCALAR_GEMM_V1_TARGET
            || target.pointer_width() != PointerWidth::Bits64
            || target.endianness() != fe2o3_artifacts::Endianness::Little
        {
            return Err(ScalarGemmV1ProfileError::TargetMismatch);
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    enum ExpectedArgument {
        SharedF32Slice,
        DisjointF32Slice,
        ScalarU32,
    }

    fn validate_abi(abi: &fe2o3_artifacts::AbiLayout) -> Result<(), ScalarGemmV1ProfileError> {
        const EXPECTED: &[(&str, u64, ExpectedArgument)] = &[
            ("a", 0, ExpectedArgument::SharedF32Slice),
            ("b", 16, ExpectedArgument::SharedF32Slice),
            ("c", 32, ExpectedArgument::DisjointF32Slice),
            ("m", 48, ExpectedArgument::ScalarU32),
            ("n", 52, ExpectedArgument::ScalarU32),
            ("k", 56, ExpectedArgument::ScalarU32),
        ];
        if abi.size() != SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES as u64
            || abi.alignment() != 8
            || abi.pointer_width() != PointerWidth::Bits64
            || abi.fields().len() != EXPECTED.len()
        {
            return Err(ScalarGemmV1ProfileError::AbiMismatch);
        }
        for (index, (field, (name, offset, expected))) in
            abi.fields().iter().zip(EXPECTED).enumerate()
        {
            validate_field(field, index, name, *offset, *expected)?;
        }
        Ok(())
    }

    fn validate_field(
        field: &AbiField,
        index: usize,
        name: &str,
        offset: u64,
        expected: ExpectedArgument,
    ) -> Result<(), ScalarGemmV1ProfileError> {
        let matches = match expected {
            ExpectedArgument::SharedF32Slice => {
                field.kind()
                    == AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    }
                    && field.size() == 16
                    && field.alignment() == 8
                    && field.mutability() == Mutability::Immutable
                    && field.access() == Access::ReadOnly
                    && field.address_space() == AddressSpace::Global
                    && field.type_identity()
                        == f32::shared_slice_type_identity_v1(PointerWidth::Bits64)
                    && field.ownership() == ArgumentOwnership::SharedBorrow
                    && field.alias_class() == AliasClass::SharedReadOnly
            }
            ExpectedArgument::DisjointF32Slice => {
                field.kind()
                    == AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    }
                    && field.size() == 16
                    && field.alignment() == 8
                    && field.mutability() == Mutability::Mutable
                    && field.access() == Access::ReadWrite
                    && field.address_space() == AddressSpace::Global
                    && field.type_identity()
                        == f32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
                    && field.ownership() == ArgumentOwnership::UniqueBorrow
                    && field.alias_class() == AliasClass::Exclusive
            }
            ExpectedArgument::ScalarU32 => {
                field.kind() == AbiKind::Scalar(ScalarType::U32)
                    && field.size() == 4
                    && field.alignment() == 4
                    && field.mutability() == Mutability::Immutable
                    && field.access() == Access::ByValue
                    && field.address_space() == AddressSpace::Value
                    && field.type_identity() == u32::scalar_type_identity_v1(PointerWidth::Bits64)
                    && field.ownership() == ArgumentOwnership::ByValue
                    && field.alias_class() == AliasClass::Value
            }
        };
        if field.name().as_str() != name || field.offset() != offset || !matches {
            return Err(ScalarGemmV1ProfileError::AbiFieldMismatch { index });
        }
        Ok(())
    }

    fn validate_launch(
        launch: &fe2o3_artifacts::LaunchContract,
    ) -> Result<(), ScalarGemmV1ProfileError> {
        let exact = match launch.block_size() {
            BlockSize::Exact(exact) => exact,
            BlockSize::Any | BlockSize::AtMost(_) => {
                return Err(ScalarGemmV1ProfileError::LaunchMismatch);
            }
        };
        let max_grid = launch.max_grid();
        if launch.rank() != 1
            || [exact.x(), exact.y(), exact.z()] != [SCALAR_GEMM_V1_BLOCK_X, 1, 1]
            || [max_grid.x(), max_grid.y(), max_grid.z()] != [u32::MAX, 1, 1]
            || launch.static_shared_memory_bytes() != 0
            || launch.max_dynamic_shared_memory_bytes() != 0
        {
            return Err(ScalarGemmV1ProfileError::LaunchMismatch);
        }
        Ok(())
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ScalarGemmShape {
        a_len: usize,
        b_len: usize,
        c_len: usize,
    }

    fn checked_shape(dimensions: [u32; 3]) -> Result<ScalarGemmShape, ScalarGemmV1ArgumentError> {
        let [m, n, k] = dimensions;
        fn extent(
            field: &'static str,
            rows: u32,
            columns: u32,
        ) -> Result<usize, ScalarGemmV1ArgumentError> {
            let elements = u64::from(rows)
                .checked_mul(u64::from(columns))
                .ok_or(ScalarGemmV1ArgumentError::ElementCountOverflow { field })?;
            elements
                .checked_mul(size_of::<f32>() as u64)
                .ok_or(ScalarGemmV1ArgumentError::ByteCountOverflow { field })?;
            usize::try_from(elements)
                .map_err(|_| ScalarGemmV1ArgumentError::HostLengthOverflow { field, elements })
        }
        Ok(ScalarGemmShape {
            a_len: extent("a", m, k)?,
            b_len: extent("b", k, n)?,
            c_len: extent("c", m, n)?,
        })
    }

    type ValidatedPack<'allocation> = (
        GeneratedPackedArgumentsV1<'allocation>,
        Option<ArgumentAliasAdmission<'allocation>>,
        Option<InFlightRegionRegistration<'allocation>>,
        usize,
    );

    fn validate_pack_and_admit<'allocation>(
        plan: &GeneratedArgumentPackingPlanV1,
        binding: GeneratedScalarGemmV1ArgumentBinding<'allocation>,
        observed: &ObservedContext,
    ) -> Result<ValidatedPack<'allocation>, ScalarGemmV1ArgumentError> {
        let shape = checked_shape(binding.dimensions)?;
        validate_slice_binding(plan, &binding, shape)?;
        let dimensions = binding.dimensions;
        let packed = plan
            .pack(binding.inputs)
            .map_err(ScalarGemmV1ArgumentError::Pack)?;
        if packed.kernel_id() != plan.kernel_id()
            || packed.len() != SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES
            || packed.alignment() != plan.kernarg_alignment()
        {
            return Err(ScalarGemmV1ArgumentError::PackedSubstitution);
        }
        validate_packed_dimensions(packed.bytes(), dimensions)?;

        if shape.c_len == 0 {
            return Ok((packed, None, None, 0));
        }
        let (admission, registration) =
            admit_and_register(observed.alias_registry(), observed, binding.accesses)
                .map_err(ScalarGemmV1ArgumentError::Alias)?;
        Ok((packed, Some(admission), Some(registration), shape.c_len))
    }

    fn validate_slice_binding(
        plan: &GeneratedArgumentPackingPlanV1,
        binding: &GeneratedScalarGemmV1ArgumentBinding<'_>,
        shape: ScalarGemmShape,
    ) -> Result<(), ScalarGemmV1ArgumentError> {
        let mut slices = binding
            .inputs
            .iter()
            .filter_map(GeneratedArgumentInputV1::slice_description_v1)
            .collect::<Vec<_>>();
        slices.sort_unstable_by_key(|slice| slice.argument_index);
        let expected = [
            (0, shape.a_len, ArgumentAccessMode::SharedRead),
            (1, shape.b_len, ArgumentAccessMode::SharedRead),
            (2, shape.c_len, ArgumentAccessMode::ExclusiveReadWrite),
        ];
        if slices.len() != expected.len() || binding.accesses.len() != expected.len() {
            return Err(ScalarGemmV1ArgumentError::SliceCount {
                slices: slices.len(),
                accesses: binding.accesses.len(),
            });
        }
        for ((slice, access), (argument_index, expected_len, mode)) in
            slices.iter().zip(&binding.accesses).zip(expected)
        {
            if slice.argument_index != argument_index {
                return Err(ScalarGemmV1ArgumentError::SliceSubstitution { argument_index });
            }
            let actual = usize::try_from(slice.length).map_err(|_| {
                ScalarGemmV1ArgumentError::HostLengthOverflow {
                    field: ["a", "b", "c"][argument_index],
                    elements: slice.length,
                }
            })?;
            if actual != expected_len {
                return Err(ScalarGemmV1ArgumentError::LengthMismatch {
                    argument_index,
                    expected: expected_len,
                    actual,
                });
            }
            if plan.argument(argument_index).is_none()
                || !access.matches_generated_slice_v1(
                    slice.address,
                    slice.length,
                    slice.element_size,
                    mode,
                )
            {
                return Err(ScalarGemmV1ArgumentError::SliceSubstitution { argument_index });
            }
        }
        Ok(())
    }

    fn validate_packed_dimensions(
        bytes: &[u8],
        dimensions: [u32; 3],
    ) -> Result<(), ScalarGemmV1ArgumentError> {
        for (offset, expected) in [48_usize, 52, 56].into_iter().zip(dimensions) {
            let actual = u32::from_le_bytes(
                bytes
                    .get(offset..offset + 4)
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or(ScalarGemmV1ArgumentError::PackedSubstitution)?,
            );
            if actual != expected {
                return Err(ScalarGemmV1ArgumentError::DimensionSubstitution { offset });
            }
        }
        if bytes.get(60..64) != Some(&[0, 0, 0, 0]) {
            return Err(ScalarGemmV1ArgumentError::NonzeroPadding);
        }
        Ok(())
    }

    fn scalar_gemm_geometry(
        identity: &ArtifactKernelIdentityV1,
        physical: &crate::PublishedPhysicalLaunchLayoutV1,
        output_extent: usize,
    ) -> Result<Option<HsaLaunchGeometryV1>, ScalarGemmV1GeometryError> {
        if output_extent == 0 {
            return Ok(None);
        }
        let launch = identity.launch();
        let exact = match launch.block_size() {
            BlockSize::Exact(exact) => exact,
            BlockSize::Any | BlockSize::AtMost(_) => {
                return Err(ScalarGemmV1GeometryError::UnsupportedLaunchContract);
            }
        };
        if launch.rank() != 1
            || [exact.x(), exact.y(), exact.z()] != [SCALAR_GEMM_V1_BLOCK_X, 1, 1]
            || launch.static_shared_memory_bytes() != 0
            || launch.max_dynamic_shared_memory_bytes() != 0
        {
            return Err(ScalarGemmV1GeometryError::UnsupportedLaunchContract);
        }
        let groups = scalar_gemm_groups(output_extent)?.expect("nonzero extent has a grid");
        let geometry = HsaLaunchGeometryV1::new([groups, 1, 1], [SCALAR_GEMM_V1_BLOCK_X, 1, 1], 0);
        validate_launch_geometry_contract(launch, physical, geometry)
            .map_err(ScalarGemmV1GeometryError::LaunchAuthorization)?;
        Ok(Some(geometry))
    }

    fn scalar_gemm_groups(output_extent: usize) -> Result<Option<u32>, ScalarGemmV1GeometryError> {
        if output_extent == 0 {
            return Ok(None);
        }
        let groups = u32::try_from(output_extent.div_ceil(SCALAR_GEMM_V1_BLOCK_X as usize))
            .map_err(|_| ScalarGemmV1GeometryError::GridOverflow { output_extent })?;
        groups.checked_mul(SCALAR_GEMM_V1_BLOCK_X).ok_or(
            ScalarGemmV1GeometryError::GlobalIndexDomainOverflow {
                output_extent,
                groups,
            },
        )?;
        Ok(Some(groups))
    }

    fn prepare_physical_kernarg<P, K, A: ReviewedHsaImplicitKernargAdapterV1>(
        resolved: &ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>,
        plan: &GeneratedArgumentPackingPlanV1,
        packed: &GeneratedPackedArgumentsV1<'_>,
    ) -> Result<ScalarGemmAlignedKernarg, ScalarGemmV1PhysicalKernargError> {
        let physical = resolved.physical_kernel().launch();
        prepare_physical_kernarg_parts(
            plan,
            packed,
            ScalarGemmPhysicalFacts {
                physical_size: physical.kernarg_segment_size(),
                physical_alignment: physical.kernarg_segment_alignment(),
                implicit_offset: physical.implicit_argument_offset(),
                implicit_size: physical.implicit_argument_size(),
                resolved_size: resolved.kernel_observation().kernarg_segment_size(),
                resolved_alignment: resolved.kernel_observation().kernarg_segment_alignment(),
            },
        )
    }

    #[derive(Clone, Copy)]
    struct ScalarGemmPhysicalFacts {
        physical_size: u64,
        physical_alignment: u64,
        implicit_offset: PhysicalMetadataValueV1<u64>,
        implicit_size: u64,
        resolved_size: u64,
        resolved_alignment: u64,
    }

    fn prepare_physical_kernarg_parts(
        plan: &GeneratedArgumentPackingPlanV1,
        packed: &GeneratedPackedArgumentsV1<'_>,
        facts: ScalarGemmPhysicalFacts,
    ) -> Result<ScalarGemmAlignedKernarg, ScalarGemmV1PhysicalKernargError> {
        if plan.kernarg_size() != SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES as u64
            || packed.len() != SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES
            || packed.alignment() != plan.kernarg_alignment()
        {
            return Err(ScalarGemmV1PhysicalKernargError::PackedSubstitution);
        }
        if facts.physical_size != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES as u64
            || facts.resolved_size != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES as u64
        {
            return Err(ScalarGemmV1PhysicalKernargError::KernargSegmentSize {
                physical: facts.physical_size,
                resolved: facts.resolved_size,
            });
        }
        if facts.implicit_offset
            != PhysicalMetadataValueV1::Known(SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES as u64)
            || facts.implicit_size != COV6_IMPLICIT_KERNARG_BYTES as u64
        {
            return Err(ScalarGemmV1PhysicalKernargError::ImplicitLayout);
        }
        if facts.physical_alignment != u64::from(plan.kernarg_alignment()) {
            return Err(ScalarGemmV1PhysicalKernargError::PhysicalAlignment {
                manifest: plan.kernarg_alignment(),
                physical: facts.physical_alignment,
            });
        }
        let expected_hsa_alignment = facts.physical_alignment.max(HSA_MINIMUM_KERNARG_ALIGNMENT);
        if facts.resolved_alignment != expected_hsa_alignment {
            return Err(ScalarGemmV1PhysicalKernargError::ResolvedAlignment {
                expected: expected_hsa_alignment,
                actual: facts.resolved_alignment,
            });
        }
        let alignment = usize::try_from(facts.resolved_alignment)
            .map_err(|_| ScalarGemmV1PhysicalKernargError::AlignmentOverflow)?;
        let mut storage =
            ScalarGemmAlignedKernarg::new(SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES, alignment)?;
        storage.bytes_mut()[..SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES]
            .copy_from_slice(packed.bytes());
        Ok(storage)
    }

    struct ScalarGemmAlignedKernarg {
        pointer: NonNull<u8>,
        layout: Layout,
    }

    impl ScalarGemmAlignedKernarg {
        fn new(
            byte_len: usize,
            alignment: usize,
        ) -> Result<Self, ScalarGemmV1PhysicalKernargError> {
            let layout = Layout::from_size_align(byte_len, alignment).map_err(|_| {
                ScalarGemmV1PhysicalKernargError::InvalidAllocationLayout {
                    byte_len,
                    alignment,
                }
            })?;
            // SAFETY: layout is valid and nonzero; Drop uses the same layout.
            let raw = unsafe { alloc_zeroed(layout) };
            let pointer = NonNull::new(raw).unwrap_or_else(|| handle_alloc_error(layout));
            Ok(Self { pointer, layout })
        }

        fn len(&self) -> usize {
            self.layout.size()
        }

        fn alignment(&self) -> usize {
            self.layout.align()
        }

        fn bytes_mut(&mut self) -> &mut [u8] {
            // SAFETY: this value uniquely owns exactly layout.size initialized bytes.
            unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
        }
    }

    impl Drop for ScalarGemmAlignedKernarg {
        fn drop(&mut self) {
            // SAFETY: pointer was allocated with this exact layout and is unique.
            unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum ScalarGemmV1ProfileError {
        UnsupportedGeneratedProfile,
        GeneratedIdentitySubstitution,
        KernelIdentitySubstitution,
        TargetMismatch,
        AbiMismatch,
        AbiFieldMismatch { index: usize },
        LaunchMismatch,
    }

    #[derive(Debug)]
    #[non_exhaustive]
    pub enum GeneratedScalarGemmV1PrepareError<PrerequisiteError, AdapterError> {
        UnsupportedProfile,
        ContextDeviceMismatch,
        Selection(WorkerV2TypedKernelSelectionError),
        Authentication(WorkerV2ExecutableAuthenticationError<PrerequisiteError>),
        Resolution(HsaExecutableLoadError<AdapterError>),
        Profile(ScalarGemmV1ProfileError),
        GeneratedLayout(GeneratedArgumentLayoutError),
        PackingPlan(GeneratedArgumentPackingError),
        Bind(GeneratedArgumentPackError),
        Arguments(ScalarGemmV1ArgumentError),
        Geometry(ScalarGemmV1GeometryError),
        PhysicalKernarg(ScalarGemmV1PhysicalKernargError),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum ScalarGemmV1ArgumentError {
        ElementCountOverflow {
            field: &'static str,
        },
        ByteCountOverflow {
            field: &'static str,
        },
        HostLengthOverflow {
            field: &'static str,
            elements: u64,
        },
        SliceCount {
            slices: usize,
            accesses: usize,
        },
        LengthMismatch {
            argument_index: usize,
            expected: usize,
            actual: usize,
        },
        SliceSubstitution {
            argument_index: usize,
        },
        DimensionSubstitution {
            offset: usize,
        },
        NonzeroPadding,
        Pack(GeneratedArgumentPackError),
        PackedSubstitution,
        Alias(AliasAdmissionError),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum ScalarGemmV1GeometryError {
        UnsupportedLaunchContract,
        GridOverflow { output_extent: usize },
        GlobalIndexDomainOverflow { output_extent: usize, groups: u32 },
        LaunchAuthorization(HsaLaunchAuthorizationError),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum ScalarGemmV1PhysicalKernargError {
        PackedSubstitution,
        KernargSegmentSize { physical: u64, resolved: u64 },
        ImplicitLayout,
        PhysicalAlignment { manifest: u32, physical: u64 },
        ResolvedAlignment { expected: u64, actual: u64 },
        AlignmentOverflow,
        InvalidAllocationLayout { byte_len: usize, alignment: usize },
    }

    impl<PrerequisiteError: fmt::Debug + fmt::Display, AdapterError: fmt::Debug + fmt::Display>
        fmt::Display for GeneratedScalarGemmV1PrepareError<PrerequisiteError, AdapterError>
    {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "Scalar GEMM V1 preparation failed: {self:?}")
        }
    }

    impl fmt::Display for ScalarGemmV1ProfileError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "Scalar GEMM V1 profile mismatch: {self:?}")
        }
    }

    impl fmt::Display for ScalarGemmV1ArgumentError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "Scalar GEMM V1 argument rejection: {self:?}")
        }
    }

    impl fmt::Display for ScalarGemmV1GeometryError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "Scalar GEMM V1 geometry rejection: {self:?}")
        }
    }

    impl fmt::Display for ScalarGemmV1PhysicalKernargError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Scalar GEMM V1 physical kernarg rejection: {self:?}"
            )
        }
    }

    impl<PrerequisiteError, AdapterError> std::error::Error
        for GeneratedScalarGemmV1PrepareError<PrerequisiteError, AdapterError>
    where
        PrerequisiteError: fmt::Debug + fmt::Display,
        AdapterError: fmt::Debug + fmt::Display,
    {
    }

    impl std::error::Error for ScalarGemmV1ProfileError {}
    impl std::error::Error for ScalarGemmV1ArgumentError {}
    impl std::error::Error for ScalarGemmV1GeometryError {}
    impl std::error::Error for ScalarGemmV1PhysicalKernargError {}

    #[cfg(any(test, feature = "hardware-test-hooks"))]
    pub(crate) fn scalar_gemm_v1_test_abi() -> AbiLayout {
        let scalar_u32 = |name: &str, offset: u64| {
            AbiField::new(
                Name::new(name).unwrap(),
                offset,
                4,
                4,
                AbiKind::Scalar(ScalarType::U32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                u32::scalar_type_identity_v1(PointerWidth::Bits64),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap()
        };
        let slice = |name: &str, offset: u64, read_write: bool| {
            AbiField::new(
                Name::new(name).unwrap(),
                offset,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                if read_write {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                },
                if read_write {
                    Access::ReadWrite
                } else {
                    Access::ReadOnly
                },
                AddressSpace::Global,
                if read_write {
                    f32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
                } else {
                    f32::shared_slice_type_identity_v1(PointerWidth::Bits64)
                },
                if read_write {
                    ArgumentOwnership::UniqueBorrow
                } else {
                    ArgumentOwnership::SharedBorrow
                },
                if read_write {
                    AliasClass::Exclusive
                } else {
                    AliasClass::SharedReadOnly
                },
            )
            .unwrap()
        };
        AbiLayout::new(
            64,
            8,
            PointerWidth::Bits64,
            vec![
                slice("a", 0, false),
                slice("b", 16, false),
                slice("c", 32, true),
                scalar_u32("m", 48),
                scalar_u32("n", 52),
                scalar_u32("k", 56),
            ],
        )
        .unwrap()
    }

    #[cfg(any(test, feature = "hardware-test-hooks"))]
    pub(crate) fn scalar_gemm_v1_test_launch() -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(SCALAR_GEMM_V1_BLOCK_X, 1, 1).unwrap()),
            Dimensions::new(u32::MAX, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::generated_argument_plan::validate_argument_packing;
        use crate::{AllocationProvenance, KernelId};
        use fe2o3_artifacts::{
            AbiLayout, Dimensions, Endianness, IdentityText, LaunchContract, Name,
        };

        fn scalar_u32(name: &str, offset: u64) -> AbiField {
            AbiField::new(
                Name::new(name).unwrap(),
                offset,
                4,
                4,
                AbiKind::Scalar(ScalarType::U32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                u32::scalar_type_identity_v1(PointerWidth::Bits64),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap()
        }

        fn slice(name: &str, offset: u64, read_write: bool) -> AbiField {
            AbiField::new(
                Name::new(name).unwrap(),
                offset,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                if read_write {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                },
                if read_write {
                    Access::ReadWrite
                } else {
                    Access::ReadOnly
                },
                AddressSpace::Global,
                if read_write {
                    f32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
                } else {
                    f32::shared_slice_type_identity_v1(PointerWidth::Bits64)
                },
                if read_write {
                    ArgumentOwnership::UniqueBorrow
                } else {
                    ArgumentOwnership::SharedBorrow
                },
                if read_write {
                    AliasClass::Exclusive
                } else {
                    AliasClass::SharedReadOnly
                },
            )
            .unwrap()
        }

        fn fields() -> Vec<AbiField> {
            vec![
                slice("a", 0, false),
                slice("b", 16, false),
                slice("c", 32, true),
                scalar_u32("m", 48),
                scalar_u32("n", 52),
                scalar_u32("k", 56),
            ]
        }

        fn abi() -> AbiLayout {
            super::scalar_gemm_v1_test_abi()
        }

        fn plan(seed: u8) -> GeneratedArgumentPackingPlanV1 {
            let generated =
                CompilerGeneratedArgumentLayoutV1::new(64, 8, PointerWidth::Bits64, fields())
                    .unwrap();
            validate_argument_packing(KernelId::from_bytes([seed; 32]), &abi(), &generated).unwrap()
        }

        unsafe fn access<'allocation>(
            observed: &ObservedContext,
            owner: &'allocation (),
            address: usize,
            length: usize,
            mode: ArgumentAccessMode,
        ) -> ArgumentAccess<'allocation> {
            let byte_length = length.checked_mul(size_of::<f32>()).unwrap();
            // SAFETY: these inert unit-test addresses are never dispatched.
            let provenance = unsafe {
                AllocationProvenance::from_raw_parts(
                    observed,
                    owner,
                    address as *mut u8,
                    byte_length,
                )
                .unwrap()
            };
            ArgumentAccess::new(provenance.region(0, byte_length).unwrap(), mode)
        }

        fn binding<'allocation>(
            plan: &GeneratedArgumentPackingPlanV1,
            dimensions: [u32; 3],
            packed_dimensions: [u32; 3],
            lengths: [usize; 3],
            addresses: [usize; 3],
            accesses: Vec<ArgumentAccess<'allocation>>,
        ) -> GeneratedScalarGemmV1ArgumentBinding<'allocation> {
            let mut inputs = Vec::new();
            for (index, ((length, address), access)) in lengths
                .into_iter()
                .zip(addresses)
                .zip([Access::ReadOnly, Access::ReadOnly, Access::ReadWrite])
                .enumerate()
            {
                // SAFETY: inert addresses and matching accesses are supplied by
                // each test and are never sent to a runtime adapter.
                inputs.push(unsafe {
                    plan.slice(
                        index,
                        address as *const (),
                        length as u64,
                        PointerWidth::Bits64,
                        AddressSpace::Global,
                        access,
                    )
                    .unwrap()
                });
            }
            inputs.push(plan.scalar_u32(3, packed_dimensions[0]).unwrap());
            inputs.push(plan.scalar_u32(4, packed_dimensions[1]).unwrap());
            inputs.push(plan.scalar_u32(5, packed_dimensions[2]).unwrap());
            // SAFETY: helper arguments explicitly model the unsafe generated SPI.
            unsafe {
                GeneratedScalarGemmV1ArgumentBinding::from_compiler_generated_parts_v1(
                    inputs, accesses, dimensions,
                )
            }
        }

        fn launch() -> LaunchContract {
            super::scalar_gemm_v1_test_launch()
        }

        fn physical_facts() -> ScalarGemmPhysicalFacts {
            ScalarGemmPhysicalFacts {
                physical_size: 320,
                physical_alignment: 8,
                implicit_offset: PhysicalMetadataValueV1::Known(64),
                implicit_size: 256,
                resolved_size: 320,
                resolved_alignment: 16,
            }
        }

        #[test]
        fn exact_abi_and_launch_reject_substitution() {
            assert_eq!(validate_abi(&abi()), Ok(()));
            assert_eq!(validate_launch(&launch()), Ok(()));

            let wrong_name = AbiLayout::new(
                64,
                8,
                PointerWidth::Bits64,
                vec![
                    slice("arg0", 0, false),
                    slice("b", 16, false),
                    slice("c", 32, true),
                    scalar_u32("m", 48),
                    scalar_u32("n", 52),
                    scalar_u32("k", 56),
                ],
            )
            .unwrap();
            assert_eq!(
                validate_abi(&wrong_name),
                Err(ScalarGemmV1ProfileError::AbiFieldMismatch { index: 0 })
            );

            let wrong_grid = LaunchContract::new(
                1,
                BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
                Dimensions::new(u32::MAX - 1, 1, 1).unwrap(),
                0,
                0,
            )
            .unwrap();
            assert_eq!(
                validate_launch(&wrong_grid),
                Err(ScalarGemmV1ProfileError::LaunchMismatch)
            );
        }

        #[test]
        fn target_identity_is_exact_gfx942_xnack_minus() {
            let target = |architecture: &str| {
                TargetIdentity::new(
                    IdentityText::new("amdgcn-amd-amdhsa").unwrap(),
                    IdentityText::new(architecture).unwrap(),
                    PointerWidth::Bits64,
                    Endianness::Little,
                    vec![],
                )
                .unwrap()
            };
            assert_eq!(validate_target(&target(SCALAR_GEMM_V1_TARGET)), Ok(()));
            assert_eq!(
                validate_target(&target("gfx942")),
                Err(ScalarGemmV1ProfileError::TargetMismatch)
            );
            assert_eq!(
                validate_target(&target("gfx942:xnack+")),
                Err(ScalarGemmV1ProfileError::TargetMismatch)
            );
        }

        #[test]
        fn checked_shape_covers_rectangles_zeroes_and_overflow() {
            assert_eq!(
                checked_shape([2, 3, 4]).unwrap(),
                ScalarGemmShape {
                    a_len: 8,
                    b_len: 12,
                    c_len: 6,
                }
            );
            assert_eq!(
                checked_shape([2, 3, 0]).unwrap(),
                ScalarGemmShape {
                    a_len: 0,
                    b_len: 0,
                    c_len: 6,
                }
            );
            assert_eq!(
                checked_shape([0, u32::MAX, 1]).unwrap(),
                ScalarGemmShape {
                    a_len: 0,
                    b_len: u32::MAX as usize,
                    c_len: 0,
                }
            );
            assert!(matches!(
                checked_shape([u32::MAX, u32::MAX, u32::MAX]),
                Err(ScalarGemmV1ArgumentError::ByteCountOverflow { .. })
            ));
        }

        #[test]
        fn exact_pack_retains_order_dimensions_padding_and_cov6_size() {
            let observed = ObservedContext::for_test(0x91, 0, SCALAR_GEMM_V1_TARGET, 1_024, 65_536);
            let a_owner = ();
            let b_owner = ();
            let c_owner = ();
            let plan = plan(0x91);
            let accesses = vec![
                unsafe {
                    access(
                        &observed,
                        &a_owner,
                        0x1000,
                        8,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &b_owner,
                        0x2000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &c_owner,
                        0x3000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            let binding = binding(
                &plan,
                [2, 3, 4],
                [2, 3, 4],
                [8, 12, 6],
                [0x1000, 0x2000, 0x3000],
                accesses,
            );
            let (packed, admission, registration, extent) =
                validate_pack_and_admit(&plan, binding, &observed).unwrap();
            assert_eq!(extent, 6);
            assert!(admission.is_some());
            assert!(registration.is_some());
            assert_eq!(
                &packed.bytes()[48..60],
                &[2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]
            );
            assert_eq!(&packed.bytes()[60..64], &[0; 4]);

            let storage = prepare_physical_kernarg_parts(&plan, &packed, physical_facts()).unwrap();
            assert_eq!(storage.len(), 320);
            assert_eq!(storage.alignment(), 16);
        }

        #[test]
        fn dimensions_lengths_and_aliases_fail_closed_while_inputs_may_alias() {
            let observed = ObservedContext::for_test(0x92, 0, SCALAR_GEMM_V1_TARGET, 1_024, 65_536);
            let shared_owner = ();
            let c_owner = ();
            let plan = plan(0x92);
            let shared_inputs = vec![
                unsafe {
                    access(
                        &observed,
                        &shared_owner,
                        0x1000,
                        8,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &shared_owner,
                        0x1000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &c_owner,
                        0x3000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            let _validated = validate_pack_and_admit(
                &plan,
                binding(
                    &plan,
                    [2, 3, 4],
                    [2, 3, 4],
                    [8, 12, 6],
                    [0x1000, 0x1000, 0x3000],
                    shared_inputs,
                ),
                &observed,
            )
            .unwrap();

            let owners = [(), (), ()];
            let wrong_length = vec![
                unsafe {
                    access(
                        &observed,
                        &owners[0],
                        0x1000,
                        7,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[1],
                        0x2000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[2],
                        0x3000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            assert!(matches!(
                validate_pack_and_admit(
                    &plan,
                    binding(
                        &plan,
                        [2, 3, 4],
                        [2, 3, 4],
                        [7, 12, 6],
                        [0x1000, 0x2000, 0x3000],
                        wrong_length,
                    ),
                    &observed,
                ),
                Err(ScalarGemmV1ArgumentError::LengthMismatch {
                    argument_index: 0,
                    ..
                })
            ));

            let wrong_dimensions = vec![
                unsafe {
                    access(
                        &observed,
                        &owners[0],
                        0x1000,
                        8,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[1],
                        0x2000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[2],
                        0x3000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            assert!(matches!(
                validate_pack_and_admit(
                    &plan,
                    binding(
                        &plan,
                        [2, 3, 4],
                        [9, 3, 4],
                        [8, 12, 6],
                        [0x1000, 0x2000, 0x3000],
                        wrong_dimensions,
                    ),
                    &observed,
                ),
                Err(ScalarGemmV1ArgumentError::DimensionSubstitution { offset: 48 })
            ));

            let alias_owner = ();
            let aliased_output = vec![
                unsafe {
                    access(
                        &observed,
                        &alias_owner,
                        0x4000,
                        8,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[1],
                        0x5000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &alias_owner,
                        0x4000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            assert!(matches!(
                validate_pack_and_admit(
                    &plan,
                    binding(
                        &plan,
                        [2, 3, 4],
                        [2, 3, 4],
                        [8, 12, 6],
                        [0x4000, 0x5000, 0x4000],
                        aliased_output,
                    ),
                    &observed,
                ),
                Err(ScalarGemmV1ArgumentError::Alias(
                    AliasAdmissionError::Conflict { .. }
                ))
            ));
        }

        #[test]
        fn zero_output_is_no_dispatch_but_zero_k_still_dispatches() {
            let observed = ObservedContext::for_test(0x93, 0, SCALAR_GEMM_V1_TARGET, 1_024, 65_536);
            let owners = [(), (), ()];
            let plan = plan(0x93);
            let zero_output_accesses = vec![
                unsafe { access(&observed, &owners[0], 0, 0, ArgumentAccessMode::SharedRead) },
                unsafe {
                    access(
                        &observed,
                        &owners[1],
                        0x2000,
                        12,
                        ArgumentAccessMode::SharedRead,
                    )
                },
                unsafe {
                    access(
                        &observed,
                        &owners[2],
                        0,
                        0,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            let (_, admission, registration, extent) = validate_pack_and_admit(
                &plan,
                binding(
                    &plan,
                    [0, 3, 4],
                    [0, 3, 4],
                    [0, 12, 0],
                    [0, 0x2000, 0],
                    zero_output_accesses,
                ),
                &observed,
            )
            .unwrap();
            assert_eq!(extent, 0);
            assert!(admission.is_none());
            assert!(registration.is_none());
            assert_eq!(scalar_gemm_groups(extent).unwrap(), None);

            assert_eq!(scalar_gemm_groups(1).unwrap(), Some(1));
            assert_eq!(scalar_gemm_groups(256).unwrap(), Some(1));
            assert_eq!(scalar_gemm_groups(257).unwrap(), Some(2));
            let maximum = u32::MAX as usize - 255;
            assert!(scalar_gemm_groups(maximum).is_ok());
            assert!(matches!(
                scalar_gemm_groups(maximum + 1),
                Err(ScalarGemmV1GeometryError::GlobalIndexDomainOverflow { .. })
            ));

            let zero_k_accesses = vec![
                unsafe { access(&observed, &owners[0], 0, 0, ArgumentAccessMode::SharedRead) },
                unsafe { access(&observed, &owners[1], 0, 0, ArgumentAccessMode::SharedRead) },
                unsafe {
                    access(
                        &observed,
                        &owners[2],
                        0x3000,
                        6,
                        ArgumentAccessMode::ExclusiveReadWrite,
                    )
                },
            ];
            let (_, admission, registration, extent) = validate_pack_and_admit(
                &plan,
                binding(
                    &plan,
                    [2, 3, 0],
                    [2, 3, 0],
                    [0, 0, 6],
                    [0, 0, 0x3000],
                    zero_k_accesses,
                ),
                &observed,
            )
            .unwrap();
            assert_eq!(extent, 6);
            assert!(admission.is_some());
            assert!(registration.is_some());
        }

        #[test]
        fn physical_descriptor_must_be_exact_cov6() {
            let plan = plan(0x94);
            let packed = plan
                .pack([
                    unsafe {
                        plan.slice(
                            0,
                            0x1000usize as *const (),
                            1,
                            PointerWidth::Bits64,
                            AddressSpace::Global,
                            Access::ReadOnly,
                        )
                        .unwrap()
                    },
                    unsafe {
                        plan.slice(
                            1,
                            0x2000usize as *const (),
                            1,
                            PointerWidth::Bits64,
                            AddressSpace::Global,
                            Access::ReadOnly,
                        )
                        .unwrap()
                    },
                    unsafe {
                        plan.slice(
                            2,
                            0x3000usize as *const (),
                            1,
                            PointerWidth::Bits64,
                            AddressSpace::Global,
                            Access::ReadWrite,
                        )
                        .unwrap()
                    },
                    plan.scalar_u32(3, 1).unwrap(),
                    plan.scalar_u32(4, 1).unwrap(),
                    plan.scalar_u32(5, 1).unwrap(),
                ])
                .unwrap();
            assert_eq!(
                prepare_physical_kernarg_parts(&plan, &packed, physical_facts())
                    .unwrap()
                    .len(),
                320
            );
            let mut wrong = physical_facts();
            wrong.resolved_size = 319;
            assert!(matches!(
                prepare_physical_kernarg_parts(&plan, &packed, wrong),
                Err(ScalarGemmV1PhysicalKernargError::KernargSegmentSize { .. })
            ));
            let mut wrong = physical_facts();
            wrong.implicit_offset = PhysicalMetadataValueV1::Known(60);
            assert!(matches!(
                prepare_physical_kernarg_parts(&plan, &packed, wrong),
                Err(ScalarGemmV1PhysicalKernargError::ImplicitLayout)
            ));
        }
    }
}

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub use worker_v2::{
    CompilerGeneratedScalarGemmV1Arguments, GeneratedScalarGemmV1ArgumentBinding,
    GeneratedScalarGemmV1Completion, GeneratedScalarGemmV1PrepareError,
    GeneratedScalarGemmV1PrepareResult, GeneratedScalarGemmV1PreparedInvocation,
    ScalarGemmV1ArgumentError, ScalarGemmV1DispatchIdentity, ScalarGemmV1GeometryError,
    ScalarGemmV1PhysicalKernargError, ScalarGemmV1ProfileError,
};
#[cfg(test)]
pub(crate) use worker_v2::{scalar_gemm_v1_test_abi, scalar_gemm_v1_test_launch};
