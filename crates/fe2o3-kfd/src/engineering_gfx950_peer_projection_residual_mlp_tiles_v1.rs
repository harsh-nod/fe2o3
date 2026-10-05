//! Opt-in TP2 queue-ordered projection residual -> typed MLP. Not policy-neutral.

use super::super::{ordered_batch as ordered, wave_mlp_tiles_v2 as profile};
use super::wave_mlp_tiles_state_v2::Activation;
use super::*;
use fe2o3_aql::AqlPreparedKernelDispatchV1;

const PROJECTION_SYMBOL: &str = "ferric_qwen3_tp_peer_tp2_projection_residual_bf16_v1";

/// Both commands name the same ordered owner-0/owner-1 completed F32 partials.
/// The caller must supply distinct Down outputs; the old O/Down alias is unsafe
/// when the two ranks independently advance from their residual into MLP.
pub struct Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1<'a> {
    pub projection_kernel: &'a Gfx950EngineeringPeerKernelV1,
    pub projection_object_sha256: [u8; 32],
    pub partials: [Gfx950EngineeringPeerBufferV1; 2],
    pub residual_input: Gfx950EngineeringPeerBufferV1,
    pub mlp: Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerProjectionResidualMlpTilesRoundV1 {
    pub final_states: [[u32; 548]; 2],
    /// Inclusive host interval: preflight, fences, staging, both publications,
    /// polling, terminal readback and final fence. Not GPU or per-kernel time.
    pub segment_host_ns: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RankIdentity {
    rank: usize,
    unique_id: u64,
    gpu_id: u32,
    queue_id: Option<u32>,
    epoch: u64,
    storage: ordered::PairStorageIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScratchPhase {
    Idle { retired_frontiers: [u64; 2] },
    Busy,
    Poisoned,
}

/// Fixed metadata only. Context owns the two existing 1 MiB arenas and signals.
pub(super) struct Scratch {
    incarnation: u64,
    ranks: [RankIdentity; 2],
    phase: ScratchPhase,
}

impl Scratch {
    fn require_idle(
        &self,
        incarnation: u64,
        ranks: [RankIdentity; 2],
        writes: [u64; 2],
        completed: [u64; 2],
    ) -> Result<()> {
        let ScratchPhase::Idle { retired_frontiers } = self.phase else {
            return Err("projection MLP scratch is not healthy Idle".into());
        };
        if incarnation != self.incarnation
            || ranks != self.ranks
            || writes != completed
            || (0..2).any(|rank| writes[rank] < retired_frontiers[rank])
        {
            return Err("projection MLP scratch identity or retirement drift".into());
        }
        Ok(())
    }

    fn retire(&mut self, writes: [u64; 2]) -> Result<()> {
        if self.phase != ScratchPhase::Busy {
            return Err("projection MLP retirement without Busy ownership".into());
        }
        self.phase = ScratchPhase::Idle {
            retired_frontiers: writes,
        };
        Ok(())
    }
}

fn timeout(left: u32, right: u32) -> Result<u32> {
    if left != right || !(1..=10_000).contains(&left) {
        return Err("projection MLP requires one common 1..10000 ms deadline".into());
    }
    Ok(left)
}

fn deadline_check(now: Instant, deadline: Instant) -> Result<()> {
    if now >= deadline {
        return Err("projection MLP aggregate deadline exceeded".into());
    }
    Ok(())
}

fn validate_projection_metadata(m: &KernelMetadataV1, sha: [u8; 32]) -> Result<()> {
    if sha == [0; 32]
        || m.object_sha256 != sha
        || m.symbol != PROJECTION_SYMBOL
        || m.kernarg_bytes != 424
        || m.kernarg_alignment != 8
        || m.wavefront_size != 64
        || m.private_segment_bytes != 0
        || m.group_segment_bytes != 0
        || m.implicit_argument_offset != Some(168)
        || m.implicit_argument_bytes != 256
        || m.explicit_arguments.len() != 22
    {
        return Err("projection MLP residual physical ABI/resources".into());
    }
    for (index, argument) in m.explicit_arguments.iter().enumerate() {
        let pointer = index < 20 && index % 2 == 0;
        let offset = if index < 20 {
            index as u32 * 8
        } else {
            160 + (index as u32 - 20) * 4
        };
        let width = if index < 20 { 8 } else { 4 };
        let alignment = if index < 16 { 4 } else { 2 };
        let access = if index == 18 {
            BufferAccessV1::Write
        } else {
            BufferAccessV1::Read
        };
        if argument.offset != offset
            || argument.bytes != width
            || argument.global_buffer != pointer
            || argument
                .pointee_alignment
                .is_some_and(|v| !pointer || v != alignment)
            || argument.access.is_some_and(|v| !pointer || v != access)
        {
            return Err("projection MLP residual pointer/scalar role".into());
        }
    }
    Ok(())
}

fn projection_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 424];
    for slot in [0usize, 1, 8, 9] {
        bytes[slot * 16 + 8..slot * 16 + 16].copy_from_slice(&4096u64.to_le_bytes());
    }
    bytes[160..164].copy_from_slice(&1u32.to_le_bytes());
    bytes[164..168].copy_from_slice(&2u32.to_le_bytes());
    bytes
}

