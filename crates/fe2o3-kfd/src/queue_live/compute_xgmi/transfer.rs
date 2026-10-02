//! Recycled compute data crosses an exact native peer mapping and returns local.

use super::*;
use crate::queue::dispatch_binding::{DispatchDataInputStorageV1, DispatchDataStorageRefV1};
use crate::sdma::ComputeXgmiCopyCustodyV1;
use crate::shared_memory::ComputeXgmiBufferV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

struct DetachedCertificate {
    generation: Option<u64>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    insertion: Option<usize>,
    ordinal: usize,
}

impl DetachedCertificate {
    fn capture(session: &ComputeAqlQueueSessionV1, ordinal: usize) -> Self {
        Self {
            generation: session.detached_dispatch_generation,
            identities: session.detached_data_identities.clone(),
            insertion: session.detached_next_insertion_index,
            ordinal,
        }
    }

    fn matches(&self, session: &ComputeAqlQueueSessionV1) -> bool {
        self.generation == session.detached_dispatch_generation
            && self.identities == session.detached_data_identities
            && self.identities.len() == session.detached_data_count
            && self.insertion == session.detached_next_insertion_index
            && self.ordinal < self.identities.len()
    }
}

pub(super) struct TransferRoot {
    inputs: [Option<Gfx942FixedDispatchDataV1>; 2],
    outputs: [Option<Gfx942FixedDispatchDataV1>; 2],
    certificates: [DetachedCertificate; 2],
    core: TransferCore,
}

pub(super) struct TransferCore {
    pub(super) buffers: [Option<ComputeXgmiBufferV1>; 2],
    pub(super) rosters: [Option<Box<[u32]>>; 2],
    copy: ComputeXgmiCopyCustodyV1,
    progress: Progress,
    bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    SourceLocalUnmap,
    DestinationLocalUnmap,
    SourcePeerMap,
    DestinationPeerMap,
    Submit,
    Wait,
    SourcePeerUnmap,
    DestinationPeerUnmap,
    SourceLocalMap,
    DestinationLocalMap,
}

const STEPS: [Step; 10] = [
    Step::SourceLocalUnmap,
    Step::DestinationLocalUnmap,
    Step::SourcePeerMap,
    Step::DestinationPeerMap,
    Step::Submit,
    Step::Wait,
    Step::SourcePeerUnmap,
    Step::DestinationPeerUnmap,
    Step::SourceLocalMap,
    Step::DestinationLocalMap,
];
const BEGIN_STEP_COUNT: usize = 5;
const FINISH_STEP_START: usize = BEGIN_STEP_COUNT + 1;

#[derive(Default)]
struct Progress {
    attempted: Option<Step>,
    completed: Option<Step>,
}

fn run_steps<E>(
    progress: &mut Progress,
    steps: &[Step],
    mut operation: impl FnMut(Step) -> Result<(), E>,
) -> Result<(), E> {
    for &step in steps {
        progress.attempted = Some(step);
        operation(step)?;
        progress.completed = Some(step);
    }
    Ok(())
}

impl TransferRoot {
    fn prepare(&mut self) {
        for index in 0..2 {
            let data = self.inputs[index]
                .take()
                .unwrap_or_else(|| std::process::abort());
            let DispatchDataInputStorageV1::Device(lease) = data.into_parts().storage else {
                std::process::abort();
            };
            self.core.buffers[index] = Some(ComputeXgmiBufferV1::new(
                lease,
                self.core.rosters[index]
                    .take()
                    .unwrap_or_else(|| std::process::abort()),
            ));
        }
    }

    fn prepare_outputs(&mut self) {
        for index in 0..2 {
            let local = self.core.buffers[index]
                .as_mut()
                .and_then(ComputeXgmiBufferV1::take_local)
                .unwrap_or_else(|| std::process::abort());
            // A complete copy establishes initialization, not an authenticated digest.
            self.outputs[index] = Some(Gfx942FixedDispatchDataV1::initialized_storage(local));
        }
    }
}

impl TransferCore {
    pub(super) fn new(roster: [u32; 2], bytes: u32) -> Self {
        Self {
            buffers: [None, None],
            rosters: [
                Some(Vec::from(roster).into_boxed_slice()),
                Some(Vec::from(roster).into_boxed_slice()),
            ],
            copy: Default::default(),
            progress: Default::default(),
            bytes,
        }
    }

