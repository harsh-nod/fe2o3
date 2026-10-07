//! CPU mapped-byte execution of the production single-copy driver, not GPU data movement.

#![allow(clippy::result_large_err)]

use super::*;
#[path = "integration_initialized_storage_tests.rs"]
mod initialized_storage;
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

#[path = "integration_sdma_synchronous_tests/synchronous_tests.rs"]
mod synchronous_tests;
