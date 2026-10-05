//! Closed paired resident launch over existing roots and private V2 state.

use super::super::wave_mlp_tiles_v2 as profile;
use super::wave_mlp_tiles_state_v2::Activation;
use super::*;

/// The ten existing rank-local MLP roots in source order, plus a typed V2 state.
/// Roots are input, norm weight, gate/up/down NxK weights, normalized, gate,
/// up, activation and F32 down partial. Equal-size roles are not authenticated
/// by geometry: the unsafe launch caller still owes the model/catalog joins.
pub struct Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'a> {
    pub kernel: &'a Gfx950EngineeringPeerKernelV1,
    pub object_sha256: [u8; 32],
    pub roots: [Gfx950EngineeringPeerBufferV1; 10],
    pub state: &'a mut Gfx950EngineeringPeerWaveMlpTilesStateV2,
    pub timeout_ms: u32,
}

/// Completed queue and acquired terminal observations, in rank order.
/// Allocations and contexts remain resident. This does NOT claim group teardown,
/// numerical equivalence, a GPU execution-time measurement, or reuse authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerWaveMlpTilesRoundV2 {
    pub final_states: [[u32; 548]; 2],
    /// Existing paired publisher host timers, excluding allocation and readback.
    pub dispatch_elapsed_ns: [u64; 2],
}

fn validate_command_identity(
    group: u64,
    world: usize,
    rank: usize,
    command: &Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>,
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
        return Err("MLP tiles V2 rank, kernel, root or activation identity".into());
    }
    profile::validate_metadata(retained, command.object_sha256, profile::SYMBOL)
}

fn validate_data_record(
    rank: usize,
    index: usize,
    token: Gfx950EngineeringPeerBufferV1,
    record: &BufferRecord,
) -> Result<()> {
    if index >= 10
        || record.token != token
        || record.kind != BufferKind::PublicVram
        || record.mapping.phase != Phase::PeersMapped
        || token.owner != rank
        || token.bytes != profile::EXTENTS[index] as u64
    {
        return Err("MLP tiles V2 rank-local data root".into());
    }
    Ok(())
}

pub(super) fn validate_pair_regions(regions: &[[profile::OwnedRegion; 11]; 2]) -> Result<()> {
    for roots in regions {
        profile::validate_regions(roots, &profile::fixups(roots))?;
    }
    for left in &regions[0] {
        for right in &regions[1] {
            if left.buffer == right.buffer
                || (left.base < right.base + right.backing as u64
                    && right.base < left.base + left.backing as u64)
            {
                return Err("MLP tiles V2 cross-rank allocation alias".into());
            }
        }
    }
    Ok(())
}

