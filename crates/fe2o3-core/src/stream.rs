use crate::{GpuContext, Result, check};
use std::sync::Arc;

#[derive(Debug)]
pub struct Stream {
    raw: fe2o3_hip_sys::hipStream_t,
    owned: bool,
    context: Arc<GpuContext>,
}

impl Stream {
    pub(crate) fn default_for(context: Arc<GpuContext>) -> Self {
        Self {
            raw: core::ptr::null_mut(),
            owned: false,
            context,
        }
    }

    pub(crate) fn create(context: Arc<GpuContext>) -> Result<Self> {
        context.bind_to_thread()?;
        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipStreamCreate(&mut raw) })?;
        Ok(Self {
            raw,
            owned: true,
            context,
        })
    }

    pub fn synchronize(&self) -> Result<()> {
        self.context.bind_to_thread()?;
        check(unsafe { fe2o3_hip_sys::hipStreamSynchronize(self.raw) })
    }

    pub fn context(&self) -> &Arc<GpuContext> {
        &self.context
    }

    pub unsafe fn raw(&self) -> fe2o3_hip_sys::hipStream_t {
        self.raw
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            let _ = self.context.bind_to_thread();
            let _ = check(unsafe { fe2o3_hip_sys::hipStreamDestroy(self.raw) });
        }
    }
}
