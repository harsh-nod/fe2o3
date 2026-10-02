//! Compose production custody/transition/publication algorithms, not Linux GPU IO.

use super::*;
use crate::persistent_allocation::PersistentOwnerSnapshotForTestV1;
use crate::persistent_directional_sdma::{
    Gfx942PersistentDirectionalSdmaPairV1, promote_directional_persistent_sdma_custody_v1,
};
use crate::queue::QueueModelFoundationV1;
use crate::sdma::{ComputeXgmiCopyCustodyV1, ComputeXgmiQueueFixtureV1, SdmaSingleMemoryV1};
use crate::shared_memory::{
    Gfx942DeviceMemoryIdentityV1, Gfx942XgmiMappedDeviceMemoryV1, LiveQueueModelFoundationLoanV1,
    PreparationMemoryFixtureV1, PreparationMemoryObservationV1,
};

const LOGICAL_BYTES: u32 = 2048;
const PHYSICAL_BYTES: [usize; 2] = [4096, 8192];
const GPU_IDS: [u32; 2] = [1001, 1002];

struct Pair {
    memory: [PreparationMemoryFixtureV1; 2],
    foundations: [QueueModelFoundationV1; 2],
    queue: ComputeXgmiQueueFixtureV1,
    retakes: [usize; 2],
    poisoned: [bool; 3],
    retake_fault: Option<(usize, bool, bool)>,
    complete_on_submit: bool,
    waits: usize,
}

struct Before {
    owners: [PersistentOwnerSnapshotForTestV1; 2],
    identities: [Gfx942DeviceMemoryIdentityV1; 2],
    addresses: [u64; 2],
    memory: [PreparationMemoryObservationV1; 2],
    models: [(u64, Option<u64>, u64); 2],
}

fn fixture(configured: bool) -> (Pair, TransferRoot, Before) {
    let mut memory = [
        PreparationMemoryFixtureV1::compute_xgmi_v1(GPU_IDS[0], 0x1_0000, configured),
        PreparationMemoryFixtureV1::compute_xgmi_v1(GPU_IDS[1], 0x41_0000, configured),
    ];
    let keys = std::array::from_fn::<_, 2, _>(|index| QueueKeyV1 {
        vm: memory[index].primary_vm(),
        id: QueueInstanceIdV1(11 + index as u64),
        generation: QueueGenerationV1(1),
    });
    let queue = ComputeXgmiQueueFixtureV1::new(&mut memory[0], keys[0]);
    let mut foundations = memory.each_mut().map(|m| m.primary_transfer(&[]).unwrap());
    let allocations = std::array::from_fn::<_, 2, _>(|index| {
        let loan = memory[index].primary_loan(&mut foundations[index]).unwrap();
        let lease = memory[index].compute_xgmi_lease_v1(PHYSICAL_BYTES[index]);
        memory[index]
            .primary_reclaim(&mut foundations[index], loan)
            .unwrap();
        let buffer = Gfx942SdmaBufferV1::compute_xgmi_fixture_v1(
            lease,
            keys[index],
            7,
            u64::from(LOGICAL_BYTES),
            u64::from(LOGICAL_BYTES),
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
    });
    let before = Before {
        owners: allocations
            .each_ref()
            .map(|a| a.owner.ownership_snapshot_for_test_v1()),
        identities: allocations
            .each_ref()
            .map(|a| a.owner.local_native_for_sdma().unwrap().storage_identity()),
        addresses: std::array::from_fn(|index| {
            memory[index]
                .single_device_facts(allocations[index].owner.local_native_for_sdma().unwrap())
                .unwrap()
                .checked_gpu_subrange(0, u64::from(LOGICAL_BYTES), 1)
                .unwrap()
        }),
        memory: memory
            .each_ref()
            .map(PreparationMemoryFixtureV1::observation),
        models: std::array::from_fn(|index| {
            memory[index].primary_loan_state_v1(&foundations[index])
        }),
    };
    let mut root = TransferRoot::new(
        allocations.each_ref().map(|a| a.attachment),
        GPU_IDS,
        LOGICAL_BYTES,
    );
    root.allocations = allocations.map(Some);
    (
        Pair {
            memory,
            foundations,
            queue,
            retakes: [0; 2],
            poisoned: [false; 3],
            retake_fault: None,
            complete_on_submit: false,
            waits: 0,
        },
        root,
        before,
    )
}

impl model_pair_loan::Context for Pair {
    type Loan = LiveQueueModelFoundationLoanV1;
    type Error = ComputeAqlQueueSessionErrorV1;

    fn open(&mut self, endpoint: usize) -> Result<Self::Loan, Self::Error> {
        Ok(self.memory[endpoint].primary_loan(&mut self.foundations[endpoint])?)
    }

    fn retake(&mut self, endpoint: usize, loan: Self::Loan) -> Result<(), Self::Error> {
        self.retakes[endpoint] += 1;
        let selected = self.retake_fault.filter(|fault| fault.0 == endpoint);
        let inject = |fault: Option<(usize, bool, bool)>| {
            if let Some((_, _, panic)) = fault {
                if panic {
                    panic!("composed model retake");
                }
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "composed model retake",
                ));
            }
            Ok(())
        };
        inject(selected.filter(|fault| !fault.1))?;
        self.memory[endpoint].primary_reclaim(&mut self.foundations[endpoint], loan)?;
        inject(selected.filter(|fault| fault.1))
    }

    fn poison_endpoint(&mut self, endpoint: usize) {
        self.memory[endpoint].primary_quarantine_release_v1();
        self.queue.poison();
        self.poisoned[endpoint] = true;
    }

    fn poison_process(&mut self) {
        self.poisoned[2] = true;
    }
}

