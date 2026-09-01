use crate::argument_alias::{InFlightRegionRegistration, admit_and_register};
use crate::generated_argument_plan::{
    GeneratedArgumentInputV1, GeneratedPackedArgumentsV1, GeneratedSliceInputDescriptionV1,
};
use crate::{
    AliasAdmissionError, ArgumentAccess, ArgumentAccessMode, ArgumentAliasAdmission,
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedPackingComponentKindV1, GeneratedSliceArgumentPairV1,
    HsaCompletedWorkerV3DispatchV1, HsaLaunchAuthorizationError, HsaLaunchGeometryV1,
    LoadedWorkerV3HsaExecutableV1, ObservedContext, RecoveredWorkerV3AdmissionErrorV1,
    ReviewedHsaImplicitKernargAdapterV1, WorkerV3GeneratedDispatchErrorV1,
};
use fe2o3_artifacts::{AbiKind, Access, ArgumentOwnership};
use fe2o3_runtime_model::{
    FormalRuntimePreparationPhaseV1, FormalRuntimeResourceOwnerV1, FormalVecaddAbiComponentKindV1,
    FormalVecaddAbiComponentV1, FormalVecaddArgumentOwnershipV1, FormalVecaddEffectV1,
    FormalVecaddGeometryV1, FormalVecaddResourceV1, FormalVecaddRuntimeInputV1,
    FormalVecaddRuntimePreparationErrorV1, FormalVecaddRuntimePreparationEvidenceV1,
    admit_formal_vecadd_runtime_preparation_v1,
};
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::fmt;
use std::ptr::NonNull;

const HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;

/// Opaque binding from one compiler-generated Worker V3 `Arguments` value to an exact packing
/// plan and its retained allocation effects.
#[doc(hidden)]
pub struct GeneratedWorkerV3ArgumentBindingV1<'allocation> {
    inputs: Vec<GeneratedArgumentInputV1<'allocation>>,
    accesses: Vec<ArgumentAccess<'allocation>>,
}

impl fmt::Debug for GeneratedWorkerV3ArgumentBindingV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkerV3ArgumentBindingV1")
            .field("input_count", &self.inputs.len())
            .field("access_count", &self.accesses.len())
            .finish_non_exhaustive()
    }
}

impl<'allocation> GeneratedWorkerV3ArgumentBindingV1<'allocation> {
    /// Joins compiler-bound scalar inputs and capability-bound memory arguments.
    ///
    /// Each opaque memory pair carries an input and access record emitted by the same retained
    /// capability. The packing plan later rejects omitted, duplicate, and cross-plan inputs.
    #[doc(hidden)]
    pub fn from_compiler_generated_parts_v1(
        scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
        memory_arguments: Vec<GeneratedSliceArgumentPairV1<'allocation>>,
    ) -> Self {
        let mut inputs = Vec::with_capacity(scalar_inputs.len() + memory_arguments.len());
        inputs.extend(scalar_inputs);
        let mut accesses = Vec::with_capacity(memory_arguments.len());
        for argument in memory_arguments {
            let (input, access) = argument.into_parts();
            inputs.push(input);
            accesses.push(access);
        }
        Self { inputs, accesses }
    }
}

/// Compiler-generated bridge for one exact Worker V3 kernel signature.
///
/// # Safety
///
/// An implementation must be emitted from the same compiler-authenticated Rust signature and
/// host contract as `K`. It must bind every source argument exactly once and retain all referenced
/// allocations in `self` through synchronous completion. A false implementation can authorize
/// native GPU access under the wrong Rust ownership contract.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedWorkerV3ArgumentsV1<
    'allocation,
    K: CompilerGeneratedKernelExpectationV1,
>
{
    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedWorkerV3ArgumentBindingV1<'allocation>, GeneratedArgumentPackError>;
}

/// Linear, one-shot invocation prepared from an exact verified Worker V3 executable.
#[must_use = "a prepared Worker V3 invocation does no work until dispatched"]
#[doc(hidden)]
pub struct GeneratedWorkerV3PreparedInvocationV1<
    'loaded,
    'allocation,
    K,
    A: ReviewedHsaImplicitKernargAdapterV1,
    Arguments,
> {
    loaded: &'loaded mut LoadedWorkerV3HsaExecutableV1<K, A>,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: usize,
    implicit_byte_len: usize,
    kernarg: WorkerV3AlignedKernargV1,
    arguments: Arguments,
    admission: ArgumentAliasAdmission<'allocation>,
    registration: InFlightRegionRegistration<'allocation>,
    runtime_refinement: Option<FormalVecaddRuntimePreparationEvidenceV1>,
}

