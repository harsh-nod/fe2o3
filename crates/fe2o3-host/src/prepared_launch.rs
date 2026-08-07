#[cfg(any(test, feature = "hardware-test-hooks"))]
use crate::argument_alias::fresh_alias_registry;
use crate::argument_alias::{
    AliasAdmissionRegistry, InFlightRegionRegistration, admit_and_register, shared_alias_registry,
};
use crate::{AliasAdmissionError, ArgumentAccess, ArgumentAliasAdmission};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_core::{GpuContext, Result as CoreResult};
use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// A device identity obtained from an observed HIP context.
///
/// Its fields are private because parsed target text and caller-provided device
/// ordinals are declarations, not observations. This identity does not by
/// itself authorize loading code or launching a kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    ordinal: i32,
    target: String,
    target_id: AmdTargetId,
}

impl DeviceIdentity {
    pub const fn ordinal(&self) -> i32 {
        self.ordinal
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the canonical target ID obtained from HIP.
    pub const fn target_id(&self) -> AmdTargetId {
        self.target_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContextIdentity(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeviceLaunchLimits {
    max_threads_per_block: u32,
    max_block_dimensions: [u32; 3],
    max_grid_dimensions: [u32; 3],
    max_shared_memory_per_block: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HipCapabilityFacts {
    default_warp_size: u32,
    global_int32_atomics: bool,
    shared_int32_atomics: bool,
    global_int64_atomics: bool,
    shared_int64_atomics: bool,
    warp_vote: bool,
    warp_ballot: bool,
    warp_shuffle: bool,
}

/// An exact context wrapper, device identity, launch-limit observation, and
/// copy of HIP's coarse device capability facts.
///
/// Production values retain the `Arc<GpuContext>` whose address forms the
/// context identity, preventing address reuse while the observation or a
/// prepared launch remains alive. Observing a context grants no module-loading
/// or kernel-launch authority.
#[derive(Clone)]
pub struct ObservedContext {
    identity: ContextIdentity,
    device: DeviceIdentity,
    limits: DeviceLaunchLimits,
    hip_capabilities: HipCapabilityFacts,
    alias_registry: Arc<AliasAdmissionRegistry>,
    retained_context: Option<Arc<GpuContext>>,
}

impl fmt::Debug for ObservedContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservedContext")
            .field("device", &self.device)
            .field("limits", &self.limits)
            .field("hip_capabilities", &self.hip_capabilities)
            .finish_non_exhaustive()
    }
}

impl ObservedContext {
    /// Observes the device and launch limits associated with one exact context
    /// wrapper.
    pub fn observe(context: &Arc<GpuContext>) -> CoreResult<Self> {
        let observed = context.observe_target()?;
        debug_assert_eq!(observed.device_id(), context.device_id());
        let target_id = observed.target_id();

        Ok(Self {
            identity: ContextIdentity(Arc::as_ptr(context) as usize),
            device: DeviceIdentity {
                ordinal: observed.device_id(),
                target: target_id.to_string(),
                target_id,
            },
            limits: DeviceLaunchLimits {
                max_threads_per_block: observed.max_threads_per_block(),
                max_block_dimensions: observed.max_block_dimensions(),
                max_grid_dimensions: observed.max_grid_dimensions(),
                // Opt-in shared memory requires a separate function attribute
                // operation. Until that path exists, preparation uses only the
                // portable default limit.
                max_shared_memory_per_block: observed.shared_memory_per_block(),
            },
            hip_capabilities: HipCapabilityFacts {
                default_warp_size: observed.hip_default_warp_size(),
                global_int32_atomics: observed.has_global_int32_atomics(),
                shared_int32_atomics: observed.has_shared_int32_atomics(),
                global_int64_atomics: observed.has_global_int64_atomics(),
                shared_int64_atomics: observed.has_shared_int64_atomics(),
                warp_vote: observed.has_warp_vote(),
                warp_ballot: observed.has_warp_ballot(),
                warp_shuffle: observed.has_warp_shuffle(),
            },
            alias_registry: shared_alias_registry(Arc::as_ptr(context) as usize),
            retained_context: Some(context.clone()),
        })
    }

    pub const fn device(&self) -> &DeviceIdentity {
        &self.device
    }

    /// Returns HIP's device-level default warp size.
    ///
    /// This is not a per-kernel wavefront-size observation and therefore does
    /// not establish subgroup or AMD-wave artifact capabilities.
    pub const fn hip_default_warp_size(&self) -> u32 {
        self.hip_capabilities.default_warp_size
    }

    /// Returns HIP's coarse global 32-bit integer-atomics device bit.
    pub const fn has_global_int32_atomics(&self) -> bool {
        self.hip_capabilities.global_int32_atomics
    }

    /// Returns HIP's coarse shared 32-bit integer-atomics device bit.
    pub const fn has_shared_int32_atomics(&self) -> bool {
        self.hip_capabilities.shared_int32_atomics
    }

    /// Returns HIP's coarse global 64-bit integer-atomics device bit.
    pub const fn has_global_int64_atomics(&self) -> bool {
        self.hip_capabilities.global_int64_atomics
    }

    /// Returns HIP's coarse shared 64-bit integer-atomics device bit.
    pub const fn has_shared_int64_atomics(&self) -> bool {
        self.hip_capabilities.shared_int64_atomics
    }

    /// Returns HIP's coarse warp-vote device bit.
    pub const fn has_warp_vote(&self) -> bool {
        self.hip_capabilities.warp_vote
    }

    /// Returns HIP's coarse warp-ballot device bit.
    pub const fn has_warp_ballot(&self) -> bool {
        self.hip_capabilities.warp_ballot
    }

    /// Returns HIP's coarse warp-shuffle device bit.
    pub const fn has_warp_shuffle(&self) -> bool {
        self.hip_capabilities.warp_shuffle
    }

    /// Returns whether this observation names this exact `GpuContext` wrapper.
    pub fn is_for_context(&self, context: &Arc<GpuContext>) -> bool {
        self.identity == ContextIdentity(Arc::as_ptr(context) as usize)
            && self.retained_context.is_some()
    }

    pub(crate) fn same_context(&self, other: &Self) -> bool {
        self.identity == other.identity
    }

    pub(crate) const fn context_key(&self) -> usize {
        self.identity.0
    }

    pub(crate) const fn alias_registry(&self) -> &Arc<AliasAdmissionRegistry> {
        &self.alias_registry
    }

    pub(crate) fn same_launch_limits(&self, other: &Self) -> bool {
        self.limits == other.limits
    }

    pub(crate) fn same_hip_capabilities(&self, other: &Self) -> bool {
        self.hip_capabilities == other.hip_capabilities
    }

    pub(crate) const fn max_threads_per_block(&self) -> u32 {
        self.limits.max_threads_per_block
    }

    pub(crate) const fn max_shared_memory_per_block(&self) -> u64 {
        self.limits.max_shared_memory_per_block
    }

    #[cfg(any(test, feature = "hardware-test-hooks"))]
    pub(crate) fn for_test(
        identity: usize,
        ordinal: i32,
        target: &str,
        max_threads_per_block: u32,
        max_shared_memory_per_block: u64,
    ) -> Self {
        let target_id = AmdTargetId::parse(target).expect("test target must be canonical");
        Self {
            identity: ContextIdentity(identity),
            device: DeviceIdentity {
                ordinal,
                target: target.into(),
                target_id,
            },
            limits: DeviceLaunchLimits {
                max_threads_per_block,
                max_block_dimensions: [max_threads_per_block; 3],
                max_grid_dimensions: [u32::MAX; 3],
                max_shared_memory_per_block,
            },
            hip_capabilities: HipCapabilityFacts {
                default_warp_size: 64,
                global_int32_atomics: true,
                shared_int32_atomics: true,
                global_int64_atomics: true,
                shared_int64_atomics: true,
                warp_vote: true,
                warp_ballot: true,
                warp_shuffle: true,
            },
            alias_registry: fresh_alias_registry(identity),
            retained_context: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_changed_test_hip_capabilities(mut self) -> Self {
        self.hip_capabilities.warp_ballot = !self.hip_capabilities.warp_ballot;
        self
    }
}

/// Caller-supplied kernel metadata that carries no validation authority.
///
/// Decoding a manifest or manually constructing this value does not create a
/// [`KernelBrand`], authorize module loading, or authorize launch. A future
/// generated binding must independently match both validated artifact identity
/// and the marker `K` before it can mint a brand.
pub struct UntrustedKernelDeclaration<K> {
    kernel: KernelId,
    launch: LaunchConstraintsV1,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> Clone for UntrustedKernelDeclaration<K> {
    fn clone(&self) -> Self {
        Self {
            kernel: self.kernel,
            launch: self.launch.clone(),
            marker: PhantomData,
        }
    }
}

impl<K> fmt::Debug for UntrustedKernelDeclaration<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UntrustedKernelDeclaration")
            .field("kernel", &self.kernel)
            .field("launch", &self.launch)
            .finish_non_exhaustive()
    }
}

impl<K> PartialEq for UntrustedKernelDeclaration<K> {
    fn eq(&self, other: &Self) -> bool {
        self.kernel == other.kernel && self.launch == other.launch
    }
}

impl<K> Eq for UntrustedKernelDeclaration<K> {}

impl<K> UntrustedKernelDeclaration<K> {
    pub fn new(kernel: KernelId, launch: LaunchConstraintsV1) -> Self {
        Self {
            kernel,
            launch,
            marker: PhantomData,
        }
    }

    pub const fn kernel(&self) -> KernelId {
        self.kernel
    }

    pub const fn launch(&self) -> &LaunchConstraintsV1 {
        &self.launch
    }
}

/// Inert, caller-supplied geometry for one typed kernel marker.
///
/// This value deliberately accepts malformed dimensions. All arithmetic,
/// contract, context, and live-device checks happen when a sealed
/// [`KernelBrand`] prepares it.
pub struct UntrustedLaunchRequest<K> {
    kernel: KernelId,
    rank: u8,
    grid: [u32; 3],
    block: [u32; 3],
    dynamic_shared_memory_bytes: u32,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> Clone for UntrustedLaunchRequest<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for UntrustedLaunchRequest<K> {}

impl<K> fmt::Debug for UntrustedLaunchRequest<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UntrustedLaunchRequest")
            .field("kernel", &self.kernel)
            .field("rank", &self.rank)
            .field("grid", &self.grid)
            .field("block", &self.block)
            .field(
                "dynamic_shared_memory_bytes",
                &self.dynamic_shared_memory_bytes,
            )
            .finish_non_exhaustive()
    }
}

impl<K> PartialEq for UntrustedLaunchRequest<K> {
    fn eq(&self, other: &Self) -> bool {
        self.kernel == other.kernel
            && self.rank == other.rank
            && self.grid == other.grid
            && self.block == other.block
            && self.dynamic_shared_memory_bytes == other.dynamic_shared_memory_bytes
    }
}

impl<K> Eq for UntrustedLaunchRequest<K> {}

impl<K> UntrustedLaunchRequest<K> {
    pub const fn new(
        kernel: KernelId,
        rank: u8,
        grid: [u32; 3],
        block: [u32; 3],
        dynamic_shared_memory_bytes: u32,
    ) -> Self {
        Self {
            kernel,
            rank,
            grid,
            block,
            dynamic_shared_memory_bytes,
            marker: PhantomData,
        }
    }

    pub const fn kernel(&self) -> KernelId {
        self.kernel
    }

    pub const fn rank(&self) -> u8 {
        self.rank
    }

    pub const fn grid(&self) -> [u32; 3] {
        self.grid
    }

    pub const fn block(&self) -> [u32; 3] {
        self.block
    }

    pub const fn dynamic_shared_memory_bytes(&self) -> u32 {
        self.dynamic_shared_memory_bytes
    }
}

struct BrandSeal;

/// A sealed association between one marker `K` and bound kernel metadata.
///
/// There is intentionally no public constructor and no conversion from
/// [`UntrustedKernelDeclaration`]. A future generated path must establish the
/// marker-to-kernel association in addition to artifact validation before it
/// can mint this brand. A brand contains no HIP module or function handle and
/// cannot launch anything in this skeleton.
pub struct KernelBrand<K> {
    kernel: KernelId,
    launch: LaunchConstraintsV1,
    context: ObservedContext,
    seal: Arc<BrandSeal>,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> fmt::Debug for KernelBrand<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KernelBrand")
            .field("kernel", &self.kernel)
            .field("launch", &self.launch)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl<K> KernelBrand<K> {
    // Crate privacy prevents callers from turning structural metadata into a
    // marker association. Future generated code must add unforgeable K evidence.
    #[allow(dead_code)]
    pub(crate) fn from_internal_binding(
        kernel: KernelId,
        launch: LaunchConstraintsV1,
        context: ObservedContext,
    ) -> Self {
        Self {
            kernel,
            launch,
            context,
            seal: Arc::new(BrandSeal),
            marker: PhantomData,
        }
    }

    pub const fn kernel(&self) -> KernelId {
        self.kernel
    }

    pub const fn launch_constraints(&self) -> &LaunchConstraintsV1 {
        &self.launch
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.context.device()
    }

    /// Checks an inert request against this exact kernel, context, device,
    /// contract, and observed device limits.
    ///
    /// The result remains data only. It has no module/function handle and no
    /// safe or unsafe launch method.
    pub fn prepare(
        &self,
        context: &ObservedContext,
        request: UntrustedLaunchRequest<K>,
    ) -> Result<PreparedLaunch<K>, PrepareLaunchError> {
        if request.kernel != self.kernel {
            return Err(PrepareLaunchError::WrongKernel {
                expected: self.kernel,
                actual: request.kernel,
            });
        }
        if context.device != self.context.device {
            return Err(PrepareLaunchError::WrongDevice {
                expected: self.context.device.clone(),
                actual: context.device.clone(),
            });
        }
        if !context.same_context(&self.context) {
            return Err(PrepareLaunchError::WrongContext);
        }
        if context.limits != self.context.limits {
            return Err(PrepareLaunchError::DeviceLimitsChanged);
        }
        if !context.same_hip_capabilities(&self.context) {
            return Err(PrepareLaunchError::DeviceCapabilitiesChanged);
        }

        let geometry = validate_geometry(&self.launch, &request)?;
        let resources = validate_resources(&self.launch, context.limits, &request)?;

        Ok(PreparedLaunch {
            kernel: self.kernel,
            context: self.context.clone(),
            geometry,
            resources,
            seal: self.seal.clone(),
            marker: PhantomData,
        })
    }
}

/// One validated dimension tuple and its checked product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedDimensions {
    dimensions: [u32; 3],
    product: u64,
}

impl CheckedDimensions {
    pub const fn dimensions(self) -> [u32; 3] {
        self.dimensions
    }

    pub const fn product(self) -> u64 {
        self.product
    }
}

/// Checked grid/block geometry for one prepared request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedGeometry {
    rank: u8,
    grid: CheckedDimensions,
    block: CheckedDimensions,
    total_threads: u64,
}

impl PreparedGeometry {
    pub const fn rank(self) -> u8 {
        self.rank
    }

    pub const fn grid(self) -> CheckedDimensions {
        self.grid
    }

    pub const fn block(self) -> CheckedDimensions {
        self.block
    }

    pub const fn total_threads(self) -> u64 {
        self.total_threads
    }
}

/// Checked per-block shared-memory requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResources {
    static_shared_memory_bytes: u32,
    dynamic_shared_memory_bytes: u32,
    total_shared_memory_bytes: u32,
}

impl PreparedResources {
    pub const fn static_shared_memory_bytes(self) -> u32 {
        self.static_shared_memory_bytes
    }