impl transfer::TransferIo for Pair {
    fn local_unmap(
        &mut self,
        endpoint: usize,
        buffer: &mut ComputeXgmiBufferV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(self.memory[endpoint].compute_xgmi_transition_v1(buffer, 0)?)
    }

    fn peer_transition(
        &mut self,
        endpoint: usize,
        buffer: &mut ComputeXgmiBufferV1,
        mapping: bool,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(
            self.memory[endpoint]
                .compute_xgmi_transition_v1(buffer, if mapping { 1 } else { 2 })?,
        )
    }

    fn local_map(
        &mut self,
        endpoint: usize,
        buffer: &mut ComputeXgmiBufferV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(self.memory[endpoint].compute_xgmi_transition_v1(buffer, 3)?)
    }

    fn submit(
        &mut self,
        source: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        destination: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        bytes: u32,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let [source_memory, destination_memory] = &mut self.memory;
        self.queue.submit(
            source_memory,
            destination_memory,
            source,
            destination,
            bytes,
            custody,
        )?;
        if self.complete_on_submit {
            self.queue.complete(source_memory, custody);
        }
        Ok(())
    }

    fn poll(
        &mut self,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        Ok(self.queue.poll(&mut self.memory[0], custody)?)
    }

    fn wait(
        &mut self,
        timeout: Duration,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.waits += 1;
        Ok(self.queue.wait(&mut self.memory[0], timeout, custody)?)
    }
}

fn begin(pair: &mut Pair, root: &mut TransferRoot) {
    root.begin(pair, |pair, core| core.begin_with(pair))
        .unwrap();
}

fn ready(pair: &mut Pair, root: &mut TransferRoot) {
    pair.queue
        .complete(&mut pair.memory[0], root.core.copy_custody_for_test());
    assert!(root.poll(pair, |pair, core| core.poll_with(pair)).unwrap());
}

fn assert_debits(pair: &Pair, before: &Before) {
    for index in 0..2 {
        let after = pair.memory[index].observation();
        let original = &before.memory[index];
        assert_eq!(
            after
                .device
                .map(|a| (a.used_backing_bytes, a.used_allocation_records)),
            original
                .device
                .map(|a| (a.used_backing_bytes, a.used_allocation_records))
        );
        assert_eq!(
            after
                .host
                .map(|a| (a.used_backing_bytes, a.used_allocation_records)),
            original
                .host
                .map(|a| (a.used_backing_bytes, a.used_allocation_records))
        );
        assert_eq!(
            &after.calls[6..],
            &original.calls[6..],
            "no FREE or VA release"
        );
    }
}

fn assert_models_retaken(pair: &Pair, before: &Before, operations: u64) {
    for index in 0..2 {
        let state = pair.memory[index].primary_loan_state_v1(&pair.foundations[index]);
        assert_eq!(
            state,
            (
                before.models[index].0,
                None,
                before.models[index].2 + operations
            )
        );
        pair.memory[index]
            .primary_authenticate(&pair.foundations[index])
            .unwrap();
    }
}

