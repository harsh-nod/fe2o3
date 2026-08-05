use crate::{DeviceCopy, DevicePtr, Error, GpuContext, Result, Stream, check};
use core::ffi::c_void;
use std::sync::Arc;

#[derive(Debug)]
pub struct DeviceBuffer<T: DeviceCopy> {
    ptr: *mut T,
    len: usize,
    context: Arc<GpuContext>,
}

unsafe impl<T: DeviceCopy> Send for DeviceBuffer<T> {}
unsafe impl<T: DeviceCopy> Sync for DeviceBuffer<T> {}

impl<T: DeviceCopy> DeviceBuffer<T> {
    /// Allocates `len` elements and enqueues a zero fill on `stream`.
    ///
    /// This operation preserves stream ordering but does not synchronize. If
    /// zero-fill enqueueing fails after allocation, the allocation is dropped
    /// before the error is returned.
    pub fn zeroed(stream: &Stream, len: usize) -> Result<Self> {
        let context = stream.context().clone();
        context.bind_to_thread()?;
        let size = byte_len::<T>(len)?;
        if size == 0 {
            return Ok(Self {
                ptr: core::ptr::null_mut(),
                len,
                context,
            });
        }

        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipMalloc(&mut raw, size) })?;
        let buffer = Self {
            ptr: raw.cast::<T>(),
            len,
            context,
        };
        check(unsafe { fe2o3_hip_sys::hipMemsetAsync(raw, 0, size, stream.raw()) })?;
        Ok(buffer)
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

    /// Returns the allocation's untyped HIP device address.
    ///
    /// # Safety
    ///
    /// The caller must not dereference this pointer on the host, free it, or
    /// use it after this buffer is dropped. Device operations using the pointer
    /// must target this buffer's HIP device and uphold all access, aliasing,
    /// bounds, synchronization, and asynchronous lifetime requirements.
    pub unsafe fn raw_device_ptr(&self) -> *mut T {
        self.ptr
    }
}

impl<T: DeviceCopy> DeviceBuffer<T> {
    /// Copies `values` into a new device allocation.
    ///
    /// The borrowed upload is synchronous with respect to `stream`: after the
    /// copy is enqueued, the function waits for that stream before returning,
    /// so the caller may immediately mutate or drop `values`. On an enqueue or
    /// synchronization error, destination cleanup happens before the error is
    /// returned. There is intentionally no safe nonblocking borrowed upload.
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
            return finish_borrowed_transfer(buffer, || stream.synchronize());
        }
        Ok(buffer)
    }

    /// Copies the buffer to host memory after validating the stream's device.
    ///
    /// The returned vector is initialized only after stream synchronization.
    /// On an enqueue or synchronization error, its backing allocation is
    /// dropped before the error is returned.
    pub fn to_host_vec(&self, stream: &Stream) -> Result<Vec<T>> {
        ensure_same_device(self.context.device_id(), stream.context().device_id())?;
        stream.context().bind_to_thread()?;
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
        let mut values = finish_borrowed_transfer(values, || stream.synchronize())?;
        // SAFETY: a successful copy and synchronization initialized every
        // element, and `DeviceCopy` guarantees every bit pattern is valid.
        unsafe {
            values.set_len(self.len);
        }
        Ok(values)
    }
}

impl<T: DeviceCopy> Drop for DeviceBuffer<T> {
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

fn ensure_same_device(buffer_device: i32, stream_device: i32) -> Result<()> {
    if buffer_device == stream_device {
        Ok(())
    } else {
        Err(Error::DeviceMismatch {
            buffer_device,
            stream_device,
        })
    }
}

fn finish_borrowed_transfer<T>(value: T, synchronize: impl FnOnce() -> Result<()>) -> Result<T> {
    synchronize()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{DeviceBuffer, ensure_same_device, finish_borrowed_transfer};
    use crate::{Error, GpuContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct DropRecorder(Rc<RefCell<Vec<&'static str>>>);

    impl Drop for DropRecorder {
        fn drop(&mut self) {
            self.0.borrow_mut().push("drop");
        }
    }

    #[test]
    fn device_identity_uses_hip_device_ids() {
        assert!(ensure_same_device(2, 2).is_ok());

        assert!(matches!(
            ensure_same_device(2, 4),
            Err(Error::DeviceMismatch {
                buffer_device: 2,
                stream_device: 4,
            })
        ));
    }

    #[test]
    fn synchronization_failure_precedes_transfer_cleanup() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let value = DropRecorder(events.clone());

        let result = finish_borrowed_transfer(value, || {
            events.borrow_mut().push("synchronize");
            Err(Error::SizeOverflow)
        });

        assert!(matches!(result, Err(Error::SizeOverflow)));
        assert_eq!(*events.borrow(), ["synchronize", "drop"]);
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn arrays_round_trip_across_same_device_context_wrappers() -> crate::Result<()> {
        let upload_context = GpuContext::new(0)?;
        let upload_stream = upload_context.default_stream();
        let download_context = GpuContext::new(0)?;
        let download_stream = download_context.default_stream();
        let input = [[1_u32, 2, 3], [u32::MAX, 5, 6]];

        let buffer = DeviceBuffer::from_host(&upload_stream, &input)?;
        let output = buffer.to_host_vec(&download_stream)?;

        assert_eq!(output, input);
        Ok(())
    }

    #[test]
    #[ignore = "requires two working HIP devices"]
    fn different_device_stream_is_rejected_when_available() -> crate::Result<()> {
        let buffer_context = GpuContext::new(0)?;
        let buffer_stream = buffer_context.default_stream();
        let buffer = DeviceBuffer::<u32>::zeroed(&buffer_stream, 1)?;
        let stream_context = match GpuContext::new(1) {
            Ok(context) => context,
            Err(Error::NoDevice { .. }) => return Ok(()),
            Err(error) => return Err(error),
        };
        let stream = stream_context.default_stream();

        assert!(matches!(
            buffer.to_host_vec(&stream),
            Err(Error::DeviceMismatch {
                buffer_device: 0,
                stream_device: 1,
            })
        ));
        Ok(())
    }
}
