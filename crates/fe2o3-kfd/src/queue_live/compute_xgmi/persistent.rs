//! Checked logical copies retain the original complete directional persistent owners.

use super::*;
use crate::persistent_allocation::{detach_sdma_buffer_pair_v1, restore_sdma_buffer_pair_v1};
use crate::sdma::{
    Gfx942ComputeXgmiCopyWindowV1, Gfx942ComputeXgmiSegmentsPlanV1,
    Gfx942SdmaBufferCleanupMetadataV1, Gfx942SdmaBufferStorageV1,
};
use crate::shared_memory::{
    ComputeXgmiBufferV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;

#[cfg(test)]
#[path = "persistent_composed_tests.rs"]
mod composed_tests;

pub(super) struct TransferRoot {
    phase: Phase,
    allocations: [Option<Gfx942DirectionalQueuePersistentAllocationV1>; 2],
    certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
    sdma: [Option<Gfx942SdmaBufferV1>; 2],
    metadata: [Option<Gfx942SdmaBufferCleanupMetadataV1>; 2],
    locals: [Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>>; 2],
    core: transfer::TransferCore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Admitted,
    Published,
    Ready,
    Finished,
    Terminal,
}

enum CopyPlan {
    Window(Option<Gfx942ComputeXgmiCopyWindowV1>),
    Segments(Arc<Gfx942ComputeXgmiSegmentsPlanV1>),
}

impl CopyPlan {
    fn admit(
        self,
        roster: [u32; 2],
        source_bytes: u64,
        destination_bytes: u64,
    ) -> Result<transfer::TransferCore, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Window(window) => Ok(transfer::TransferCore::with_window(
                roster,
                admit_window(source_bytes, destination_bytes, window)?,
            )),
            Self::Segments(plan) => {
                if plan.source_logical_bytes() != source_bytes
                    || plan.destination_logical_bytes() != destination_bytes
                {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "compute-XGMI segments logical extents changed",
                    ));
                }
                Ok(transfer::TransferCore::with_segments(roster, plan))
            }
        }
    }
}

impl TransferRoot {
    #[cfg(test)]
    fn new(
        certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
        roster: [u32; 2],
        window: Gfx942ComputeXgmiCopyWindowV1,
    ) -> Self {
        Self::with_core(
            certificates,
            transfer::TransferCore::with_window(roster, window),
        )
    }

    fn with_core(
        certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
        core: transfer::TransferCore,
    ) -> Self {
        Self {
            phase: Phase::Admitted,
            allocations: [None, None],
            certificates,
            sdma: [None, None],
            metadata: [None, None],
            locals: [None, None],
            core,
        }
    }

