//! Barrier-probe completion and exact signal recycling.

use super::*;

impl CompletionSignalArenaOwnerV1 {
    pub(in super::super) fn bind_barrier_probe(
        &mut self,
    ) -> Result<BoundBarrierProbeV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        if self
            .slots
            .iter()
            .any(|record| record.phase != CompletionSlotPhaseV1::Available)
        {
            return Err(Gfx942CompletionErrorV1::BatchStillRetained);
        }
        let next_probe_id = self
            .next_batch_id
            .checked_add(1)
            .ok_or(Gfx942CompletionErrorV1::BatchIdentityExhausted)?;
        let slot = CompletionSlotLeaseV1 {
            index: 0,
            generation: self.slots[0].generation,
        };
        let signal = ObservedGpuAddressV1::new(self.gpu_base)
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion address"))?;
        let packet = AqlBarrierAndPacketV1::new_unpublished(signal)
            .map_err(Gfx942CompletionErrorV1::BarrierPacketBinding)?;
        self.slots[0].phase = CompletionSlotPhaseV1::Bound {
            batch_id: self.next_batch_id,
        };
        let retention = BarrierProbeRetentionV1 {
            probe_id: self.next_batch_id,
            queue: self.queue,
            signal_mapping: self.signal_mapping,
            slot,
            packet_id: None,
        };
        self.next_batch_id = next_probe_id;
        self.phase = CompletionOwnerPhaseV1::ProbeActive;
        Ok(BoundBarrierProbeV1 { packet, retention })
    }

    pub(in super::super) fn cancel_bound_barrier_probe(
        &mut self,
        retention: BarrierProbeRetentionV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &retention,
            None,
            CompletionSlotPhaseV1::Bound {
                batch_id: retention.probe_id,
            },
        )?;
        self.slots[retention.slot.index as usize].phase = CompletionSlotPhaseV1::Available;
        self.phase = CompletionOwnerPhaseV1::Ready;
        Ok(())
    }

    pub(in super::super) fn mark_barrier_probe_published(
        &mut self,
        mut retention: BarrierProbeRetentionV1,
        packet_id: u64,
    ) -> Result<Gfx942BarrierProbeV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &retention,
            None,
            CompletionSlotPhaseV1::Bound {
                batch_id: retention.probe_id,
            },
        )?;
        self.slots[retention.slot.index as usize].phase = CompletionSlotPhaseV1::Published {
            batch_id: retention.probe_id,
        };
        retention.packet_id = Some(packet_id);
        Ok(Gfx942BarrierProbeV1 { retention })
    }

    pub(in super::super) fn observe_barrier_probe_once<B: NativeCompletionSignalBackendV1>(
        &mut self,
        probe: Gfx942BarrierProbeV1,
        backend: &mut B,
    ) -> Result<Gfx942BarrierProbePollV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &probe.retention,
            probe.retention.packet_id,
            CompletionSlotPhaseV1::Published {
                batch_id: probe.retention.probe_id,
            },
        )?;
        let observations = match backend.observe_batch_acquire(&[probe.retention.slot.index]) {
            Ok(observations) if observations.len() == 1 => observations,
            Ok(_) | Err(Gfx942CompletionErrorV1::Observation) => {
                return self.poison(Gfx942CompletionErrorV1::Observation);
            }
            Err(Gfx942CompletionErrorV1::Currentness) => {
                return self.poison(Gfx942CompletionErrorV1::Currentness);
            }
            Err(_) => return self.poison(Gfx942CompletionErrorV1::Observation),
        };
        match observations[0] {
            AqlCompletionObservationV1::Pending => Ok(Gfx942BarrierProbePollV1::Pending {
                probe,
                progress: Gfx942BarrierProbeProgressV1 {
                    signal: Gfx942TimeoutSignalObservationV1::Pending,
                },
            }),
            AqlCompletionObservationV1::Completed => {
                self.slots[probe.retention.slot.index as usize].phase =
                    CompletionSlotPhaseV1::Completed {
                        batch_id: probe.retention.probe_id,
                    };
                Ok(Gfx942BarrierProbePollV1::Ready {
                    completed: Gfx942CompletedBarrierProbeV1 {
                        retention: probe.retention,
                    },
                    progress: Gfx942BarrierProbeProgressV1 {
                        signal: Gfx942TimeoutSignalObservationV1::Completed,
                    },
                })
            }
            AqlCompletionObservationV1::Unexpected(value) => {
                self.poison(Gfx942CompletionErrorV1::Fault {
                    slot: probe.retention.slot.index,
                    value,
                })
            }
        }
    }

    pub(in super::super) fn wait_barrier_probe_bounded<B: NativeCompletionSignalBackendV1>(
        &mut self,
        mut probe: Gfx942BarrierProbeV1,
        polls: u32,
        backend: &mut B,
    ) -> Result<Gfx942CompletedBarrierProbeV1, Gfx942BarrierProbeWaitFailureV1> {
        if polls > MAX_COMPLETION_POLL_ATTEMPTS_V1 {
            return self
                .poison(Gfx942CompletionErrorV1::InvalidPollBound {
                    requested: polls,
                    maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
                })
                .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal);
        }
        self.validate_barrier_probe(
            &probe.retention,
            probe.retention.packet_id,
            CompletionSlotPhaseV1::Published {
                batch_id: probe.retention.probe_id,
            },
        )
        .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal)?;
        let mut wait = MonotonicWaitV1::without_deadline();
        for poll in 0..polls {
            match self
                .observe_barrier_probe_once(probe, backend)
                .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal)?
            {
                Gfx942BarrierProbePollV1::Pending {
                    probe: pending,
                    progress,
                } => {
                    debug_assert_eq!(progress.packet_count(), 1);
                    debug_assert_eq!(progress.signal(), Gfx942TimeoutSignalObservationV1::Pending);
                    probe = pending;
                    if poll + 1 < polls {
                        wait.pause();
                    }
                }
                Gfx942BarrierProbePollV1::Ready {
                    completed,
                    progress,
                } => {
                    debug_assert_eq!(progress.packet_count(), 1);
                    debug_assert_eq!(
                        progress.signal(),
                        Gfx942TimeoutSignalObservationV1::Completed
                    );
                    return Ok(completed);
                }
            }
        }
        Err(Gfx942BarrierProbeWaitFailureV1::Timeout {
            probe: Box::new(probe),
            polls,
        })
    }

    pub(in super::super) fn recycle_barrier_probe<B: NativeCompletionSignalBackendV1>(
        &mut self,
        completed: Gfx942CompletedBarrierProbeV1,
        backend: &mut B,
    ) -> Result<Gfx942BarrierProbeRecycleObservationV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &completed.retention,
            completed.retention.packet_id,
            CompletionSlotPhaseV1::Completed {
                batch_id: completed.retention.probe_id,
            },
        )?;
        let record = &self.slots[completed.retention.slot.index as usize];
        let Some(next_generation) = record.generation.checked_add(1) else {
            return self.poison(Gfx942CompletionErrorV1::SignalGenerationExhausted);
        };
        self.checked_currentness(backend)?;
        if backend
            .reset_pending_release(completed.retention.slot.index)
            .is_err()
        {
            return self.poison(Gfx942CompletionErrorV1::Recycle);
        }
        self.checked_currentness(backend)?;
        let record = &mut self.slots[completed.retention.slot.index as usize];
        record.generation = next_generation;
        record.phase = CompletionSlotPhaseV1::Available;
        self.phase = CompletionOwnerPhaseV1::Ready;
        Ok(Gfx942BarrierProbeRecycleObservationV1)
    }
}
