//! One closed warm Prefix/guarded-MLP/readback transaction. No public scope.
use super::*;

#[path = "engineering_gfx950_peer_scoped_census_layer_v1.rs"]
mod census;
pub use census::{
    Gfx950EngineeringPeerScopedCapacityCensusObservationV1,
    Gfx950EngineeringPeerScopedCensusWarmLayerObservationV1,
};

pub struct Gfx950EngineeringPeerScopedPrefixInputsV1<'a> {
    pub kernel: &'a Gfx950EngineeringPeerKernelV1,
    pub object_sha256: [u8; 32],
    pub roots: [Gfx950EngineeringPeerBufferV1; 14],
    pub timeout_ms: u32,
}

/// Observed work counts only; never currentness, retirement or rearm authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerScopedCurrentnessCountsV1 {
    pub full_discoveries: u64,
    pub local_checkpoints: u64,
    pub before_calls: u64,
    pub after_calls: u64,
    pub generation_probes: u64,
}
impl From<crate::device::ScopedCountsV1> for Gfx950EngineeringPeerScopedCurrentnessCountsV1 {
    fn from(v: crate::device::ScopedCountsV1) -> Self {
        Self {
            full_discoveries: v.full_discoveries,
            local_checkpoints: v.local_checkpoints,
            before_calls: v.before_calls,
            after_calls: v.after_calls,
            generation_probes: v.generation_probes,
        }
    }
}

/// Data only, exposed after hidden validation and the mandatory full exit.
#[derive(Debug)]
pub struct Gfx950EngineeringPeerScopedWarmLayerObservationV1 {
    pub prefix: Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6,
    pub mlp: facade::Gfx950EngineeringPeerGuardedMlpObservationV1,
    pub hidden: [Vec<u8>; 2],
    pub currentness: Gfx950EngineeringPeerScopedCurrentnessCountsV1,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    pub currentness_durations: crate::Gfx950EngineeringCurrentnessDurationsV1,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    pub layer_durations: Gfx950EngineeringPeerScopedLayerDurationsV1,
}

struct LayerOperation<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    prefixes: [&'a mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2],
    pair: &'a mut RetainedPair,
    committed: bool,
}
impl LayerOperation<'_> {
    fn quarantine(&mut self) {
        // Same terminal states as mixed-bank custody failure. No cleanup,
        // reset, fallible observation, retry or fabricated retirement.
        for prefix in &mut self.prefixes {
            prefix.activation = wave_qkv_attention_output_tiles_state_v6::Activation::Submitted;
        }
        self.pair.phase = Phase::Poisoned;
        for owner in &mut self.pair.owners {
            owner.poison(self.group);
        }
        poison_group(self.group);
        for context in &mut self.group.contexts {
            crate::device::ScopedCurrentnessV1::poison_devices(&mut [context
                .backend
                .engineering_peer_device()]);
        }
    }
}
impl Drop for LayerOperation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.quarantine();
        }
    }
}

fn warm_admission(
    phase: Phase,
    policy: ArenaPolicy,
    generation: u64,
    has_reuse: bool,
    has_completed: bool,
) -> Result<()> {
    if phase != Phase::Ready
        || policy != ArenaPolicy::ReuseRetired
        || generation <= 1
        || has_completed
    {
        return Err("scoped layer requires a ready genuinely rearmed warm pair".into());
    }
    policy.require_run(generation, has_reuse)
}
fn hidden_pair(hidden: &[Vec<u8>; 2]) -> Result<()> {
    if hidden[0].len() != 8192 || hidden[1].len() != 8192 || hidden[0] != hidden[1] {
        return Err("scoped hidden pair extent or rank parity".into());
    }
    if hidden[0]
        .chunks_exact(2)
        .any(|p| u16::from_le_bytes([p[0], p[1]]) & 0x7f80 == 0x7f80)
    {
        return Err("scoped hidden pair contains nonfinite BF16".into());
    }
    Ok(())
}