    pub(super) fn run(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        queue: &mut Gfx942NativeXgmiSdmaQueueV1,
        timeout: Duration,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.run_steps(source, destination, queue, timeout, &STEPS)
    }

    pub(super) fn begin(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.run_steps(
            source,
            destination,
            queue,
            Duration::ZERO,
            &STEPS[..BEGIN_STEP_COUNT],
        )
    }

    pub(super) fn poll(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.progress.attempted = Some(Step::Wait);
        let ready = queue.poll_compute_xgmi_rooted_v1(source, destination, &mut self.copy)?;
        if ready {
            require_completed_extent(&self.copy, self.bytes)?;
            self.progress.completed = Some(Step::Wait);
        }
        Ok(ready)
    }

    pub(super) fn finish(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let [source_buffer, destination_buffer] = &mut self.buffers;
        restore_completed_mappings(
            &mut self.copy,
            self.bytes,
            source_buffer
                .as_mut()
                .unwrap_or_else(|| std::process::abort()),
            destination_buffer
                .as_mut()
                .unwrap_or_else(|| std::process::abort()),
        )?;
        self.run_steps(
            source,
            destination,
            queue,
            Duration::ZERO,
            &STEPS[FINISH_STEP_START..],
        )
    }

    fn run_steps(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        queue: &mut Gfx942NativeXgmiSdmaQueueV1,
        timeout: Duration,
        steps: &[Step],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let [source_buffer, destination_buffer] = &mut self.buffers;
        let source_buffer = source_buffer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let destination_buffer = destination_buffer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let route = queue.route();
        run_steps(&mut self.progress, steps, |step| {
            match step {
                Step::SourceLocalUnmap => source.unmap_compute_xgmi_local_v1(source_buffer)?,
                Step::DestinationLocalUnmap => {
                    destination.unmap_compute_xgmi_local_v1(destination_buffer)?
                }
                Step::SourcePeerMap => source.transition_compute_xgmi_peer_v1(
                    destination,
                    route,
                    source_buffer,
                    true,
                )?,
                Step::DestinationPeerMap => destination.transition_compute_xgmi_peer_v1(
                    source,
                    route,
                    destination_buffer,
                    true,
                )?,
                Step::Submit => queue.submit_compute_xgmi_rooted_v1(
                    source,
                    destination,
                    &mut source_buffer.peer,
                    &mut destination_buffer.peer,
                    self.bytes,
                    &mut self.copy,
                )?,
                Step::Wait => {
                    queue.wait_compute_xgmi_rooted_v1(
                        source,
                        destination,
                        timeout,
                        &mut self.copy,
                    )?;
                    restore_completed_mappings(
                        &mut self.copy,
                        self.bytes,
                        source_buffer,
                        destination_buffer,
                    )?;
                }
                Step::SourcePeerUnmap => source.transition_compute_xgmi_peer_v1(
                    destination,
                    route,
                    source_buffer,
                    false,
                )?,
                Step::DestinationPeerUnmap => destination.transition_compute_xgmi_peer_v1(
                    source,
                    route,
                    destination_buffer,
                    false,
                )?,
                Step::SourceLocalMap => source.map_compute_xgmi_local_v1(source_buffer)?,
                Step::DestinationLocalMap => {
                    destination.map_compute_xgmi_local_v1(destination_buffer)?
                }
            }
            Ok(())
        })
    }
}

fn require_completed_extent(
    copy: &ComputeXgmiCopyCustodyV1,
    bytes: u32,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if copy
        .completed
        .as_ref()
        .map(|completed| completed.copy_bytes())
        != Some(bytes)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI completed extent mismatch",
        ));
    }
    Ok(())
}

fn restore_completed_mappings(
    copy: &mut ComputeXgmiCopyCustodyV1,
    bytes: u32,
    source: &mut ComputeXgmiBufferV1,
    destination: &mut ComputeXgmiBufferV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    require_completed_extent(copy, bytes)?;
    if source.peer.is_some() || destination.peer.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI completed mapping slots occupied",
        ));
    }
    let completed = copy
        .completed
        .take()
        .unwrap_or_else(|| std::process::abort());
    source.peer = Some(completed.source);
    destination.peer = Some(completed.destination);
    Ok(())
}

