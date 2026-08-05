use core::fmt;
use core::marker::PhantomData;

/// Type-level index space for the logical one-dimensional launch index.
#[derive(Debug)]
pub enum Index1D {}

/// Type-level index space for a row-major two-dimensional launch.
///
/// Encoding the row stride in the type prevents a witness derived for one
/// layout from indexing a view declared for another layout.
#[derive(Debug)]
pub enum Index2D<const ROW_STRIDE: usize> {}

/// A non-duplicable index witness for the current device invocation.
///
/// `IndexSpace` identifies the mapping from invocation coordinates to the
/// flattened element index. The marker fields are zero-sized, so the device
/// representation remains one `usize`.
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_thread_index"]
pub struct ThreadIndex<IndexSpace = Index1D> {
    raw: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<IndexSpace> ThreadIndex<IndexSpace> {
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_get"]
    pub fn get(&self) -> usize {
        self.raw
    }

    #[rustc_diagnostic_item = "fe2o3_device_thread_index_offset"]
    pub fn offset(&self, offset: usize) -> usize {
        self.raw + offset
    }

    #[rustc_diagnostic_item = "fe2o3_device_thread_index_offset_signed"]
    pub fn offset_signed(&self, offset: isize) -> usize {
        self.raw.wrapping_add_signed(offset)
    }

    #[rustc_diagnostic_item = "fe2o3_device_thread_index_stride"]
    pub fn stride(&self, stride: usize) -> usize {
        self.raw.wrapping_mul(stride)
    }

    #[rustc_diagnostic_item = "fe2o3_device_thread_index_stride_offset"]
    pub fn stride_offset(&self, stride: usize, offset: isize) -> usize {
        self.raw.wrapping_mul(stride).wrapping_add_signed(offset)
    }

    pub fn in_bounds(&self, len: usize) -> bool {
        self.raw < len
    }
}

impl<IndexSpace> fmt::Debug for ThreadIndex<IndexSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ThreadIndex")
            .field(&self.raw)
            .finish()
    }
}

/// Returns the current invocation's zero-based ID in the logical 1D launch.
///
/// This is a target-neutral device intrinsic. A backend may derive it from
/// physical workgroup and local-invocation coordinates, but callers do not
/// observe that mapping.
#[inline(never)]
pub fn global_id_1d() -> usize {
    unreachable!("global_id_1d must be lowered by the fe2o3 backend")
}

/// Returns the number of invocations in the logical 1D launch.
///
/// This is the logical global extent, independent of how a backend partitions
/// the launch into physical workgroups.
#[inline(never)]
pub fn launch_extent_1d() -> usize {
    unreachable!("launch_extent_1d must be lowered by the fe2o3 backend")
}

#[inline(always)]
#[rustc_diagnostic_item = "fe2o3_device_thread_index_1d"]
pub fn index_1d() -> ThreadIndex {
    ThreadIndex {
        raw: global_id_1d(),
        _index_space: PhantomData,
        _not_send_sync: PhantomData,
    }
}

/// Returns the current invocation's row-major index for a static row stride.
///
/// A zero row stride cannot describe an injective two-dimensional mapping, so
/// it produces no witness.
#[inline(always)]
pub fn index_2d<const ROW_STRIDE: usize>() -> Option<ThreadIndex<Index2D<ROW_STRIDE>>> {
    let row = (block_idx_y() as usize)
        .checked_mul(block_dim_y() as usize)?
        .checked_add(thread_idx_y() as usize)?;
    let col = (block_idx_x() as usize)
        .checked_mul(block_dim_x() as usize)?
        .checked_add(thread_idx_x() as usize)?;
    if ROW_STRIDE != 0 && col < ROW_STRIDE {
        let raw = row.checked_mul(ROW_STRIDE)?.checked_add(col)?;
        Some(ThreadIndex {
            raw,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        })
    } else {
        None
    }
}

#[inline(always)]
pub fn index_2d_row() -> usize {
    (block_idx_y() * block_dim_y() + thread_idx_y()) as usize
}

#[inline(always)]
pub fn index_2d_col() -> usize {
    (block_idx_x() * block_dim_x() + thread_idx_x()) as usize
}

#[inline(never)]
pub fn thread_idx_x() -> u32 {
    unreachable!("thread_idx_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn thread_idx_y() -> u32 {
    unreachable!("thread_idx_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn thread_idx_z() -> u32 {
    unreachable!("thread_idx_z must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_idx_x() -> u32 {
    unreachable!("block_idx_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_idx_y() -> u32 {
    unreachable!("block_idx_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_idx_z() -> u32 {
    unreachable!("block_idx_z must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_dim_x() -> u32 {
    unreachable!("block_dim_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_dim_y() -> u32 {
    unreachable!("block_dim_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
pub fn block_dim_z() -> u32 {
    unreachable!("block_dim_z must be lowered by the fe2o3 backend")
}

#[cfg(test)]
mod tests {
    use super::{Index1D, Index2D, ThreadIndex};
    use core::mem::{align_of, size_of};

    #[test]
    fn index_space_markers_do_not_change_the_witness_abi() {
        assert_eq!(size_of::<ThreadIndex<Index1D>>(), size_of::<usize>());
        assert_eq!(align_of::<ThreadIndex<Index1D>>(), align_of::<usize>());
        assert_eq!(size_of::<ThreadIndex<Index2D<64>>>(), size_of::<usize>());
        assert_eq!(align_of::<ThreadIndex<Index2D<64>>>(), align_of::<usize>());
    }
}
