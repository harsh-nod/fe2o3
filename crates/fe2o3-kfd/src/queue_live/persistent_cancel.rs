//! Prepared cancellation keeps every owner outside the live-memory loan result.

use super::recycled_detach::RecycledDetachLedgerV1;
use super::*;
use crate::persistent_compute::{
    Gfx942PersistentComputeTerminalStageV1, PersistentComputeCancellationCustodyV1,
};
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
use arrayvec::ArrayVec;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::queue) enum CancelShapeV1 {
    Single,
    Three,
}

impl CancelShapeV1 {
    fn count(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Three => 3,
        }
    }

    fn error(self, single: &'static str, three: &'static str) -> ComputeAqlQueueSessionErrorV1 {
        ComputeAqlQueueSessionErrorV1::Contract(match self {
            Self::Single => single,
            Self::Three => three,
        })
    }
}

pub(in crate::queue) struct PersistentCancelRootV1 {
    pub(in crate::queue) attachment: Option<BoundedPersistentComputeAttachmentV1>,
    pub(in crate::queue) native: PersistentComputeCancellationCustodyV1,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::queue) enum CancelPointV1 {
    Returned,
    RestorePreflight,
    Restore(usize),
    CancelPreflight,
    Cancel(usize),
    Output,
}

pub(in crate::queue) trait PersistentCancelContextV1 {
    type Memory: PristineControlReleaseV1;

    fn attachment(&mut self) -> &mut Option<BoundedPersistentComputeAttachmentV1>;
    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1>;
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>;
    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_>;
    fn retain(&mut self, root: PersistentCancelRootV1);
    fn poison(&mut self, process: bool);

    #[cfg(test)]
    fn returned_fixture(&mut self) -> Option<(u64, Vec<Gfx942FixedDispatchDataV1>)> {
        None
    }

    #[cfg(test)]
    fn checkpoint(
        &mut self,
        _point: CancelPointV1,
        _root: &mut PersistentCancelRootV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(())
    }
}

impl PersistentComputeCancellationCustodyV1 {
    fn new(count: usize) -> Self {
        Self {
            cleanup: None,
            returned: Vec::new(),
            generation: None,
            mapped: std::array::from_fn(|_| None),
            initialized: [false; 3],
            count,
            original_attached: false,
            restore_started: false,
            restored: 0,
            cancelled: 0,
            output: ArrayVec::new(),
        }
    }

    pub(crate) fn stage(&self) -> Option<Gfx942PersistentComputeTerminalStageV1> {
        use Gfx942PersistentComputeTerminalStageV1 as Stage;
        if self.original_attached {
            return Some(Stage::Attached);
        }
        if self
            .cleanup
            .as_ref()
            .is_some_and(|cleanup| !cleanup.is_complete())
        {
            return None;
        }
        if self.restored == self.count && (self.cancelled == 0 || self.cancelled == self.count) {
            return Some(Stage::Restored);
        }
        if self.restored != 0 || self.cancelled != 0 {
            return None;
        }
        if self.returned.len() == self.count {
            Some(Stage::DataDetached)
        } else if self.mapped.iter().flatten().count() == self.count {
            Some(Stage::StorageDetached)
        } else {
            None
        }
    }
}