    pub const fn dynamic_shared_memory_bytes(self) -> u32 {
        self.dynamic_shared_memory_bytes
    }

    pub const fn total_shared_memory_bytes(self) -> u32 {
        self.total_shared_memory_bytes
    }
}

/// Checked launch identity, geometry, and resources for exactly one kernel `K`.
///
/// This G0 type intentionally does not contain a module or function handle and
/// exposes no launch operation. It cannot make the existing raw [`crate::launch!`]
/// macro safe. Artifact authentication, ABI validation, loading, argument
/// binding, and asynchronous resource lifetimes remain future work.
pub struct PreparedLaunch<K> {
    kernel: KernelId,
    context: ObservedContext,
    geometry: PreparedGeometry,
    resources: PreparedResources,
    seal: Arc<BrandSeal>,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> fmt::Debug for PreparedLaunch<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedLaunch")
            .field("kernel", &self.kernel)
            .field("device", &self.context.device)
            .field("geometry", &self.geometry)
            .field("resources", &self.resources)
            .finish_non_exhaustive()
    }
}

impl<K> PreparedLaunch<K> {
    pub const fn kernel(&self) -> KernelId {
        self.kernel
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.context.device()
    }

    pub const fn geometry(&self) -> PreparedGeometry {
        self.geometry
    }

