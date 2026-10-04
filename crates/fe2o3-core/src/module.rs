use crate::{GpuContext, Result, check};
use core::ffi::c_void;
use std::ffi::CString;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct GpuModule {
    raw: fe2o3_hip_sys::hipModule_t,
    context: Arc<GpuContext>,
    _image: Option<Box<[u8]>>,
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
    /// Loads a HIP code object from a file without validating its provenance or contract.
    ///
    /// # Safety
    ///
    /// The code object is not authenticated and its ABI, target compatibility, and
    /// semantics are not checked. The caller must trust all executable behavior in
    /// the object, including initialization during loading and finalization during
    /// unloading. `path` must name a readable, self-contained HIP code object.
    /// The caller may use only functions whose target and ABI are compatible
    /// with this context, and must uphold the existing unsafe launch contract
    /// for every launch.
    ///
    /// This is an explicit escape hatch. Validated loading will use a separate,
    /// sealed API.
    pub unsafe fn load_module_from_file_unchecked(
        self: &Arc<Self>,
        path: impl AsRef<Path>,
    ) -> Result<Arc<GpuModule>> {
        let image = std::fs::read(path)?;
        // SAFETY: this method has the same trust contract as the in-memory
        // escape hatch and retains the loaded image for the module lifetime.
        unsafe { self.load_module_from_bytes_unchecked(&image) }
    }

    /// Loads a HIP code object from memory without validating its provenance or contract.
    ///
    /// # Safety
    ///
    /// The code object is not authenticated and its ABI, target compatibility, and
    /// semantics are not checked. The caller must trust all executable behavior in
    /// the object, including initialization during loading and finalization during
    /// unloading. `image` must be a self-contained HIP code object; its bytes are
    /// retained until the module is unloaded. The caller may use only functions
    /// whose target and ABI are compatible with this context, and must uphold the
    /// existing unsafe launch contract for every launch.
    ///
    /// This is an explicit escape hatch. Validated loading will use a separate,
    /// sealed API.
    pub unsafe fn load_module_from_bytes_unchecked(
        self: &Arc<Self>,
        image: &[u8],
    ) -> Result<Arc<GpuModule>> {
        self.bind_to_thread()?;
        let image: Box<[u8]> = image.into();
        let mut raw = core::ptr::null_mut();
        check(unsafe {
            fe2o3_hip_sys::hipModuleLoadData(&mut raw, image.as_ptr().cast::<c_void>())
        })?;
        Ok(Arc::new(GpuModule {
            raw,
            context: self.clone(),
            _image: Some(image),
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
        let image = self._image.take();
        let unloaded = self.context.bind_to_thread().is_ok()
            && check(unsafe { fe2o3_hip_sys::hipModuleUnload(self.raw) }).is_ok();
        if !unloaded {
            // The native module may still refer to its image, so leak both together.
            core::mem::forget(image);
        }
    }
}

impl GpuFunction {
    /// Returns the borrowed HIP function handle.
    ///
    /// # Safety
    ///
    /// The caller must not destroy the handle or use it after this function is
    /// dropped. Any direct launch must use the module's HIP device and provide
    /// an exact kernel ABI and valid launch resources.
    pub unsafe fn raw(&self) -> fe2o3_hip_sys::hipFunction_t {
        self.raw
    }
}
