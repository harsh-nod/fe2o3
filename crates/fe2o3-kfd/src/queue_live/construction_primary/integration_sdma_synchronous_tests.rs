//! CPU mapped-byte execution of the production single-copy driver, not GPU data movement.

#![allow(clippy::result_large_err)]

use super::*;
use crate::persistent_directional_sdma::Gfx942DirectionalPersistentSdmaTerminalStageV1 as Stage;
use crate::queue::live::sdma_synchronous::{
    self, OutcomeV1, SdmaSynchronousContextV1, SdmaSynchronousCustodyV1, UseV1,
};
use crate::sdma::{
    MappedHostBufferV1, SdmaControlAuthorityV1, SdmaRingAuthorityV1, SdmaSingleMemoryV1,
    SingleSdmaCopyCustodyV1,
};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Currentness(usize),
    ControlRead,
    HostFacts(usize),
    DeviceFacts,
    Reset,
    Ring,
    ControlWrite,
    Doorbell,
    Read,
}

#[derive(Default)]
struct Trace {
    calls: RefCell<Vec<Call>>,
    currentness: Cell<usize>,
    hosts: Cell<usize>,
    fault: Option<(Call, Fault)>,
    mapping_panic: Option<Call>,
    completion: Option<i64>,
    packet: RefCell<Option<[u8; 64]>>,
}

impl Trace {
    fn before(&self, call: Call) -> Result<(), MemorySessionError> {
        self.calls.borrow_mut().push(call);
        self.check(call, false)
    }
    fn check(&self, call: Call, after: bool) -> Result<(), MemorySessionError> {
        match self.fault {
            Some((selected, fault)) if selected == call => match (fault, after) {
                (Fault::Error, false) | (Fault::AfterError, true) => {
                    Err(MemorySessionError::Model("single-copy injected error"))
                }
                (Fault::Panic, false) | (Fault::AfterPanic, true) => {
                    std::panic::panic_any(("single-copy injected panic", call, after))
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }
}

struct Forward<'a> {
    memory: &'a mut Memory,
    trace: &'a Trace,
}

macro_rules! forward {
    ($self:ident, $call:expr, $operation:expr) => {{
        let call = $call;
        $self.trace.before(call)?;
        let result = $operation?;
        $self.trace.check(call, true)?;
        Ok(result)
    }};
}

impl SdmaSingleMemoryV1 for Forward<'_> {
    fn check_queue_operational_currentness(&mut self) -> Result<(), MemorySessionError> {
        let ordinal = self.trace.currentness.get() + 1;
        self.trace.currentness.set(ordinal);
        forward!(
            self,
            Call::Currentness(ordinal),
            self.memory.check_queue_operational_currentness()
        )
    }
    fn observe_aql_control_counters_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
    ) -> Result<(u64, u64), MemorySessionError> {
        if self.trace.mapping_panic == Some(Call::ControlRead) {
            self.memory
                .sdma_resource_panic_v1(control, "observe_aql_counters");
        }
        forward!(
            self,
            Call::ControlRead,
            self.memory
                .observe_aql_control_counters_in_current_scope(control)
        )
    }
    fn single_host_facts(
        &self,
        token: &MappedHostBufferV1,
    ) -> Result<crate::shared_memory::SharedGttMappedResourceFactsV1, MemorySessionError> {
        let ordinal = self.trace.hosts.get() + 1;
        self.trace.hosts.set(ordinal);
        forward!(
            self,
            Call::HostFacts(ordinal),
            self.memory.single_host_facts(token)
        )
    }
    fn single_device_facts(
        &self,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<crate::shared_memory::Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError> {
        forward!(
            self,
            Call::DeviceFacts,
            self.memory.single_device_facts(lease)
        )
    }
    fn overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), MemorySessionError> {
        if self.trace.mapping_panic == Some(Call::Reset) {
            self.memory.sdma_mapping_panic_v1(token, "with_bytes_mut");
        }
        forward!(
            self,
            Call::Reset,
            self.memory
                .overwrite_mapped_host_visible_subrange_in_current_scope(token, offset, bytes)
        )
    }
    fn write_sdma_ring_slot_in_current_scope(
        &mut self,
        ring: &mut SdmaRingAuthorityV1,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        *self.trace.packet.borrow_mut() = Some(*packet);
        if self.trace.mapping_panic == Some(Call::Ring) {
            self.memory.sdma_resource_panic_v1(ring, "write_sdma_slot");
        }
        forward!(
            self,
            Call::Ring,
            self.memory
                .write_sdma_ring_slot_in_current_scope(ring, slot, packet)
        )
    }
    fn publish_sdma_control_write_release_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
        expected: u64,
        new: u64,
    ) -> Result<(), MemorySessionError> {
        if self.trace.mapping_panic == Some(Call::ControlWrite) {
            self.memory
                .sdma_resource_panic_v1(control, "publish_sdma_write_release");
        }
        forward!(
            self,
            Call::ControlWrite,
            self.memory
                .publish_sdma_control_write_release_in_current_scope(control, expected, new)
        )
    }
    fn observe_mapped_host_visible_i64_at_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
    ) -> Result<i64, MemorySessionError> {
        self.trace.before(Call::Read)?;
        if self.trace.mapping_panic == Some(Call::Read) {
            self.memory
                .sdma_mapping_panic_v1(token, "observe_i64_acquire");
        }
        // Simulated device completion goes through the real fixture mapping before real polling.
        if let Some(value) = self.trace.completion {
            self.memory
                .overwrite_mapped_host_visible_subrange_in_current_scope(
                    token,
                    offset,
                    &value.to_le_bytes(),
                )?;
        }
        let value = self
            .memory
            .observe_mapped_host_visible_i64_at_in_current_scope(token, offset)?;
        self.trace.check(Call::Read, true)?;
        Ok(value)
    }
    fn single_doorbell(
        &mut self,
        doorbell: &mut LinuxDoorbellSliceV1,
        write: u64,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        forward!(
            self,
            Call::Doorbell,
            self.memory.single_doorbell(doorbell, write)
        )
    }
}