fn projection_pointers(
    command: &Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1<'_>,
) -> [Gfx950EngineeringPeerPointerV1; 10] {
    core::array::from_fn(|slot| match slot {
        0..=7 => command.partials[if slot < 2 { slot } else { 0 }].pointer(
            slot as u32 * 16,
            0,
            if slot < 2 { 16_384 } else { 0 },
            BufferAccessV1::Read,
        ),
        8 => command
            .residual_input
            .pointer(128, 0, 8192, BufferAccessV1::Read),
        _ => command.mlp.roots[0].pointer(144, 0, 8192, BufferAccessV1::Write),
    })
}

fn overlap(left: profile::OwnedRegion, right: profile::OwnedRegion) -> bool {
    left.buffer == right.buffer
        || (left.base < right.base + right.backing as u64
            && right.base < left.base + left.backing as u64)
}

fn validate_cross_stage(
    mlp: &[[profile::OwnedRegion; 11]; 2],
    partials: &[profile::OwnedRegion; 2],
    inputs: &[profile::OwnedRegion; 2],
) -> Result<()> {
    wave_mlp_tiles_v2::validate_pair_regions(mlp)?;
    if overlap(partials[0], partials[1]) {
        return Err("projection MLP partial allocations alias".into());
    }
    // Both ranks may read either partial. Neither MLP may overwrite it until
    // the peer residual has completed; local WaitForPrior cannot prove that.
    // The only intended cross-stage writer/read bridge is output -> roots[0].
    for read in partials.iter().chain(inputs) {
        for rank_roots in mlp {
            for (index, root) in rank_roots.iter().enumerate() {
                if (index == 0 || profile::role_access(index) != BufferAccessV1::Read)
                    && overlap(*read, *root)
                {
                    return Err("projection MLP unexpected cross-stage writer alias".into());
                }
            }
        }
    }
    Ok(())
}

trait SegmentBackend {
    fn now(&mut self) -> Instant;
    fn preflight(&mut self) -> Result<()>;
    fn consume(&mut self) -> Result<()>;
    fn stage(&mut self) -> Result<()>;
    fn publication_fence(&mut self) -> Result<()>;
    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()>;
    fn poll(&mut self, rank: usize) -> Result<bool>;
    fn validate_signals(&mut self, rank: usize) -> Result<()>;
    fn retire_queues(&mut self) -> Result<()>;
    fn terminal(&mut self) -> Result<[[u32; 548]; 2]>;
    fn complete(&mut self) -> Result<()>;
    fn pause(&mut self);
    fn poison(&mut self);
}

