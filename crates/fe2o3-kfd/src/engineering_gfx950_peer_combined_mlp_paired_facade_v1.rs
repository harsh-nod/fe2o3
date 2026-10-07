//! Engineering-only adapters; copied observations never confer execution rights.
use super::*;

/// Borrow-free initialized storage, consumed exactly once by an unsafe bind.
/// Backing remains in its original Group until Close. This is not production
/// authority and exposes neither a buffer token nor the underlying atomics.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound;
/// fn expose(value: Unbound) { let _ = value.owners; }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound;
/// fn duplicate(value: Unbound) { let _ = value.clone(); }
/// ```
/// Unbound storage is not dispatchable:
/// ```compile_fail
/// use fe2o3_kfd::{Gfx950EngineeringPeerGroupV1 as Group,
///     Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound,
///     Gfx950EngineeringPeerGuardedMlpInputsV1 as Inputs};
/// fn dispatch(group: &mut Group, storage: &mut Unbound, inputs: Inputs<'_>) {
///     let _ = group.dispatch_guarded_mlp_pair_v1(storage, inputs, 10);
/// }
/// ```
/// Even an unsuccessful bind consumes the storage:
/// ```compile_fail
/// use fe2o3_kfd::{Gfx950EngineeringPeerGroupV1 as Group,
///     Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound,
///     Gfx950EngineeringPeerGuardedMlpInputsV1 as Inputs};
/// unsafe fn bind_twice(group: &mut Group, storage: Unbound, inputs: &Inputs<'_>) {
///     unsafe {
///         let _ = group.bind_guarded_mlp_pair_unchecked_v1(storage, inputs, 10);
///         let _ = group.bind_guarded_mlp_pair_unchecked_v1(storage, inputs, 10);
///     }
/// }
/// ```
pub type Gfx950EngineeringPeerUnboundGuardedMlpPairV1 = retained::UnboundPair;

/// Opaque, non-Clone paired owners with no persistent Group or kernel borrow.
/// The unsafe bind's lifetime contract applies until these owners are dropped.
/// Dropping them does not release backing before Group Close.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair;
/// fn expose(value: Pair) { let _ = value.owners; }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair;
/// fn duplicate(value: Pair) { let _ = value.clone(); }
/// ```
/// Retirement proofs remain private even after successful dispatch:
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair;
/// fn expose_proof(value: Pair) { let _ = value.completed; }
/// ```
/// Supplying every field name does not make an owner constructible:
/// ```compile_fail
/// use fe2o3_kfd::Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair;
/// fn fabricate() -> Pair {
///     Pair { owners: todo!(), binding: todo!(), phase: todo!(), completed: None,
///         arena_policy: todo!(), reusable: None }
/// }
/// ```
/// Observations cannot be promoted to paired ownership:
/// ```compile_fail
/// use fe2o3_kfd::{Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair,
///     Gfx950EngineeringPeerGuardedMlpObservationV1 as Observation};
/// fn promote(observation: Observation) -> Pair { observation.into() }
/// ```
pub type Gfx950EngineeringPeerRetainedGuardedMlpPairV1 = retained::RetainedPair;

/// Ordered roles are forwarded unchanged; construction does not validate them.
pub struct Gfx950EngineeringPeerGuardedMlpRankInputsV1<'a> {
    /// Exact role order: R1, MLP, validator, guarded R2.
    pub kernels: [&'a Gfx950EngineeringPeerKernelV1; 4],
    pub mlp_roots: [Gfx950EngineeringPeerBufferV1; 10],
    pub residual_input: Gfx950EngineeringPeerBufferV1,
    pub output: Gfx950EngineeringPeerBufferV1,
}

/// Two rank-local role sets and the reviewed projection/MLP image identities.
pub struct Gfx950EngineeringPeerGuardedMlpInputsV1<'a> {
    pub ranks: [Gfx950EngineeringPeerGuardedMlpRankInputsV1<'a>; 2],
    pub partials: [Gfx950EngineeringPeerBufferV1; 2],
    pub projection_sha256: [u8; 32],
    pub mlp_sha256: [u8; 32],
}

