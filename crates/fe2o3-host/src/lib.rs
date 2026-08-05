mod argument_alias;
mod artifact_binding;
mod loaded_kernel;
mod prepared_launch;

pub use argument_alias::{
    AliasAdmissionError, AllocationIdentity, AllocationProvenance, ArgumentAccess,
    ArgumentAccessMode, ArgumentAliasAdmission, ArgumentAliasValidator, AtomicAccess,
    AtomicOperation, AtomicOrdering, AtomicScope, CheckedByteRegion, ConflictSource,
    InvalidAtomicOrdering, RegionError,
};
pub use artifact_binding::{
    ARTIFACT_KERNEL_IDENTITY_VERSION, ArtifactBindingError, ArtifactKernelIdentityV1,
    ArtifactLaunchContractError, ArtifactRevalidationError, ValidatedArtifactSelectionV1,
};
#[doc(hidden)]
pub use artifact_binding::{GeneratedKernelBindingV1, GeneratedMarkerBindingError};
pub use fe2o3_core::{KernelParams, LaunchConfig};
pub use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};
#[doc(hidden)]
pub use loaded_kernel::{GeneratedKernelParams, LoadedKernelLoadError};
pub use loaded_kernel::{
    LoadedArgumentAdmittedLaunch, LoadedKernel, LoadedKernelMatchError, LoadedLaunchError,
    LoadedPreparedLaunch,
};
pub use prepared_launch::{
    ArgumentAdmittedLaunch, CheckedDimensions, DeviceIdentity, KernelBrand, LaunchAxis,
    LaunchDimension, ObservedContext, PrepareLaunchError, PreparedGeometry, PreparedLaunch,
    PreparedResources, UntrustedKernelDeclaration, UntrustedLaunchRequest,
};

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
