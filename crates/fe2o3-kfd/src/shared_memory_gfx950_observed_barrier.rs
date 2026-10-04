//! One actual zero-dependency barrier; no kernel or reusable queue authority.

use super::*;
use crate::queue::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionOwnerV1,
};
use crate::wait::MonotonicWaitV1;
use fe2o3_aql::{
    AQL_INVALID_PACKET_HEADER_V1, AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1, AqlBarrierAndPacketV1,
    ObservedGpuAddressV1,
};
use std::time::{Duration, Instant};

const TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    write: u64,
    read: u64,
    slot: u32,
    header: u16,
    setup: u16,
    kind: i64,
    value: i64,
}
impl Snapshot {
    fn primed(self) -> bool {
        self.write == 0
            && self.read == 0
            && self.slot == 0
            && self.header == AQL_INVALID_PACKET_HEADER_V1
            && self.setup == 0
            && self.kind == 1
            && self.value == 1
    }
    fn valid_published(self) -> bool {
        self.write == 1
            && self.read <= 1
            && self.slot == 0
            && self.setup == 0
            && matches!(
                self.header,
                AQL_INVALID_PACKET_HEADER_V1 | AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1
            )
            && self.kind == 1
            && matches!(self.value, 0 | 1)
    }
    fn complete_and_retired(self) -> bool {
        self.valid_published() && self.read == 1 && self.value == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Retirement {
    StrictReadV1,
    QueueDestroyV2,
}
impl Retirement {
    fn completed(self, snapshot: Snapshot) -> bool {
        match self {
            Self::StrictReadV1 => snapshot.complete_and_retired(),
            Self::QueueDestroyV2 => snapshot.valid_published() && snapshot.value == 0,
        }
    }
}

pub(super) struct Signal {
    cpu: Option<Cpu<HostVisibleCoherentGttV1>>,
    mapped: Option<Mapped<HostVisibleCoherentGttV1>>,
    pub(super) completed: Option<Snapshot>,
    diagnostic: Diagnostic,
    retirement: Retirement,
    last_read: u64,
    destroyed: bool,
    after_destroy: Option<Snapshot>,
}
impl Signal {
    fn new() -> Self {
        Self {
            cpu: None,
            mapped: None,
            completed: None,
            diagnostic: Diagnostic::default(),
            retirement: Retirement::StrictReadV1,
            last_read: 0,
            destroyed: false,
            after_destroy: None,
        }
    }
    fn destroy_v2() -> Self {
        Self {
            retirement: Retirement::QueueDestroyV2,
            ..Self::new()
        }
    }
    fn record_completion(&mut self, snapshot: Snapshot) -> QueueResult<()> {
        if self.completed.is_some() || !self.retirement.completed(snapshot) {
            return Err("barrier missing or repeated completion".into());
        }
        self.last_read = snapshot.read;
        self.completed = Some(snapshot);
        Ok(())
    }
    pub(super) fn observe_counters(&mut self, (write, read): (u64, u64)) -> QueueResult<()> {
        if self.completed.is_none() {
            if (write, read) != (0, 0) {
                return Err("barrier queue changed before publication".into());
            }
        } else if write != 1
            || read > write
            || read < self.last_read
            || (self.retirement == Retirement::StrictReadV1 && read != 1)
        {
            return Err("barrier read shadow is unbounded or decreasing".into());
        }
        self.last_read = read;
        Ok(())
    }
    fn observe_completed(&mut self, snapshot: Snapshot) -> QueueResult<()> {
        if self.completed.is_none() || !self.retirement.completed(snapshot) {
            return Err("barrier completion changed before resource release".into());
        }
        self.observe_counters((snapshot.write, snapshot.read))?;
        if self.destroyed {
            self.after_destroy = Some(snapshot);
        }
        Ok(())
    }
    pub(super) fn confirm_destroyed(&mut self) -> QueueResult<()> {
        if self.destroyed || self.completed.is_none() {
            return Err("barrier destroy confirmation out of order".into());
        }
        self.destroyed = true;
        Ok(())
    }
    fn release_ready(&self) -> bool {
        self.destroyed
            && self.completed.is_some()
            && self.after_destroy.is_some_and(|snapshot| {
                self.retirement.completed(snapshot) && snapshot.read == self.last_read
            })
    }
    pub(super) fn released(&self) -> bool {
        self.cpu.is_none() && self.mapped.is_none() && self.release_ready()
    }
}

/// Redacted observations returned only after barrier retirement and queue teardown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950ObservedBarrierAndV1 {
    unique_id: u64,
    queue_id: u32,
    doorbell_offset: u64,
    preserved_allocations: usize,
    completed: Snapshot,
}
impl Gfx950ObservedBarrierAndV1 {
    /// Retained selected-device identity, not driver authentication.
    pub fn unique_id(&self) -> u64 {
        self.unique_id
    }
    /// Native queue ID observed only after confirmed destruction.
    pub fn destroyed_queue_id(&self) -> u32 {
        self.queue_id
    }
    /// Offset inside the complete doorbell mapping, never a GPU address.
    pub fn doorbell_byte_offset(&self) -> u64 {
        self.doorbell_offset
    }
    /// Original allocation records preserved by the same-engine operation.
    pub fn preserved_allocation_records(&self) -> usize {
        self.preserved_allocations
    }
    /// Exactly four private queue allocations were released.
    pub fn released_queue_allocations(&self) -> usize {
        4
    }
    /// One private typed completion-signal allocation was released.
    pub fn released_signal_allocations(&self) -> usize {
        1
    }
    /// Exactly one zero-dependency BARRIER_AND was published.
    pub fn published_packets(&self) -> u64 {
        1
    }
    /// Exact acquire-observed write index after completion and retirement.
    pub fn write_index(&self) -> u64 {
        self.completed.write
    }
    /// Exact acquire-observed read index after completion and retirement.
    pub fn read_index(&self) -> u64 {
        self.completed.read
    }
    /// Actual system barrier header or firmware-invalidated consumed header.
    pub fn packet_header(&self) -> u16 {
        self.completed.header
    }
    /// Actual AMD busy-signal kind, checked to equal one.
    pub fn signal_kind(&self) -> i64 {
        self.completed.kind
    }
    /// Actual completion value, checked to transition from one to zero.
    pub fn signal_value(&self) -> i64 {
        self.completed.value
    }
}

/// Publish one barrier on the same observed owner and retire it within 10 seconds.
///
/// This distinct mode never loads a kernel or exposes a queue, address, packet
/// writer or reusable signal. Completion requires both the acquire-observed
/// signal transition 1 -> 0 and exact queue read/write indices 1/1. The original
/// initialized roots are not referenced. Success is returned only after the
/// existing event/runtime/doorbell/memory teardown and full currentness checks.
/// It is an observed firmware interaction, not production execution admission.
/// Any failure retains/quarantines custody until disposable process teardown.
pub fn observe_gfx950_queue_barrier_and_v1(
    session: Gfx950ObservedMemorySessionV1,
) -> Result<(Gfx950ObservedMemorySessionV1, Gfx950ObservedBarrierAndV1), Gfx950ObservedQueueFailureV1>
{
    let mut owner = Box::new(NativeOwner::new(session));
    owner.barrier = Some(Signal::new());
    match run_lifecycle(owner.as_mut()) {
        Ok(()) => {
            let queue = owner.observation.take().expect("successful queue teardown");
            let completed = owner
                .barrier
                .as_mut()
                .expect("barrier mode")
                .completed
                .take()
                .expect("completed and retired signal");
            let observation = Gfx950ObservedBarrierAndV1 {
                unique_id: queue.unique_id,
                queue_id: queue.queue_id,
                doorbell_offset: queue.doorbell_offset,
                preserved_allocations: queue.preserved_allocations,
                completed,
            };
            Ok((
                owner.memory.take().expect("same retained memory owner"),
                observation,
            ))
        }
        Err(message) => Err(Gfx950ObservedQueueFailureV1 {
            message,
            retained: ManuallyDrop::new(owner),
        }),
    }
}

/// One-shot signal completion followed by confirmed native queue destruction.
/// The bounded read shadow is an observation, not reusable ring capacity or a
/// claim that firmware advanced its reported read index before destruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950ObservedBarrierAndV2 {
    queue: Gfx950ObservedQueueLifecycleV1,
    completed: Snapshot,
    after_destroy: Snapshot,
}
impl Gfx950ObservedBarrierAndV2 {
    /// Retained selected-device identity, not driver authentication.
    pub fn unique_id(&self) -> u64 {
        self.queue.unique_id
    }
    /// Actual queue ID after successful DESTROY_QUEUE and ordered teardown.
    pub fn destroyed_queue_id(&self) -> u32 {
        self.queue.queue_id
    }
    /// Doorbell mapping offset, not a reusable address.
    pub fn doorbell_byte_offset(&self) -> u64 {
        self.queue.doorbell_offset
    }
    /// Same-owner original allocation records preserved.
    pub fn preserved_allocation_records(&self) -> usize {
        self.queue.preserved_allocations
    }
    /// Four private queue allocations released after destruction.
    pub fn released_queue_allocations(&self) -> usize {
        4
    }
    /// One private signal allocation released after destruction.
    pub fn released_signal_allocations(&self) -> usize {
        1
    }
    /// Exactly one zero-dependency packet, without reuse or reset.
    pub fn published_packets(&self) -> u64 {
        1
    }
    /// Actual write index at acquired signal completion.
    pub fn completion_write_index(&self) -> u64 {
        self.completed.write
    }
    /// Actual bounded read shadow at acquired signal completion.
    pub fn completion_read_index(&self) -> u64 {
        self.completed.read
    }
    /// Actual write index after destruction and immediately before release.
    pub fn write_index(&self) -> u64 {
        self.after_destroy.write
    }
    /// Actual nondecreasing read shadow after destruction, possibly still zero.
    pub fn read_index(&self) -> u64 {
        self.after_destroy.read
    }
    /// Observed header after destruction and before release.
    pub fn packet_header(&self) -> u16 {
        self.after_destroy.header
    }
    /// Actual checked signal kind.
    pub fn signal_kind(&self) -> i64 {
        self.after_destroy.kind
    }
    /// Acquired zero completion, re-observed before signal release.
    pub fn signal_value(&self) -> i64 {
        self.after_destroy.value
    }
}

