use crate::argument_alias::{InFlightRegionRegistration, admit_and_register};
use crate::generated_argument_plan::{
    GeneratedArgumentInputV1, GeneratedDeviceScalarV1, GeneratedPackedArgumentsV1,
    GeneratedSliceInputDescriptionV1,
};
use crate::hsa_executable_lifecycle::{
    AuthenticatedLoadedWorkerV2KernelSelectionV1, HsaCompletedSelectedWorkerV2DispatchV1,
    ResolvedLoadedWorkerV2KernelSelectionV1, validate_launch_geometry_contract,
};
use crate::{
    AliasAdmissionError, ArgumentAccess, ArgumentAccessMode, ArgumentAliasAdmission,
    ArtifactKernelIdentityV1, CompilerGeneratedArgumentLayoutV1,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedSliceArgumentPairV1, HsaCompletedDispatchV1,
    HsaExecutableLoadError, HsaGeneratedDispatchError, HsaLaunchAuthorizationError,
    HsaLaunchGeometryV1, KernelId, LoadedHsaExecutableV1, ObservedContext, PhysicalMetadataValueV1,
    ReviewedHsaExecutableLifecycleAdapterV1, ReviewedHsaImplicitKernargAdapterV1,
    WorkerV2ExecutableAuthenticationError, WorkerV2PrerequisiteAuthenticatorV1,
    WorkerV2TypedKernelSelectionError,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Mutability,
    PointerWidth, ScalarType,
};
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::fmt;
use std::ptr::NonNull;

const ALPHA_ZETA_COV6_BLOCK_X: u32 = 256;
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
const HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const ALPHA_EXPLICIT_KERNARG_BYTES: usize = 40;
const ZETA_EXPLICIT_KERNARG_BYTES: usize = 56;

/// Opaque output of one compiler-generated alpha/zeta argument adapter.
///
/// Its fields are private. Only an unsafe generated adapter implementation can
/// assert that every scalar input and retained slice capability describes the
/// exact profile signature.
#[doc(hidden)]
pub struct GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation> {
    inputs: Vec<GeneratedArgumentInputV1<'allocation>>,
    accesses: Vec<ArgumentAccess<'allocation>>,
}

impl fmt::Debug for GeneratedAlphaZetaCov6ArgumentBindingV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAlphaZetaCov6ArgumentBindingV1")
            .field("input_count", &self.inputs.len())
            .field("access_count", &self.accesses.len())
            .finish_non_exhaustive()
    }
}

impl<'allocation> GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation> {
    /// Joins compiler-bound scalars to opaque slice input/access pairs.
    ///
    /// # Safety
    ///
    /// `scalar_inputs` must contain every scalar source argument exactly once
    /// and no slice input. `slice_arguments` must contain every slice in source
    /// order. The enclosing unsafe adapter value must retain the capabilities
    /// that emitted those opaque pairs through synchronous completion.
    #[doc(hidden)]
    pub unsafe fn from_compiler_generated_parts_v1(
        scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
        slice_arguments: Vec<GeneratedSliceArgumentPairV1<'allocation>>,
    ) -> Self {
        let mut inputs: Vec<GeneratedArgumentInputV1<'allocation>> =
            Vec::with_capacity(scalar_inputs.len() + slice_arguments.len());
        inputs.extend(scalar_inputs);
        let mut accesses = Vec::with_capacity(slice_arguments.len());
        for argument in slice_arguments {
            let (input, access) = argument.into_parts();
            inputs.push(input);
            accesses.push(access);
        }
        Self { inputs, accesses }
    }
}

/// Exact kernel role supported by this bounded transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum AlphaZetaCov6KernelRoleV1 {
    Alpha,
    Zeta,
}

impl AlphaZetaCov6KernelRoleV1 {
    const fn logical_name(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Zeta => "zeta",
        }
    }
}

/// Versioned generated dispatch-domain descriptor authenticated against the
/// backend-witness-validated marker and admitted Worker V2 kernel identity.
///
/// V1 denotes exactly the alpha/zeta COV6 domain: every slice is a non-empty
/// `f32` slice of one equal logical length, block size is exactly 256, and the
/// one-dimensional grid is tail-rounded while remaining representable in the
/// kernel's `u32` global-index domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct AlphaZetaCov6DispatchIdentityV1 {
    role: AlphaZetaCov6KernelRoleV1,
    expected_kernel_id: KernelId,
    kernel_binding_id: [u8; 32],
    generated_host_contract_identity: [u8; 32],
}

impl AlphaZetaCov6DispatchIdentityV1 {
    pub const fn new(
        role: AlphaZetaCov6KernelRoleV1,
        expected_kernel_id: KernelId,
        kernel_binding_id: [u8; 32],
        generated_host_contract_identity: [u8; 32],
    ) -> Self {
        Self {
            role,
            expected_kernel_id,
            kernel_binding_id,
            generated_host_contract_identity,
        }
    }
}

/// Compiler-generated bridge from one exact `Arguments` type to host launch
/// plumbing.
///
/// # Safety
///
/// An implementation must be emitted from the same independently reconstructed
/// Rust signature and effects that produced `K`'s backend semantic witness. It
/// must return the exact generated layout and identity descriptor for `K`, bind
/// every field without substitution, obtain every slice pair from its retained
/// generated capability, and retain all resources in `self` until it is
/// dropped. A false implementation can authorize native GPU memory accesses.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedAlphaZetaCov6ArgumentsV1<
    'allocation,
    K: CompilerGeneratedKernelExpectationV1,
>
{
    fn dispatch_identity_v1() -> AlphaZetaCov6DispatchIdentityV1;

    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation>, GeneratedArgumentPackError>;
}

/// Linear prepared invocation for one selected alpha/zeta COV6 kernel.
///
/// This value is intentionally neither `Clone` nor `Copy`. It retains the
/// loaded executable borrow, selected kernel handle, generated argument owner,
/// alias admission, in-flight registration, and complete aligned COV6 kernarg.
#[must_use = "a prepared alpha/zeta COV6 invocation does no work until dispatched"]
#[doc(hidden)]
pub struct GeneratedAlphaZetaCov6PreparedInvocationV1<
    'loaded,
    'allocation,
    P,
    K,
    A: ReviewedHsaImplicitKernargAdapterV1,
    Arguments,
