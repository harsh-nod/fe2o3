//! Borrow-free paired owners, with short exclusive group custody per operation.
use super::*;

#[path = "engineering_gfx950_peer_combined_mlp_paired_mixed_bank_v1.rs"]
mod mixed_bank;
pub use mixed_bank::Entry as GuardedBankEntry;
pub(in super::super) use mixed_bank::{initial_bank, rearm_bank};

#[derive(Eq, PartialEq)]
struct Binding {
    group: u64,
    output_policy: profiles::OutputPolicy,
    kernels: [[(u64, usize, u64, [u8; 32]); 4]; 2],
    roots: [[Gfx950EngineeringPeerBufferV1; 10]; 2],
    partials: [Gfx950EngineeringPeerBufferV1; 2],
    residuals: [Gfx950EngineeringPeerBufferV1; 2],
    outputs: [Gfx950EngineeringPeerBufferV1; 2],
    images: [[u8; 32]; 2],
}

impl Binding {
    fn capture(group: &Gfx950EngineeringPeerGroupV1, inputs: &Inputs<'_>) -> Self {
        Self::capture_with_policy(group, inputs, profiles::OutputPolicy::Strict)
    }

    fn capture_with_policy(
        group: &Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        output_policy: profiles::OutputPolicy,
    ) -> Self {
        Self {
            group: group.incarnation,
            output_policy,
            kernels: inputs.ranks.each_ref().map(|rank| {
                rank.kernels.map(|kernel| {
                    (
                        kernel.group,
                        kernel.rank,
                        kernel.id,
                        kernel.metadata.object_sha256,
                    )
                })
            }),
            roots: inputs.ranks.each_ref().map(|rank| rank.mlp_roots),
            partials: inputs.partials,
            residuals: inputs.ranks.each_ref().map(|rank| rank.residual_input),
            outputs: inputs.ranks.each_ref().map(|rank| rank.output),
            images: [inputs.projection_sha256, inputs.mlp_sha256],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    Busy,
    Completed,
    Poisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArenaPolicy {
    Fresh,
    ReuseRetired,
}

impl ArenaPolicy {
    fn require_run(self, generation: u64, reusable: bool) -> Result<()> {
        let valid = match self {
            Self::Fresh => generation != 0 && !reusable,
            Self::ReuseRetired => generation != 0 && reusable == (generation > 1),
        };
        if !valid {
            return Err("retained paired arena policy or generation custody changed".into());
        }
        Ok(())
    }
}

struct Completed {
    generation: u64,
    states: [CombinedMlpSnapshotV1; 2],
    proof: arena::Retired,
}

/// No borrowed Group/kernel references, mutable owner views, or caller proofs.
pub struct RetainedPair {
    owners: [CombinedMlpStateV1; 2],
    binding: Binding,
    phase: Phase,
    completed: Option<Completed>,
    arena_policy: ArenaPolicy,
    reusable: Option<arena::Reusable>,
}

/// Initialized storage only: it cannot run, rearm, or expose owner pointers.
/// Dropping this non-Clone value leaves its allocation backing in Group custody.
pub struct UnboundPair {
    group: u64,
    owners: [CombinedMlpStateV1; 2],
}

fn poison_group(group: &mut Gfx950EngineeringPeerGroupV1) {
    group.poisoned = true;
    for context in &mut group.contexts {
        context.ordered_batch_poisoned = true;
    }
}

struct Allocation<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    owners: Vec<CombinedMlpStateV1>,
    committed: bool,
}

impl Drop for Allocation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            for owner in &mut self.owners {
                owner.poison(self.group);
            }
            poison_group(self.group);
        }
    }
}

