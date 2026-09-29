//! Persistent prepayment for one late holder in an already-owned native slot.

use super::super::{DeferredReaperV1, RESERVED, ReapSlotV1, ReaperMode};
use super::{Budget, Failure, Resource, Service, Storage};
use crate::{LateRetainedCustodyV2 as Holder, cleanup_bridge::LateRetainedPayloadV2 as Payload};
use std::sync::atomic::Ordering;

impl Service {
    pub(crate) fn reserve_late<T: Payload>(
        &mut self,
        slot: &ReapSlotV1<'_>,
        storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Holder<T>, Storage), Failure> {
        let quota = Holder::<T>::quota(storage)?;
        b.with_prepaid_scope(0, 8, quota.work, quota.scratch, |_| {
            if !std::ptr::eq(self.reaper, slot.reaper)
                || !slot.armed
                || slot.cell.state.load(Ordering::Acquire) != RESERVED
            {
                return Err(Failure::State);
            }
            let mut late = slot.cell.late.try_lock().map_err(|_| Failure::Busy)?;
            if late.is_some() {
                return Err(Failure::State);
            }
            let mut mode = self.reaper.mode.lock().unwrap_or_else(|e| e.into_inner());
            let native = self.native(&mut mode)?;
            native.check_storage()?;
            if !native.admission_open {
                return Err(Failure::AdmissionStopped);
            }
            native.charge(quota.work)?;
            let turn = Self::TURN_WORK
                .checked_add(Self::CELL_WORK)
                .and_then(|n| n.checked_add(quota.retirement_work))
                .ok_or(Resource::Arithmetic)?;
            if native
                .ledger
                .work_limit()
                .saturating_sub(native.ledger.work())
                < turn
            {
                native.admission_open = false;
                return Err(Failure::AdmissionStopped);
            }
            let total = native
                .retained_storage
                .checked_add(quota.persistent)
                .ok_or(Resource::Arithmetic)?;
            let cell_total = slot
                .cell
                .retained_storage
                .load(Ordering::Acquire)
                .checked_add(quota.persistent)
                .ok_or(Resource::Arithmetic)?;
            if let Err(error) = native
                .ledger
                .with_budget(|b| b.reserve_storage(quota.persistent))
            {
                native.admission_open = false;
                return Err(error.into());
            }
            native.retained_storage = total;
            slot.cell
                .retained_storage
                .store(cell_total, Ordering::Release);
            drop(mode);
            // No external payload exists yet. All later custody is installed only
            // through a token returned AFTER its request accounting scope succeeds.
            let (owner, payload) = Holder::pair(quota);
            *late = Some(payload);
            Ok((owner, Storage(quota.retained)))
        })
    }
}

impl DeferredReaperV1 {
    pub(in crate::process_reaper) fn fund_late_retirement(
        &self,
        work: usize,
    ) -> Result<(), Failure> {
        let mut mode = self.mode.lock().unwrap_or_else(|e| e.into_inner());
        let ReaperMode::Native(native) = &mut *mode else {
            return Err(Failure::State);
        };
        if let Err(error) = native.check_storage() {
            native.admission_open = false;
            return Err(error);
        }
        native.charge(work)
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
#[path = "process_reaper_late_tests.rs"]
mod tests;