struct SynchronousParent {
    promotion: PromotionParent,
    root: Option<SdmaSynchronousCustodyV1>,
    trace: Trace,
    loan_count: usize,
    fault_loan: usize,
    opening: Fault,
    closing: Fault,
    seals: usize,
    poisons: usize,
    after_run: Option<fn(&mut SdmaSynchronousCustodyV1)>,
    replacement_use: Option<UseV1>,
    displaced_use: Option<UseV1>,
}

impl SynchronousParent {
    fn new() -> Self {
        Self {
            promotion: PromotionParent::new(),
            root: None,
            trace: Trace {
                completion: Some(1),
                ..Trace::default()
            },
            loan_count: 0,
            fault_loan: 0,
            opening: Fault::None,
            closing: Fault::None,
            seals: 0,
            poisons: 0,
            after_run: None,
            replacement_use: None,
            displaced_use: None,
        }
    }
    fn input(
        &mut self,
        direction: Gfx942PersistentSdmaDirectionV1,
        bytes: usize,
    ) -> DirectionalPersistentSdmaAdmittedRequestV1 {
        let device = self.promotion.buffer(bytes);
        let allocation = promote_in_place(&mut self.promotion, device)
            .unwrap_or_else(|failure| panic!("{:?}", failure.error()));
        let host = self.promotion.base.allocate(true, bytes).unwrap();
        self.promotion.base.calls.clear();
        self.promotion
            .base
            .parent
            .engine
            .backend
            .session
            .enable_sdma_mapped_bytes_v1();
        let p = &mut self.promotion.base.parent;
        p.sdma
            .as_mut()
            .unwrap()
            .seed_single_bytes_for_test_v1(&mut p.engine.backend.session);
        DirectionalPersistentSdmaAdmittedRequestV1 {
            allocation,
            host,
            direction,
            host_offset: 3,
            device_offset: 5,
            copy_bytes: (bytes - 5) as u32,
        }
    }
    fn snapshots(&self) -> Vec<crate::sdma::SingleQueueSnapshotV1> {
        let p = &self.promotion.base.parent;
        p.sdma
            .as_ref()
            .unwrap()
            .single_snapshots_for_test_v1(&p.engine.backend.session)
    }
    fn dispose(mut self, completed: Gfx942DirectionalPersistentSdmaCompletedV1) {
        let (allocation, host, frontier) = completed.into_parts();
        let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
        let (device, _) = demote_directional_persistent_sdma_custody_v1(
            allocation,
            self.promotion.base.outstanding,
        )
        .unwrap();
        self.promotion.base.release(host);
        self.promotion.base.release(device);
        self.promotion.base.shutdown();
    }
}

impl SdmaSynchronousContextV1 for SynchronousParent {
    fn root(&mut self) -> &mut Option<SdmaSynchronousCustodyV1> {
        &mut self.root
    }
    fn opening(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let (result, retake) = execute_live_model_custody_v1(
            self,
            Self::loan,
            |f| {
                Forward {
                    memory: &mut f.promotion.base.parent.engine.backend.session,
                    trace: &f.trace,
                }
                .check_queue_operational_currentness()
                .map_err(Into::into)
            },
            Self::retake,
            sdma_synchronous::poison,
        )?;
        retake?;
        result
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.loan_count += 1;
        self.promotion.base.opening = if self.loan_count == self.fault_loan {
            self.opening
        } else {
            Fault::None
        };
        SdmaAllocationContextV1::loan(&mut self.promotion.base)
    }
    fn run(&mut self, timeout: Duration) -> OutcomeV1 {
        let p = &mut self.promotion.base.parent;
        let result = sdma_synchronous::run_in_place(
            self.root.as_mut().unwrap(),
            p.sdma.as_mut().unwrap(),
            &mut Forward {
                memory: &mut p.engine.backend.session,
                trace: &self.trace,
            },
            timeout,
        );
        let root = self.root.as_mut().unwrap();
        if let Some(mutate) = self.after_run {
            mutate(root);
        }
        if let Some(replacement) = self.replacement_use.take() {
            self.displaced_use = root.usage.replace(replacement);
        }
        result
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.promotion.base.closing = if self.loan_count == self.fault_loan {
            self.closing
        } else {
            Fault::None
        };
        SdmaAllocationContextV1::retake(&mut self.promotion.base, loan)
    }
    fn seal(&mut self) {
        self.seals += 1;
        self.promotion.base.parent.poison_release();
    }
    fn poison(&mut self) {
        self.poisons += 1;
        SdmaAllocationContextV1::poison(&mut self.promotion.base);
    }
}

