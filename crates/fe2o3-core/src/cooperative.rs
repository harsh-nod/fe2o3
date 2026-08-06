use crate::{Error, GpuContext, GpuFunction, KernelParams, LaunchConfig, Result, Stream, check};
use core::fmt;
use std::sync::Arc;

trait CooperativeBackend {
    fn bind(&self, context: &GpuContext) -> Result<()>;
    fn cooperative_launch(&self, device: i32) -> Result<i32>;

    unsafe fn launch(
        &self,
        function: &GpuFunction,
        config: LaunchConfig,
        stream: &Stream,
        params: &mut KernelParams,
    ) -> Result<()>;
}

struct HipCooperativeBackend;

impl CooperativeBackend for HipCooperativeBackend {
    fn bind(&self, context: &GpuContext) -> Result<()> {
        context.bind_to_thread()
    }

    fn cooperative_launch(&self, device: i32) -> Result<i32> {
        let mut supported = 0;
        check(unsafe {
            fe2o3_hip_sys::hipDeviceGetAttribute(
                &mut supported,
                fe2o3_hip_sys::HIP_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH,
                device,
            )
        })?;
        Ok(supported)
    }

    unsafe fn launch(
        &self,
        function: &GpuFunction,
        config: LaunchConfig,
        stream: &Stream,
        params: &mut KernelParams,
    ) -> Result<()> {
        check(unsafe {
            fe2o3_hip_sys::hipModuleLaunchCooperativeKernel(
                function.raw(),
                config.grid_dim.0,
                config.grid_dim.1,
                config.grid_dim.2,
                config.block_dim.0,
                config.block_dim.1,
                config.block_dim.2,
                config.shared_mem_bytes,
                stream.raw(),
                params.as_mut_ptr(),
            )
        })
    }
}

/// Live evidence that one exact context wrapper's HIP device reported support
/// for a single-device cooperative launch.
///
/// This value carries no function, stream, occupancy, argument, memory, or
/// launch authority. In particular, a device ordinal or a manifest declaration
/// cannot construct it.
#[derive(Debug)]
pub struct CooperativeLaunchCapability {
    context: Arc<GpuContext>,
}

impl CooperativeLaunchCapability {
    pub fn device_id(&self) -> i32 {
        self.context.device_id()
    }

    pub fn is_for_context(&self, context: &Arc<GpuContext>) -> bool {
        Arc::ptr_eq(&self.context, context)
    }
}

impl GpuContext {
    /// Queries HIP for single-device cooperative-launch support on this exact
    /// live context wrapper.
    pub fn observe_cooperative_launch(
        self: &Arc<Self>,
    ) -> std::result::Result<CooperativeLaunchCapability, CooperativeCapabilityError> {
        observe_cooperative_launch(self, &HipCooperativeBackend)
    }
}

fn observe_cooperative_launch<B: CooperativeBackend>(
    context: &Arc<GpuContext>,
    backend: &B,
) -> std::result::Result<CooperativeLaunchCapability, CooperativeCapabilityError> {
    backend
        .bind(context)
        .map_err(CooperativeCapabilityError::Hip)?;
    match backend
        .cooperative_launch(context.device_id())
        .map_err(CooperativeCapabilityError::Hip)?
    {
        1 => Ok(CooperativeLaunchCapability {
            context: context.clone(),
        }),
        0 => Err(CooperativeCapabilityError::Unsupported {
            device: context.device_id(),
        }),
        value => Err(CooperativeCapabilityError::InvalidCapabilityValue {
            device: context.device_id(),
            value,
        }),
    }
}

/// Enqueues a raw single-device HIP cooperative kernel launch.
///
/// This function performs no occupancy calculation. HIP remains the final
/// authority for whether all requested workgroups can reside concurrently.
/// Higher-level safe admission currently permits only one workgroup.
///
/// # Safety
///
/// The caller must establish every obligation of
/// [`crate::launch_kernel_on_stream`], prove that `function` belongs to the
/// exact context of `stream`, and retain all reachable resources through
/// completion. The kernel must be valid for cooperative execution. A
/// successful return does not prove race freedom, memory safety, completion,
/// or kernel semantics.
pub unsafe fn launch_cooperative_kernel_on_stream(
    function: &GpuFunction,
    config: LaunchConfig,
    stream: &Stream,
    params: &mut KernelParams,
) -> Result<()> {
    stream.context().bind_to_thread()?;
    unsafe { HipCooperativeBackend.launch(function, config, stream, params) }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CooperativeCapabilityError {
    Unsupported { device: i32 },
    InvalidCapabilityValue { device: i32, value: i32 },
    Hip(Error),
}

impl fmt::Display for CooperativeCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { device } => {
                write!(
                    formatter,
                    "HIP device {device} does not support cooperative launch"
                )
            }
            Self::InvalidCapabilityValue { device, value } => write!(
                formatter,
                "HIP device {device} returned invalid cooperative-launch capability {value}"
            ),
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CooperativeCapabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            Self::Unsupported { .. } | Self::InvalidCapabilityValue { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockBackend {
        capability: Result<i32>,
        observed_devices: Mutex<Vec<i32>>,
    }

    impl CooperativeBackend for MockBackend {
        fn bind(&self, _context: &GpuContext) -> Result<()> {
            Ok(())
        }

        fn cooperative_launch(&self, device: i32) -> Result<i32> {
            self.observed_devices.lock().unwrap().push(device);
            match &self.capability {
                Ok(value) => Ok(*value),
                Err(_) => Err(Error::SizeOverflow),
            }
        }

        unsafe fn launch(
            &self,
            _function: &GpuFunction,
            _config: LaunchConfig,
            _stream: &Stream,
            _params: &mut KernelParams,
        ) -> Result<()> {
            unreachable!("query tests never launch")
        }
    }

    fn context(device: i32) -> Arc<GpuContext> {
        Arc::new(GpuContext::for_test(device))
    }

    #[test]
    fn live_positive_observation_retains_exact_context() {
        let context = context(3);
        let backend = MockBackend {
            capability: Ok(1),
            observed_devices: Mutex::new(Vec::new()),
        };

        let capability = observe_cooperative_launch(&context, &backend).unwrap();

        assert_eq!(capability.device_id(), 3);
        assert!(capability.is_for_context(&context));
        assert_eq!(*backend.observed_devices.lock().unwrap(), [3]);
    }

    #[test]
    fn declaration_or_other_wrapper_does_not_match() {
        let observed_context = context(3);
        let other = context(3);
        let backend = MockBackend {
            capability: Ok(1),
            observed_devices: Mutex::new(Vec::new()),
        };

        let capability = observe_cooperative_launch(&observed_context, &backend).unwrap();

        assert!(!capability.is_for_context(&other));
    }

    #[test]
    fn unsupported_and_malformed_values_fail_closed() {
        for (value, expected) in [(0, "does not support"), (2, "invalid")] {
            let error = observe_cooperative_launch(
                &context(0),
                &MockBackend {
                    capability: Ok(value),
                    observed_devices: Mutex::new(Vec::new()),
                },
            )
            .unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn query_errors_do_not_mint_capability() {
        let error = observe_cooperative_launch(
            &context(0),
            &MockBackend {
                capability: Err(Error::SizeOverflow),
                observed_devices: Mutex::new(Vec::new()),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CooperativeCapabilityError::Hip(Error::SizeOverflow)
        ));
    }
}