/// Publish exactly one barrier, then destroy its queue before releasing memory.
///
/// Signal completion does not fabricate a read index, retire a reusable slot,
/// permit a second packet, or authenticate the loaded driver. Success requires
/// actual DESTROY_QUEUE success, fresh bounded/monotonic counter observations,
/// and the same event/runtime/doorbell/memory teardown as the strict V1 mode.
/// Failures retain custody; the 10-second completion deadline is unchanged.
pub fn observe_gfx950_queue_barrier_and_v2(
    session: Gfx950ObservedMemorySessionV1,
) -> Result<(Gfx950ObservedMemorySessionV1, Gfx950ObservedBarrierAndV2), Gfx950ObservedQueueFailureV1>
{
    let mut owner = Box::new(NativeOwner::new(session));
    owner.barrier = Some(Signal::destroy_v2());
    match run_lifecycle(owner.as_mut()) {
        Ok(()) => {
            let signal = owner.barrier.as_ref().expect("retained completed signal");
            let observation = Gfx950ObservedBarrierAndV2 {
                queue: owner.observation.take().expect("confirmed queue teardown"),
                completed: signal.completed.expect("acquired completion"),
                after_destroy: signal.after_destroy.expect("post-destroy observation"),
            };
            Ok((
                owner.memory.take().expect("same retained memory owner"),
                observation,
            ))
        }
        Err(message) => Err(Gfx950ObservedQueueFailureV1 {
            message,
            retained: ManuallyDrop::new(owner),
        }),
    }
}