    pub const fn resources(&self) -> PreparedResources {
        self.resources
    }

    pub fn is_for_context(&self, context: &Arc<GpuContext>) -> bool {
        self.context.is_for_context(context)
    }

    /// Confirms both the compile-time marker and exact runtime brand binding.
    pub fn belongs_to(&self, brand: &KernelBrand<K>) -> bool {
        Arc::ptr_eq(&self.seal, &brand.seal)
    }

    /// Validates and reserves this launch's argument byte regions in its exact
    /// observed context.
    ///
    /// The returned guard is tied to this prepared launch's private brand and
    /// keeps the reservation active until it is dropped. It carries no module,
    /// function, executable-verification, or launch authority.
    pub fn admit_arguments<'allocation>(
        self,
        arguments: impl IntoIterator<Item = ArgumentAccess<'allocation>>,
    ) -> Result<ArgumentAdmittedLaunch<'allocation, K>, AliasAdmissionError> {
        let (admission, registration) =
            admit_and_register(self.context.alias_registry(), &self.context, arguments)?;
        Ok(ArgumentAdmittedLaunch {
            prepared: self,
            admission,
            _registration: registration,
        })
    }

    pub(crate) const fn observed_context(&self) -> &ObservedContext {
        &self.context
    }
}

/// A branded prepared launch whose argument regions are reserved in its exact
/// context's in-flight registry.
///
/// Fields and constructors are private so standalone alias validation cannot
/// forge this state transition. This value still has no launch method and does
/// not represent executable verification.
pub struct ArgumentAdmittedLaunch<'allocation, K> {
    prepared: PreparedLaunch<K>,
    admission: ArgumentAliasAdmission<'allocation>,
    _registration: InFlightRegionRegistration<'allocation>,
}

impl<K> fmt::Debug for ArgumentAdmittedLaunch<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArgumentAdmittedLaunch")
            .field("prepared", &self.prepared)
            .field("argument_count", &self.admission.len())
            .finish_non_exhaustive()
    }
}

