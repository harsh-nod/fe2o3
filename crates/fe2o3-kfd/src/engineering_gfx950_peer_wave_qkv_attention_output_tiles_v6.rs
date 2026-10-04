//! Closed paired resident launch over existing roots and private V6 state.

use super::super::wave_qkv_attention_output_tiles_v6 as profile;
use super::wave_qkv_attention_output_tiles_state_v6::Activation;
use super::*;

/// Fourteen rank-local prefix roots in source order, plus a typed V6 state.
/// Roots are input, norm/QKV/head weights, rotary, cache metadata, output weight,
/// normalized, QKV output, query, key/value cache, attention and F32 O partial.
/// Equal-size roles are not authenticated by geometry: the unsafe launch caller
/// still owes the model/catalog and logical-to-physical page joins.
pub struct Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'a> {
    pub kernel: &'a Gfx950EngineeringPeerKernelV1,
    pub object_sha256: [u8; 32],
    pub roots: [Gfx950EngineeringPeerBufferV1; 14],
    pub state: &'a mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    pub timeout_ms: u32,
}

/// Completed queue and acquired terminal observations, in rank order.
/// Allocations and contexts remain resident. This does NOT claim group teardown,
/// numerical equivalence, a GPU execution-time measurement, or reuse authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6 {
    pub final_states: [[u32; 284]; 2],
    /// Existing host intervals include currentness, kernarg/packet publication,
    /// GPU completion and idle checks; exclude allocation and state readback.
    pub dispatch_elapsed_ns: [u64; 2],
}

fn validate_command_identity(
    group: u64,
    world: usize,
    rank: usize,
    command: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>,
    retained: &KernelMetadataV1,
) -> Result<()> {
    if world != 2
        || rank >= 2
        || command.kernel.group != group
        || command.kernel.rank != rank
        || command.state.owner_rank() != rank
        || command.state.buffer.group != group
        || command.state.activation != Activation::Ready
        || command.kernel.metadata != *retained
        || command
            .roots
            .iter()
            .any(|root| root.group != group || root.owner != rank)
    {
        return Err("prefix tiles V6 rank, kernel, root or activation identity".into());
    }
    profile::validate_metadata(retained, command.object_sha256, profile::SYMBOL)
}

fn validate_data_record(
    rank: usize,
    index: usize,
    token: Gfx950EngineeringPeerBufferV1,
    record: &BufferRecord,
) -> Result<()> {
    if index >= 14
        || record.token != token
        || record.kind != BufferKind::PublicVram
        || record.mapping.phase != Phase::PeersMapped
        || token.owner != rank
        || token.bytes != profile::EXTENTS[index] as u64
    {
        return Err("prefix tiles V6 rank-local data root".into());
    }
    Ok(())
}

fn validate_pair_regions(regions: &[[profile::OwnedRegion; 15]; 2]) -> Result<()> {
    for roots in regions {
        profile::validate_regions(roots, &profile::fixups(roots))?;
    }
    for left in &regions[0] {
        for right in &regions[1] {
            if left.buffer == right.buffer
                || (left.base < right.base + right.backing as u64
                    && right.base < left.base + left.backing as u64)
            {
                return Err("prefix tiles V6 cross-rank allocation alias".into());
            }
        }
    }
    Ok(())
}

fn dispatch<'a>(
    command: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'a>,
) -> Gfx950EngineeringPeerDispatchV1<'a> {
    let mut pointers = command
        .roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            root.pointer(
                index as u32 * 8,
                0,
                profile::EXTENTS[index] as u64,
                profile::role_access(index),
            )
        })
        .collect::<Vec<_>>();
    pointers.push(command.state.pointer());
    Gfx950EngineeringPeerDispatchV1 {
        kernel: command.kernel,
        bytes: vec![0; profile::KERNARG_BYTES],
        workgroup: profile::WORKGROUP,
        grid: profile::GRID,
        pointers,
        timeout_ms: command.timeout_ms,
    }
}

trait ResidentBackend {
    fn check(&mut self) -> Result<()>;
    fn validate(
        &mut self,
        rank: usize,
        command: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>,
    ) -> Result<[profile::OwnedRegion; 15]>;
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; 284]>;
    fn submit_and_complete(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>>;
}