impl Allocation<'_> {
    fn allocate_owners(&mut self, until: Instant) -> Result<()> {
        self.group.require_active()?;
        profiles::validate_policy(self.group)?;
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)?;
        for rank in 0..2 {
            self.owners.push(super::super::allocate(self.group, rank)?);
            deadline_check(Instant::now(), until)?;
        }
        Ok(())
    }

    fn bind(
        &mut self,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
        until: Instant,
        output_policy: profiles::OutputPolicy,
    ) -> Result<RetainedPair> {
        let owners: &mut [CombinedMlpStateV1; 2] = self
            .owners
            .as_mut_slice()
            .try_into()
            .map_err(|_| "retained paired owner count")?;
        match output_policy {
            profiles::OutputPolicy::Strict => {
                profiles::prepare(self.group, owners, inputs, timeout_ms)?
            }
            profiles::OutputPolicy::ExactOwnResidual => {
                profiles::prepare_exact_own_residual(self.group, owners, inputs, timeout_ms)?
            }
        };
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)?;
        deadline_check(Instant::now(), until)?;
        let binding = Binding::capture_with_policy(self.group, inputs, output_policy);
        let owners = std::mem::take(&mut self.owners)
            .try_into()
            .map_err(|_| "retained paired allocated owner count")?;
        let pair = RetainedPair {
            owners,
            binding,
            phase: Phase::Ready,
            completed: None,
            arena_policy: ArenaPolicy::Fresh,
            reusable: None,
        };
        deadline_check(Instant::now(), until)?;
        self.committed = true;
        Ok(pair)
    }
}

fn unbound_identity(
    captured_group: u64,
    current_group: u64,
    owners: &[CombinedMlpStateV1],
) -> Result<()> {
    if captured_group == 0 || captured_group != current_group || owners.len() != 2 {
        return Err("unbound paired group or owner count".into());
    }
    for (rank, owner) in owners.iter().enumerate() {
        if owner.buffer.group != captured_group
            || owner.buffer.id == 0
            || owner.buffer.owner != rank
            || owner.buffer.bytes != COMBINED_BYTES as u64
            || owner.generation != 1
            || owner.activation != Activation::Ready
        {
            return Err("unbound paired initial identity changed".into());
        }
    }
    if owners[0].buffer.id == owners[1].buffer.id {
        return Err("unbound paired duplicate owner".into());
    }
    Ok(())
}

impl UnboundPair {
    /// No kernels or payload roles are needed to reserve genuine initial state.
    pub(in super::super) fn allocate(
        group: &mut Gfx950EngineeringPeerGroupV1,
        timeout_ms: u32,
    ) -> Result<Self> {
        let mut custody = Allocation {
            group,
            owners: Vec::with_capacity(2),
            committed: false,
        };
        let until = deadline(Instant::now(), timeout_ms)?;
        custody.allocate_owners(until)?;
        unbound_identity(
            custody.group.incarnation,
            custody.group.incarnation,
            &custody.owners,
        )?;
        let owners: &mut [CombinedMlpStateV1; 2] = custody
            .owners
            .as_mut_slice()
            .try_into()
            .map_err(|_| "unbound paired allocated owner count")?;
        // Physical identity and the initial atomic readback are checked now and
        // again at bind, after potentially intervening catalog initialization.
        profiles::validate_owners(custody.group, owners)?;
        check_contexts(
            &mut custody.group.contexts,
            custody.group.shared_full_currentness,
        )?;
        deadline_check(Instant::now(), until)?;
        let value = Self {
            group: custody.group.incarnation,
            owners: std::mem::take(&mut custody.owners)
                .try_into()
                .map_err(|_| "unbound paired allocated owner count")?,
        };
        deadline_check(Instant::now(), until)?;
        custody.committed = true;
        Ok(value)
    }