impl<K, A, Arguments> GeneratedWorkerV3PreparedInvocationV1<'_, '_, K, A, Arguments>
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
    }

    pub const fn explicit_byte_len(&self) -> usize {
        self.explicit_byte_len
    }

    pub const fn implicit_byte_len(&self) -> usize {
        self.implicit_byte_len
    }

    pub fn physical_kernarg_byte_len(&self) -> usize {
        self.kernarg.len()
    }

    pub fn physical_kernarg_alignment(&self) -> usize {
        self.kernarg.alignment()
    }

    /// Mechanically checked preparation evidence for the exact formal vecadd profile.
    ///
    /// Other generated kernels return `None`; the absence never weakens their existing
    /// admission checks or implies a formal runtime claim.
    pub const fn formal_vecadd_runtime_refinement_v1(
        &self,
    ) -> Option<&FormalVecaddRuntimePreparationEvidenceV1> {
        self.runtime_refinement.as_ref()
    }

    /// Initializes the complete implicit suffix and synchronously dispatches exactly once.
    pub fn dispatch(
        self,
    ) -> Result<HsaCompletedWorkerV3DispatchV1<K>, WorkerV3GeneratedDispatchErrorV1<A::Error>> {
        let Self {
            loaded,
            geometry,
            explicit_byte_len,
            implicit_byte_len,
            mut kernarg,
            arguments,
            admission,
            registration,
            runtime_refinement,
        } = self;
        let retained = (&arguments, &admission, &registration, &runtime_refinement);
        // SAFETY: preparation matched the generated ABI to the independently admitted V3
        // descriptor, checked every capability/input pair, admitted all aliases, and allocated
        // exact aligned physical storage. The reviewed adapter is synchronous.
        let completed = unsafe {
            loaded.dispatch_generated_and_wait(
                geometry,
                kernarg.bytes_mut(),
                explicit_byte_len,
                explicit_byte_len,
                implicit_byte_len,
            )
        }?;
        let _ = retained;
        Ok(completed)
    }
}

impl<K, A> LoadedWorkerV3HsaExecutableV1<K, A>
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    /// Prepares one exact generated Worker V3 invocation without exposing raw handles or bytes.
    #[doc(hidden)]
    pub fn prepare_generated_worker_v3_v1<'loaded, 'allocation, Arguments>(
        &'loaded mut self,
        observed: &ObservedContext,
        geometry: HsaLaunchGeometryV1,
        arguments: Arguments,
    ) -> Result<
        GeneratedWorkerV3PreparedInvocationV1<'loaded, 'allocation, K, A, Arguments>,
        GeneratedWorkerV3PrepareErrorV1,
    >
    where
        Arguments: CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K>,
    {
        if !self.matches_observed_context(observed) {
            return Err(GeneratedWorkerV3PrepareErrorV1::ContextMismatch);
        }
        self.revalidate_currentness()
            .map_err(GeneratedWorkerV3PrepareErrorV1::CurrentPublication)?;
        self.validate_worker_v3_launch_geometry(geometry)
            .map_err(GeneratedWorkerV3PrepareErrorV1::LaunchAuthorization)?;

        let generated = Arguments::generated_argument_layout_v1()
            .map_err(GeneratedWorkerV3PrepareErrorV1::GeneratedLayout)?;
        // SAFETY: the unsafe generated trait implementation supplies an independent compiler
        // layout for the exact marker already authenticated by the Worker V3 verifier.
        let plan = unsafe { self.validate_worker_v3_argument_packing(&generated) }
            .map_err(GeneratedWorkerV3PrepareErrorV1::PackingPlan)?;
        let binding = arguments
            .bind_arguments_v1(&plan)
            .map_err(GeneratedWorkerV3PrepareErrorV1::Bind)?;
        validate_memory_pairs(&binding).map_err(GeneratedWorkerV3PrepareErrorV1::Arguments)?;
        let formal_runtime_input =
            project_formal_vecadd_runtime_input_v1(self, &plan, &binding, geometry);
        let packed = plan
            .pack(binding.inputs)
            .map_err(GeneratedWorkerV3PrepareErrorV1::Bind)?;
        if packed.kernel_id() != self.descriptor().kernel_id()
            || packed.len() != usize::try_from(plan.kernarg_size()).unwrap_or(usize::MAX)
            || packed.alignment() != plan.kernarg_alignment()
        {
            return Err(GeneratedWorkerV3PrepareErrorV1::PackedSubstitution);
        }
        let (admission, registration) =
            admit_and_register(observed.alias_registry(), observed, binding.accesses)
                .map_err(GeneratedWorkerV3PrepareErrorV1::Alias)?;
        let (kernarg, implicit_byte_len) = prepare_physical_kernarg(self, &plan, &packed)?;
        let runtime_refinement = formal_runtime_input
            .map(admit_formal_vecadd_runtime_preparation_v1)
            .transpose()
            .map_err(GeneratedWorkerV3PrepareErrorV1::FormalRuntimeRefinement)?;

        Ok(GeneratedWorkerV3PreparedInvocationV1 {
            loaded: self,
            geometry,
            explicit_byte_len: packed.len(),
            implicit_byte_len,
            kernarg,
            arguments,
            admission,
            registration,
            runtime_refinement,
        })
    }
}