impl PersistentCancelRootV1 {
    fn validate_returned(&self, shape: CancelShapeV1) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let attachment = self
            .attachment
            .as_ref()
            .expect("rooted cancellation attachment");
        let exact = self.native.generation
            == Some(attachment.predecessor_dispatch_generation.unwrap_or(0))
            && self.native.returned.len() == shape.count()
            && self
                .native
                .returned
                .iter()
                .zip(&attachment.entries)
                .all(|(data, entry)| {
                    entry.storage_identity.is_some_and(|identity| {
                        data.sdma_storage_identity()
                            == Gfx942SdmaBufferStorageIdentityV1::Device(identity)
                    }) && (shape == CancelShapeV1::Single
                        || data.is_fully_initialized() == entry.fully_initialized)
                        && (entry.authenticated_sha256.is_none() || data.is_fully_initialized())
                });
        if !exact {
            return Err(shape.error(
                "persistent compute cancellation returned substituted storage",
                "three-binding persistent cancellation returned substituted storage",
            ));
        }
        Ok(())
    }

    fn map_returned(&mut self) {
        // Validation bounds every slot and excludes host storage before any move.
        for (index, data) in self.native.returned.drain(..).enumerate() {
            self.native.initialized[index] = data.is_fully_initialized();
            let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
                unreachable!("validated cancellation device storage")
            };
            self.native.mapped[index] = Some(lease);
        }
    }

    fn restore_preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        for (index, entry) in self.attachment.as_ref().unwrap().entries.iter().enumerate() {
            let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
            };
            entry
                .allocation
                .owner
                .preflight_restore_local_native_from_cancelled_compute(
                    prepared,
                    self.native.mapped[index]
                        .as_ref()
                        .expect("rooted mapped lease"),
                )
                .map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "three-binding persistent cancellation native restore preflight",
                    )
                })?;
        }
        Ok(())
    }

    fn restore(
        &mut self,
        index: usize,
        shape: CancelShapeV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let entry = &mut self.attachment.as_mut().unwrap().entries[index];
        let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        };
        let lease = self.native.mapped[index]
            .take()
            .expect("rooted mapped lease");
        if let Err((_, lease)) = entry
            .allocation
            .owner
            .restore_local_native_from_cancelled_compute(prepared, lease)
        {
            self.native.mapped[index] = Some(lease);
            return Err(shape.error(
                "persistent compute cancellation native restore",
                "three-binding persistent cancellation native restore",
            ));
        }
        self.native.restored += 1;
        Ok(())
    }

    fn cancel_preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        for entry in &self.attachment.as_ref().unwrap().entries {
            let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
            };
            entry
                .allocation
                .owner
                .preflight_cancel_prepared(prepared)
                .map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "three-binding persistent cancellation ledger preflight",
                    )
                })?;
        }
        Ok(())
    }

    fn cancel(
        &mut self,
        index: usize,
        shape: CancelShapeV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let entry = &mut self.attachment.as_mut().unwrap().entries[index];
        // The existing helper puts Err(prepared) back; a one-entry call stops the suffix.
        if !matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if !cancel_persistent_compute_prepublication_entries_v1([entry]) {
            return Err(shape.error(
                "persistent compute cancellation ledger transition",
                "three-binding persistent cancellation ledger transition",
            ));
        }
        self.native.cancelled += 1;
        Ok(())
    }
}

