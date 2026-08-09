//! Invocation-bound GPU group handles.
//!
//! Handles in this module carry source-level execution identity. They do not
//! authenticate a compiled artifact or a launch, and the current compiler does
//! not lower their synchronization entry point.

use core::fmt;
use core::marker::PhantomData;

use crate::sync;
use crate::thread::Invocation3D;
use crate::wave::{Wave64, WaveLane};

mod sealed {
    pub trait Group {}
    pub trait SynchronizationContract {}
    pub trait Gfx942SubgroupWidth {}
}

/// Execution and memory scopes named by a group synchronization contract.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GroupScope {
    /// Every work-item in a launch grid.
    Grid,
    /// Every work-item in one workgroup.
    Workgroup,
    /// Participating lanes in one physical wave.
    Subgroup,
}

/// Memory ordering established by a supported group synchronization operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GroupMemoryOrdering {
    /// Writes before the operation are released and reads after it acquire them.
    AcquireRelease,
}

/// Address spaces ordered by a supported group synchronization operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GroupMemorySpace {
    /// AMD local data share visible within one workgroup.
    Workgroup,
}

/// Sealed type-level description of a group's legal synchronization behavior.
///
/// A contract with `SUPPORTED == false` deliberately exposes no barrier method.
/// A supported contract records the execution scope, memory scope, ordering,
/// address spaces, and convergence requirement that compiler IR must preserve.
pub trait SynchronizationContract: sealed::SynchronizationContract {
    /// Whether this source group has a modeled synchronization operation.
    const SUPPORTED: bool;
    /// Participants that must reach the same dynamic synchronization operation.
    const EXECUTION_SCOPE: Option<GroupScope>;
    /// Visibility scope of memory ordered by the operation.
    const MEMORY_SCOPE: Option<GroupScope>;
    /// Ordering established by the operation.
    const ORDERING: Option<GroupMemoryOrdering>;
    /// Address spaces ordered by the operation.
    const ADDRESS_SPACES: &'static [GroupMemorySpace];
    /// Whether uniform arrival by the execution scope must be proven.
    const REQUIRES_UNIFORM_CONVERGENCE: bool;
}

/// Marker for a group whose synchronization behavior is intentionally absent.
#[derive(Debug)]
pub enum UnsupportedSynchronization {}

impl sealed::SynchronizationContract for UnsupportedSynchronization {}

impl SynchronizationContract for UnsupportedSynchronization {
    const SUPPORTED: bool = false;
    const EXECUTION_SCOPE: Option<GroupScope> = None;
    const MEMORY_SCOPE: Option<GroupScope> = None;
    const ORDERING: Option<GroupMemoryOrdering> = None;
    const ADDRESS_SPACES: &'static [GroupMemorySpace] = &[];
    const REQUIRES_UNIFORM_CONVERGENCE: bool = false;
}

/// Marker for a uniform workgroup barrier over workgroup memory.
#[derive(Debug)]
pub enum WorkgroupSynchronization {}

impl sealed::SynchronizationContract for WorkgroupSynchronization {}

impl SynchronizationContract for WorkgroupSynchronization {
    const SUPPORTED: bool = true;
    const EXECUTION_SCOPE: Option<GroupScope> = Some(GroupScope::Workgroup);
    const MEMORY_SCOPE: Option<GroupScope> = Some(GroupScope::Workgroup);
    const ORDERING: Option<GroupMemoryOrdering> = Some(GroupMemoryOrdering::AcquireRelease);
    const ADDRESS_SPACES: &'static [GroupMemorySpace] = &[GroupMemorySpace::Workgroup];
    const REQUIRES_UNIFORM_CONVERGENCE: bool = true;
}

/// Universal interface implemented by every supported typed group handle.
///
/// Sizes and ranks use `u64` so the same generic algorithm can inspect grid,
/// workgroup, subgroup-tile, and active-lane groups without truncation.
pub trait Group: sealed::Group {
    /// Static synchronization policy for this group kind.
    type Synchronization: SynchronizationContract;

    /// Number of work-items or lanes in this exact group.
    fn size(&self) -> u64;

    /// Zero-based rank of the current invocation within this exact group.
    fn thread_rank(&self) -> u64;
}

/// The complete launch grid containing the current invocation.
///
/// Construction requires an authenticated [`Invocation3D`] and fails when the
/// grid's total size or row-major rank cannot be represented in `u64`. This
/// handle is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`.
pub struct Grid<'invocation> {
    size: u64,
    thread_rank: u64,
    _invocation: PhantomData<&'invocation Invocation3D>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'invocation> Grid<'invocation> {
    /// Derives a grid handle from the current invocation capability.
    pub fn from_invocation(invocation: &'invocation Invocation3D) -> Option<Self> {
        let extent = invocation.global_grid_size();
        Some(Self {
            size: extent.volume()?,
            thread_rank: invocation.global_workitem_id().linear(extent)?,
            _invocation: PhantomData,
            _not_send_sync: PhantomData,
        })
    }
}