trait BarrierBackend: NativeAqlSubmissionBackendV1 {
    fn snapshot(&mut self) -> QueueResult<Snapshot>;
    fn expired(&self) -> bool;
    fn pause(&mut self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticStage {
    Currentness,
    Counters,
    ReserveWrite,
    PacketBody,
    ReleaseHeader,
    Doorbell,
    Snapshot,
    Pause,
}

/// Only returned observations and host operation progress are retained. No
/// address, packet body, mapping, signal handle or reusable authority escapes.
#[derive(Debug, Default, Eq, PartialEq)]
struct Diagnostic {
    attempted: Option<DiagnosticStage>,
    completed: Option<DiagnosticStage>,
    calls: u64,
    currentness_checks: u64,
    snapshots: u64,
    last_snapshot: Option<Snapshot>,
    last_counters: Option<(u64, u64)>,
    reservation_old_write: Option<u64>,
    body_written: bool,
    header_published: bool,
    doorbell_stored: bool,
    deadline_observed_after_error: bool,
}
impl Diagnostic {
    fn attempt(&mut self, stage: DiagnosticStage) {
        self.attempted = Some(stage);
        self.calls = self.calls.saturating_add(1);
    }
    fn complete(&mut self, stage: DiagnosticStage) {
        self.completed = Some(stage);
    }
}

struct Traced<'a, B> {
    backend: &'a mut B,
    diagnostic: &'a mut Diagnostic,
}
impl<B: BarrierBackend> NativeAqlSubmissionBackendV1 for Traced<'_, B> {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::Currentness);
        self.diagnostic.currentness_checks = self.diagnostic.currentness_checks.saturating_add(1);
        self.backend.check_currentness()?;
        self.diagnostic.complete(DiagnosticStage::Currentness);
        Ok(())
    }
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::Counters);
        let counters = self.backend.observe_counters_acquire()?;
        self.diagnostic.last_counters = Some(counters);
        self.diagnostic.complete(DiagnosticStage::Counters);
        Ok(counters)
    }
    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::ReserveWrite);
        let old = self.backend.fetch_add_write_acq_rel(increment)?;
        self.diagnostic.reservation_old_write = Some(old);
        self.diagnostic.complete(DiagnosticStage::ReserveWrite);
        Ok(old)
    }
    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::PacketBody);
        self.backend.write_unpublished(slot, packet)?;
        self.diagnostic.body_written = true;
        self.diagnostic.complete(DiagnosticStage::PacketBody);
        Ok(())
    }
    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::ReleaseHeader);
        self.backend.publish_release_header(slot, header)?;
        self.diagnostic.header_published = true;
        self.diagnostic.complete(DiagnosticStage::ReleaseHeader);
        Ok(())
    }
    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.diagnostic.attempt(DiagnosticStage::Doorbell);
        self.backend.ring_doorbell_release(packet_id)?;
        self.diagnostic.doorbell_stored = true;
        self.diagnostic.complete(DiagnosticStage::Doorbell);
        Ok(())
    }
}
impl<B: BarrierBackend> BarrierBackend for Traced<'_, B> {
    fn snapshot(&mut self) -> QueueResult<Snapshot> {
        self.diagnostic.attempt(DiagnosticStage::Snapshot);
        let snapshot = self.backend.snapshot()?;
        self.diagnostic.snapshots = self.diagnostic.snapshots.saturating_add(1);
        self.diagnostic.last_snapshot = Some(snapshot);
        self.diagnostic.complete(DiagnosticStage::Snapshot);
        Ok(snapshot)
    }
    fn expired(&self) -> bool {
        self.backend.expired()
    }
    fn pause(&mut self) {
        self.diagnostic.attempt(DiagnosticStage::Pause);
        self.backend.pause();
        self.diagnostic.complete(DiagnosticStage::Pause);
    }
}

