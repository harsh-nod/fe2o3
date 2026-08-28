use core::fmt;
use core::marker::PhantomData;

/// A work-item's local coordinate within its workgroup.
///
/// This is copyable coordinate data, not evidence that the values describe the
/// current invocation. [`Invocation3D`] groups related caller assertions but
/// likewise does not authenticate invocation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_workitem_id_3d"]
pub struct WorkitemId {
    x: u32,
    y: u32,
    z: u32,
}

impl WorkitemId {
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }
}

/// A workgroup's coordinate within a grid.
///
/// This is copyable coordinate data, not evidence that the values describe the
/// current invocation. [`Invocation3D`] groups related caller assertions but
/// likewise does not authenticate invocation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_id_3d"]
pub struct WorkgroupId {
    x: u32,
    y: u32,
    z: u32,
}

impl WorkgroupId {
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }
}

/// Nonzero three-dimensional workgroup dimensions, measured in work-items.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_size_3d"]
pub struct WorkgroupSize {
    x: u32,
    y: u32,
    z: u32,
}

impl WorkgroupSize {
    pub const fn new(x: u32, y: u32, z: u32) -> Option<Self> {
        if x == 0 || y == 0 || z == 0 {
            return None;
        }
        let Some(xy) = (x as u64).checked_mul(y as u64) else {
            return None;
        };
        if xy.checked_mul(z as u64).is_none() {
            return None;
        }
        Some(Self { x, y, z })
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }

    pub const fn contains(self, id: WorkitemId) -> bool {
        id.x < self.x && id.y < self.y && id.z < self.z
    }

    pub const fn volume(self) -> u64 {
        self.x as u64 * self.y as u64 * self.z as u64
    }
}

/// Nonzero three-dimensional grid dimensions, measured in workgroups.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_grid_size_3d"]
pub struct GridSize {
    x: u32,
    y: u32,
    z: u32,
}

impl GridSize {
    pub const fn new(x: u32, y: u32, z: u32) -> Option<Self> {
        if x == 0 || y == 0 || z == 0 {
            None
        } else {
            Some(Self { x, y, z })
        }
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn z(self) -> u32 {
        self.z
    }

    pub const fn contains(self, id: WorkgroupId) -> bool {
        id.x < self.x && id.y < self.y && id.z < self.z
    }

    pub const fn volume(self) -> Option<u64> {
        match (self.x as u64).checked_mul(self.y as u64) {
            Some(xy) => xy.checked_mul(self.z as u64),
            None => None,
        }
    }
}

/// A global work-item coordinate within the full grid.
///
/// Components are 64-bit because a grid dimension and its workgroup dimension
/// are independently 32-bit quantities.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct GlobalWorkitemId {
    x: u64,
    y: u64,
    z: u64,
}

impl GlobalWorkitemId {
    pub const fn x(self) -> u64 {
        self.x
    }

    pub const fn y(self) -> u64 {
        self.y
    }

    pub const fn z(self) -> u64 {
        self.z
    }

    /// Returns the row-major linear coordinate, with x as the fastest axis.
    pub const fn linear(self, grid: GlobalGridSize) -> Option<u64> {
        if !grid.contains(self) {
            return None;
        }
        let zy = match self.z.checked_mul(grid.y) {
            Some(value) => value,
            None => return None,
        };
        let row = match zy.checked_add(self.y) {
            Some(value) => value,
            None => return None,
        };
        match row.checked_mul(grid.x) {
            Some(value) => value.checked_add(self.x),
            None => None,
        }
    }
}

/// Full grid dimensions measured in work-items.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct GlobalGridSize {
    x: u64,
    y: u64,
    z: u64,
}

impl GlobalGridSize {
    pub const fn x(self) -> u64 {
        self.x
    }

    pub const fn y(self) -> u64 {
        self.y
    }

    pub const fn z(self) -> u64 {
        self.z
    }

    pub const fn contains(self, id: GlobalWorkitemId) -> bool {
        id.x < self.x && id.y < self.y && id.z < self.z
    }

    pub const fn volume(self) -> Option<u64> {
        match self.x.checked_mul(self.y) {
            Some(xy) => xy.checked_mul(self.z),
            None => None,
        }
    }
}