    fn prepare(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let [source, destination] = &mut self.allocations;
        let source = source.as_mut().unwrap_or_else(|| std::process::abort());
        let destination = destination
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let scope = |a: Gfx942PersistentDirectionalSdmaAttachmentV1| {
            (a.queue, a.pool_generation, a.logical_bytes)
        };
        let (source, destination) = detach_sdma_buffer_pair_v1(
            &mut source.owner,
            scope(self.certificates[0]),
            &mut destination.owner,
            scope(self.certificates[1]),
        )
        .map_err(map_directional_persistent_sdma_use_error_v1)?;
        self.sdma = [Some(source), Some(destination)];
        for index in 0..2 {
            let (storage, metadata) = self.sdma[index]
                .take()
                .unwrap_or_else(|| std::process::abort())
                .into_cleanup_parts();
            self.metadata[index] = Some(metadata);
            let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
                std::process::abort();
            };
            self.core.buffers[index] = Some(ComputeXgmiBufferV1::new(
                lease,
                self.core.rosters[index]
                    .take()
                    .unwrap_or_else(|| std::process::abort()),
            ));
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        for index in 0..2 {
            self.locals[index] = self.core.buffers[index]
                .as_mut()
                .and_then(ComputeXgmiBufferV1::take_local);
            self.sdma[index] = Some(Gfx942SdmaBufferV1::restore_compute_xgmi_local_v1(
                &mut self.locals[index],
                &mut self.metadata[index],
            )?);
        }
        let [source, destination] = &mut self.allocations;
        let source = source.as_mut().unwrap_or_else(|| std::process::abort());
        let destination = destination
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        match restore_sdma_buffer_pair_v1(
            &mut source.owner,
            self.sdma[0].take().unwrap_or_else(|| std::process::abort()),
            &mut destination.owner,
            self.sdma[1].take().unwrap_or_else(|| std::process::abort()),
        ) {
            Ok(()) => Ok(()),
            Err((error, source, destination)) => {
                self.sdma = [Some(source), Some(destination)];
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }

    fn execute<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        operation: impl FnOnce(
            &mut C,
            &mut transfer::TransferCore,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.settle(
            context,
            Phase::Admitted,
            Phase::Finished,
            |root, context| {
                root.prepare()?;
                operation(context, &mut root.core)
            },
            Self::restore,
        )
    }

    fn begin<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        operation: impl FnOnce(
            &mut C,
            &mut transfer::TransferCore,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.settle(
            context,
            Phase::Admitted,
            Phase::Published,
            |root, context| {
                root.prepare()?;
                operation(context, &mut root.core)
            },
            |_| Ok(()),
        )
    }

    fn poll<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        operation: impl FnOnce(
            &mut C,
            &mut transfer::TransferCore,
        ) -> Result<bool, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.require_pollable()?;
        if self.phase == Phase::Ready {
            return Ok(true);
        }
        let mut ready = false;
        self.settle(
            context,
            Phase::Published,
            Phase::Published,
            |root, context| {
                ready = operation(context, &mut root.core)?;
                Ok(())
            },
            |_| Ok(()),
        )?;
        if ready {
            self.phase = Phase::Ready;
        }
        Ok(ready)
    }

    fn finish<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        operation: impl FnOnce(
            &mut C,
            &mut transfer::TransferCore,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.settle(
            context,
            Phase::Ready,
            Phase::Finished,
            |root, context| operation(context, &mut root.core),
            Self::restore,
        )
    }

    fn progress<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        operation: impl FnOnce(
            &mut C,
            &mut transfer::TransferCore,
        )
            -> Result<Gfx942ComputeXgmiProgressV1, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<Gfx942ComputeXgmiProgressV1, ComputeAqlQueueSessionErrorV1> {
        self.require_pollable()?;
        if self.phase == Phase::Ready {
            return Ok(Gfx942ComputeXgmiProgressV1::Ready);
        }
        let mut progress = Gfx942ComputeXgmiProgressV1::Pending;
        self.settle(
            context,
            Phase::Published,
            Phase::Published,
            |root, context| {
                progress = operation(context, &mut root.core)?;
                Ok(())
            },
            |_| Ok(()),
        )?;
        if progress == Gfx942ComputeXgmiProgressV1::Ready {
            self.phase = Phase::Ready;
        }
        Ok(progress)
    }

    fn require_phase(&self, phase: Phase) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.phase != phase {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute-XGMI transfer phase",
            ));
        }
        Ok(())
    }

    fn require_pollable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.phase == Phase::Ready {
            return Ok(());
        }
        self.require_phase(Phase::Published)
    }

    fn settle<C: model_pair_loan::Context<Error = ComputeAqlQueueSessionErrorV1>>(
        &mut self,
        context: &mut C,
        expected: Phase,
        next: Phase,
        operation: impl FnOnce(&mut Self, &mut C) -> Result<(), ComputeAqlQueueSessionErrorV1>,
        after_retakes: impl FnOnce(&mut Self) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_phase(expected)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            model_pair_loan::execute(context, |context| {
                operation(self, context).map_err(|error| Failure {
                    error,
                    terminal: true,
                })
            })
            .map_err(session_error)?;
            // Neither original backing nor outputs are restored before BOTH retakes.
            after_retakes(self)
        }));
        match result {
            Ok(Ok(())) => {
                self.phase = next;
                Ok(())
            }
            Ok(Err(error)) => {
                self.phase = Phase::Terminal;
                Err(error)
            }
            Err(payload) => {
                self.phase = Phase::Terminal;
                resume_unwind(payload)
            }
        }
    }

    fn validate_restored(
        &self,
        source: &ComputeAqlQueueSessionV1,
        destination: &ComputeAqlQueueSessionV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        for (index, session) in [source, destination].into_iter().enumerate() {
            let allocation = self.allocations[index]
                .as_ref()
                .unwrap_or_else(|| std::process::abort());
            if allocation.attachment != self.certificates[index] {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute-XGMI attachment changed",
                ));
            }
            require_allocation(session, allocation)?;
        }
        Ok(())
    }
}

