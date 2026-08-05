use crate::{DeviceCopy, Error, GpuContext, Result, check};
use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::Arc;

/// An initialized, page-locked host allocation managed by HIP.
///
/// Safe constructors finish initialization before exposing slices. Raw
/// asynchronous access remains unsafe; the owned and scoped device-operation
/// APIs retain or borrow this allocation until completion before safe Rust can
/// access or destroy it again.
///
/// Types without an audited [`DeviceCopy`] implementation are rejected:
///
/// ```compile_fail
/// use fe2o3_core::PinnedHostBuffer;
///
/// fn requires_device_copy(_: Option<PinnedHostBuffer<bool>>) {}
/// ```
#[derive(Debug)]
pub struct PinnedHostBuffer<T: DeviceCopy> {
    ptr: *mut T,
    len: usize,
    context: Arc<GpuContext>,
}

// Safe access follows ordinary slice borrowing. Raw access is unsafe and
// carries the synchronization obligations documented on `raw_mut_ptr`.
unsafe impl<T: DeviceCopy> Send for PinnedHostBuffer<T> {}
unsafe impl<T: DeviceCopy> Sync for PinnedHostBuffer<T> {}

impl<T: DeviceCopy> PinnedHostBuffer<T> {
    /// Allocates pinned host memory and initializes it from `values`.
    pub fn from_slice(context: &Arc<GpuContext>, values: &[T]) -> Result<Self> {
        let buffer = Self::allocate(context, values.len())?;
        if !values.is_empty() && core::mem::size_of::<T>() != 0 {
            // SAFETY: the non-empty allocation has room for `values.len()`
            // elements, and the source and fresh allocation do not overlap.
            unsafe {
                core::ptr::copy_nonoverlapping(values.as_ptr(), buffer.ptr, values.len());
            }
        }
        Ok(buffer)
    }

    /// Allocates `len` pinned elements and initializes each one to `value`.
    pub fn filled(context: &Arc<GpuContext>, len: usize, value: T) -> Result<Self> {
        let buffer = Self::allocate(context, len)?;
        if core::mem::size_of::<T>() != 0 {
            for index in 0..len {
                // SAFETY: `allocate` reserved `len` elements, and every slot is
                // written exactly once before the buffer is returned.
                unsafe {
                    buffer.ptr.add(index).write(value);
                }
            }
        }
        Ok(buffer)
    }

    fn allocate(context: &Arc<GpuContext>, len: usize) -> Result<Self> {
        let size = allocation_size::<T>(len)?;
        if size == 0 {
            return Ok(Self {
                ptr: core::ptr::null_mut(),
                len,
                context: context.clone(),
            });
        }

        context.bind_to_thread()?;
        let mut raw = core::ptr::null_mut();
        check(unsafe {
            fe2o3_hip_sys::hipHostMalloc(&mut raw, size, fe2o3_hip_sys::HIP_HOST_MALLOC_DEFAULT)
        })?;
        let ptr = NonNull::<T>::new(raw.cast::<T>()).ok_or(Error::NullHostAllocation)?;
        Ok(Self {
            ptr: ptr.as_ptr(),
            len,
            context: context.clone(),
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: constructors initialize every element before returning.
        // `slice_ptr` supplies an aligned non-null pointer for zero-byte cases.
        unsafe { core::slice::from_raw_parts(self.slice_ptr(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len;
        let ptr = self.slice_ptr();
        // SAFETY: the allocation is uniquely borrowed and fully initialized.
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }

    pub fn context(&self) -> &Arc<GpuContext> {
        &self.context
    }

    /// Returns the allocation's raw host pointer, or null for zero bytes.
    ///
    /// # Safety
    ///
    /// The caller must not free the pointer or use it after this buffer is
    /// dropped. Any asynchronous HIP operation using it must finish before the
    /// allocation is read, mutably borrowed, or dropped. The caller must also
    /// prevent concurrent access that violates Rust's aliasing rules. If the
    /// pointer is used from another device, work on every such device must be
    /// complete before drop; `hipHostFree` only synchronizes the current device.
    pub unsafe fn raw_mut_ptr(&self) -> *mut T {
        self.ptr
    }

    fn slice_ptr(&self) -> *mut T {
        if self.ptr.is_null() {
            NonNull::<T>::dangling().as_ptr()
        } else {
            self.ptr
        }
    }
}

impl<T: DeviceCopy> Drop for PinnedHostBuffer<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        // hipHostFree performs an implicit device synchronization. Bind the
        // allocation's owning device first; if that fails, leak rather than
        // synchronizing and freeing under an unrelated current device.
        if self.context.bind_to_thread().is_ok() {
            let _ = check(unsafe { fe2o3_hip_sys::hipHostFree(self.ptr.cast::<c_void>()) });
        }
    }
}

fn allocation_size<T>(len: usize) -> Result<usize> {
    let size = len
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::SizeOverflow)?;
    if size > isize::MAX as usize {
        return Err(Error::SizeOverflow);
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::{PinnedHostBuffer, allocation_size};
    use crate::{DeviceBuffer, Error, GpuContext};

    #[test]
    fn allocation_size_rejects_overflow_and_slice_incompatible_sizes() {
        assert_eq!(allocation_size::<u32>(4).unwrap(), 16);
        assert_eq!(allocation_size::<[u8; 0]>(usize::MAX).unwrap(), 0);
        assert!(matches!(
            allocation_size::<u16>(usize::MAX),
            Err(Error::SizeOverflow)
        ));
        assert!(matches!(
            allocation_size::<u8>((isize::MAX as usize) + 1),
            Err(Error::SizeOverflow)
        ));
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn primitive_and_array_values_round_trip_through_device_memory() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.default_stream();

        let mut primitives = PinnedHostBuffer::from_slice(&context, &[1_u32, 2, 3, 4])?;
        primitives.as_mut_slice()[2] = 30;
        let primitive_device = DeviceBuffer::from_host(&stream, primitives.as_slice())?;
        assert_eq!(primitive_device.to_host_vec(&stream)?, [1, 2, 30, 4]);

        let arrays = PinnedHostBuffer::from_slice(&context, &[[1_u16, 2], [3, 4]])?;
        let array_device = DeviceBuffer::from_host(&stream, arrays.as_slice())?;
        assert_eq!(array_device.to_host_vec(&stream)?, [[1, 2], [3, 4]]);
        Ok(())
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn filled_empty_and_zero_sized_buffers_are_initialized() -> crate::Result<()> {
        let context = GpuContext::new(0)?;

        let filled = PinnedHostBuffer::filled(&context, 3, -7_i32)?;
        assert_eq!(filled.as_slice(), [-7, -7, -7]);

        let empty = PinnedHostBuffer::<u64>::from_slice(&context, &[])?;
        assert!(empty.is_empty());
        assert!(unsafe { empty.raw_mut_ptr() }.is_null());

        let zero_sized = PinnedHostBuffer::filled(&context, 5, [0_u8; 0])?;
        assert_eq!(zero_sized.len(), 5);
        assert_eq!(zero_sized.as_slice(), [[0_u8; 0]; 5]);
        assert!(unsafe { zero_sized.raw_mut_ptr() }.is_null());
        Ok(())
    }
}
