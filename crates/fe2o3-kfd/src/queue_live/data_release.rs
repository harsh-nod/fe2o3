//! Retain detached data through disposal, model retake and ledger commit.

use super::data_insertion::DetachedInsertionLedgerV1;
use super::*;
use crate::shared_memory::{DataCleanupCustodyV1, DispatchDataReleaseV1};

pub(in crate::queue) trait DataReleaseContextV1 {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_>;
    fn release(
        &mut self,
        root: &mut DataCleanupCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn retain(&mut self, root: DataCleanupCustodyV1);
    fn poison(&mut self, panicked: bool);
}

pub(in crate::queue) struct SettledDataReleaseV1 {
    pub(in crate::queue) result: std::thread::Result<Result<(), ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledDataReleaseV1 {
    pub(super) fn into_result(self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

pub(in crate::queue) fn settle_data_release_v1(
    context: &mut impl DataReleaseContextV1,
    data: Gfx942FixedDispatchDataV1,
) -> SettledDataReleaseV1 {
    let identity = data.storage_identity();
    let mut root = DataCleanupCustodyV1::new(data);
    let mut entered = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        context.require_unbound()?;
        entered = true;
        let ledger = context.ledger();
        let mut matching = ledger
            .identities
            .iter()
            .enumerate()
            .filter(|(_, retained)| **retained == identity);
        let Some((index, _)) = matching.next() else {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: *ledger.count,
                detail: "detached release storage identity",
            }
            .into());
        };
        if matching.next().is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "duplicate detached storage identity",
            ));
        }
        let next_count =
            ledger
                .count
                .checked_sub(1)
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "detached dispatch-data ledger underflow",
                ))?;
        context.release(&mut root)?;
        if !root.is_complete() {
            return Err(MemorySessionError::InvalidAllocationAuthority.into());
        }
        // The lower adapter changes memory ownership, never the detached ledger.
        let ledger = context.ledger();
        ledger.identities.remove(index);
        *ledger.count = next_count;
        *ledger.next = Some(index);
        Ok(())
    }));
    let failed = !matches!(result, Ok(Ok(())));
    let transport = result.is_err() || entered && failed;
    if failed {
        // Preflight also consumes the caller's input; retain it without poisoning a healthy parent.
        context.retain(root);
        if transport {
            context.poison(result.is_err());
        }
    }
    SettledDataReleaseV1 { result, transport }
}

impl DataReleaseContextV1 for ComputeAqlQueueSessionV1 {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()
    }

    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        DetachedInsertionLedgerV1 {
            identities: &mut self.detached_data_identities,
            count: &mut self.detached_data_count,
            next: &mut self.detached_next_insertion_index,
        }
    }

    fn release(
        &mut self,
        root: &mut DataCleanupCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.with_live_queue_memory_model(|memory| memory.release_data(root).map_err(Into::into))
    }

    // Keep the no-drop custody policy explicit even though current tokens have no destructor.
    #[allow(clippy::forget_non_drop)]
    fn retain(&mut self, root: DataCleanupCustodyV1) {
        core::mem::forget(root);
    }

    fn poison(&mut self, panicked: bool) {
        self.poison_terminal();
        if panicked {
            poison_process_global_after_dispatch_terminal_v1();
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn release_data_settled_v1(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
    ) -> SettledDataReleaseV1 {
        settle_data_release_v1(self, data)
    }
}