    /// # Safety
    /// All RetainedPair::allocate lifetime and future-payload obligations apply.
    /// Binding consumes storage once; any failure is terminal, not retryable.
    pub(in super::super) unsafe fn bind(
        self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<RetainedPair> {
        // SAFETY: caller supplies the same entire-lifetime reviewed contract.
        unsafe { self.bind_with_policy(group, inputs, timeout_ms, profiles::OutputPolicy::Strict) }
    }

    /// # Safety
    /// All RetainedPair::allocate_exact_own_residual lifetime obligations apply,
    /// including exclusive quiescent updates and the R1-before-R2 alias lifetime.
    pub(in super::super) unsafe fn bind_exact_own_residual(
        self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<RetainedPair> {
        // SAFETY: caller supplies the closed exact-residual lifetime contract.
        unsafe {
            self.bind_with_policy(
                group,
                inputs,
                timeout_ms,
                profiles::OutputPolicy::ExactOwnResidual,
            )
        }
    }

    /// # Safety
    /// The exact-own-residual bind contract applies. Arena reuse is immutable
    /// for this pair and requires the private whole-bank retirement path.
    pub(in super::super) unsafe fn bind_exact_own_residual_reusable(
        self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<RetainedPair> {
        // SAFETY: same reviewed lifetime contract; no new memory access rights.
        let mut pair = unsafe { self.bind_exact_own_residual(group, inputs, timeout_ms)? };
        pair.arena_policy = ArenaPolicy::ReuseRetired;
        Ok(pair)
    }

    unsafe fn bind_with_policy(
        self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
        output_policy: profiles::OutputPolicy,
    ) -> Result<RetainedPair> {
        let mut custody = Allocation {
            group,
            owners: self.owners.into(),
            committed: false,
        };
        let until = deadline(Instant::now(), timeout_ms)?;
        unbound_identity(self.group, custody.group.incarnation, &custody.owners)?;
        custody.group.require_active()?;
        profiles::validate_policy(custody.group)?;
        check_contexts(
            &mut custody.group.contexts,
            custody.group.shared_full_currentness,
        )?;
        deadline_check(Instant::now(), until)?;
        let owners: &[CombinedMlpStateV1; 2] = custody
            .owners
            .as_slice()
            .try_into()
            .map_err(|_| "unbound paired bind owner count")?;
        if profiles::validate_owners(custody.group, owners)? != 1 {
            return Err("unbound paired bind requires initial generation".into());
        }
        deadline_check(Instant::now(), until)?;
        custody.bind(inputs, timeout_ms, until, output_policy)
    }
}

struct Operation<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    pairs: &'a mut [RetainedPair],
    committed: bool,
    owner_ids: BTreeSet<(u64, u64)>,
    arena_ids: BTreeSet<(u64, u64)>,
}

impl<'a> Operation<'a> {
    fn new(group: &'a mut Gfx950EngineeringPeerGroupV1, pairs: &'a mut [RetainedPair]) -> Self {
        Self {
            group,
            pairs,
            committed: false,
            owner_ids: BTreeSet::new(),
            arena_ids: BTreeSet::new(),
        }
    }

    fn begin(&mut self, expected: Phase) -> Result<()> {
        if self.pairs.iter().any(|pair| pair.phase != expected) {
            return Err("retained paired custody phase mismatch".into());
        }
        for pair in &mut *self.pairs {
            pair.phase = Phase::Busy;
        }
        self.group.require_active()?;
        profiles::validate_policy(self.group)?;
        if self
            .pairs
            .iter()
            .any(|pair| pair.binding.group != self.group.incarnation)
        {
            return Err("retained paired group incarnation mismatch".into());
        }
        Ok(())
    }

    fn quarantine(&mut self) {
        for pair in &mut *self.pairs {
            pair.phase = Phase::Poisoned;
            for owner in &mut pair.owners {
                owner.poison(self.group);
            }
        }
        poison_group(self.group);
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.quarantine();
        }
    }
}

fn deadline(start: Instant, timeout_ms: u32) -> Result<Instant> {
    if !(1..=10_000).contains(&timeout_ms) {
        return Err("retained paired timeout outside 1..10000 ms".into());
    }
    start
        .checked_add(Duration::from_millis(u64::from(timeout_ms)))
        .ok_or_else(|| "retained paired deadline overflow".into())
}

impl RetainedPair {
    /// # Safety
    /// Dispatch safety obligations hold for the entire pair lifetime, including
    /// every future payload update/run: dedicated disposable process, reviewed
    /// exact images and model-role joins, completed coherent attention producers,
    /// and no other dispatch accessing the private owner storage. Failures are
    /// terminal. Whole-model bank/forward ordering is the Ferric caller's job.
    pub(in super::super) unsafe fn allocate(
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<Self> {
        // SAFETY: the original strict constructor's lifetime obligations apply.
        unsafe {
            Self::allocate_with_policy(group, inputs, timeout_ms, profiles::OutputPolicy::Strict)
        }
    }

    /// # Safety
    /// All allocate() obligations apply. Both outputs must be the exact old R1
    /// residual allocations, disjoint from every MLP root and peer live input.
    /// Reuse relies on R1 completion followed by both validators before R2;
    /// no other producer/consumer may access them during an in-flight paired
    /// execution. Between-run updates require completed, quiescent custody.
    pub(in super::super) unsafe fn allocate_exact_own_residual(
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<Self> {
        // SAFETY: this constructor requires the closed cross-stage lifetime.
        unsafe {
            Self::allocate_with_policy(
                group,
                inputs,
                timeout_ms,
                profiles::OutputPolicy::ExactOwnResidual,
            )
        }
    }

    unsafe fn allocate_with_policy(
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: &Inputs<'_>,
        timeout_ms: u32,
        output_policy: profiles::OutputPolicy,
    ) -> Result<Self> {
        let mut custody = Allocation {
            group,
            owners: Vec::with_capacity(2),
            committed: false,
        };
        let until = deadline(Instant::now(), timeout_ms)?;
        custody.allocate_owners(until)?;
        custody.bind(inputs, timeout_ms, until, output_policy)
    }

    pub(in super::super) fn run(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        inputs: Inputs<'_>,
        timeout_ms: u32,
    ) -> Result<Completion> {
        let mut op = Operation::new(group, std::slice::from_mut(self));
        let until = deadline(Instant::now(), timeout_ms)?;
        // Reject missing/extra private reuse custody before touching a context
        // or allowing any fresh allocation. Operation already owns quarantine.
        op.pairs[0].arena_policy.require_run(
            op.pairs[0].owners[0].generation,
            op.pairs[0].reusable.is_some(),
        )?;
        op.begin(Phase::Ready)?;
        let pair = &mut op.pairs[0];
        if pair.completed.is_some()
            || pair.binding
                != Binding::capture_with_policy(op.group, &inputs, pair.binding.output_policy)
        {
            return Err("retained paired input/kernel role binding changed".into());
        }
        let mut native = Native {
            group: op.group,
            owners: &mut pair.owners,
            inputs,
            output_policy: pair.binding.output_policy,
            timeout_ms,
            generation: 0,
            staged: None,
            reusable: pair.reusable.take().map(|proof| (proof, until)),
            validated_terminal: None,
        };
        // Keep the unchanged coordinator's strict in-flight reservation checks.
        let result = coordinate_until(&mut native, timeout_ms, Some(until))?;
        deadline_check(Instant::now(), until)?;
        let proof = native
            .staged
            .take()
            .ok_or("retained paired completed arena absent")?
            .seal_retired(native.group, until)?;
        let generation = native.generation;
        pair.completed = Some(Completed {
            generation,
            states: result.states.clone(),
            proof,
        });
        deadline_check(Instant::now(), until)?;
        pair.phase = Phase::Completed;
        op.committed = true;
        Ok(result)
    }

    fn finish_rearm(&mut self, next: u64) -> Result<()> {
        if self.reusable.is_some() || self.phase != Phase::Busy {
            return Err("retained paired rearm arena custody changed".into());
        }
        let old = self
            .completed
            .take()
            .ok_or("retained paired rearm proof absent")?;
        if old.generation == 0 || old.generation.checked_add(1) != Some(next) {
            return Err("retained paired rearm arena generation changed".into());
        }
        if self.arena_policy == ArenaPolicy::ReuseRetired {
            self.reusable = Some(arena::Reusable::new(old.proof, old.generation, next)?);
        }
        // Fresh mode deliberately drops only metadata; backing stays in Group.
        self.phase = Phase::Ready;
        Ok(())
    }
}

trait RearmBackend {
    fn now(&mut self) -> Instant;
    fn validate(&mut self, pair: usize, deadline: Instant) -> Result<u64>;
    fn fence(&mut self) -> Result<()>;
    fn reset(&mut self, pair: usize, rank: usize, next: u64) -> Result<()>;
    fn initial(&mut self, pair: usize, next: u64) -> Result<()>;
    fn quarantine(&mut self);
}

fn rearm_all(
    backend: &mut impl RearmBackend,
    count: usize,
    timeout_ms: u32,
    outer_deadline: Option<Instant>,
) -> Result<u64> {
    let result = (|| {
        let until = deadline(backend.now(), timeout_ms)?;
        let until = outer_deadline.map_or(until, |outer| outer.min(until));
        deadline_check(backend.now(), until)?;
        if !(1..=72).contains(&count) {
            return Err("retained paired batch count outside 1..72".into());
        }
        let mut next = None;
        // Validate the whole selected set before any atomic reset. No external
        // snapshots are accepted and every member must advance the same local generation.
        for pair in 0..count {
            let old = backend.validate(pair, until)?;
            let candidate = old
                .checked_add(1)
                .filter(|_| old != 0)
                .ok_or("retained paired generation exhausted or zero")?;
            if next.is_some_and(|value| value != candidate) {
                return Err("retained paired batch generations differ".into());
            }
            next = Some(candidate);
            deadline_check(backend.now(), until)?;
        }
        let next = next.ok_or("retained paired batch empty")?;
        backend.fence()?;
        deadline_check(backend.now(), until)?;
        for pair in 0..count {
            for rank in 0..2 {
                backend.reset(pair, rank, next)?;
                deadline_check(backend.now(), until)?;
            }
        }
        for pair in 0..count {
            backend.initial(pair, next)?;
            deadline_check(backend.now(), until)?;
        }
        backend.fence()?;
        deadline_check(backend.now(), until)?;
        Ok(next)
    })();
    if result.is_err() {
        backend.quarantine();
    }
    result
}

fn validate_completed_pair(
    group: &mut Gfx950EngineeringPeerGroupV1,
    pair: &mut RetainedPair,
    owner_ids: &mut BTreeSet<(u64, u64)>,
    arena_ids: &mut BTreeSet<(u64, u64)>,
    until: Instant,
) -> Result<u64> {
    if pair.phase != Phase::Busy || pair.binding.group != group.incarnation {
        return Err("retained paired rearm custody/group mismatch".into());
    }
    let old = pair
        .completed
        .as_mut()
        .ok_or("retained paired terminal proof missing")?;
    old.proof.recheck(group, until)?;
    for id in old.proof.identities() {
        if !arena_ids.insert(id) {
            return Err("retained paired duplicate arena".into());
        }
    }
    for rank in 0..2 {
        let owner = &pair.owners[rank];
        if owner.buffer.owner != rank || !owner_ids.insert((owner.buffer.group, owner.buffer.id)) {
            return Err("retained paired duplicate or misranked owner".into());
        }
        require_activation(owner.activation, Activation::Completed)?;
        if owner.generation != old.generation || owner.observe(group)? != old.states[rank] {
            return Err("retained paired terminal generation/state drift".into());
        }
        require_terminal(&old.states[rank], old.generation)?;
        deadline_check(Instant::now(), until)?;
    }
    Ok(old.generation)
}

impl RearmBackend for Operation<'_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn validate(&mut self, index: usize, until: Instant) -> Result<u64> {
        validate_completed_pair(
            self.group,
            &mut self.pairs[index],
            &mut self.owner_ids,
            &mut self.arena_ids,
            until,
        )
    }