fn assert_original_owners(root: &TransferRoot, before: &Before, restored: bool) {
    for (index, physical_bytes) in PHYSICAL_BYTES.into_iter().enumerate() {
        let allocation = root.allocations[index].as_ref().unwrap();
        let owner = allocation.owner.ownership_snapshot_for_test_v1();
        assert!(before.owners[index].same_allocation(&owner));
        assert_eq!(allocation.attachment, root.certificates[index]);
        assert_eq!(allocation.byte_len(), u64::from(LOGICAL_BYTES));
        assert_eq!(allocation.physical_byte_len(), physical_bytes as u64);
        assert_eq!(allocation.attachment.pool_generation, 7);
        assert_eq!(owner.local_native().is_some(), restored);
        if restored {
            assert_eq!(owner, before.owners[index]);
        }
    }
}

fn assert_conserved(pair: &Pair, root: &TransferRoot, before: &Before) {
    let mut retained = pair.queue.retained_identities();
    for allocation in root.allocations.iter().flatten() {
        retained.extend(
            allocation
                .owner
                .local_native_for_sdma()
                .map(|lease| lease.storage_identity()),
        );
    }
    for buffer in root.core.buffers.iter().flatten() {
        retained.extend(buffer.compute_xgmi_identities_v1());
    }
    if let Some(completed) = root.core.copy_custody_for_test().completed.as_ref() {
        retained.extend([
            completed.source.lease().storage_identity(),
            completed.destination.lease().storage_identity(),
        ]);
    }
    assert_eq!(
        retained.len(),
        2,
        "exactly two native device authorities remain"
    );
    for identity in before.identities {
        assert_eq!(retained.iter().filter(|&&item| item == identity).count(), 1);
    }
    assert_debits(pair, before);
}

fn assert_packet(pair: &Pair, before: &Before) {
    let snapshot = pair.queue.snapshot(&pair.memory[0]);
    let fence_address = u64::from_le_bytes(snapshot.ring[32..40].try_into().unwrap());
    let expected = crate::sdma::Gfx942SdmaCopySubmissionV1::new(
        before.addresses[0],
        before.addresses[1],
        LOGICAL_BYTES,
        fence_address,
        snapshot.generations[0],
    )
    .unwrap();
    assert_eq!(&snapshot.ring[..64], expected.bytes());
    assert_eq!(snapshot.retained, before.identities);
    assert!(snapshot.uncertain.is_none());
    assert!(!snapshot.poisoned);
    assert_eq!(snapshot.doorbell, 64);
}

fn assert_terminal(pair: &Pair, root: &TransferRoot, before: &Before) {
    assert_eq!(root.phase, Phase::Terminal);
    assert_eq!(pair.poisoned, [true; 3]);
    assert!(
        pair.memory
            .iter()
            .all(PreparationMemoryFixtureV1::primary_is_quarantined_v1)
    );
    assert_original_owners(root, before, false);
    assert_conserved(pair, root, before);
}