impl<K> ArgumentAdmittedLaunch<'_, K> {
    pub const fn kernel(&self) -> KernelId {
        self.prepared.kernel()
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.prepared.device()
    }

    pub const fn geometry(&self) -> PreparedGeometry {
        self.prepared.geometry()
    }

    pub const fn resources(&self) -> PreparedResources {
        self.prepared.resources()
    }

    pub fn argument_count(&self) -> usize {
        self.admission.len()
    }

    pub(crate) const fn prepared(&self) -> &PreparedLaunch<K> {
        &self.prepared
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchDimension {
    Grid,
    Block,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchAxis {
    X,
    Y,
    Z,
}

/// Failure while checking a launch request against trusted kernel metadata and
/// an observed context/device.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrepareLaunchError {
    WrongKernel {
        expected: KernelId,
        actual: KernelId,
    },
    WrongDevice {
        expected: DeviceIdentity,
        actual: DeviceIdentity,
    },
    WrongContext,
    DeviceLimitsChanged,
    DeviceCapabilitiesChanged,
    InvalidRank {
        actual: u8,
    },
    RankMismatch {
        required: u8,
        actual: u8,
    },
    ZeroDimension {
        dimension: LaunchDimension,
        axis: LaunchAxis,
    },
    RankDimensionMismatch {
        rank: u8,
        dimension: LaunchDimension,
        axis: LaunchAxis,
        actual: u32,
    },
    DimensionProductOverflow {
        dimension: LaunchDimension,
    },
    TotalThreadCountOverflow,
    KernelDimensionExceeded {
        dimension: LaunchDimension,
        axis: LaunchAxis,
        actual: u32,
        max: u32,
    },
    BlockShapeMismatch {
        required: [u32; 3],
        actual: [u32; 3],
    },
    KernelThreadsPerBlockExceeded {
        actual: u64,
        max: u32,
    },
    DynamicSharedMemoryExceeded {
        actual: u32,
        max: u32,
    },
    DeviceDimensionExceeded {
        dimension: LaunchDimension,
        axis: LaunchAxis,
        actual: u32,
        max: u32,
    },
    DeviceThreadsPerBlockExceeded {
        actual: u64,
        max: u32,
    },
    SharedMemoryTotalOverflow {
        static_bytes: u32,
        dynamic_bytes: u32,
    },
    DeviceSharedMemoryExceeded {
        total: u64,
        max: u64,
    },
}

impl fmt::Display for PrepareLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKernel { expected, actual } => {
                write!(
                    formatter,
                    "wrong kernel: expected {expected:?}, got {actual:?}"
                )
            }
            Self::WrongDevice { expected, actual } => {
                write!(
                    formatter,
                    "wrong device: expected {expected:?}, got {actual:?}"
                )
            }
            Self::WrongContext => formatter.write_str("wrong context"),
            Self::DeviceLimitsChanged => {
                formatter.write_str("device launch limits changed since kernel validation")
            }
            Self::DeviceCapabilitiesChanged => {
                formatter.write_str("HIP device capability facts changed since kernel validation")
            }
            Self::InvalidRank { actual } => write!(formatter, "invalid launch rank {actual}"),
            Self::RankMismatch { required, actual } => write!(
                formatter,
                "launch rank mismatch: kernel requires {required}, request has {actual}"
            ),
            Self::ZeroDimension { dimension, axis } => {
                write!(formatter, "zero {dimension:?} dimension on {axis:?}")
            }
            Self::RankDimensionMismatch {
                rank,
                dimension,
                axis,
                actual,
            } => write!(
                formatter,
                "rank {rank} launch has non-unit {dimension:?} {axis:?} dimension {actual}"
            ),
            Self::DimensionProductOverflow { dimension } => {
                write!(formatter, "{dimension:?} dimension product overflows u64")
            }
            Self::TotalThreadCountOverflow => {
                formatter.write_str("total launch thread count overflows u64")
            }
            Self::KernelDimensionExceeded {
                dimension,
                axis,
                actual,
                max,
            } => write!(
                formatter,
                "{dimension:?} {axis:?} dimension {actual} exceeds kernel maximum {max}"
            ),
            Self::BlockShapeMismatch { required, actual } => write!(
                formatter,
                "block shape {actual:?} does not match required shape {required:?}"
            ),
            Self::KernelThreadsPerBlockExceeded { actual, max } => write!(
                formatter,
                "block has {actual} threads, exceeding kernel maximum {max}"
            ),
            Self::DynamicSharedMemoryExceeded { actual, max } => write!(
                formatter,
                "dynamic shared memory {actual} exceeds kernel maximum {max}"
            ),
            Self::DeviceDimensionExceeded {
                dimension,
                axis,
                actual,
                max,
            } => write!(
                formatter,
                "{dimension:?} {axis:?} dimension {actual} exceeds device maximum {max}"
            ),
            Self::DeviceThreadsPerBlockExceeded { actual, max } => write!(
                formatter,
                "block has {actual} threads, exceeding device maximum {max}"
            ),
            Self::SharedMemoryTotalOverflow {
                static_bytes,
                dynamic_bytes,
            } => write!(
                formatter,
                "static shared memory {static_bytes} plus dynamic shared memory {dynamic_bytes} overflows u32"
            ),
            Self::DeviceSharedMemoryExceeded { total, max } => write!(
                formatter,
                "total shared memory {total} exceeds device maximum {max}"
            ),
        }
    }
}

impl std::error::Error for PrepareLaunchError {}

fn validate_geometry<K>(
    contract: &LaunchConstraintsV1,
    request: &UntrustedLaunchRequest<K>,
) -> Result<PreparedGeometry, PrepareLaunchError> {
    if !(1..=3).contains(&request.rank) {
        return Err(PrepareLaunchError::InvalidRank {
            actual: request.rank,
        });
    }
    if request.rank != contract.rank() {
        return Err(PrepareLaunchError::RankMismatch {
            required: contract.rank(),
            actual: request.rank,
        });
    }

    let grid = checked_dimensions(LaunchDimension::Grid, request.grid)?;
    let block = checked_dimensions(LaunchDimension::Block, request.block)?;
    validate_rank_dimensions(request.rank, LaunchDimension::Grid, request.grid)?;
    validate_rank_dimensions(request.rank, LaunchDimension::Block, request.block)?;

    validate_axes(
        LaunchDimension::Grid,
        request.grid,
        dimensions_array(contract.max_grid()),
        |dimension, axis, actual, max| PrepareLaunchError::KernelDimensionExceeded {
            dimension,
            axis,
            actual,
            max,
        },
    )?;

    match contract.block_size() {
        BlockSizeV1::Any => {}
        BlockSizeV1::Exact(required) => {
            let required = dimensions_array(required);
            if request.block != required {
                return Err(PrepareLaunchError::BlockShapeMismatch {
                    required,
                    actual: request.block,
                });
            }
        }
        BlockSizeV1::AtMost(max) => validate_axes(
            LaunchDimension::Block,
            request.block,
            dimensions_array(max),
            |dimension, axis, actual, max| PrepareLaunchError::KernelDimensionExceeded {
                dimension,
                axis,
                actual,
                max,
            },
        )?,
    }

    if block.product > u64::from(contract.max_flat_workgroup_size()) {
        return Err(PrepareLaunchError::KernelThreadsPerBlockExceeded {
            actual: block.product,
            max: contract.max_flat_workgroup_size(),
        });
    }

    let total_threads = grid
        .product
        .checked_mul(block.product)
        .ok_or(PrepareLaunchError::TotalThreadCountOverflow)?;
    Ok(PreparedGeometry {
        rank: request.rank,
        grid,
        block,
        total_threads,
    })
}

