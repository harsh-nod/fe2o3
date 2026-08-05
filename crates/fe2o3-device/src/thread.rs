#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ThreadIndex(usize);

impl ThreadIndex {
    pub fn get(self) -> usize {
        self.0
    }

    pub fn offset(self, offset: usize) -> usize {
        self.0 + offset
    }

    pub fn offset_signed(self, offset: isize) -> usize {
        self.0.wrapping_add_signed(offset)
    }

    pub fn stride(self, stride: usize) -> usize {
        self.0.wrapping_mul(stride)
    }

    pub fn stride_offset(self, stride: usize, offset: isize) -> usize {
        self.0.wrapping_mul(stride).wrapping_add_signed(offset)
    }

    pub fn in_bounds(self, len: usize) -> bool {
        self.0 < len
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
pub fn index_1d() -> ThreadIndex {
    ThreadIndex(global_id_1d())
}

#[inline(always)]
pub fn index_2d(row_stride: usize) -> Option<ThreadIndex> {
    let row = (block_idx_y() * block_dim_y() + thread_idx_y()) as usize;
    let col = (block_idx_x() * block_dim_x() + thread_idx_x()) as usize;
    if col < row_stride {
        Some(ThreadIndex(row * row_stride + col))
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
