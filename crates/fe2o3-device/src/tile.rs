//! Source contracts for masked rank-one tiles and local fragments.
//!
//! These staged providers are explicitly rejected by production until the
//! shared capability schema, authenticated importer and schedule-aware lowering
//! are integrated. Terminals never execute a host or device fallback.
//! The private payload is transport storage, not an unscheduled lane mapping.

#![forbid(unsafe_code)]

use core::marker::PhantomData;

use crate::execution::{InitialEpoch, SynchronizationEpoch, WorkgroupCapability};

/// Maximum invocation count in the initial rank-one tile contract.
pub const MAX_MASKED_TILE_LANES_V1: usize = 256;

/// Maximum component count per invocation in the initial u32 contract.
///
/// Two arrays and two zero-field markers use `2 * E + 5` structural nodes
/// including the carrier; extracted parts use `2 * E + 3`. These bounds fit
/// the existing 256-node structural budget without raising it.
pub const MAX_MASKED_TILE_ELEMENTS_PER_LANE_V1: usize = 125;

type TileContract<'workgroup, Brand, Epoch> = fn(
    WorkgroupCapability<'workgroup, Brand, Epoch>,
) -> WorkgroupCapability<'workgroup, Brand, Epoch>;

/// A logical tile of L * E elements bound to one workgroup and epoch.
///
/// Kernel brand, generative workgroup lifetime and epoch are invariant. The
/// carrier is privately constructed, move-only and neither Send nor Sync.
/// Its source type selects no blocked or striped distribution.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_masked_tile_1d_v1"]
pub struct MaskedTile1D<
    'workgroup,
    T,
    const L: usize,
    const E: usize,
    Brand,
    Epoch: SynchronizationEpoch = InitialEpoch,
> {
    values: [T; E],
    active: [bool; E],
    _contract: PhantomData<TileContract<'workgroup, Brand, Epoch>>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Invocation-local values and mask after checked distribution.
///
/// This distinct nominal carrier retains its tile's kernel, workgroup and
/// epoch identity until consumed into ordinary values. Those values confer
/// no tile, collective or memory authority.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_lane_fragment_1d_v1"]
pub struct LaneFragment<
    'workgroup,
    T,
    const L: usize,
    const E: usize,
    Brand,
    Epoch: SynchronizationEpoch = InitialEpoch,
> {
    values: [T; E],
    active: [bool; E],
    _contract: PhantomData<TileContract<'workgroup, Brand, Epoch>>,
    _not_send_sync: PhantomData<*mut ()>,
}

const fn assert_tile_geometry<const L: usize, const E: usize>() {
    assert!(L > 0, "a logical tile needs at least one invocation");
    assert!(
        L <= MAX_MASKED_TILE_LANES_V1,
        "tile invocation count exceeds the contract"
    );
    assert!(E > 0, "a lane fragment cannot be empty");
    assert!(
        E <= MAX_MASKED_TILE_ELEMENTS_PER_LANE_V1,
        "tile component count exceeds the complete carrier budget"
    );
}

impl<'workgroup, const L: usize, const E: usize, Brand, Epoch: SynchronizationEpoch>
    MaskedTile1D<'workgroup, u32, L, E, Brand, Epoch>
{
    /// Loads a logical interval with defined zero for inactive elements.
    ///
    /// Activity requires checked base-plus-offset addition and an in-bounds
    /// input element. Inactive elements cause no read. Arrival, input and base
    /// must be workgroup-uniform; a mask does not excuse collective divergence.
    /// Admission must retain the read's source effect position and authenticate
    /// the exact capability producer, launch, provenance and selected schedule.
    ///
    /// The receiver borrow is intentionally shorter than the generative
    /// workgroup lifetime. It must work with the callback-local owned token.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_masked_tile_1d_load_masked_v1"]
    pub fn load_masked(
        context: &WorkgroupCapability<'workgroup, Brand, Epoch>,
        input: &[u32],
        base: usize,
    ) -> Self {
        const { assert_tile_geometry::<L, E>() };
        let _ = (context, input, base);
        unreachable!("masked tile load requires authenticated schedule-aware lowering")
    }

    /// Consumes the tile into a fragment with the same producer and mask.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_masked_tile_1d_into_fragment_v1"]
    pub fn into_fragment(self) -> LaneFragment<'workgroup, u32, L, E, Brand, Epoch> {
        const { assert_tile_geometry::<L, E>() };
        let _ = (self.values, self.active);
        unreachable!("tile fragment extraction requires authenticated schedule-aware lowering")
    }
}

impl<const L: usize, const E: usize, Brand, Epoch: SynchronizationEpoch>
    LaneFragment<'_, u32, L, E, Brand, Epoch>
{
    /// Consumes the fragment into ordinary per-invocation values and flags.
    ///
    /// Transforming an inactive zero does not make it active. Reconstructing
    /// collective authority from these arrays is not supported.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_lane_fragment_1d_into_parts_v1"]
    pub fn into_parts(self) -> ([u32; E], [bool; E]) {
        const { assert_tile_geometry::<L, E>() };
        let _ = (self.values, self.active);
        unreachable!("lane fragment parts require authenticated schedule-aware lowering")
    }
}