fn terminal_root(
    failure: Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
) -> (ComputeAqlQueueSessionErrorV1, SdmaSynchronousCustodyV1) {
    let (error, custody) = match failure {
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(failure) => {
            let (error, custody) = failure.into_parts();
            let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(root) = custody
            else {
                panic!("terminal submission")
            };
            (error, root)
        }
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(failure) => {
            let (error, custody) = failure.into_parts();
            let Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(root) = custody
            else {
                panic!("terminal execution")
            };
            (error, root)
        }
    };
    let Gfx942DirectionalPersistentSdmaTerminalStateV1::Synchronous(root) = custody.state else {
        panic!("synchronous root")
    };
    (error, root)
}

fn execute(
    f: &mut SynchronousParent,
    input: DirectionalPersistentSdmaAdmittedRequestV1,
) -> Result<
    Gfx942DirectionalPersistentSdmaCompletedV1,
    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
> {
    sdma_synchronous::execute_in_place(f, input, Duration::ZERO)
}

fn failure_error(
    failure: &Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
) -> &ComputeAqlQueueSessionErrorV1 {
    match failure {
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(failure) => {
            failure.error()
        }
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(failure) => {
            failure.error()
        }
    }
}

#[test]
fn constructed_sdma_synchronous_success_uses_exact_mappings_and_two_model_loans() {
    for direction in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ] {
        for bytes in [17, 4097] {
            let mut f = SynchronousParent::new();
            let input = f.input(direction, bytes);
            let attachment = input.allocation.attachment;
            let host = buffer_observation(&input.host);
            let p = &f.promotion.base.parent;
            let memory = &p.engine.backend.session;
            let host_address = input
                .host
                .checked_gpu_subrange(memory, 3, input.copy_bytes as u64)
                .unwrap();
            let device_address = memory
                .single_device_facts(input.allocation.owner.local_native_for_sdma().unwrap())
                .unwrap()
                .checked_gpu_subrange(5, input.copy_bytes as u64, 1)
                .unwrap();
            let completion_address = p
                .sdma
                .as_ref()
                .unwrap()
                .single_completion_address_for_test_v1(memory, attachment.pair.queue_id(direction));
            let (source, destination) =
                if direction == Gfx942PersistentSdmaDirectionV1::HostToDevice {
                    (host_address, device_address)
                } else {
                    (device_address, host_address)
                };
            let expected_packet = crate::sdma::Gfx942SdmaCopySubmissionV1::new(
                source,
                destination,
                input.copy_bytes,
                completion_address,
                1,
            )
            .unwrap();
            let before = f.snapshots();
            let resources = original_resource_ids(&f.promotion.base.parent);
            let completed = execute(&mut f, input)
                .unwrap_or_else(|failure| panic!("{:?}", failure_error(&failure)));
            assert!(f.root.is_none());
            assert_eq!((f.seals, f.poisons), (0, 0));
            assert_eq!(f.promotion.base.outstanding, 2);
            assert_eq!(f.promotion.base.calls, ["loan", "retake", "loan", "retake"]);
            assert_eq!(f.trace.currentness.get(), 3);
            let after = f.snapshots();
            for (old, new) in before.iter().zip(&after) {
                if old.queue_id != attachment.pair.queue_id(direction) {
                    assert_eq!(old, new);
                    continue;
                }
                assert!(new.records.is_empty() && !new.poisoned);
                assert_eq!(new.generations[0], old.generations[0] + 1);
                assert_eq!(&new.generations[1..], &old.generations[1..]);
                assert_eq!(&new.ring[..64], &f.trace.packet.borrow().unwrap());
                assert_eq!(&new.ring[..64], expected_packet.bytes());
                assert_eq!(&new.ring[64..], &old.ring[64..]);
                let offset = crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1;
                assert_eq!(&new.control[offset..offset + 8], &64u64.to_le_bytes());
                assert_eq!(&new.completions[..8], &1i64.to_le_bytes());
                assert_eq!(&new.completions[8..], &old.completions[8..]);
            }
            assert_eq!(original_resource_ids(&f.promotion.base.parent), resources);
            assert_eq!(completed.direction(), direction);
            assert_eq!(completed.copy_bytes(), (bytes - 5) as u32);
            let window = completed.into_single_packet_window_v1();
            assert_eq!(
                (
                    window.host_offset(),
                    window.device_offset(),
                    window.packet_count()
                ),
                (3, 5, 1)
            );
            let (allocation, actual_host, frontier) = window.into_parts();
            assert_eq!(allocation.attachment, attachment);
            assert_eq!(buffer_observation(&actual_host), host);
            assert_eq!(allocation.owner.live_use_count(), 0);
            assert!(allocation.owner.quarantine_reason().is_none());
            let completed = Gfx942DirectionalPersistentSdmaCompletedV1::from_settled_v1(
                allocation,
                actual_host,
                frontier,
                direction,
                3,
                5,
                (bytes - 5) as u32,
            );
            f.dispose(completed);
        }
    }
}