impl fmt::Debug for Grid<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Grid")
            .field("size", &self.size)
            .field("thread_rank", &self.thread_rank)
            .finish()
    }
}

impl sealed::Group for Grid<'_> {}

impl Group for Grid<'_> {
    type Synchronization = UnsupportedSynchronization;

    fn size(&self) -> u64 {
        self.size
    }

    fn thread_rank(&self) -> u64 {
        self.thread_rank
    }
}

/// The workgroup containing the current invocation.
///
/// Construction requires an authenticated [`Invocation3D`] and fails when the
/// workgroup's total size or row-major rank cannot be represented in `u64`.
/// This handle is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`.
pub struct Workgroup<'invocation> {
    size: u64,
    thread_rank: u64,
    _invocation: PhantomData<&'invocation Invocation3D>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'invocation> Workgroup<'invocation> {
    /// Derives a workgroup handle from the current invocation capability.
    pub fn from_invocation(invocation: &'invocation Invocation3D) -> Option<Self> {
        let size = invocation.workgroup_size();
        let id = invocation.workitem_id();
        let thread_rank = linear_rank_3d(
            u64::from(id.x()),
            u64::from(id.y()),
            u64::from(id.z()),
            u64::from(size.x()),
            u64::from(size.y()),
        )?;
        Some(Self {
            size: size.volume()?,
            thread_rank,
            _invocation: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    /// Claims uniform convergence for one exact workgroup barrier.
    ///
    /// The returned one-shot witness can be consumed by
    /// [`WorkgroupConvergence::synchronize`].
    ///
    /// # Safety
    ///
    /// Every active work-item in this workgroup must reach the resulting
    /// witness's `synchronize` call exactly once, at the same dynamic program
    /// point and in the same barrier sequence. No participating work-item may
    /// diverge, return, panic, or otherwise skip that call. The caller must also
    /// ensure that the compiler preserves the workgroup convergence claim and
    /// the synchronization semantics in [`WorkgroupSynchronization`]. The
    /// repository's current source compiler path does not yet establish those
    /// facts.
    pub unsafe fn assume_uniform(&self) -> WorkgroupConvergence<'_, 'invocation> {
        WorkgroupConvergence {
            _workgroup: self,
            _not_send_sync: PhantomData,
        }
    }
}

impl fmt::Debug for Workgroup<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Workgroup")
            .field("size", &self.size)
            .field("thread_rank", &self.thread_rank)
            .finish()
    }
}

impl sealed::Group for Workgroup<'_> {}

impl Group for Workgroup<'_> {
    type Synchronization = WorkgroupSynchronization;

    fn size(&self) -> u64 {
        self.size
    }

    fn thread_rank(&self) -> u64 {
        self.thread_rank
    }
}

/// One-shot evidence that an exact workgroup barrier is reached uniformly.
///
/// This witness is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`.
/// Its only public constructor is the unsafe [`Workgroup::assume_uniform`]
/// capability boundary.
#[must_use = "a convergence witness must be consumed by synchronize"]
pub struct WorkgroupConvergence<'group, 'invocation> {
    _workgroup: &'group Workgroup<'invocation>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl WorkgroupConvergence<'_, '_> {
    /// Executes the workgroup barrier represented by this one-shot witness.
    ///
    /// This is safe only because constructing the witness requires the unsafe
    /// uniform-convergence proof boundary. On a host, and until compiler
    /// recognition is implemented, this operation always panics closed.
    pub fn synchronize(self) {
        // SAFETY: `Workgroup::assume_uniform` is the only constructor and its
        // contract establishes the requirements of `sync::syncthreads`.
        unsafe { sync::syncthreads() }
    }
}

impl fmt::Debug for WorkgroupConvergence<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkgroupConvergence")
            .finish_non_exhaustive()
    }
}

/// Type-level candidate for one gfx942 subgroup-tile width.
///
/// [`ValidGfx942SubgroupWidth`] is implemented only for `1`, `2`, `4`, `8`,
/// `16`, `32`, and `64`: the power-of-two divisors of gfx942's physical wave64.
#[derive(Debug)]
pub struct Gfx942SubgroupWidth<const N: u32>;

/// Sealed proof that a const subgroup width is legal for gfx942 wave64.
pub trait ValidGfx942SubgroupWidth: sealed::Gfx942SubgroupWidth {}