> {
    resolved: ResolvedLoadedWorkerV2KernelSelectionV1<'loaded, P, K, A>,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: usize,
    kernarg: AlignedKernargStorageV1,
    arguments: Arguments,
    admission: ArgumentAliasAdmission<'allocation>,
    registration: InFlightRegionRegistration<'allocation>,
}

/// Result of preparing one generated selected-kernel invocation.
#[doc(hidden)]
pub type GeneratedAlphaZetaCov6PrepareResultV1<
    'loaded,
    'allocation,
    P,
    K,
    A,
    Arguments,
    PrerequisiteError,
> = Result<
    GeneratedAlphaZetaCov6PreparedInvocationV1<'loaded, 'allocation, P, K, A, Arguments>,
    GeneratedAlphaZetaCov6PrepareError<
        PrerequisiteError,
        <A as ReviewedHsaExecutableLifecycleAdapterV1>::Error,
    >,
>;

impl<P, K, A, Arguments> GeneratedAlphaZetaCov6PreparedInvocationV1<'_, '_, P, K, A, Arguments>
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
    }

    pub const fn explicit_byte_len(&self) -> usize {
        self.explicit_byte_len
    }

    pub fn physical_kernarg_byte_len(&self) -> usize {
        self.kernarg.len()
    }

    pub fn physical_kernarg_alignment(&self) -> usize {
        self.kernarg.alignment()
    }

    /// Initializes COV6 implicit bytes and synchronously dispatches the exact
    /// selected kernel. All retained resources are released only after the
    /// reviewed adapter reports quiescent completion or a definite failure.
    pub fn dispatch(
        self,
    ) -> Result<GeneratedAlphaZetaCov6CompletionV1<K>, HsaGeneratedDispatchError<A::Error>> {
        let Self {
            resolved,
            geometry,
            explicit_byte_len,
            mut kernarg,
            arguments,
            admission,
            registration,
        } = self;
        let retained = (&arguments, &admission, &registration);
        // SAFETY: the unsafe generated adapter contract binds every packed
        // value to `arguments`; host validation matched every slice to the
        // retained access records; admission and registration reserve those
        // regions; storage is exact, initialized, and aligned; the operation
        // is synchronous by the reviewed adapter contract.
        let completed = unsafe {
            resolved.dispatch_generated_and_wait(
                geometry,
                kernarg.bytes_mut(),
                explicit_byte_len,
                explicit_byte_len,
                COV6_IMPLICIT_KERNARG_BYTES,
            )
        }?;
        let _ = retained;
        Ok(GeneratedAlphaZetaCov6CompletionV1 { completed })
    }
}

/// Quiescent completion evidence for one exact generated alpha/zeta COV6 marker.
#[derive(Debug)]
#[doc(hidden)]
pub struct GeneratedAlphaZetaCov6CompletionV1<K> {
    completed: HsaCompletedSelectedWorkerV2DispatchV1<K>,
}

impl<K> GeneratedAlphaZetaCov6CompletionV1<K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.completed.artifact_identity()
    }

    pub const fn completed_dispatch(&self) -> &HsaCompletedDispatchV1 {
        self.completed.completed_dispatch()
    }
}

