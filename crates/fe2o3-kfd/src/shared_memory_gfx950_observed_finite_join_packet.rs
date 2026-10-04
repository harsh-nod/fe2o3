//! Fixed kernel publication and one-shot completion; no queue reuse.

use super::*;
use crate::queue::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionOwnerV1,
};
use crate::wait::MonotonicWaitV1;
use fe2o3_aql::{
    AQL_INVALID_PACKET_HEADER_V1, AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1,
    AqlPreparedKernelDispatchBatchV2,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::shared_memory::gfx950_observed::queue) struct Snapshot {
    pub write: u64,
    pub read: u64,
    pub slot: u32,
    pub header: u16,
    pub setup: u16,
    pub kind: i64,
    pub value: i64,
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
    fn published(self) -> bool {
        // A consumed packet can have its entire header/setup word cleared to
        // INVALID. Accept that observation only with acquired completion; it
        // establishes neither ring capacity nor permission to release memory.
        let packet_word = (self.header == AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1
            && self.setup == 1)
            || (self.header == AQL_INVALID_PACKET_HEADER_V1
                && (self.setup == 1 || (self.setup == 0 && self.value == 0)));
        self.write == 1
            && self.read <= 1
            && self.slot == 0
            && packet_word
            && self.kind == 1
            && matches!(self.value, 0 | 1)
    }
}

#[derive(Default)]
pub(in crate::shared_memory::gfx950_observed::queue) struct Progress {
    pub published: bool,
    pub completed: Option<Snapshot>,
    pub destroyed: bool,
    pub after_destroy: Option<Snapshot>,
    pub released: bool,
    last_read: u64,
}
impl Progress {
    pub fn counters(&mut self, counters: (u64, u64)) -> QueueResult<()> {
        if (!self.published && counters != (0, 0))
            || (self.published
                && (counters.0 != 1 || counters.1 > 1 || counters.1 < self.last_read))
        {
            return Err("fixed kernel queue counter regression".into());
        }
        self.last_read = counters.1;
        Ok(())
    }
    pub fn complete(&mut self, s: Snapshot) -> QueueResult<()> {
        if self.published
            || self.completed.is_some()
            || self.destroyed
            || !s.published()
            || s.value != 0
        {
            return Err("fixed kernel completion phase".into());
        }
        self.published = true;
        self.last_read = s.read;
        self.completed = Some(s);
        Ok(())
    }
    pub fn destroy(&mut self) -> QueueResult<()> {
        if !self.published || self.completed.is_none() || self.destroyed || self.released {
            return Err("fixed kernel destruction phase".into());
        }
        self.destroyed = true;
        Ok(())
    }
    pub fn observe_completed(&mut self, s: Snapshot) -> QueueResult<()> {
        if self.completed.is_none()
            || self.released
            || !s.published()
            || s.value != 0
            || s.read < self.last_read
        {
            return Err("fixed kernel post-completion regression".into());
        }
        self.last_read = s.read;
        if self.destroyed {
            self.after_destroy = Some(s);
        }
        Ok(())
    }
}

pub(in crate::shared_memory::gfx950_observed::queue) trait KernelBackend:
    NativeAqlSubmissionBackendV1
{
    fn snapshot(&mut self) -> QueueResult<Snapshot>;
    fn expired(&self) -> bool;
    fn pause(&mut self);
}

pub(in crate::shared_memory::gfx950_observed::queue) fn complete_one_kernel(
    backend: &mut impl KernelBackend,
    packet: AqlPreparedKernelDispatchV1,
) -> QueueResult<Snapshot> {
    if backend.expired() {
        return Err("kernel deadline before publication".into());
    }
    backend.check_currentness().map_err(describe)?;
    if !backend.snapshot()?.primed() {
        return Err("fixed kernel requires fresh signal and empty queue".into());
    }
    if backend.expired() {
        return Err("kernel deadline before publication".into());
    }
    let mut submission = NativeAqlSubmissionOwnerV1::new(PAGE as u32).map_err(describe)?;
    if submission
        .submit_batch(AqlPreparedKernelDispatchBatchV2::one(packet), backend)
        .map_err(describe)?
        != 0
    {
        return Err("fixed kernel packet ID was not zero".into());
    }
    let mut read = 0;
    loop {
        if backend.expired() {
            return Err("fixed kernel completion deadline".into());
        }
        backend.check_currentness().map_err(describe)?;
        let s = backend.snapshot()?;
        backend.check_currentness().map_err(describe)?;
        if backend.expired() {
            return Err("fixed kernel completion deadline".into());
        }
        if !s.published() || s.read < read {
            return Err("fixed kernel signal/counter/header regression".into());
        }
        read = s.read;
        if s.value == 0 {
            return Ok(s);
        }
        backend.pause();
    }
}