pub(super) fn dispatch<'a>(
    command: &Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'a>,
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
        command: &Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>,
    ) -> Result<[profile::OwnedRegion; 11]>;
    fn observe(&mut self, state: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<[u32; 548]>;
    fn submit_and_complete(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>>;
}

fn run_resident(
    backend: &mut impl ResidentBackend,
    commands: [Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>; 2],
) -> Result<Gfx950EngineeringPeerWaveMlpTilesRoundV2> {
    commands.iter().try_fold(0_u32, |total, command| {
        total
            .checked_add(command.timeout_ms)
            .filter(|&sum| command.timeout_ms != 0 && sum <= 600_000)
            .ok_or("MLP tiles V2 aggregate timeout outside 1..600000 ms")
    })?;
    backend.check()?;
    let regions = [
        backend.validate(0, &commands[0])?,
        backend.validate(1, &commands[1])?,
    ];
    validate_pair_regions(&regions)?;
    for command in &commands {
        if backend.observe(command.state)? != profile::INITIAL_STATE {
            return Err("MLP tiles V2 state is not freshly initialized/rearmed".into());
        }
    }
    backend.check()?;
    let native = commands.iter().map(dispatch).collect::<Vec<_>>();
    // Consume both activations before any publication. An uncertain partial
    // round leaves both Submitted and the outer Group poisoned, never reusable.
    for command in &commands {
        if command.state.activation != Activation::Ready {
            return Err("MLP tiles V2 repeated activation".into());
        }
    }
    let [left, right] = commands;
    left.state.activation = Activation::Submitted;
    right.state.activation = Activation::Submitted;
    let elapsed: [u64; 2] = backend
        .submit_and_complete(native)?
        .try_into()
        .map_err(|_| "MLP tiles V2 paired completion census")?;
    backend.check()?;
    let final_states = [backend.observe(left.state)?, backend.observe(right.state)?];
    for words in &final_states {
        profile::validate_final_state(*words)?;
    }
    backend.check()?;
    left.state.activation = Activation::Completed;
    right.state.activation = Activation::Completed;
    Ok(Gfx950EngineeringPeerWaveMlpTilesRoundV2 {
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
        command: &Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>,
    ) -> Result<[profile::OwnedRegion; 11]> {
        let context = self
            .0
            .contexts
            .get(rank)
            .ok_or("MLP tiles V2 missing rank")?;
        let kernel = context
            .kernels
            .get(&command.kernel.id)
            .ok_or("MLP tiles V2 missing retained kernel")?;
        validate_command_identity(
            self.0.incarnation,
            self.0.contexts.len(),
            rank,
            command,
            &kernel.metadata,
        )?;
        command.state.local_id(self.0)?;
        let tokens: [Gfx950EngineeringPeerBufferV1; 11] = core::array::from_fn(|index| {
            if index < 10 {
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
        }; 11];
        for (index, token) in tokens.into_iter().enumerate() {
            let record = self.0.validate_token(token)?;
            if index < 10 {
                validate_data_record(rank, index, token, record)?;
            }
            let allocation = context
                .buffers
                .get(&record.local_id)
                .ok_or("MLP tiles V2 missing local root")?;
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

    fn observe(&mut self, state: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<[u32; 548]> {
        // SAFETY: run_resident brackets each initial/terminal pair with fresh
        // full group fences, retains this exclusive borrow, and never publishes
        // or marks Completed until the corresponding post-fence succeeds.
        unsafe { super::wave_mlp_tiles_state_v2::observe_within_resident_fence(self.0, state) }
    }

    fn submit_and_complete(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>> {
        // SAFETY: the outer unsafe entry owes the exact reviewed V2 image/model
        // contracts. The unchanged publisher retains all allocations and queues.
        if self.1.is_some() {
            // The same coordinator still validates every terminal state before
            // these private observations can escape through the new entry.
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
    round: Gfx950EngineeringPeerWaveMlpTilesRoundV2,
    observations: Vec<Gfx950EngineeringRawTimestampObservationV1>,
) -> Result<(
    Gfx950EngineeringPeerWaveMlpTilesRoundV2,
    [Gfx950EngineeringRawTimestampObservationV1; 2],
)> {
    let raw: [Gfx950EngineeringRawTimestampObservationV1; 2] = observations
        .try_into()
        .map_err(|_| "MLP tiles timestamp paired census")?;
    for (rank, value) in raw.iter().enumerate() {
        profile::validate_final_state(round.final_states[rank])?;
        if value.rank() != rank || value.host_elapsed_ns() != round.dispatch_elapsed_ns[rank] {
            return Err("MLP tiles timestamp rank/host interval join".into());
        }
    }
    if raw[0].group_incarnation() != raw[1].group_incarnation()
        || raw[0].unique_id() == raw[1].unique_id()
    {
        return Err("MLP tiles timestamp group/device join".into());
    }
    Ok((round, raw))
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Dispatch one V2 MLP on each of exactly two retained ranks. Both queues
    /// complete and all 548 terminal words validate before returning observations.
    /// # Safety
    /// Inherit the Group's trusted-code/process isolation contract. Independently
    /// bind each reviewed source/HSACO digest to this exact tiled V2 ABI and
    /// arithmetic, including atomic/payload coherence and progress obligations.
    /// Bind every original weight and scratch root to its authenticated model
    /// role; equal extents do not distinguish gate/up/down weights. Retain all
    /// model/KV/catalog lifetimes and prohibit concurrent commands or host access.
    /// This is engineering-only; no production or numerical admission is granted.
    /// On any failure the group is poisoned and the disposable process must exit.
    pub unsafe fn dispatch_wave_mlp_tiles_round_v2(
        &mut self,
        commands: [Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>; 2],
    ) -> Result<Gfx950EngineeringPeerWaveMlpTilesRoundV2> {
        self.require_active()?;
        let result = run_resident(&mut NativeResident(self, None), commands);
        self.finish(result)
    }

    /// Same typed paired V2 launch with raw completion-signal ticks. All existing
    /// state/ABI/alias checks are retained. Only fresh timestamp-enabled groups
    /// accept this entry; it never toggles a queue or falls back to host timing.
    /// Observations do not claim close, calibration or cross-device overlap.
    /// # Safety
    /// All obligations of `dispatch_wave_mlp_tiles_round_v2` apply unchanged,
    /// together with `open_raw_timestamps_unchecked`. Retain observations private
    /// until successful group close; any failure is terminal for this process.
    pub unsafe fn dispatch_wave_mlp_tiles_round_with_raw_timestamps_unchecked_v1(
        &mut self,
        commands: [Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>; 2],
    ) -> Result<(
        Gfx950EngineeringPeerWaveMlpTilesRoundV2,
        [Gfx950EngineeringRawTimestampObservationV1; 2],
    )> {
        self.require_active()?;
        let result = (|| {
            let mut backend = NativeResident(self, Some(Vec::new()));
            let round = run_resident(&mut backend, commands)?;
            let raw = backend
                .1
                .take()
                .ok_or("missing MLP tiles timestamp capture")?;
            finish_timestamp_round(round, raw)
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_mlp_tiles_v2_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engineering_gfx950_peer_wave_mlp_tiles_timestamp_tests.rs"]
mod timestamp_tests;

// Reuse typed metadata, allocation and state checks without standalone publication.
pub(super) fn validate_retained(
    group: &mut Gfx950EngineeringPeerGroupV1,
    rank: usize,
    command: &Gfx950EngineeringPeerWaveMlpTilesDispatchV2<'_>,
) -> Result<[profile::OwnedRegion; 11]> {
    NativeResident(group, None).validate(rank, command)
}