#[test]
fn compute_xgmi_composed_async_pending_ready_finish_preserves_packet_models_and_owners() {
    for configured in [false, true] {
        let (mut pair, mut root, before) = fixture(configured);
        begin(&mut pair, &mut root);
        assert_packet(&pair, &before);
        let submitted = pair.queue.snapshot(&pair.memory[0]);
        let ticket = root.core.copy_custody_for_test().ticket;
        assert!(ticket.is_some());
        assert_eq!(root.phase, Phase::Published);
        assert_models_retaken(&pair, &before, 1);
        for operations in 2..=3 {
            assert!(
                !root
                    .poll(&mut pair, |pair, core| core.poll_with(pair))
                    .unwrap()
            );
            assert_eq!(root.phase, Phase::Published);
            assert_eq!(root.core.copy_custody_for_test().ticket, ticket);
            assert!(root.core.copy_custody_for_test().completed.is_none());
            let pending = pair.queue.snapshot(&pair.memory[0]);
            assert_eq!(pending.ring, submitted.ring);
            assert_eq!(pending.control, submitted.control);
            assert_eq!(pending.completions, submitted.completions);
            assert_eq!(pending.retained, submitted.retained);
            assert_eq!(pending.generations, submitted.generations);
            assert_eq!(pending.doorbell, submitted.doorbell);
            for memory in &pair.memory {
                let state = memory.compute_xgmi_snapshot_v1();
                assert_eq!(state.maps, [(GPU_IDS.to_vec(), 0)]);
                assert!(
                    state.unmaps.is_empty(),
                    "Pending cannot unmap the peer authority"
                );
            }
            assert_original_owners(&root, &before, false);
            assert_conserved(&pair, &root, &before);
            assert_models_retaken(&pair, &before, operations);
        }
        ready(&mut pair, &mut root);
        assert_eq!(root.phase, Phase::Ready);
        assert!(pair.queue.retained_identities().is_empty());
        assert!(root.core.copy_custody_for_test().completed.is_some());
        assert_original_owners(&root, &before, false);
        assert_conserved(&pair, &root, &before);
        assert_models_retaken(&pair, &before, 4);
        root.finish(&mut pair, |pair, core| core.finish_with(pair))
            .unwrap();
        assert_eq!(root.phase, Phase::Finished);
        assert_original_owners(&root, &before, true);
        assert_conserved(&pair, &root, &before);
        assert_models_retaken(&pair, &before, 5);
        assert_eq!(pair.retakes, [5, 5]);
        assert_eq!(pair.waits, 0, "async phases never enter the wait loop");
        assert!(root.metadata.iter().all(Option::is_none));
        for (index, physical_bytes) in PHYSICAL_BYTES.into_iter().enumerate() {
            let after = pair.memory[index].observation();
            assert_eq!(after.device, before.memory[index].device);
            assert_eq!(after.host, before.memory[index].host);
            assert_eq!(after.calls[4] - before.memory[index].calls[4], 1);
            assert_eq!(after.calls[5] - before.memory[index].calls[5], 1);
            let state = pair.memory[index].compute_xgmi_snapshot_v1();
            assert_eq!(state.identities, [before.identities[index]]);
            assert_eq!(state.addresses, [before.addresses[index]]);
            assert_eq!(state.physical_bytes, [physical_bytes as u64]);
            assert_eq!(state.states, ["mapped"]);
            assert_eq!(state.maps, [(GPU_IDS.to_vec(), 0)]);
            assert_eq!(state.unmaps, [(GPU_IDS.to_vec(), 0)]);
        }
    }
}

#[test]
fn compute_xgmi_composed_synchronous_transfer_uses_the_same_real_sequence() {
    let (mut pair, mut root, before) = fixture(true);
    pair.complete_on_submit = true;
    root.execute(&mut pair, |pair, core| {
        core.run_with(pair, Duration::from_secs(1))
    })
    .unwrap();
    assert_eq!(root.phase, Phase::Finished);
    assert_eq!(pair.waits, 1);
    assert_original_owners(&root, &before, true);
    assert_conserved(&pair, &root, &before);
    assert_models_retaken(&pair, &before, 1);
}

#[test]
fn compute_xgmi_composed_mapping_prefix_errors_and_unwinds_retain_every_original_authority() {
    for configured in [false, true] {
        for endpoint in 0..2 {
            for stage in 0..4 {
                for panic in [false, true] {
                    let (mut pair, mut root, before) = fixture(configured);
                    if stage >= 2 {
                        begin(&mut pair, &mut root);
                        ready(&mut pair, &mut root);
                    }
                    let retakes = pair.retakes;
                    pair.memory[endpoint].compute_xgmi_fault_v1(
                        stage,
                        u32::from(stage == 1 || stage == 2),
                        false,
                        panic,
                    );
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if stage < 2 {
                            root.begin(&mut pair, |pair, core| core.begin_with(pair))
                        } else {
                            root.finish(&mut pair, |pair, core| core.finish_with(pair))
                        }
                    }));
                    assert_eq!(result.is_err(), panic, "stage={stage} endpoint={endpoint}");
                    if !panic {
                        assert!(result.unwrap().is_err());
                    }
                    assert_eq!(pair.retakes, retakes.map(|n| n + 1));
                    assert_terminal(&pair, &root, &before);
                    let state = root.core.buffers[endpoint]
                        .as_ref()
                        .unwrap()
                        .compute_xgmi_snapshot_v1();
                    assert_eq!(
                        state.progress[stage],
                        (
                            true,
                            (!panic).then_some(true),
                            (!panic).then_some(u32::from(stage == 1 || stage == 2)),
                        )
                    );
                }
            }
        }
    }
}