fn coordinate(
    backend: &mut impl SegmentBackend,
    timeout_ms: u32,
) -> Result<Gfx950EngineeringPeerProjectionResidualMlpTilesRoundV1> {
    let started = backend.now();
    let result = (|| {
        timeout(timeout_ms, timeout_ms)?;
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(timeout_ms)))
            .ok_or("projection MLP deadline overflow")?;
        backend.preflight()?;
        deadline_check(backend.now(), deadline)?;
        backend.consume()?;
        deadline_check(backend.now(), deadline)?;
        backend.stage()?;
        deadline_check(backend.now(), deadline)?;
        for rank in 0..2 {
            backend.publication_fence()?;
            deadline_check(backend.now(), deadline)?;
            backend.publish(rank, deadline)?;
        }
        // Both final doorbells precede any completion poll. No serial rank wait.
        let mut done = [false; 2];
        while done != [true; 2] {
            deadline_check(backend.now(), deadline)?;
            for (rank, complete) in done.iter_mut().enumerate() {
                if !*complete {
                    *complete = backend.poll(rank)?;
                }
            }
            if done != [true; 2] {
                backend.pause();
            }
        }
        for rank in 0..2 {
            backend.validate_signals(rank)?;
        }
        deadline_check(backend.now(), deadline)?;
        backend.retire_queues()?;
        let final_states = backend.terminal()?;
        deadline_check(backend.now(), deadline)?;
        backend.complete()?;
        let finished = backend.now();
        deadline_check(finished, deadline)?;
        let elapsed = finished.saturating_duration_since(started).as_nanos();
        Ok(Gfx950EngineeringPeerProjectionResidualMlpTilesRoundV1 {
            final_states,
            segment_host_ns: u64::try_from(elapsed)
                .map_err(|_| "projection MLP host timer overflow")?,
        })
    })();
    if result.is_err() {
        backend.poison();
    }
    result
}

struct Native<'group, 'command> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    commands: [Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1<'command>; 2],
    prepared: [Option<[PreparedDispatch; 2]>; 2],
    packets: [Option<[AqlPreparedKernelDispatchV1; 2]>; 2],
    pending: [Option<ordered::OrderedPending>; 2],
}

impl Native<'_, '_> {
    fn full_fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn identities(&self) -> Result<[RankIdentity; 2]> {
        let identity = |rank: usize| -> Result<RankIdentity> {
            let context = &self.group.contexts[rank];
            Ok(RankIdentity {
                rank,
                unique_id: context.unique_id,
                gpu_id: context.backend.gpu_id(),
                queue_id: context.queue_id,
                epoch: context.queue_epoch,
                storage: ordered::pair_storage_identity(context)?,
            })
        };
        Ok([identity(0)?, identity(1)?])
    }

    fn frontiers(&self) -> ([u64; 2], [u64; 2]) {
        (
            core::array::from_fn(|rank| self.group.contexts[rank].ring.write()),
            core::array::from_fn(|rank| self.group.contexts[rank].completed_write),
        )
    }

    fn region(
        &self,
        token: Gfx950EngineeringPeerBufferV1,
        owner: usize,
        bytes: usize,
    ) -> Result<profile::OwnedRegion> {
        let record = self.group.validate_token(token)?;
        if token.owner != owner
            || token.bytes != bytes as u64
            || record.kind != BufferKind::PublicVram
            || record.mapping.phase != Phase::PeersMapped
        {
            return Err("projection MLP public input owner/extent/phase".into());
        }
        let allocation = self.group.contexts[owner]
            .buffers
            .get(&record.local_id)
            .ok_or("projection MLP input allocation missing")?;
        if allocation.requested != bytes
            || allocation.backing < bytes
            || !allocation.va.is_multiple_of(PAGE_BYTES as u64)
            || !allocation.backing.is_multiple_of(PAGE_BYTES)
            || allocation
                .va
                .checked_add(allocation.backing as u64)
                .is_none()
        {
            return Err("projection MLP input allocation range".into());
        }
        Ok(profile::OwnedRegion {
            buffer: token.id,
            base: allocation.va,
            requested: allocation.requested,
            backing: allocation.backing,
        })
    }

    fn observe(&mut self, rank: usize) -> Result<[u32; 548]> {
        // SAFETY: exclusive Group borrow; coordinator brackets paired initial
        // and terminal reads with fresh full group fences, with no publication.
        unsafe {
            wave_mlp_tiles_state_v2::observe_within_resident_fence(
                self.group,
                self.commands[rank].mlp.state,
            )
        }
    }
}

