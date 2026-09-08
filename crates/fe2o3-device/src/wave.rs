use core::fmt;
use core::marker::PhantomData;

use crate::context::UnbrandedCapability;

mod sealed {
    pub trait Sealed {}
}

/// A statically known target-neutral subgroup width.
///
/// This sealed trait admits only [`SubgroupWidth32`] and [`SubgroupWidth64`].
/// It records an exact source requirement; it does not claim that the selected
/// target supports that width.
pub trait SubgroupWidth: sealed::Sealed + 'static {
    const LANES: u32;
}

/// Type-level identity for an exact 32-lane subgroup requirement.
#[derive(Debug)]
pub enum SubgroupWidth32 {}

impl sealed::Sealed for SubgroupWidth32 {}

impl SubgroupWidth for SubgroupWidth32 {
    const LANES: u32 = 32;
}

/// Type-level identity for an exact 64-lane subgroup requirement.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_wave64_width_v1"]
pub enum SubgroupWidth64 {}

impl sealed::Sealed for SubgroupWidth64 {}

impl SubgroupWidth for SubgroupWidth64 {
    const LANES: u32 = 64;
}

/// AMD wave-width compatibility bound.
///
/// New target-neutral code should use [`SubgroupWidth`]. This sealed aliasing
/// trait cannot admit a width that the neutral contract did not already admit.
#[deprecated(note = "use the target-neutral SubgroupWidth bound")]
pub trait WaveWidth: SubgroupWidth {}

#[allow(deprecated)]
impl<Width: SubgroupWidth> WaveWidth for Width {}

/// Compatibility alias for an exact AMD wave32 requirement.
#[deprecated(note = "use SubgroupWidth32; target legalization still checks the exact width")]
pub type Wave32 = SubgroupWidth32;

/// Compatibility alias for an exact AMD wave64 requirement.
#[deprecated(note = "use SubgroupWidth64; target legalization still checks the exact width")]
pub type Wave64 = SubgroupWidth64;