impl NativeOwner {
    fn finite_join_live_check(&mut self) -> QueueResult<()> {
        self.currentness()?;
        self.runtime
            .as_ref()
            .ok_or("missing runtime")?
            .validate_queue_live_process(active_engine(&mut self.memory)?.backend.opener_pid())
            .map_err(describe)?;
        self.check_event()?;
        self.currentness()
    }
    pub(in crate::shared_memory::gfx950_observed::queue) fn execute_finite_join(
        &mut self,
    ) -> QueueResult<()> {
        let w = self.finite_join.as_ref().ok_or("missing fixed workload")?;
        if w.progress.published || w.progress.completed.is_some() {
            return Err("fixed kernel cannot activate twice".into());
        }
        let timeout_ms = w.timeout_ms;
        let packet = self.finite_join_packet()?;
        let start = Instant::now();
        let deadline = start
            .checked_add(Duration::from_millis(u64::from(timeout_ms)))
            .ok_or("kernel deadline overflow")?;
        let result = {
            let mut backend = NativeBackend {
                owner: self,
                wait: MonotonicWaitV1::until(deadline),
                stage: "before publication",
                last: None,
            };
            complete_one_kernel(&mut backend, packet).map_err(|error| {
                format!(
                    "{error}; fixed_kernel_diagnostic_v1 stage={} snapshot={:?} expired={}",
                    backend.stage,
                    backend.last,
                    backend.expired()
                )
            })
        };
        let completed = result?;
        let elapsed = u64::try_from(start.elapsed().as_nanos()).map_err(describe)?;
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        w.progress.complete(completed)?;
        w.dispatch_host_ns = elapsed;
        Ok(())
    }
}

struct NativeBackend<'a> {
    owner: &'a mut NativeOwner,
    wait: MonotonicWaitV1,
    stage: &'static str,
    last: Option<Snapshot>,
}
impl NativeAqlSubmissionBackendV1 for NativeBackend<'_> {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.stage = "currentness";
        if self.wait.expired() {
            return Err(NativeAqlSubmissionErrorV1::Currentness);
        }
        self.owner
            .finite_join_live_check()
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)?;
        if self.wait.expired() {
            return Err(NativeAqlSubmissionErrorV1::Currentness);
        }
        Ok(())
    }
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.stage = "counters";
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
    fn fetch_add_write_acq_rel(&mut self, n: u64) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.stage = "reserve write";
        if n != 1 {
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
            .fetch_add_aql_write_in_current_scope(&mut mapped.control, n)
            .map_err(|_| NativeAqlSubmissionErrorV1::CounterObservation)
    }
    fn write_unpublished(
        &mut self,
        slot: u32,
        bytes: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.stage = "packet body";
        if slot != 0 || !fixed_body(bytes) {
            return Err(NativeAqlSubmissionErrorV1::PacketBody);
        }
        let engine = active_engine(&mut self.owner.memory)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)?;
        let w = self
            .owner
            .finite_join
            .as_ref()
            .ok_or(NativeAqlSubmissionErrorV1::PacketBody)?;
        let expected = (|| {
            let descriptor = resource_va(
                engine,
                w.code.as_ref().ok_or("code")?,
                SharedAllocationPhaseV1::GpuAccessibleExecutable,
            )?
            .checked_add(w.descriptor_offset)
            .ok_or("descriptor overflow")?;
            let kernarg = resource_va(
                engine,
                w.kernarg.as_ref().ok_or("kernarg")?,
                SharedAllocationPhaseV1::GpuAccessibleMutable,
            )?;
            let signal = resource_va(
                engine,
                w.signal.as_ref().ok_or("signal")?,
                SharedAllocationPhaseV1::GpuAccessibleMutable,
            )?;
            Ok::<_, String>([descriptor, kernarg, signal])
        })()
        .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)?;
        for (offset, address) in [32, 40, 56].into_iter().zip(expected) {
            if bytes[offset..offset + 8] != address.to_le_bytes() {
                return Err(NativeAqlSubmissionErrorV1::PacketBody);
            }
        }
        let mapped = self
            .owner
            .mapped
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::PacketBody)?;
        engine
            .write_aql_slot_in_current_scope(&mut mapped.ring, slot, bytes)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)
    }
    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.stage = "release header";
        if slot != 0 || header != AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1 {
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
    fn ring_doorbell_release(&mut self, id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.stage = "doorbell";
        if id != 0 {
            return Err(NativeAqlSubmissionErrorV1::Doorbell);
        }
        self.owner
            .doorbell
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::Doorbell)?
            .store_packet_id_release(id)
            .map_err(|_| NativeAqlSubmissionErrorV1::Doorbell)
    }
}
impl KernelBackend for NativeBackend<'_> {
    fn snapshot(&mut self) -> QueueResult<Snapshot> {
        self.stage = "snapshot";
        self.owner.finite_join_live_check()?;
        let s = self.owner.finite_join_snapshot()?;
        self.last = Some(s);
        self.owner.finite_join_live_check()?;
        Ok(s)
    }
    fn expired(&self) -> bool {
        self.wait.expired()
    }
    fn pause(&mut self) {
        self.stage = "wait";
        self.wait.pause();
    }
}

fn fixed_body(b: &[u8; 64]) -> bool {
    b[0..4] == [1, 0, 1, 0]
        && b[4..12] == [128, 0, 1, 0, 1, 0, 0, 0]
        && b[12..24] == [0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]
        && b[24..32] == [0; 8]
        && b[48..56] == [0; 8]
}

#[cfg(test)]
#[path = "shared_memory_gfx950_observed_finite_join_packet_tests.rs"]
mod tests;