#[test]
fn constructed_sdma_synchronous_healthy_preparation_retries_cancel_then_succeed() {
    for call in [
        Call::ControlRead,
        Call::HostFacts(1),
        Call::DeviceFacts,
        Call::HostFacts(2),
    ] {
        let mut f = SynchronousParent::new();
        let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
        let before = f.snapshots();
        let attachment = input.allocation.attachment;
        let owner_before = input.allocation.owner.ownership_snapshot_for_test_v1();
        let host_before = buffer_observation(&input.host);
        f.trace.fault = Some((call, Fault::Error));
        let failure = execute(&mut f, input).err().unwrap();
        let Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(failure) =
            failure
        else {
            panic!("submission")
        };
        let (error, custody) = failure.into_parts();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                MemorySessionError::Model("single-copy injected error")
            ))
        ));
        let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable { allocation, host } =
            custody
        else {
            panic!("healthy retry")
        };
        assert_eq!(allocation.attachment, attachment);
        let owner_after = allocation.owner.ownership_snapshot_for_test_v1();
        assert!(owner_after.same_allocation(&owner_before));
        assert_eq!(owner_after.local_native(), owner_before.local_native());
        let (generation, sequence) = owner_before.reservation_history();
        assert_eq!(
            owner_after.reservation_history(),
            (generation + 1, sequence + 1)
        );
        assert_eq!(buffer_observation(&host), host_before);
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert!(allocation.owner.quarantine_reason().is_none());
        assert_eq!(f.snapshots(), before);
        assert_eq!((f.seals, f.poisons), (0, 0));
        f.trace = Trace {
            completion: Some(1),
            ..Trace::default()
        };
        let completed = execute(
            &mut f,
            DirectionalPersistentSdmaAdmittedRequestV1 {
                allocation,
                host,
                direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
                host_offset: 3,
                device_offset: 5,
                copy_bytes: 12,
            },
        )
        .unwrap_or_else(|failure| panic!("{:?}", failure_error(&failure)));
        f.dispose(completed);
    }
}

#[test]
fn constructed_sdma_synchronous_publication_faults_retain_exact_queue_records() {
    for call in [Call::Reset, Call::Ring, Call::ControlWrite, Call::Doorbell] {
        for fault in [
            Fault::Error,
            Fault::AfterError,
            Fault::Panic,
            Fault::AfterPanic,
        ] {
            let mut f = SynchronousParent::new();
            let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 4097);
            let attachment = input.allocation.attachment;
            let source = input.host.storage_identity();
            let destination = attachment.storage_identity;
            f.trace.fault = Some((call, fault));
            f.promotion.base.poison_panics = true;
            let before = f.snapshots();
            let result = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)));
            let root = if matches!(fault, Fault::Panic | Fault::AfterPanic) {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, Call, bool)>(),
                    Some(&(
                        "single-copy injected panic",
                        call,
                        fault == Fault::AfterPanic
                    ))
                );
                assert!(f.poisons > 0);
                f.root.take().unwrap()
            } else {
                let (error, root) = terminal_root(result.unwrap().err().unwrap());
                assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                        MemorySessionError::Model("single-copy injected error")
                    ))
                ));
                assert_eq!(root.stage, Stage::PreparedQueueRetained);
                assert_eq!(
                    root.allocation.as_ref().unwrap().owner.quarantine_reason(),
                    Some(
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate
                    )
                );
                assert_eq!((f.seals, f.poisons), (1, 0));
                root
            };
            assert_eq!(root.allocation.as_ref().unwrap().attachment, attachment);
            assert!(root.host.is_none());
            let Some(SingleSdmaCopyCustodyV1::QueueRetained(ticket)) = root.data else {
                panic!("queue custody")
            };
            let after = f.snapshots();
            for (old, new) in before.iter().zip(&after) {
                if old.queue_id
                    != attachment
                        .pair
                        .queue_id(Gfx942PersistentSdmaDirectionV1::HostToDevice)
                {
                    assert_eq!(old, new);
                } else {
                    assert!(new.poisoned);
                    assert_eq!(
                        new.records,
                        [(ticket, source, destination, 3, 5, 4092, true)]
                    );
                    assert_eq!(new.generations[0], 1);
                    let boundary = match call {
                        Call::Reset => 0,
                        Call::Ring => 1,
                        Call::ControlWrite => 2,
                        Call::Doorbell => 3,
                        _ => unreachable!(),
                    };
                    let after_effect = matches!(fault, Fault::AfterError | Fault::AfterPanic);
                    let reset = boundary > 0 || after_effect;
                    assert_eq!(
                        &new.completions[..8],
                        &if reset { 0i64 } else { 13i64 }.to_le_bytes()
                    );
                    if boundary > 1 || (boundary == 1 && after_effect) {
                        assert_eq!(&new.ring[..64], &f.trace.packet.borrow().unwrap());
                    } else {
                        assert_eq!(new.ring, old.ring);
                    }
                    let offset = crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1;
                    let published = boundary > 2 || (boundary == 2 && after_effect);
                    let mut expected_control = old.control.clone();
                    if published {
                        expected_control[offset..offset + 8].copy_from_slice(&64u64.to_le_bytes());
                    }
                    assert_eq!(new.control, expected_control);
                    assert_eq!(&new.ring[64..], &old.ring[64..]);
                    assert_eq!(&new.completions[8..], &old.completions[8..]);
                }
            }
            assert_eq!(f.promotion.base.outstanding, 2);
        }
    }
}

