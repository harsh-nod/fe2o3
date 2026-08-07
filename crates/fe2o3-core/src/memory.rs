use crate::{DeviceCopy, DevicePtr, Error, GpuContext, Result, Stream, check};
use core::ffi::c_void;
use core::ops::{Bound, RangeBounds};
use fe2o3_completion::{CompletionError, complete_borrowed, complete_owned};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DEVICE_BUFFER_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Exact process-local identity of one [`DeviceBuffer`] allocation.
///
/// This copyable value is descriptive only. It grants no ownership, memory
/// access, launch, or cleanup authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceBufferIdentity(u64);

impl DeviceBufferIdentity {
    fn fresh() -> Self {
        let identity = NEXT_DEVICE_BUFFER_IDENTITY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .unwrap_or_else(|_| panic!("process-local device-buffer identity space exhausted"));
        Self(identity)
    }
}

/// Failure to select a checked half-open region of a [`DeviceBuffer`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceBufferRangeError {
    BoundOverflow,
    OutOfBounds {
        start: usize,
        end: usize,
        allocation_len: usize,
    },
    AllocationSizeOverflow,
    AllocationAddressOverflow,
    NullAllocation,
}

impl core::fmt::Display for DeviceBufferRangeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BoundOverflow => write!(formatter, "device-buffer range bound overflowed"),
            Self::OutOfBounds {
                start,
                end,
                allocation_len,
            } => write!(
                formatter,
                "device-buffer range {start}..{end} is outside allocation length {allocation_len}"
            ),
            Self::AllocationSizeOverflow => {
                write!(formatter, "device-buffer allocation size overflowed")
            }
            Self::AllocationAddressOverflow => {
                write!(formatter, "device-buffer allocation address overflowed")
            }
            Self::NullAllocation => write!(
                formatter,
                "non-empty device-buffer allocation has a null address"
            ),
        }
    }
}

impl std::error::Error for DeviceBufferRangeError {}

mod region_sealed {
    pub trait Sealed {}
}

/// Common allocation and selected-region metadata for device buffers and views.
///
/// This interface is sealed so safe downstream code can consume trusted
/// provenance without manufacturing it from arbitrary raw pointers.
#[doc(hidden)]
pub trait DeviceBufferRegion<T: DeviceCopy>: region_sealed::Sealed {
    fn allocation_identity(&self) -> DeviceBufferIdentity;
    fn context(&self) -> &Arc<GpuContext>;
    fn allocation_device_ptr(&self) -> DevicePtr<T>;
    fn allocation_len(&self) -> usize;
    fn region_device_ptr(&self) -> DevicePtr<T>;
    fn region_len(&self) -> usize;
}

#[derive(Debug)]
pub struct DeviceBuffer<T: DeviceCopy> {
    ptr: *mut T,
    len: usize,
    context: Arc<GpuContext>,
    identity: DeviceBufferIdentity,
}

/// Shared, borrow-typed contiguous region of a [`DeviceBuffer`].
pub struct DeviceBufferView<'allocation, T: DeviceCopy> {
    buffer: &'allocation DeviceBuffer<T>,
    ptr: *mut T,
    len: usize,
}

/// Exclusive, borrow-typed contiguous region of a [`DeviceBuffer`].
pub struct DeviceBufferViewMut<'allocation, T: DeviceCopy> {
    buffer: &'allocation mut DeviceBuffer<T>,
    ptr: *mut T,
    len: usize,
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
                identity: DeviceBufferIdentity::fresh(),
            });
        }

        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipMalloc(&mut raw, size) })?;
        let buffer = Self {
            ptr: raw.cast::<T>(),
            len,
            context,
            identity: DeviceBufferIdentity::fresh(),
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

    pub fn context(&self) -> &Arc<GpuContext> {
        &self.context
    }

    pub fn allocation_identity(&self) -> DeviceBufferIdentity {
        self.identity
    }

    /// Selects a shared region after normalizing and checking its half-open
    /// element range.
    pub fn view<R: RangeBounds<usize>>(
        &self,
        range: R,
    ) -> core::result::Result<DeviceBufferView<'_, T>, DeviceBufferRangeError> {
        let region = checked_region(self.ptr, self.len, range)?;
        Ok(DeviceBufferView {
            buffer: self,
            ptr: region.ptr,
            len: region.len,
        })
    }

    /// Selects an exclusive region after normalizing and checking its
    /// half-open element range.
    pub fn view_mut<R: RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> core::result::Result<DeviceBufferViewMut<'_, T>, DeviceBufferRangeError> {
        let region = checked_region(self.ptr, self.len, range)?;
        Ok(DeviceBufferViewMut {
            buffer: self,
            ptr: region.ptr,
            len: region.len,
        })
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

impl<T: DeviceCopy> region_sealed::Sealed for DeviceBuffer<T> {}
impl<T: DeviceCopy> region_sealed::Sealed for DeviceBufferView<'_, T> {}
impl<T: DeviceCopy> region_sealed::Sealed for DeviceBufferViewMut<'_, T> {}

