use crate::argument_alias::{InFlightRegionRegistration, admit_and_register};
use crate::generated_argument_plan::{
    GeneratedArgumentInputV1, GeneratedPackedArgumentsV1, GeneratedSliceInputDescriptionV1,
};
use crate::{
    AliasAdmissionError, ArgumentAccess, ArgumentAccessMode, ArgumentAliasAdmission,
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedSliceArgumentPairV1, HsaCompletedWorkerV3DispatchV1,
    HsaLaunchAuthorizationError, HsaLaunchGeometryV1, LoadedWorkerV3HsaExecutableV1,
    ObservedContext, RecoveredWorkerV3AdmissionErrorV1, ReviewedHsaImplicitKernargAdapterV1,
    WorkerV3GeneratedDispatchErrorV1,
};
use fe2o3_artifacts::Access;
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
        } = self;
        let retained = (&arguments, &admission, &registration);
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

        Ok(GeneratedWorkerV3PreparedInvocationV1 {
            loaded: self,
            geometry,
            explicit_byte_len: packed.len(),
            implicit_byte_len,
            kernarg,
            arguments,
            admission,
            registration,
        })
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
    if implicit != COV6_IMPLICIT_KERNARG_BYTES
        || packed.len() != explicit
        || packed.alignment() != plan.kernarg_alignment()
        || plan.kernarg_size() != u64::from(descriptor.explicit_argument_size())
        || plan.kernarg_alignment() != descriptor.kernarg_segment_alignment()
        || physical.kernarg_segment_size() != total as u64
        || physical.kernarg_segment_alignment() != u64::from(descriptor.kernarg_segment_alignment())
        || physical.implicit_argument_offset() != Some(explicit as u64)
        || physical.implicit_argument_size() != implicit as u64
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