impl<'a> Gfx950EngineeringPeerGuardedMlpInputsV1<'a> {
    fn private_inputs(&self) -> Inputs<'a> {
        Inputs {
            ranks: self.ranks.each_ref().map(|rank| RankInputs {
                kernels: rank.kernels,
                mlp_roots: rank.mlp_roots,
                residual_input: rank.residual_input,
                output: rank.output,
            }),
            partials: self.partials,
            projection_sha256: self.projection_sha256,
            mlp_sha256: self.mlp_sha256,
        }
    }
}

/// Data only, never a completion, quiescence, rearm, or production capability.
#[derive(Debug)]
pub struct Gfx950EngineeringPeerGuardedMlpObservationV1 {
    pub prefixes: [[u32; 548]; 2],
    pub guards: [[u32; 4]; 2],
    /// Actual observed (write, read) counters; reads may lag logical retirement.
    pub observed_queue_frontiers: [(u64, u64); 2],
    /// Host elapsed time, not a GPU kernel latency measurement.
    pub segment_host_ns: u64,
}

impl From<Completion> for Gfx950EngineeringPeerGuardedMlpObservationV1 {
    fn from(value: Completion) -> Self {
        let [first, second] = value.states;
        Self {
            prefixes: [first.prefix, second.prefix],
            guards: [first.guard, second.guard],
            observed_queue_frontiers: value.observed_queue_frontiers,
            segment_host_ns: value.segment_host_ns,
        }
    }
}

/// One mixed-bank entry: two genuine PrefixV6 owners and one opaque MLP pair.
/// All borrows are temporary; no token is inferred from an allocation extent.
pub type Gfx950EngineeringPeerGuardedMlpBankEntryV1<'a> = retained::GuardedBankEntry<'a>;