fn require_data(
    session: &ComputeAqlQueueSessionV1,
    data: &Gfx942FixedDispatchDataV1,
    ordinal: usize,
) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
    session.require_unbound_fixed_dispatch()?;
    if !session.unpublished_dispatch.is_clear()
        || session
            .detached_dispatch_generation
            .is_none_or(|generation| generation == 0)
        || session.detached_data_identities.get(ordinal) != Some(&data.storage_identity())
        || !data.is_fully_initialized()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI requires exact recycled initialized data",
        ));
    }
    let DispatchDataStorageRefV1::Device(lease) = data.storage_ref() else {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI requires PUBLIC device data",
        ));
    };
    if lease.layout().uapi_flags() != fe2o3_kfd_uapi::KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI requires PUBLIC device data",
        ));
    }
    let memory = &session
        .engine
        .as_ref()
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "missing compute-XGMI data engine",
        ))?
        .backend
        .session;
    if memory.mapped_gfx942_device_memory_facts(lease)?.vm() != session.key.vm {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI data VM mismatch",
        ));
    }
    Ok(data.layout().requested_bytes())
}

pub(super) fn exact_extent(
    source: u64,
    destination: u64,
) -> Result<u32, ComputeAqlQueueSessionErrorV1> {
    if source == 0
        || source != destination
        || source > u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute-XGMI requires equal complete bounded extents",
        ));
    }
    Ok(source as u32)
}

