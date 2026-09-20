//! Engineering-only packet-processing timestamps in the raw GPU clock domain.

use super::*;

trait TimestampBackend {
    fn admit(&mut self, count: usize) -> Result<[u64; 3]>;
    fn begin(&mut self, count: usize) -> Result<()>;
    fn execute(&mut self) -> Result<u64>;
    fn observe(&mut self, slot: u32) -> Result<(u64, u64)>;
    fn finish(&mut self, identity: [u64; 3], count: usize) -> Result<()>;
    fn poison(&mut self);
}

fn run_profiled_batch(backend: &mut impl TimestampBackend, kernels: &[u64]) -> Result<ResponseV1> {
    let result = (|| {
        if kernels.is_empty() || kernels.len() > MAX_ORDERED_BATCH64_DISPATCHES_V1 {
            return Err("timestamp batch count".into());
        }
        let identity = backend.admit(kernels.len())?;
        identity[2]
            .checked_add(kernels.len() as u64)
            .ok_or("timestamp packet frontier overflow")?;
        backend.begin(kernels.len())?;
        let elapsed_ns = backend.execute()?;
        let mut timestamps = Vec::with_capacity(kernels.len());
        for (slot, kernel) in kernels.iter().enumerate() {
            let (start_tick, end_tick) = backend.observe(slot as u32)?;
            if start_tick == 0 || end_tick <= start_tick {
                return Err(format!(
                    "unqualified dispatch timestamps: slot={slot}, start_tick={start_tick}, end_tick={end_tick}"
                ));
            }
            timestamps.push(DispatchTimestampTicksV1 {
                packet_id: identity[2] + slot as u64,
                kernel: *kernel,
                start_tick,
                end_tick,
            });
        }
        backend.finish(identity, kernels.len())?;
        Ok(ResponseV1::DispatchOrderedBatch64ProfiledCompleted {
            device_unique_id: identity[0],
            queue_epoch: identity[1],
            elapsed_ns,
            timestamps,
        })
    })();
    if result.is_err() {
        // Every failure is terminal, including failed timestamp qualification.
        // Do not mutate properties or retry cleanup after uncertain execution.
        backend.poison();
    }
    result
}

struct NativeTimestamps<'a> {
    context: &'a mut Context,
    dispatches: Option<Vec<OrderedBatchDispatchV1>>,
    payload: Option<Vec<u8>>,
    timeout_ms: u32,
    saved_properties: Option<u32>,
}

impl TimestampBackend for NativeTimestamps<'_> {
    fn admit(&mut self, count: usize) -> Result<[u64; 3]> {
        if self.context.ordered_batch_poisoned {
            return Err("timestamp profiling requires an unpoisoned queue".into());
        }
        let dispatches = self
            .dispatches
            .as_ref()
            .ok_or("missing timestamp commands")?;
        let payload = self.payload.as_ref().ok_or("missing timestamp payload")?;
        if count != dispatches.len()
            || ordered_batch64_payload_bytes(dispatches, self.timeout_ms).map_err(explain)?
                != payload.len()
        {
            return Err("timestamp command cardinality or payload".into());
        }
        self.context.check_currentness(true)?;
        self.context.check_idle()?;
        require_completed_frontier(self.context.completed_write, self.context.ring.write())?;
        Ok([
            self.context.unique_id,
            self.context.queue_epoch,
            self.context.ring.write(),
        ])
    }

    fn begin(&mut self, count: usize) -> Result<()> {
        // SAFETY: admit checked the completed frontier and idle/current owner;
        // this exclusive borrow cannot publish while timestamps are cleared.
        unsafe {
            Backend::clear_engineering_dispatch_timestamps(
                &mut self.context.internal[SIGNAL].mapping,
                count,
            )
        }
        .map_err(explain)?;
        // SAFETY: only this idle engineering queue's CPU-owned property word
        // changes; no outstanding dispatch can observe a transition.
        self.saved_properties = Some(
            unsafe {
                Backend::enable_engineering_dispatch_timestamps(
                    &mut self.context.internal[CONTROL].mapping,
                )
            }
            .map_err(explain)?,
        );
        Ok(())
    }

    fn execute(&mut self) -> Result<u64> {
        let dispatches = self
            .dispatches
            .take()
            .ok_or("timestamp commands already consumed")?;
        let expected = dispatches.len();
        let payload = self
            .payload
            .take()
            .ok_or("timestamp payload already consumed")?;
        // SAFETY: the entry retains exactly the existing engineering caller
        // obligations and delegates every dispatch check to ordered64.
        let response = unsafe {
            self.context
                .dispatch_ordered_batch64(dispatches, payload, self.timeout_ms)
        }?;
        match response {
            ResponseV1::DispatchOrderedBatch64Completed {
                completed_dispatches,
                elapsed_ns,
            } if completed_dispatches as usize == expected => Ok(elapsed_ns),
            _ => Err("timestamp ordered64 response identity".into()),
        }
    }

    fn observe(&mut self, slot: u32) -> Result<(u64, u64)> {
        // SAFETY: ordered64 observed every signal complete before returning,
        // and this exclusive context cannot reuse any slot until capture ends.
        unsafe {
            Backend::observe_engineering_dispatch_timestamps(
                &mut self.context.internal[SIGNAL].mapping,
                slot,
            )
        }
        .map_err(explain)
    }

    fn finish(&mut self, identity: [u64; 3], count: usize) -> Result<()> {
        let next = identity[2]
            .checked_add(count as u64)
            .ok_or("timestamp frontier overflow")?;
        require_pending_dispatch_identity(
            [
                self.context.unique_id,
                self.context.queue_epoch,
                self.context.ring.write(),
            ],
            [identity[0], identity[1], next],
            self.context.ordered_batch_poisoned,
        )?;
        require_completed_frontier(self.context.completed_write, next)?;
        self.context.check_currentness(true)?;
        self.context.check_idle()?;
        let saved = self
            .saved_properties
            .ok_or("missing timestamp properties")?;
        // SAFETY: successful completion, stable identity and fresh idle fence
        // precede restoration. No recovery path attempts this operation.
        unsafe {
            Backend::restore_engineering_dispatch_timestamps(
                &mut self.context.internal[CONTROL].mapping,
                saved,
            )
        }
        .map_err(explain)
    }

    fn poison(&mut self) {
        self.context.ordered_batch_poisoned = true;
    }
}

impl Context {
    /// Uses only a disposable trusted-code engineering worker. Any failure is
    /// terminal, with the owner retained until process teardown.
    pub(super) unsafe fn dispatch_ordered_batch64_profiled(
        &mut self,
        dispatches: Vec<OrderedBatchDispatchV1>,
        payload: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<ResponseV1> {
        let kernels = dispatches
            .iter()
            .map(|dispatch| dispatch.kernel)
            .collect::<Vec<_>>();
        run_profiled_batch(
            &mut NativeTimestamps {
                context: self,
                dispatches: Some(dispatches),
                payload: Some(payload),
                timeout_ms,
                saved_properties: None,
            },
            &kernels,
        )
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_dispatch_timestamps_tests.rs"]
mod tests;