#[test]
fn constructed_sdma_synchronous_final_currentness_and_retake_have_distinct_custody() {
    for at_retake in [false, true] {
        for fault in [
            Fault::Error,
            Fault::Panic,
            Fault::AfterError,
            Fault::AfterPanic,
        ] {
            let mut f = SynchronousParent::new();
            let input = f.input(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 17);
            let attachment = input.allocation.attachment;
            if at_retake {
                f.fault_loan = 2;
                f.closing = fault;
            } else {
                f.trace.fault = Some((Call::Currentness(3), fault));
            }
            f.promotion.base.poison_panics = true;
            let result = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)));
            let panics = matches!(fault, Fault::Panic | Fault::AfterPanic);
            let root = if panics {
                let panic = result.err().unwrap();
                if at_retake {
                    assert_eq!(
                        panic.downcast_ref::<&str>(),
                        Some(&if fault == Fault::Panic {
                            "allocation retake"
                        } else {
                            "allocation retake after"
                        })
                    );
                } else {
                    assert_eq!(
                        panic.downcast_ref::<(&str, Call, bool)>(),
                        Some(&(
                            "single-copy injected panic",
                            Call::Currentness(3),
                            fault == Fault::AfterPanic
                        ))
                    );
                }
                f.root.take().unwrap()
            } else {
                let (error, root) = terminal_root(result.unwrap().err().unwrap());
                if at_retake {
                    assert!(
                        matches!(error, ComputeAqlQueueSessionErrorV1::Contract(message) if message == if fault == Fault::Error { "allocation retake" } else { "allocation retake after" })
                    );
                } else {
                    assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                            MemorySessionError::Model("single-copy injected error")
                        ))
                    ));
                }
                assert_eq!(
                    root.allocation.as_ref().unwrap().owner.quarantine_reason(),
                    Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
                );
                assert_eq!(
                    root.stage,
                    if at_retake {
                        Stage::CompletedUnrestored
                    } else {
                        Stage::PublishedQueueRetained
                    }
                );
                root
            };
            assert_eq!(root.allocation.as_ref().unwrap().attachment, attachment);
            assert!(root.host.is_none());
            assert_eq!(
                matches!(root.data, Some(SingleSdmaCopyCustodyV1::Completed(_))),
                at_retake
            );
            assert_eq!(
                matches!(root.data, Some(SingleSdmaCopyCustodyV1::QueueRetained(_))),
                !at_retake
            );
            let snapshots = f.snapshots();
            let selected = snapshots
                .iter()
                .find(|s| {
                    s.queue_id
                        == attachment
                            .pair
                            .queue_id(Gfx942PersistentSdmaDirectionV1::DeviceToHost)
                })
                .unwrap();
            assert_eq!(selected.records.len(), usize::from(!at_retake));
            assert_eq!(f.poisons > 0, at_retake || panics);
        }
    }
}

#[test]
fn constructed_sdma_synchronous_timeout_returns_published_lease_and_queue_custody() {
    let mut f = SynchronousParent::new();
    let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
    let attachment = input.allocation.attachment;
    f.trace.completion = None;
    let failure = execute(&mut f, input).err().unwrap();
    let Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(failure) = failure
    else {
        panic!("execution")
    };
    let (error, custody) = failure.into_parts();
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)
    ));
    let Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(pending) = custody else {
        panic!("pending")
    };
    assert_eq!(pending.allocation.attachment, attachment);
    assert_eq!(pending.allocation.owner.live_use_count(), 1);
    assert_eq!(pending.published.sequence(), 1);
    assert!(pending.allocation.owner.quarantine_reason().is_none());
    assert!(f.root.is_none());
    assert_eq!((f.seals, f.poisons), (0, 0));
    assert_eq!(
        f.snapshots().iter().map(|s| s.records.len()).sum::<usize>(),
        1
    );
    assert_eq!(f.trace.currentness.get(), 3);
}

#[test]
fn constructed_sdma_synchronous_preparation_panics_preserve_original_payload_and_request() {
    for call in [
        Call::ControlRead,
        Call::HostFacts(1),
        Call::DeviceFacts,
        Call::HostFacts(2),
    ] {
        for fault in [Fault::Panic, Fault::AfterPanic] {
            for closing in [Fault::None, Fault::Error, Fault::Panic] {
                let mut f = SynchronousParent::new();
                let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
                let owner_before = input.allocation.owner.ownership_snapshot_for_test_v1();
                let attachment = input.allocation.attachment;
                let host_before = buffer_observation(&input.host);
                let before = f.snapshots();
                f.trace.fault = Some((call, fault));
                f.fault_loan = 2;
                f.closing = closing;
                f.promotion.base.poison_panics = true;
                let panic = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)))
                    .err()
                    .unwrap();
                assert_eq!(
                    panic.downcast_ref::<(&str, Call, bool)>(),
                    Some(&(
                        "single-copy injected panic",
                        call,
                        fault == Fault::AfterPanic
                    ))
                );
                let root = f.root.as_ref().unwrap();
                let allocation = root.allocation.as_ref().unwrap();
                assert_eq!(allocation.attachment, attachment);
                let owner_after = allocation.owner.ownership_snapshot_for_test_v1();
                assert!(owner_before.same_allocation(&owner_after));
                assert!(owner_after.local_native().is_none());
                assert_eq!(owner_after.reservation_history(), (2, 2));
                assert!(matches!(root.usage, Some(UseV1::Prepared(_))));
                let Some(SingleSdmaCopyCustodyV1::Request(request)) = &root.data else {
                    panic!("original request")
                };
                assert_eq!(buffer_observation(&request.source), host_before);
                assert_eq!(
                    request.destination.storage_identity(),
                    attachment.storage_identity
                );
                assert!(root.host.is_none());
                assert_eq!(f.snapshots(), before);
                assert_eq!(f.promotion.base.calls, ["loan", "retake", "loan", "retake"]);
                assert!(f.poisons > 0 && f.promotion.base.parent.poisoned);
                assert_eq!(f.promotion.base.outstanding, 2);
            }
        }
    }
}

