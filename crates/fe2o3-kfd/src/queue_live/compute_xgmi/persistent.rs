//! Full logical copies retain the original directional persistent owners.

use super::*;
use crate::persistent_allocation::{detach_sdma_buffer_pair_v1, restore_sdma_buffer_pair_v1};
use crate::sdma::{Gfx942SdmaBufferCleanupMetadataV1, Gfx942SdmaBufferStorageV1};
use crate::shared_memory::{
    ComputeXgmiBufferV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) struct TransferRoot {
    allocations: [Option<Gfx942DirectionalQueuePersistentAllocationV1>; 2],
    certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
    sdma: [Option<Gfx942SdmaBufferV1>; 2],
    metadata: [Option<Gfx942SdmaBufferCleanupMetadataV1>; 2],
    locals: [Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>>; 2],
    core: transfer::TransferCore,
}

impl TransferRoot {
    fn new(
        certificates: [Gfx942PersistentDirectionalSdmaAttachmentV1; 2],
        roster: [u32; 2],
        bytes: u32,
    ) -> Self {
        Self {
            allocations: [None, None],
            certificates,
            sdma: [None, None],
            metadata: [None, None],
            locals: [None, None],
            core: transfer::TransferCore::new(roster, bytes),
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
        model_pair_loan::execute(context, |context| {
            self.prepare()
                .and_then(|()| operation(context, &mut self.core))
                .map_err(|error| Failure {
                    error,
                    terminal: true,
                })
        })
        .map_err(session_error)?;
        // Neither persistent backing nor output is restored before BOTH model retakes.
        self.restore()
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
    if memory.mapped_gfx942_device_memory_facts(lease)?.vm() != session.key.vm {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "persistent compute-XGMI data VM mismatch",
        ));
    }
    Ok(bytes)
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

impl Gfx942ComputeXgmiQueueV1 {
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
        if self.transfer.is_some()
            || self.persistent_transfer.is_some()
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
        preflight(source, destination, self.attachment.route)?;
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
        let bytes = transfer::exact_extent(
            require_allocation(source, source_allocation)?,
            require_allocation(destination, destination_allocation)?,
        )?;
        self.persistent_transfer = Some(Box::new(TransferRoot::new(
            [
                source_allocation.attachment,
                destination_allocation.attachment,
            ],
            self.attachment.route.canonical_mapping_gpu_ids(),
            bytes,
        )));
        let root = self
            .persistent_transfer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        root.allocations = [source_data.take(), destination_data.take()];
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
        match result {
            Ok(Ok(())) => {
                *source_data = root.allocations[0].take();
                *destination_data = root.allocations[1].take();
                self.persistent_transfer = None;
                Ok(())
            }
            Ok(Err(error)) => {
                poison(&mut self.queue, source, destination);
                Err(terminal_creation("persistent compute-XGMI transfer", error))
            }
            Err(payload) => {
                poison(&mut self.queue, source, destination);
                resume_unwind(payload)
            }
        }
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
            2048,
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
        retakes: [u8; 2],
        trace: Vec<usize>,
        poisoned: [bool; 3],
    }

    impl model_pair_loan::Context for Context {
        type Loan = usize;
        type Error = ComputeAqlQueueSessionErrorV1;
        fn open(&mut self, endpoint: usize) -> Result<usize, Self::Error> {
            self.trace.push(endpoint);
            Ok(endpoint)
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
}