impl<P, A> LoadedHsaExecutableV1<P, A>
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    /// Selects, authenticates, resolves, and prepares one exact alpha/zeta COV6
    /// kernel from this already loaded Worker V2 executable.
    ///
    /// This is generated-code SPI. Application code cannot safely implement
    /// the required argument adapter or construct its opaque binding.
    #[doc(hidden)]
    pub fn prepare_generated_alpha_zeta_cov6_selected_kernel_v1<
        'loaded,
        'allocation,
        K,
        Authenticator,
        Arguments,
    >(
        &'loaded mut self,
        observed: &ObservedContext,
        authenticator: &mut Authenticator,
        arguments: Arguments,
    ) -> GeneratedAlphaZetaCov6PrepareResultV1<
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
        Arguments: CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, K>,
    {
        if !matches!(
            K::PROFILE,
            CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 { .. }
        ) {
            return Err(GeneratedAlphaZetaCov6PrepareError::UnsupportedProfile);
        }
        validate_context_device(self, observed)
            .map_err(|()| GeneratedAlphaZetaCov6PrepareError::ContextDeviceMismatch)?;

        let selection = self
            .select_typed_kernel::<K>()
            .map_err(GeneratedAlphaZetaCov6PrepareError::Selection)?;
        // `authenticate` consumes and validates K's backend semantic witness
        // before entering the unsafe prerequisite authenticator.
        let authenticated: AuthenticatedLoadedWorkerV2KernelSelectionV1<K> = selection
            .authenticate(authenticator)
            .map_err(GeneratedAlphaZetaCov6PrepareError::Authentication)?;
        let resolved = authenticated
            .resolve(self)
            .map_err(GeneratedAlphaZetaCov6PrepareError::Resolution)?;

        validate_alpha_zeta_cov6_profile(&resolved, Arguments::dispatch_identity_v1())
            .map_err(GeneratedAlphaZetaCov6PrepareError::Profile)?;

        let generated = Arguments::generated_argument_layout_v1()
            .map_err(GeneratedAlphaZetaCov6PrepareError::GeneratedLayout)?;
        // SAFETY: only an unsafe generated adapter implementation can supply
        // this independently generated layout for K.
        let plan = unsafe { resolved.validate_argument_packing(&generated) }
            .map_err(GeneratedAlphaZetaCov6PrepareError::PackingPlan)?;
        let binding = arguments
            .bind_arguments_v1(&plan)
            .map_err(GeneratedAlphaZetaCov6PrepareError::Bind)?;
        let (packed, admission, registration, logical_length) =
            validate_pack_and_admit(&plan, binding, observed)
                .map_err(GeneratedAlphaZetaCov6PrepareError::Arguments)?;
        let geometry = alpha_zeta_cov6_geometry(
            resolved.artifact_identity(),
            resolved.physical_kernel().launch(),
            logical_length,
        )
        .map_err(GeneratedAlphaZetaCov6PrepareError::Geometry)?;
        let kernarg = prepare_physical_kernarg(&resolved, &plan, &packed)
            .map_err(GeneratedAlphaZetaCov6PrepareError::PhysicalKernarg)?;

        Ok(GeneratedAlphaZetaCov6PreparedInvocationV1 {
            resolved,
            geometry,
            explicit_byte_len: packed.len(),
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
    {
        return Err(());
    }
    Ok(())
}

fn validate_alpha_zeta_cov6_profile<
    P,
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
>(
    resolved: &ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>,
    generated: AlphaZetaCov6DispatchIdentityV1,
) -> Result<(), AlphaZetaCov6ProfileError> {
    let artifact = resolved.artifact_identity();
    let expected_contract = match K::PROFILE {
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity,
        } => generated_host_contract_identity,
        _ => return Err(AlphaZetaCov6ProfileError::UnsupportedGeneratedProfile),
    };
    if generated.kernel_binding_id != K::KERNEL_BINDING_ID_V1
        || generated.generated_host_contract_identity != expected_contract
    {
        return Err(AlphaZetaCov6ProfileError::GeneratedIdentitySubstitution);
    }
    if generated.expected_kernel_id != artifact.kernel_id() {
        return Err(AlphaZetaCov6ProfileError::KernelIdentitySubstitution);
    }
    let expected_name = generated.role.logical_name();
    if K::LOGICAL_NAME != expected_name
        || K::EXPORT_NAME != expected_name
        || artifact.name().as_str() != expected_name
        || artifact.symbol().as_str() != expected_name
        || resolved.physical_kernel().export_symbol() != expected_name
    {
        return Err(AlphaZetaCov6ProfileError::KernelRoleSubstitution);
    }
    validate_alpha_zeta_cov6_abi(artifact.abi(), generated.role)?;
    validate_alpha_zeta_cov6_launch(artifact.launch())?;
    Ok(())
}

#[derive(Clone, Copy)]
enum AlphaZetaExpectedArgumentV1 {
    ScalarF32,
    SharedF32Slice,
    DisjointF32Slice,
}

fn validate_alpha_zeta_cov6_abi(
    abi: &fe2o3_artifacts::AbiLayout,
    role: AlphaZetaCov6KernelRoleV1,
) -> Result<(), AlphaZetaCov6ProfileError> {
    let (size, expected): (u64, &[(&str, u64, AlphaZetaExpectedArgumentV1)]) = match role {
        AlphaZetaCov6KernelRoleV1::Alpha => (
            ALPHA_EXPLICIT_KERNARG_BYTES as u64,
            &[
                ("scale", 0, AlphaZetaExpectedArgumentV1::ScalarF32),
                ("input", 8, AlphaZetaExpectedArgumentV1::SharedF32Slice),
                ("output", 24, AlphaZetaExpectedArgumentV1::DisjointF32Slice),
            ],
        ),
        AlphaZetaCov6KernelRoleV1::Zeta => (
            ZETA_EXPLICIT_KERNARG_BYTES as u64,
            &[
                ("a", 0, AlphaZetaExpectedArgumentV1::SharedF32Slice),
                ("b", 16, AlphaZetaExpectedArgumentV1::SharedF32Slice),
                ("bias", 32, AlphaZetaExpectedArgumentV1::ScalarF32),
                ("output", 40, AlphaZetaExpectedArgumentV1::DisjointF32Slice),
            ],
        ),
    };
    if abi.size() != size
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != expected.len()
    {
        return Err(AlphaZetaCov6ProfileError::AbiMismatch);
    }
    for (index, (field, (name, offset, expected))) in abi.fields().iter().zip(expected).enumerate()
    {
        validate_alpha_zeta_cov6_field(field, index, name, *offset, *expected)?;
    }
    Ok(())
}

fn validate_alpha_zeta_cov6_field(
    field: &AbiField,
    index: usize,
    name: &str,
    offset: u64,
    expected: AlphaZetaExpectedArgumentV1,
) -> Result<(), AlphaZetaCov6ProfileError> {
    let matches = match expected {
        AlphaZetaExpectedArgumentV1::ScalarF32 => {
            field.kind() == AbiKind::Scalar(ScalarType::F32)
                && field.size() == 4
                && field.alignment() == 4
                && field.mutability() == Mutability::Immutable
                && field.access() == Access::ByValue
                && field.address_space() == AddressSpace::Value
                && field.type_identity() == f32::scalar_type_identity_v1(PointerWidth::Bits64)
                && field.ownership() == ArgumentOwnership::ByValue
                && field.alias_class() == AliasClass::Value
        }
        AlphaZetaExpectedArgumentV1::SharedF32Slice => {
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
                && field.type_identity() == f32::shared_slice_type_identity_v1(PointerWidth::Bits64)
                && field.ownership() == ArgumentOwnership::SharedBorrow
                && field.alias_class() == AliasClass::SharedReadOnly
        }
        AlphaZetaExpectedArgumentV1::DisjointF32Slice => {
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
    };
    if field.name().as_str() != name || field.offset() != offset || !matches {
        return Err(AlphaZetaCov6ProfileError::AbiFieldMismatch { index });
    }
    Ok(())
}

fn validate_alpha_zeta_cov6_launch(
    launch: &fe2o3_artifacts::LaunchContract,
) -> Result<(), AlphaZetaCov6ProfileError> {
    let exact = match launch.block_size() {
        BlockSize::Exact(exact) => exact,
        BlockSize::Any | BlockSize::AtMost(_) => {
            return Err(AlphaZetaCov6ProfileError::LaunchMismatch);
        }
    };
    let max_grid = launch.max_grid();
    if launch.rank() != 1
        || [exact.x(), exact.y(), exact.z()] != [ALPHA_ZETA_COV6_BLOCK_X, 1, 1]
        || [max_grid.x(), max_grid.y(), max_grid.z()] != [u32::MAX, 1, 1]
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(AlphaZetaCov6ProfileError::LaunchMismatch);
    }
    Ok(())
}

fn validate_pack_and_admit<'allocation>(
    plan: &GeneratedArgumentPackingPlanV1,
    binding: GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation>,
    observed: &ObservedContext,
) -> Result<
    (
        GeneratedPackedArgumentsV1<'allocation>,
        ArgumentAliasAdmission<'allocation>,
        InFlightRegionRegistration<'allocation>,
        usize,
    ),
    GeneratedAlphaZetaCov6ArgumentError,
> {
    let logical_length = validate_argument_binding(plan, &binding)?;
    let packed = plan
        .pack(binding.inputs)
        .map_err(GeneratedAlphaZetaCov6ArgumentError::Pack)?;
    if packed.kernel_id() != plan.kernel_id()
        || packed.len() != usize::try_from(plan.kernarg_size()).unwrap_or(usize::MAX)
        || packed.alignment() != plan.kernarg_alignment()
    {
        return Err(GeneratedAlphaZetaCov6ArgumentError::PackedSubstitution);
    }
    let (admission, registration) =
        admit_and_register(observed.alias_registry(), observed, binding.accesses)
            .map_err(GeneratedAlphaZetaCov6ArgumentError::Alias)?;
    Ok((packed, admission, registration, logical_length))
}