/// Caller-asserted arithmetic snapshot of one lane in a subgroup.
///
/// `Width` makes the required native wave width part of the Rust type. The
/// witness is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`; a lane
/// number copied as plain integer data is not a substitute for the related
/// snapshot. The type does not authenticate a target, wave mode, current lane,
/// control-flow epoch, or compiler-provided value.
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_wave_lane"]
pub struct SubgroupLane<Width: SubgroupWidth, Brand = UnbrandedCapability> {
    lane: u32,
    _width: PhantomData<fn() -> Width>,
    _brand: PhantomData<fn(Brand) -> Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<Width: SubgroupWidth> SubgroupLane<Width> {
    /// Returns the current invocation's unbranded compatibility wave lane.
    ///
    /// Authenticated lowering must prove that the target's native wave width
    /// is exactly `Width::LANES`. The result carries no nominal kernel, target,
    /// or launch brand; new kernels should use [`crate::KernelContext::lane`].
    /// Unsupported lowering and host execution trap.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_wave_lane_current"]
    pub fn current() -> Self {
        unreachable!("the current wave lane must be issued by authenticated lowering")
    }

    /// Constructs a lane witness from a backend-provided lane ID.
    ///
    /// Returns `None` when `lane` is outside `Width`. This API is unsafe because
    /// the range check cannot establish lane identity or the target wave mode.
    ///
    /// # Safety
    ///
    /// `lane` must be the current invocation's lane ID, and the active kernel
    /// must execute with a native wave width of exactly `Width::LANES` at the
    /// source point where the snapshot is used. The caller must establish both
    /// facts from matching compiler and launch metadata. The current compiler
    /// does not lower this constructor or expose a checked lane intrinsic.
    #[rustc_diagnostic_item = "fe2o3_device_wave_lane_from_raw"]
    pub unsafe fn from_raw(lane: u32) -> Option<Self> {
        Self::checked(lane)
    }

    #[cfg(test)]
    // Builds checked CPU model data without asserting a hardware lane or mode.
    pub(crate) const fn from_model_snapshot(lane: u32) -> Option<Self> {
        Self::checked(lane)
    }
}

impl<Width: SubgroupWidth, Brand> SubgroupLane<Width, Brand> {
    pub(crate) fn current_branded() -> Self {
        let lane = SubgroupLane::<Width>::current();
        Self {
            lane: lane.lane,
            _width: PhantomData,
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    const fn checked(lane: u32) -> Option<Self> {
        if lane >= Width::LANES {
            return None;
        }
        Some(Self {
            lane,
            _width: PhantomData,
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub const fn get(&self) -> u32 {
        self.lane
    }

    /// Consumes the lane witness and returns its authenticated scalar lane ID.
    ///
    /// Consuming the witness preserves its move-only custody when callers no
    /// longer need a typed lane capability.
    pub const fn into_lane_id(self) -> u32 {
        self.lane
    }

    pub const fn width(&self) -> u32 {
        Width::LANES
    }

    pub const fn is_first(&self) -> bool {
        self.lane == 0
    }

    pub const fn is_last(&self) -> bool {
        self.lane + 1 == Width::LANES
    }
}

impl<Width: SubgroupWidth, Brand> fmt::Debug for SubgroupLane<Width, Brand> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubgroupLane")
            .field("lane", &self.lane)
            .field("width", &Width::LANES)
            .finish()
    }
}

/// AMD wave-lane compatibility alias.
///
/// The alias preserves the exact neutral width parameter and therefore cannot
/// erase or weaken a 32- or 64-lane requirement.
#[deprecated(note = "use the target-neutral SubgroupLane")]
pub type WaveLane<Width, Brand = UnbrandedCapability> = SubgroupLane<Width, Brand>;

#[cfg(test)]
mod tests {
    use super::{SubgroupLane, SubgroupWidth, SubgroupWidth32, SubgroupWidth64};
    use core::mem::{align_of, size_of};

    #[test]
    fn widths_are_explicit_and_sealed() {
        assert_eq!(SubgroupWidth32::LANES, 32);
        assert_eq!(SubgroupWidth64::LANES, 64);
    }

    #[test]
    fn lane_witnesses_validate_the_static_width() {
        let first = SubgroupLane::<SubgroupWidth32>::from_model_snapshot(0).unwrap();
        let last = SubgroupLane::<SubgroupWidth32>::from_model_snapshot(31).unwrap();
        assert!(first.is_first());
        assert!(last.is_last());
        assert_eq!(last.get(), 31);
        assert_eq!(last.width(), 32);
        assert!(SubgroupLane::<SubgroupWidth32>::from_model_snapshot(32).is_none());
        assert!(SubgroupLane::<SubgroupWidth64>::from_model_snapshot(63).is_some());
        assert!(SubgroupLane::<SubgroupWidth64>::from_model_snapshot(64).is_none());
    }

    #[test]
    fn lane_witnesses_consume_into_exact_endpoint_ids() {
        assert_eq!(
            SubgroupLane::<SubgroupWidth32>::from_model_snapshot(0)
                .unwrap()
                .into_lane_id(),
            0
        );
        assert_eq!(
            SubgroupLane::<SubgroupWidth32>::from_model_snapshot(31)
                .unwrap()
                .into_lane_id(),
            31
        );
        assert_eq!(
            SubgroupLane::<SubgroupWidth64>::from_model_snapshot(0)
                .unwrap()
                .into_lane_id(),
            0
        );
        assert_eq!(
            SubgroupLane::<SubgroupWidth64>::from_model_snapshot(63)
                .unwrap()
                .into_lane_id(),
            63
        );
    }

    #[test]
    fn width_markers_do_not_change_the_lane_abi() {
        assert_eq!(size_of::<SubgroupLane<SubgroupWidth32>>(), size_of::<u32>());
        assert_eq!(
            align_of::<SubgroupLane<SubgroupWidth32>>(),
            align_of::<u32>()
        );
        assert_eq!(size_of::<SubgroupLane<SubgroupWidth64>>(), size_of::<u32>());
        assert_eq!(
            align_of::<SubgroupLane<SubgroupWidth64>>(),
            align_of::<u32>()
        );
    }

    #[test]
    fn current_lane_fails_closed_on_host() {
        assert!(std::panic::catch_unwind(SubgroupLane::<SubgroupWidth64>::current).is_err());
    }
}