pub(in crate::queue) fn settle_persistent_cancel_v1(
    context: &mut impl PersistentCancelContextV1,
    shape: CancelShapeV1,
) -> Result<ArrayVec<Gfx942PersistentComputeInputV1, 3>, ComputeAqlQueueSessionErrorV1> {
    let mut root = PersistentCancelRootV1 {
        attachment: None,
        native: PersistentComputeCancellationCustodyV1::new(shape.count()),
    };
    let mut original = None;
    let mut lower = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let attachment = context
            .attachment()
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        if attachment.entries.len() != shape.count() || attachment.terminal_custody.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        root.attachment = context.attachment().take();
        if root
            .attachment
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)))
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        #[cfg(test)]
        let fixture = context.returned_fixture();
        #[cfg(not(test))]
        let fixture: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)> = None;
        if let Some((generation, data)) = fixture {
            root.native.generation = Some(generation);
            root.native.returned = data;
        } else {
            original = context.dispatch().take();
            root.native.original_attached = original.is_some();
            if original.is_none() {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
            }
            let ((), retake) = context.with_memory_custody(|memory| {
                root.native.cleanup = Some(ReturningControlCleanupCustodyV1::new(
                    original
                        .take()
                        .expect("opened cancellation retains dispatch"),
                    ReturningControlModeV1::PersistentBeforePublication,
                ));
                root.native.original_attached = false;
                lower = Some(catch_unwind(AssertUnwindSafe(|| {
                    let cleanup = root.native.cleanup.as_mut().unwrap();
                    let result = cleanup.release_in_place(memory);
                    match cleanup.take_persistent_data() {
                        Ok((generation, data)) => {
                            root.native.generation = Some(generation);
                            root.native.returned = data;
                        }
                        Err(error) if result.is_ok() => return Err(error.into()),
                        Err(_) => {}
                    }
                    result.map_err(ComputeAqlQueueSessionErrorV1::from)
                })));
            })?;
            retake?;
            match lower.take() {
                Some(Ok(result)) => result?,
                Some(Err(payload)) => resume_unwind(payload),
                None => {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent cancellation callback did not execute",
                    ));
                }
            }
        }
        #[cfg(test)]
        context.checkpoint(CancelPointV1::Returned, &mut root)?;
        root.validate_returned(shape)?;
        root.map_returned();
        if shape == CancelShapeV1::Three {
            #[cfg(test)]
            context.checkpoint(CancelPointV1::RestorePreflight, &mut root)?;
            root.restore_preflight()?;
        }
        root.native.restore_started = true;
        for index in 0..shape.count() {
            #[cfg(test)]
            context.checkpoint(CancelPointV1::Restore(index), &mut root)?;
            root.restore(index, shape)?;
        }
        if shape == CancelShapeV1::Three {
            #[cfg(test)]
            context.checkpoint(CancelPointV1::CancelPreflight, &mut root)?;
            root.cancel_preflight()?;
        }
        for index in 0..shape.count() {
            #[cfg(test)]
            context.checkpoint(CancelPointV1::Cancel(index), &mut root)?;
            root.cancel(index, shape)?;
        }
        #[cfg(test)]
        context.checkpoint(CancelPointV1::Output, &mut root)?;
        let ledger = context.ledger();
        for (index, entry) in root
            .attachment
            .as_mut()
            .unwrap()
            .entries
            .drain(..)
            .enumerate()
        {
            root.native
                .output
                .push(Gfx942PersistentComputeInputV1::from_parts(
                    entry.allocation,
                    entry.authenticated_sha256,
                    root.native.initialized[index],
                ));
        }
        // Output is fully validated and rooted; no fallible operation follows these writes.
        *ledger.generation = root.native.generation;
        *ledger.next = Some(0);
        *ledger.count = 0;
        ledger.identities.clear();
        Ok(())
    }));
    let mut result = match lower {
        Some(Err(payload)) => {
            core::mem::forget(result);
            Err(payload)
        }
        _ => result,
    };
    if original.is_some() {
        *context.dispatch() = original;
    }
    if !matches!(result, Ok(Ok(()))) {
        // Before any restore attempt, retain the existing quarantine behavior.
        // A changed restore/cancel prefix must never be replayed or overwritten.
        let quarantine = catch_unwind(AssertUnwindSafe(|| {
            if !root.native.restore_started
                && let Some(attachment) = &mut root.attachment
            {
                for entry in &mut attachment.entries {
                    if matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)) {
                        quarantine_persistent_compute_entries_v1(
                            [entry],
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                        );
                    }
                }
            }
        }));
        if let Err(payload) = quarantine {
            if result.is_err() {
                core::mem::forget(payload);
            } else {
                result = Err(payload);
            }
        }
        context.retain(root);
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
            context.poison(shape == CancelShapeV1::Three || result.is_err())
        })) {
            if result.is_err() {
                core::mem::forget(payload);
            } else {
                result = Err(payload);
            }
        }
        return match result {
            Ok(result) => result.map(|()| ArrayVec::new()),
            Err(payload) => resume_unwind(payload),
        };
    }
    Ok(core::mem::take(&mut root.native.output))
}

impl PersistentCancelContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;

    fn attachment(&mut self) -> &mut Option<BoundedPersistentComputeAttachmentV1> {
        &mut self.persistent_compute
    }
    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1> {
        &mut self.dispatch
    }
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>
    {
        self.with_live_queue_memory_model_custody(operation)
    }
    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_> {
        RecycledDetachLedgerV1 {
            generation: &mut self.detached_dispatch_generation,
            count: &mut self.detached_data_count,
            identities: &mut self.detached_data_identities,
            next: &mut self.detached_next_insertion_index,
        }
    }
    fn retain(&mut self, mut root: PersistentCancelRootV1) {
        if let Some(mut attachment) = root.attachment.take() {
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Cancellation(root.native),
            );
            self.persistent_compute = Some(attachment);
        }
    }
    fn poison(&mut self, process: bool) {
        self.poison_terminal();
        if process {
            poison_process_global_after_dispatch_terminal_v1();
        }
    }
    #[cfg(test)]
    fn returned_fixture(&mut self) -> Option<(u64, Vec<Gfx942FixedDispatchDataV1>)> {
        self.persistent_compute_test_release.take()
    }
}
