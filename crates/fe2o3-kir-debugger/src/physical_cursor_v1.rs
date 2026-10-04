//! Shared transactional navigation over typed immutable CPU captures, not Engine replay.
use crate::PhysicalEntryDebugNavigationV20 as Navigation;
use fe2o3_kir_sim::{
    PhysicalEntryDebugCaptureV20, PhysicalEntryDebugOutcomeV20, PhysicalGlobalCopyDebugCaptureV21,
    PhysicalLdsExchangeDebugCaptureV22,
};

pub(super) trait Capture {
    fn len(&self) -> usize;
    fn ordinal(&self, index: usize) -> Option<u64>;
    fn failed(&self) -> bool;
    fn preflight_refused(&self) -> bool;
    fn completed(&self) -> bool;
    fn stopped(&self) -> bool;
    fn charge(&mut self) -> bool;
}
macro_rules! capture {
    ($ty:ty) => {
        impl Capture for $ty {
            fn len(&self) -> usize {
                <$ty>::len(self)
            }
            fn ordinal(&self, index: usize) -> Option<u64> {
                self.record(index).map(|r| r.ordinal())
            }
            fn failed(&self) -> bool {
                self.error().is_some()
            }
            fn preflight_refused(&self) -> bool {
                self.outcome() == PhysicalEntryDebugOutcomeV20::PreflightRefused
            }
            fn completed(&self) -> bool {
                self.outcome() == PhysicalEntryDebugOutcomeV20::Completed
            }
            fn stopped(&self) -> bool {
                self.stop().is_some()
            }
            fn charge(&mut self) -> bool {
                self.charge_navigation(1).is_ok()
            }
        }
    };
}
capture!(PhysicalEntryDebugCaptureV20);
capture!(PhysicalGlobalCopyDebugCaptureV21);
capture!(PhysicalLdsExchangeDebugCaptureV22);

#[derive(Default)]
pub(super) struct Cursor(Option<usize>);
const _: () = assert!(std::mem::size_of::<Cursor>() == std::mem::size_of::<Option<usize>>());
impl Cursor {
    pub(super) const fn index(&self) -> Option<usize> {
        self.0
    }
    pub(super) fn rewind(&mut self, capture: &mut impl Capture) -> Navigation {
        if capture.failed() || capture.preflight_refused() || !capture.charge() {
            return Navigation::Unavailable;
        }
        self.0 = None;
        Navigation::Beginning
    }
    pub(super) fn seek(&mut self, capture: &mut impl Capture, index: usize) -> Navigation {
        if !capture.charge() || capture.failed() || index > capture.len() {
            return Navigation::Unavailable;
        }
        if let Some(ordinal) = capture.ordinal(index) {
            self.0 = Some(index);
            return Navigation::Record { index, ordinal };
        }
        if capture.stopped() {
            return Navigation::Incomplete;
        }
        if !capture.completed() {
            return Navigation::Unavailable;
        }
        self.0 = Some(index);
        Navigation::End
    }
    pub(super) fn forward(&mut self, capture: &mut impl Capture) -> Navigation {
        match self.0.map_or(Some(0), |n| n.checked_add(1)) {
            Some(n) if n <= capture.len() => self.seek(capture, n),
            _ => Navigation::End,
        }
    }
    pub(super) fn reverse(&mut self, capture: &mut impl Capture) -> Navigation {
        if capture.failed() || capture.preflight_refused() {
            return Navigation::Unavailable;
        }
        if let Some(previous) = self.0.and_then(|n| n.checked_sub(1)) {
            return self.seek(capture, previous);
        }
        if !capture.charge() {
            return Navigation::Unavailable;
        }
        self.0 = None;
        Navigation::Beginning
    }
}
