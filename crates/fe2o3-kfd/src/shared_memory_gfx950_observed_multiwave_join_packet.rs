//! Fixed kernel publication and one-shot completion; no queue reuse.

use super::*;
use crate::queue::submit::{NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1};
use crate::wait::MonotonicWaitV1;
#[cfg(test)]
use fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1;
use fe2o3_aql::AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1;
use std::time::{Duration, Instant};

use super::super::finite_join::packet::{KernelBackend, complete_one_kernel};

impl NativeOwner {
    fn multiwave_join_live_check(&mut self) -> QueueResult<()> {
        self.currentness()?;
        self.runtime
            .as_ref()
            .ok_or("missing runtime")?
            .validate_queue_live_process(active_engine(&mut self.memory)?.backend.opener_pid())
            .map_err(describe)?;
        self.check_event()?;
        self.currentness()
    }
    pub(in crate::shared_memory::gfx950_observed::queue) fn execute_multiwave_join(
        &mut self,
    ) -> QueueResult<()> {
        let w = self
            .multiwave_join
            .as_ref()
            .ok_or("missing fixed workload")?;
        if w.progress.published || w.progress.completed.is_some() {
            return Err("fixed kernel cannot activate twice".into());
        }
        let timeout_ms = w.timeout_ms;
        let packet = self.multiwave_join_packet()?;
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
        let w = self
            .multiwave_join
            .as_mut()
            .ok_or("missing fixed workload")?;
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
            .multiwave_join_live_check()
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
            .multiwave_join
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
        self.owner.multiwave_join_live_check()?;
        let s = self.owner.multiwave_join_snapshot()?;
        self.last = Some(s);
        self.owner.multiwave_join_live_check()?;
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
        && b[24..28] == [0; 4]
        && b[28..32] == GROUP_BYTES.to_le_bytes()
        && b[48..56] == [0; 8]
}

#[cfg(test)]
#[path = "shared_memory_gfx950_observed_multiwave_join_packet_tests.rs"]
mod tests;
