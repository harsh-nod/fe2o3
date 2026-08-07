use crate::argument_alias::{InFlightRegionRegistration, admit_and_register};
use crate::artifact_binding::validate_generated_profile;
use crate::generated_argument_plan::{
    GeneratedArgumentPackError, GeneratedPackedArgumentsV1, validate_argument_packing,
};
use crate::generated_vecadd::{
    checked_vecadd_grid, generated_vecadd_argument_layout_v2, validate_vecadd_profile,
};
use crate::{
    AliasAdmissionError, ArgumentAliasAdmission, CompilerGeneratedKernelContractV1,
    CompilerGeneratedKernelProfileV1, GeneratedArgumentPackingError, GeneratedKernelProfileError,
    GeneratedReadDeviceSlice, GeneratedVecAddPrepareError, GeneratedVecAddProfileError,
    GeneratedWriteDeviceSlice, HsaCompletedDispatchV1, HsaExecutableUnloadError,
    HsaGeneratedDispatchError, HsaLaunchAuthorizationError, HsaLaunchGeometryV1,
    LoadedHsaExecutableV1, ObservedContext, PhysicalMetadataValueV1, RegionError,
    ReviewedHsaImplicitKernargAdapterV1, UnloadedHsaExecutableV1,
};
use fe2o3_artifacts::{Access, AddressSpace, PointerWidth};
use fe2o3_core::{
    DeviceBuffer, DeviceBufferView, DeviceBufferViewMut, Error as CoreError, GpuContext,
};
use std::fmt;
use std::sync::Arc;

const TARGET: &str = "gfx942";
const BLOCK_SIZE: u32 = 256;
const EXPLICIT_KERNARG_BYTES: usize = 48;
const IMPLICIT_KERNARG_BYTES: usize = 256;
const TOTAL_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + IMPLICIT_KERNARG_BYTES;
const PHYSICAL_KERNARG_ALIGNMENT: u64 = 8;

type VecAddResources<'allocation> = (
    GeneratedReadDeviceSlice<'allocation, f32>,
    GeneratedReadDeviceSlice<'allocation, f32>,
    GeneratedWriteDeviceSlice<'allocation, f32>,
);

#[repr(C, align(16))]
struct AlignedVecAddKernargV1 {
    bytes: [u8; TOTAL_KERNARG_BYTES],
}

/// Typed Worker V2 executor for the exact gfx942/COV6 `f32` vecadd profile.
///
/// Construction consumes an authenticated, loaded HSA executable and pins the
/// matching HIP context used to derive allocation provenance. This value is
/// intentionally neither `Clone` nor `Copy`.
#[doc(hidden)]
pub struct GeneratedWorkerV2VecAddExecutorV1<
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
> {
    loaded: LoadedHsaExecutableV1<K, A>,
    observed: ObservedContext,
    packing: crate::GeneratedArgumentPackingPlanV1,
}

impl<K, A> fmt::Debug for GeneratedWorkerV2VecAddExecutorV1<K, A>
where
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkerV2VecAddExecutorV1")
            .field("artifact_identity", self.loaded.artifact_identity())
            .field("device", self.observed.device())
            .finish_non_exhaustive()
    }
}

