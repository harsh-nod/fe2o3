//! One fixed-capacity cleanup pool, with mutually exclusive legacy/native funding.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY;
use crate::native_cgroup::NativeCgroupDomainV1;
use crate::process_cleanup::{ChildCleanupV1, CleanupPollV1, CleanupRecordV1};
use crate::retained_late::LatePayload;
use crate::retained_resources::RetainedPayload;

#[path = "process_reaper_native.rs"]
mod native;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use native::isolated_cleanup;
pub use native::{
    ProtectedServiceCleanupAdmissionErrorV2, ProtectedServiceCleanupErrorV2,
    ProtectedServiceCleanupReportV2, ProtectedServiceCleanupReservationV2,
    ProtectedServiceCleanupServiceV2,
};

enum ReaperMode {
    Uninitialized,
    Legacy,
    Native(native::NativeAccount),
    Closed,
}

const EMPTY: u8 = 0;
const RESERVED: u8 = 1;
const DEFERRED: u8 = 2;
const QUARANTINED: u8 = 3;
const RETIRING: u8 = 4;
// Child/domain terminal: only nonblocking late-payload retirement remains.
const TERMINAL_PENDING: u8 = 5;

/// Fixed refusal categories for the trusted legacy cleanup protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyCleanupReservationErrorV1 {
    /// Native or closed mode forbids the unmetered legacy worker.
    Mode,
    /// The fixed pool is full or its legacy worker could not start.
    Capacity,
}

struct ReapCellV1 {
    state: AtomicU8,
    child: Mutex<Option<CleanupRecordV1>>,
    retained: Mutex<Option<RetainedPayload>>,
    late: Mutex<Option<LatePayload>>,
    retained_storage: AtomicUsize,
}

impl ReapCellV1 {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            child: Mutex::new(None),
            retained: Mutex::new(None),
            late: Mutex::new(None),
            retained_storage: AtomicUsize::new(0),
        }
    }
}

pub(crate) struct DeferredReaperV1 {
    cells: [ReapCellV1; CAPACITY],
    thread_started: OnceLock<bool>,
    mode: Mutex<ReaperMode>,
}

impl DeferredReaperV1 {
    const fn new() -> Self {
        Self {
            cells: [const { ReapCellV1::new() }; CAPACITY],
            thread_started: OnceLock::new(),
            mode: Mutex::new(ReaperMode::Uninitialized),
        }
    }

    pub(crate) fn reserve(
        &'static self,
    ) -> Result<ReapSlotV1<'static>, LegacyCleanupReservationErrorV1> {
        {
            let mut mode = self.mode.lock().unwrap_or_else(|error| error.into_inner());
            match &*mode {
                ReaperMode::Uninitialized => *mode = ReaperMode::Legacy,
                ReaperMode::Legacy => {}
                ReaperMode::Native(_) | ReaperMode::Closed => {
                    return Err(LegacyCleanupReservationErrorV1::Mode);
                }
            }
        }
        if !*self.thread_started.get_or_init(|| {
            std::thread::Builder::new()
                .name("fe2o3-issuer-reaper-v1".to_owned())
                .spawn(move || {
                    loop {
                        self.pump();
                        std::thread::sleep(Duration::from_millis(10));
                    }
                })
                .is_ok()
        }) {
            return Err(LegacyCleanupReservationErrorV1::Capacity);
        }
        self.reserve_slot()
    }

    fn reserve_slot(&self) -> Result<ReapSlotV1<'_>, LegacyCleanupReservationErrorV1> {
        for cell in &self.cells {
            if cell
                .state
                .compare_exchange(EMPTY, RESERVED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(ReapSlotV1 {
                    reaper: self,
                    cell,
                    armed: true,
                });
            }
        }
        Err(LegacyCleanupReservationErrorV1::Capacity)
    }

    // Each deferred record receives one finite cleanup step. Quarantine retains capacity
    // and all custody; ownership loss must not look like our successful terminal wait.
    fn pump(&self) {
        for cell in &self.cells {
            self.pump_cell(cell);
        }
    }

    fn pump_cell(&self, cell: &ReapCellV1) {
        if cell.state.load(Ordering::Acquire) == TERMINAL_PENDING {
            self.retire_cell(cell, None);
            return;
        }
        if cell.state.load(Ordering::Acquire) != DEFERRED {
            return;
        }
        let mut child = cell.child.lock().unwrap_or_else(|error| error.into_inner());
        let next_state = match child.as_mut().map(CleanupRecordV1::step) {
            Some(CleanupPollV1::Pending) => None,
            Some(CleanupPollV1::Reaped) => {
                let terminal = child.take();
                drop(child);
                self.retire_cell(cell, terminal);
                return;
            }
            Some(CleanupPollV1::Quarantined) | None => Some(QUARANTINED),
        };
        drop(child);
        // Both modes serialize pumping; retire the guard before publishing reuse.
        if let Some(state) = next_state {
            cell.state.store(state, Ordering::Release);
        }
    }
}