macro_rules! valid_gfx942_subgroup_widths {
    ($($width:literal),+ $(,)?) => {
        $(
            impl sealed::Gfx942SubgroupWidth for Gfx942SubgroupWidth<$width> {}
            impl ValidGfx942SubgroupWidth for Gfx942SubgroupWidth<$width> {}
        )+
    };
}

valid_gfx942_subgroup_widths!(1, 2, 4, 8, 16, 32, 64);

/// A static contiguous tile within the current gfx942 physical wave64.
///
/// `N` must be a supported power-of-two divisor of 64. Construction consumes
/// an authenticated wave64 lane witness. This handle is deliberately neither
/// `Copy`, `Clone`, `Send`, nor `Sync`.
pub struct SubgroupTile<const N: u32>
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    lane: u32,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<const N: u32> SubgroupTile<N>
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    fn from_lane(lane: u32) -> Self {
        Self {
            lane,
            _not_send_sync: PhantomData,
        }
    }

    /// Zero-based index of this tile within its physical wave.
    pub const fn tile_index(&self) -> u32 {
        self.lane / N
    }
}

impl<const N: u32> fmt::Debug for SubgroupTile<N>
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubgroupTile")
            .field("width", &N)
            .field("tile_index", &self.tile_index())
            .field("thread_rank", &(self.lane % N))
            .finish()
    }
}

impl<const N: u32> sealed::Group for SubgroupTile<N> where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth
{
}

impl<const N: u32> Group for SubgroupTile<N>
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    type Synchronization = UnsupportedSynchronization;

    fn size(&self) -> u64 {
        u64::from(N)
    }

    fn thread_rank(&self) -> u64 {
        u64::from(self.lane % N)
    }
}

/// The exact active-lane set at one convergent point in a gfx942 wave64.
///
/// Construction consumes an authenticated wave64 lane and requires an unsafe
/// assertion that `member_mask` is the exact hardware active-lane mask at the
/// same source point. This handle is deliberately neither `Copy`, `Clone`,
/// `Send`, nor `Sync`.
pub struct ActiveLaneGroup {
    lane: u32,
    member_mask: u64,
    _not_send_sync: PhantomData<*mut ()>,
}

impl ActiveLaneGroup {
    const fn checked(lane: u32, member_mask: u64) -> Option<Self> {
        if lane >= 64 || member_mask & (1_u64 << lane) == 0 {
            return None;
        }
        Some(Self {
            lane,
            member_mask,
            _not_send_sync: PhantomData,
        })
    }

    /// Exact gfx942 EXEC-mask membership represented by this group.
    pub const fn member_mask(&self) -> u64 {
        self.member_mask
    }
}

impl fmt::Debug for ActiveLaneGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveLaneGroup")
            .field("member_mask", &format_args!("{:#018x}", self.member_mask))
            .field("thread_rank", &self.thread_rank())
            .finish()
    }
}

impl sealed::Group for ActiveLaneGroup {}

impl Group for ActiveLaneGroup {
    type Synchronization = UnsupportedSynchronization;

    fn size(&self) -> u64 {
        u64::from(self.member_mask.count_ones())
    }

    fn thread_rank(&self) -> u64 {
        let lower_lanes = (1_u64 << self.lane) - 1;
        u64::from((self.member_mask & lower_lanes).count_ones())
    }
}

impl WaveLane<Wave64> {
    /// Partitions the current gfx942 wave64 into static contiguous `N`-lane
    /// tiles and returns the tile containing this lane.
    pub fn into_subgroup_tile<const N: u32>(self) -> SubgroupTile<N>
    where
        Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
    {
        SubgroupTile::from_lane(self.get())
    }

    /// Forms the exact active-lane group at the current convergent source point.
    ///
    /// Returns `None` if `member_mask` does not contain the current lane.
    ///
    /// # Safety
    ///
    /// `member_mask` must be the exact gfx942 wave64 EXEC mask observed for the
    /// same invocation and at the same convergent source point as this lane
    /// witness. Every set bit must identify one participating lane in the same
    /// physical wave. The current compiler does not yet provide an authenticated
    /// source for this mask.
    pub unsafe fn into_active_lane_group(self, member_mask: u64) -> Option<ActiveLaneGroup> {
        ActiveLaneGroup::checked(self.get(), member_mask)
    }
}

const fn linear_rank_3d(x: u64, y: u64, z: u64, extent_x: u64, extent_y: u64) -> Option<u64> {
    let zy = match z.checked_mul(extent_y) {
        Some(value) => value,
        None => return None,
    };
    let row = match zy.checked_add(y) {
        Some(value) => value,
        None => return None,
    };
    match row.checked_mul(extent_x) {
        Some(value) => value.checked_add(x),
        None => None,
    }
}