impl Gfx950EngineeringPeerGroupV1 {
    /// Allocate one genuine initial 552-word owner per rank before full binding.
    /// This reserves Group storage only and does not load kernels or dispatch.
    pub fn allocate_guarded_mlp_pair_storage_v1(
        &mut self,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerUnboundGuardedMlpPairV1> {
        UnboundPair::allocate(self, timeout_ms)
    }

    /// Bind the exact reviewed roles without additional Group allocations.
    /// Failure consumes the storage and quarantines the Group.
    ///
    /// # Safety
    /// The caller must review the supplied machine code, image identities,
    /// argument roles, extents, access effects, synchronization, and pointer
    /// lifetime for every future dispatch in a dedicated disposable process.
    /// Attention producers must be completed and their writes coherent before
    /// every dispatch. Whole-model bank and forward ordering remain the caller's
    /// responsibility; failures are terminal. All kernels and payload allocations
    /// must remain live in this Group throughout the retained pair's lifetime.
    /// Writable roots must be disjoint under the strict paired profile. Future
    /// payload writes require exclusive quiescence; no external GPU work may
    /// race a dispatch or alter the private state. This engineering contract
    /// does not grant protected or production authority.
    pub unsafe fn bind_guarded_mlp_pair_unchecked_v1(
        &mut self,
        storage: Gfx950EngineeringPeerUnboundGuardedMlpPairV1,
        inputs: &Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerRetainedGuardedMlpPairV1> {
        // SAFETY: the caller supplies the private constructor's full contract.
        unsafe { storage.bind(self, &inputs.private_inputs(), timeout_ms) }
    }

    /// Bind with only exact own-rank residual/output identity aliasing allowed.
    /// No token, extent, role, or pointer is substituted by this adapter.
    ///
    /// # Safety
    /// All obligations of `bind_guarded_mlp_pair_unchecked_v1` apply, except
    /// that both outputs must be exactly their same-rank, full residual
    /// allocations, disjoint from every MLP root and peer live input. R1 must
    /// finish reading them, and both validators must complete before guarded R2
    /// overwrites them. The caller
    /// must exclude every other producer/consumer during an in-flight segment
    /// and perform future finite residual updates only under exclusive
    /// quiescence. Cross-rank, partial, and other live-root aliases remain invalid.
    pub unsafe fn bind_guarded_mlp_pair_exact_own_residual_unchecked_v1(
        &mut self,
        storage: Gfx950EngineeringPeerUnboundGuardedMlpPairV1,
        inputs: &Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerRetainedGuardedMlpPairV1> {
        // SAFETY: caller established the entire-lifetime exact-residual contract.
        unsafe { storage.bind_exact_own_residual(self, &inputs.private_inputs(), timeout_ms) }
    }

    /// Bind exact-own-residual roles with bounded private arena reuse.
    /// The first dispatch allocates one paired arena per rank. Later dispatches
    /// reuse those same allocations only after successful whole-bank rearm,
    /// all ten completion signals, actual consumption of the old queue packets,
    /// exact next generation, and unchanged identities/currentness are checked.
    /// A failed check is terminal; no fresh-allocation fallback or pool exists.
    /// Existing binding methods continue allocating fresh arenas per dispatch.
    ///
    /// # Safety
    /// All obligations of `bind_guarded_mlp_pair_exact_own_residual_unchecked_v1`
    /// apply. In particular the caller must exclude external references to
    /// private signal/kernarg storage and keep bank retirement, all payloads,
    /// kernels and the Group live under exclusive custody. This does not enable
    /// cached admission, operational currentness, or production authority.
    ///
    /// The explicit public signature is usable without exposing private state:
    /// ```no_run
    /// use fe2o3_kfd::{Gfx950EngineeringPeerGroupV1 as Group,
    ///     Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound,
    ///     Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair,
    ///     Gfx950EngineeringPeerGuardedMlpInputsV1 as Inputs};
    /// unsafe fn bind(group: &mut Group, storage: Unbound, inputs: &Inputs<'_>) {
    ///     let _: Result<Pair, _> = unsafe {
    ///         group.bind_guarded_mlp_pair_exact_own_residual_reusable_unchecked_v1(storage, inputs, 10)
    ///     };
    /// }
    /// ```
    pub unsafe fn bind_guarded_mlp_pair_exact_own_residual_reusable_unchecked_v1(
        &mut self,
        storage: Gfx950EngineeringPeerUnboundGuardedMlpPairV1,
        inputs: &Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerRetainedGuardedMlpPairV1> {
        // SAFETY: same exact-residual lifetime contract, with private reuse only.
        unsafe {
            storage.bind_exact_own_residual_reusable(self, &inputs.private_inputs(), timeout_ms)
        }
    }

    /// Synchronously run the already-bound five-packet-per-rank coordinator.
    /// The returned data is not a public retirement proof or a second R2 request.
    pub fn dispatch_guarded_mlp_pair_v1(
        &mut self,
        pair: &mut Gfx950EngineeringPeerRetainedGuardedMlpPairV1,
        inputs: Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerGuardedMlpObservationV1> {
        pair.run(self, inputs.private_inputs(), timeout_ms)
            .map(Into::into)
    }

    /// Explicitly use paired terminal fences for one reusable-pair dispatch.
    /// Only pairs bound by the reusable exact-own-residual constructor qualify;
    /// Fresh-policy pairs are refused and quarantined. Their first dispatch
    /// still allocates its arenas normally; the arena policy is not changed.
    ///
    /// This changes currentness cadence inside the exclusive terminal-owner
    /// transition: two fresh full Group fences replace eight. All intermediate
    /// queue/fault probes, three atomic state observations per rank, actual
    /// completion signals, and deadline checks remain. Both owners stay
    /// Submitted until the trailing full fence and deadline succeed. Refusal or
    /// unwind quarantines both owners and the Group; no result is published.
    ///
    /// This is engineering-only, not cached or operational currentness. It does
    /// not change global policy, rearm/seal checks, or the ordinary dispatch
    /// method's cadence. No timing or throughput gain is implied.
    pub fn dispatch_guarded_mlp_pair_paired_terminal_v1(
        &mut self,
        pair: &mut Gfx950EngineeringPeerRetainedGuardedMlpPairV1,
        inputs: Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerGuardedMlpObservationV1> {
        pair.run_paired_terminal(self, inputs.private_inputs(), timeout_ms)
            .map(Into::into)
    }

    /// Validate the whole initial mixed bank without resetting any owner.
    pub fn validate_guarded_mlp_initial_bank_v1(
        &mut self,
        entries: &mut [Gfx950EngineeringPeerGuardedMlpBankEntryV1<'_>],
        timeout_ms: u32,
    ) -> Result<u64> {
        retained::initial_bank(self, entries, timeout_ms)
    }

    /// Validate all terminal owners before resetting any mixed-bank member.
    ///
    /// # Safety
    /// Every PrefixV6 producer and consumer must be permanently retired under
    /// the caller's whole-bank completion ledger. No submitted work may retain
    /// access to any bank state or reset payload. The caller must hold exclusive
    /// bank quiescence through validation, reset, and publication of the returned
    /// next generation. Private paired retirement checks remain mandatory;
    /// copied observations or empty queues alone cannot discharge this contract.
    pub unsafe fn rearm_guarded_mlp_bank_unchecked_v1(
        &mut self,
        entries: &mut [Gfx950EngineeringPeerGuardedMlpBankEntryV1<'_>],
        timeout_ms: u32,
    ) -> Result<u64> {
        // SAFETY: the caller supplies the PrefixV6/global-ledger premise.
        unsafe { retained::rearm_bank(self, entries, timeout_ms) }
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_facade_v1_tests.rs"]
mod tests;

impl Gfx950EngineeringPeerGroupV1 {
    /// Rearm exactly36 completed reusable exact-own-residual pairs under one
    /// full-entry/full-exit window. All private proof, signal, queue, identity,
    /// generation and acquired-state checks remain; internal topology checks
    /// become participant-local checks with fresh root/generation brackets.
    /// Transient changes which revert without generation advancement can
    /// escape. This is not temporally equivalent to ordinary full rearm.
    /// No shader, user callback, signal reset or lasting policy is introduced.
    /// Returned counts describe only this bank window, not a layer or route.
    ///
    /// # Safety
    /// All safety obligations of rearm_guarded_mlp_bank_unchecked_v1 apply.
    /// Every old Prefix producer/consumer must be permanently retired under
    /// the caller's whole-bank completion ledger. No queued or host access may
    /// interleave with this exclusive Group and typed36-entry borrow. Entries
    /// must belong to the same bank and generation; copied state observations
    /// are not retirement authority. Errors and unwinds quarantine the entire
    /// roster and Group, including partially reset or committed owners.
    pub unsafe fn rearm_guarded_mlp_bank_scoped_currentness_unchecked_v1(
        &mut self,
        entries: &mut [Gfx950EngineeringPeerGuardedMlpBankEntryV1<'_>],
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerScopedBankRearmObservationV1> {
        // SAFETY: the caller supplies the same permanent-retirement premise.
        unsafe { retained::rearm_bank_scoped(self, entries, timeout_ms) }
    }

    /// A separately named temporal policy: full entry/exit, participant-local
    /// checks and fresh root/generation brackets inside one warm layer. Changes
    /// that revert without generation advancement can escape; not equivalent to
    /// full topology rediscovery at every internal boundary. No policy persists.
    ///
    /// # Safety
    /// All retained exact-own-residual/reusable-bind and mixed-bank rearm
    /// obligations apply. The caller must associate these two genuine PrefixV6
    /// owners, retained pair, kernel roles, model/cache pages and current bank.
    /// No generation-binding capability is fabricated by this method. All code
    /// and mappings remain owned by this exclusive disposable-process Group.
    /// This call submits Prefix then guarded MLP and reads both hidden outputs;
    /// no other producer/consumer or host access may interleave. Initial Fresh
    /// uses are refused, and any error/unwind permanently quarantines all owners.
    pub unsafe fn dispatch_warm_layer_scoped_currentness_unchecked_v1(
        &mut self,
        prefixes: [&mut Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2],
        pair: &mut Gfx950EngineeringPeerRetainedGuardedMlpPairV1,
        prefix_inputs: [Gfx950EngineeringPeerScopedPrefixInputsV1<'_>; 2],
        mlp_inputs: &Gfx950EngineeringPeerGuardedMlpInputsV1<'_>,
        timeout_ms: u32,
    ) -> Result<Gfx950EngineeringPeerScopedWarmLayerObservationV1> {
        retained::scoped_layer::run(
            self,
            prefixes,
            pair,
            prefix_inputs,
            mlp_inputs.private_inputs(),
            timeout_ms,
        )
    }
}
