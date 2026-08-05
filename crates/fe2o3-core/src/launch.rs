use crate::{GpuFunction, Result, Stream, check};
use core::any::Any;
use core::ffi::c_void;
use core::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub struct LaunchConfig {
    pub grid_dim: (u32, u32, u32),
    pub block_dim: (u32, u32, u32),
    pub shared_mem_bytes: u32,
}

impl LaunchConfig {
    pub fn for_num_elems(n: u32) -> Self {
        const DEFAULT_BLOCK_SIZE: u32 = 256;
        Self {
            grid_dim: (n.div_ceil(DEFAULT_BLOCK_SIZE), 1, 1),
            block_dim: (DEFAULT_BLOCK_SIZE, 1, 1),
            shared_mem_bytes: 0,
        }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct DevicePtr<T> {
    raw: *mut T,
    _marker: PhantomData<T>,
}

impl<T> Clone for DevicePtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for DevicePtr<T> {}

impl<T> DevicePtr<T> {
    pub fn from_raw(raw: *mut T) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn as_raw(self) -> *mut T {
        self.raw
    }
}

/// Owns the host-side values whose addresses HIP reads during launch.
#[derive(Default)]
pub struct KernelParams {
    owned: Vec<Box<dyn Any>>,
    params: Vec<*mut c_void>,
}

impl KernelParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<T>(&mut self, value: T)
    where
        T: Copy + 'static,
    {
        let mut owned = Box::new(value);
        let ptr = (&mut *owned) as *mut T as *mut c_void;
        self.params.push(ptr);
        self.owned.push(owned);
    }

    pub fn len(&self) -> usize {
        self.params.len()
    }

    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    pub fn as_mut_ptr(&mut self) -> *mut *mut c_void {
        self.params.as_mut_ptr()
    }
}

/// Enqueues a raw HIP kernel launch on `stream`.
///
/// # Safety
///
/// The caller must ensure that `function` belongs to a module compatible with
/// the stream's context and that `params` exactly matches the function's ABI in
/// field count, order, type, size, and alignment. All device addresses reachable
/// through `params` must be valid for the kernel's accesses and remain alive
/// until the stream completes the launch. The module that owns `function` must
/// also remain loaded until that completion. The caller must uphold the kernel's
/// aliasing and synchronization requirements and provide valid grid, block, and
/// shared-memory dimensions in `config`.
pub unsafe fn launch_kernel_on_stream(
    function: &GpuFunction,
    config: LaunchConfig,
    stream: &Stream,
    params: &mut KernelParams,
) -> Result<()> {
    stream.context().bind_to_thread()?;
    check(unsafe {
        fe2o3_hip_sys::hipModuleLaunchKernel(
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
            core::ptr::null_mut(),
        )
    })
}
