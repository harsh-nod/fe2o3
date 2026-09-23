//! Fixed-capacity legacy cleanup custodian; not a native resource account.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::process_cleanup::{ChildCleanupV1, CleanupPollV1};
use crate::{MAX_PROTECTED_ISSUER_PROCESSES_V1, ProtectedIssuerLaunchErrorV1};

const EMPTY: u8 = 0;
const RESERVED: u8 = 1;
const DEFERRED: u8 = 2;
const QUARANTINED: u8 = 3;

struct ReapCellV1 {
    state: AtomicU8,
    child: Mutex<Option<ChildCleanupV1>>,
}

impl ReapCellV1 {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            child: Mutex::new(None),
        }
    }
}

pub(crate) struct DeferredReaperV1 {
    cells: [ReapCellV1; MAX_PROTECTED_ISSUER_PROCESSES_V1],
    thread_started: OnceLock<bool>,
}

impl DeferredReaperV1 {
    const fn new() -> Self {
        Self {
            cells: [const { ReapCellV1::new() }; MAX_PROTECTED_ISSUER_PROCESSES_V1],
            thread_started: OnceLock::new(),
        }
    }

    pub(crate) fn reserve(
        &'static self,
    ) -> Result<ReapSlotV1<'static>, ProtectedIssuerLaunchErrorV1> {
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
            return Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity);
        }
        self.reserve_slot()
    }

    fn reserve_slot(&self) -> Result<ReapSlotV1<'_>, ProtectedIssuerLaunchErrorV1> {
        for cell in &self.cells {
            if cell
                .state
                .compare_exchange(EMPTY, RESERVED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(ReapSlotV1 { cell, armed: true });
            }
        }
        Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity)
    }

    // Each deferred record receives one finite cleanup step. Quarantine retains capacity
    // and all custody; ownership loss must not look like our successful terminal wait.
    fn pump(&self) {
        for cell in &self.cells {
            if cell.state.load(Ordering::Acquire) != DEFERRED {
                continue;
            }
            let mut child = cell.child.lock().unwrap_or_else(|error| error.into_inner());
            let next_state = match child.as_mut().map(ChildCleanupV1::step) {
                Some(CleanupPollV1::Pending) => None,
                Some(CleanupPollV1::Reaped) => {
                    drop(child.take());
                    Some(EMPTY)
                }
                Some(CleanupPollV1::Quarantined) | None => Some(QUARANTINED),
            };
            drop(child);
            // The sole worker retires the payload and mutex guard before slot reuse.
            if let Some(state) = next_state {
                cell.state.store(state, Ordering::Release);
            }
        }
    }
}

pub(crate) struct ReapSlotV1<'a> {
    cell: &'a ReapCellV1,
    armed: bool,
}

impl ReapSlotV1<'_> {
    pub(crate) fn complete(mut self) {
        self.cell.state.store(EMPTY, Ordering::Release);
        self.armed = false;
    }

    pub(crate) fn defer(mut self, child: ChildCleanupV1) {
        *self
            .cell
            .child
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(child);
        self.cell.state.store(DEFERRED, Ordering::Release);
        self.armed = false;
    }
}

impl Drop for ReapSlotV1<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.cell.state.store(EMPTY, Ordering::Release);
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
        let mut slots: Vec<_> = (0..MAX_PROTECTED_ISSUER_PROCESSES_V1)
            .map(|_| reaper.reserve_slot().unwrap())
            .collect();
        assert!(matches!(
            reaper.reserve_slot(),
            Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity)
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