fn admit_allocation(
    allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    compute_queue: QueueKeyV1,
    attachment_current: bool,
) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
    let attachment = allocation.attachment;
    if attachment.queue != compute_queue || !attachment_current {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI requires the exact compute and SDMA queue pair",
        ));
    }
    if allocation.owner.local_native_for_sdma().map(|lease| {
        crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Device(lease.storage_identity())
    }) != Some(attachment.storage_identity)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI storage identity changed",
        ));
    }
    allocation
        .owner
        .preflight_initialized_storage_for_xgmi(
            attachment.queue,
            attachment.pool_generation,
            attachment.logical_bytes,
            attachment.physical_bytes,
        )
        .map_err(map_directional_persistent_sdma_use_error_v1)?;
    Ok(attachment.logical_bytes)
}

fn require_allocation(
    session: &ComputeAqlQueueSessionV1,
    allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
    let bytes = admit_allocation(
        allocation,
        session.compute_lane_session,
        session.directional_persistent_sdma_attachment_is_current(&allocation.attachment),
    )?;
    let memory = &session
        .engine
        .as_ref()
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "missing persistent compute-XGMI engine",
        ))?
        .backend
        .session;
    let lease = allocation
        .owner
        .local_native_for_sdma()
        .unwrap_or_else(|| std::process::abort());
    let facts = memory.mapped_gfx942_device_memory_facts(lease)?;
    if facts.vm() != session.key.vm {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI data VM mismatch",
        ));
    }
    if facts.checked_gpu_subrange(0, bytes, 1).is_none() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI logical address extent",
        ));
    }
    Ok(bytes)
}

fn admit_window(
    source_bytes: u64,
    destination_bytes: u64,
    window: Option<Gfx942ComputeXgmiCopyWindowV1>,
) -> Result<Gfx942ComputeXgmiCopyWindowV1, ComputeAqlQueueSessionErrorV1> {
    match window {
        Some(window)
            if window.source_logical_bytes() == source_bytes
                && window.destination_logical_bytes() == destination_bytes =>
        {
            Ok(window)
        }
        Some(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI window logical extents changed",
        )),
        None => {
            Gfx942ComputeXgmiCopyWindowV1::new(source_bytes, destination_bytes, 0, 0, source_bytes)
                .filter(|_| source_bytes == destination_bytes)
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "compute-XGMI requires equal complete bounded extents",
                ))
        }
    }
}

fn poison(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source: &mut ComputeAqlQueueSessionV1,
    destination: &mut ComputeAqlQueueSessionV1,
) {
    // A secondary poison panic must not release the root or replace the original failure.
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| {
        queue.poison_compute_xgmi_transfer_v1();
    })));
    for session in [source, destination] {
        core::mem::forget(catch_unwind(AssertUnwindSafe(|| session.poison_terminal())));
    }
    core::mem::forget(catch_unwind(AssertUnwindSafe(
        permanently_poison_process_global_kfd_runtime_gate_v1,
    )));
}

fn require_empty_outputs(
    source: &Option<Gfx942DirectionalQueuePersistentAllocationV1>,
    destination: &Option<Gfx942DirectionalQueuePersistentAllocationV1>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if source.is_some() || destination.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI output slots occupied",
        ));
    }
    Ok(())
}

fn settle_result<T>(
    result: std::thread::Result<Result<T, ComputeAqlQueueSessionErrorV1>>,
    root: &mut TransferRoot,
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source: &mut ComputeAqlQueueSessionV1,
    destination: &mut ComputeAqlQueueSessionV1,
) -> Result<T, ComputeAqlQueueSessionErrorV1> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            root.phase = Phase::Terminal;
            poison(queue, source, destination);
            Err(terminal_creation("persistent compute-XGMI transfer", error))
        }
        Err(payload) => {
            root.phase = Phase::Terminal;
            poison(queue, source, destination);
            resume_unwind(payload)
        }
    }
}