#[test]
fn constructed_sdma_synchronous_opening_and_fused_loan_failures_keep_inputs() {
    for fault_loan in [1, 2] {
        for fault in [Fault::Error, Fault::Panic] {
            let mut f = SynchronousParent::new();
            let input = f.input(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 17);
            let owner_before = input.allocation.owner.ownership_snapshot_for_test_v1();
            let host_before = buffer_observation(&input.host);
            let before = f.snapshots();
            f.fault_loan = fault_loan;
            f.opening = fault;
            f.promotion.base.poison_panics = true;
            let result = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)));
            let root = if fault == Fault::Panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<&str>(),
                    Some(&"allocation loan")
                );
                f.root.take().unwrap()
            } else {
                let (error, root) = terminal_root(result.unwrap().err().unwrap());
                assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Contract("allocation loan")
                ));
                assert_eq!(
                    root.stage,
                    if fault_loan == 1 {
                        Stage::AdmissionRestored
                    } else {
                        Stage::PreparedRestored
                    }
                );
                root
            };
            let owner_after = root
                .allocation
                .as_ref()
                .unwrap()
                .owner
                .ownership_snapshot_for_test_v1();
            assert!(owner_after.same_allocation(&owner_before));
            assert_eq!(
                owner_after.reservation_history(),
                if fault_loan == 1 { (1, 1) } else { (2, 2) }
            );
            if fault_loan == 1 || fault == Fault::Error {
                assert_eq!(buffer_observation(root.host.as_ref().unwrap()), host_before);
                assert_eq!(owner_after.local_native(), owner_before.local_native());
            } else {
                let Some(SingleSdmaCopyCustodyV1::Request(request)) = &root.data else {
                    panic!("request")
                };
                assert_eq!(buffer_observation(&request.destination), host_before);
                assert!(owner_after.local_native().is_none());
            }
            assert_eq!(f.snapshots(), before);
            assert_eq!(f.poisons > 0, fault == Fault::Panic);
            assert_eq!(f.promotion.base.outstanding, 2);
        }
    }
    for ordinal in [1, 2] {
        let mut f = SynchronousParent::new();
        let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
        let before = f.snapshots();
        f.trace.fault = Some((Call::Currentness(ordinal), Fault::Error));
        let (error, root) = terminal_root(execute(&mut f, input).err().unwrap());
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                "single-copy injected error"
            ))
        ));
        assert_eq!(
            root.stage,
            if ordinal == 1 {
                Stage::AdmissionRestored
            } else {
                Stage::PreparedRestored
            }
        );
        assert!(root.host.is_some() && root.data.is_none());
        assert_eq!(f.snapshots(), before);
        assert_eq!((f.seals, f.poisons), (1, 0));
    }
}

#[test]
fn constructed_sdma_synchronous_retake_error_precedes_lower_error_without_changing_cause() {
    for call in [Call::ControlRead, Call::Ring, Call::Read] {
        let mut f = SynchronousParent::new();
        let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
        f.trace.fault = Some((call, Fault::Error));
        f.fault_loan = 2;
        f.closing = Fault::Error;
        let (error, root) = terminal_root(execute(&mut f, input).err().unwrap());
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Contract("allocation retake")
        ));
        let expected = if call == Call::Ring {
            Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate
        } else {
            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
        };
        assert_eq!(
            root.allocation.as_ref().unwrap().owner.quarantine_reason(),
            Some(expected)
        );
        assert!(f.poisons > 0);
    }
}

#[test]
fn constructed_sdma_synchronous_mapped_backend_panics_preserve_profile_quarantine_and_data() {
    for (call, operation) in [
        (Call::ControlRead, "observe_aql_counters"),
        (Call::Reset, "with_bytes_mut"),
        (Call::Ring, "write_sdma_slot"),
        (Call::ControlWrite, "publish_sdma_write_release"),
        (Call::Read, "observe_i64_acquire"),
    ] {
        let mut f = SynchronousParent::new();
        let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
        let attachment = input.allocation.attachment;
        let host_before = buffer_observation(&input.host);
        f.trace.mapping_panic = Some(call);
        f.promotion.base.poison_panics = true;
        let panic = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)))
            .err()
            .unwrap();
        assert_eq!(
            panic.downcast_ref::<(&str, &str)>(),
            Some(&(
                if call == Call::Reset {
                    "N2 native panic"
                } else {
                    "N1 mapped panic"
                },
                operation
            ))
        );
        let root = f.root.as_ref().unwrap();
        assert_eq!(root.allocation.as_ref().unwrap().attachment, attachment);
        assert_eq!(
            f.promotion
                .base
                .parent
                .engine
                .backend
                .session
                .primary_is_quarantined_v1(),
            matches!(call, Call::Reset | Call::Read)
        );
        assert!(f.poisons > 0);
        assert_eq!(f.promotion.base.outstanding, 2);
        let snapshots = f.snapshots();
        if call == Call::ControlRead {
            let Some(SingleSdmaCopyCustodyV1::Request(request)) = &root.data else {
                panic!("preparation request")
            };
            assert_eq!(buffer_observation(&request.source), host_before);
            assert!(snapshots.iter().all(|s| s.records.is_empty()));
        } else {
            let Some(SingleSdmaCopyCustodyV1::QueueRetained(ticket)) = root.data else {
                panic!("queued data")
            };
            let record = snapshots
                .iter()
                .flat_map(|s| &s.records)
                .find(|r| r.0 == ticket)
                .unwrap();
            assert_eq!(
                (record.1, record.2),
                (host_before.0, attachment.storage_identity)
            );
        }
    }
}

