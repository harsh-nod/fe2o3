use crate::{DeviceCopy, DevicePtr, Error, GpuContext, Result, Stream, check};
use core::ffi::c_void;
use fe2o3_completion::{CompletionError, complete_borrowed, complete_owned};
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
    /// Allocates `len` elements and fills them with zero on `stream`.
    ///
    /// The function synchronizes the stream before returning, including after
    /// an enqueue error. If synchronization cannot establish completion, the
    /// allocation is leaked rather than freed while HIP may still use it. A
    /// failed enqueue plus failed recovery reports both errors.
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
        complete_owned(
            buffer,
            || check(unsafe { fe2o3_hip_sys::hipMemsetAsync(raw, 0, size, stream.raw()) }),
            || stream.synchronize(),
        )
        .map_err(completion_error)
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
    /// copy is attempted, the function waits for that stream before returning,
    /// so the caller may immediately mutate or drop `values`. An enqueue error
    /// is followed by a recovery synchronization before destination cleanup. If
    /// completion remains ambiguous, the process aborts rather than release the
    /// borrow of `values`. There is intentionally no safe nonblocking borrowed
    /// upload.
    pub fn from_host(stream: &Stream, values: &[T]) -> Result<Self> {
        let size = byte_len::<T>(values.len())?;
        let buffer = Self::zeroed(stream, values.len())?;
        if size != 0 {
            complete_borrowed(
                || {
                    check(unsafe {
                        fe2o3_hip_sys::hipMemcpyAsync(
                            buffer.ptr.cast::<c_void>(),
                            values.as_ptr().cast::<c_void>(),
                            size,
                            fe2o3_hip_sys::HIP_MEMCPY_HOST_TO_DEVICE,
                            stream.raw(),
                        )
                    })
                },
                || stream.synchronize(),
            )
            .map_err(completion_error)?;
        }
        Ok(buffer)
    }

    /// Copies the buffer to host memory after validating the stream's device.
    ///
    /// The returned vector is initialized only after stream synchronization.
    /// An enqueue error is followed by a recovery synchronization before host
    /// allocation cleanup. If completion remains ambiguous, the process aborts
    /// rather than release the borrow of this device buffer.
    pub fn to_host_vec(&self, stream: &Stream) -> Result<Vec<T>> {
        ensure_same_device(self.context.device_id(), stream.context().device_id())?;
        stream.context().bind_to_thread()?;
        let size = byte_len::<T>(self.len)?;
        let mut values = Vec::<T>::with_capacity(self.len);
        if size != 0 {
            complete_borrowed(
                || {
                    check(unsafe {
                        fe2o3_hip_sys::hipMemcpyAsync(
                            values.as_mut_ptr().cast::<c_void>(),
                            self.ptr.cast::<c_void>(),
                            size,
                            fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_HOST,
                            stream.raw(),
                        )
                    })
                },
                || stream.synchronize(),
            )
            .map_err(completion_error)?;
        }
        // SAFETY: a successful copy and synchronization initialized every
        // non-ZST element. ZSTs require no backing bytes, and `DeviceCopy`
        // guarantees every bit pattern is valid.
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

fn completion_error(error: CompletionError<Error, Error>) -> Error {
    match error {
        CompletionError::Operation(error) | CompletionError::Synchronization(error) => error,
        CompletionError::OperationAndSynchronization {
            operation,
            synchronization,
        } => Error::OperationRecoveryFailed {
            operation: Box::new(operation),
            synchronization: Box::new(synchronization),
        },
    }
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

#[cfg(test)]
mod tests {
    use super::{DeviceBuffer, byte_len, completion_error, ensure_same_device};
    use crate::{Error, GpuContext};
    use fe2o3_completion::CompletionError;

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
    fn empty_and_zst_buffers_have_no_transfer_bytes() {
        assert_eq!(byte_len::<u32>(0).unwrap(), 0);
        assert_eq!(byte_len::<[u8; 0]>(usize::MAX).unwrap(), 0);
    }

    #[test]
    fn dual_completion_failure_maps_both_errors() {
        let error = completion_error(CompletionError::OperationAndSynchronization {
            operation: Error::SizeOverflow,
            synchronization: Error::DeviceMismatch {
                buffer_device: 3,
                stream_device: 4,
            },
        });

        assert!(matches!(
            error,
            Error::OperationRecoveryFailed {
                operation,
                synchronization,
            } if matches!(*operation, Error::SizeOverflow)
                && matches!(
                    *synchronization,
                    Error::DeviceMismatch {
                        buffer_device: 3,
                        stream_device: 4
                    }
                )
        ));
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
