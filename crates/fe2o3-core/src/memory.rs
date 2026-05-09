use crate::{DevicePtr, GpuContext, Result, Stream, check};
use core::ffi::c_void;
use std::sync::Arc;

#[derive(Debug)]
pub struct DeviceBuffer<T> {
    ptr: *mut T,
    len: usize,
    context: Arc<GpuContext>,
}

unsafe impl<T: Send> Send for DeviceBuffer<T> {}
unsafe impl<T: Sync> Sync for DeviceBuffer<T> {}

impl<T> DeviceBuffer<T> {
    pub fn zeroed(stream: &Stream, len: usize) -> Result<Self> {
        let context = stream.context().clone();
        context.bind_to_thread()?;
        let size = byte_len::<T>(len)?;
        let ptr = if size == 0 {
            core::ptr::null_mut()
        } else {
            let mut raw = core::ptr::null_mut();
            check(unsafe { fe2o3_hip_sys::hipMalloc(&mut raw, size) })?;
            check(unsafe { fe2o3_hip_sys::hipMemsetAsync(raw, 0, size, stream.raw()) })?;
            raw.cast::<T>()
        };
        Ok(Self { ptr, len, context })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_device_ptr(&self) -> DevicePtr<T> {
        DevicePtr::from_raw(self.ptr)
    }

    pub unsafe fn raw_device_ptr(&self) -> *mut T {
        self.ptr
    }
}

impl<T: Copy + 'static> DeviceBuffer<T> {
    pub fn from_host(stream: &Stream, values: &[T]) -> Result<Self> {
        let buffer = Self::zeroed(stream, values.len())?;
        if !values.is_empty() {
            check(unsafe {
                fe2o3_hip_sys::hipMemcpyAsync(
                    buffer.ptr.cast::<c_void>(),
                    values.as_ptr().cast::<c_void>(),
                    byte_len::<T>(values.len())?,
                    fe2o3_hip_sys::HIP_MEMCPY_HOST_TO_DEVICE,
                    stream.raw(),
                )
            })?;
        }
        Ok(buffer)
    }

    pub fn to_host_vec(&self, stream: &Stream) -> Result<Vec<T>> {
        let mut values = Vec::<T>::with_capacity(self.len);
        if self.len != 0 {
            check(unsafe {
                fe2o3_hip_sys::hipMemcpyAsync(
                    values.as_mut_ptr().cast::<c_void>(),
                    self.ptr.cast::<c_void>(),
                    byte_len::<T>(self.len)?,
                    fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_HOST,
                    stream.raw(),
                )
            })?;
        }
        stream.synchronize()?;
        unsafe {
            values.set_len(self.len);
        }
        Ok(values)
    }
}

impl<T> Drop for DeviceBuffer<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            let _ = self.context.bind_to_thread();
            let _ = check(unsafe { fe2o3_hip_sys::hipFree(self.ptr.cast::<c_void>()) });
        }
    }
}

fn byte_len<T>(len: usize) -> Result<usize> {
    len.checked_mul(core::mem::size_of::<T>())
        .ok_or(crate::Error::SizeOverflow)
}