fn run_resident(
    backend: &mut impl ResidentBackend,
    commands: [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>; 2],
) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6> {
    commands.iter().try_fold(0_u32, |total, command| {
        total
            .checked_add(command.timeout_ms)
            .filter(|&sum| command.timeout_ms != 0 && sum <= 600_000)
            .ok_or("prefix tiles V6 aggregate timeout outside 1..600000 ms")
    })?;
    backend.check()?;
    let regions = [
        backend.validate(0, &commands[0])?,
        backend.validate(1, &commands[1])?,
    ];
    validate_pair_regions(&regions)?;
    for command in &commands {
        if backend.observe(command.state)? != profile::INITIAL_STATE {
            return Err("prefix tiles V6 state is not freshly initialized/rearmed".into());
        }
    }
    backend.check()?;
    let native = commands.iter().map(dispatch).collect::<Vec<_>>();
    // Consume both activations before any publication. An uncertain partial
    // round leaves both Submitted and the outer Group poisoned, never reusable.
    for command in &commands {
        if command.state.activation != Activation::Ready {
            return Err("prefix tiles V6 repeated activation".into());
        }
    }
    let [left, right] = commands;
    left.state.activation = Activation::Submitted;
    right.state.activation = Activation::Submitted;
    let elapsed: [u64; 2] = backend
        .submit_and_complete(native)?
        .try_into()
        .map_err(|_| "prefix tiles V6 paired completion census")?;
    backend.check()?;
    let final_states = [backend.observe(left.state)?, backend.observe(right.state)?];
    for words in &final_states {
        profile::validate_final_state(*words)?;
    }
    backend.check()?;
    left.state.activation = Activation::Completed;
    right.state.activation = Activation::Completed;
    Ok(Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6 {
        final_states,
        dispatch_elapsed_ns: elapsed,
    })
}

struct NativeResident<'a>(
    &'a mut Gfx950EngineeringPeerGroupV1,
    Option<Vec<Gfx950EngineeringRawTimestampObservationV1>>,
);

impl ResidentBackend for NativeResident<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.0.contexts, self.0.shared_full_currentness)
    }

    fn validate(
        &mut self,
        rank: usize,
        command: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>,
    ) -> Result<[profile::OwnedRegion; 15]> {
        let context = self
            .0
            .contexts
            .get(rank)
            .ok_or("prefix tiles V6 missing rank")?;
        let kernel = context
            .kernels
            .get(&command.kernel.id)
            .ok_or("prefix tiles V6 missing retained kernel")?;
        validate_command_identity(
            self.0.incarnation,
            self.0.contexts.len(),
            rank,
            command,
            &kernel.metadata,
        )?;
        command.state.local_id(self.0)?;
        let tokens: [Gfx950EngineeringPeerBufferV1; 15] = core::array::from_fn(|index| {
            if index < 14 {
                command.roots[index]
            } else {
                command.state.buffer
            }
        });
        let mut regions = [profile::OwnedRegion {
            buffer: 0,
            base: 0,
            requested: 0,
            backing: 0,
        }; 15];
        for (index, token) in tokens.into_iter().enumerate() {
            let record = self.0.validate_token(token)?;
            if index < 14 {
                validate_data_record(rank, index, token, record)?;
            }
            let allocation = context
                .buffers
                .get(&record.local_id)
                .ok_or("prefix tiles V6 missing local root")?;
            regions[index] = profile::OwnedRegion {
                buffer: token.id,
                base: allocation.va,
                requested: allocation.requested,
                backing: allocation.backing,
            };
        }
        profile::validate_regions(&regions, &profile::fixups(&regions))?;
        Ok(regions)
    }

    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; 284]> {
        // SAFETY: run_resident brackets each initial/terminal pair with fresh
        // full group fences, retains this exclusive borrow, and never publishes
        // or marks Completed until the corresponding post-fence succeeds.
        unsafe {
            super::wave_qkv_attention_output_tiles_state_v6::observe_within_resident_fence(
                self.0, state,
            )
        }
    }

    fn submit_and_complete(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>> {
        // SAFETY: the outer unsafe entry owes the exact reviewed V6 image/model
        // contracts. The unchanged publisher retains all allocations and queues.
        if self.1.is_some() {
            // Keep observations private until the unchanged coordinator has
            // checked every terminal word and its final full fence.
            let observations = unsafe {
                self.0
                    .dispatch_round_with_raw_timestamps_unchecked(commands)
            }?;
            let elapsed = observations
                .iter()
                .map(|value| value.host_elapsed_ns())
                .collect();
            self.1 = Some(observations);
            Ok(elapsed)
        } else {
            unsafe { self.0.dispatch_round_unchecked(commands) }
        }
    }
}

