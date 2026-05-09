use crate::{Result, Stream, check};
use std::sync::Arc;

#[derive(Debug)]
pub struct GpuContext {
    device_id: i32,
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
        Ok(Arc::new(Self { device_id }))
    }

    pub fn device_id(&self) -> i32 {
        self.device_id
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
}