fn project_formal_vecadd_runtime_input_v1<K, A>(
    loaded: &LoadedWorkerV3HsaExecutableV1<K, A>,
    plan: &GeneratedArgumentPackingPlanV1,
    binding: &GeneratedWorkerV3ArgumentBindingV1<'_>,
    geometry: HsaLaunchGeometryV1,
) -> Option<FormalVecaddRuntimeInputV1>
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    if loaded.descriptor().logical_name().as_str() != "vecadd"
        || plan.argument_count() != 3
        || plan.component_count() != 6
        || binding.accesses.len() != 3
    {
        return None;
    }
    for index in 0..3 {
        let field = plan.argument(index)?;
        if !matches!(
            field.kind(),
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4
            }
        ) {
            return None;
        }
    }

    let components = core::array::from_fn(|index| {
        let component = plan
            .component(index)
            .expect("the exact component count was checked");
        let field = plan
            .argument(component.argument_index())
            .expect("a validated packing component names one argument");
        FormalVecaddAbiComponentV1 {
            argument_index: u8::try_from(component.argument_index()).unwrap_or(u8::MAX),
            kind: match component.kind() {
                GeneratedPackingComponentKindV1::SlicePointer => {
                    FormalVecaddAbiComponentKindV1::SlicePointer
                }
                GeneratedPackingComponentKindV1::SliceLength => {
                    FormalVecaddAbiComponentKindV1::SliceLength
                }
                _ => FormalVecaddAbiComponentKindV1::Unsupported,
            },
            offset: component.offset(),
            size: component.size(),
            alignment: component.alignment(),
            effect: formal_effect_v1(field.access()),
            argument_ownership: match field.ownership() {
                ArgumentOwnership::SharedBorrow => FormalVecaddArgumentOwnershipV1::SharedBorrow,
                ArgumentOwnership::UniqueBorrow => FormalVecaddArgumentOwnershipV1::UniqueBorrow,
                _ => FormalVecaddArgumentOwnershipV1::Unsupported,
            },
        }
    });

    let mut slices = binding
        .inputs
        .iter()
        .filter_map(GeneratedArgumentInputV1::slice_description_v1)
        .collect::<Vec<_>>();
    slices.sort_unstable_by_key(|slice| slice.argument_index);
    let resources: [FormalVecaddResourceV1; 3] = slices
        .into_iter()
        .zip(&binding.accesses)
        .map(|(slice, access)| {
            let region = access.formal_runtime_region_v1();
            FormalVecaddResourceV1 {
                argument_index: u8::try_from(slice.argument_index).unwrap_or(u8::MAX),
                allocation_context: region.map_or(0, |value| value.allocation_context),
                allocation_identity: region.map_or(0, |value| value.allocation_identity),
                allocation_base: region.map_or(0, |value| value.allocation_base),
                byte_offset: region.map_or(0, |value| value.byte_offset),
                byte_len: region.map_or(0, |value| value.byte_len),
                encoded_address: slice.address,
                element_count: slice.length,
                effect: formal_effect_v1(slice.access),
                owner: FormalRuntimeResourceOwnerV1::Caller,
            }
        })
        .collect::<Vec<_>>()
        .try_into()
        .ok()?;

    let descriptor = loaded.descriptor();
    let descriptor_abi = descriptor.abi_layout();
    let source = descriptor.launch();
    let source_required_workgroup = match source.block_size() {
        crate::BlockSizeV1::Exact(value) => [value.x(), value.y(), value.z()],
        crate::BlockSizeV1::Any | crate::BlockSizeV1::AtMost(_) => [0, 0, 0],
    };
    let max_grid = source.max_grid();
    let physical = loaded.physical_kernel();
    let resolution = loaded.kernel_observation();
    let verification = loaded.authenticated_verification_v1();
    Some(FormalVecaddRuntimeInputV1 {
        kernel_identity: *descriptor.kernel_id().as_bytes(),
        generated_host_contract_identity: verification.generated_host_contract_identity(),
        rust_layout_contract_identity: verification.rust_type_layout_contract_sha256(),
        rust_effect_contract_identity: verification.rust_effect_contract_sha256(),
        explicit_byte_len: u64::from(descriptor_abi.explicit_argument_size()),
        implicit_byte_offset: physical.implicit_argument_offset().unwrap_or(u64::MAX),
        implicit_byte_len: physical.implicit_argument_size(),
        physical_byte_len: physical.kernarg_segment_size(),
        descriptor_alignment: descriptor_abi.kernarg_segment_alignment(),
        runtime_alignment: u32::try_from(resolution.kernarg_segment_alignment())
            .unwrap_or(u32::MAX),
        components,
        geometry: FormalVecaddGeometryV1 {
            grid: geometry.grid(),
            workgroup: geometry.workgroup(),
            dynamic_group_bytes: geometry.dynamic_shared_memory_bytes(),
            source_max_grid: [max_grid.x(), max_grid.y(), max_grid.z()],
            physical_max_grid: physical
                .max_workgroups()
                .map(|maximum| maximum.unwrap_or(u32::MAX)),
            source_max_flat_workgroup: source.max_flat_workgroup_size(),
            physical_max_flat_workgroup: physical.max_flat_workgroup_size(),
            source_required_workgroup,
            physical_required_workgroup: physical.required_workgroup_size(),
            source_static_group_bytes: source.static_shared_memory_bytes(),
            physical_static_group_bytes: physical.group_segment_fixed_size(),
            physical_private_segment_bytes: physical.private_segment_fixed_size(),
        },
        resources,
        source_phase: FormalRuntimePreparationPhaseV1::Loaded,
    })
}