impl SegmentBackend for Native<'_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn preflight(&mut self) -> Result<()> {
        self.group.require_active()?;
        if self.group.contexts.len() != 2
            || self.group.contexts.iter().any(|context| {
                context.raw_timestamps_enabled
                    || context.performance.is_some_and(|p| {
                        p.cache_kernel_admission || p.operational_currentness || p.profile
                    })
            })
        {
            return Err("projection MLP requires TP2 fresh full-currentness policy".into());
        }
        timeout(
            self.commands[0].mlp.timeout_ms,
            self.commands[1].mlp.timeout_ms,
        )?;
        if self.commands[0].partials != self.commands[1].partials
            || self.commands[0].projection_object_sha256
                != self.commands[1].projection_object_sha256
            || self.commands[0].mlp.object_sha256 != self.commands[1].mlp.object_sha256
        {
            return Err("projection MLP ordered partial/image pair mismatch".into());
        }
        self.full_fence()?;
        let mlp = [
            wave_mlp_tiles_v2::validate_retained(self.group, 0, &self.commands[0].mlp)?,
            wave_mlp_tiles_v2::validate_retained(self.group, 1, &self.commands[1].mlp)?,
        ];
        let partials = [
            self.region(self.commands[0].partials[0], 0, 16_384)?,
            self.region(self.commands[0].partials[1], 1, 16_384)?,
        ];
        let inputs = [
            self.region(self.commands[0].residual_input, 0, 8192)?,
            self.region(self.commands[1].residual_input, 1, 8192)?,
        ];
        validate_cross_stage(&mlp, &partials, &inputs)?;
        for rank in 0..2 {
            let command = &self.commands[rank];
            let retained = self.group.contexts[rank]
                .kernels
                .get(&command.projection_kernel.id)
                .ok_or("projection MLP residual kernel missing")?;
            if command.projection_kernel.group != self.group.incarnation
                || command.projection_kernel.rank != rank
                || command.projection_kernel.metadata != retained.metadata
            {
                return Err("projection MLP residual retained identity".into());
            }
            validate_projection_metadata(&retained.metadata, command.projection_object_sha256)?;
        }
        for rank in 0..2 {
            if self.observe(rank)? != profile::INITIAL_STATE {
                return Err("projection MLP state is not freshly Ready".into());
            }
        }
        self.full_fence()?;
        // Prepare every image and pointer fixup before reserving/publishing either queue.
        for rank in 0..2 {
            let command = &self.commands[rank];
            let residual = self.group.prepare_peer_dispatch(
                command.projection_kernel,
                projection_bytes(),
                [64, 1, 1],
                [4096, 1, 1],
                &projection_pointers(command),
                command.mlp.timeout_ms,
            )?;
            let mlp = wave_mlp_tiles_v2::dispatch(&command.mlp);
            let mlp = self.group.prepare_peer_dispatch(
                mlp.kernel,
                mlp.bytes,
                mlp.workgroup,
                mlp.grid,
                &mlp.pointers,
                mlp.timeout_ms,
            )?;
            let context = &self.group.contexts[rank];
            require_sequence_capacity(context.ring.write(), context.last_observed_read, 2)?;
            self.prepared[rank] = Some([residual, mlp]);
        }
        if self.group.projection_mlp_scratch.is_none() {
            for context in &mut self.group.contexts {
                ordered::retain_pair_storage(context)?;
            }
            let (writes, completed) = self.frontiers();
            if writes != completed {
                return Err("projection MLP initial scratch busy".into());
            }
            self.group.projection_mlp_scratch = Some(Scratch {
                incarnation: self.group.incarnation,
                ranks: self.identities()?,
                phase: ScratchPhase::Idle {
                    retired_frontiers: writes,
                },
            });
        }
        self.full_fence()?;
        let (writes, completed) = self.frontiers();
        self.group
            .projection_mlp_scratch
            .as_ref()
            .ok_or("projection MLP scratch missing")?
            .require_idle(
                self.group.incarnation,
                self.identities()?,
                writes,
                completed,
            )
    }

    fn consume(&mut self) -> Result<()> {
        if self
            .commands
            .iter()
            .any(|c| c.mlp.state.activation != Activation::Ready)
        {
            return Err("projection MLP repeated activation".into());
        }
        self.group
            .projection_mlp_scratch
            .as_mut()
            .ok_or("projection MLP scratch missing")?
            .phase = ScratchPhase::Busy;
        for command in &mut self.commands {
            command.mlp.state.activation = Activation::Submitted;
        }
        Ok(())
    }

    fn stage(&mut self) -> Result<()> {
        for rank in 0..2 {
            self.packets[rank] = Some(ordered::stage_pair(
                &mut self.group.contexts[rank],
                self.prepared[rank]
                    .take()
                    .ok_or("projection MLP prepared pair missing")?,
            )?);
        }
        Ok(())
    }

    fn publication_fence(&mut self) -> Result<()> {
        round::fresh_publication_fence(self.group)
    }

    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.pending[rank] = Some(ordered::publish_pair(
            &mut self.group.contexts[rank],
            self.packets[rank]
                .take()
                .ok_or("projection MLP staged pair missing")?,
            deadline,
        )?);
        Ok(())
    }

    fn poll(&mut self, rank: usize) -> Result<bool> {
        ordered::poll_pair(
            &mut self.group.contexts[rank],
            self.pending[rank]
                .as_mut()
                .ok_or("projection MLP pending pair missing")?,
        )
    }

    fn validate_signals(&mut self, rank: usize) -> Result<()> {
        ordered::validate_pair_signals(
            &mut self.group.contexts[rank],
            self.pending[rank]
                .as_ref()
                .ok_or("projection MLP pending pair missing")?,
        )
    }

    fn retire_queues(&mut self) -> Result<()> {
        for rank in 0..2 {
            ordered::retire_pair(
                &mut self.group.contexts[rank],
                self.pending[rank]
                    .take()
                    .ok_or("projection MLP pending pair missing")?,
            )?;
        }
        Ok(())
    }

    fn terminal(&mut self) -> Result<[[u32; 548]; 2]> {
        self.full_fence()?;
        let states = [self.observe(0)?, self.observe(1)?];
        for state in states {
            profile::validate_final_state(state)?;
        }
        self.full_fence()?;
        Ok(states)
    }

    fn complete(&mut self) -> Result<()> {
        let identities = self.identities()?;
        let (writes, completed) = self.frontiers();
        let scratch = self
            .group
            .projection_mlp_scratch
            .as_mut()
            .ok_or("projection MLP scratch missing")?;
        if identities != scratch.ranks || writes != completed {
            return Err("projection MLP completion identity drift".into());
        }
        scratch.retire(writes)?;
        for command in &mut self.commands {
            command.mlp.state.activation = Activation::Completed;
        }
        Ok(())
    }

    fn pause(&mut self) {
        std::thread::sleep(Duration::from_micros(50));
    }

    fn poison(&mut self) {
        self.group.poisoned = true;
        if let Some(scratch) = &mut self.group.projection_mlp_scratch {
            scratch.phase = ScratchPhase::Poisoned;
        }
        for context in &mut self.group.contexts {
            context.ordered_batch_poisoned = true;
        }
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Publish residual then MLP on each of two rank-owned queues, then wait for
    /// both. Removes the intermediate host fence; WaitForPrior replaces it only
    /// within each rank. Cross-rank read/write overlap is rejected, not ordered.
    /// Both embedded timeouts must equal one aggregate 1..10000 ms budget.
    ///
    /// # Safety
    /// Dedicated single-threaded disposable process and exact reviewed image,
    /// model, source/ISA, System memory-order and lifetime contracts are required.
    /// Both Prefix284 producers must have completed and passed their genuine
    /// state checks before entry. Distinct Down partials must remain retained
    /// through both subsequent final residuals. This method proves no arithmetic,
    /// scheduling fairness, end-to-end model correctness or throughput claim.
    /// Every error is terminal: process teardown, no retry, rollback or rearm.
    pub unsafe fn dispatch_projection_residual_mlp_tiles_round_unchecked_v1(
        &mut self,
        commands: [Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1<'_>; 2],
    ) -> Result<Gfx950EngineeringPeerProjectionResidualMlpTilesRoundV1> {
        let timeout_ms = commands[0].mlp.timeout_ms;
        coordinate(
            &mut Native {
                group: self,
                commands,
                prepared: [None, None],
                packets: [None, None],
                pending: [None, None],
            },
            timeout_ms,
        )
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_projection_residual_mlp_tiles_v1_tests.rs"]
mod tests;
