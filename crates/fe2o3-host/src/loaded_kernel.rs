use crate::artifact_binding::ArtifactKernelBrandV1;
use crate::{
    ArtifactKernelIdentityV1, DeviceIdentity, KernelBrand, KernelParams, LaunchConfig,
    ObservedContext, PrepareLaunchError, PreparedGeometry, PreparedLaunch, PreparedResources,
    UntrustedLaunchRequest, ValidatedArtifactSelectionV1,
};
use fe2o3_core::{GpuContext, GpuFunction, GpuModule, Stream};
use std::fmt;
use std::sync::Arc;

struct HipKernelOwnership {
    // Keep the module ownership explicit even though GpuFunction also retains
    // it. This boundary must not degrade to a bare native function handle.
    _module: Arc<GpuModule>,
    function: GpuFunction,
}

enum KernelOwnership {
    Runtime(HipKernelOwnership),
    #[cfg(test)]
    Test,
}

/// Owned HIP module/function authority for exactly one kernel marker `K`.
///
/// There is no public constructor. In particular,
/// [`ValidatedArtifactSelectionV1`] cannot mint this type: structural artifact
/// validation does not prove that an arbitrary Rust marker denotes the
/// selected kernel or its ABI. A future generated binding may invoke the
/// crate's unsafe issuance path only after establishing that relationship and
/// executable trust.
///
/// This type owns both the HIP module and function boundary. It does not expose
/// either raw handle, and it is intentionally neither `Clone` nor `Copy`.
pub struct LoadedKernel<K> {
    identity: Arc<ArtifactKernelIdentityV1>,
    payload: Arc<[u8]>,
    context: ObservedContext,
    brand: KernelBrand<K>,
    ownership: KernelOwnership,
}

impl<K> fmt::Debug for LoadedKernel<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedKernel")
            .field("identity", &self.identity)
            .field("payload_len", &self.payload.len())
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl<K> LoadedKernel<K> {
    /// Issues loaded authority after the private marker binding and validated
    /// artifact selection have been matched exactly.
    ///
    /// # Safety
    ///
    /// The caller must have independently authenticated or otherwise trusted
    /// the executable payload, including behavior during HIP module loading and
    /// unloading. The caller must also establish that `K` is the generated
    /// marker for `identity`, including the complete argument ABI and semantic
    /// contract. [`ValidatedArtifactSelectionV1`] checks only a conservative
    /// structural ABI subset and cannot discharge either obligation.
    pub(crate) unsafe fn load(
        binding: ArtifactKernelBrandV1<K>,
        validated: &ValidatedArtifactSelectionV1,
        observed: &ObservedContext,
        context: &Arc<GpuContext>,
    ) -> Result<Self, LoadedKernelLoadError> {
        validate_issuance(&binding, validated, observed)?;
        if !observed.is_for_context(context) {
            return Err(LoadedKernelLoadError::WrongContextWrapper);
        }

        // SAFETY: Executable trust and compatibility are obligations of this
        // unsafe function. Exact payload, target, device, context, symbol, and
        // structural ABI identity were checked before reaching this call.
        let module = unsafe { context.load_module_from_bytes_unchecked(&binding.payload) }
            .map_err(LoadedKernelLoadError::Hip)?;
        let function = module
            .load_function(binding.identity.symbol().as_str())
            .map_err(LoadedKernelLoadError::Hip)?;

        Ok(Self {
            identity: binding.identity,
            payload: binding.payload,
            context: binding.context,
            brand: binding.brand,
            ownership: KernelOwnership::Runtime(HipKernelOwnership {
                _module: module,
                function,
            }),
        })
    }

    pub fn identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.identity
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.context.device()
    }

    /// Prepares geometry and resource use under this exact loaded authority.
    pub fn prepare(
        &self,
        observed: &ObservedContext,
        request: UntrustedLaunchRequest<K>,
    ) -> Result<PreparedLaunch<K>, PrepareLaunchError> {
        self.brand.prepare(observed, request)
    }

    /// Consumes a prepared launch only when both its compile-time marker and
    /// runtime brand/context identity match this loaded authority.
    pub fn bind(
        &self,
        prepared: PreparedLaunch<K>,
    ) -> Result<LoadedPreparedLaunch<'_, K>, LoadedKernelMatchError> {
        let prepared_context = prepared.observed_context();
        if prepared.device() != self.context.device() {
            return Err(LoadedKernelMatchError::WrongDevice);
        }
        if !prepared_context.same_context(&self.context) {
            return Err(LoadedKernelMatchError::WrongContext);
        }
        if !prepared_context.same_launch_limits(&self.context) {
            return Err(LoadedKernelMatchError::DeviceLimitsChanged);
        }
        if !prepared_context.same_hip_capabilities(&self.context) {
            return Err(LoadedKernelMatchError::DeviceCapabilitiesChanged);
        }
        if prepared.kernel() != self.identity.kernel_id() {
            return Err(LoadedKernelMatchError::WrongKernel);
        }
        if !prepared.belongs_to(&self.brand) {
            return Err(LoadedKernelMatchError::WrongArtifactAuthority);
        }

        Ok(LoadedPreparedLaunch {
            loaded: self,
            prepared,
        })
    }

    fn function(&self) -> &GpuFunction {
        match &self.ownership {
            KernelOwnership::Runtime(ownership) => &ownership.function,
            #[cfg(test)]
            KernelOwnership::Test => panic!("test loaded kernels have no HIP function"),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_binding(binding: ArtifactKernelBrandV1<K>) -> Self {
        Self {
            identity: binding.identity,
            payload: binding.payload,
            context: binding.context,
            brand: binding.brand,
            ownership: KernelOwnership::Test,
        }
    }
}

/// A consumed [`PreparedLaunch`] borrowing the exact loaded module/function
/// authority that admitted it.
///
/// This value still carries no safe launch permission because raw argument
/// values, executable semantics, memory accesses, aliasing, and asynchronous
/// lifetimes have not been proven.
pub struct LoadedPreparedLaunch<'loaded, K> {
    loaded: &'loaded LoadedKernel<K>,
    prepared: PreparedLaunch<K>,
}