impl<K, A> GeneratedWorkerV2VecAddExecutorV1<K, A>
where
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    /// Binds the authenticated HSA executable to the exact generated profile
    /// and the HIP context that owns all later buffers.
    pub fn bind(
        loaded: LoadedHsaExecutableV1<K, A>,
        context: &Arc<GpuContext>,
    ) -> Result<Self, GeneratedWorkerV2VecAddBindError> {
        let observed =
            ObservedContext::observe(context).map_err(GeneratedWorkerV2VecAddBindError::Observe)?;
        Self::bind_observed(loaded, observed)
    }

    #[cfg(test)]
    pub(crate) fn bind_observed_for_test(
        loaded: LoadedHsaExecutableV1<K, A>,
        observed: ObservedContext,
    ) -> Result<Self, GeneratedWorkerV2VecAddBindError> {
        Self::bind_observed(loaded, observed)
    }

    fn bind_observed(
        loaded: LoadedHsaExecutableV1<K, A>,
        observed: ObservedContext,
    ) -> Result<Self, GeneratedWorkerV2VecAddBindError> {
        if K::PROFILE != CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2 {
            return Err(GeneratedWorkerV2VecAddBindError::UnsupportedGeneratedProfile);
        }
        validate_generated_profile(
            K::PROFILE,
            K::KERNEL_BINDING_ID_V1,
            loaded.artifact_identity(),
        )
        .map_err(GeneratedWorkerV2VecAddBindError::GeneratedProfile)?;
        validate_vecadd_profile(loaded.artifact_identity())
            .map_err(GeneratedWorkerV2VecAddBindError::VecAddProfile)?;

        let physical_device = loaded.environment().physical_device();
        if observed.device().ordinal() != physical_device.hip_ordinal()
            || observed.device().target_id() != physical_device.target()
            || observed.device().target_id().processor() != TARGET
        {
            return Err(GeneratedWorkerV2VecAddBindError::ContextDeviceMismatch);
        }

        let physical = loaded.physical_kernel().launch();
        if physical.kernarg_segment_size() != TOTAL_KERNARG_BYTES as u64 {
            return Err(GeneratedWorkerV2VecAddBindError::KernargSegmentSize {
                actual: physical.kernarg_segment_size(),
            });
        }
        if physical.kernarg_segment_alignment() != PHYSICAL_KERNARG_ALIGNMENT {
            return Err(GeneratedWorkerV2VecAddBindError::KernargSegmentAlignment {
                actual: physical.kernarg_segment_alignment(),
            });
        }
        if physical.implicit_argument_offset()
            != PhysicalMetadataValueV1::Known(EXPLICIT_KERNARG_BYTES as u64)
        {
            return Err(GeneratedWorkerV2VecAddBindError::ImplicitArgumentOffset {
                actual: physical.implicit_argument_offset(),
            });
        }
        if physical.implicit_argument_size() != IMPLICIT_KERNARG_BYTES as u64 {
            return Err(GeneratedWorkerV2VecAddBindError::ImplicitArgumentSize {
                actual: physical.implicit_argument_size(),
            });
        }

        let generated = generated_vecadd_argument_layout_v2()
            .map_err(GeneratedWorkerV2VecAddBindError::VecAddProfile)?;
        let packing = validate_argument_packing(
            loaded.artifact_identity().kernel_id(),
            loaded.artifact_identity().abi(),
            &generated,
        )
        .map_err(GeneratedWorkerV2VecAddBindError::Packing)?;
        Ok(Self {
            loaded,
            observed,
            packing,
        })
    }

    /// Packs and reserves one exact read/read/write vecadd invocation.
    pub fn prepare<'allocation>(
        &mut self,
        a: &'allocation DeviceBuffer<f32>,
        b: &'allocation DeviceBuffer<f32>,
        c: &'allocation mut DeviceBuffer<f32>,
    ) -> Result<
        GeneratedWorkerV2VecAddPreparedV1<'_, 'allocation, K, A>,
        GeneratedWorkerV2VecAddPrepareError,
    > {
        let grid_x = checked_vecadd_grid(a.len(), b.len(), c.len())
            .map_err(GeneratedWorkerV2VecAddPrepareError::Shape)?;
        let a = GeneratedReadDeviceSlice::new(&self.observed, a)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        let b = GeneratedReadDeviceSlice::new(&self.observed, b)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        let c = GeneratedWriteDeviceSlice::new(&self.observed, c)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        self.prepare_slices(grid_x, a, b, c)
    }

    /// Packs and reserves vecadd over checked subregions, preserving canaries
    /// outside each selected range. The output view is consumed so no shared
    /// view can enter the writable path.
    pub fn prepare_views<'allocation>(
        &mut self,
        a: DeviceBufferView<'allocation, f32>,
        b: DeviceBufferView<'allocation, f32>,
        c: DeviceBufferViewMut<'allocation, f32>,
    ) -> Result<
        GeneratedWorkerV2VecAddPreparedV1<'_, 'allocation, K, A>,
        GeneratedWorkerV2VecAddPrepareError,
    > {
        let grid_x = checked_vecadd_grid(a.len(), b.len(), c.len())
            .map_err(GeneratedWorkerV2VecAddPrepareError::Shape)?;
        let a = GeneratedReadDeviceSlice::from_view(&self.observed, a)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        let b = GeneratedReadDeviceSlice::from_view(&self.observed, b)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        let c = GeneratedWriteDeviceSlice::from_view_mut(&self.observed, c)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Region)?;
        self.prepare_slices(grid_x, a, b, c)
    }

    fn prepare_slices<'allocation>(
        &mut self,
        grid_x: u32,
        a: GeneratedReadDeviceSlice<'allocation, f32>,
        b: GeneratedReadDeviceSlice<'allocation, f32>,
        c: GeneratedWriteDeviceSlice<'allocation, f32>,
    ) -> Result<
        GeneratedWorkerV2VecAddPreparedV1<'_, 'allocation, K, A>,
        GeneratedWorkerV2VecAddPrepareError,
    > {
        let (admission, registration) = admit_and_register(
            self.observed.alias_registry(),
            &self.observed,
            [
                a.argument_access(),
                b.argument_access(),
                c.argument_access(),
            ],
        )
        .map_err(GeneratedWorkerV2VecAddPrepareError::Alias)?;

        let a_len = u64::try_from(a.len()).expect("vecadd length was checked against u32");
        let b_len = u64::try_from(b.len()).expect("vecadd length was checked against u32");
        let c_len = u64::try_from(c.len()).expect("vecadd length was checked against u32");
        // SAFETY: each pointer and length comes from the retained generated
        // capability for that exact argument. Alias admission above reserves
        // the same selected regions through synchronous completion.
        let inputs = unsafe {
            [
                self.packing.slice(
                    0,
                    a.device_pointer(),
                    a_len,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )?,
                self.packing.slice(
                    1,
                    b.device_pointer(),
                    b_len,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )?,
                self.packing.slice(
                    2,
                    c.device_pointer(),
                    c_len,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::WriteOnly,
                )?,
            ]
        };
        let packed = self.packing.pack(inputs)?;
        validate_packed_arguments(self.loaded.artifact_identity().kernel_id(), &packed)?;
        let mut explicit = [0_u8; EXPLICIT_KERNARG_BYTES];
        explicit.copy_from_slice(packed.bytes());
        let mut kernarg = AlignedVecAddKernargV1 {
            bytes: [0; TOTAL_KERNARG_BYTES],
        };
        kernarg.bytes[..EXPLICIT_KERNARG_BYTES].copy_from_slice(&explicit);

        let geometry = HsaLaunchGeometryV1::new([grid_x, 1, 1], [BLOCK_SIZE, 1, 1], 0);
        let authorization = self
            .loaded
            .authorize_launch(geometry)
            .map_err(GeneratedWorkerV2VecAddPrepareError::Launch)?;
        Ok(GeneratedWorkerV2VecAddPreparedV1 {
            authorization,
            explicit,
            kernarg,
            resources: (a, b, c),
            _admission: admission,
            _registration: registration,
        })
    }

    pub fn unload(self) -> Result<UnloadedHsaExecutableV1, HsaExecutableUnloadError<A::Error>> {
        self.loaded.unload()
    }
}