impl Gfx942ComputeXgmiQueueV1 {
    fn require_persistent_transfer_attachment(
        &self,
        source: &ComputeAqlQueueSessionV1,
        destination: &ComputeAqlQueueSessionV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.transfer.is_some()
            || !attachment_matches(source, destination, self.attachment)
            || self.queue.route() != self.attachment.route
            || self
                .queue
                .observation()
                .is_none_or(|observation| observation.queue_id != self.attachment.native_queue_id)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute-XGMI transfer attachment mismatch",
            ));
        }
        preflight(source, destination, self.attachment.route)
    }

    fn admit_persistent_transfer(
        &mut self,
        source: &ComputeAqlQueueSessionV1,
        destination: &ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        plan: CopyPlan,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.persistent_transfer.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute-XGMI transfer attachment mismatch",
            ));
        }
        self.require_persistent_transfer_attachment(source, destination)?;
        let source_allocation =
            source_data
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing persistent compute-XGMI source",
                ))?;
        let destination_allocation =
            destination_data
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing persistent compute-XGMI destination",
                ))?;
        let source_bytes = require_allocation(source, source_allocation)?;
        let destination_bytes = require_allocation(destination, destination_allocation)?;
        let core = plan.admit(
            self.attachment.route.canonical_mapping_gpu_ids(),
            source_bytes,
            destination_bytes,
        )?;
        self.persistent_transfer = Some(Box::new(TransferRoot::with_core(
            [
                source_allocation.attachment,
                destination_allocation.attachment,
            ],
            core,
        )));
        let root = self
            .persistent_transfer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        root.allocations = [source_data.take(), destination_data.take()];
        Ok(())
    }

    /// Copies equal, complete logical extents of initialized PUBLIC persistent storage.
    ///
    /// Both exact directional SDMA attachments must be idle with their settled
    /// frontiers retired. Physical pool extents may differ. Rejection before
    /// admission leaves both slots unchanged. After admission the queue retains
    /// both original owners until local mappings and both VM models are restored.
    /// Any admitted failure or unwind is terminal and retains that custody here.
    pub fn copy_persistent_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        timeout: Duration,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.copy_persistent_data_with_peer(
            source,
            destination,
            source_data,
            destination_data,
            None,
            timeout,
        )
    }

    /// Copies checked logical subranges while retaining both complete original owners.
    ///
    /// Both owners must be fully initialized PUBLIC storage with exact current
    /// attachments. The admitted window must match their actual logical lengths,
    /// not pool padding. Outside-destination bytes remain untouched. All full-copy
    /// quiescence, model-loan, restoration, and terminal-custody rules also apply.
    pub fn copy_persistent_data_range_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        window: Gfx942ComputeXgmiCopyWindowV1,
        timeout: Duration,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.copy_persistent_data_with_peer(
            source,
            destination,
            source_data,
            destination_data,
            Some(window),
            timeout,
        )
    }

    fn copy_persistent_data_with_peer(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        window: Option<Gfx942ComputeXgmiCopyWindowV1>,
        timeout: Duration,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.admit_persistent_transfer(
            source,
            destination,
            source_data,
            destination_data,
            CopyPlan::Window(window),
        )?;
        let root = self
            .persistent_transfer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.execute(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions, core| {
                    let (source, destination) = sessions.memories();
                    core.run(source, destination, &mut self.queue, timeout)
                },
            )?;
            root.validate_restored(source, destination)
        }));
        settle_result(result, root, &mut self.queue, source, destination)?;
        *source_data = root.allocations[0].take();
        *destination_data = root.allocations[1].take();
        self.persistent_transfer = None;
        Ok(())
    }

    /// Publishes the first packet of a full logical copy and retains both owners here.
    ///
    /// This does not wait for GPU completion. Mapping and currentness operations
    /// still make synchronous native calls. Keep this queue and both sessions
    /// alive, and return both endpoints quiescent before each poll or finish.
    /// Rooted inputs are unavailable to other operations; this API does not grant
    /// general same-VM concurrency. Both model foundations are retaken on return.
    /// Preflight rejection preserves inputs. Every admitted error or unwind is
    /// terminal and retains the transfer for process teardown.
    pub fn begin_persistent_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.begin_persistent_data_with_peer(
            source,
            destination,
            source_data,
            destination_data,
            CopyPlan::Window(None),
        )
    }

    /// Publishes the first packet of a checked subrange and retains both complete owners.
    ///
    /// Use the existing poll/progress/finish methods to settle this transfer.
    /// The window is checked against both original logical extents before either
    /// owner is taken. There is no suballocation or same-child concurrency grant.
    /// Every failure after admission retains terminal custody exactly as for a full copy.
    pub fn begin_persistent_data_range_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        window: Gfx942ComputeXgmiCopyWindowV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.begin_persistent_data_with_peer(
            source,
            destination,
            source_data,
            destination_data,
            CopyPlan::Window(Some(window)),
        )
    }

    /// Publishes an immutable ordered list using one queue and one mapping pair.
    ///
    /// The entire plan is checked against both actual logical owner lengths
    /// before extraction. Every descriptor, including duplicates and overlapping
    /// destination writes, completes in list order. Poll never publishes a later
    /// packet or segment; explicit progress publishes at most one packet per call.
    /// Ready and finish apply only to the complete list. Both original owners,
    /// physical extents, attachments, and model-retake obligations are retained
    /// throughout. A failure may leave an applied prefix; every admitted error or
    /// unwind is terminal and retains all custody here, without partial outputs.
    pub fn begin_persistent_data_segments_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        plan: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.begin_persistent_data_with_peer(
            source,
            destination,
            source_data,
            destination_data,
            CopyPlan::Segments(plan),
        )
    }

    fn begin_persistent_data_with_peer(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        plan: CopyPlan,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.admit_persistent_transfer(source, destination, source_data, destination_data, plan)?;
        let root = self
            .persistent_transfer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.begin(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions, core| {
                    let (source, destination) = sessions.memories();
                    core.begin(source, destination, &mut self.queue)
                },
            )
        }));
        settle_result(result, root, &mut self.queue, source, destination)
    }

    /// Samples one completion without waiting, remapping, or returning owners.
    ///
    /// `false` is a successful pending observation. `true` means finish may run;
    /// all custody remains here. Repeated ready observations are effect-free.
    /// This never publishes another packet. Multi-packet transfers require the
    /// explicit progress method to advance after an intermediate completion.
    /// Attachment, endpoint-quiescence, and phase rejection preserve the transfer.
    /// Errors or unwinds after the paired model loan starts are terminal.
    pub fn poll_persistent_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.require_persistent_transfer_attachment(source, destination)?;
        let root =
            self.persistent_transfer
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing persistent compute-XGMI transfer",
                ))?;
        root.require_pollable()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.poll(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions, core| {
                    let (source, destination) = sessions.memories();
                    core.poll(source, destination, &mut self.queue)
                },
            )
        }));
        settle_result(result, root, &mut self.queue, source, destination)
    }

    /// Advances at most one packet publication or one completion sample.
    ///
    /// Each active step retakes both VM models; repeated `Ready` observations
    /// are effect-free. `Changed` means a packet
    /// completed or the next packet was published; it never returns allocation
    /// authority. `Ready` requires the entire ordered extent, but still requires
    /// finish followed by peer queue retirement. There is no GPU wait loop.
    /// Native mapping/currentness calls remain synchronous. Every admitted error
    /// or unwind is terminal and retains both original owners for teardown.
    pub fn progress_persistent_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
    ) -> Result<Gfx942ComputeXgmiProgressV1, ComputeAqlQueueSessionErrorV1> {
        self.require_persistent_transfer_attachment(source, destination)?;
        let root =
            self.persistent_transfer
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing persistent compute-XGMI transfer",
                ))?;
        root.require_pollable()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.progress(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions, core| {
                    let (source, destination) = sessions.memories();
                    core.progress(source, destination, &mut self.queue)
                },
            )
        }));
        settle_result(result, root, &mut self.queue, source, destination)
    }

    /// Restores local mappings and returns the exact original persistent owners.
    ///
    /// Both output slots must be empty and a prior poll must have returned true,
    /// or explicit progress must have returned `Ready`.
    /// Structural or endpoint-quiescence rejection has no native effects and
    /// preserves both the transfer and output slots. Once admitted, every error
    /// or unwind is terminal. Both model retakes precede owner restoration and
    /// output publication. The peer queue still requires explicit retirement.
    pub fn finish_persistent_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        require_empty_outputs(source_data, destination_data)?;
        self.require_persistent_transfer_attachment(source, destination)?;
        let root =
            self.persistent_transfer
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing persistent compute-XGMI transfer",
                ))?;
        root.require_phase(Phase::Ready)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.finish(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions, core| {
                    let (source, destination) = sessions.memories();
                    core.finish(source, destination, &mut self.queue)
                },
            )?;
            root.validate_restored(source, destination)
        }));
        settle_result(result, root, &mut self.queue, source, destination)?;
        *source_data = root.allocations[0].take();
        *destination_data = root.allocations[1].take();
        self.persistent_transfer = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "persistent/tests.rs"]
mod tests;