    fn fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn reset(&mut self, index: usize, rank: usize, next: u64) -> Result<()> {
        let pair = &mut self.pairs[index];
        let old = pair
            .completed
            .as_ref()
            .ok_or("retained paired reset lost proof")?;
        // SAFETY: all selected sealed batches/signals/terminal owners and the
        // current idle queues were validated before the first store. Exclusive
        // Group custody prevents intervening publication. No arenas are reset.
        unsafe { pair.owners[rank].rearm_quiescent(self.group, &old.states[rank], next) }
    }

    fn initial(&mut self, index: usize, next: u64) -> Result<()> {
        if profiles::validate_owners(self.group, &self.pairs[index].owners)? != next {
            return Err("retained paired batch initial readback/generation mismatch".into());
        }
        Ok(())
    }

    fn quarantine(&mut self) {
        Operation::quarantine(self);
    }
}

/// Only the guarded MLP owners are covered here. Ferric must also fence and
/// validate its Prefix284 bank and global forward ledger before whole-bank reuse.
pub(in super::super) fn rearm_pairs(
    group: &mut Gfx950EngineeringPeerGroupV1,
    pairs: &mut [RetainedPair],
    timeout_ms: u32,
) -> Result<u64> {
    let mut op = Operation::new(group, pairs);
    let until = deadline(Instant::now(), timeout_ms)?;
    if !(1..=72).contains(&op.pairs.len()) {
        return Err("retained paired batch count outside 1..72".into());
    }
    op.begin(Phase::Completed)?;
    let count = op.pairs.len();
    let next = rearm_all(&mut op, count, timeout_ms, Some(until))?;
    for pair in &mut *op.pairs {
        pair.finish_rearm(next)?;
    }
    deadline_check(Instant::now(), until)?;
    op.committed = true;
    Ok(next)
}

#[cfg(test)]
pub(in super::super) struct DiagnosticPair {
    pub owners: [[u64; 4]; 2],
    pub generation: u64,
    pub completed: bool,
    pub states: [CombinedMlpSnapshotV1; 2],
    pub frontiers: [(u64, u64); 2],
}

#[cfg(test)]
impl RetainedPair {
    /// Observation only: no token, mutable owner, or transferable retired proof.
    pub(in super::super) fn observe_for_test(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        timeout_ms: u32,
    ) -> Result<DiagnosticPair> {
        let mut op = Operation::new(group, std::slice::from_mut(self));
        let until = deadline(Instant::now(), timeout_ms)?;
        let phase = op.pairs[0].phase;
        if !matches!(phase, Phase::Ready | Phase::Completed) {
            return Err("retained diagnostic requires quiescent custody".into());
        }
        op.begin(phase)?;
        check_contexts(&mut op.group.contexts, op.group.shared_full_currentness)?;
        let generation = if phase == Phase::Completed {
            let pair = &op.pairs[0];
            let old = pair
                .completed
                .as_ref()
                .ok_or("retained diagnostic terminal proof absent")?;
            if pair.owners[0].buffer.id == pair.owners[1].buffer.id {
                return Err("retained diagnostic duplicate owners".into());
            }
            for rank in 0..2 {
                require_activation(pair.owners[rank].activation, Activation::Completed)?;
                if pair.owners[rank].owner_rank() != rank
                    || pair.owners[rank].generation != old.generation
                {
                    return Err("retained diagnostic terminal owner generation/rank".into());
                }
            }
            old.generation
        } else {
            if op.pairs[0].completed.is_some() {
                return Err("retained diagnostic Ready with old proof".into());
            }
            profiles::validate_owners(op.group, &op.pairs[0].owners)?
        };
        let pair = &mut op.pairs[0];
        let states = [
            pair.owners[0].observe(op.group)?,
            pair.owners[1].observe(op.group)?,
        ];
        for state in &states {
            if phase == Phase::Ready {
                require_initial(state, generation)?;
            } else {
                require_terminal(state, generation)?;
            }
        }
        if phase == Phase::Completed
            && states
                != pair
                    .completed
                    .as_ref()
                    .ok_or("retained diagnostic completed proof missing")?
                    .states
        {
            return Err("retained diagnostic terminal readback changed".into());
        }
        check_contexts(&mut op.group.contexts, op.group.shared_full_currentness)?;
        let value = DiagnosticPair {
            owners: pair.owners.each_ref().map(|owner| {
                let token = owner.buffer;
                [token.group, token.id, token.owner as u64, token.bytes]
            }),
            generation,
            completed: phase == Phase::Completed,
            states,
            frontiers: std::array::from_fn(|rank| {
                let context = &op.group.contexts[rank];
                (context.ring.write(), context.last_observed_read)
            }),
        };
        deadline_check(Instant::now(), until)?;
        pair.phase = phase;
        op.committed = true;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_retained_v1_tests.rs"]
mod tests;