fn validate_packed_arguments(
    kernel_id: crate::KernelId,
    packed: &GeneratedPackedArgumentsV1<'_>,
) -> Result<(), GeneratedWorkerV2VecAddPrepareError> {
    if packed.kernel_id() != kernel_id
        || packed.len() != EXPLICIT_KERNARG_BYTES
        || packed.alignment() != PHYSICAL_KERNARG_ALIGNMENT as u32
    {
        return Err(GeneratedWorkerV2VecAddPrepareError::PackedArgumentSubstitution);
    }
    Ok(())
}

/// Linear prepared invocation retaining exact allocation and alias witnesses.
#[must_use = "a prepared Worker V2 vecadd invocation does no work until dispatched"]
#[doc(hidden)]
pub struct GeneratedWorkerV2VecAddPreparedV1<
    'loaded,
    'allocation,
    K,
    A: ReviewedHsaImplicitKernargAdapterV1,
> {
    authorization: crate::HsaKernelLaunchAuthorizationV1<'loaded, K, A>,
    explicit: [u8; EXPLICIT_KERNARG_BYTES],
    kernarg: AlignedVecAddKernargV1,
    resources: VecAddResources<'allocation>,
    _admission: ArgumentAliasAdmission<'allocation>,
    _registration: InFlightRegionRegistration<'allocation>,
}

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> GeneratedWorkerV2VecAddPreparedV1<'_, '_, K, A> {
    /// Initializes hidden arguments through the reviewed adapter, dispatches,
    /// and returns only after authenticated synchronous completion.
    pub fn dispatch(
        mut self,
    ) -> Result<GeneratedWorkerV2VecAddCompletionV1, HsaGeneratedDispatchError<A::Error>> {
        let retained = (&self.resources, &self._admission, &self._registration);
        let completed = self.authorization.launch_generated_with_implicit_kernarg(
            &self.explicit,
            EXPLICIT_KERNARG_BYTES,
            IMPLICIT_KERNARG_BYTES,
            &mut self.kernarg.bytes,
        )?;
        let _ = retained;
        Ok(GeneratedWorkerV2VecAddCompletionV1 { completed })
    }
}