/// Reserved cleanup capacity; the trusted bridge binds its lifecycle to one child.
#[must_use]
pub struct ReapSlotV1<'a> {
    reaper: &'a DeferredReaperV1,
    cell: &'a ReapCellV1,
    armed: bool,
}

impl ReapSlotV1<'_> {
    pub(crate) fn check_late<T: crate::cleanup_bridge::LateRetainedPayloadV2>(
        &self,
        owner: &crate::LateRetainedCustodyV2<T>,
    ) -> Result<(), ProtectedServiceCleanupErrorV2> {
        let late = self
            .cell
            .late
            .try_lock()
            .map_err(|_| ProtectedServiceCleanupErrorV2::Busy)?;
        if !self.armed
            || self.cell.state.load(Ordering::Acquire) != RESERVED
            || !late.as_ref().is_some_and(|payload| owner.matches(payload))
        {
            return Err(ProtectedServiceCleanupErrorV2::State);
        }
        Ok(())
    }

    pub(crate) fn complete(mut self) {
        self.armed = false;
        self.reaper.retire_cell(self.cell, None);
    }

    /// Transfers complete unresolved custody into this slot without further I/O.
    pub fn defer(mut self, child: ChildCleanupV1) {
        self.install(CleanupRecordV1::Child(child));
    }

    /// Retains a pre-clone domain and the original inputs/capacity on uncertainty.
    /// The caller must establish that no child was created for this reservation.
    pub(crate) fn defer_domain(mut self, domain: NativeCgroupDomainV1) {
        self.install(CleanupRecordV1::unspawned_domain(domain));
    }

    fn install(&mut self, record: CleanupRecordV1) {
        *self
            .cell
            .child
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(record);
        self.cell.state.store(DEFERRED, Ordering::Release);
        self.armed = false;
    }
}

impl Drop for ReapSlotV1<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.reaper.retire_cell(self.cell, None);
        }
    }
}

pub(crate) fn deferred_reaper() -> &'static DeferredReaperV1 {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    &REAPER
}

#[cfg(test)]
#[path = "process_reaper_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_slots_count_toward_capacity_and_rollback_without_a_child() {
        let reaper = DeferredReaperV1::new();
        let mut slots: Vec<_> = (0..CAPACITY)
            .map(|_| reaper.reserve_slot().unwrap())
            .collect();
        assert!(matches!(
            reaper.reserve_slot(),
            Err(LegacyCleanupReservationErrorV1::Capacity)
        ));
        drop(slots.pop());
        reaper.reserve_slot().unwrap().complete();
        drop(slots);
        assert!(
            reaper
                .cells
                .iter()
                .all(|cell| cell.state.load(Ordering::Acquire) == EMPTY)
        );
    }

    #[test]
    fn idle_pump_does_not_touch_foreground_reservations() {
        let reaper = DeferredReaperV1::new();
        let slot = reaper.reserve_slot().unwrap();
        reaper.pump();
        assert_eq!(slot.cell.state.load(Ordering::Acquire), RESERVED);
        assert!(slot.cell.child.lock().unwrap().is_none());
    }
}
