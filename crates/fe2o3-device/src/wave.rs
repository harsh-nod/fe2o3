use core::fmt;
use core::marker::PhantomData;

mod sealed {
    pub trait Sealed {}
}

/// A statically known AMD wave width.
///
/// This sealed trait admits only [`Wave32`] and [`Wave64`]. It describes a
/// contract with code generation; it does not detect the target's configured
/// wave width.
pub trait WaveWidth: sealed::Sealed + 'static {
    const LANES: u32;
}

/// Type-level identity for a 32-lane wave.
#[derive(Debug)]
pub enum Wave32 {}

impl sealed::Sealed for Wave32 {}

impl WaveWidth for Wave32 {
    const LANES: u32 = 32;
}

/// Type-level identity for a 64-lane wave.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_wave64_width_v1"]
pub enum Wave64 {}

impl sealed::Sealed for Wave64 {}

impl WaveWidth for Wave64 {
    const LANES: u32 = 64;
}

/// Caller-asserted arithmetic snapshot of one lane in a wave.
///
/// `Width` makes the required native wave width part of the Rust type. The
/// witness is deliberately neither `Copy`, `Clone`, `Send`, nor `Sync`; a lane
/// number copied as plain integer data is not a substitute for the related
/// snapshot. The type does not authenticate a target, wave mode, current lane,
/// control-flow epoch, or compiler-provided value.
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_wave_lane"]
pub struct WaveLane<Width: WaveWidth> {
    lane: u32,
    _width: PhantomData<fn() -> Width>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<Width: WaveWidth> WaveLane<Width> {
    /// Returns the current invocation's compiler-authenticated wave lane.
    ///
    /// Authenticated lowering must prove that the target's native wave width
    /// is exactly `Width::LANES`. Unsupported lowering and host execution trap.
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

    const fn checked(lane: u32) -> Option<Self> {
        if lane >= Width::LANES {
            return None;
        }
        Some(Self {
            lane,
            _width: PhantomData,
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

impl<Width: WaveWidth> fmt::Debug for WaveLane<Width> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WaveLane")
            .field("lane", &self.lane)
            .field("width", &Width::LANES)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Wave32, Wave64, WaveLane, WaveWidth};
    use core::mem::{align_of, size_of};

    #[test]
    fn widths_are_explicit_and_sealed() {
        assert_eq!(Wave32::LANES, 32);
        assert_eq!(Wave64::LANES, 64);
    }

    #[test]
    fn lane_witnesses_validate_the_static_width() {
        let first = WaveLane::<Wave32>::from_model_snapshot(0).unwrap();
        let last = WaveLane::<Wave32>::from_model_snapshot(31).unwrap();
        assert!(first.is_first());
        assert!(last.is_last());
        assert_eq!(last.get(), 31);
        assert_eq!(last.width(), 32);
        assert!(WaveLane::<Wave32>::from_model_snapshot(32).is_none());
        assert!(WaveLane::<Wave64>::from_model_snapshot(63).is_some());
        assert!(WaveLane::<Wave64>::from_model_snapshot(64).is_none());
    }

    #[test]
    fn lane_witnesses_consume_into_exact_endpoint_ids() {
        assert_eq!(
            WaveLane::<Wave32>::from_model_snapshot(0)
                .unwrap()
                .into_lane_id(),
            0
        );
        assert_eq!(
            WaveLane::<Wave32>::from_model_snapshot(31)
                .unwrap()
                .into_lane_id(),
            31
        );
        assert_eq!(
            WaveLane::<Wave64>::from_model_snapshot(0)
                .unwrap()
                .into_lane_id(),
            0
        );
        assert_eq!(
            WaveLane::<Wave64>::from_model_snapshot(63)
                .unwrap()
                .into_lane_id(),
            63
        );
    }

    #[test]
    fn width_markers_do_not_change_the_lane_abi() {
        assert_eq!(size_of::<WaveLane<Wave32>>(), size_of::<u32>());
        assert_eq!(align_of::<WaveLane<Wave32>>(), align_of::<u32>());
        assert_eq!(size_of::<WaveLane<Wave64>>(), size_of::<u32>());
        assert_eq!(align_of::<WaveLane<Wave64>>(), align_of::<u32>());
    }

    #[test]
    fn current_lane_fails_closed_on_host() {
        assert!(std::panic::catch_unwind(WaveLane::<Wave64>::current).is_err());
    }
}
