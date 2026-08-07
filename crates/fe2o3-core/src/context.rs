use crate::{ContextIdentity, ObservedDeviceTarget, Result, Stream, check};
use std::sync::Arc;

#[derive(Debug)]
pub struct GpuContext {
    device_id: i32,
    identity: ContextIdentity,
}

impl GpuContext {
    pub fn new(device_id: i32) -> Result<Arc<Self>> {
        check(unsafe { fe2o3_hip_sys::hipInit(0) })?;

        let mut count = 0;
        check(unsafe { fe2o3_hip_sys::hipGetDeviceCount(&mut count) })?;
        if device_id < 0 || device_id >= count {
            return Err(crate::Error::NoDevice {
                requested: device_id,
                count,
            });
        }

        check(unsafe { fe2o3_hip_sys::hipSetDevice(device_id) })?;
        Ok(Arc::new(Self {
            device_id,
            identity: ContextIdentity::fresh(device_id),
        }))
    }

    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Exact process-local identity of this context wrapper.
    ///
    /// This value distinguishes independently created wrappers for the same
    /// HIP device. It is descriptive and grants no native context authority.
    pub const fn identity(&self) -> ContextIdentity {
        self.identity
    }

    /// Obtains validated device facts required by trusted loading paths.
    ///
    /// Basic context and raw runtime operations do not require this observation,
    /// so builds without HIP headers and newly introduced processors remain
    /// usable through existing unsafe APIs. Safe artifact loading must call this
    /// method and fail closed when authoritative discovery is unavailable.
    pub fn observe_target(&self) -> Result<ObservedDeviceTarget> {
        ObservedDeviceTarget::query_hip(self.device_id)
    }

    pub fn bind_to_thread(&self) -> Result<()> {
        check(unsafe { fe2o3_hip_sys::hipSetDevice(self.device_id) })
    }

    pub fn default_stream(self: &Arc<Self>) -> Stream {
        Stream::default_for(self.clone())
    }

    pub fn create_stream(self: &Arc<Self>) -> Result<Stream> {
        Stream::create(self.clone())
    }

    #[cfg(test)]
    pub(crate) fn for_test(device_id: i32) -> Self {
        Self {
            device_id,
            identity: ContextIdentity::fresh(device_id),
        }
    }
}