trait ClosedBackend {
    type Prefix;
    type Pending;
    type Hidden;
    type Counts;
    type Output;
    fn enter(&mut self) -> Result<()>;
    fn prefix(&mut self) -> Result<Self::Prefix>;
    fn mlp(&mut self) -> Result<Self::Pending>;
    fn hidden(&mut self) -> Result<Self::Hidden>;
    fn exit(&mut self) -> Result<Self::Counts>;
    fn commit(
        &mut self,
        prefix: Self::Prefix,
        pending: Self::Pending,
        hidden: Self::Hidden,
        counts: Self::Counts,
    ) -> Result<Self::Output>;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    fn accept_layer_durations(
        &mut self,
        _output: &mut Self::Output,
        _phase_ns: [u64; 6],
        _body_ns: u64,
    ) -> Result<()> {
        Ok(())
    }
    fn quarantine(&mut self);
}
struct ClosedAttempt<'a, B: ClosedBackend> {
    backend: &'a mut B,
    committed: bool,
}
impl<B: ClosedBackend> Drop for ClosedAttempt<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}
fn closed_layer<B: ClosedBackend>(backend: &mut B) -> Result<B::Output> {
    closed_layer_recorded(
        backend,
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        &mut layer_duration::Monotonic,
    )
}

fn closed_layer_recorded<B: ClosedBackend>(
    backend: &mut B,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    clock: &mut dyn layer_duration::Clock,
) -> Result<B::Output> {
    let mut guard = ClosedAttempt {
        backend,
        committed: false,
    };
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    let mut recording = layer_duration::Recorder::<6>::new(clock);
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    guard.backend.enter()?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    let prefix = guard.backend.prefix()?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    let pending = guard.backend.mlp()?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    let hidden = guard.backend.hidden()?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    let counts = guard.backend.exit()?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    {
        recording.mark()?;
    }
    let result = guard.backend.commit(prefix, pending, hidden, counts)?;
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    let result = {
        let mut result = result;
        recording.mark()?;
        let (phase_ns, body_ns) = recording.finish()?;
        guard
            .backend
            .accept_layer_durations(&mut result, phase_ns, body_ns)?;
        result
    };
    guard.committed = true;
    Ok(result)
}