/// Caller-asserted snapshot of one complete three-dimensional launch index.
///
/// Coordinate getters return ordinary copyable data. The witness itself is
/// deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`: code that later
/// performs arithmetic from it retains the lexical association with the
/// snapshot. The type is not branded and does not authenticate a launch,
/// current invocation, control-flow epoch, or compiler-provided value.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_invocation_3d"]
pub struct Invocation3D {
    workitem: WorkitemId,
    workgroup: WorkgroupId,
    workgroup_size: WorkgroupSize,
    grid_size: GridSize,
    _not_send_sync: PhantomData<*mut ()>,
}

impl Invocation3D {
    /// Returns a compiler-authenticated snapshot of the current invocation.
    ///
    /// Unsupported lowering and host execution trap. Unlike
    /// [`Self::from_raw_parts`], this API accepts no caller-asserted identity.
    #[inline(always)]
    #[rustc_diagnostic_item = "fe2o3_device_invocation_3d_current"]
    pub fn current() -> Self {
        Self {
            workitem: WorkitemId {
                x: thread_idx_x(),
                y: thread_idx_y(),
                z: thread_idx_z(),
            },
            workgroup: WorkgroupId {
                x: block_idx_x(),
                y: block_idx_y(),
                z: block_idx_z(),
            },
            workgroup_size: WorkgroupSize {
                x: block_dim_x(),
                y: block_dim_y(),
                z: block_dim_z(),
            },
            grid_size: GridSize {
                x: grid_dim_x(),
                y: grid_dim_y(),
                z: grid_dim_z(),
            },
            _not_send_sync: PhantomData,
        }
    }

    /// Constructs an invocation snapshot from caller-asserted coordinates.
    ///
    /// Returns `None` when either coordinate is outside its corresponding
    /// extent. This API is unsafe because bounds checks cannot establish the
    /// invocation identity or execution epoch.
    ///
    /// # Safety
    ///
    /// All four arguments must describe the current device invocation and its
    /// active launch at the source point where the snapshot is used. The caller
    /// must establish that they came from matching backend values for one
    /// invocation. The current compiler does not lower this constructor or
    /// provide a checked source for these values.
    #[rustc_diagnostic_item = "fe2o3_device_invocation_3d_from_raw_parts"]
    pub unsafe fn from_raw_parts(
        workitem: WorkitemId,
        workgroup: WorkgroupId,
        workgroup_size: WorkgroupSize,
        grid_size: GridSize,
    ) -> Option<Self> {
        Self::checked(workitem, workgroup, workgroup_size, grid_size)
    }

    #[cfg(test)]
    // Builds checked CPU model data without asserting a current invocation.
    pub(crate) const fn from_model_snapshot(
        workitem: WorkitemId,
        workgroup: WorkgroupId,
        workgroup_size: WorkgroupSize,
        grid_size: GridSize,
    ) -> Option<Self> {
        Self::checked(workitem, workgroup, workgroup_size, grid_size)
    }

    const fn checked(
        workitem: WorkitemId,
        workgroup: WorkgroupId,
        workgroup_size: WorkgroupSize,
        grid_size: GridSize,
    ) -> Option<Self> {
        if !workgroup_size.contains(workitem) || !grid_size.contains(workgroup) {
            return None;
        }
        Some(Self {
            workitem,
            workgroup,
            workgroup_size,
            grid_size,
            _not_send_sync: PhantomData,
        })
    }

    pub const fn workitem_id(&self) -> WorkitemId {
        self.workitem
    }

    pub const fn workgroup_id(&self) -> WorkgroupId {
        self.workgroup
    }

    pub const fn workgroup_size(&self) -> WorkgroupSize {
        self.workgroup_size
    }

    pub const fn grid_size(&self) -> GridSize {
        self.grid_size
    }

    pub const fn global_workitem_id(&self) -> GlobalWorkitemId {
        GlobalWorkitemId {
            x: self.workgroup.x as u64 * self.workgroup_size.x as u64 + self.workitem.x as u64,
            y: self.workgroup.y as u64 * self.workgroup_size.y as u64 + self.workitem.y as u64,
            z: self.workgroup.z as u64 * self.workgroup_size.z as u64 + self.workitem.z as u64,
        }
    }