/// Authenticated completion of one exact typed Worker V2 vecadd invocation.
#[derive(Debug)]
#[doc(hidden)]
pub struct GeneratedWorkerV2VecAddCompletionV1 {
    completed: HsaCompletedDispatchV1,
}

impl GeneratedWorkerV2VecAddCompletionV1 {
    pub const fn completed(&self) -> &HsaCompletedDispatchV1 {
        &self.completed
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV2VecAddBindError {
    UnsupportedGeneratedProfile,
    GeneratedProfile(GeneratedKernelProfileError),
    VecAddProfile(GeneratedVecAddProfileError),
    Observe(CoreError),
    ContextDeviceMismatch,
    KernargSegmentSize {
        actual: u64,
    },
    KernargSegmentAlignment {
        actual: u64,
    },
    ImplicitArgumentOffset {
        actual: PhysicalMetadataValueV1<u64>,
    },
    ImplicitArgumentSize {
        actual: u64,
    },
    Packing(GeneratedArgumentPackingError),
}

impl fmt::Display for GeneratedWorkerV2VecAddBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedGeneratedProfile => {
                formatter.write_str("generated kernel is not the exact typed vecadd V2 profile")
            }
            Self::GeneratedProfile(error) => write!(formatter, "generated profile: {error}"),
            Self::VecAddProfile(error) => write!(formatter, "vecadd profile: {error}"),
            Self::Observe(error) => write!(formatter, "HIP context observation: {error}"),
            Self::ContextDeviceMismatch => formatter
                .write_str("HIP allocation context does not match the authenticated HSA device"),
            Self::KernargSegmentSize { actual } => {
                write!(formatter, "physical kernarg size {actual} is not 304 bytes")
            }
            Self::KernargSegmentAlignment { actual } => {
                write!(
                    formatter,
                    "physical kernarg alignment {actual} is not 8 bytes"
                )
            }
            Self::ImplicitArgumentOffset { actual } => write!(
                formatter,
                "physical implicit-kernarg offset {actual:?} is not known offset 48"
            ),
            Self::ImplicitArgumentSize { actual } => write!(
                formatter,
                "physical implicit-kernarg size {actual} is not 256 bytes"
            ),
            Self::Packing(error) => write!(formatter, "generated argument packing: {error}"),
        }
    }
}