#[test]
fn compute_xgmi_composed_closing_map_currentness_fault_retains_returned_native_prefix() {
    for configured in [false, true] {
        for endpoint in 0..2 {
            for panic in [false, true] {
                let (mut pair, mut root, before) = fixture(configured);
                // Local unmap checks twice; the peer map's second check follows the native return.
                pair.memory[endpoint].compute_xgmi_currentness_fault_v1(4, panic);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    root.begin(&mut pair, |pair, core| core.begin_with(pair))
                }));
                assert_eq!(result.is_err(), panic);
                if !panic {
                    assert!(result.unwrap().is_err());
                }
                assert_terminal(&pair, &root, &before);
                assert_eq!(pair.retakes, [1; 2]);
                let state = root.core.buffers[endpoint]
                    .as_ref()
                    .unwrap()
                    .compute_xgmi_snapshot_v1();
                assert_eq!(state.slots, [false, false, true]);
                assert_eq!(state.progress[1], (true, Some(true), Some(2)));
                assert_eq!(state.peer_prefix, Some((2, 0, false, false)));
                assert!(pair.queue.retained_identities().is_empty());
            }
        }
    }
}

#[test]
fn compute_xgmi_composed_publication_errors_and_unwinds_keep_real_queue_records_rooted() {
    for operation in ["reset", "ring", "control", "doorbell"] {
        for panic in [false, true] {
            let (mut pair, mut root, before) = fixture(true);
            pair.queue.fault(&mut pair.memory[0], operation, panic);
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.begin(&mut pair, |pair, core| core.begin_with(pair))
            }));
            assert_eq!(result.is_err(), panic, "operation={operation}");
            if !panic {
                assert!(result.unwrap().is_err());
            }
            assert_terminal(&pair, &root, &before);
            assert_eq!(pair.retakes, [1; 2]);
            let snapshot = pair.queue.snapshot(&pair.memory[0]);
            assert_eq!(snapshot.retained, before.identities);
            assert!(snapshot.uncertain.is_some());
            assert!(root.core.copy_custody_for_test().completed.is_none());
        }
    }
}

#[test]
fn compute_xgmi_composed_poll_errors_and_unwinds_never_restore_original_owners() {
    for panic in [false, true] {
        let (mut pair, mut root, before) = fixture(true);
        begin(&mut pair, &mut root);
        let ticket = root.core.copy_custody_for_test().ticket;
        pair.queue.fault(&mut pair.memory[0], "observe", panic);
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.poll(&mut pair, |pair, core| core.poll_with(pair))
        }));
        assert_eq!(result.is_err(), panic);
        if !panic {
            assert!(result.unwrap().is_err());
        }
        assert_terminal(&pair, &root, &before);
        assert_eq!(pair.retakes, [2; 2]);
        assert_eq!(root.core.copy_custody_for_test().ticket, ticket);
        assert!(root.core.copy_custody_for_test().completed.is_none());
        assert_eq!(pair.queue.retained_identities(), before.identities);
    }
}

#[test]
fn compute_xgmi_composed_every_phase_model_retake_boundary_preserves_native_custody() {
    for phase in 0..4 {
        for endpoint in 0..2 {
            for after in [false, true] {
                for panic in [false, true] {
                    let (mut pair, mut root, before) = fixture(true);
                    if phase > 0 {
                        begin(&mut pair, &mut root);
                    }
                    if phase >= 2 {
                        pair.queue
                            .complete(&mut pair.memory[0], root.core.copy_custody_for_test());
                    }
                    if phase == 3 {
                        assert!(
                            root.poll(&mut pair, |pair, core| core.poll_with(pair))
                                .unwrap()
                        );
                    }
                    let retakes = pair.retakes;
                    pair.retake_fault = Some((endpoint, after, panic));
                    let result = catch_unwind(AssertUnwindSafe(|| match phase {
                        0 => root.begin(&mut pair, |pair, core| core.begin_with(pair)),
                        1 | 2 => root
                            .poll(&mut pair, |pair, core| core.poll_with(pair))
                            .map(|_| ()),
                        _ => root.finish(&mut pair, |pair, core| core.finish_with(pair)),
                    }));
                    assert_eq!(result.is_err(), panic);
                    if !panic {
                        assert!(result.unwrap().is_err());
                    }
                    assert_terminal(&pair, &root, &before);
                    assert_eq!(pair.retakes, retakes.map(|n| n + 1));
                    for index in 0..2 {
                        let state =
                            pair.memory[index].primary_loan_state_v1(&pair.foundations[index]);
                        assert_eq!(state.1.is_some(), index == endpoint && !after);
                    }
                    assert_eq!(
                        root.core.copy_custody_for_test().completed.is_some(),
                        phase == 2
                    );
                    let retakes = pair.retakes;
                    assert!(
                        root.poll(&mut pair, |pair, core| core.poll_with(pair))
                            .is_err()
                    );
                    assert_eq!(
                        pair.retakes, retakes,
                        "terminal root never retries native IO"
                    );
                }
            }
        }
    }
}