fn validate_argument_binding(
    plan: &GeneratedArgumentPackingPlanV1,
    binding: &GeneratedAlphaZetaCov6ArgumentBindingV1<'_>,
) -> Result<usize, GeneratedAlphaZetaCov6ArgumentError> {
    let mut slices = binding
        .inputs
        .iter()
        .filter_map(GeneratedArgumentInputV1::slice_description_v1)
        .collect::<Vec<_>>();
    slices.sort_unstable_by_key(|slice| slice.argument_index);
    if slices.is_empty() {
        return Err(GeneratedAlphaZetaCov6ArgumentError::MissingSliceDomain);
    }
    if slices.len() != binding.accesses.len() {
        return Err(GeneratedAlphaZetaCov6ArgumentError::AccessCount {
            slices: slices.len(),
            accesses: binding.accesses.len(),
        });
    }

    let first = usize::try_from(slices[0].length).map_err(|_| {
        GeneratedAlphaZetaCov6ArgumentError::LogicalLengthOverflow {
            argument_index: slices[0].argument_index,
            length: slices[0].length,
        }
    })?;
    if first == 0 {
        return Err(GeneratedAlphaZetaCov6ArgumentError::EmptySlice {
            argument_index: slices[0].argument_index,
        });
    }

    for (slice, access) in slices.iter().zip(&binding.accesses) {
        let length = usize::try_from(slice.length).map_err(|_| {
            GeneratedAlphaZetaCov6ArgumentError::LogicalLengthOverflow {
                argument_index: slice.argument_index,
                length: slice.length,
            }
        })?;
        if length == 0 {
            return Err(GeneratedAlphaZetaCov6ArgumentError::EmptySlice {
                argument_index: slice.argument_index,
            });
        }
        if length != first {
            return Err(GeneratedAlphaZetaCov6ArgumentError::LengthMismatch {
                expected_argument: slices[0].argument_index,
                expected: first,
                actual_argument: slice.argument_index,
                actual: length,
            });
        }
        let expected_mode = expected_access_mode(*slice)?;
        if !access.matches_generated_slice_v1(
            slice.address,
            slice.length,
            slice.element_size,
            expected_mode,
        ) {
            return Err(GeneratedAlphaZetaCov6ArgumentError::AccessSubstitution {
                argument_index: slice.argument_index,
            });
        }
        if plan.argument(slice.argument_index).is_none() {
            return Err(GeneratedAlphaZetaCov6ArgumentError::AccessSubstitution {
                argument_index: slice.argument_index,
            });
        }
    }
    Ok(first)
}

fn expected_access_mode(
    slice: GeneratedSliceInputDescriptionV1,
) -> Result<ArgumentAccessMode, GeneratedAlphaZetaCov6ArgumentError> {
    match slice.access {
        Access::ReadOnly => Ok(ArgumentAccessMode::SharedRead),
        Access::ReadWrite => Ok(ArgumentAccessMode::ExclusiveReadWrite),
        _ => Err(
            GeneratedAlphaZetaCov6ArgumentError::UnsupportedSliceAccess {
                argument_index: slice.argument_index,
                access: slice.access,
            },
        ),
    }
}

fn alpha_zeta_cov6_geometry(
    identity: &ArtifactKernelIdentityV1,
    physical: &crate::PublishedPhysicalLaunchLayoutV1,
    logical_length: usize,
) -> Result<HsaLaunchGeometryV1, GeneratedAlphaZetaCov6GeometryError> {
    let launch = identity.launch();
    let exact_block = match launch.block_size() {
        BlockSize::Exact(block) => block,
        BlockSize::Any | BlockSize::AtMost(_) => {
            return Err(GeneratedAlphaZetaCov6GeometryError::UnsupportedLaunchContract);
        }
    };
    if launch.rank() != 1
        || [exact_block.x(), exact_block.y(), exact_block.z()] != [ALPHA_ZETA_COV6_BLOCK_X, 1, 1]
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(GeneratedAlphaZetaCov6GeometryError::UnsupportedLaunchContract);
    }
    let grid_x = alpha_zeta_cov6_grid_x(logical_length)?;
    let geometry = HsaLaunchGeometryV1::new([grid_x, 1, 1], [ALPHA_ZETA_COV6_BLOCK_X, 1, 1], 0);
    validate_launch_geometry_contract(launch, physical, geometry)
        .map_err(GeneratedAlphaZetaCov6GeometryError::LaunchAuthorization)?;
    Ok(geometry)
}

fn alpha_zeta_cov6_grid_x(
    logical_length: usize,
) -> Result<u32, GeneratedAlphaZetaCov6GeometryError> {
    if logical_length == 0 {
        return Err(GeneratedAlphaZetaCov6GeometryError::UnsupportedLaunchContract);
    }
    let grid_x = u32::try_from(logical_length.div_ceil(ALPHA_ZETA_COV6_BLOCK_X as usize))
        .map_err(|_| GeneratedAlphaZetaCov6GeometryError::GridOverflow { logical_length })?;
    grid_x.checked_mul(ALPHA_ZETA_COV6_BLOCK_X).ok_or(
        GeneratedAlphaZetaCov6GeometryError::GlobalIndexDomainOverflow {
            logical_length,
            grid_x,
        },
    )?;
    Ok(grid_x)
}