impl std::error::Error for GeneratedWorkerV2VecAddBindError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GeneratedProfile(error) => Some(error),
            Self::VecAddProfile(error) => Some(error),
            Self::Observe(error) => Some(error),
            Self::Packing(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV2VecAddPrepareError {
    Shape(GeneratedVecAddPrepareError),
    Region(RegionError),
    Alias(AliasAdmissionError),
    Pack(GeneratedArgumentPackError),
    PackedArgumentSubstitution,
    Launch(HsaLaunchAuthorizationError),
}

impl fmt::Display for GeneratedWorkerV2VecAddPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(error) => write!(formatter, "vecadd shape: {error}"),
            Self::Region(error) => write!(formatter, "allocation region: {error}"),
            Self::Alias(error) => write!(formatter, "argument alias admission: {error}"),
            Self::Pack(error) => write!(formatter, "argument packing: {error}"),
            Self::PackedArgumentSubstitution => {
                formatter.write_str("packed arguments do not match the authenticated kernel")
            }
            Self::Launch(error) => write!(formatter, "HSA launch authorization: {error:?}"),
        }
    }
}

impl std::error::Error for GeneratedWorkerV2VecAddPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shape(error) => Some(error),
            Self::Region(error) => Some(error),
            Self::Alias(error) => Some(error),
            Self::Pack(error) => Some(error),
            Self::PackedArgumentSubstitution | Self::Launch(_) => None,
        }
    }
}

impl From<GeneratedArgumentPackError> for GeneratedWorkerV2VecAddPrepareError {
    fn from(error: GeneratedArgumentPackError) -> Self {
        Self::Pack(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_vecadd::generated_vecadd_abi_v2;

    fn plan(kernel: u8) -> crate::GeneratedArgumentPackingPlanV1 {
        let abi = generated_vecadd_abi_v2().unwrap();
        let generated = generated_vecadd_argument_layout_v2().unwrap();
        validate_argument_packing(crate::KernelId::from_bytes([kernel; 32]), &abi, &generated)
            .unwrap()
    }

    fn packed(kernel: u8) -> GeneratedPackedArgumentsV1<'static> {
        let plan = plan(kernel);
        // SAFETY: these inert test addresses are never dispatched or
        // dereferenced. The test covers only deterministic ABI encoding.
        let inputs = unsafe {
            [
                plan.slice(
                    0,
                    0x1000usize as *const (),
                    7,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap(),
                plan.slice(
                    1,
                    0x2000usize as *const (),
                    7,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
                .unwrap(),
                plan.slice(
                    2,
                    0x3000usize as *const (),
                    7,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::WriteOnly,
                )
                .unwrap(),
            ]
        };
        plan.pack(inputs).unwrap()
    }

    #[test]
    fn exact_vecadd_arguments_pack_into_the_48_byte_explicit_prefix() {
        let packed = packed(0x11);
        assert_eq!(packed.len(), EXPLICIT_KERNARG_BYTES);
        assert_eq!(packed.alignment(), PHYSICAL_KERNARG_ALIGNMENT as u32);
        for (offset, value) in [
            (0, 0x1000_u64),
            (8, 7),
            (16, 0x2000),
            (24, 7),
            (32, 0x3000),
            (40, 7),
        ] {
            assert_eq!(packed.bytes()[offset..offset + 8], value.to_le_bytes());
        }
    }

    #[test]
    fn packed_kernel_substitution_fails_closed() {
        let packed = packed(0x22);
        assert!(matches!(
            validate_packed_arguments(crate::KernelId::from_bytes([0x23; 32]), &packed),
            Err(GeneratedWorkerV2VecAddPrepareError::PackedArgumentSubstitution)
        ));
    }

    #[test]
    fn full_kernarg_storage_has_the_exact_cov6_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<AlignedVecAddKernargV1>(),
            TOTAL_KERNARG_BYTES
        );
        assert_eq!(std::mem::align_of::<AlignedVecAddKernargV1>(), 16);
        let storage = AlignedVecAddKernargV1 {
            bytes: [0; TOTAL_KERNARG_BYTES],
        };
        assert!(storage.bytes.as_ptr().addr().is_multiple_of(16));
    }
}