    pub const fn global_grid_size(&self) -> GlobalGridSize {
        GlobalGridSize {
            x: self.grid_size.x as u64 * self.workgroup_size.x as u64,
            y: self.grid_size.y as u64 * self.workgroup_size.y as u64,
            z: self.grid_size.z as u64 * self.workgroup_size.z as u64,
        }
    }
}

/// Type-level index space for the logical one-dimensional launch index.
#[derive(Debug)]
pub enum Index1D {}

/// Type-level mapping for an injective positive translation of an index space.
///
/// The mapping is part of a [`crate::DisjointSlice`] type so safe code cannot
/// use an identity index and a translated index interchangeably. `OFFSET` is a
/// compile-time constant shared by every invocation; invocation-dependent
/// offsets remain ordinary integers and grant no disjoint-write authority.
///
/// This revision defines the device-side type contract. Production typed
/// artifact extraction still admits only `Index1D` and must fail closed until
/// it learns to authenticate this mapping.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_shifted_index_space"]
pub enum Shifted<IndexSpace, const OFFSET: usize> {
    _IndexSpace(core::convert::Infallible, PhantomData<fn() -> IndexSpace>),
}

/// Type-level mapping for fixed-width, component-interleaved output blocks.
///
/// For source index `i`, lane count `L`, and component count `E`, component
/// `c` maps to `(i / L) * L * E + c * L + i % L`. Both dimensions are part of
/// the output view's type, so incompatible block layouts cannot be mixed.
/// Production lowering must authenticate the diagnostic items on this marker,
/// [`DisjointBlock`], and their accessors before treating them as GPU authority.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_blocked_index_space"]
pub enum Blocked<IndexSpace, const LANES_PER_BLOCK: usize, const ELEMENTS_PER_LANE: usize> {
    _IndexSpace(core::convert::Infallible, PhantomData<fn() -> IndexSpace>),
}

/// Type-level mapping for a grid of row-major two-dimensional output tiles.
///
/// Lanes are distributed across tile columns and each lane owns a fixed number
/// of tile rows. The static geometry must satisfy
/// `LANES_PER_TILE * ELEMENTS_PER_LANE == TILE_ROWS * TILE_COLUMNS` and
/// `LANES_PER_TILE / TILE_COLUMNS * ELEMENTS_PER_LANE == TILE_ROWS`.
/// Runtime logical extents and row stride are checked by the matching access.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_tiled_2d_index_space"]
pub enum Tiled2D<
    IndexSpace,
    const LANES_PER_TILE: usize,
    const TILE_ROWS: usize,
    const TILE_COLUMNS: usize,
    const ELEMENTS_PER_LANE: usize,
> {
    _IndexSpace(core::convert::Infallible, PhantomData<fn() -> IndexSpace>),
}

/// Type-level mapping for compact, dynamically-strided row output.
///
/// For source invocation `i`, lane count `L`, and component `c`, the matching
/// access maps to row `i / L` and column `i % L + c * L`. Runtime row and
/// column extents and the row stride are checked by the access operation.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_row_striped_2d_index_space"]
pub enum RowStriped2D<IndexSpace, const LANES_PER_ROW: usize, const ELEMENTS_PER_LANE: usize> {
    _IndexSpace(core::convert::Infallible, PhantomData<fn() -> IndexSpace>),
}

/// Type-level mapping reserved for the unique leader of the full grid.
///
/// Unlike `Index1D`, this space has no safe `ThreadIndex` producer. Mutable
/// views declared with it can therefore expose arbitrary-index operations only
/// through [`GridLeader`] without racing an identity-mapped safe access.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_grid_exclusive_index_space"]
pub enum GridExclusive {}

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