impl Gfx942ComputeXgmiQueueV1 {
    /// Copies complete, initialized PUBLIC allocations from recycled dispatches.
    ///
    /// Ordinals name the exact detached data in each owning compute session.
    /// Preflight rejection leaves both input slots unchanged. Once admitted,
    /// both slots are empty until both mappings are local and both VM models
    /// have been retaken. Success restores initialized storage, without a stale
    /// content digest. Any subsequent failure (including timeout) is terminal:
    /// this peer queue retains custody and neither endpoint may be released.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_recycled_data_full_extent_with_peer_v1(
        &mut self,
        source: &mut ComputeAqlQueueSessionV1,
        destination: &mut ComputeAqlQueueSessionV1,
        source_ordinal: usize,
        source_data: &mut Option<Gfx942FixedDispatchDataV1>,
        destination_ordinal: usize,
        destination_data: &mut Option<Gfx942FixedDispatchDataV1>,
        timeout: Duration,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.transfer.is_some()
            || self.persistent_transfer.is_some()
            || !attachment_matches(source, destination, self.attachment)
            || self.queue.route() != self.attachment.route
            || self
                .queue
                .observation()
                .is_none_or(|o| o.queue_id != self.attachment.native_queue_id)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute-XGMI transfer attachment mismatch",
            ));
        }
        preflight(source, destination, self.attachment.route)?;
        let bytes = exact_extent(
            require_data(
                source,
                source_data
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing compute-XGMI input",
                    ))?,
                source_ordinal,
            )?,
            require_data(
                destination,
                destination_data
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing compute-XGMI input",
                    ))?,
                destination_ordinal,
            )?,
        )?;
        let certificates = [
            DetachedCertificate::capture(source, source_ordinal),
            DetachedCertificate::capture(destination, destination_ordinal),
        ];
        let roster = self.attachment.route.canonical_mapping_gpu_ids();
        let core = TransferCore::new(roster, bytes);
        self.transfer = Some(TransferRoot {
            inputs: [source_data.take(), destination_data.take()],
            outputs: [None, None],
            certificates,
            core,
        });
        let root = self
            .transfer
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let result = catch_unwind(AssertUnwindSafe(|| {
            model_pair_loan::execute(
                &mut Sessions {
                    source: &mut *source,
                    destination: &mut *destination,
                },
                |sessions| {
                    root.prepare();
                    let (source, destination) = sessions.memories();
                    root.core
                        .run(source, destination, &mut self.queue, timeout)
                        .map_err(|error| Failure {
                            error,
                            terminal: true,
                        })
                },
            )
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(failure)) => {
                self.queue.poison_compute_xgmi_transfer_v1();
                return Err(session_error(failure));
            }
            Err(payload) => {
                self.queue.poison_compute_xgmi_transfer_v1();
                resume_unwind(payload);
            }
        }
        if !root.certificates[0].matches(source) || !root.certificates[1].matches(destination) {
            self.queue.poison_compute_xgmi_transfer_v1();
            source.poison_terminal();
            destination.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute-XGMI detached ledger changed during transfer",
            ));
        }
        root.prepare_outputs();
        source.detached_data_identities[source_ordinal] = root.outputs[0]
            .as_ref()
            .unwrap_or_else(|| std::process::abort())
            .storage_identity();
        destination.detached_data_identities[destination_ordinal] = root.outputs[1]
            .as_ref()
            .unwrap_or_else(|| std::process::abort())
            .storage_identity();
        *source_data = root.outputs[0].take();
        *destination_data = root.outputs[1].take();
        self.transfer = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::{
        persistent_compute_cancellation_test_session, test_queue_key,
    };
    use super::*;

    fn data(id: u64) -> Gfx942FixedDispatchDataV1 {
        Gfx942FixedDispatchDataV1::initialized_storage(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
        )
    }

    #[test]
    fn compute_xgmi_detached_preflight_rejects_wrong_ordinal_variant_and_generation_without_consumption()
     {
        for case in 0..7 {
            let mut session =
                persistent_compute_cancellation_test_session(test_queue_key(7, 1), None, None);
            let input = Some(data(11));
            let identity = input.as_ref().unwrap().storage_identity();
            session.detached_dispatch_generation = Some(9);
            session.detached_data_count = 1;
            session.detached_data_identities = vec![identity];
            match case {
                0 => session.detached_dispatch_generation = None,
                1 => session.detached_dispatch_generation = Some(0),
                2 => session.detached_data_identities[0] = data(12).storage_identity(),
                3 => {
                    session.detached_data_identities[0] = Gfx942FixedDispatchDataV1::uninitialized(
                        crate::shared_memory::local_mapping_for_persistent_sdma_test(11),
                    )
                    .storage_identity()
                }
                4 => session.detached_data_count = 2,
                5 | 6 => {}
                _ => unreachable!(),
            }
            let error = require_data(&session, input.as_ref().unwrap(), usize::from(case == 5))
                .unwrap_err();
            match case {
                0 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )
                )),
                4 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "detached dispatch-data identity ledger"
                    )
                )),
                6 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Contract("missing compute-XGMI data engine")
                )),
                _ => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "compute-XGMI requires exact recycled initialized data"
                    )
                )),
            }
            assert_eq!(input.as_ref().unwrap().storage_identity(), identity);
            assert!(!session.terminal_poisoned);
            session.detached_data_count = 0;
            session.detached_data_identities.clear();
            session.detached_dispatch_generation = None;
        }
    }

    #[test]
    fn compute_xgmi_detached_certificate_checks_complete_roster_and_generation() {
        let mut session =
            persistent_compute_cancellation_test_session(test_queue_key(7, 1), None, None);
        session.detached_dispatch_generation = Some(9);
        session.detached_data_count = 2;
        session.detached_data_identities =
            vec![data(11).storage_identity(), data(12).storage_identity()];
        for case in 0..6 {
            let mut certificate = DetachedCertificate::capture(&session, 0);
            match case {
                0 => {}
                1 => certificate.generation = Some(10),
                2 => certificate.identities[1] = data(13).storage_identity(),
                3 => certificate.identities.pop().map(|_| ()).unwrap(),
                4 => certificate.ordinal = 2,
                5 => certificate.insertion = Some(0),
                _ => unreachable!(),
            }
            assert_eq!(certificate.matches(&session), case == 0);
        }
        session.detached_data_count = 0;
        session.detached_data_identities.clear();
        session.detached_dispatch_generation = None;
    }

    #[test]
    fn compute_xgmi_storage_conversion_discards_both_authenticated_content_descriptors() {
        let session =
            persistent_compute_cancellation_test_session(test_queue_key(7, 1), None, None);
        let role = crate::queue::Gfx942DeviceContentRoleV1::new([1; 32], 0).unwrap();
        let descriptor =
            crate::queue::Gfx942DeviceContentDescriptorV1::new(role, 4096, [2; 32]).unwrap();
        let inputs = [11,12].map(|id| Some(Gfx942FixedDispatchDataV1::initialized(
            crate::shared_memory::Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                crate::shared_memory::local_mapping_for_persistent_sdma_test(id), descriptor).unwrap())));
        let storage = inputs
            .each_ref()
            .map(|input| input.as_ref().unwrap().sdma_storage_identity());
        let mut root = TransferRoot {
            inputs,
            outputs: [None, None],
            certificates: [
                DetachedCertificate::capture(&session, 0),
                DetachedCertificate::capture(&session, 0),
            ],
            core: TransferCore::new([7, 9], 4096),
        };
        root.prepare();
        assert!(root.inputs.iter().all(Option::is_none));
        root.prepare_outputs();
        for (index, output) in root.outputs.into_iter().enumerate() {
            let output = output.unwrap();
            assert_eq!(output.sdma_storage_identity(), storage[index]);
            assert!(matches!(
                output.storage_identity(),
                Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedStorage(_)
            ));
            assert!(output.into_parts().initialized_content.is_none());
        }
    }

    #[test]
    fn compute_xgmi_full_extent_rejects_empty_partial_and_oversized_copies() {
        let max = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        for (source, destination) in [
            (0, 0),
            (1, 0),
            (1, 2),
            (max + 1, max + 1),
            (u64::MAX, u64::MAX),
        ] {
            assert!(exact_extent(source, destination).is_err());
        }
        for size in [1, 4095, 4096, max] {
            assert_eq!(exact_extent(size, size).unwrap(), size as u32);
        }
    }

    #[test]
    fn compute_xgmi_transfer_order_and_every_error_boundary_record_progress() {
        for fail in 0..=STEPS.len() {
            let mut progress = Progress::default();
            let mut trace = Vec::new();
            let result = run_steps(&mut progress, &STEPS, |step| {
                trace.push(step);
                if trace.len() == fail + 1 {
                    Err(())
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.is_ok(), fail == STEPS.len());
            assert_eq!(trace, STEPS[..(fail + 1).min(STEPS.len())]);
            assert_eq!(progress.attempted, trace.last().copied());
            assert_eq!(
                progress.completed,
                fail.checked_sub(1).map(|index| STEPS[index])
            );
        }
    }

    #[test]
    fn compute_xgmi_transfer_records_attempt_before_every_unwind() {
        for (fail, failed_step) in STEPS.iter().copied().enumerate() {
            let mut progress = Progress::default();
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_steps(&mut progress, &STEPS, |step| {
                    if step == failed_step {
                        panic!("injected transition failure");
                    }
                    Ok::<_, ()>(())
                })
            }));
            assert!(result.is_err());
            assert_eq!(progress.attempted, Some(failed_step));
            assert_eq!(
                progress.completed,
                fail.checked_sub(1).map(|index| STEPS[index])
            );
        }
    }

    #[test]
    fn compute_xgmi_async_boundaries_partition_the_same_transfer_sequence_without_waiting() {
        let begin_steps = &STEPS[..BEGIN_STEP_COUNT];
        let finish_steps = &STEPS[FINISH_STEP_START..];
        assert_eq!(
            begin_steps,
            [
                Step::SourceLocalUnmap,
                Step::DestinationLocalUnmap,
                Step::SourcePeerMap,
                Step::DestinationPeerMap,
                Step::Submit,
            ]
        );
        assert_eq!(
            finish_steps,
            [
                Step::SourcePeerUnmap,
                Step::DestinationPeerUnmap,
                Step::SourceLocalMap,
                Step::DestinationLocalMap,
            ]
        );
        let mut progress = Progress::default();
        let mut trace = Vec::new();
        run_steps(&mut progress, begin_steps, |step| {
            trace.push(step);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(progress.completed, Some(Step::Submit));
        assert_eq!(trace, begin_steps);
        assert!(!trace.contains(&Step::Wait));
        assert!(!trace.contains(&Step::SourcePeerUnmap));
        assert!(!trace.contains(&Step::SourceLocalMap));
        progress.attempted = Some(Step::Wait);
        assert_eq!(progress.completed, Some(Step::Submit));
        progress.completed = Some(Step::Wait);
        trace.push(Step::Wait);
        run_steps(&mut progress, finish_steps, |step| {
            trace.push(step);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(trace, STEPS);
        assert_eq!(progress.completed, Some(Step::DestinationLocalMap));
    }
}