#[cfg(test)]
fn complete_one_barrier_traced(
    backend: &mut impl BarrierBackend,
    signal_address: u64,
    diagnostic: &mut Diagnostic,
) -> QueueResult<Snapshot> {
    complete_one_barrier_traced_mode(
        backend,
        signal_address,
        diagnostic,
        Retirement::StrictReadV1,
    )
}

fn complete_one_barrier_traced_mode(
    backend: &mut impl BarrierBackend,
    signal_address: u64,
    diagnostic: &mut Diagnostic,
    retirement: Retirement,
) -> QueueResult<Snapshot> {
    let result = complete_one_barrier_mode(
        &mut Traced {
            backend,
            diagnostic,
        },
        signal_address,
        retirement,
    );
    if result.is_err() {
        // A clock-only observation, not a second native read or a diagnosis of
        // the original error. An expired clock does not prove why a check failed.
        diagnostic.deadline_observed_after_error = backend.expired();
    }
    result.map_err(|message| format!("{message}; barrier_diagnostic_v1={diagnostic:?}"))
}

#[cfg(test)]
fn complete_one_barrier(
    backend: &mut impl BarrierBackend,
    signal_address: u64,
) -> QueueResult<Snapshot> {
    complete_one_barrier_mode(backend, signal_address, Retirement::StrictReadV1)
}