/// A non-forgeable element index for one declared disjoint mapping.
///
/// Safe construction starts with a compiler-issued [`ThreadIndex`]. The
/// mapping remains in `IndexSpace`, preventing an index transformed under one
/// mapping from accessing a [`crate::DisjointSlice`] declared for another.
/// This value is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`.
#[repr(transparent)]
#[must_use = "disjoint write authority is lost when the index is discarded"]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_index"]
pub struct DisjointIndex<IndexSpace = Index1D> {
    raw: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Move-only authority for one lane's elements in a fixed blocked mapping.
///
/// Borrowing this witness permits repeated component writes through a matching
/// [`crate::DisjointSlice`]. Its fields are private, and it is neither `Copy`,
/// `Clone`, `Send`, nor `Sync`.
#[must_use = "blocked output authority is lost when the witness is discarded"]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_block"]
pub struct DisjointBlock<IndexSpace, const LANES_PER_BLOCK: usize, const ELEMENTS_PER_LANE: usize> {
    block_base: usize,
    lane: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Move-only authority for one lane's elements in a two-dimensional tile.
#[repr(transparent)]
#[must_use = "tiled output authority is lost when the witness is discarded"]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_tile_2d"]
pub struct DisjointTile2D<
    IndexSpace,
    const LANES_PER_TILE: usize,
    const TILE_ROWS: usize,
    const TILE_COLUMNS: usize,
    const ELEMENTS_PER_LANE: usize,
> {
    raw: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Move-only authority for one invocation's elements in a compact output row.
#[repr(transparent)]
#[must_use = "row-striped output authority is lost when the witness is discarded"]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_row_stripe_2d"]
pub struct DisjointRowStripe2D<
    IndexSpace,
    const LANES_PER_ROW: usize,
    const ELEMENTS_PER_LANE: usize,
> {
    raw: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Proof that the current invocation is the unique leader of the full grid.
///
/// Safe construction is available only through [`grid_leader`], which checks
/// the compiler-issued global invocation index. The capability permits
/// arbitrary sequential accesses, but grants no cross-invocation
/// synchronization or host/device launch authority. It is neither `Copy`,
/// `Clone`, `Send`, nor `Sync`.
#[must_use = "exclusive grid authority is lost when the leader is discarded"]
#[rustc_diagnostic_item = "fe2o3_device_grid_leader"]
pub struct GridLeader {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl<IndexSpace> ThreadIndex<IndexSpace> {
    #[inline(never)]
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

    /// Converts the current invocation's index into identity-mapped disjoint
    /// write authority.
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_into_disjoint"]
    pub fn into_disjoint(self) -> DisjointIndex<IndexSpace> {
        DisjointIndex {
            raw: self.raw,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Applies one compile-time constant, overflow-checked positive shift.
    ///
    /// The returned type records the mapping. This is injective for all
    /// successfully translated inputs; overflow produces `None` instead of
    /// wrapping two invocation indices onto the same element.
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_checked_shift"]
    pub fn checked_shift<const OFFSET: usize>(
        self,
    ) -> Option<DisjointIndex<Shifted<IndexSpace, OFFSET>>> {
        self.into_disjoint().checked_shift::<OFFSET>()
    }

    /// Converts this invocation index into one fixed blocked-output witness.
    ///
    /// Zero dimensions and every arithmetic overflow return `None`. The
    /// returned witness can address exactly `ELEMENTS_PER_LANE` components in
    /// a matching [`crate::DisjointSlice`] mapping.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_checked_block"]
    pub fn checked_block<const LANES_PER_BLOCK: usize, const ELEMENTS_PER_LANE: usize>(
        self,
    ) -> Option<DisjointBlock<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE>> {
        DisjointBlock::checked_from_raw(self.raw)
    }

    /// Converts this invocation index into one statically shaped 2D tile witness.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_checked_tiled_2d"]
    pub fn checked_tiled_2d<
        const LANES_PER_TILE: usize,
        const TILE_ROWS: usize,
        const TILE_COLUMNS: usize,
        const ELEMENTS_PER_LANE: usize,
    >(
        self,
    ) -> Option<
        DisjointTile2D<IndexSpace, LANES_PER_TILE, TILE_ROWS, TILE_COLUMNS, ELEMENTS_PER_LANE>,
    > {
        DisjointTile2D::checked_from_raw(self.raw)
    }

    /// Converts this invocation index into one dynamically-strided row witness.
    ///
    /// The static geometry is rejected when either dimension is zero or the
    /// greatest component column cannot be represented by `usize`.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_thread_index_checked_row_striped_2d"]
    pub fn checked_row_striped_2d<const LANES_PER_ROW: usize, const ELEMENTS_PER_LANE: usize>(
        self,
    ) -> Option<DisjointRowStripe2D<IndexSpace, LANES_PER_ROW, ELEMENTS_PER_LANE>> {
        DisjointRowStripe2D::checked_from_raw(self.raw)
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

impl<IndexSpace> DisjointIndex<IndexSpace> {
    /// Returns the mapped element index as coordinate data.
    ///
    /// The integer does not carry the disjoint-write authority of `self`.
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_index_get"]
    pub fn get(&self) -> usize {
        self.raw
    }

    /// Applies another compile-time constant, overflow-checked positive shift.
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_index_checked_shift"]
    pub fn checked_shift<const OFFSET: usize>(
        self,
    ) -> Option<DisjointIndex<Shifted<IndexSpace, OFFSET>>> {
        Some(DisjointIndex {
            raw: self.raw.checked_add(OFFSET)?,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_model_index(raw: usize) -> Self {
        Self {
            raw,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<IndexSpace> fmt::Debug for DisjointIndex<IndexSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DisjointIndex")
            .field(&self.raw)
            .finish()
    }
}

impl<IndexSpace, const LANES_PER_BLOCK: usize, const ELEMENTS_PER_LANE: usize>
    DisjointBlock<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE>
{
    fn checked_from_raw(raw: usize) -> Option<Self> {
        if LANES_PER_BLOCK == 0 || ELEMENTS_PER_LANE == 0 {
            return None;
        }

        let block = raw / LANES_PER_BLOCK;
        let lane = raw % LANES_PER_BLOCK;
        let block_elements = LANES_PER_BLOCK.checked_mul(ELEMENTS_PER_LANE)?;
        let block_base = block.checked_mul(block_elements)?;
        let final_component_offset = (ELEMENTS_PER_LANE - 1)
            .checked_mul(LANES_PER_BLOCK)?
            .checked_add(lane)?;
        block_base.checked_add(final_component_offset)?;

        Some(Self {
            block_base,
            lane,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    /// Returns the checked element index for one component of this lane.
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_block_component_index"]
    pub fn component_index(&self, component: usize) -> Option<usize> {
        if component >= ELEMENTS_PER_LANE {
            return None;
        }
        let component_offset = component.checked_mul(LANES_PER_BLOCK)?;
        self.block_base
            .checked_add(component_offset.checked_add(self.lane)?)
    }

    #[cfg(test)]
    pub(crate) fn from_model_index(raw: usize) -> Option<Self> {
        Self::checked_from_raw(raw)
    }
}

impl<
    IndexSpace,
    const LANES_PER_TILE: usize,
    const TILE_ROWS: usize,
    const TILE_COLUMNS: usize,
    const ELEMENTS_PER_LANE: usize,
> DisjointTile2D<IndexSpace, LANES_PER_TILE, TILE_ROWS, TILE_COLUMNS, ELEMENTS_PER_LANE>
{
    fn checked_from_raw(raw: usize) -> Option<Self> {
        if LANES_PER_TILE == 0
            || TILE_ROWS == 0
            || TILE_COLUMNS == 0
            || ELEMENTS_PER_LANE == 0
            || !LANES_PER_TILE.is_multiple_of(TILE_COLUMNS)
            || LANES_PER_TILE.checked_mul(ELEMENTS_PER_LANE)?
                != TILE_ROWS.checked_mul(TILE_COLUMNS)?
            || (LANES_PER_TILE / TILE_COLUMNS).checked_mul(ELEMENTS_PER_LANE)? != TILE_ROWS
        {
            return None;
        }
        Some(Self {
            raw,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub(crate) fn component_index(
        &self,
        component: usize,
        rows: usize,
        columns: usize,
        row_stride: usize,
    ) -> Option<usize> {
        if component >= ELEMENTS_PER_LANE || row_stride < columns {
            return None;
        }
        let tiles_per_row = columns
            .checked_add(TILE_COLUMNS.checked_sub(1)?)?
            .checked_div(TILE_COLUMNS)?;
        if tiles_per_row == 0 {
            return None;
        }
        let tile = self.raw.checked_div(LANES_PER_TILE)?;
        let lane = self.raw.checked_rem(LANES_PER_TILE)?;
        let tile_row = tile.checked_div(tiles_per_row)?;
        let tile_column = tile.checked_rem(tiles_per_row)?;
        let local_row = lane
            .checked_div(TILE_COLUMNS)?
            .checked_mul(ELEMENTS_PER_LANE)?
            .checked_add(component)?;
        let local_column = lane.checked_rem(TILE_COLUMNS)?;
        let row = tile_row.checked_mul(TILE_ROWS)?.checked_add(local_row)?;
        let column = tile_column
            .checked_mul(TILE_COLUMNS)?
            .checked_add(local_column)?;
        if row >= rows || column >= columns {
            return None;
        }
        row.checked_mul(row_stride)?.checked_add(column)
    }

    #[cfg(test)]
    pub(crate) fn from_model_index(raw: usize) -> Option<Self> {
        Self::checked_from_raw(raw)
    }
}

impl<IndexSpace, const LANES_PER_ROW: usize, const ELEMENTS_PER_LANE: usize>
    DisjointRowStripe2D<IndexSpace, LANES_PER_ROW, ELEMENTS_PER_LANE>
{
    fn checked_from_raw(raw: usize) -> Option<Self> {
        if LANES_PER_ROW == 0 || ELEMENTS_PER_LANE == 0 {
            return None;
        }
        (ELEMENTS_PER_LANE - 1)
            .checked_mul(LANES_PER_ROW)?
            .checked_add(LANES_PER_ROW - 1)?;
        Some(Self {
            raw,
            _index_space: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub(crate) fn component_index(
        &self,
        component: usize,
        rows: usize,
        columns: usize,
        row_stride: usize,
    ) -> Option<usize> {
        if component >= ELEMENTS_PER_LANE || row_stride < columns {
            return None;
        }
        let row = self.raw.checked_div(LANES_PER_ROW)?;
        let lane = self.raw.checked_rem(LANES_PER_ROW)?;
        let column = component.checked_mul(LANES_PER_ROW)?.checked_add(lane)?;
        if row >= rows || column >= columns {
            return None;
        }
        row.checked_mul(row_stride)?.checked_add(column)
    }

    #[cfg(test)]
    pub(crate) fn from_model_index(raw: usize) -> Option<Self> {
        Self::checked_from_raw(raw)
    }
}

impl<IndexSpace, const LANES_PER_BLOCK: usize, const ELEMENTS_PER_LANE: usize> fmt::Debug
    for DisjointBlock<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DisjointBlock")
            .field("block_base", &self.block_base)
            .field("lane", &self.lane)
            .field("lanes_per_block", &LANES_PER_BLOCK)
            .field("elements_per_lane", &ELEMENTS_PER_LANE)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for GridLeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("GridLeader").finish_non_exhaustive()
    }
}

#[cfg(test)]
impl GridLeader {
    pub(crate) fn for_host_test() -> Self {
        Self {
            _private: (),
            _not_send_sync: PhantomData,
        }
    }
}

/// Returns the current invocation's zero-based ID in the logical 1D launch.
///
/// This is a target-neutral device intrinsic. A backend may derive it from
/// physical workgroup and local-invocation coordinates, but callers do not
/// observe that mapping.
#[inline(always)]
pub fn global_id_1d() -> usize {
    let workgroup = block_idx_x() as u64;
    let workgroup_size = block_dim_x() as u64;
    let local = thread_idx_x() as u64;
    (workgroup * workgroup_size + local) as usize
}

/// Returns the number of invocations in the logical 1D launch.
///
/// This is the logical global extent, independent of how a backend partitions
/// the launch into physical workgroups.
#[inline(always)]
pub fn launch_extent_1d() -> usize {
    let workgroups = grid_dim_x() as u64;
    let workgroup_size = block_dim_x() as u64;
    (workgroups * workgroup_size) as usize
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_thread_index_1d"]
pub fn index_1d() -> ThreadIndex {
    ThreadIndex {
        raw: global_id_1d(),
        _index_space: PhantomData,
        _not_send_sync: PhantomData,
    }
}

/// Returns exclusive grid authority to the unique global invocation zero.
///
/// This is safe for the same reason as [`index_1d`]: `global_id_1d` is a
/// compiler-issued intrinsic, not caller-provided coordinate data. A backend
/// must preserve the one-to-one logical launch mapping. Until the production
/// importer recognizes this diagnostic identity, it must reject the call
/// rather than lower it as an ordinary host function.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_grid_leader_current"]
pub fn grid_leader() -> Option<GridLeader> {
    if global_id_1d() == 0 {
        Some(GridLeader {
            _private: (),
            _not_send_sync: PhantomData,
        })
    } else {
        None
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
#[rustc_diagnostic_item = "fe2o3_device_thread_index_x_v1"]
pub fn thread_idx_x() -> u32 {
    unreachable!("thread_idx_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_thread_index_y_v1"]
pub fn thread_idx_y() -> u32 {
    unreachable!("thread_idx_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_thread_index_z_v1"]
pub fn thread_idx_z() -> u32 {
    unreachable!("thread_idx_z must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_index_x_v1"]
pub fn block_idx_x() -> u32 {
    unreachable!("block_idx_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_index_y_v1"]
pub fn block_idx_y() -> u32 {
    unreachable!("block_idx_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_index_z_v1"]
pub fn block_idx_z() -> u32 {
    unreachable!("block_idx_z must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_dimension_x_v1"]
pub fn block_dim_x() -> u32 {
    unreachable!("block_dim_x must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_dimension_y_v1"]
pub fn block_dim_y() -> u32 {
    unreachable!("block_dim_y must be lowered by the fe2o3 backend")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_dimension_z_v1"]
pub fn block_dim_z() -> u32 {
    unreachable!("block_dim_z must be lowered by the fe2o3 backend")
}

/// Returns the number of workgroups in the x dimension of the active launch.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_grid_dimension_x_v1"]
pub fn grid_dim_x() -> u32 {
    unreachable!("grid_dim_x must be lowered by the fe2o3 backend")
}

/// Returns the number of workgroups in the y dimension of the active launch.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_grid_dimension_y_v1"]
pub fn grid_dim_y() -> u32 {
    unreachable!("grid_dim_y must be lowered by the fe2o3 backend")
}

/// Returns the number of workgroups in the z dimension of the active launch.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_grid_dimension_z_v1"]
pub fn grid_dim_z() -> u32 {
    unreachable!("grid_dim_z must be lowered by the fe2o3 backend")
}

#[cfg(test)]
mod tests {
    use super::{
        DisjointBlock, DisjointIndex, GridLeader, GridSize, Index1D, Index2D, Invocation3D,
        Shifted, ThreadIndex, WorkgroupId, WorkgroupSize, WorkitemId,
    };
    use core::mem::{align_of, size_of};
    use std::collections::BTreeSet;
    use std::panic::catch_unwind;

    #[test]
    fn current_invocation_fails_closed_on_host() {
        assert!(catch_unwind(Invocation3D::current).is_err());
    }

    #[test]
    fn index_space_markers_do_not_change_the_witness_abi() {
        assert_eq!(size_of::<ThreadIndex<Index1D>>(), size_of::<usize>());
        assert_eq!(align_of::<ThreadIndex<Index1D>>(), align_of::<usize>());
        assert_eq!(size_of::<ThreadIndex<Index2D<64>>>(), size_of::<usize>());
        assert_eq!(align_of::<ThreadIndex<Index2D<64>>>(), align_of::<usize>());
        assert_eq!(size_of::<DisjointIndex<Index1D>>(), size_of::<usize>());
        assert_eq!(align_of::<DisjointIndex<Index1D>>(), align_of::<usize>());
        assert_eq!(
            size_of::<DisjointIndex<Shifted<Index1D, 1>>>(),
            size_of::<usize>()
        );
    }

    #[test]
    fn checked_shift_preserves_distinct_indices_and_rejects_overflow() {
        let first = DisjointIndex::<Index1D>::from_model_index(7)
            .checked_shift::<3>()
            .unwrap();
        let second = DisjointIndex::<Index1D>::from_model_index(8)
            .checked_shift::<3>()
            .unwrap();
        assert_eq!(first.get(), 10);
        assert_eq!(second.get(), 11);
        assert_ne!(first.get(), second.get());

        assert!(
            DisjointIndex::<Index1D>::from_model_index(usize::MAX)
                .checked_shift::<1>()
                .is_none()
        );
    }

    fn assert_block_mapping_is_injective<const L: usize, const E: usize>(raw_count: usize) {
        let mut indices = BTreeSet::new();
        for raw in 0..raw_count {
            let block = DisjointBlock::<Index1D, L, E>::from_model_index(raw).unwrap();
            for component in 0..E {
                assert!(indices.insert(block.component_index(component).unwrap()));
            }
            assert_eq!(block.component_index(E), None);
        }
        assert_eq!(indices.len(), raw_count * E);
    }

    #[test]
    fn blocked_mappings_are_injective_for_contiguous_and_interleaved_outputs() {
        assert_block_mapping_is_injective::<1, 2>(64);
        assert_block_mapping_is_injective::<16, 4>(64);
    }

    #[test]
    fn blocked_mapping_rejects_zero_dimensions_and_overflow() {
        assert!(DisjointBlock::<Index1D, 0, 2>::from_model_index(0).is_none());
        assert!(DisjointBlock::<Index1D, 2, 0>::from_model_index(0).is_none());
        assert!(DisjointBlock::<Index1D, 2, 2>::from_model_index(usize::MAX).is_none());
        assert!(DisjointBlock::<Index1D, { usize::MAX }, 2>::from_model_index(0).is_none());

        let last = DisjointBlock::<Index1D, 1, 1>::from_model_index(usize::MAX).unwrap();
        assert_eq!(last.component_index(0), Some(usize::MAX));
    }

    #[test]
    fn grid_leader_is_zero_sized_and_thread_bound() {
        assert_eq!(size_of::<GridLeader>(), 0);
        assert_eq!(align_of::<GridLeader>(), 1);
    }

    #[test]
    fn grid_leader_acquisition_fails_closed_on_host() {
        assert!(catch_unwind(super::grid_leader).is_err());
    }

    #[test]
    fn dimensions_reject_zero_and_bound_their_coordinates() {
        assert_eq!(WorkgroupSize::new(0, 1, 1), None);
        assert_eq!(GridSize::new(1, 0, 1), None);

        let workgroup_size = WorkgroupSize::new(8, 4, 2).unwrap();
        let grid_size = GridSize::new(3, 5, 7).unwrap();
        assert!(workgroup_size.contains(WorkitemId::new(7, 3, 1)));
        assert!(!workgroup_size.contains(WorkitemId::new(8, 3, 1)));
        assert!(grid_size.contains(WorkgroupId::new(2, 4, 6)));
        assert!(!grid_size.contains(WorkgroupId::new(3, 4, 6)));
        assert_eq!(workgroup_size.volume(), 64);
        assert_eq!(grid_size.volume(), Some(105));
    }

    #[test]
    fn invocation_derives_global_3d_coordinates() {
        let invocation = Invocation3D::from_model_snapshot(
            WorkitemId::new(3, 2, 1),
            WorkgroupId::new(4, 5, 6),
            WorkgroupSize::new(8, 4, 2).unwrap(),
            GridSize::new(10, 20, 30).unwrap(),
        )
        .unwrap();

        let global = invocation.global_workitem_id();
        let extent = invocation.global_grid_size();
        assert_eq!((global.x(), global.y(), global.z()), (35, 22, 13));
        assert_eq!((extent.x(), extent.y(), extent.z()), (80, 80, 60));
        assert_eq!(global.linear(extent), Some((13 * 80 + 22) * 80 + 35));
        assert_eq!(extent.volume(), Some(384_000));
    }

    #[test]
    fn invocation_rejects_out_of_range_coordinates() {
        let workgroup_size = WorkgroupSize::new(8, 4, 2).unwrap();
        let grid_size = GridSize::new(10, 20, 30).unwrap();

        assert!(
            Invocation3D::from_model_snapshot(
                WorkitemId::new(8, 0, 0),
                WorkgroupId::new(0, 0, 0),
                workgroup_size,
                grid_size,
            )
            .is_none()
        );
        assert!(
            Invocation3D::from_model_snapshot(
                WorkitemId::new(0, 0, 0),
                WorkgroupId::new(0, 20, 0),
                workgroup_size,
                grid_size,
            )
            .is_none()
        );
    }
}