fn prepare_physical_kernarg<P, K, A: ReviewedHsaImplicitKernargAdapterV1>(
    resolved: &ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>,
    plan: &GeneratedArgumentPackingPlanV1,
    packed: &GeneratedPackedArgumentsV1<'_>,
) -> Result<AlignedKernargStorageV1, GeneratedAlphaZetaCov6PhysicalKernargError> {
    let physical = resolved.physical_kernel().launch();
    prepare_physical_kernarg_parts(
        plan,
        packed,
        AlphaZetaCov6PhysicalKernargFactsV1 {
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
struct AlphaZetaCov6PhysicalKernargFactsV1 {
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
    facts: AlphaZetaCov6PhysicalKernargFactsV1,
) -> Result<AlignedKernargStorageV1, GeneratedAlphaZetaCov6PhysicalKernargError> {
    let explicit = usize::try_from(plan.kernarg_size())
        .map_err(|_| GeneratedAlphaZetaCov6PhysicalKernargError::ExplicitSizeOverflow)?;
    if !matches!(
        explicit,
        ALPHA_EXPLICIT_KERNARG_BYTES | ZETA_EXPLICIT_KERNARG_BYTES
    ) {
        return Err(
            GeneratedAlphaZetaCov6PhysicalKernargError::UnsupportedExplicitSize {
                actual: explicit,
            },
        );
    }
    if packed.len() != explicit || packed.alignment() != plan.kernarg_alignment() {
        return Err(GeneratedAlphaZetaCov6PhysicalKernargError::PackedSubstitution);
    }

    let total = explicit
        .checked_add(COV6_IMPLICIT_KERNARG_BYTES)
        .ok_or(GeneratedAlphaZetaCov6PhysicalKernargError::TotalSizeOverflow)?;
    if facts.physical_size != total as u64 || facts.resolved_size != total as u64 {
        return Err(
            GeneratedAlphaZetaCov6PhysicalKernargError::KernargSegmentSize {
                expected: total,
                physical: facts.physical_size,
                resolved: facts.resolved_size,
            },
        );
    }
    if facts.implicit_offset != PhysicalMetadataValueV1::Known(plan.kernarg_size())
        || facts.implicit_size != COV6_IMPLICIT_KERNARG_BYTES as u64
    {
        return Err(GeneratedAlphaZetaCov6PhysicalKernargError::ImplicitLayout);
    }
    if facts.physical_alignment != u64::from(plan.kernarg_alignment()) {
        return Err(
            GeneratedAlphaZetaCov6PhysicalKernargError::PhysicalAlignment {
                manifest: plan.kernarg_alignment(),
                physical: facts.physical_alignment,
            },
        );
    }
    let expected_hsa_alignment = facts.physical_alignment.max(HSA_MINIMUM_KERNARG_ALIGNMENT);
    if facts.resolved_alignment != expected_hsa_alignment {
        return Err(
            GeneratedAlphaZetaCov6PhysicalKernargError::ResolvedAlignment {
                expected: expected_hsa_alignment,
                actual: facts.resolved_alignment,
            },
        );
    }
    let alignment = usize::try_from(facts.resolved_alignment)
        .map_err(|_| GeneratedAlphaZetaCov6PhysicalKernargError::AlignmentOverflow)?;
    let mut storage = AlignedKernargStorageV1::new(total, alignment)?;
    storage.bytes_mut()[..explicit].copy_from_slice(packed.bytes());
    Ok(storage)
}

struct AlignedKernargStorageV1 {
    pointer: NonNull<u8>,
    layout: Layout,
}

impl AlignedKernargStorageV1 {
    fn new(
        byte_len: usize,
        alignment: usize,
    ) -> Result<Self, GeneratedAlphaZetaCov6PhysicalKernargError> {
        let layout = Layout::from_size_align(byte_len, alignment).map_err(|_| {
            GeneratedAlphaZetaCov6PhysicalKernargError::InvalidAllocationLayout {
                byte_len,
                alignment,
            }
        })?;
        // SAFETY: `layout` is nonzero and valid. The allocation is owned by
        // this value and released with the same layout in `Drop`.
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
        // SAFETY: the allocation is live, uniquely borrowed, and has exactly
        // `layout.size()` initialized bytes from `alloc_zeroed`.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

impl Drop for AlignedKernargStorageV1 {
    fn drop(&mut self) {
        // SAFETY: `pointer` was allocated with this exact layout and is owned
        // exclusively by this non-cloneable value.
        unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AlphaZetaCov6ProfileError {
    UnsupportedGeneratedProfile,
    GeneratedIdentitySubstitution,
    KernelIdentitySubstitution,
    KernelRoleSubstitution,
    AbiMismatch,
    AbiFieldMismatch { index: usize },
    LaunchMismatch,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedAlphaZetaCov6PrepareError<PrerequisiteError, AdapterError> {
    UnsupportedProfile,
    ContextDeviceMismatch,
    Selection(WorkerV2TypedKernelSelectionError),
    Authentication(WorkerV2ExecutableAuthenticationError<PrerequisiteError>),
    Resolution(HsaExecutableLoadError<AdapterError>),
    Profile(AlphaZetaCov6ProfileError),
    GeneratedLayout(GeneratedArgumentLayoutError),
    PackingPlan(GeneratedArgumentPackingError),
    Bind(GeneratedArgumentPackError),
    Arguments(GeneratedAlphaZetaCov6ArgumentError),
    Geometry(GeneratedAlphaZetaCov6GeometryError),
    PhysicalKernarg(GeneratedAlphaZetaCov6PhysicalKernargError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedAlphaZetaCov6ArgumentError {
    MissingSliceDomain,
    AccessCount {
        slices: usize,
        accesses: usize,
    },
    EmptySlice {
        argument_index: usize,
    },
    LogicalLengthOverflow {
        argument_index: usize,
        length: u64,
    },
    LengthMismatch {
        expected_argument: usize,
        expected: usize,
        actual_argument: usize,
        actual: usize,
    },
    UnsupportedSliceAccess {
        argument_index: usize,
        access: Access,
    },
    AccessSubstitution {
        argument_index: usize,
    },
    Pack(GeneratedArgumentPackError),
    PackedSubstitution,
    Alias(AliasAdmissionError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedAlphaZetaCov6GeometryError {
    UnsupportedLaunchContract,
    GridOverflow { logical_length: usize },
    GlobalIndexDomainOverflow { logical_length: usize, grid_x: u32 },
    LaunchAuthorization(HsaLaunchAuthorizationError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedAlphaZetaCov6PhysicalKernargError {
    ExplicitSizeOverflow,
    UnsupportedExplicitSize {
        actual: usize,
    },
    PackedSubstitution,
    TotalSizeOverflow,
    KernargSegmentSize {
        expected: usize,
        physical: u64,
        resolved: u64,
    },
    ImplicitLayout,
    PhysicalAlignment {
        manifest: u32,
        physical: u64,
    },
    ResolvedAlignment {
        expected: u64,
        actual: u64,
    },
    AlignmentOverflow,
    InvalidAllocationLayout {
        byte_len: usize,
        alignment: usize,
    },
}

impl<PrerequisiteError, AdapterError> fmt::Display
    for GeneratedAlphaZetaCov6PrepareError<PrerequisiteError, AdapterError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => {
                formatter.write_str("kernel is not the bounded alpha/zeta COV6 profile")
            }
            Self::ContextDeviceMismatch => {
                formatter.write_str("HIP context does not match the loaded HSA device")
            }
            Self::Selection(error) => write!(formatter, "typed kernel selection: {error}"),
            Self::Authentication(_) => {
                formatter.write_str("selected-kernel prerequisite authentication failed")
            }
            Self::Resolution(_) => formatter.write_str("selected HSA kernel resolution failed"),
            Self::Profile(error) => write!(formatter, "alpha/zeta COV6 profile: {error}"),
            Self::GeneratedLayout(error) => write!(formatter, "generated layout: {error}"),
            Self::PackingPlan(error) => write!(formatter, "generated packing plan: {error}"),
            Self::Bind(error) => write!(formatter, "generated argument binding: {error}"),
            Self::Arguments(error) => write!(formatter, "generated arguments: {error}"),
            Self::Geometry(error) => write!(formatter, "generated geometry: {error}"),
            Self::PhysicalKernarg(error) => write!(formatter, "physical kernarg: {error}"),
        }
    }
}

impl fmt::Display for GeneratedAlphaZetaCov6ArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl fmt::Display for AlphaZetaCov6ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl fmt::Display for GeneratedAlphaZetaCov6GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl fmt::Display for GeneratedAlphaZetaCov6PhysicalKernargError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl<PrerequisiteError: fmt::Debug, AdapterError: fmt::Debug> std::error::Error
    for GeneratedAlphaZetaCov6PrepareError<PrerequisiteError, AdapterError>
{
}

impl std::error::Error for GeneratedAlphaZetaCov6ArgumentError {}
impl std::error::Error for AlphaZetaCov6ProfileError {}
impl std::error::Error for GeneratedAlphaZetaCov6GeometryError {}
impl std::error::Error for GeneratedAlphaZetaCov6PhysicalKernargError {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::KernelId;
    use crate::argument_alias::{AllocationProvenance, generated_slice_argument_pair_for_test};
    use crate::generated_argument_plan::{GeneratedDeviceScalarV1, validate_argument_packing};
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, AddressSpace, AliasClass, ArgumentOwnership, Dimensions,
        LaunchContract, Mutability, Name, PointerWidth,
    };

    fn scalar<T: GeneratedDeviceScalarV1>(name: &str, offset: u64) -> AbiField {
        let size = T::RUST_SCALAR_TYPE.size_bytes();
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            size,
            u32::try_from(size).unwrap(),
            AbiKind::Scalar(T::ABI_SCALAR_TYPE),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
            T::scalar_type_identity_v1(PointerWidth::Bits64),
            ArgumentOwnership::ByValue,
            AliasClass::Value,
        )
        .unwrap()
    }

    fn slice<T: GeneratedDeviceScalarV1>(name: &str, offset: u64, read_write: bool) -> AbiField {
        let element_size = T::RUST_SCALAR_TYPE.size_bytes();
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            16,
            8,
            AbiKind::Slice {
                element_size,
                element_alignment: u32::try_from(element_size).unwrap(),
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
                T::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            } else {
                T::shared_slice_type_identity_v1(PointerWidth::Bits64)
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

    fn alpha_fields() -> Vec<AbiField> {
        vec![
            scalar::<f32>("scale", 0),
            slice::<f32>("input", 8, false),
            slice::<f32>("output", 24, true),
        ]
    }

    pub(crate) fn alpha_test_abi() -> AbiLayout {
        AbiLayout::new(
            ALPHA_EXPLICIT_KERNARG_BYTES as u64,
            8,
            PointerWidth::Bits64,
            alpha_fields(),
        )
        .unwrap()
    }

    fn alpha_plan(kernel: u8) -> GeneratedArgumentPackingPlanV1 {
        plan(kernel, 40, alpha_fields())
    }

    fn zeta_plan(kernel: u8) -> GeneratedArgumentPackingPlanV1 {
        let fields = vec![
            slice::<f32>("a", 0, false),
            slice::<f32>("b", 16, false),
            scalar::<f32>("bias", 32),
            slice::<f32>("output", 40, true),
        ];
        plan(kernel, 56, fields)
    }

    fn plan(kernel: u8, size: u64, fields: Vec<AbiField>) -> GeneratedArgumentPackingPlanV1 {
        let abi = AbiLayout::new(size, 8, PointerWidth::Bits64, fields.clone()).unwrap();
        let generated =
            CompilerGeneratedArgumentLayoutV1::new(size, 8, PointerWidth::Bits64, fields).unwrap();
        validate_argument_packing(KernelId::from_bytes([kernel; 32]), &abi, &generated).unwrap()
    }

    unsafe fn access<'allocation>(
        observed: &ObservedContext,
        owner: &'allocation (),
        address: usize,
        length: usize,
        mode: ArgumentAccessMode,
    ) -> ArgumentAccess<'allocation> {
        let byte_length = length.checked_mul(size_of::<f32>()).unwrap();
        // SAFETY: unit tests use inert addresses only for packing/admission and
        // never submit them to a runtime adapter.
        let allocation = unsafe {
            AllocationProvenance::from_raw_parts(observed, owner, address as *mut u8, byte_length)
                .unwrap()
        };
        ArgumentAccess::new(allocation.region(0, byte_length).unwrap(), mode)
    }

    fn alpha_binding<'allocation>(
        plan: &GeneratedArgumentPackingPlanV1,
        length: usize,
        input_address: usize,
        output_address: usize,
        accesses: Vec<ArgumentAccess<'allocation>>,
    ) -> GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation> {
        let scalar_inputs = vec![plan.scalar(0, 1.5_f32).unwrap()];
        let slice_inputs = vec![
            // SAFETY: inert test addresses correspond exactly to `accesses`.
            unsafe {
                plan.slice(
                    1,
                    input_address as *const (),
                    length as u64,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap()
            },
            // SAFETY: inert test addresses correspond exactly to `accesses`.
            unsafe {
                plan.slice(
                    2,
                    output_address as *const (),
                    length as u64,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadWrite,
                )
                .unwrap()
            },
        ];
        let slice_arguments = slice_inputs
            .into_iter()
            .zip(accesses)
            .map(|(input, access)| generated_slice_argument_pair_for_test(input, access))
            .collect();
        // SAFETY: this helper supplies every exact scalar and opaque test pair.
        unsafe {
            GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
                scalar_inputs,
                slice_arguments,
            )
        }
    }

    fn exact_physical_facts(explicit: u64) -> AlphaZetaCov6PhysicalKernargFactsV1 {
        AlphaZetaCov6PhysicalKernargFactsV1 {
            physical_size: explicit + COV6_IMPLICIT_KERNARG_BYTES as u64,
            physical_alignment: 8,
            implicit_offset: PhysicalMetadataValueV1::Known(explicit),
            implicit_size: COV6_IMPLICIT_KERNARG_BYTES as u64,
            resolved_size: explicit + COV6_IMPLICIT_KERNARG_BYTES as u64,
            resolved_alignment: 16,
        }
    }

    pub(crate) fn alpha_test_launch() -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(ALPHA_ZETA_COV6_BLOCK_X, 1, 1).unwrap()),
            Dimensions::new(u32::MAX, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    #[test]
    fn alpha_zeta_profile_accepts_only_exact_role_abi_and_launch_policy() {
        let alpha = alpha_test_abi();
        let zeta = AbiLayout::new(
            ZETA_EXPLICIT_KERNARG_BYTES as u64,
            8,
            PointerWidth::Bits64,
            vec![
                slice::<f32>("a", 0, false),
                slice::<f32>("b", 16, false),
                scalar::<f32>("bias", 32),
                slice::<f32>("output", 40, true),
            ],
        )
        .unwrap();
        assert_eq!(
            validate_alpha_zeta_cov6_abi(&alpha, AlphaZetaCov6KernelRoleV1::Alpha),
            Ok(())
        );
        assert_eq!(
            validate_alpha_zeta_cov6_abi(&zeta, AlphaZetaCov6KernelRoleV1::Zeta),
            Ok(())
        );
        assert_eq!(
            validate_alpha_zeta_cov6_abi(&alpha, AlphaZetaCov6KernelRoleV1::Zeta),
            Err(AlphaZetaCov6ProfileError::AbiMismatch)
        );
        assert_eq!(
            validate_alpha_zeta_cov6_abi(&zeta, AlphaZetaCov6KernelRoleV1::Alpha),
            Err(AlphaZetaCov6ProfileError::AbiMismatch)
        );

        let substituted_names = AbiLayout::new(
            ALPHA_EXPLICIT_KERNARG_BYTES as u64,
            8,
            PointerWidth::Bits64,
            vec![
                scalar::<f32>("arg0", 0),
                slice::<f32>("input", 8, false),
                slice::<f32>("output", 24, true),
            ],
        )
        .unwrap();
        assert_eq!(
            validate_alpha_zeta_cov6_abi(&substituted_names, AlphaZetaCov6KernelRoleV1::Alpha,),
            Err(AlphaZetaCov6ProfileError::AbiFieldMismatch { index: 0 })
        );

        assert_eq!(
            validate_alpha_zeta_cov6_launch(&alpha_test_launch()),
            Ok(())
        );
        let bounded_grid = LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(ALPHA_ZETA_COV6_BLOCK_X, 1, 1).unwrap()),
            Dimensions::new(u32::MAX - 1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            validate_alpha_zeta_cov6_launch(&bounded_grid),
            Err(AlphaZetaCov6ProfileError::LaunchMismatch)
        );
    }

    #[test]
    fn alpha_and_zeta_plans_produce_exact_variable_cov6_storage() {
        let observed = ObservedContext::for_test(0xa1, 0, "gfx942:xnack-", 1_024, 65_536);
        let input_owner = ();
        let output_owner = ();
        let alpha = alpha_plan(0xa1);
        let accesses = vec![
            // SAFETY: inert regions are never dispatched.
            unsafe {
                access(
                    &observed,
                    &input_owner,
                    0x1000,
                    257,
                    ArgumentAccessMode::SharedRead,
                )
            },
            // SAFETY: inert regions are never dispatched.
            unsafe {
                access(
                    &observed,
                    &output_owner,
                    0x2000,
                    257,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let binding = alpha_binding(&alpha, 257, 0x1000, 0x2000, accesses);
        let (packed, admission, registration, length) =
            validate_pack_and_admit(&alpha, binding, &observed).unwrap();
        assert_eq!(length, 257);
        assert_eq!(packed.len(), 40);
        let mut storage =
            prepare_physical_kernarg_parts(&alpha, &packed, exact_physical_facts(40)).unwrap();
        assert_eq!(storage.len(), 296);
        assert_eq!(storage.alignment(), 16);
        assert!(storage.bytes_mut().as_ptr().addr().is_multiple_of(16));
        assert_eq!(&storage.bytes_mut()[..40], packed.bytes());
        assert!(storage.bytes_mut()[40..].iter().all(|byte| *byte == 0));
        drop((admission, registration));

        let zeta = zeta_plan(0xb2);
        let slice_inputs = vec![
            // SAFETY: inert test addresses are validated against access records below.
            unsafe {
                zeta.slice(
                    0,
                    0x3000usize as *const (),
                    257,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap()
            },
            // SAFETY: as above.
            unsafe {
                zeta.slice(
                    1,
                    0x4000usize as *const (),
                    257,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap()
            },
            // SAFETY: as above.
            unsafe {
                zeta.slice(
                    3,
                    0x5000usize as *const (),
                    257,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadWrite,
                )
                .unwrap()
            },
        ];
        let a_owner = ();
        let b_owner = ();
        let c_owner = ();
        let accesses = vec![
            unsafe {
                access(
                    &observed,
                    &a_owner,
                    0x3000,
                    257,
                    ArgumentAccessMode::SharedRead,
                )
            },
            unsafe {
                access(
                    &observed,
                    &b_owner,
                    0x4000,
                    257,
                    ArgumentAccessMode::SharedRead,
                )
            },
            unsafe {
                access(
                    &observed,
                    &c_owner,
                    0x5000,
                    257,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let slice_arguments = slice_inputs
            .into_iter()
            .zip(accesses)
            .map(|(input, access)| generated_slice_argument_pair_for_test(input, access))
            .collect();
        let binding = unsafe {
            GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
                vec![zeta.scalar(2, 0.25_f32).unwrap()],
                slice_arguments,
            )
        };
        let (packed, _, _, _) = validate_pack_and_admit(&zeta, binding, &observed).unwrap();
        let storage =
            prepare_physical_kernarg_parts(&zeta, &packed, exact_physical_facts(56)).unwrap();
        assert_eq!(storage.len(), 312);
        assert_eq!(storage.alignment(), 16);
    }

    #[test]
    fn argument_binding_rejects_length_mismatch_and_region_substitution() {
        let observed = ObservedContext::for_test(0xa2, 0, "gfx942:xnack-", 1_024, 65_536);
        let input_owner = ();
        let output_owner = ();
        let plan = alpha_plan(0xa2);
        let scalar_inputs = vec![plan.scalar(0, 1.0_f32).unwrap()];
        let slice_inputs = vec![
            unsafe {
                plan.slice(
                    1,
                    0x1000usize as *const (),
                    7,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap()
            },
            unsafe {
                plan.slice(
                    2,
                    0x2000usize as *const (),
                    8,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadWrite,
                )
                .unwrap()
            },
        ];
        let accesses = vec![
            unsafe {
                access(
                    &observed,
                    &input_owner,
                    0x1000,
                    7,
                    ArgumentAccessMode::SharedRead,
                )
            },
            unsafe {
                access(
                    &observed,
                    &output_owner,
                    0x2000,
                    8,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let slice_arguments = slice_inputs
            .into_iter()
            .zip(accesses)
            .map(|(input, access)| generated_slice_argument_pair_for_test(input, access))
            .collect();
        let binding = unsafe {
            GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
                scalar_inputs,
                slice_arguments,
            )
        };
        assert!(matches!(
            validate_argument_binding(&plan, &binding),
            Err(GeneratedAlphaZetaCov6ArgumentError::LengthMismatch { .. })
        ));

        let accesses = vec![
            unsafe {
                access(
                    &observed,
                    &input_owner,
                    0x1004,
                    7,
                    ArgumentAccessMode::SharedRead,
                )
            },
            unsafe {
                access(
                    &observed,
                    &output_owner,
                    0x2000,
                    7,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let binding = alpha_binding(&plan, 7, 0x1000, 0x2000, accesses);
        assert!(matches!(
            validate_argument_binding(&plan, &binding),
            Err(GeneratedAlphaZetaCov6ArgumentError::AccessSubstitution { argument_index: 1 })
        ));
    }

    #[test]
    fn cross_kernel_inputs_and_conflicting_aliases_fail_closed() {
        let observed = ObservedContext::for_test(0xa3, 0, "gfx942:xnack-", 1_024, 65_536);
        let owner = ();
        let alpha = alpha_plan(0xa3);
        let zeta = alpha_plan(0xb3);
        let accesses = vec![
            unsafe { access(&observed, &owner, 0x1000, 4, ArgumentAccessMode::SharedRead) },
            unsafe {
                access(
                    &observed,
                    &owner,
                    0x2000,
                    4,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let binding = alpha_binding(&zeta, 4, 0x1000, 0x2000, accesses);
        assert!(matches!(
            validate_pack_and_admit(&alpha, binding, &observed),
            Err(GeneratedAlphaZetaCov6ArgumentError::Pack(_))
        ));

        let accesses = vec![
            unsafe { access(&observed, &owner, 0x3000, 4, ArgumentAccessMode::SharedRead) },
            unsafe {
                access(
                    &observed,
                    &owner,
                    0x3000,
                    4,
                    ArgumentAccessMode::ExclusiveReadWrite,
                )
            },
        ];
        let binding = alpha_binding(&alpha, 4, 0x3000, 0x3000, accesses);
        assert!(matches!(
            validate_pack_and_admit(&alpha, binding, &observed),
            Err(GeneratedAlphaZetaCov6ArgumentError::Alias(
                AliasAdmissionError::Conflict { .. }
            ))
        ));
    }

    #[test]
    fn tail_rounding_and_physical_rejections_are_checked() {
        for (length, grid) in [(1, 1), (255, 1), (256, 1), (257, 2), (1023, 4)] {
            assert_eq!(alpha_zeta_cov6_grid_x(length).unwrap(), grid);
        }
        assert!(alpha_zeta_cov6_grid_x(0).is_err());
        let maximum_rounded_domain = u32::MAX as usize - 255;
        assert_eq!(
            alpha_zeta_cov6_grid_x(maximum_rounded_domain).unwrap(),
            u32::MAX / ALPHA_ZETA_COV6_BLOCK_X
        );
        assert!(matches!(
            alpha_zeta_cov6_grid_x(maximum_rounded_domain + 1),
            Err(GeneratedAlphaZetaCov6GeometryError::GlobalIndexDomainOverflow { .. })
        ));

        let alpha = alpha_plan(0xa4);
        let packed = alpha
            .pack([
                alpha.scalar(0, 1.0_f32).unwrap(),
                unsafe {
                    alpha
                        .slice(
                            1,
                            0x1000usize as *const (),
                            1,
                            PointerWidth::Bits64,
                            AddressSpace::Global,
                            Access::ReadOnly,
                        )
                        .unwrap()
                },
                unsafe {
                    alpha
                        .slice(
                            2,
                            0x2000usize as *const (),
                            1,
                            PointerWidth::Bits64,
                            AddressSpace::Global,
                            Access::ReadWrite,
                        )
                        .unwrap()
                },
            ])
            .unwrap();
        let mut wrong_size = exact_physical_facts(40);
        wrong_size.resolved_size -= 1;
        assert!(matches!(
            prepare_physical_kernarg_parts(&alpha, &packed, wrong_size),
            Err(GeneratedAlphaZetaCov6PhysicalKernargError::KernargSegmentSize { .. })
        ));
        let mut wrong_alignment = exact_physical_facts(40);
        wrong_alignment.resolved_alignment = 32;
        assert!(matches!(
            prepare_physical_kernarg_parts(&alpha, &packed, wrong_alignment),
            Err(GeneratedAlphaZetaCov6PhysicalKernargError::ResolvedAlignment { .. })
        ));

        let unsupported = plan(0xc4, 48, vec![slice::<f32>("arg0", 0, false)]);
        let packed = unsupported
            .pack([unsafe {
                unsupported
                    .slice(
                        0,
                        0x1000usize as *const (),
                        1,
                        PointerWidth::Bits64,
                        AddressSpace::Global,
                        Access::ReadOnly,
                    )
                    .unwrap()
            }])
            .unwrap();
        assert!(matches!(
            prepare_physical_kernarg_parts(&unsupported, &packed, exact_physical_facts(48)),
            Err(GeneratedAlphaZetaCov6PhysicalKernargError::UnsupportedExplicitSize { actual: 48 })
        ));
    }
}