impl<T: DeviceCopy> DeviceBufferRegion<T> for DeviceBuffer<T> {
    fn allocation_identity(&self) -> DeviceBufferIdentity {
        self.identity
    }

    fn context(&self) -> &Arc<GpuContext> {
        &self.context
    }

    fn allocation_device_ptr(&self) -> DevicePtr<T> {
        self.as_device_ptr()
    }

    fn allocation_len(&self) -> usize {
        self.len
    }

    fn region_device_ptr(&self) -> DevicePtr<T> {
        self.as_device_ptr()
    }

    fn region_len(&self) -> usize {
        self.len
    }
}

macro_rules! impl_device_buffer_view {
    ($view:ident) => {
        impl<T: DeviceCopy> core::fmt::Debug for $view<'_, T> {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter
                    .debug_struct(stringify!($view))
                    .field("allocation_identity", &self.buffer.identity)
                    .field("allocation_len", &self.buffer.len)
                    .field("region_ptr", &self.ptr)
                    .field("region_len", &self.len)
                    .finish_non_exhaustive()
            }
        }

        impl<T: DeviceCopy> $view<'_, T> {
            pub fn len(&self) -> usize {
                self.len
            }

            pub fn is_empty(&self) -> bool {
                self.len == 0
            }

            pub fn as_device_ptr(&self) -> DevicePtr<T> {
                DevicePtr::from_raw(self.ptr)
            }

            pub fn context(&self) -> &Arc<GpuContext> {
                &self.buffer.context
            }

            pub fn allocation_identity(&self) -> DeviceBufferIdentity {
                self.buffer.identity
            }
        }

        impl<T: DeviceCopy> DeviceBufferRegion<T> for $view<'_, T> {
            fn allocation_identity(&self) -> DeviceBufferIdentity {
                self.buffer.identity
            }

            fn context(&self) -> &Arc<GpuContext> {
                &self.buffer.context
            }

            fn allocation_device_ptr(&self) -> DevicePtr<T> {
                self.buffer.as_device_ptr()
            }

            fn allocation_len(&self) -> usize {
                self.buffer.len
            }

            fn region_device_ptr(&self) -> DevicePtr<T> {
                DevicePtr::from_raw(self.ptr)
            }

            fn region_len(&self) -> usize {
                self.len
            }
        }
    };
}

impl_device_buffer_view!(DeviceBufferView);
impl_device_buffer_view!(DeviceBufferViewMut);

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

struct CheckedRegion<T> {
    ptr: *mut T,
    len: usize,
}