fn complete_one_barrier_mode(
    backend: &mut impl BarrierBackend,
    signal_address: u64,
    retirement: Retirement,
) -> QueueResult<Snapshot> {
    if backend.expired() {
        return Err("barrier deadline before publication".into());
    }
    backend.check_currentness().map_err(describe)?;
    if !backend.snapshot()?.primed() {
        return Err("barrier requires fresh pending signal and empty queue".into());
    }
    if backend.expired() {
        return Err("barrier deadline before publication".into());
    }
    let packet = AqlBarrierAndPacketV1::new_unpublished(
        ObservedGpuAddressV1::new(signal_address).map_err(describe)?,
    )
    .map_err(describe)?;
    let mut submission = NativeAqlSubmissionOwnerV1::new(PAGE as u32).map_err(describe)?;
    if submission
        .submit_barrier_and(packet, backend)
        .map_err(describe)?
        != 0
    {
        return Err("barrier packet ID was not zero".into());
    }
    // Completion and retirement are distinct observations. Neither permits an
    // early release or a fabricated CPU store to the signal completion value.
    let mut read = 0;
    let mut completed = false;
    loop {
        if backend.expired() {
            return Err("barrier completion/retirement deadline".into());
        }
        backend.check_currentness().map_err(describe)?;
        let observed = backend.snapshot()?;
        backend.check_currentness().map_err(describe)?;
        if backend.expired() {
            return Err("barrier completion/retirement deadline".into());
        }
        if !observed.valid_published() || observed.read < read || (completed && observed.value != 0)
        {
            return Err("barrier signal/counter/header regression".into());
        }
        read = observed.read;
        completed |= observed.value == 0;
        if retirement.completed(observed) {
            return Ok(observed);
        }
        backend.pause();
    }
}

fn allocate_signal<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    initialize: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
) -> QueueResult<Cpu<HostVisibleCoherentGttV1>> {
    engine.check_currentness().map_err(describe)?;
    let token = engine
        .allocate::<HostVisibleCoherentGttV1>(PAGE)
        .map_err(describe)?;
    let index = engine
        .index(&token, SharedAllocationPhaseV1::CpuWritable)
        .map_err(describe)?;
    let record = &mut engine.allocations[index];
    if record.layout.requested_bytes != PAGE
        || record.profile != SharedGttProfileV1::HostVisibleCoherent
    {
        return Err("private signal allocation identity".into());
    }
    initialize(
        record
            .mapping
            .as_mut()
            .ok_or("missing signal CPU mapping")?,
    )
    .map_err(describe)?;
    engine.check_currentness().map_err(describe)?;
    Ok(token)
}