fn formal_effect_v1(access: Access) -> FormalVecaddEffectV1 {
    match access {
        Access::ReadOnly => FormalVecaddEffectV1::SharedRead,
        Access::WriteOnly => FormalVecaddEffectV1::ExclusiveWrite,
        Access::ByValue | Access::ReadWrite => FormalVecaddEffectV1::Unsupported,
    }
}

fn validate_memory_pairs(
    binding: &GeneratedWorkerV3ArgumentBindingV1<'_>,
) -> Result<(), GeneratedWorkerV3ArgumentErrorV1> {
    let mut slices = binding
        .inputs
        .iter()
        .filter_map(GeneratedArgumentInputV1::slice_description_v1)
        .collect::<Vec<_>>();
    slices.sort_unstable_by_key(|slice| slice.argument_index);
    if slices.len() != binding.accesses.len() {
        return Err(GeneratedWorkerV3ArgumentErrorV1::AccessCount {
            memory_arguments: slices.len(),
            accesses: binding.accesses.len(),
        });
    }
    for (slice, access) in slices.into_iter().zip(&binding.accesses) {
        let expected = expected_access_mode(slice)?;
        if !access.matches_generated_slice_v1(
            slice.address,
            slice.length,
            slice.element_size,
            expected,
        ) {
            return Err(GeneratedWorkerV3ArgumentErrorV1::AccessSubstitution {
                argument_index: slice.argument_index,
            });
        }
    }
    Ok(())
}

fn expected_access_mode(
    slice: GeneratedSliceInputDescriptionV1,
) -> Result<ArgumentAccessMode, GeneratedWorkerV3ArgumentErrorV1> {
    match slice.access {
        Access::ReadOnly => Ok(ArgumentAccessMode::SharedRead),
        Access::WriteOnly => Ok(ArgumentAccessMode::ExclusiveWrite),
        Access::ReadWrite => Ok(ArgumentAccessMode::ExclusiveReadWrite),
        access => Err(GeneratedWorkerV3ArgumentErrorV1::UnsupportedMemoryAccess {
            argument_index: slice.argument_index,
            access,
        }),
    }
}