fn checked_region<T, R: RangeBounds<usize>>(
    allocation_ptr: *mut T,
    allocation_len: usize,
    range: R,
) -> core::result::Result<CheckedRegion<T>, DeviceBufferRangeError> {
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(&start) => start,
        Bound::Excluded(&start) => start
            .checked_add(1)
            .ok_or(DeviceBufferRangeError::BoundOverflow)?,
    };
    let end = match range.end_bound() {
        Bound::Unbounded => allocation_len,
        Bound::Excluded(&end) => end,
        Bound::Included(&end) => end
            .checked_add(1)
            .ok_or(DeviceBufferRangeError::BoundOverflow)?,
    };
    if start > end || end > allocation_len {
        return Err(DeviceBufferRangeError::OutOfBounds {
            start,
            end,
            allocation_len,
        });
    }

    let element_size = core::mem::size_of::<T>();
    let allocation_bytes = allocation_len
        .checked_mul(element_size)
        .ok_or(DeviceBufferRangeError::AllocationSizeOverflow)?;
    if allocation_bytes != 0 && allocation_ptr.is_null() {
        return Err(DeviceBufferRangeError::NullAllocation);
    }
    allocation_ptr
        .addr()
        .checked_add(allocation_bytes)
        .ok_or(DeviceBufferRangeError::AllocationAddressOverflow)?;

    let byte_offset = start
        .checked_mul(element_size)
        .ok_or(DeviceBufferRangeError::AllocationSizeOverflow)?;
    let region_address = allocation_ptr
        .addr()
        .checked_add(byte_offset)
        .ok_or(DeviceBufferRangeError::AllocationAddressOverflow)?;
    Ok(CheckedRegion {
        ptr: allocation_ptr.with_addr(region_address),
        len: end - start,
    })
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
    use super::{
        DeviceBuffer, DeviceBufferIdentity, DeviceBufferRangeError, DeviceBufferRegion, byte_len,
        completion_error, ensure_same_device,
    };
    use crate::{DeviceCopy, Error, GpuContext};
    use core::mem::ManuallyDrop;
    use core::ops::Bound;
    use fe2o3_completion::CompletionError;
    use std::sync::Arc;

    fn test_buffer<T: DeviceCopy>(ptr: *mut T, len: usize) -> ManuallyDrop<DeviceBuffer<T>> {
        ManuallyDrop::new(DeviceBuffer {
            ptr,
            len,
            context: Arc::new(GpuContext::for_test(7)),
            identity: DeviceBufferIdentity::fresh(),
        })
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
    fn empty_and_zst_buffers_have_no_transfer_bytes() {
        assert_eq!(byte_len::<u32>(0).unwrap(), 0);
        assert_eq!(byte_len::<[u8; 0]>(usize::MAX).unwrap(), 0);
    }

    #[test]
    fn device_buffer_views_normalize_range_bounds() {
        let buffer = test_buffer(0x1000_usize as *mut u32, 8);

        let full = buffer.view(..).unwrap();
        assert_eq!(full.as_device_ptr().as_raw().addr(), 0x1000);
        assert_eq!(full.len(), 8);

        let inclusive = buffer.view(2..=4).unwrap();
        assert_eq!(inclusive.as_device_ptr().as_raw().addr(), 0x1008);
        assert_eq!(inclusive.len(), 3);

        let explicit = buffer
            .view((Bound::Excluded(1), Bound::Included(3)))
            .unwrap();
        assert_eq!(explicit.as_device_ptr().as_raw().addr(), 0x1008);
        assert_eq!(explicit.len(), 2);
    }

    #[test]
    fn device_buffer_views_reject_reversed_and_out_of_bounds_ranges() {
        let buffer = test_buffer(0x1000_usize as *mut u32, 8);

        assert_eq!(
            buffer
                .view((Bound::Included(6), Bound::Excluded(5)))
                .unwrap_err(),
            DeviceBufferRangeError::OutOfBounds {
                start: 6,
                end: 5,
                allocation_len: 8,
            }
        );
        assert_eq!(
            buffer.view(..9).unwrap_err(),
            DeviceBufferRangeError::OutOfBounds {
                start: 0,
                end: 9,
                allocation_len: 8,
            }
        );
    }

    #[test]
    fn device_buffer_views_preserve_empty_endpoint_addresses() {
        let buffer = test_buffer(0x1000_usize as *mut u32, 8);

        let at_start = buffer.view(..0).unwrap();
        assert!(at_start.is_empty());
        assert_eq!(at_start.as_device_ptr().as_raw().addr(), 0x1000);

        let at_end = buffer.view(8..).unwrap();
        assert!(at_end.is_empty());
        assert_eq!(at_end.as_device_ptr().as_raw().addr(), 0x1020);
    }

    #[test]
    fn device_buffer_views_reject_bound_size_and_address_overflow() {
        let buffer = test_buffer(0x1000_usize as *mut u32, 8);
        assert_eq!(
            buffer.view(..=usize::MAX).unwrap_err(),
            DeviceBufferRangeError::BoundOverflow
        );
        assert_eq!(
            buffer
                .view((Bound::Excluded(usize::MAX), Bound::Unbounded))
                .unwrap_err(),
            DeviceBufferRangeError::BoundOverflow
        );

        let oversized = test_buffer(core::ptr::null_mut::<u16>(), usize::MAX);
        assert_eq!(
            oversized.view(..0).unwrap_err(),
            DeviceBufferRangeError::AllocationSizeOverflow
        );

        let wrapped = test_buffer((usize::MAX - 1) as *mut u32, 1);
        assert_eq!(
            wrapped.view(..).unwrap_err(),
            DeviceBufferRangeError::AllocationAddressOverflow
        );
    }

    #[test]
    fn device_buffer_views_keep_allocation_and_region_provenance_distinct() {
        let mut buffer = test_buffer(0x2000_usize as *mut u32, 16);
        let identity = buffer.allocation_identity();
        let context = buffer.context().clone();
        let view = buffer.view_mut(3..7).unwrap();

        assert_eq!(view.allocation_identity(), identity);
        assert!(Arc::ptr_eq(view.context(), &context));
        assert_eq!(view.allocation_device_ptr().as_raw().addr(), 0x2000);
        assert_eq!(view.allocation_len(), 16);
        assert_eq!(view.region_device_ptr().as_raw().addr(), 0x200c);
        assert_eq!(view.region_len(), 4);
    }

    #[test]
    fn device_buffer_identity_distinguishes_logical_allocations_at_one_address() {
        let first = test_buffer(core::ptr::null_mut::<[u8; 0]>(), 1);
        let second = test_buffer(core::ptr::null_mut::<[u8; 0]>(), 1);

        assert_ne!(first.allocation_identity(), second.allocation_identity());
    }

    #[test]
    fn device_buffer_views_handle_zst_and_null_allocations_without_arithmetic() {
        let zst = test_buffer(core::ptr::null_mut::<[u8; 0]>(), usize::MAX);
        let view = zst.view((usize::MAX - 1)..usize::MAX).unwrap();
        assert_eq!(view.as_device_ptr().as_raw(), core::ptr::null_mut());
        assert_eq!(view.len(), 1);
        assert_eq!(view.allocation_len(), usize::MAX);

        let invalid = test_buffer(core::ptr::null_mut::<u32>(), 1);
        assert_eq!(
            invalid.view(0..0).unwrap_err(),
            DeviceBufferRangeError::NullAllocation
        );
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
