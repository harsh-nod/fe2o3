#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ThreadIndex(usize);

impl ThreadIndex {
    pub fn get(self) -> usize {
        self.0
    }

    pub fn in_bounds(self, len: usize) -> bool {
        self.0 < len
    }
}

#[inline(always)]
pub fn index_1d() -> ThreadIndex {
    let tid = thread_idx_x();
    let bid = block_idx_x();
    let bdim = block_dim_x();
    ThreadIndex((bid * bdim + tid) as usize)
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