fn finish_timestamp_round(
    round: Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6,
    observations: Vec<Gfx950EngineeringRawTimestampObservationV1>,
) -> Result<(
    Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6,
    [Gfx950EngineeringRawTimestampObservationV1; 2],
)> {
    let raw: [Gfx950EngineeringRawTimestampObservationV1; 2] = observations
        .try_into()
        .map_err(|_| "prefix tiles timestamp paired census")?;
    for (rank, value) in raw.iter().enumerate() {
        profile::validate_final_state(round.final_states[rank])?;
        if value.rank() != rank || value.host_elapsed_ns() != round.dispatch_elapsed_ns[rank] {
            return Err("prefix tiles timestamp rank/host interval join".into());
        }
    }
    if raw[0].group_incarnation() != raw[1].group_incarnation()
        || raw[0].unique_id() == raw[1].unique_id()
    {
        return Err("prefix tiles timestamp group/device join".into());
    }
    Ok((round, raw))
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Dispatch one V6 prefix on each of exactly two retained ranks. Both queues
    /// complete and all 284 terminal words validate before returning observations.
    /// # Safety
    /// Inherit the Group's trusted-code/process isolation contract. Independently
    /// bind each reviewed source/HSACO digest to this exact tiled V6 ABI and
    /// arithmetic, including atomic/payload coherence and progress obligations.
    /// Bind every original weight and scratch root to its authenticated model
    /// role; equal extents do not authenticate semantic roles. Retain all model,
    /// KV and catalog lifetimes, validate the causal cache metadata/page mapping,
    /// and prohibit concurrent commands or host access. The fixed polling budget
    /// does not guarantee progress for 64 workgroups. Both ranks must use the
    /// separately reviewed ordinary source and its exact 512-byte LDS protocol.
    /// This is engineering-only; no production or numerical admission is granted.
    /// On any failure the group is poisoned and the disposable process must exit.
    pub unsafe fn dispatch_wave_qkv_attention_output_tiles_round_v6(
        &mut self,
        commands: [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>; 2],
    ) -> Result<Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6> {
        self.require_active()?;
        let result = run_resident(&mut NativeResident(self, None), commands);
        self.finish(result)
    }

    /// Same typed paired V6 launch with raw completion-signal ticks. All existing
    /// state/ABI/alias checks are retained. Only fresh timestamp-enabled groups
    /// accept this entry; it never toggles a queue or falls back to host timing.
    /// Observations do not claim close, calibration or cross-device overlap.
    /// # Safety
    /// All obligations of `dispatch_wave_qkv_attention_output_tiles_round_v6`
    /// apply unchanged, together with `open_raw_timestamps_unchecked`. Retain
    /// observations private until successful group close; any failure is terminal.
    pub unsafe fn dispatch_wave_qkv_attention_output_tiles_round_with_raw_timestamps_unchecked_v1(
        &mut self,
        commands: [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>; 2],
    ) -> Result<(
        Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6,
        [Gfx950EngineeringRawTimestampObservationV1; 2],
    )> {
        self.require_active()?;
        let result = (|| {
            let mut backend = NativeResident(self, Some(Vec::new()));
            let round = run_resident(&mut backend, commands)?;
            let raw = backend
                .1
                .take()
                .ok_or("missing prefix tiles timestamp capture")?;
            finish_timestamp_round(round, raw)
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_qkv_attention_output_tiles_v6_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_qkv_attention_output_tiles_timestamp_tests.rs"]
mod timestamp_tests;