#[test]
fn constructed_sdma_synchronous_wrong_completion_is_terminal_completion_indeterminate() {
    let mut f = SynchronousParent::new();
    let input = f.input(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 17);
    f.trace.completion = Some(7);
    let (error, root) = terminal_root(execute(&mut f, input).err().unwrap());
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract(
            "unexpected SDMA completion value"
        ))
    ));
    assert_eq!(root.stage, Stage::PublishedQueueRetained);
    assert_eq!(
        root.allocation.as_ref().unwrap().owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
    );
    assert_eq!((f.seals, f.poisons), (1, 0));
    assert_eq!(
        f.snapshots().iter().map(|s| s.records.len()).sum::<usize>(),
        1
    );
}

#[test]
fn constructed_sdma_synchronous_restoration_rejection_preserves_data_phase_and_error() {
    for preparation in [true, false] {
        let mut f = SynchronousParent::new();
        let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
        let attachment = input.allocation.attachment;
        let host_before = buffer_observation(&input.host);
        if preparation {
            f.trace.fault = Some((Call::ControlRead, Fault::Error));
        }
        // Inject returned-metadata corruption after the real lower algorithm, before real retake.
        f.after_run = Some(|root| match root.data.as_mut().unwrap() {
            SingleSdmaCopyCustodyV1::Request(request) => request.source_offset += 1,
            SingleSdmaCopyCustodyV1::Completed(completed) => completed.source_offset += 1,
            _ => panic!("expected request or completed metadata"),
        });
        let (error, root) = terminal_root(execute(&mut f, input).err().unwrap());
        assert!(
            matches!(error, ComputeAqlQueueSessionErrorV1::Contract(message) if message == if preparation {
            "synchronous directional persistent SDMA preparation restoration"
        } else { "synchronous directional persistent SDMA completion identity" })
        );
        assert_eq!(
            root.stage,
            if preparation {
                Stage::PreparedUnrestored
            } else {
                Stage::CompletedUnrestored
            }
        );
        assert_eq!(root.allocation.as_ref().unwrap().attachment, attachment);
        assert_eq!(
            root.allocation.as_ref().unwrap().owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
        );
        let (source, destination) = match root.data.as_ref().unwrap() {
            SingleSdmaCopyCustodyV1::Request(request) if preparation => {
                (&request.source, &request.destination)
            }
            SingleSdmaCopyCustodyV1::Completed(completed) if !preparation => {
                (&completed.source, &completed.destination)
            }
            _ => panic!("returned owner phase changed"),
        };
        assert_eq!(buffer_observation(source), host_before);
        assert_eq!(destination.storage_identity(), attachment.storage_identity);
        assert_eq!((f.seals, f.poisons), (1, 0));
    }
}

#[test]
fn constructed_sdma_synchronous_failed_lease_transition_retains_returned_lease_and_restored_owners()
{
    let mut f = SynchronousParent::new();
    let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
    let owner_before = input.allocation.owner.ownership_snapshot_for_test_v1();
    let host_before = buffer_observation(&input.host);
    let mut foreign = f.input(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 17);
    let request =
        Gfx942PersistentUseRequestV1::new(Gfx942PersistentOperationV1::LocalSdmaSource, 0, 17)
            .unwrap();
    let canceled = foreign.allocation.owner.reserve(request, None).unwrap();
    foreign.allocation.owner.cancel_reserved(canceled).unwrap();
    let reserved = foreign.allocation.owner.reserve(request, None).unwrap();
    let prepared = foreign.allocation.owner.prepare(reserved).unwrap();
    let published = foreign.allocation.owner.publish(prepared).unwrap();
    assert_eq!(published.sequence(), 2);
    let foreign_before = foreign.allocation.owner.ownership_snapshot_for_test_v1();
    f.replacement_use = Some(UseV1::Published(published));
    let (_, mut root) = terminal_root(execute(&mut f, input).err().unwrap());
    assert_eq!(root.stage, Stage::SynchronousUnsettled);
    assert!(root.data.is_none());
    assert_eq!(buffer_observation(root.host.as_ref().unwrap()), host_before);
    let allocation = root.allocation.as_ref().unwrap();
    let owner_after = allocation.owner.ownership_snapshot_for_test_v1();
    assert!(owner_after.same_allocation(&owner_before));
    assert_eq!(owner_after.local_native(), owner_before.local_native());
    assert_eq!(allocation.owner.live_use_count(), 1);
    let Some(UseV1::Published(returned)) = root.usage.take() else {
        panic!("returned published lease")
    };
    assert_eq!(returned.sequence(), 2);
    let timeout = foreign.allocation.owner.observe_timeout(returned).unwrap();
    assert_eq!(timeout.into_published().sequence(), 2);
    assert_eq!(
        foreign.allocation.owner.ownership_snapshot_for_test_v1(),
        foreign_before
    );
    assert!(matches!(f.displaced_use, Some(UseV1::Published(_))));
}