struct PendingLayer {
    completion: Completion,
    proof: arena::Retired,
    generation: u64,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    paired_durations: ([u64; 7], u64),
}
struct NativeLayer<'owner, 'kernel> {
    op: LayerOperation<'owner>,
    prefix_inputs: Option<[Gfx950EngineeringPeerScopedPrefixInputsV1<'kernel>; 2]>,
    inputs: Option<Inputs<'kernel>>,
    outputs: [Gfx950EngineeringPeerBufferV1; 2],
    timeout_ms: u32,
    until: Option<Instant>,
    window: Option<scoped_currentness::Window>,
}
impl ClosedBackend for NativeLayer<'_, '_> {
    type Prefix = Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6;
    type Pending = PendingLayer;
    type Hidden = [Vec<u8>; 2];
    type Counts = crate::device::ScopedCountsV1;
    type Output = Gfx950EngineeringPeerScopedWarmLayerObservationV1;
    fn enter(&mut self) -> Result<()> {
        let op = &mut self.op;
        let until = deadline(Instant::now(), self.timeout_ms)?;
        warm_admission(
            op.pair.phase,
            op.pair.arena_policy,
            op.pair.owners[0].generation,
            op.pair.reusable.is_some(),
            op.pair.completed.is_some(),
        )?;
        op.pair
            .reusable
            .as_ref()
            .ok_or("scoped genuine reusable proof absent")?
            .require_generation(op.pair.owners[0].generation)?;
        op.group.require_active()?;
        profiles::validate_policy(op.group)?;
        if op.pair.binding.output_policy != profiles::OutputPolicy::ExactOwnResidual {
            return Err("scoped layer requires exact own residual policy".into());
        }
        if op.pair.binding
            != Binding::capture_with_policy(
                op.group,
                self.inputs.as_ref().ok_or("scoped inputs absent")?,
                op.pair.binding.output_policy,
            )
        {
            return Err("scoped layer retained input/kernel role binding changed".into());
        }
        op.pair.phase = Phase::Busy;
        self.window = Some(scoped_currentness::Window::enter(op.group, until)?);
        self.until = Some(until);
        profiles::validate_owners(op.group, &op.pair.owners)?;
        scoped_currentness::Currentness::Scoped(
            self.window.as_mut().ok_or("scoped window absent")?,
        )
        .idle_group(op.group)?;
        Ok(())
    }
    fn prefix(&mut self) -> Result<Self::Prefix> {
        let [left, right] = self
            .prefix_inputs
            .take()
            .ok_or("scoped prefix inputs consumed")?;
        let [left_state, right_state] = &mut self.op.prefixes;
        let prefix = wave_qkv_attention_output_tiles_v6::run_scoped(
            self.op.group,
            [
                Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6 {
                    kernel: left.kernel,
                    object_sha256: left.object_sha256,
                    roots: left.roots,
                    state: left_state,
                    timeout_ms: left.timeout_ms,
                },
                Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6 {
                    kernel: right.kernel,
                    object_sha256: right.object_sha256,
                    roots: right.roots,
                    state: right_state,
                    timeout_ms: right.timeout_ms,
                },
            ],
            &mut scoped_currentness::Currentness::Scoped(
                self.window.as_mut().ok_or("scoped window absent")?,
            ),
        )?;
        deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
        Ok(prefix)
    }
    fn mlp(&mut self) -> Result<Self::Pending> {
        let until = self.until.ok_or("scoped deadline absent")?;
        let mut native = Native {
            group: self.op.group,
            owners: &mut self.op.pair.owners,
            inputs: self.inputs.take().ok_or("scoped MLP inputs consumed")?,
            output_policy: self.op.pair.binding.output_policy,
            timeout_ms: self.timeout_ms,
            generation: 0,
            staged: None,
            reusable: self.op.pair.reusable.take().map(|proof| (proof, until)),
            validated_terminal: None,
            currentness: scoped_currentness::Currentness::Scoped(
                self.window.as_mut().ok_or("scoped window absent")?,
            ),
        };
        #[cfg(not(feature = "engineering-currentness-duration-diagnostics"))]
        let completion = coordinate_with_terminal_policy(
            &mut native,
            self.timeout_ms,
            Some(until),
            TerminalPolicy::Legacy,
        )?;
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        let (completion, paired_durations) = {
            let mut clock = layer_duration::Monotonic;
            let mut recording = layer_duration::Recorder::<7>::new(&mut clock);
            let completion = coordinate_recorded(
                &mut native,
                self.timeout_ms,
                Some(until),
                TerminalPolicy::Legacy,
                Some(&mut recording),
            )?;
            (completion, recording.finish()?)
        };
        deadline_check(Instant::now(), until)?;
        let proof = native
            .staged
            .take()
            .ok_or("scoped paired completed arena absent")?
            .seal_retired_currentness(native.group, until, &mut native.currentness)?;
        Ok(PendingLayer {
            completion,
            proof,
            generation: native.generation,
            #[cfg(feature = "engineering-currentness-duration-diagnostics")]
            paired_durations,
        })
    }
    fn hidden(&mut self) -> Result<Self::Hidden> {
        // Ordinary two reads preserve their original group/rank fence placements.
        let mut route = scoped_currentness::Currentness::Scoped(
            self.window.as_mut().ok_or("scoped window absent")?,
        );
        let hidden = [
            self.op
                .group
                .read_currentness(self.outputs[0], 0, 8192, &mut route)?,
            self.op
                .group
                .read_currentness(self.outputs[1], 0, 8192, &mut route)?,
        ];
        hidden_pair(&hidden)?;
        deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
        Ok(hidden)
    }
    fn exit(&mut self) -> Result<Self::Counts> {
        let counts = self
            .window
            .as_mut()
            .ok_or("scoped window absent")?
            .finish(self.op.group)?;
        deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
        Ok(counts)
    }
    fn commit(
        &mut self,
        prefix: Self::Prefix,
        pending: Self::Pending,
        hidden: Self::Hidden,
        counts: Self::Counts,
    ) -> Result<Self::Output> {
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        let currentness_durations = self
            .window
            .as_ref()
            .ok_or("scoped duration window absent")?
            .durations()?;
        deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
        // No Completed pair or reusable proof escapes before the full exit.
        self.op.pair.completed = Some(Completed {
            generation: pending.generation,
            states: pending.completion.states.clone(),
            proof: pending.proof,
        });
        self.op.pair.phase = Phase::Completed;
        let result = Gfx950EngineeringPeerScopedWarmLayerObservationV1 {
            prefix,
            mlp: pending.completion.into(),
            hidden,
            currentness: counts.into(),
            #[cfg(feature = "engineering-currentness-duration-diagnostics")]
            currentness_durations,
            #[cfg(feature = "engineering-currentness-duration-diagnostics")]
            layer_durations: Gfx950EngineeringPeerScopedLayerDurationsV1 {
                phase_ns: [0; 6],
                layer_body_ns: 0,
                paired_mlp_phase_ns: pending.paired_durations.0,
                paired_mlp_body_ns: pending.paired_durations.1,
            },
        };
        #[cfg(not(feature = "engineering-currentness-duration-diagnostics"))]
        {
            deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
            self.op.committed = true;
        }
        Ok(result)
    }
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    fn accept_layer_durations(
        &mut self,
        output: &mut Self::Output,
        phase_ns: [u64; 6],
        body_ns: u64,
    ) -> Result<()> {
        let mut value = output.layer_durations;
        value.phase_ns = phase_ns;
        value.layer_body_ns = body_ns;
        value.validate()?;
        if output
            .currentness_durations
            .checked_sum_ns()
            .map_err(explain)?
            > body_ns
        {
            return Err("layer callbacks exceed closed body".into());
        }
        output.layer_durations = value;
        // Keep both guards armed through acceptance and the original final deadline.
        deadline_check(Instant::now(), self.until.ok_or("scoped deadline absent")?)?;
        self.op.committed = true;
        Ok(())
    }
    fn quarantine(&mut self) {
        self.op.quarantine();
    }
}

pub(in super::super) fn run(
    group: &mut Gfx950EngineeringPeerGroupV1,
    prefixes: [&mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2],
    pair: &mut RetainedPair,
    prefix_inputs: [Gfx950EngineeringPeerScopedPrefixInputsV1<'_>; 2],
    inputs: Inputs<'_>,
    timeout_ms: u32,
) -> Result<Gfx950EngineeringPeerScopedWarmLayerObservationV1> {
    let outputs = inputs.ranks.each_ref().map(|rank| rank.output);
    closed_layer(&mut NativeLayer {
        op: LayerOperation {
            group,
            prefixes,
            pair,
            committed: false,
        },
        prefix_inputs: Some(prefix_inputs),
        inputs: Some(inputs),
        outputs,
        timeout_ms,
        until: None,
        window: None,
    })
}

pub(in super::super) fn run_census(
    group: &mut Gfx950EngineeringPeerGroupV1,
    prefixes: [&mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2],
    pair: &mut RetainedPair,
    prefix_inputs: [Gfx950EngineeringPeerScopedPrefixInputsV1<'_>; 2],
    inputs: Inputs<'_>,
    expected_owner_counts: [usize; 2],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringPeerScopedCensusWarmLayerObservationV1> {
    let outputs = inputs.ranks.each_ref().map(|rank| rank.output);
    census::run(
        NativeLayer {
            op: LayerOperation {
                group,
                prefixes,
                pair,
                committed: false,
            },
            prefix_inputs: Some(prefix_inputs),
            inputs: Some(inputs),
            outputs,
            timeout_ms,
            until: None,
            window: None,
        },
        expected_owner_counts,
    )
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_scoped_layer_v1_tests.rs"]
mod tests;
