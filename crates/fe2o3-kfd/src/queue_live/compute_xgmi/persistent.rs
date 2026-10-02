//! Checked logical copies retain the original complete directional persistent owners.

use super::*;
use crate::persistent_allocation::{detach_sdma_buffer_pair_v1, restore_sdma_buffer_pair_v1};
use crate::sdma::{
    Gfx942ComputeXgmiCopyWindowV1, Gfx942SdmaBufferCleanupMetadataV1, Gfx942SdmaBufferStorageV1,
};
use crate::shared_memory::{
    ComputeXgmiBufferV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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

impl TransferRoot {
    fn new(
        certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
        roster: [u32; 2],
        window: Gfx942ComputeXgmiCopyWindowV1,
    ) -> Self {
        Self {
            phase: Phase::Admitted,
            allocations: [None, None],
            certificates,
            sdma: [None, None],
            metadata: [None, None],
            locals: [None, None],
            core: transfer::TransferCore::with_window(roster, window),
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
        window: Option<Gfx942ComputeXgmiCopyWindowV1>,
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
        let window = admit_window(source_bytes, destination_bytes, window)?;
        self.persistent_transfer = Some(Box::new(TransferRoot::new(
            [
                source_allocation.attachment,
                destination_allocation.attachment,
            ],
            self.attachment.route.canonical_mapping_gpu_ids(),
            window,
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
        self.admit_persistent_transfer(source, destination, source_data, destination_data, window)?;
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
            None,
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
            Some(window),
        )
    }

    fn begin_persistent_data_with_peer(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        destination_data: &mut Option<Gfx942DirectionalQueuePersistentAllocationV1>,
        window: Option<Gfx942ComputeXgmiCopyWindowV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.admit_persistent_transfer(source, destination, source_data, destination_data, window)?;
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
mod tests {
    use super::super::super::tests::test_queue_key;
    use super::*;
    use crate::persistent_allocation::{Gfx942PersistentOperationV1, Gfx942PersistentUseRequestV1};
    use crate::persistent_directional_sdma::{
        Gfx942PersistentDirectionalSdmaPairV1, promote_directional_persistent_sdma_custody_v1,
    };

    fn allocation(
        id: u64,
        physical: u64,
        initialized: bool,
        public: bool,
    ) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        let lease = if public {
            crate::shared_memory::local_mapping_with_extent_for_persistent_sdma_test(id, physical)
        } else {
            crate::shared_memory::private_local_mapping_for_sdma_pool_test(id)
        };
        let buffer = Gfx942SdmaBufferV1::compute_xgmi_fixture_v1(
            lease,
            test_queue_key(id, 1),
            7,
            2048,
            if initialized { 2048 } else { 1024 },
        );
        promote_directional_persistent_sdma_custody_v1(
            buffer,
            Gfx942PersistentDirectionalSdmaPairV1 {
                host_to_device_queue_id: 41,
                device_to_host_queue_id: 42,
            },
            1,
        )
        .unwrap()
        .0
    }

    fn root() -> TransferRoot {
        let allocations = [
            allocation(11, 4096, true, true),
            allocation(12, 8192, true, true),
        ];
        let mut root = TransferRoot::new(
            allocations
                .each_ref()
                .map(|allocation| allocation.attachment),
            [7, 9],
            Gfx942ComputeXgmiCopyWindowV1::new(2048, 2048, 0, 0, 2048).unwrap(),
        );
        root.allocations = allocations.map(Some);
        root
    }

    #[test]
    fn compute_xgmi_persistent_preflight_rejects_wrong_scope_identity_extent_and_initialization() {
        for case in 0..11 {
            let mut allocation = allocation(11, 4096, case != 8, case != 9);
            let queue = allocation.attachment.queue;
            match case {
                0 | 1 | 2 | 8 | 9 => {}
                3 => {
                    allocation.attachment.storage_identity = self::allocation(12, 4096, true, true)
                        .attachment
                        .storage_identity
                }
                4 => allocation.attachment.pool_generation += 1,
                5 => allocation.attachment.logical_bytes -= 1,
                6 => allocation.attachment.physical_bytes *= 2,
                7 => allocation.attachment.logical_bytes = 0,
                10 => allocation
                    .owner
                    .quarantine_for_caller_reported_currentness_loss(),
                _ => unreachable!(),
            }
            let before = allocation.owner.ownership_snapshot_for_test_v1();
            let attachment = allocation.attachment;
            let result = admit_allocation(
                &allocation,
                if case == 1 {
                    test_queue_key(99, 1)
                } else {
                    queue
                },
                case != 2,
            );
            assert_eq!(result.is_ok(), case == 0, "case {case}");
            assert_eq!(allocation.attachment, attachment);
            assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
        }
    }

    #[test]
    fn compute_xgmi_persistent_requires_empty_ledger_and_exact_frontier_retirement() {
        let mut allocation = allocation(11, 4096, true, true);
        let queue = allocation.attachment.queue;
        let use_lease = allocation
            .owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(
                    Gfx942PersistentOperationV1::ComputeRead,
                    0,
                    2048,
                )
                .unwrap(),
                None,
            )
            .unwrap();
        assert!(admit_allocation(&allocation, queue, true).is_err());
        let prepared = allocation.owner.prepare(use_lease).unwrap();
        let detached = allocation
            .owner
            .detach_local_native_for_compute(&prepared)
            .unwrap();
        assert!(admit_allocation(&allocation, queue, true).is_err());
        allocation
            .owner
            .restore_local_native_from_cancelled_compute(&prepared, detached)
            .unwrap();
        let published = allocation.owner.publish(prepared).unwrap();
        let completed = allocation.owner.complete(published).unwrap();
        let frontier = allocation.owner.settle(completed).unwrap();
        assert!(admit_allocation(&allocation, queue, true).is_err());
        allocation.owner.retire_settled_frontier(frontier).unwrap();
        assert_eq!(admit_allocation(&allocation, queue, true).unwrap(), 2048);
    }

    #[derive(Default)]
    struct Context {
        opens: [u8; 2],
        retakes: [u8; 2],
        trace: Vec<usize>,
        poisoned: [bool; 3],
    }

    impl model_pair_loan::Context for Context {
        type Loan = usize;
        type Error = ComputeAqlQueueSessionErrorV1;
        fn open(&mut self, endpoint: usize) -> Result<usize, Self::Error> {
            self.trace.push(endpoint);
            match self.opens[endpoint] {
                0 => Ok(endpoint),
                1 => Err(ComputeAqlQueueSessionErrorV1::Contract("injected open")),
                _ => panic!("injected open"),
            }
        }
        fn retake(&mut self, endpoint: usize, loan: usize) -> Result<(), Self::Error> {
            assert_eq!(endpoint, loan);
            self.trace.push(endpoint + 2);
            match self.retakes[endpoint] {
                0 => Ok(()),
                1 => Err(ComputeAqlQueueSessionErrorV1::Contract("injected retake")),
                _ => panic!("injected retake"),
            }
        }
        fn poison_endpoint(&mut self, endpoint: usize) {
            self.poisoned[endpoint] = true;
        }
        fn poison_process(&mut self) {
            self.poisoned[2] = true;
        }
    }

    #[test]
    fn compute_xgmi_persistent_roundtrip_preserves_original_owners_generations_and_unequal_pool_extents()
     {
        let mut root = root();
        let before = root
            .allocations
            .each_ref()
            .map(|a| a.as_ref().unwrap().owner.ownership_snapshot_for_test_v1());
        let mut context = Context::default();
        root.execute(&mut context, |context, core| {
            assert_eq!(context.trace, [0, 1]);
            assert!(core.buffers.iter().all(Option::is_some));
            Ok(())
        })
        .unwrap();
        assert_eq!(context.trace, [0, 1, 3, 2]);
        for (index, before) in before.into_iter().enumerate() {
            let allocation = root.allocations[index].as_ref().unwrap();
            assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
            assert_eq!(allocation.attachment, root.certificates[index]);
            assert_eq!(allocation.byte_len(), 2048);
            assert_eq!(allocation.physical_byte_len(), [4096, 8192][index]);
            assert_eq!(allocation.attachment.pool_generation, 7);
        }
        assert!(root.sdma.iter().all(Option::is_none));
        assert!(root.metadata.iter().all(Option::is_none));
    }

    #[test]
    fn compute_xgmi_persistent_every_retake_error_and_unwind_keeps_backing_out_of_outputs() {
        for source_retake in 0..3 {
            for destination_retake in 0..3 {
                if source_retake == 0 && destination_retake == 0 {
                    continue;
                }
                let mut root = root();
                let before = root
                    .allocations
                    .each_ref()
                    .map(|a| a.as_ref().unwrap().owner.ownership_snapshot_for_test_v1());
                let mut context = Context {
                    retakes: [source_retake, destination_retake],
                    ..Default::default()
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    root.execute(&mut context, |_, _| Ok(()))
                }));
                assert!(result.is_err() || result.unwrap().is_err());
                assert_eq!(context.trace, [0, 1, 3, 2]);
                assert_eq!(context.poisoned, [true; 3]);
                assert!(root.core.buffers.iter().all(Option::is_some));
                assert!(root.metadata.iter().all(Option::is_some));
                for (index, before) in before.iter().enumerate() {
                    let owner = &root.allocations[index].as_ref().unwrap().owner;
                    assert!(owner.local_native_for_sdma().is_none());
                    assert!(before.same_allocation(&owner.ownership_snapshot_for_test_v1()));
                }
            }
        }
    }

    #[test]
    fn compute_xgmi_persistent_operation_failure_or_unwind_retakes_both_and_retains_original_custody()
     {
        for panic in [false, true] {
            let mut root = root();
            let mut context = Context::default();
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.execute(&mut context, |_, _| {
                    if panic {
                        panic!("injected transfer");
                    }
                    Err(ComputeAqlQueueSessionErrorV1::Contract("injected transfer"))
                })
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert_eq!(context.trace, [0, 1, 3, 2]);
            assert_eq!(context.poisoned, [true; 3]);
            for index in 0..2 {
                assert!(
                    root.allocations[index]
                        .as_ref()
                        .unwrap()
                        .owner
                        .local_native_for_sdma()
                        .is_none()
                );
                assert!(root.core.buffers[index].is_some());
                assert!(root.metadata[index].is_some());
            }
        }
    }

    #[test]
    fn compute_xgmi_persistent_restoration_failure_keeps_both_original_mappings_rooted() {
        let mut root = root();
        root.prepare().unwrap();
        root.metadata.swap(0, 1);
        assert!(root.restore().is_err());
        assert!(root.locals[0].is_some());
        assert!(root.metadata.iter().all(Option::is_some));
        assert!(
            root.core.buffers[1]
                .as_mut()
                .unwrap()
                .take_local()
                .is_some()
        );
        assert!(
            root.allocations.iter().all(|a| a
                .as_ref()
                .unwrap()
                .owner
                .local_native_for_sdma()
                .is_none())
        );
    }

    fn assert_async_detached(root: &TransferRoot) {
        assert!(root.allocations.iter().all(|allocation| {
            allocation
                .as_ref()
                .unwrap()
                .owner
                .local_native_for_sdma()
                .is_none()
        }));
        assert!(root.core.buffers.iter().all(Option::is_some));
        assert!(root.metadata.iter().all(Option::is_some));
        assert!(root.sdma.iter().all(Option::is_none));
        assert!(root.locals.iter().all(Option::is_none));
    }

    fn advance_async(root: &mut TransferRoot, phase: usize) {
        let mut context = Context::default();
        if phase >= 1 {
            root.begin(&mut context, |_, _| Ok(())).unwrap();
        }
        if phase >= 2 {
            assert!(root.poll(&mut context, |_, _| Ok(true)).unwrap());
        }
    }

    fn async_operation(
        root: &mut TransferRoot,
        context: &mut Context,
        phase: usize,
        operation: impl FnOnce() -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match phase {
            0 => root.begin(context, |_, _| operation()),
            1 => root
                .poll(context, |_, _| operation().map(|()| true))
                .map(|_| ()),
            2 => root.finish(context, |_, _| operation()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn compute_xgmi_async_pending_and_ready_retake_models_without_restoring_original_owners() {
        let mut root = root();
        let before = root.allocations.each_ref().map(|allocation| {
            allocation
                .as_ref()
                .unwrap()
                .owner
                .ownership_snapshot_for_test_v1()
        });
        let mut context = Context::default();
        root.begin(&mut context, |context, _| {
            context.trace.push(4);
            Ok(())
        })
        .unwrap();
        assert_eq!(context.trace, [0, 1, 4, 3, 2]);
        assert_eq!(root.phase, Phase::Published);
        assert_async_detached(&root);
        let mut samples = 0;
        for ready in [false, false, true] {
            context.trace.clear();
            assert_eq!(
                root.poll(&mut context, |context, _| {
                    samples += 1;
                    context.trace.push(5);
                    Ok(ready)
                })
                .unwrap(),
                ready
            );
            assert_eq!(context.trace, [0, 1, 5, 3, 2]);
            assert_eq!(
                root.phase,
                if ready {
                    Phase::Ready
                } else {
                    Phase::Published
                }
            );
            assert_async_detached(&root);
        }
        assert_eq!(samples, 3);
        context.trace.clear();
        assert!(
            root.poll(&mut context, |_, _| panic!("ready must not sample again"))
                .unwrap()
        );
        assert!(context.trace.is_empty());
        assert_async_detached(&root);
        root.finish(&mut context, |context, _| {
            context.trace.push(6);
            Ok(())
        })
        .unwrap();
        assert_eq!(context.trace, [0, 1, 6, 3, 2]);
        assert_eq!(root.phase, Phase::Finished);
        for (index, original) in before.into_iter().enumerate() {
            let allocation = root.allocations[index].as_ref().unwrap();
            assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), original);
            assert_eq!(allocation.attachment, root.certificates[index]);
            assert_eq!(allocation.byte_len(), 2048);
            assert_eq!(allocation.physical_byte_len(), [4096, 8192][index]);
        }
        assert!(root.metadata.iter().all(Option::is_none));
        assert!(context.poisoned.iter().all(|poisoned| !poisoned));
    }

    #[test]
    fn compute_xgmi_async_every_phase_retake_failure_retains_detached_custody_and_forbids_retry() {
        for phase in 0..3 {
            for source in 0..3 {
                for destination in 0..3 {
                    if source == 0 && destination == 0 {
                        continue;
                    }
                    let mut root = root();
                    let originals = root.allocations.each_ref().map(|allocation| {
                        allocation
                            .as_ref()
                            .unwrap()
                            .owner
                            .ownership_snapshot_for_test_v1()
                    });
                    advance_async(&mut root, phase);
                    let mut context = Context {
                        retakes: [source, destination],
                        ..Default::default()
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        async_operation(&mut root, &mut context, phase, || Ok(()))
                    }));
                    assert!(result.is_err() || result.unwrap().is_err());
                    assert_eq!(root.phase, Phase::Terminal);
                    assert_eq!(context.trace, [0, 1, 3, 2]);
                    assert_eq!(context.poisoned, [true; 3]);
                    assert_async_detached(&root);
                    for (index, original) in originals.iter().enumerate() {
                        assert!(
                            original.same_allocation(
                                &root.allocations[index]
                                    .as_ref()
                                    .unwrap()
                                    .owner
                                    .ownership_snapshot_for_test_v1()
                            )
                        );
                    }
                    context.trace.clear();
                    assert!(
                        root.poll(&mut context, |_, _| panic!("terminal retry"))
                            .is_err()
                    );
                    assert!(context.trace.is_empty());
                }
            }
        }
    }

    #[test]
    fn compute_xgmi_async_operation_errors_and_panics_settle_both_models_in_every_phase() {
        for phase in 0..3 {
            for panic in [false, true] {
                let mut root = root();
                advance_async(&mut root, phase);
                let mut context = Context {
                    retakes: if panic { [2, 1] } else { [0, 0] },
                    ..Default::default()
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    async_operation(&mut root, &mut context, phase, || {
                        if panic {
                            panic!("async operation panic");
                        }
                        Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "async operation error",
                        ))
                    })
                }));
                if panic {
                    assert_eq!(
                        result.err().unwrap().downcast_ref::<&str>(),
                        Some(&"async operation panic")
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(root.phase, Phase::Terminal);
                assert_eq!(context.trace, [0, 1, 3, 2]);
                assert_eq!(context.poisoned, [true; 3]);
                assert_async_detached(&root);
            }
        }
    }

    #[test]
    fn compute_xgmi_async_open_failures_never_run_phase_and_keep_exact_rooted_inputs() {
        for phase in 0..3 {
            for endpoint in 0..2 {
                for fault in 1..3 {
                    let mut root = root();
                    advance_async(&mut root, phase);
                    let before = root.allocations.each_ref().map(|allocation| {
                        allocation
                            .as_ref()
                            .unwrap()
                            .owner
                            .ownership_snapshot_for_test_v1()
                    });
                    let mut context = Context::default();
                    context.opens[endpoint] = fault;
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        async_operation(&mut root, &mut context, phase, || {
                            panic!("phase must not run")
                        })
                    }));
                    assert!(result.is_err() || result.unwrap().is_err());
                    assert_eq!(root.phase, Phase::Terminal);
                    assert_eq!(
                        context.trace,
                        if endpoint == 0 {
                            vec![0]
                        } else {
                            vec![0, 1, 2]
                        }
                    );
                    assert_eq!(context.poisoned, [true; 3]);
                    for (index, original) in before.into_iter().enumerate() {
                        assert_eq!(
                            root.allocations[index]
                                .as_ref()
                                .unwrap()
                                .owner
                                .ownership_snapshot_for_test_v1(),
                            original
                        );
                    }
                    if phase == 0 {
                        assert!(root.metadata.iter().all(Option::is_none));
                        assert!(root.core.buffers.iter().all(Option::is_none));
                    } else {
                        assert_async_detached(&root);
                    }
                }
            }
        }
    }

    #[test]
    fn compute_xgmi_async_phase_and_output_rejection_are_effect_free() {
        let mut root = root();
        let mut context = Context::default();
        assert!(
            root.poll(&mut context, |_, _| panic!("not published"))
                .is_err()
        );
        assert!(
            root.finish(&mut context, |_, _| panic!("not ready"))
                .is_err()
        );
        assert_eq!(root.phase, Phase::Admitted);
        assert!(context.trace.is_empty());
        root.begin(&mut context, |_, _| Ok(())).unwrap();
        context.trace.clear();
        assert!(
            root.begin(&mut context, |_, _| panic!("duplicate publish"))
                .is_err()
        );
        assert!(
            root.finish(&mut context, |_, _| panic!("pending cannot remap"))
                .is_err()
        );
        assert_eq!(root.phase, Phase::Published);
        assert!(context.trace.is_empty());
        assert_async_detached(&root);
        assert!(context.poisoned.iter().all(|poisoned| !poisoned));
        for source in [false, true] {
            for destination in [false, true] {
                let outputs = [
                    source.then(|| allocation(21, 4096, true, true)),
                    destination.then(|| allocation(22, 8192, true, true)),
                ];
                let before = outputs.each_ref().map(|output| {
                    output
                        .as_ref()
                        .map(|owner| owner.owner.ownership_snapshot_for_test_v1())
                });
                assert_eq!(
                    require_empty_outputs(&outputs[0], &outputs[1]).is_ok(),
                    !source && !destination
                );
                assert_eq!(
                    outputs.each_ref().map(|output| output
                        .as_ref()
                        .map(|owner| owner.owner.ownership_snapshot_for_test_v1())),
                    before
                );
                assert_eq!(root.phase, Phase::Published);
                assert_async_detached(&root);
            }
        }
    }
}