fn validate_resources<K>(
    contract: &LaunchConstraintsV1,
    limits: DeviceLaunchLimits,
    request: &UntrustedLaunchRequest<K>,
) -> Result<PreparedResources, PrepareLaunchError> {
    if request.dynamic_shared_memory_bytes > contract.max_dynamic_shared_memory_bytes() {
        return Err(PrepareLaunchError::DynamicSharedMemoryExceeded {
            actual: request.dynamic_shared_memory_bytes,
            max: contract.max_dynamic_shared_memory_bytes(),
        });
    }

    validate_axes(
        LaunchDimension::Grid,
        request.grid,
        limits.max_grid_dimensions,
        |dimension, axis, actual, max| PrepareLaunchError::DeviceDimensionExceeded {
            dimension,
            axis,
            actual,
            max,
        },
    )?;
    validate_axes(
        LaunchDimension::Block,
        request.block,
        limits.max_block_dimensions,
        |dimension, axis, actual, max| PrepareLaunchError::DeviceDimensionExceeded {
            dimension,
            axis,
            actual,
            max,
        },
    )?;

    let block_threads =
        checked_product(request.block).ok_or(PrepareLaunchError::DimensionProductOverflow {
            dimension: LaunchDimension::Block,
        })?;
    if block_threads > u64::from(limits.max_threads_per_block) {
        return Err(PrepareLaunchError::DeviceThreadsPerBlockExceeded {
            actual: block_threads,
            max: limits.max_threads_per_block,
        });
    }

    let static_bytes = contract.static_shared_memory_bytes();
    let dynamic_bytes = request.dynamic_shared_memory_bytes;
    let total_shared_memory_bytes = checked_shared_memory_total(static_bytes, dynamic_bytes)?;
    if u64::from(total_shared_memory_bytes) > limits.max_shared_memory_per_block {
        return Err(PrepareLaunchError::DeviceSharedMemoryExceeded {
            total: u64::from(total_shared_memory_bytes),
            max: limits.max_shared_memory_per_block,
        });
    }

    Ok(PreparedResources {
        static_shared_memory_bytes: static_bytes,
        dynamic_shared_memory_bytes: dynamic_bytes,
        total_shared_memory_bytes,
    })
}

fn checked_dimensions(
    dimension: LaunchDimension,
    dimensions: [u32; 3],
) -> Result<CheckedDimensions, PrepareLaunchError> {
    for (axis, value) in axes(dimensions) {
        if value == 0 {
            return Err(PrepareLaunchError::ZeroDimension { dimension, axis });
        }
    }
    let product = checked_product(dimensions)
        .ok_or(PrepareLaunchError::DimensionProductOverflow { dimension })?;
    Ok(CheckedDimensions {
        dimensions,
        product,
    })
}

fn checked_shared_memory_total(
    static_bytes: u32,
    dynamic_bytes: u32,
) -> Result<u32, PrepareLaunchError> {
    static_bytes
        .checked_add(dynamic_bytes)
        .ok_or(PrepareLaunchError::SharedMemoryTotalOverflow {
            static_bytes,
            dynamic_bytes,
        })
}

fn validate_rank_dimensions(
    rank: u8,
    dimension: LaunchDimension,
    dimensions: [u32; 3],
) -> Result<(), PrepareLaunchError> {
    for (axis_index, (axis, actual)) in axes(dimensions).enumerate() {
        if axis_index >= usize::from(rank) && actual != 1 {
            return Err(PrepareLaunchError::RankDimensionMismatch {
                rank,
                dimension,
                axis,
                actual,
            });
        }
    }
    Ok(())
}

fn validate_axes<E>(
    dimension: LaunchDimension,
    actual: [u32; 3],
    max: [u32; 3],
    error: impl Fn(LaunchDimension, LaunchAxis, u32, u32) -> E,
) -> Result<(), E> {
    for ((axis, actual), max) in axes(actual).zip(max) {
        if actual > max {
            return Err(error(dimension, axis, actual, max));
        }
    }
    Ok(())
}

fn dimensions_array(dimensions: DimensionsV1) -> [u32; 3] {
    [dimensions.x(), dimensions.y(), dimensions.z()]
}

fn checked_product(dimensions: [u32; 3]) -> Option<u64> {
    u64::from(dimensions[0])
        .checked_mul(u64::from(dimensions[1]))?
        .checked_mul(u64::from(dimensions[2]))
}