impl NativeOwner {
    pub(super) fn allocate_barrier_signal(&mut self) -> QueueResult<()> {
        let signal = self.barrier.as_mut().ok_or("missing barrier mode")?;
        if signal.cpu.is_some() || signal.mapped.is_some() || signal.completed.is_some() {
            return Err("barrier signal cannot be allocated twice".into());
        }
        let token = allocate_signal(active_engine(&mut self.memory)?, |mapping| {
            LinuxGfx950MemoryBackend::initialize_engineering_signal_slots(mapping, 1)
        })?;
        signal.cpu = Some(token);
        Ok(())
    }
    pub(super) fn map_barrier_signal(&mut self) -> QueueResult<()> {
        let signal = self.barrier.as_mut().ok_or("missing barrier mode")?;
        let token = signal.cpu.take().ok_or("missing initialized signal")?;
        signal.mapped = Some(
            active_engine(&mut self.memory)?
                .map_mutable(token)
                .map_err(describe)?,
        );
        Ok(())
    }
    fn barrier_snapshot(&mut self) -> QueueResult<Snapshot> {
        self.currentness()?;
        let engine = active_engine(&mut self.memory)?;
        let signal = self
            .barrier
            .as_mut()
            .ok_or("missing barrier mode")?
            .mapped
            .as_mut()
            .ok_or("missing mapped signal")?;
        let (kind, value) = engine
            .observe_completion_signal_state(signal, 0)
            .map_err(describe)?;
        let mapped = self.mapped.as_mut().ok_or("missing queue resources")?;
        let (write, read) = engine
            .observe_aql_counters_in_current_scope(&mut mapped.control)
            .map_err(describe)?;
        let (slot, header, setup) = engine
            .observe_aql_packet_header(&mut mapped.ring, 0)
            .map_err(describe)?;
        self.currentness()?;
        Ok(Snapshot {
            write,
            read,
            slot,
            header,
            setup,
            kind,
            value,
        })
    }
    fn barrier_live_check(&mut self) -> QueueResult<()> {
        self.currentness()?;
        let engine = active_engine(&mut self.memory)?;
        self.runtime
            .as_ref()
            .ok_or("missing runtime")?
            .validate_queue_live_process(engine.backend.opener_pid())
            .map_err(describe)?;
        self.check_event()?;
        self.currentness()
    }
    pub(super) fn execute_barrier(&mut self) -> QueueResult<()> {
        let engine = active_engine(&mut self.memory)?;
        let signal = self.barrier.as_ref().ok_or("missing barrier mode")?;
        if signal.completed.is_some() {
            return Err("barrier cannot be repeated".into());
        }
        let address = resource_va(
            engine,
            signal.mapped.as_ref().ok_or("missing signal")?,
            SharedAllocationPhaseV1::GpuAccessibleMutable,
        )?;
        let retirement = signal.retirement;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(TIMEOUT_MS))
            .ok_or("barrier deadline overflow")?;
        let mut diagnostic = Diagnostic::default();
        let result = complete_one_barrier_traced_mode(
            &mut NativeBackend {
                owner: self,
                wait: MonotonicWaitV1::until(deadline),
            },
            address,
            &mut diagnostic,
            retirement,
        );
        self.barrier.as_mut().expect("retained signal").diagnostic = diagnostic;
        let completed = result?;
        self.barrier
            .as_mut()
            .expect("retained signal")
            .record_completion(completed)
    }
    pub(super) fn check_completed_barrier(&mut self) -> QueueResult<()> {
        if self
            .barrier
            .as_ref()
            .and_then(|signal| signal.completed)
            .is_none()
        {
            return Err("barrier teardown before completion and retirement".into());
        }
        let snapshot = self.barrier_snapshot()?;
        self.barrier
            .as_mut()
            .expect("retained signal")
            .observe_completed(snapshot)
    }
    pub(super) fn release_barrier_signal(&mut self) -> QueueResult<()> {
        self.check_completed_barrier()?;
        let signal = self.barrier.as_mut().ok_or("missing barrier mode")?;
        if !signal.release_ready() {
            return Err(
                "barrier release before confirmed destroy and post-destroy observation".into(),
            );
        }
        let mapped = signal.mapped.take().ok_or("missing mapped signal")?;
        let engine = active_engine(&mut self.memory)?;
        let cpu = engine.unmap_mutable(mapped).map_err(describe)?;
        engine
            .release(cpu, SharedAllocationPhaseV1::CpuWritable)
            .map_err(describe)
    }
}