fn prepare_physical_kernarg<K, A>(
    loaded: &LoadedWorkerV3HsaExecutableV1<K, A>,
    plan: &GeneratedArgumentPackingPlanV1,
    packed: &GeneratedPackedArgumentsV1<'_>,
) -> Result<(WorkerV3AlignedKernargV1, usize), GeneratedWorkerV3PrepareErrorV1>
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let descriptor = loaded.descriptor().abi_layout();
    let physical = loaded.physical_kernel();
    let resolution = loaded.kernel_observation();
    let explicit = usize::try_from(descriptor.explicit_argument_size())
        .map_err(|_| GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg)?;
    let total = usize::try_from(descriptor.kernarg_segment_size())
        .map_err(|_| GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg)?;
    let implicit = total
        .checked_sub(explicit)
        .ok_or(GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg)?;
    if !physical_implicit_kernarg_metadata_matches(
        explicit,
        implicit,
        physical.implicit_argument_offset(),
        physical.implicit_argument_size(),
    ) || packed.len() != explicit
        || packed.alignment() != plan.kernarg_alignment()
        || plan.kernarg_size() != u64::from(descriptor.explicit_argument_size())
        || plan.kernarg_alignment() != descriptor.kernarg_segment_alignment()
        || physical.kernarg_segment_size() != total as u64
        || physical.kernarg_segment_alignment() != u64::from(descriptor.kernarg_segment_alignment())
        || resolution.kernarg_segment_size() != total as u64
    {
        return Err(GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg);
    }
    let expected_runtime_alignment = physical
        .kernarg_segment_alignment()
        .max(HSA_MINIMUM_KERNARG_ALIGNMENT);
    if resolution.kernarg_segment_alignment() != expected_runtime_alignment {
        return Err(GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg);
    }
    let alignment = usize::try_from(expected_runtime_alignment)
        .map_err(|_| GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg)?;
    let mut storage = WorkerV3AlignedKernargV1::new(total, alignment)
        .map_err(|_| GeneratedWorkerV3PrepareErrorV1::PhysicalKernarg)?;
    storage.bytes_mut()[..explicit].copy_from_slice(packed.bytes());
    Ok((storage, implicit))
}

fn physical_implicit_kernarg_metadata_matches(
    explicit_byte_len: usize,
    implicit_byte_len: usize,
    physical_offset: Option<u64>,
    physical_size: u64,
) -> bool {
    match implicit_byte_len {
        0 => physical_offset.is_none() && physical_size == 0,
        COV6_IMPLICIT_KERNARG_BYTES => {
            u64::try_from(explicit_byte_len).is_ok_and(|explicit| physical_offset == Some(explicit))
                && physical_size == COV6_IMPLICIT_KERNARG_BYTES as u64
        }
        _ => false,
    }
}

struct WorkerV3AlignedKernargV1 {
    pointer: NonNull<u8>,
    layout: Layout,
}

impl WorkerV3AlignedKernargV1 {
    fn new(byte_len: usize, alignment: usize) -> Result<Self, ()> {
        let layout = Layout::from_size_align(byte_len, alignment).map_err(|_| ())?;
        // SAFETY: the validated nonzero layout is retained for exact deallocation in `Drop`.
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
        // SAFETY: this value uniquely owns exactly `layout.size()` initialized bytes.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

impl Drop for WorkerV3AlignedKernargV1 {
    fn drop(&mut self) {
        // SAFETY: `pointer` was allocated with this exact retained layout.
        unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3PrepareErrorV1 {
    ContextMismatch,
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    LaunchAuthorization(HsaLaunchAuthorizationError),
    GeneratedLayout(GeneratedArgumentLayoutError),
    PackingPlan(GeneratedArgumentPackingError),
    Bind(GeneratedArgumentPackError),
    Arguments(GeneratedWorkerV3ArgumentErrorV1),
    PackedSubstitution,
    Alias(AliasAdmissionError),
    PhysicalKernarg,
    FormalRuntimeRefinement(FormalVecaddRuntimePreparationErrorV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedWorkerV3ArgumentErrorV1 {
    AccessCount {
        memory_arguments: usize,
        accesses: usize,
    },
    UnsupportedMemoryAccess {
        argument_index: usize,
        access: Access,
    },
    AccessSubstitution {
        argument_index: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_kernarg_admits_only_explicit_only_or_exact_cov6_hidden_metadata() {
        assert!(physical_implicit_kernarg_metadata_matches(48, 0, None, 0));
        assert!(physical_implicit_kernarg_metadata_matches(
            48,
            COV6_IMPLICIT_KERNARG_BYTES,
            Some(48),
            COV6_IMPLICIT_KERNARG_BYTES as u64,
        ));

        for (implicit, offset, size) in [
            (0, Some(48), 0),
            (0, None, COV6_IMPLICIT_KERNARG_BYTES as u64),
            (1, None, 0),
            (255, Some(48), 255),
            (257, Some(48), 257),
            (COV6_IMPLICIT_KERNARG_BYTES, None, 256),
            (COV6_IMPLICIT_KERNARG_BYTES, Some(47), 256),
            (COV6_IMPLICIT_KERNARG_BYTES, Some(48), 0),
        ] {
            assert!(!physical_implicit_kernarg_metadata_matches(
                48, implicit, offset, size,
            ));
        }
    }
}
