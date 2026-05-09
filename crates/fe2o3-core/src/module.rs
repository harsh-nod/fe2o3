use crate::{GpuContext, Result, check};
use core::ffi::c_void;
use std::ffi::CString;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct GpuModule {
    raw: fe2o3_hip_sys::hipModule_t,
    context: Arc<GpuContext>,
}

unsafe impl Send for GpuModule {}
unsafe impl Sync for GpuModule {}

#[derive(Clone, Debug)]
pub struct GpuFunction {
    raw: fe2o3_hip_sys::hipFunction_t,
    #[allow(dead_code)]
    module: Arc<GpuModule>,
}

unsafe impl Send for GpuFunction {}
unsafe impl Sync for GpuFunction {}

impl GpuContext {
    pub fn load_module_from_file(
        self: &Arc<Self>,
        path: impl AsRef<Path>,
    ) -> Result<Arc<GpuModule>> {
        self.bind_to_thread()?;
        let path = CString::new(path.as_ref().to_string_lossy().as_bytes())?;
        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipModuleLoad(&mut raw, path.as_ptr()) })?;
        Ok(Arc::new(GpuModule {
            raw,
            context: self.clone(),
        }))
    }

    pub fn load_module_from_bytes(self: &Arc<Self>, image: &[u8]) -> Result<Arc<GpuModule>> {
        self.bind_to_thread()?;
        let mut raw = core::ptr::null_mut();
        check(unsafe {
            fe2o3_hip_sys::hipModuleLoadData(&mut raw, image.as_ptr().cast::<c_void>())
        })?;
        Ok(Arc::new(GpuModule {
            raw,
            context: self.clone(),
        }))
    }
}

impl GpuModule {
    pub fn load_function(self: &Arc<Self>, name: &str) -> Result<GpuFunction> {
        self.context.bind_to_thread()?;
        let name = CString::new(name)?;
        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipModuleGetFunction(&mut raw, self.raw, name.as_ptr()) })?;
        Ok(GpuFunction {
            raw,
            module: self.clone(),
        })
    }
}

impl Drop for GpuModule {
    fn drop(&mut self) {
        let _ = self.context.bind_to_thread();
        let _ = check(unsafe { fe2o3_hip_sys::hipModuleUnload(self.raw) });
    }
}

impl GpuFunction {
    pub unsafe fn raw(&self) -> fe2o3_hip_sys::hipFunction_t {
        self.raw
    }
}