impl<K> fmt::Debug for LoadedPreparedLaunch<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedPreparedLaunch")
            .field("identity", &self.loaded.identity)
            .field("prepared", &self.prepared)
            .finish_non_exhaustive()
    }
}

impl<K> LoadedPreparedLaunch<'_, K> {
    pub fn identity(&self) -> &ArtifactKernelIdentityV1 {
        self.loaded.identity()
    }

    pub const fn geometry(&self) -> PreparedGeometry {
        self.prepared.geometry()
    }

    pub const fn resources(&self) -> PreparedResources {
        self.prepared.resources()
    }

    pub fn launch_config(&self) -> LaunchConfig {
        let grid = self.geometry().grid().dimensions();
        let block = self.geometry().block().dimensions();
        LaunchConfig {
            grid_dim: (grid[0], grid[1], grid[2]),
            block_dim: (block[0], block[1], block[2]),
            shared_mem_bytes: self.resources().dynamic_shared_memory_bytes(),
        }
    }

    /// Enqueues this checked geometry with caller-described raw arguments.
    ///
    /// The stream is rejected unless it belongs to the exact `GpuContext`
    /// wrapper observed when this authority was issued.
    ///
    /// # Safety
    ///
    /// `params` must exactly match the generated kernel ABI in field count,
    /// order, type, size, alignment, and pointer address space. Every reachable
    /// device allocation must belong to this context, be in bounds for every
    /// kernel access, obey aliasing and synchronization requirements, and
    /// remain alive until the stream completes. The loaded authority must also
    /// remain alive until completion. The caller remains responsible for the
    /// executable's authenticity and semantics, including freedom from data
    /// races and illegal memory accesses. This method enqueues work; it does not
    /// synchronize the stream.
    pub unsafe fn launch_raw(
        self,
        stream: &Stream,
        params: &mut KernelParams,
    ) -> Result<(), LoadedLaunchError> {
        if !self.loaded.context.is_for_context(stream.context()) {
            return Err(LoadedLaunchError::WrongStreamContext);
        }
        let config = self.launch_config();
        // SAFETY: The caller owns the raw ABI, memory, semantic, and completion
        // obligations documented above. The stream/context, function owner,
        // marker, artifact identity, and launch geometry were checked here.
        unsafe {
            fe2o3_core::launch_kernel_on_stream(self.loaded.function(), config, stream, params)
        }
        .map_err(LoadedLaunchError::Hip)
    }
}

pub(crate) fn validate_issuance<K>(
    binding: &ArtifactKernelBrandV1<K>,
    validated: &ValidatedArtifactSelectionV1,
    observed: &ObservedContext,
) -> Result<(), LoadedKernelLoadError> {
    if !Arc::ptr_eq(&binding.identity, &validated.identity)
        || !Arc::ptr_eq(&binding.payload, &validated.payload)
    {
        return Err(LoadedKernelLoadError::WrongValidatedSelection);
    }
    if observed.device() != binding.context.device() {
        return Err(LoadedKernelLoadError::WrongDevice);
    }
    if !observed.same_context(&binding.context) {
        return Err(LoadedKernelLoadError::WrongContext);
    }
    if !observed.same_launch_limits(&binding.context) {
        return Err(LoadedKernelLoadError::DeviceLimitsChanged);
    }
    if !observed.same_hip_capabilities(&binding.context) {
        return Err(LoadedKernelLoadError::DeviceCapabilitiesChanged);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LoadedKernelMatchError {
    WrongDevice,
    WrongContext,
    DeviceLimitsChanged,
    DeviceCapabilitiesChanged,
    WrongKernel,
    WrongArtifactAuthority,
}

impl fmt::Display for LoadedKernelMatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::WrongDevice => "prepared launch belongs to a different device",
            Self::WrongContext => "prepared launch belongs to a different context",
            Self::DeviceLimitsChanged => "prepared launch uses different observed device limits",
            Self::DeviceCapabilitiesChanged => {
                "prepared launch uses different observed device capabilities"
            }
            Self::WrongKernel => "prepared launch names a different kernel",
            Self::WrongArtifactAuthority => {
                "prepared launch belongs to a different artifact authority"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for LoadedKernelMatchError {}

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadedLaunchError {
    WrongStreamContext,
    Hip(fe2o3_core::Error),
}

impl fmt::Display for LoadedLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongStreamContext => {
                formatter.write_str("launch stream belongs to a different context")
            }
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LoadedLaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            Self::WrongStreamContext => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum LoadedKernelLoadError {
    WrongValidatedSelection,
    WrongDevice,
    WrongContext,
    DeviceLimitsChanged,
    DeviceCapabilitiesChanged,
    WrongContextWrapper,
    Hip(fe2o3_core::Error),
}

impl fmt::Display for LoadedKernelLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongValidatedSelection => {
                formatter.write_str("marker binding and validated selection differ")
            }
            Self::WrongDevice => formatter.write_str("observed device changed before loading"),
            Self::WrongContext => formatter.write_str("observed context changed before loading"),
            Self::DeviceLimitsChanged => {
                formatter.write_str("observed device limits changed before loading")
            }
            Self::DeviceCapabilitiesChanged => {
                formatter.write_str("observed device capabilities changed before loading")
            }
            Self::WrongContextWrapper => {
                formatter.write_str("GPU context wrapper does not match the observation")
            }
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LoadedKernelLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            _ => None,
        }
    }
}