struct NativeBackend<'a> {
    owner: &'a mut NativeOwner,
    wait: MonotonicWaitV1,
}
impl NativeAqlSubmissionBackendV1 for NativeBackend<'_> {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        if self.wait.expired() {
            return Err(NativeAqlSubmissionErrorV1::Currentness);
        }
        self.owner
            .barrier_live_check()
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)?;
        if self.wait.expired() {
            return Err(NativeAqlSubmissionErrorV1::Currentness);
        }
        Ok(())
    }
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        let engine = active_engine(&mut self.owner.memory)
            .map_err(|_| NativeAqlSubmissionErrorV1::CounterObservation)?;
        let mapped = self
            .owner
            .mapped
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::CounterObservation)?;
        engine
            .observe_aql_counters_in_current_scope(&mut mapped.control)
            .map_err(|_| NativeAqlSubmissionErrorV1::CounterObservation)
    }
    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        if increment != 1 {
            return Err(NativeAqlSubmissionErrorV1::CounterObservation);
        }
        let engine = active_engine(&mut self.owner.memory)
            .map_err(|_| NativeAqlSubmissionErrorV1::CounterObservation)?;
        let mapped = self
            .owner
            .mapped
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::CounterObservation)?;
        engine
            .fetch_add_aql_write_in_current_scope(&mut mapped.control, increment)
            .map_err(|_| NativeAqlSubmissionErrorV1::CounterObservation)
    }
    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        if slot != 0 {
            return Err(NativeAqlSubmissionErrorV1::PacketBody);
        }
        let engine = active_engine(&mut self.owner.memory)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)?;
        let mapped = self
            .owner
            .mapped
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::PacketBody)?;
        engine
            .write_aql_slot_in_current_scope(&mut mapped.ring, slot, packet)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)
    }
    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        if slot != 0 || header != AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1 {
            return Err(NativeAqlSubmissionErrorV1::PacketHeader);
        }
        let engine = active_engine(&mut self.owner.memory)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketHeader)?;
        let mapped = self
            .owner
            .mapped
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::PacketHeader)?;
        engine
            .publish_aql_header_in_current_scope(&mut mapped.ring, slot, header)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketHeader)
    }
    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        if packet_id != 0 {
            return Err(NativeAqlSubmissionErrorV1::Doorbell);
        }
        self.owner
            .doorbell
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::Doorbell)?
            .store_packet_id_release(packet_id)
            .map_err(|_| NativeAqlSubmissionErrorV1::Doorbell)
    }
}
impl BarrierBackend for NativeBackend<'_> {
    fn snapshot(&mut self) -> QueueResult<Snapshot> {
        self.owner.barrier_live_check()?;
        let snapshot = self.owner.barrier_snapshot()?;
        self.owner.barrier_live_check()?;
        Ok(snapshot)
    }
    fn expired(&self) -> bool {
        self.wait.expired()
    }
    fn pause(&mut self) {
        self.wait.pause();
    }
}