#[test]
fn constructed_sdma_synchronous_public_reentry_is_inert_and_unfinished_drop_aborts() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SDMA_SYNCHRONOUS_DROP";
    const TEST: &str = "queue::live::construction_primary::integration_tests::release_cases::sdma_allocation_cases::promotion::synchronous::constructed_sdma_synchronous_public_reentry_is_inert_and_unfinished_drop_aborts";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("synchronous guards checked; dropping retained root"));
        assert!(!stderr.contains("panicked at"));
        return;
    }
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    let mut f = SynchronousParent::new();
    let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
    let owner_before = input.allocation.owner.ownership_snapshot_for_test_v1();
    let host_before = buffer_observation(&input.host);
    let before = f.snapshots();
    let e = &f.promotion.base.parent.engine;
    let loan_before = e.backend.session.primary_loan_state_v1(&e.foundation);
    let mut session = core::mem::ManuallyDrop::new(
        crate::queue::live::tests::persistent_compute_cancellation_test_session(
            f.promotion.owner(),
            None,
            None,
        ),
    );
    session.sdma_synchronous = Some(SdmaSynchronousCustodyV1::new(input));
    for _ in 0..2 {
        for result in [
            session.enable_sdma_copy_engine().map(|_| ()),
            session
                .enable_gfx942_directional_sdma_copy_engines()
                .map(|_| ()),
            session.allocate_sdma_host_buffer(17).map(|_| ()),
            session.allocate_sdma_device_buffer(17, 4096).map(|_| ()),
            session.allocate_sdma_pooled_host_buffer(17).map(|_| ()),
            session
                .allocate_sdma_pooled_device_buffer(17, 4096)
                .map(|_| ()),
            session.trim_sdma_memory_pool().map(|_| ()),
            session.supports_retained_primary_release_v1().map(|_| ()),
            session.preflight_primary_release_v1(),
            session
                .destroy_queue_and_event(QueueDestroyModeV1::Release)
                .map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "unfinished synchronous SDMA copy"
                ))
            ));
        }
    }
    let root = session.sdma_synchronous.as_ref().unwrap();
    assert_eq!(
        root.allocation
            .as_ref()
            .unwrap()
            .owner
            .ownership_snapshot_for_test_v1(),
        owner_before
    );
    assert_eq!(buffer_observation(root.host.as_ref().unwrap()), host_before);
    assert!(root.data.is_none() && root.usage.is_none());
    assert_eq!(f.snapshots(), before);
    let e = &f.promotion.base.parent.engine;
    assert_eq!(
        e.backend.session.primary_loan_state_v1(&e.foundation),
        loan_before
    );
    assert_eq!(f.promotion.base.outstanding, 2);
    assert!(!session.sdma_device_pool.activity_started);
    assert_eq!(
        (
            session.sdma_outstanding_buffers,
            session.sdma_pool_reuse_count
        ),
        (0, 0)
    );
    eprintln!("synchronous guards checked; dropping retained root");
    drop(core::mem::ManuallyDrop::into_inner(session));
    panic!("unfinished synchronous Drop returned");
}

#[test]
fn constructed_sdma_synchronous_native_currentness_failure_preserves_model_and_memory_phase() {
    for ordinal in [1, 2, 3] {
        for panic in [false, true] {
            let mut f = SynchronousParent::new();
            let input = f.input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 17);
            let attachment = input.allocation.attachment;
            let host_before = buffer_observation(&input.host);
            let snapshots = f.snapshots();
            let e = &mut f.promotion.base.parent.engine;
            let loan_before = e.backend.session.primary_loan_state_v1(&e.foundation);
            e.backend
                .session
                .sdma_fail_operational_currentness_v1(ordinal, panic);
            let result = catch_unwind(AssertUnwindSafe(|| execute(&mut f, input)));
            let root = if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N1 native panic", "operational_currentness"))
                );
                f.root.take().unwrap()
            } else {
                let (error, root) = terminal_root(result.unwrap().err().unwrap());
                assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                        "operational_currentness"
                    )) | ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
                        MemorySessionError::Injected("operational_currentness")
                    ))
                ));
                root
            };
            assert_eq!(root.allocation.as_ref().unwrap().attachment, attachment);
            let e = &f.promotion.base.parent.engine;
            assert_eq!(e.backend.session.primary_is_quarantined_v1(), !panic);
            assert!(e.backend.foundation_in_engine);
            assert_eq!(
                e.backend.session.primary_loan_state_v1(&e.foundation),
                (
                    loan_before.0,
                    None,
                    loan_before.2 + if ordinal == 1 { 1 } else { 2 }
                )
            );
            assert_eq!(f.poisons > 0, panic);
            let after = f.snapshots();
            for (old, new) in snapshots.iter().zip(&after) {
                if ordinal < 3
                    || old.queue_id
                        != attachment
                            .pair
                            .queue_id(Gfx942PersistentSdmaDirectionV1::HostToDevice)
                {
                    assert_eq!(old, new);
                } else {
                    assert_eq!(new.records.len(), 1);
                    let record = &new.records[0];
                    assert_eq!(
                        (record.1, record.2, record.3, record.4, record.5),
                        (host_before.0, attachment.storage_identity, 3, 5, 12)
                    );
                    assert!(matches!(
                        root.data,
                        Some(SingleSdmaCopyCustodyV1::QueueRetained(_))
                    ));
                }
            }
        }
    }
}
