//! Dependencies stay in the funded slot through root reap and domain cleanup.

use super::super::{
    CleanupRecordV1, DeferredReaperV1, EMPTY, QUARANTINED, RETIRING, ReapCellV1, ReaperMode,
    TERMINAL_PENDING,
};
use super::{Budget, Failure, Resource, Service, Storage};
use crate::RetainedResourcesV2 as Retained;
use std::{mem::size_of, sync::atomic::Ordering};

impl Service {
    /// Work on each original ledger for retaining and retiring a full dependency.
    /// Includes reservation/rollback and Arc bookkeeping; nested child cleanup
    /// must retain its own existing prepaid cancellation obligation.
    pub fn retained_launch_work<T: Send + 'static>(retained: usize) -> Result<usize, Failure> {
        Retained::<T>::payload_storage(retained)?
            .checked_mul(64)
            .and_then(|n| n.checked_add(Self::RESERVATION_WORK + 8 + 4 * (1024 + 64)))
            .ok_or(Resource::Arithmetic.into())
    }

    /// Additional request peak, including the full new owner overlapping inputs.
    /// Persistent service storage independently grows by payload_storage.
    pub fn retained_launch_scratch<T: Send + 'static>(retained: usize) -> Result<usize, Failure> {
        Retained::<T>::storage_for(retained)?
            .checked_add(4 * size_of::<(Retained<T>, Failure, usize)>() + 4096)
            .ok_or(Resource::Arithmetic.into())
    }

    pub(crate) fn reserve_retaining<T: Send + 'static>(
        &mut self,
        value: T,
        retained: usize,
        b: &mut Budget<'_>,
    ) -> Result<
        (
            super::ProtectedServiceCleanupReservationV2,
            Retained<T>,
            Storage,
        ),
        Failure,
    > {
        let bytes = Retained::<T>::payload_storage(retained)?;
        let growth = Retained::<T>::storage_for(retained)?
            .checked_sub(retained)
            .ok_or(Resource::Accounting)?;
        let work = Self::retained_launch_work::<T>(retained)?;
        let scratch = Self::retained_launch_scratch::<T>(retained)?;
        b.with_prepaid_scope(retained, 8, work, scratch, |_| {
            let mut mode = self.reaper.mode.lock().unwrap_or_else(|e| e.into_inner());
            let native = self.native(&mut mode)?;
            native.check_storage()?;
            if !native.admission_open {
                return Err(Failure::AdmissionStopped);
            }
            native.charge(work)?;
            // Funding must leave at least one cleanup turn; denial never renews it.
            if !native.admission_open {
                return Err(Failure::AdmissionStopped);
            }
            let slot = self.reaper.reserve_slot().map_err(|_| Failure::Capacity)?;
            let next = native
                .retained_storage
                .checked_add(bytes)
                .ok_or(Resource::Arithmetic);
            let funding = next.and_then(|next| {
                native.ledger.with_budget(|b| b.reserve_storage(bytes))?;
                native.retained_storage = next;
                slot.cell.retained_storage.store(bytes, Ordering::Release);
                Ok(())
            });
            if let Err(e) = funding {
                native.admission_open = false;
                drop(mode);
                drop(slot);
                return Err(e.into());
            }
            drop(mode);
            // The reservation rolls back funded storage on error/unwind. No mode
            // lock is held while allocating or dropping caller-owned dependencies.
            let (view, payload) = Retained::pair(value, retained)?;
            if payload.storage() != bytes {
                return Err(Resource::Accounting.into());
            }
            *slot.cell.retained.lock().unwrap_or_else(|e| e.into_inner()) = Some(payload);
            Ok((
                super::ProtectedServiceCleanupReservationV2 { slot: Some(slot) },
                view,
                Storage(growth),
            ))
        })
    }
}

impl DeferredReaperV1 {
    pub(in crate::process_reaper) fn retire_cell(
        &self,
        cell: &ReapCellV1,
        child: Option<CleanupRecordV1>,
    ) {
        cell.state.store(RETIRING, Ordering::Release);
        let mut retirement = Retirement {
            cell,
            complete: false,
        };
        drop(child);
        let late = cell
            .late
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(crate::retained_late::LatePayload::pending_copy);
        if let Some(late) = late {
            // The original child/domain is already terminal. Failed funding or
            // readiness keeps the same payload and charge, without another wait.
            if self.fund_late_retirement(late.work).is_err() || !late.try_retire() {
                retirement.complete = true;
                cell.state.store(TERMINAL_PENDING, Ordering::Release);
                return;
            }
        }
        let late = cell.late.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(late);
        let payload = cell
            .retained
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        // A dependency can itself cancel/defer another child into this same pool.
        // Keep this slot occupied, but hold neither cell nor account mutex here.
        drop(payload);
        let bytes = cell.retained_storage.load(Ordering::Acquire);
        if bytes != 0 {
            let mut mode = self.mode.lock().unwrap_or_else(|e| e.into_inner());
            let ReaperMode::Native(native) = &mut *mode else {
                return;
            };
            if native.retire_storage(bytes).is_err() {
                return;
            }
            cell.retained_storage.store(0, Ordering::Release);
        }
        retirement.complete = true;
        cell.state.store(EMPTY, Ordering::Release);
    }
}

// A violated destructor contract or accounting invariant must never publish reuse.
struct Retirement<'a> {
    cell: &'a ReapCellV1,
    complete: bool,
}
impl Drop for Retirement<'_> {
    fn drop(&mut self) {
        if !self.complete {
            self.cell.state.store(QUARANTINED, Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "process_reaper_retained_tests.rs"]
mod tests;