fn axes(dimensions: [u32; 3]) -> impl Iterator<Item = (LaunchAxis, u32)> {
    [LaunchAxis::X, LaunchAxis::Y, LaunchAxis::Z]
        .into_iter()
        .zip(dimensions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AllocationProvenance, ArgumentAccessMode, AtomicAccess, AtomicOperation, AtomicOrdering,
        AtomicScope, ConflictSource,
    };

    struct VecAdd;

    const VECADD: KernelId = KernelId::from_bytes([1; 32]);
    const OTHER: KernelId = KernelId::from_bytes([2; 32]);

    fn dimensions(x: u32, y: u32, z: u32) -> DimensionsV1 {
        DimensionsV1::new(x, y, z).unwrap()
    }

    fn contract(rank: u8, block: BlockSizeV1) -> LaunchConstraintsV1 {
        LaunchConstraintsV1::new(rank, block, dimensions(65_535, 1, 1), 256, 1_024, 8_192).unwrap()
    }

    fn context(identity: usize, ordinal: i32, target: &str) -> ObservedContext {
        let mut observed = ObservedContext::for_test(identity, ordinal, target, 1_024, 65_536);
        observed.limits.max_grid_dimensions = [u32::MAX, 65_535, 65_535];
        observed
    }

    fn brand(contract: LaunchConstraintsV1, context: ObservedContext) -> KernelBrand<VecAdd> {
        KernelBrand::from_internal_binding(VECADD, contract, context)
    }

    fn request(
        kernel: KernelId,
        rank: u8,
        grid: [u32; 3],
        block: [u32; 3],
        dynamic: u32,
    ) -> UntrustedLaunchRequest<VecAdd> {
        UntrustedLaunchRequest::new(kernel, rank, grid, block, dynamic)
    }

    fn assert_error(
        brand: &KernelBrand<VecAdd>,
        context: &ObservedContext,
        request: UntrustedLaunchRequest<VecAdd>,
        expected: PrepareLaunchError,
    ) {
        assert_eq!(brand.prepare(context, request).unwrap_err(), expected);
    }

    fn prepared(brand: &KernelBrand<VecAdd>, context: &ObservedContext) -> PreparedLaunch<VecAdd> {
        brand
            .prepare(context, request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0))
            .unwrap()
    }

    unsafe fn allocation<'a>(
        context: &ObservedContext,
        owner: &'a (),
        address: usize,
        byte_length: usize,
    ) -> AllocationProvenance<'a> {
        // SAFETY: tests use inert addresses only to exercise admission; no
        // resulting value can access memory or enqueue work.
        unsafe {
            AllocationProvenance::from_raw_parts(context, owner, address as *mut u8, byte_length)
                .unwrap()
        }
    }

    fn read(region: crate::CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(region, ArgumentAccessMode::SharedRead)
    }

    fn write(region: crate::CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(region, ArgumentAccessMode::ExclusiveWrite)
    }

    fn atomic(region: crate::CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(
            region,
            ArgumentAccessMode::Atomic(
                AtomicAccess::new(
                    AtomicOperation::ReadModifyWrite,
                    AtomicOrdering::AcquireRelease,
                    AtomicScope::Device,
                )
                .unwrap(),
            ),
        )
    }

    #[test]
    fn preparation_binds_identity_context_device_geometry_and_resources() {
        let observed = context(11, 0, "gfx942:sramecc+:xnack-");
        let brand = brand(
            contract(1, BlockSizeV1::Exact(dimensions(256, 1, 1))),
            observed.clone(),
        );
        let prepared = brand
            .prepare(&observed, request(VECADD, 1, [17, 1, 1], [256, 1, 1], 512))
            .unwrap();

        assert_eq!(prepared.kernel(), VECADD);
        assert_eq!(prepared.device(), observed.device());
        assert_eq!(prepared.geometry().rank(), 1);
        assert_eq!(prepared.geometry().grid().dimensions(), [17, 1, 1]);
        assert_eq!(prepared.geometry().grid().product(), 17);
        assert_eq!(prepared.geometry().block().dimensions(), [256, 1, 1]);
        assert_eq!(prepared.geometry().block().product(), 256);
        assert_eq!(prepared.geometry().total_threads(), 4_352);
        assert_eq!(prepared.resources().static_shared_memory_bytes(), 1_024);
        assert_eq!(prepared.resources().dynamic_shared_memory_bytes(), 512);
        assert_eq!(prepared.resources().total_shared_memory_bytes(), 1_536);
        assert!(prepared.belongs_to(&brand));
    }

    #[test]
    fn prepared_launch_matches_only_the_brand_issuance_that_created_it() {
        let observed = context(11, 0, "gfx942");
        let launch = contract(1, BlockSizeV1::Any);
        let first = brand(launch.clone(), observed.clone());
        let second = brand(launch, observed.clone());
        let prepared = first
            .prepare(&observed, request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0))
            .unwrap();

        assert!(prepared.belongs_to(&first));
        assert!(!prepared.belongs_to(&second));
    }

    #[test]
    fn admitted_launch_retains_brand_and_accepts_touching_ranges() {
        let observed = context(31, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&observed, &owner, 0x1000, 64) };

        let first = prepared(&brand, &observed)
            .admit_arguments([write(allocation.region(0, 16).unwrap())])
            .unwrap();
        let touching = prepared(&brand, &observed)
            .admit_arguments([write(allocation.region(16, 16).unwrap())])
            .unwrap();

        assert_eq!(first.argument_count(), 1);
        assert_eq!(first.kernel(), VECADD);
        assert!(first.prepared().belongs_to(&brand));
        assert_eq!(touching.geometry().total_threads(), 1);
    }

    #[test]
    fn repeated_allocation_provenance_cannot_hide_overlap() {
        let observed = context(32, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        let first_owner = ();
        let second_owner = ();
        // SAFETY: both declarations model the same live allocation.
        let first = unsafe { allocation(&observed, &first_owner, 0x2000, 64) };
        // SAFETY: see above.
        let repeated = unsafe { allocation(&observed, &second_owner, 0x2000, 64) };

        assert_eq!(first.identity(), repeated.identity());
        assert_eq!(
            prepared(&brand, &observed)
                .admit_arguments([
                    write(first.region(0, 32).unwrap()),
                    write(repeated.region(16, 32).unwrap()),
                ])
                .unwrap_err(),
            AliasAdmissionError::Conflict {
                argument_index: 1,
                conflicting_with: ConflictSource::Argument {
                    earlier_argument: 0,
                },
            }
        );
    }

    #[test]
    fn admission_rejects_unknown_cross_context_and_atomic_overlap() {
        let observed = context(33, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        assert_eq!(
            prepared(&brand, &observed)
                .admit_arguments([ArgumentAccess::unknown(ArgumentAccessMode::ExclusiveWrite,)])
                .unwrap_err(),
            AliasAdmissionError::UnknownProvenance { argument_index: 0 }
        );

        for wrong in [context(34, 0, "gfx942"), context(35, 1, "gfx950")] {
            let owner = ();
            // SAFETY: see `allocation`.
            let allocation = unsafe { allocation(&wrong, &owner, 0x3000, 16) };
            assert_eq!(
                prepared(&brand, &observed)
                    .admit_arguments([read(allocation.region(0, 16).unwrap())])
                    .unwrap_err(),
                AliasAdmissionError::ArgumentContextMismatch { argument_index: 0 }
            );
        }

        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&observed, &owner, 0x4000, 16) };
        assert!(matches!(
            prepared(&brand, &observed).admit_arguments([
                atomic(allocation.region(0, 8).unwrap()),
                write(allocation.region(4, 8).unwrap()),
            ]),
            Err(AliasAdmissionError::Conflict { .. })
        ));
    }

    #[test]
    fn registered_cross_launch_conflict_clears_when_guard_drops() {
        let observed = context(36, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&observed, &owner, 0x5000, 64) };

        let in_flight = prepared(&brand, &observed)
            .admit_arguments([write(allocation.region(8, 24).unwrap())])
            .unwrap();
        assert_eq!(
            prepared(&brand, &observed)
                .admit_arguments([read(allocation.region(0, 16).unwrap())])
                .unwrap_err(),
            AliasAdmissionError::Conflict {
                argument_index: 0,
                conflicting_with: ConflictSource::InFlight {
                    launch_index: 0,
                    argument_index: 0,
                },
            }
        );

        drop(in_flight);
        prepared(&brand, &observed)
            .admit_arguments([write(allocation.region(8, 24).unwrap())])
            .unwrap();
    }

    #[test]
    fn rejects_wrong_kernel_before_geometry_checks() {
        let observed = context(11, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        assert_error(
            &brand,
            &observed,
            request(OTHER, 1, [0, 1, 1], [0, 1, 1], 0),
            PrepareLaunchError::WrongKernel {
                expected: VECADD,
                actual: OTHER,
            },
        );
    }

    #[test]
    fn rejects_wrong_context_and_device_or_target() {
        let observed = context(11, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());

        assert_error(
            &brand,
            &context(12, 0, "gfx942"),
            request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0),
            PrepareLaunchError::WrongContext,
        );

        for wrong in [context(11, 1, "gfx942"), context(11, 0, "gfx950")] {
            assert_error(
                &brand,
                &wrong,
                request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0),
                PrepareLaunchError::WrongDevice {
                    expected: observed.device.clone(),
                    actual: wrong.device.clone(),
                },
            );
        }
    }

    #[test]
    fn rejects_changed_limits_for_the_same_context_and_device() {
        let observed = context(11, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        let mut changed = observed.clone();
        changed.limits.max_threads_per_block -= 1;

        assert_error(
            &brand,
            &changed,
            request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0),
            PrepareLaunchError::DeviceLimitsChanged,
        );
    }

    #[test]
    fn rejects_changed_hip_capabilities_for_the_same_context_device_and_limits() {
        let observed = context(11, 0, "gfx942");
        let brand = brand(contract(1, BlockSizeV1::Any), observed.clone());
        let changed = observed.clone().with_changed_test_hip_capabilities();

        assert_error(
            &brand,
            &changed,
            request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0),
            PrepareLaunchError::DeviceCapabilitiesChanged,
        );
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn observed_context_preserves_real_hip_target_limits_and_capability_facts() {
        let context = GpuContext::new(0).unwrap();
        let direct = context.observe_target().unwrap();
        let observed = ObservedContext::observe(&context).unwrap();

        assert_eq!(observed.device().ordinal(), direct.device_id());
        assert_eq!(observed.device().target_id(), direct.target_id());
        assert_eq!(observed.device().target(), direct.target_id().to_string());
        assert_eq!(
            observed.hip_default_warp_size(),
            direct.hip_default_warp_size()
        );
        assert_eq!(
            observed.has_global_int32_atomics(),
            direct.has_global_int32_atomics()
        );
        assert_eq!(
            observed.has_shared_int32_atomics(),
            direct.has_shared_int32_atomics()
        );
        assert_eq!(
            observed.has_global_int64_atomics(),
            direct.has_global_int64_atomics()
        );
        assert_eq!(
            observed.has_shared_int64_atomics(),
            direct.has_shared_int64_atomics()
        );
        assert_eq!(observed.has_warp_vote(), direct.has_warp_vote());
        assert_eq!(observed.has_warp_ballot(), direct.has_warp_ballot());
        assert_eq!(observed.has_warp_shuffle(), direct.has_warp_shuffle());
        assert_eq!(
            observed.max_shared_memory_per_block(),
            direct.shared_memory_per_block()
        );
    }

    #[test]
    fn rejects_invalid_and_mismatched_ranks() {
        let observed = context(1, 0, "gfx942");
        let brand = brand(contract(2, BlockSizeV1::Any), observed.clone());

        for rank in [0, 4, u8::MAX] {
            assert_error(
                &brand,
                &observed,
                request(VECADD, rank, [1, 1, 1], [1, 1, 1], 0),
                PrepareLaunchError::InvalidRank { actual: rank },
            );
        }
        assert_error(
            &brand,
            &observed,
            request(VECADD, 1, [1, 1, 1], [1, 1, 1], 0),
            PrepareLaunchError::RankMismatch {
                required: 2,
                actual: 1,
            },
        );
    }

    #[test]
    fn rejects_zero_on_every_grid_and_block_axis() {
        let observed = context(1, 0, "gfx942");
        let brand = brand(contract(3, BlockSizeV1::Any), observed.clone());
        for (dimension, is_grid) in [
            (LaunchDimension::Grid, true),
            (LaunchDimension::Block, false),
        ] {
            for (index, axis) in [LaunchAxis::X, LaunchAxis::Y, LaunchAxis::Z]
                .into_iter()
                .enumerate()
            {
                let mut grid = [1, 1, 1];
                let mut block = [1, 1, 1];
                if is_grid {
                    grid[index] = 0;
                } else {
                    block[index] = 0;
                }
                assert_error(
                    &brand,
                    &observed,
                    request(VECADD, 3, grid, block, 0),
                    PrepareLaunchError::ZeroDimension { dimension, axis },
                );
            }
        }
    }

    #[test]
    fn rejects_non_unit_inactive_rank_dimensions() {
        for rank in [1, 2] {
            let observed = context(1, 0, "gfx942");
            let max_grid = if rank == 1 {
                dimensions(65_535, 1, 1)
            } else {
                dimensions(65_535, 65_535, 1)
            };
            let contract =
                LaunchConstraintsV1::new(rank, BlockSizeV1::Any, max_grid, 1_024, 0, 0).unwrap();
            let brand = brand(contract, observed.clone());
            for dimension in [LaunchDimension::Grid, LaunchDimension::Block] {
                for (index, axis) in [LaunchAxis::X, LaunchAxis::Y, LaunchAxis::Z]
                    .into_iter()
                    .enumerate()
                    .skip(usize::from(rank))
                {
                    let mut grid = [1, 1, 1];
                    let mut block = [1, 1, 1];
                    if dimension == LaunchDimension::Grid {
                        grid[index] = 2;
                    } else {
                        block[index] = 2;
                    }
                    assert_error(
                        &brand,
                        &observed,
                        request(VECADD, rank, grid, block, 0),
                        PrepareLaunchError::RankDimensionMismatch {
                            rank,
                            dimension,
                            axis,
                            actual: 2,
                        },
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_dimension_and_total_thread_overflow() {
        assert_eq!(
            checked_dimensions(LaunchDimension::Grid, [u32::MAX; 3]),
            Err(PrepareLaunchError::DimensionProductOverflow {
                dimension: LaunchDimension::Grid,
            })
        );
        assert_eq!(
            checked_dimensions(LaunchDimension::Block, [u32::MAX; 3]),
            Err(PrepareLaunchError::DimensionProductOverflow {
                dimension: LaunchDimension::Block,
            })
        );

        let observed = context(1, 0, "gfx942");
        let wide_contract = LaunchConstraintsV1::new(
            3,
            BlockSizeV1::Any,
            dimensions(u32::MAX, u32::MAX, 1),
            u32::MAX,
            0,
            0,
        )
        .unwrap();
        let brand = brand(wide_contract, observed.clone());
        assert_error(
            &brand,
            &observed,
            request(VECADD, 3, [u32::MAX, u32::MAX, 1], [2, 1, 1], 0),
            PrepareLaunchError::TotalThreadCountOverflow,
        );
    }

    #[test]
    fn enforces_kernel_grid_block_flat_and_dynamic_limits() {
        let observed = context(1, 0, "gfx942");
        let contract = LaunchConstraintsV1::new(
            2,
            BlockSizeV1::AtMost(dimensions(16, 8, 1)),
            dimensions(100, 50, 1),
            128,
            1_024,
            2_048,
        )
        .unwrap();
        let brand = brand(contract, observed.clone());

        assert_error(
            &brand,
            &observed,
            request(VECADD, 2, [101, 1, 1], [1, 1, 1], 0),
            PrepareLaunchError::KernelDimensionExceeded {
                dimension: LaunchDimension::Grid,
                axis: LaunchAxis::X,
                actual: 101,
                max: 100,
            },
        );
        assert_error(
            &brand,
            &observed,
            request(VECADD, 2, [1, 1, 1], [17, 1, 1], 0),
            PrepareLaunchError::KernelDimensionExceeded {
                dimension: LaunchDimension::Block,
                axis: LaunchAxis::X,
                actual: 17,
                max: 16,
            },
        );
        assert_error(
            &brand,
            &observed,
            request(VECADD, 2, [1, 1, 1], [1, 1, 1], 2_049),
            PrepareLaunchError::DynamicSharedMemoryExceeded {
                actual: 2_049,
                max: 2_048,
            },
        );

        let flat_contract =
            LaunchConstraintsV1::new(2, BlockSizeV1::Any, dimensions(100, 50, 1), 64, 0, 0)
                .unwrap();
        let flat_brand =
            KernelBrand::from_internal_binding(VECADD, flat_contract, observed.clone());
        assert_error(
            &flat_brand,
            &observed,
            request(VECADD, 2, [1, 1, 1], [16, 8, 1], 0),
            PrepareLaunchError::KernelThreadsPerBlockExceeded {
                actual: 128,
                max: 64,
            },
        );
    }

    #[test]
    fn exact_block_requirement_checks_every_axis() {
        let observed = context(1, 0, "gfx942");
        let required = [8, 4, 2];
        let contract = LaunchConstraintsV1::new(
            3,
            BlockSizeV1::Exact(dimensions(8, 4, 2)),
            dimensions(1, 1, 1),
            64,
            0,
            0,
        )
        .unwrap();
        let brand = brand(contract, observed.clone());
        for index in 0..3 {
            let mut actual = required;
            actual[index] -= 1;
            assert_error(
                &brand,
                &observed,
                request(VECADD, 3, [1, 1, 1], actual, 0),
                PrepareLaunchError::BlockShapeMismatch { required, actual },
            );
        }
    }

    #[test]
    fn enforces_every_device_axis_and_flat_thread_limit() {
        let mut observed = context(1, 0, "gfx942");
        observed.limits.max_grid_dimensions = [10, 11, 12];
        observed.limits.max_block_dimensions = [8, 9, 10];
        observed.limits.max_threads_per_block = 16;
        let contract =
            LaunchConstraintsV1::new(3, BlockSizeV1::Any, dimensions(100, 100, 100), 1_024, 0, 0)
                .unwrap();
        let brand = brand(contract, observed.clone());

        for (dimension, limits) in [
            (LaunchDimension::Grid, observed.limits.max_grid_dimensions),
            (LaunchDimension::Block, observed.limits.max_block_dimensions),
        ] {
            for (index, (axis, max)) in [LaunchAxis::X, LaunchAxis::Y, LaunchAxis::Z]
                .into_iter()
                .zip(limits)
                .enumerate()
            {
                let mut grid = [1, 1, 1];
                let mut block = [1, 1, 1];
                if dimension == LaunchDimension::Grid {
                    grid[index] = max + 1;
                } else {
                    block[index] = max + 1;
                }
                assert_error(
                    &brand,
                    &observed,
                    request(VECADD, 3, grid, block, 0),
                    PrepareLaunchError::DeviceDimensionExceeded {
                        dimension,
                        axis,
                        actual: max + 1,
                        max,
                    },
                );
            }
        }

        assert_error(
            &brand,
            &observed,
            request(VECADD, 3, [1, 1, 1], [8, 3, 1], 0),
            PrepareLaunchError::DeviceThreadsPerBlockExceeded {
                actual: 24,
                max: 16,
            },
        );
    }

    #[test]
    fn rejects_shared_memory_overflow_and_device_limit() {
        assert_eq!(
            checked_shared_memory_total(u32::MAX, 1),
            Err(PrepareLaunchError::SharedMemoryTotalOverflow {
                static_bytes: u32::MAX,
                dynamic_bytes: 1,
            })
        );

        let mut observed = context(1, 0, "gfx942");
        observed.limits.max_shared_memory_per_block = 4_096;
        let contract =
            LaunchConstraintsV1::new(1, BlockSizeV1::Any, dimensions(1, 1, 1), 1, 3_000, 2_000)
                .unwrap();
        let brand = brand(contract, observed.clone());
        assert_error(
            &brand,
            &observed,
            request(VECADD, 1, [1, 1, 1], [1, 1, 1], 1_097),
            PrepareLaunchError::DeviceSharedMemoryExceeded {
                total: 4_097,
                max: 4_096,
            },
        );
    }

    #[test]
    fn untrusted_declaration_only_preserves_inert_data() {
        let launch = contract(1, BlockSizeV1::Any);
        let declaration = UntrustedKernelDeclaration::<VecAdd>::new(VECADD, launch.clone());
        assert_eq!(declaration.kernel(), VECADD);
        assert_eq!(declaration.launch(), &launch);
    }

    #[test]
    fn untrusted_generic_data_adds_no_marker_trait_bounds() {
        struct MarkerWithoutTraits;

        fn require_clone_debug_eq<T: Clone + fmt::Debug + Eq>(_: &T) {}
        fn require_copy<T: Copy>(_: T) {}

        let launch = contract(1, BlockSizeV1::Any);
        let declaration = UntrustedKernelDeclaration::<MarkerWithoutTraits>::new(VECADD, launch);
        let request =
            UntrustedLaunchRequest::<MarkerWithoutTraits>::new(VECADD, 1, [1, 1, 1], [1, 1, 1], 0);

        require_clone_debug_eq(&declaration);
        require_clone_debug_eq(&request);
        require_copy(request);
    }

    #[test]
    fn errors_have_actionable_display_text() {
        assert_eq!(
            PrepareLaunchError::WrongContext.to_string(),
            "wrong context"
        );
        assert!(
            PrepareLaunchError::RankMismatch {
                required: 1,
                actual: 2,
            }
            .to_string()
            .contains("requires 1")
        );
        assert!(
            PrepareLaunchError::DeviceThreadsPerBlockExceeded {
                actual: 1_024,
                max: 256,
            }
            .to_string()
            .contains("device maximum 256")
        );
    }
}
