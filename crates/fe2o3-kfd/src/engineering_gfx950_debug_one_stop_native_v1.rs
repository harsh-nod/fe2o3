//! Actual fixed target operations. No second client, mode1 token, or generic dispatch.
use super::super::super::super::{CONTROL, KERNARG, RING, SIGNAL};
use super::super::super::Backend;
use super::resources;
use super::{E, EmptyPhase, EmptyStep, Inner, OneStopInner, S, checkpoint, contract};
use crate::engineering_gfx950_profile::{PAGE_BYTES, RING_BYTES};
use crate::memory::MemoryBackend;
use fe2o3_aql::{
    AqlDispatchGeometryV1, AqlDispatchOrderingV1, AqlKernelDispatchPacketV1,
    AqlPacketPublicationTargetV1, AqlPreparedKernelDispatchV1, ObservedGpuAddressV1,
};
use fe2o3_kfd_uapi::{KfdAllocMemoryFlags, KfdIoctlDestroyQueueArgs};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

pub(super) fn check_deadline(deadline: Option<Instant>) -> Result<(), E> {
    if deadline.is_none_or(|end| !before_deadline(Instant::now(), end)) {
        Err(E::Contract("one-stop original deadline"))
    } else {
        Ok(())
    }
}
pub(super) fn before_deadline(now: Instant, end: Instant) -> bool {
    now < end
}
fn current(base: &mut Inner) -> Result<(), E> {
    // Before any inherited lock, native call or memory dereference.
    if base.cold.resources.get().context.backend.opener_pid() != std::process::id() {
        return Err(E::ProcessChanged);
    }
    base.cold
        .resources
        .get_mut()
        .context
        .check_currentness(true)?;
    Ok(())
}
pub(super) fn execute(
    base: &mut Inner,
    packet: &mut Option<AqlPreparedKernelDispatchV1>,
    snapshot: &mut Option<checkpoint::Checkpoint>,
    image_sha256: &mut [u8; 32],
    deadline: &mut Option<Instant>,
    completed: &mut bool,
    step: S,
) -> Result<(), E> {
    current(base)?;
    if deadline.is_some() {
        check_deadline(*deadline)?;
    }
    match step {
        S::ValidateFixedArtifact => {
            let c = &mut base.cold.resources.get_mut().context;
            let kernel = c
                .kernels
                .get(&1)
                .ok_or(E::Contract("fixed one-stop kernel"))?;
            let closure = contract::validate_object(&kernel.object)?;
            if closure.identity_inputs().closure_sha256() != base.closure {
                return Err(E::Contract("fixed one-stop closure"));
            }
            *image_sha256 =
                Backend::with_bytes(&kernel.code.mapping, kernel.code.backing, |bytes| {
                    Sha256::digest(bytes).into()
                });
            let logical = base
                .geometry
                .projected_logical_bytes()
                .checked_add(core::mem::size_of::<OneStopInner>() as u64)
                .ok_or(E::Contract("one-stop host envelope overflow"))?;
            if logical > 256 * 1024 * 1024 {
                return Err(E::Contract("one-stop retained logical envelope"));
            }
            // Reserve all seven internal slots before runtime/trap registration.
            c.internal
                .try_reserve_exact(7)
                .map_err(|_| E::Contract("one-stop seven retained slots"))?;
            if !c.internal.is_empty() || c.internal.capacity() != 7 {
                return Err(E::Contract("one-stop slot capacity"));
            }
        }
        S::PrepareEmpty => base.until(EmptyPhase::Ready(EmptyStep::DestroyQueue))?,
        S::AllocateOutput => {
            let c = &mut base.cold.resources.get_mut().context;
            if c.internal.len() != 6 || c.internal.capacity() != 7 {
                return Err(E::Contract("one-stop output slot"));
            }
            let output = c.allocate_resource(
                contract::OUTPUT_LOGICAL,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                contract::initialize_output,
            )?;
            // No fallible work or allocation between native return and retention.
            c.internal.push(output);
            resources::check(base)?;
        }
        S::PreparePacket => {
            if packet.is_some() || snapshot.is_some() {
                return Err(E::Phase);
            }
            resources::check(base)?;
            let c = &mut base.cold.resources.get_mut().context;
            let output = c.internal[contract::OUTPUT]
                .va
                .checked_add(8)
                .ok_or(E::Contract("output payload address"))?;
            let kernel = c.kernels.get(&1).ok_or(E::Contract("kernel custody"))?;
            let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1])
                .map_err(|e| E::Native(format!("{e:?}")))?;
            let mut args = [0_u8; 264];
            args[..8].copy_from_slice(&output.to_le_bytes());
            crate::queue::dispatch_binding::initialize_engineering_cov6_kernarg(
                &kernel.inspected,
                geometry,
                &mut args,
            )
            .map_err(|e| E::Native(format!("{e:?}")))?;
            contract::check_kernarg(&args, output)?;
            let descriptor = kernel
                .code
                .va
                .checked_add(kernel.descriptor_offset)
                .ok_or(E::Contract("descriptor address"))?;
            Backend::with_bytes_mut(&mut c.internal[KERNARG].mapping, 65536, |bytes| {
                bytes.fill(0);
                bytes[..264].copy_from_slice(&args);
            });
            Backend::reset_completion_signal_release(
                &mut c.internal[SIGNAL].mapping,
                PAGE_BYTES,
                0,
            )
            .map_err(|e| E::Native(format!("{e:?}")))?;
            *packet = Some(
                AqlKernelDispatchPacketV1::new_unpublished(
                    geometry,
                    0,
                    0,
                    ObservedGpuAddressV1::new(descriptor)
                        .map_err(|e| E::Native(format!("{e:?}")))?,
                    ObservedGpuAddressV1::new(c.internal[KERNARG].va)
                        .map_err(|e| E::Native(format!("{e:?}")))?,
                    8,
                    ObservedGpuAddressV1::new(c.internal[SIGNAL].va)
                        .map_err(|e| E::Native(format!("{e:?}")))?,
                )
                .map_err(|e| E::Native(format!("{e:?}")))?,
            );
            resources::check_prepublication(base)?;
            *snapshot = Some(checkpoint::capture(
                base,
                packet.as_ref().ok_or(E::Phase)?,
                *image_sha256,
            )?);
        }
        S::OwnedCheckpoint => {
            check_deadline(*deadline)?;
            resources::check_prepublication(base)?;
            let actual =
                checkpoint::capture(base, packet.as_ref().ok_or(E::Phase)?, *image_sha256)?;
            if snapshot.as_ref() != Some(&actual) {
                return Err(E::Contract("one-stop checkpoint substitution"));
            }
            // Same immutable boxed storage before and after the source rendezvous.
            // Returning does NOT prove any debugger gate; that is the unsafe
            // caller's explicit same-client/supervisor obligation.
            checkpoint::rendezvous(snapshot.as_ref().ok_or(E::Phase)?);
            check_deadline(*deadline)?;
            current(base)?;
            resources::check_prepublication(base)?;
            let after = checkpoint::capture(base, packet.as_ref().ok_or(E::Phase)?, *image_sha256)?;
            if snapshot.as_ref() != Some(&after) {
                return Err(E::Contract("one-stop checkpoint currentness"));
            }
        }
        S::ReservePacket => {
            resources::check_prepublication(base)?;
            let c = &mut base.cold.resources.get_mut().context;
            let r = c
                .ring
                .reserve_one(c.last_observed_read)
                .map_err(|e| E::Native(format!("{e:?}")))?;
            if r.packet_id() != 0 || r.slot_index() != 0 || r.next_write() != 1 {
                return Err(E::Contract("one-stop sole model reservation"));
            }
        }
        S::WriteCounter => {
            let c = &mut base.cold.resources.get_mut().context;
            let prior =
                Backend::fetch_add_aql_write(&mut c.internal[CONTROL].mapping, PAGE_BYTES, 1)
                    .map_err(|e| E::Native(format!("{e:?}")))?;
            if prior != 0 {
                return Err(E::Contract("one-stop sole native counter reservation"));
            }
        }
        S::WriteBody => {
            let bytes = &snapshot.as_ref().ok_or(E::Phase)?.packet_bytes;
            Backend::write_aql_slot(
                &mut base.cold.resources.get_mut().context.internal[RING].mapping,
                RING_BYTES,
                0,
                bytes,
            )
            .map_err(|e| E::Native(format!("{e:?}")))?;
        }
        S::ReleaseHeader => {
            Backend::publish_aql_header(
                &mut base.cold.resources.get_mut().context.internal[RING].mapping,
                RING_BYTES,
                0,
                AqlDispatchOrderingV1::Independent.header(),
            )
            .map_err(|e| E::Native(format!("{e:?}")))?;
        }
        S::Doorbell => base
            .cold
            .resources
            .get_mut()
            .context
            .doorbell
            .as_mut()
            .ok_or(E::Contract("doorbell custody"))?
            .store_packet_id_release(0)
            .map_err(|e| E::Native(format!("{e:?}")))?,
        S::ObserveCompletion => {
            if *completed {
                return Err(E::Phase);
            }
            loop {
                check_deadline(*deadline)?;
                current(base)?;
                let c = &mut base.cold.resources.get_mut().context;
                if c.queue_epoch != 0
                    || c.ring.write() != 1
                    || c.completed_write != 0
                    || c.last_observed_read != 0
                {
                    return Err(E::Contract("one-stop in-flight owner frontier"));
                }
                let signal = Backend::observe_completion_signal_state_acquire(
                    &mut c.internal[SIGNAL].mapping,
                    PAGE_BYTES,
                    0,
                )
                .map_err(|e| E::Native(format!("{e:?}")))?;
                let counters =
                    Backend::observe_aql_counters(&mut c.internal[CONTROL].mapping, PAGE_BYTES)
                        .map_err(|e| E::Native(format!("{e:?}")))?;
                let exception =
                    Backend::observe_i64_acquire(&mut c.internal[CONTROL].mapping, PAGE_BYTES, 256)
                        .map_err(|e| E::Native(format!("{e:?}")))?;
                let done = resources::completion(counters, signal, exception)?;
                // Positive observations after the original deadline still refuse.
                check_deadline(*deadline)?;
                if done {
                    c.completed_write = 1;
                    c.last_observed_read = 1;
                    *completed = true;
                    break;
                }
                std::thread::sleep(Duration::from_micros(50));
            }
        }
        S::ValidateOutput => {
            if !*completed {
                return Err(E::Phase);
            }
            resources::check(base)?;
            let c = &base.cold.resources.get().context;
            Backend::with_bytes(&c.internal[contract::OUTPUT].mapping, 4096, |bytes| {
                contract::check_output(bytes, true)
            })?;
        }
        S::DestroyQueue => {
            if !*completed {
                return Err(E::Phase);
            }
            resources::check(base)?;
            let c = &mut base.cold.resources.get_mut().context;
            let signal = Backend::observe_completion_signal_state_acquire(
                &mut c.internal[SIGNAL].mapping,
                PAGE_BYTES,
                0,
            )
            .map_err(|e| E::Native(format!("{e:?}")))?;
            let counters =
                Backend::observe_aql_counters(&mut c.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(|e| E::Native(format!("{e:?}")))?;
            let exception =
                Backend::observe_i64_acquire(&mut c.internal[CONTROL].mapping, PAGE_BYTES, 256)
                    .map_err(|e| E::Native(format!("{e:?}")))?;
            if !resources::completion(counters, signal, exception)?
                || c.completed_write != 1
                || c.last_observed_read != 1
            {
                return Err(E::Contract("one-stop completed retirement frontier"));
            }
            check_deadline(*deadline)?;
            let expected = KfdIoctlDestroyQueueArgs::new(
                c.queue_id.ok_or(E::Contract("one-stop queue custody"))?,
            );
            let mut observed = expected;
            crate::queue_linux::destroy_queue(c.backend.kfd_fd(), &mut observed)
                .map_err(|e| E::Native(format!("{e:?}")))?;
            if expected != observed {
                return Err(E::Contract("one-stop DESTROY_QUEUE output"));
            }
            c.queue_id = None;
        }
        // These shared operations require actual queue destruction and retained
        // event/metadata custody, not an EmptyQueue owner or empty-queue witness.
        S::DestroyEvent
        | S::WithdrawMetadata
        | S::DisableRuntime
        | S::ClearTrap
        | S::UnmapDoorbell => {
            if !*completed {
                return Err(E::Phase);
            }
            let old = match step {
                S::DestroyEvent => EmptyStep::DestroyEvent,
                S::WithdrawMetadata => EmptyStep::WithdrawMetadata,
                S::DisableRuntime => EmptyStep::DisableRuntime,
                S::ClearTrap => EmptyStep::ClearTrap,
                S::UnmapDoorbell => EmptyStep::UnmapDoorbell,
                _ => return Err(E::Phase),
            };
            super::super::native::execute(
                &mut base.cold,
                base.geometry,
                base.closure,
                &mut base.queue_call,
                &mut base.event_destroyed,
                old,
            )?;
        }
        S::RetireAllocation { ordinal, operation } => {
            super::retirement::retire(base, ordinal, operation)?
        }
        S::Reconcile => super::retirement::reconcile(base, *completed)?,
    }
    current(base)?;
    if deadline.is_some() {
        check_deadline(*deadline)?;
    }
    Ok(())
}
pub(super) struct Publication<'a> {
    pub(super) inner: &'a mut OneStopInner,
}
impl AqlPacketPublicationTargetV1 for Publication<'_> {
    type Error = E;
    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<(), E> {
        if self.inner.cursor.phase() != super::Gfx950DebugOneStopPhaseV1::Ready(S::WriteBody)
            || self.inner.checkpoint.as_ref().map(|s| s.packet_bytes)
                != Some(packet.encode_unpublished_le())
        {
            return Err(E::Contract("same retained packet publication"));
        }
        self.inner.step()
    }
    fn publish_release_header(&mut self, header: u16) -> Result<(), E> {
        if header != AqlDispatchOrderingV1::Independent.header()
            || self.inner.cursor.phase()
                != super::Gfx950DebugOneStopPhaseV1::Ready(S::ReleaseHeader)
        {
            return Err(E::Contract("same retained packet release header"));
        }
        self.inner.step()
    }
}