#[cfg(test)]
pub(in crate::shared_memory::gfx950_observed) fn exercise_resources<B: MemoryBackend, I, A, R>(
    core: &mut ObservedMemoryCore<B>,
    initialize: I,
    before_signal: A,
    before_release: R,
) -> QueueResult<()>
where
    I: FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
    A: FnOnce(&mut SharedMemoryEngine<B>),
    R: FnOnce(&mut SharedMemoryEngine<B>),
{
    // Real coordinator + generic resource engine; native event/queue actions
    // are inert here. The initializer is injected, not a GPU atomic simulation.
    struct Resources<'a, B: MemoryBackend, I, A, R> {
        core: &'a mut ObservedMemoryCore<B>,
        initialize: Option<I>,
        before_signal: Option<A>,
        before_release: Option<R>,
        cpu: Option<CpuResources>,
        mapped: Option<MappedResources>,
        signal_cpu: Option<Cpu<HostVisibleCoherentGttV1>>,
        signal_mapped: Option<Mapped<HostVisibleCoherentGttV1>>,
    }
    impl<B: MemoryBackend, I, A, R> LifecycleBackend for Resources<'_, B, I, A, R>
    where
        I: FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
        A: FnOnce(&mut SharedMemoryEngine<B>),
        R: FnOnce(&mut SharedMemoryEngine<B>),
    {
        fn preflight(&mut self) -> QueueResult<()> {
            Ok(())
        }
        fn currentness(&mut self) -> QueueResult<()> {
            self.core
                .engine
                .as_mut()
                .ok_or("missing engine")?
                .check_currentness()
                .map_err(describe)
        }
        fn quarantine(&mut self) {
            self.core.quarantine();
        }
        fn step(&mut self, step: Step) -> QueueResult<()> {
            let engine = self.core.engine.as_mut().ok_or("missing engine")?;
            match step {
                Step::Allocate => {
                    self.cpu = Some(allocate_resources(engine, GEOMETRY)?);
                    self.before_signal.take().expect("single allocation hook")(engine);
                    self.signal_cpu = Some(allocate_signal(
                        engine,
                        self.initialize.take().expect("single signal initializer"),
                    )?);
                }
                Step::PrepareEventAndMappings => {
                    let cpu = self.cpu.take().ok_or("missing CPU resources")?;
                    let eop = engine.seal_executable(cpu.eop).map_err(describe)?;
                    let cwsr = engine.seal_executable(cpu.cwsr).map_err(describe)?;
                    self.mapped = Some(MappedResources {
                        ring: engine.map_mutable(cpu.ring).map_err(describe)?,
                        control: engine.map_mutable(cpu.control).map_err(describe)?,
                        eop: engine.map_executable(eop).map_err(describe)?,
                        cwsr: engine.map_executable(cwsr).map_err(describe)?,
                    });
                    self.signal_mapped = Some(
                        engine
                            .map_mutable(
                                self.signal_cpu
                                    .take()
                                    .ok_or("missing signal CPU resource")?,
                            )
                            .map_err(describe)?,
                    );
                }
                Step::ReleaseMemory => {
                    self.before_release.take().expect("single release hook")(engine);
                    let cpu = engine
                        .unmap_mutable(self.signal_mapped.take().ok_or("missing signal mapping")?)
                        .map_err(describe)?;
                    engine
                        .release(cpu, SharedAllocationPhaseV1::CpuWritable)
                        .map_err(describe)?;
                    release_resources(engine, self.mapped.take().ok_or("missing queue mappings")?)?;
                }
                Step::Finish => {
                    if self.cpu.is_some()
                        || self.mapped.is_some()
                        || self.signal_cpu.is_some()
                        || self.signal_mapped.is_some()
                    {
                        return Err("test resource custody not discharged".into());
                    }
                }
                _ => {}
            }
            Ok(())
        }
    }
    run_lifecycle(&mut Resources {
        core,
        initialize: Some(initialize),
        before_signal: Some(before_signal),
        before_release: Some(before_release),
        cpu: None,
        mapped: None,
        signal_cpu: None,
        signal_mapped: None,
    })
}

#[cfg(test)]
#[path = "shared_memory_gfx950_observed_barrier_tests.rs"]
mod tests;
