//! Fail-closed completed routing-to-expert join for exact MoE T8/E4/K2/C4 V2.
//!
//! V1 remains the caller-observed, denial-only compatibility surface. V2
//! defines the production-shaped completion/readback and request/batch
//! contracts, but deliberately provides no production or feature-gated issuer.
//! Consequently safe production code cannot construct a completed V2 bridge.

use crate::{
    CheckedMoeHostObservedRoutingOutputV1, MoeExpertCompactPackPlanV1, MoeRoutingOutputCandidateV1,
    MoeRoutingOutputConsistencyErrorV1, check_host_observed_moe_routing_output_v1,
};
use fe2o3_core::{
    BorrowedDeviceOperation, ContextIdentity, DeviceBuffer, DeviceBufferIdentity,
    DeviceBufferRangeError, DeviceBufferView, PinnedHostBuffer, Stream, StreamIdentity,
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    ops::Range,
};

pub(crate) const TOKENS: usize = 8;
pub(crate) const EXPERTS: usize = 4;
pub(crate) const TOP_K: usize = 2;
pub(crate) const ROUTES: usize = TOKENS * TOP_K;
pub(crate) const EXPERT_OFFSETS: usize = EXPERTS + 1;
pub(crate) const TILE_ELEMENTS: usize = 256;
pub(crate) const OUTPUT_WIDTH: usize = 16;
pub(crate) const EXPERT_OUTPUT_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;
pub(crate) const COMPACT_OUTPUT_ELEMENTS: usize = ROUTES * OUTPUT_WIDTH;
pub(crate) const COMBINED_OUTPUT_ELEMENTS: usize = TOKENS * OUTPUT_WIDTH;
const TOKEN_ACTIVATION_ELEMENTS: usize = TOKENS * 16;
const PACKED_ACTIVATION_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;
const COMPLETE_ROUTING_OBSERVATION_MASK: u16 = 0x7f;
const EXACT_PROFILE: &[u8] = b"T8/E4/K2/C4/I16/O16/gfx942:xnack-/wave64/COV6";
const BATCH_DOMAIN: &[u8] = b"FE2O3/MOE/ROUTING-EXPERT/REQUEST-BATCH/V2\0";
const LIFECYCLE_DOMAIN: &[u8] = b"FE2O3/MOE/ROUTING-EXPERT/COMPLETION-READBACK/V2\0";
const ACTIVATIONS_DOMAIN: &[u8] = b"FE2O3/MOE/TOKEN-ACTIVATIONS/BF16/V2\0";
const ROUTE_WEIGHT_POLICY_DOMAIN: &[u8] = b"FE2O3/MOE/ROUTE-WEIGHT-POLICY/F32/V2\0";
const EXPERT_INPUT_DOMAIN: &[u8] = b"FE2O3/MOE/ROUTING-EXPERT/INPUT-JOIN/V2\0";
const WEIGHT_BINDING_DOMAIN: &[u8] = b"FE2O3/MOE/EXPERT-WEIGHT-ARTIFACT-BINDING/V2\0";

/// Untrusted complete routing output for the exact V2 profile.
///
/// V2 owns this data shape so the V1 module remains completely isolated. The
/// candidate is converted through the unchanged public V1 constructor and
/// checker before it can enter any completed V2 witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingOutputCandidateV2 {
    top2_experts: [u32; ROUTES],
    requested_counts: [u32; EXPERTS],
    admitted_counts: [u32; EXPERTS],
    expert_offsets: [u32; EXPERT_OFFSETS],
    route_slots: [u32; ROUTES],
    permutation: [u32; ROUTES],
    inverse: [u32; ROUTES],
}

impl MoeRoutingOutputCandidateV2 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        top2_experts: [u32; ROUTES],
        requested_counts: [u32; EXPERTS],
        admitted_counts: [u32; EXPERTS],
        expert_offsets: [u32; EXPERT_OFFSETS],
        route_slots: [u32; ROUTES],
        permutation: [u32; ROUTES],
        inverse: [u32; ROUTES],
    ) -> Self {
        Self {
            top2_experts,
            requested_counts,
            admitted_counts,
            expert_offsets,
            route_slots,
            permutation,
            inverse,
        }
    }

    const fn as_v1(self) -> MoeRoutingOutputCandidateV1 {
        MoeRoutingOutputCandidateV1::new(
            self.top2_experts,
            self.requested_counts,
            self.admitted_counts,
            self.expert_offsets,
            self.route_slots,
            self.permutation,
            self.inverse,
        )
    }
}

fn put_field(digest: &mut Sha256, name: &[u8], value: &[u8]) {
    digest.update((name.len() as u64).to_le_bytes());
    digest.update(name);
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

fn is_nonzero(value: &[u8]) -> bool {
    value.iter().any(|byte| *byte != 0)
}

/// Pinned-toolchain encoder used only for process-local typed identities.
///
/// `ContextIdentity` and `StreamIdentity` intentionally hide their internal
/// sequence numbers. Their `Hash` implementations still cover those numbers;
/// this encoder includes them without exposing or converting them to raw
/// runtime handles. This is not a stable or durable serialization contract
/// across Rust compiler or library versions.
struct PinnedIdentityTranscriptHasher<'a> {
    digest: &'a mut Sha256,
}

impl Hasher for PinnedIdentityTranscriptHasher<'_> {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.digest.update([0]);
        self.digest.update((bytes.len() as u64).to_le_bytes());
        self.digest.update(bytes);
    }

    fn write_u8(&mut self, value: u8) {
        self.digest.update([1, value]);
    }

    fn write_u16(&mut self, value: u16) {
        self.digest.update([2]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.digest.update([3]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.digest.update([4]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.digest.update([5]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.digest.update([6]);
        self.digest.update((value as u64).to_le_bytes());
    }

    fn write_i8(&mut self, value: i8) {
        self.digest.update([7, value as u8]);
    }

    fn write_i16(&mut self, value: i16) {
        self.digest.update([8]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.digest.update([9]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_i64(&mut self, value: i64) {
        self.digest.update([10]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_i128(&mut self, value: i128) {
        self.digest.update([11]);
        self.digest.update(value.to_le_bytes());
    }

    fn write_isize(&mut self, value: isize) {
        self.digest.update([12]);
        self.digest.update((value as i64).to_le_bytes());
    }
}

fn put_typed_identity<T: Hash>(digest: &mut Sha256, name: &[u8], value: &T) {
    digest.update((name.len() as u64).to_le_bytes());
    digest.update(name);
    let mut encoder = PinnedIdentityTranscriptHasher { digest };
    value.hash(&mut encoder);
}

fn weight_binding_sha256<I: Hash>(
    request_batch_transcript_sha256: [u8; 32],
    model_expert_weight_artifact_identity: [u8; 32],
    allocation_identity: &I,
    region: Range<usize>,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(WEIGHT_BINDING_DOMAIN);
    put_field(
        &mut digest,
        b"request-batch-transcript",
        &request_batch_transcript_sha256,
    );
    put_field(
        &mut digest,
        b"model-expert-weight-artifact",
        &model_expert_weight_artifact_identity,
    );
    put_typed_identity(
        &mut digest,
        b"weight-allocation-identity",
        allocation_identity,
    );
    put_field(
        &mut digest,
        b"weight-region-start",
        &region.start.to_le_bytes(),
    );
    put_field(&mut digest, b"weight-region-end", &region.end.to_le_bytes());
    digest.finalize().into()
}

/// Opaque identity for one routing request and expert batch.
///
/// The identity covers the routing request, logits source, token activations,
/// caller route-weight policy, and the model/expert-weight artifact. There is
/// no public constructor because no current production component can attest
/// all five sources truthfully. This is a required capability, not semantic
/// proof that routing or expert computation is correct.
#[must_use = "the request/batch identity must remain joined to its lifecycle"]
pub struct MoeRoutingExpertBatchIdentityV2 {
    routing_request_identity: [u8; 32],
    routing_logits_identity: [u8; 32],
    token_activations_identity: [u8; 32],
    route_weight_policy_identity: [u8; 32],
    model_expert_weight_artifact_identity: [u8; 32],
    transcript_sha256: [u8; 32],
}

impl fmt::Debug for MoeRoutingExpertBatchIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeRoutingExpertBatchIdentityV2")
            .field("profile", &"T8/E4/K2/C4/I16/O16")
            .field("production_issuer", &"absent")
            .finish_non_exhaustive()
    }
}

fn batch_transcript_sha256(
    routing_request_identity: [u8; 32],
    routing_logits_identity: [u8; 32],
    token_activations_identity: [u8; 32],
    route_weight_policy_identity: [u8; 32],
    model_expert_weight_artifact_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(BATCH_DOMAIN);
    for (name, value) in [
        (b"routing-request".as_slice(), routing_request_identity),
        (b"routing-logits".as_slice(), routing_logits_identity),
        (b"token-activations".as_slice(), token_activations_identity),
        (
            b"route-weight-policy".as_slice(),
            route_weight_policy_identity,
        ),
        (
            b"model-expert-weight-artifact".as_slice(),
            model_expert_weight_artifact_identity,
        ),
    ] {
        put_field(&mut digest, name, &value);
    }
    digest.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MoeRoutingDispatchIdentityV2([u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MoeRoutingCompletionEventIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MoeRoutingReadbackIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MoeRoutingReadbackEventIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MoeRoutingCompletionReadbackOrderIdentityV2([u8; 32]);

struct LifecycleFactsV2<C, S> {
    batch_transcript_sha256: [u8; 32],
    dispatch_context: C,
    dispatch_stream: S,
    readback_context: C,
    readback_stream: S,
    dispatch_identity: MoeRoutingDispatchIdentityV2,
    completion_event_identity: MoeRoutingCompletionEventIdentityV2,
    readback_identity: MoeRoutingReadbackIdentityV2,
    readback_event_identity: MoeRoutingReadbackEventIdentityV2,
    completion_readback_order_identity: MoeRoutingCompletionReadbackOrderIdentityV2,
    profile_sha256: [u8; 32],
    observed_fields: u16,
    payload_sha256: [u8; 32],
    transcript_sha256: [u8; 32],
}

impl<C: Copy, S: Copy> Copy for LifecycleFactsV2<C, S> {}

impl<C: Copy, S: Copy> Clone for LifecycleFactsV2<C, S> {
    fn clone(&self) -> Self {
        *self
    }
}

fn lifecycle_transcript_sha256<C: Hash, S: Hash>(facts: &LifecycleFactsV2<C, S>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(LIFECYCLE_DOMAIN);
    put_field(
        &mut digest,
        b"request-batch-transcript",
        &facts.batch_transcript_sha256,
    );
    put_typed_identity(
        &mut digest,
        b"dispatch-context-identity",
        &facts.dispatch_context,
    );
    put_typed_identity(
        &mut digest,
        b"dispatch-stream-identity",
        &facts.dispatch_stream,
    );
    put_typed_identity(
        &mut digest,
        b"readback-context-identity",
        &facts.readback_context,
    );
    put_typed_identity(
        &mut digest,
        b"readback-stream-identity",
        &facts.readback_stream,
    );
    for (name, value) in [
        (
            b"dispatch-identity".as_slice(),
            facts.dispatch_identity.0.as_slice(),
        ),
        (
            b"completion-event-identity".as_slice(),
            facts.completion_event_identity.0.as_slice(),
        ),
        (
            b"readback-identity".as_slice(),
            facts.readback_identity.0.as_slice(),
        ),
        (
            b"readback-event-identity".as_slice(),
            facts.readback_event_identity.0.as_slice(),
        ),
        (
            b"completion-readback-order-identity".as_slice(),
            facts.completion_readback_order_identity.0.as_slice(),
        ),
        (b"profile".as_slice(), facts.profile_sha256.as_slice()),
        (
            b"observed-fields".as_slice(),
            facts.observed_fields.to_le_bytes().as_slice(),
        ),
        (
            b"routing-payload".as_slice(),
            facts.payload_sha256.as_slice(),
        ),
    ] {
        put_field(&mut digest, name, value);
    }
    digest.finalize().into()
}

/// Production-shaped provenance for one exact dispatch, completion, and full
/// routing readback.
///
/// All context, stream, event, ordering, batch, profile, and payload identities
/// are committed in one versioned, process-local transcript and are also
/// checked by exact typed equality. The typed identity encoding is pinned to
/// this Rust toolchain; it is not durable serialization across toolchains.
/// There is no public constructor, no feature-gated issuer, and no conversion
/// from synthetic evidence.
#[must_use = "completed routing provenance must be consumed exactly once"]
pub struct MoeRoutingCompletionReadbackProvenanceV2 {
    batch: MoeRoutingExpertBatchIdentityV2,
    dispatch_context: ContextIdentity,
    dispatch_stream: StreamIdentity,
    readback_context: ContextIdentity,
    readback_stream: StreamIdentity,
    dispatch_identity: MoeRoutingDispatchIdentityV2,
    completion_event_identity: MoeRoutingCompletionEventIdentityV2,
    readback_identity: MoeRoutingReadbackIdentityV2,
    readback_event_identity: MoeRoutingReadbackEventIdentityV2,
    completion_readback_order_identity: MoeRoutingCompletionReadbackOrderIdentityV2,
    profile_sha256: [u8; 32],
    observed_fields: u16,
    payload_sha256: [u8; 32],
    transcript_sha256: [u8; 32],
}

impl fmt::Debug for MoeRoutingCompletionReadbackProvenanceV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeRoutingCompletionReadbackProvenanceV2")
            .field("profile", &"T8/E4/K2/C4/I16/O16/gfx942:xnack-")
            .field("production_issuer", &"absent")
            .finish_non_exhaustive()
    }
}

impl MoeRoutingCompletionReadbackProvenanceV2 {
    fn facts(&self) -> LifecycleFactsV2<ContextIdentity, StreamIdentity> {
        LifecycleFactsV2 {
            batch_transcript_sha256: self.batch.transcript_sha256,
            dispatch_context: self.dispatch_context,
            dispatch_stream: self.dispatch_stream,
            readback_context: self.readback_context,
            readback_stream: self.readback_stream,
            dispatch_identity: self.dispatch_identity,
            completion_event_identity: self.completion_event_identity,
            readback_identity: self.readback_identity,
            readback_event_identity: self.readback_event_identity,
            completion_readback_order_identity: self.completion_readback_order_identity,
            profile_sha256: self.profile_sha256,
            observed_fields: self.observed_fields,
            payload_sha256: self.payload_sha256,
            transcript_sha256: self.transcript_sha256,
        }
    }
}

/// Exact reason production-shaped provenance was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MoeRoutingCompletionReadbackErrorV2 {
    RequestBatchIdentity,
    CrossContext,
    CrossStream,
    DispatchIdentity,
    CompletionEventIdentity,
    ReadbackIdentity,
    ReadbackEventIdentity,
    CompletionReadbackOrderIdentity,
    Profile,
    IncompleteObservation { observed: u16 },
    PayloadMismatch,
    TranscriptMismatch,
    Routing(MoeRoutingOutputConsistencyErrorV1),
}

impl fmt::Display for MoeRoutingCompletionReadbackErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "completed MoE V2 readback rejected: {self:?}")
    }
}

impl Error for MoeRoutingCompletionReadbackErrorV2 {}

fn validate_batch_identity(
    batch: &MoeRoutingExpertBatchIdentityV2,
) -> Result<(), MoeRoutingCompletionReadbackErrorV2> {
    let identities = [
        batch.routing_request_identity,
        batch.routing_logits_identity,
        batch.token_activations_identity,
        batch.route_weight_policy_identity,
        batch.model_expert_weight_artifact_identity,
    ];
    let expected = batch_transcript_sha256(
        identities[0],
        identities[1],
        identities[2],
        identities[3],
        identities[4],
    );
    if identities.iter().any(|identity| !is_nonzero(identity))
        || batch.transcript_sha256 != expected
    {
        return Err(MoeRoutingCompletionReadbackErrorV2::RequestBatchIdentity);
    }
    Ok(())
}

fn validate_lifecycle_facts<C: Copy + Eq + Hash, S: Copy + Eq + Hash>(
    facts: &LifecycleFactsV2<C, S>,
    expected_batch_transcript: [u8; 32],
    expected_payload: [u8; 32],
) -> Result<(), MoeRoutingCompletionReadbackErrorV2> {
    if !is_nonzero(&facts.batch_transcript_sha256)
        || facts.batch_transcript_sha256 != expected_batch_transcript
    {
        return Err(MoeRoutingCompletionReadbackErrorV2::RequestBatchIdentity);
    }
    if facts.dispatch_context != facts.readback_context {
        return Err(MoeRoutingCompletionReadbackErrorV2::CrossContext);
    }
    if facts.dispatch_stream != facts.readback_stream {
        return Err(MoeRoutingCompletionReadbackErrorV2::CrossStream);
    }
    for (valid, error) in [
        (
            is_nonzero(&facts.dispatch_identity.0),
            MoeRoutingCompletionReadbackErrorV2::DispatchIdentity,
        ),
        (
            is_nonzero(&facts.completion_event_identity.0),
            MoeRoutingCompletionReadbackErrorV2::CompletionEventIdentity,
        ),
        (
            is_nonzero(&facts.readback_identity.0),
            MoeRoutingCompletionReadbackErrorV2::ReadbackIdentity,
        ),
        (
            is_nonzero(&facts.readback_event_identity.0),
            MoeRoutingCompletionReadbackErrorV2::ReadbackEventIdentity,
        ),
        (
            is_nonzero(&facts.completion_readback_order_identity.0),
            MoeRoutingCompletionReadbackErrorV2::CompletionReadbackOrderIdentity,
        ),
        (
            facts.profile_sha256 == Sha256::digest(EXACT_PROFILE).as_slice(),
            MoeRoutingCompletionReadbackErrorV2::Profile,
        ),
    ] {
        if !valid {
            return Err(error);
        }
    }
    if facts.observed_fields != COMPLETE_ROUTING_OBSERVATION_MASK {
        return Err(MoeRoutingCompletionReadbackErrorV2::IncompleteObservation {
            observed: facts.observed_fields,
        });
    }
    if facts.payload_sha256 != expected_payload {
        return Err(MoeRoutingCompletionReadbackErrorV2::PayloadMismatch);
    }
    if lifecycle_transcript_sha256(facts) != facts.transcript_sha256 {
        return Err(MoeRoutingCompletionReadbackErrorV2::TranscriptMismatch);
    }
    Ok(())
}

/// Opaque completed readback joined to one request/batch and runtime transcript.
#[must_use = "the completed readback must be joined to exact expert inputs"]
pub struct CheckedMoeCompletedRoutingReadbackV2 {
    checked: CheckedMoeHostObservedRoutingOutputV1,
    routing: MoeRoutingOutputCandidateV2,
    batch: MoeRoutingExpertBatchIdentityV2,
    dispatch_context: ContextIdentity,
    dispatch_stream: StreamIdentity,
    lifecycle_transcript_sha256: [u8; 32],
}

impl fmt::Debug for CheckedMoeCompletedRoutingReadbackV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedMoeCompletedRoutingReadbackV2")
            .field("expert_offsets", &self.checked.expert_offsets())
            .field("semantic_proof", &false)
            .finish_non_exhaustive()
    }
}

impl CheckedMoeCompletedRoutingReadbackV2 {
    pub const fn proves_routing_semantics(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }
}

/// Consumes one production provenance capability and the exact complete payload.
///
/// No current production caller can reach the success path because provenance
/// issuance is intentionally absent.
pub fn check_completed_moe_routing_readback_v2(
    provenance: MoeRoutingCompletionReadbackProvenanceV2,
    candidate: MoeRoutingOutputCandidateV2,
) -> Result<CheckedMoeCompletedRoutingReadbackV2, MoeRoutingCompletionReadbackErrorV2> {
    validate_batch_identity(&provenance.batch)?;
    let checked = check_host_observed_moe_routing_output_v1(candidate.as_v1())
        .map_err(MoeRoutingCompletionReadbackErrorV2::Routing)?;
    let facts = provenance.facts();
    validate_lifecycle_facts(
        &facts,
        provenance.batch.transcript_sha256,
        checked.payload_sha256(),
    )?;
    Ok(CheckedMoeCompletedRoutingReadbackV2 {
        checked,
        routing: candidate,
        batch: provenance.batch,
        dispatch_context: provenance.dispatch_context,
        dispatch_stream: provenance.dispatch_stream,
        lifecycle_transcript_sha256: provenance.transcript_sha256,
    })
}

/// Untrusted exact-shape inputs for the bounded V2 expert join.
///
/// Route weights remain caller policy inputs. V2 checks their concrete digest
/// against the sealed request/batch identity; it does not recast them as router
/// outputs or prove that the policy is semantically correct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoeExpertInputCandidateV2 {
    route_weights: [f32; ROUTES],
    token_activations: [u16; TOKEN_ACTIVATION_ELEMENTS],
    packed_activation_tiles: [u16; PACKED_ACTIVATION_ELEMENTS],
}

impl MoeExpertInputCandidateV2 {
    pub const fn new(
        route_weights: [f32; ROUTES],
        token_activations: [u16; TOKEN_ACTIVATION_ELEMENTS],
        packed_activation_tiles: [u16; PACKED_ACTIVATION_ELEMENTS],
    ) -> Self {
        Self {
            route_weights,
            token_activations,
            packed_activation_tiles,
        }
    }
}

fn token_activations_identity(values: &[u16; TOKEN_ACTIVATION_ELEMENTS]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ACTIVATIONS_DOMAIN);
    digest.update((values.len() as u64).to_le_bytes());
    for value in values {
        digest.update(value.to_le_bytes());
    }
    digest.finalize().into()
}

fn route_weight_policy_identity(values: &[f32; ROUTES]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ROUTE_WEIGHT_POLICY_DOMAIN);
    digest.update((values.len() as u64).to_le_bytes());
    for value in values {
        digest.update(value.to_bits().to_le_bytes());
    }
    digest.finalize().into()
}

/// Exact reason caller inputs could not join the sealed V2 request/batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MoeExpertInputJoinErrorV2 {
    RouteWeight {
        token: usize,
        rank: usize,
    },
    RouteWeightPair {
        token: usize,
    },
    TokenActivation {
        index: usize,
        bits: u16,
    },
    TokenActivationIdentity,
    RouteWeightPolicyIdentity,
    PackedActivation {
        index: usize,
        expected: u16,
        actual: u16,
    },
}

impl fmt::Display for MoeExpertInputJoinErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MoE V2 expert input join rejected: {self:?}")
    }
}

impl Error for MoeExpertInputJoinErrorV2 {}

/// Move-only completed readback joined to the exact activation and route-policy
/// identities from the shared request/batch capability.
#[must_use = "checked V2 routing expert inputs must be uploaded together"]
pub struct CheckedMoeCompletedRoutingExpertInputsV2 {
    readback: CheckedMoeCompletedRoutingReadbackV2,
    route_weights: [f32; ROUTES],
    packed_activation_tiles: [u16; PACKED_ACTIVATION_ELEMENTS],
    input_transcript_sha256: [u8; 32],
}

impl fmt::Debug for CheckedMoeCompletedRoutingExpertInputsV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedMoeCompletedRoutingExpertInputsV2")
            .field("profile", &"T8/E4/K2/C4/I16/O16")
            .field("input_transcript_sha256", &self.input_transcript_sha256)
            .field("semantic_proof", &false)
            .finish_non_exhaustive()
    }
}

impl CheckedMoeCompletedRoutingExpertInputsV2 {
    pub const fn route_weights_are_caller_policy_inputs(&self) -> bool {
        true
    }

    pub const fn packed_activation_layout_is_exact(&self) -> bool {
        true
    }

    pub const fn proves_expert_execution(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }
}

fn validate_and_pack_expert_inputs(
    checked: &CheckedMoeHostObservedRoutingOutputV1,
    routing: &MoeRoutingOutputCandidateV2,
    expected_token_activations_identity: [u8; 32],
    expected_route_weight_policy_identity: [u8; 32],
    candidate: &MoeExpertInputCandidateV2,
) -> Result<(), MoeExpertInputJoinErrorV2> {
    for token in 0..TOKENS {
        let first = candidate.route_weights[token * TOP_K];
        let second = candidate.route_weights[token * TOP_K + 1];
        for (rank, value) in [first, second].into_iter().enumerate() {
            if !value.is_finite() || value < 0.0 {
                return Err(MoeExpertInputJoinErrorV2::RouteWeight { token, rank });
            }
        }
        if first + second != 1.0 {
            return Err(MoeExpertInputJoinErrorV2::RouteWeightPair { token });
        }
    }
    for (index, bits) in candidate.token_activations.iter().copied().enumerate() {
        if bits & 0x7f80 == 0x7f80 {
            return Err(MoeExpertInputJoinErrorV2::TokenActivation { index, bits });
        }
    }
    if token_activations_identity(&candidate.token_activations)
        != expected_token_activations_identity
    {
        return Err(MoeExpertInputJoinErrorV2::TokenActivationIdentity);
    }
    if route_weight_policy_identity(&candidate.route_weights)
        != expected_route_weight_policy_identity
    {
        return Err(MoeExpertInputJoinErrorV2::RouteWeightPolicyIdentity);
    }

    let mut expected = [0_u16; PACKED_ACTIVATION_ELEMENTS];
    let expert_offsets = checked.expert_offsets();
    let permutation = checked.permutation();
    let accepted = expert_offsets[EXPERTS] as usize;
    for (slot, route) in permutation.iter().copied().enumerate().take(accepted) {
        let route = route as usize;
        let expert = routing.top2_experts[route] as usize;
        let expert_row = slot - expert_offsets[expert] as usize;
        let token = route / TOP_K;
        let source = token * 16;
        let destination = expert * 256 + expert_row * 16;
        expected[destination..destination + 16]
            .copy_from_slice(&candidate.token_activations[source..source + 16]);
    }
    for (index, (&expected, &actual)) in expected
        .iter()
        .zip(&candidate.packed_activation_tiles)
        .enumerate()
    {
        if actual != expected {
            return Err(MoeExpertInputJoinErrorV2::PackedActivation {
                index,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

/// Joins exact caller inputs to one completed V2 readback and request identity.
pub fn bind_completed_moe_routing_expert_inputs_v2(
    readback: CheckedMoeCompletedRoutingReadbackV2,
    candidate: MoeExpertInputCandidateV2,
) -> Result<CheckedMoeCompletedRoutingExpertInputsV2, MoeExpertInputJoinErrorV2> {
    validate_and_pack_expert_inputs(
        &readback.checked,
        &readback.routing,
        readback.batch.token_activations_identity,
        readback.batch.route_weight_policy_identity,
        &candidate,
    )?;
    let mut digest = Sha256::new();
    digest.update(EXPERT_INPUT_DOMAIN);
    put_field(
        &mut digest,
        b"lifecycle-transcript",
        &readback.lifecycle_transcript_sha256,
    );
    put_field(
        &mut digest,
        b"request-batch-transcript",
        &readback.batch.transcript_sha256,
    );
    put_field(
        &mut digest,
        b"token-activations",
        &readback.batch.token_activations_identity,
    );
    put_field(
        &mut digest,
        b"route-weight-policy",
        &readback.batch.route_weight_policy_identity,
    );
    Ok(CheckedMoeCompletedRoutingExpertInputsV2 {
        readback,
        route_weights: candidate.route_weights,
        packed_activation_tiles: candidate.packed_activation_tiles,
        input_transcript_sha256: digest.finalize().into(),
    })
}

/// Required binding between an exact expert-weight device region and the model
/// artifact named by the shared V2 request/batch.
///
/// There is no public constructor. The current artifact pipeline cannot yet
/// issue this capability, so V2 expert preparation fails closed instead of
/// accepting an unrelated raw weight view after routing.
#[must_use = "the expert-weight artifact binding must enter V2 preparation"]
pub struct MoeExpertWeightArtifactBindingV2<'weights> {
    weight_view: DeviceBufferView<'weights, u16>,
    request_batch_transcript_sha256: [u8; 32],
    model_expert_weight_artifact_identity: [u8; 32],
    binding_sha256: [u8; 32],
}

impl fmt::Debug for MoeExpertWeightArtifactBindingV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeExpertWeightArtifactBindingV2")
            .field("production_issuer", &"absent")
            .finish_non_exhaustive()
    }
}

impl MoeExpertWeightArtifactBindingV2<'_> {
    pub(crate) const fn weight_view(&self) -> &DeviceBufferView<'_, u16> {
        &self.weight_view
    }

    pub(crate) const fn request_batch_transcript_sha256(&self) -> [u8; 32] {
        self.request_batch_transcript_sha256
    }

    pub(crate) const fn model_expert_weight_artifact_identity(&self) -> [u8; 32] {
        self.model_expert_weight_artifact_identity
    }

    pub(crate) fn binding_matches_transcript(&self) -> bool {
        self.binding_sha256
            == weight_binding_sha256(
                self.request_batch_transcript_sha256,
                self.model_expert_weight_artifact_identity,
                &self.weight_view.allocation_identity(),
                self.weight_view.region_byte_range(),
            )
    }
}

/// Exact destination role in the completed V2 upload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeCompletedRoutingExpertUploadRoleV2 {
    PackedActivationTiles,
    ExpertOffsets,
    InverseRouting,
    RouteWeights,
}

/// Retained four-region upload from one completed V2 join.
#[must_use = "the V2 bridge retains every uploaded expert-input region"]
pub struct MoeCompletedRoutingExpertBridgeV2<'activations, 'offsets, 'inverse, 'route_weights> {
    checked: CheckedMoeCompletedRoutingExpertInputsV2,
    activation_tiles_view: DeviceBufferView<'activations, u16>,
    offsets_view: DeviceBufferView<'offsets, u32>,
    inverse_view: DeviceBufferView<'inverse, u32>,
    route_weights_view: DeviceBufferView<'route_weights, f32>,
    context_identity: ContextIdentity,
    stream_identity: StreamIdentity,
    allocation_identities: [DeviceBufferIdentity; 4],
    region_byte_ranges: [Range<usize>; 4],
}

impl fmt::Debug for MoeCompletedRoutingExpertBridgeV2<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeCompletedRoutingExpertBridgeV2")
            .field("context_identity", &self.context_identity)
            .field("stream_identity", &self.stream_identity)
            .field("allocation_identities", &self.allocation_identities)
            .field("region_byte_ranges", &self.region_byte_ranges)
            .finish_non_exhaustive()
    }
}

impl MoeCompletedRoutingExpertBridgeV2<'_, '_, '_, '_> {
    pub const fn compact_pack_plan(&self) -> MoeExpertCompactPackPlanV1 {
        self.checked.readback.checked.compact_pack_plan()
    }

    pub const fn request_batch_transcript_sha256(&self) -> [u8; 32] {
        self.checked.readback.batch.transcript_sha256
    }

    pub const fn model_expert_weight_artifact_identity(&self) -> [u8; 32] {
        self.checked
            .readback
            .batch
            .model_expert_weight_artifact_identity
    }

    pub const fn proves_routing_or_expert_semantics(&self) -> bool {
        false
    }

    pub const fn grants_copy_authority(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_finalizer_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }

    pub(crate) const fn activation_tiles_view(&self) -> &DeviceBufferView<'_, u16> {
        &self.activation_tiles_view
    }

    pub(crate) const fn offsets_view(&self) -> &DeviceBufferView<'_, u32> {
        &self.offsets_view
    }

    pub(crate) const fn inverse_view(&self) -> &DeviceBufferView<'_, u32> {
        &self.inverse_view
    }

    pub(crate) const fn route_weights_view(&self) -> &DeviceBufferView<'_, f32> {
        &self.route_weights_view
    }
}

fn validate_completed_upload_facts<I: Copy + Eq, C: Copy + Eq, S: Copy + Eq>(
    lengths: [usize; 4],
    destination_contexts: [C; 4],
    stream_context: C,
    allocation_identities: [I; 4],
    actual_stream: S,
    provenance_context: C,
    provenance_stream: S,
) -> Result<(), MoeCompletedRoutingExpertUploadErrorV2> {
    let expected_lengths = [PACKED_ACTIVATION_ELEMENTS, EXPERT_OFFSETS, ROUTES, ROUTES];
    let roles = [
        MoeCompletedRoutingExpertUploadRoleV2::PackedActivationTiles,
        MoeCompletedRoutingExpertUploadRoleV2::ExpertOffsets,
        MoeCompletedRoutingExpertUploadRoleV2::InverseRouting,
        MoeCompletedRoutingExpertUploadRoleV2::RouteWeights,
    ];
    for ((role, actual), expected) in roles.into_iter().zip(lengths).zip(expected_lengths) {
        if actual != expected {
            return Err(MoeCompletedRoutingExpertUploadErrorV2::Length {
                role,
                expected,
                actual,
            });
        }
    }
    for (role, actual) in roles.into_iter().zip(destination_contexts) {
        if actual != stream_context {
            return Err(MoeCompletedRoutingExpertUploadErrorV2::Context { role });
        }
    }
    if stream_context != provenance_context {
        return Err(MoeCompletedRoutingExpertUploadErrorV2::ProvenanceContext);
    }
    if actual_stream != provenance_stream {
        return Err(MoeCompletedRoutingExpertUploadErrorV2::ProvenanceStream);
    }
    for left in 0..allocation_identities.len() {
        for right in left + 1..allocation_identities.len() {
            if allocation_identities[left] == allocation_identities[right] {
                return Err(
                    MoeCompletedRoutingExpertUploadErrorV2::AliasedDestinations {
                        left: roles[left],
                        right: roles[right],
                    },
                );
            }
        }
    }
    Ok(())
}

/// Uploads all completed V2 expert inputs on the exact dispatch/readback stream.
///
/// This function is publicly callable but constructively unreachable in safe
/// production code because its checked input has no production issuer.
pub fn upload_completed_moe_routing_expert_bridge_v2<
    'activations,
    'offsets,
    'inverse,
    'route_weights,
>(
    stream: &Stream,
    activation_tiles_destination: &'activations mut DeviceBuffer<u16>,
    offsets_destination: &'offsets mut DeviceBuffer<u32>,
    inverse_destination: &'inverse mut DeviceBuffer<u32>,
    route_weights_destination: &'route_weights mut DeviceBuffer<f32>,
    checked: CheckedMoeCompletedRoutingExpertInputsV2,
) -> Result<
    MoeCompletedRoutingExpertBridgeV2<'activations, 'offsets, 'inverse, 'route_weights>,
    MoeCompletedRoutingExpertUploadErrorV2,
> {
    let roles = [
        MoeCompletedRoutingExpertUploadRoleV2::PackedActivationTiles,
        MoeCompletedRoutingExpertUploadRoleV2::ExpertOffsets,
        MoeCompletedRoutingExpertUploadRoleV2::InverseRouting,
        MoeCompletedRoutingExpertUploadRoleV2::RouteWeights,
    ];
    let context_identity = stream.context().identity();
    let lengths = [
        activation_tiles_destination.len(),
        offsets_destination.len(),
        inverse_destination.len(),
        route_weights_destination.len(),
    ];
    let contexts = [
        activation_tiles_destination.context().identity(),
        offsets_destination.context().identity(),
        inverse_destination.context().identity(),
        route_weights_destination.context().identity(),
    ];
    let allocation_identities = [
        activation_tiles_destination.allocation_identity(),
        offsets_destination.allocation_identity(),
        inverse_destination.allocation_identity(),
        route_weights_destination.allocation_identity(),
    ];
    validate_completed_upload_facts(
        lengths,
        contexts,
        context_identity,
        allocation_identities,
        stream.identity(),
        checked.readback.dispatch_context,
        checked.readback.dispatch_stream,
    )?;

    let activation_source =
        PinnedHostBuffer::from_slice(stream.context(), &checked.packed_activation_tiles).map_err(
            |error| MoeCompletedRoutingExpertUploadErrorV2::PinnedSource {
                role: roles[0],
                error,
            },
        )?;
    let offsets = checked.readback.checked.expert_offsets();
    let offsets_source =
        PinnedHostBuffer::from_slice(stream.context(), &offsets).map_err(|error| {
            MoeCompletedRoutingExpertUploadErrorV2::PinnedSource {
                role: roles[1],
                error,
            }
        })?;
    let inverse = checked.readback.checked.inverse();
    let inverse_source =
        PinnedHostBuffer::from_slice(stream.context(), &inverse).map_err(|error| {
            MoeCompletedRoutingExpertUploadErrorV2::PinnedSource {
                role: roles[2],
                error,
            }
        })?;
    let route_weights_source =
        PinnedHostBuffer::from_slice(stream.context(), &checked.route_weights).map_err(
            |error| MoeCompletedRoutingExpertUploadErrorV2::PinnedSource {
                role: roles[3],
                error,
            },
        )?;

    BorrowedDeviceOperation::copy_to_device(
        stream,
        &activation_source,
        activation_tiles_destination,
        |_| (),
    )
    .map_err(|error| MoeCompletedRoutingExpertUploadErrorV2::Upload {
        role: roles[0],
        error,
    })?;
    BorrowedDeviceOperation::copy_to_device(stream, &offsets_source, offsets_destination, |_| ())
        .map_err(|error| MoeCompletedRoutingExpertUploadErrorV2::Upload {
        role: roles[1],
        error,
    })?;
    BorrowedDeviceOperation::copy_to_device(stream, &inverse_source, inverse_destination, |_| ())
        .map_err(|error| MoeCompletedRoutingExpertUploadErrorV2::Upload {
        role: roles[2],
        error,
    })?;
    BorrowedDeviceOperation::copy_to_device(
        stream,
        &route_weights_source,
        route_weights_destination,
        |_| (),
    )
    .map_err(|error| MoeCompletedRoutingExpertUploadErrorV2::Upload {
        role: roles[3],
        error,
    })?;

    let activation_tiles_view = activation_tiles_destination.view(..).map_err(|error| {
        MoeCompletedRoutingExpertUploadErrorV2::Region {
            role: roles[0],
            error,
        }
    })?;
    let offsets_view = offsets_destination.view(..).map_err(|error| {
        MoeCompletedRoutingExpertUploadErrorV2::Region {
            role: roles[1],
            error,
        }
    })?;
    let inverse_view = inverse_destination.view(..).map_err(|error| {
        MoeCompletedRoutingExpertUploadErrorV2::Region {
            role: roles[2],
            error,
        }
    })?;
    let route_weights_view = route_weights_destination.view(..).map_err(|error| {
        MoeCompletedRoutingExpertUploadErrorV2::Region {
            role: roles[3],
            error,
        }
    })?;
    let region_byte_ranges = [
        activation_tiles_view.region_byte_range(),
        offsets_view.region_byte_range(),
        inverse_view.region_byte_range(),
        route_weights_view.region_byte_range(),
    ];
    Ok(MoeCompletedRoutingExpertBridgeV2 {
        checked,
        activation_tiles_view,
        offsets_view,
        inverse_view,
        route_weights_view,
        context_identity,
        stream_identity: stream.identity(),
        allocation_identities,
        region_byte_ranges,
    })
}

/// Rejection before a retained V2 upload bridge is issued.
#[derive(Debug)]
#[non_exhaustive]
pub enum MoeCompletedRoutingExpertUploadErrorV2 {
    Length {
        role: MoeCompletedRoutingExpertUploadRoleV2,
        expected: usize,
        actual: usize,
    },
    Context {
        role: MoeCompletedRoutingExpertUploadRoleV2,
    },
    ProvenanceContext,
    ProvenanceStream,
    AliasedDestinations {
        left: MoeCompletedRoutingExpertUploadRoleV2,
        right: MoeCompletedRoutingExpertUploadRoleV2,
    },
    PinnedSource {
        role: MoeCompletedRoutingExpertUploadRoleV2,
        error: fe2o3_core::Error,
    },
    Upload {
        role: MoeCompletedRoutingExpertUploadRoleV2,
        error: fe2o3_core::Error,
    },
    Region {
        role: MoeCompletedRoutingExpertUploadRoleV2,
        error: DeviceBufferRangeError,
    },
}

impl fmt::Display for MoeCompletedRoutingExpertUploadErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "completed MoE V2 upload rejected: {self:?}")
    }
}

impl Error for MoeCompletedRoutingExpertUploadErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PinnedSource { error, .. } | Self::Upload { error, .. } => Some(error),
            Self::Region { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(label: &[u8]) -> [u8; 32] {
        Sha256::digest(label).into()
    }

    fn reference_candidate() -> MoeRoutingOutputCandidateV2 {
        MoeRoutingOutputCandidateV2::new(
            [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
            [8, 8, 0, 0],
            [4, 4, 0, 0],
            [0, 4, 8, 8, 8],
            [
                0,
                4,
                1,
                5,
                2,
                6,
                3,
                7,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
            ],
            [
                0,
                2,
                4,
                6,
                1,
                3,
                5,
                7,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
            ],
            [
                0,
                4,
                1,
                5,
                2,
                6,
                3,
                7,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
            ],
        )
    }

    fn checked_reference() -> (
        MoeRoutingOutputCandidateV2,
        CheckedMoeHostObservedRoutingOutputV1,
    ) {
        let routing = reference_candidate();
        let checked = check_host_observed_moe_routing_output_v1(routing.as_v1()).unwrap();
        (routing, checked)
    }

    fn exact_inputs(
        routing: &MoeRoutingOutputCandidateV2,
        checked: &CheckedMoeHostObservedRoutingOutputV1,
    ) -> MoeExpertInputCandidateV2 {
        let token_activations = std::array::from_fn(|index| (index + 1) as u16);
        let mut packed = [0_u16; PACKED_ACTIVATION_ELEMENTS];
        let offsets = checked.expert_offsets();
        let permutation = checked.permutation();
        for (slot, route) in permutation
            .iter()
            .copied()
            .enumerate()
            .take(offsets[EXPERTS] as usize)
        {
            let route = route as usize;
            let expert = routing.top2_experts[route] as usize;
            let row = slot - offsets[expert] as usize;
            let token = route / TOP_K;
            packed[expert * 256 + row * 16..expert * 256 + row * 16 + 16]
                .copy_from_slice(&token_activations[token * 16..token * 16 + 16]);
        }
        MoeExpertInputCandidateV2::new([0.5; ROUTES], token_activations, packed)
    }

    fn test_only_batch(inputs: &MoeExpertInputCandidateV2) -> MoeRoutingExpertBatchIdentityV2 {
        let routing_request_identity = identity(b"test-only-routing-request");
        let routing_logits_identity = identity(b"test-only-routing-logits");
        let token_activations_identity = token_activations_identity(&inputs.token_activations);
        let route_weight_policy_identity = route_weight_policy_identity(&inputs.route_weights);
        let model_expert_weight_artifact_identity = identity(b"test-only-expert-weights");
        let transcript_sha256 = batch_transcript_sha256(
            routing_request_identity,
            routing_logits_identity,
            token_activations_identity,
            route_weight_policy_identity,
            model_expert_weight_artifact_identity,
        );
        MoeRoutingExpertBatchIdentityV2 {
            routing_request_identity,
            routing_logits_identity,
            token_activations_identity,
            route_weight_policy_identity,
            model_expert_weight_artifact_identity,
            transcript_sha256,
        }
    }

    fn synthetic_facts<C: Copy + Hash, S: Copy + Hash>(
        context: C,
        stream: S,
        batch: [u8; 32],
        payload: [u8; 32],
    ) -> LifecycleFactsV2<C, S> {
        let mut facts = LifecycleFactsV2 {
            batch_transcript_sha256: batch,
            dispatch_context: context,
            dispatch_stream: stream,
            readback_context: context,
            readback_stream: stream,
            dispatch_identity: MoeRoutingDispatchIdentityV2([0xd1; 16]),
            completion_event_identity: MoeRoutingCompletionEventIdentityV2([0xc2; 32]),
            readback_identity: MoeRoutingReadbackIdentityV2([0xb3; 32]),
            readback_event_identity: MoeRoutingReadbackEventIdentityV2([0xa4; 32]),
            completion_readback_order_identity: MoeRoutingCompletionReadbackOrderIdentityV2(
                [0x95; 32],
            ),
            profile_sha256: Sha256::digest(EXACT_PROFILE).into(),
            observed_fields: COMPLETE_ROUTING_OBSERVATION_MASK,
            payload_sha256: payload,
            transcript_sha256: [0; 32],
        };
        facts.transcript_sha256 = lifecycle_transcript_sha256(&facts);
        facts
    }

    #[test]
    fn private_synthetic_facts_cover_the_complete_pinned_toolchain_transcript() {
        let (routing, checked) = checked_reference();
        let inputs = exact_inputs(&routing, &checked);
        let batch = test_only_batch(&inputs);
        assert!(validate_batch_identity(&batch).is_ok());
        let facts = synthetic_facts(
            7_u64,
            11_u64,
            batch.transcript_sha256,
            checked.payload_sha256(),
        );
        assert!(
            validate_lifecycle_facts(&facts, batch.transcript_sha256, checked.payload_sha256())
                .is_ok()
        );

        let mut same_context_drift = facts;
        same_context_drift.dispatch_context = 8;
        same_context_drift.readback_context = 8;
        assert_eq!(
            validate_lifecycle_facts(
                &same_context_drift,
                batch.transcript_sha256,
                checked.payload_sha256()
            ),
            Err(MoeRoutingCompletionReadbackErrorV2::TranscriptMismatch)
        );
        let mut same_stream_drift = facts;
        same_stream_drift.dispatch_stream = 12;
        same_stream_drift.readback_stream = 12;
        assert_eq!(
            validate_lifecycle_facts(
                &same_stream_drift,
                batch.transcript_sha256,
                checked.payload_sha256()
            ),
            Err(MoeRoutingCompletionReadbackErrorV2::TranscriptMismatch)
        );
        let transcript_mutations: &[fn(&mut LifecycleFactsV2<u64, u64>)] = &[
            |value| value.dispatch_identity.0[0] ^= 1,
            |value| value.completion_event_identity.0[0] ^= 1,
            |value| value.readback_identity.0[0] ^= 1,
            |value| value.readback_event_identity.0[0] ^= 1,
            |value| value.completion_readback_order_identity.0[0] ^= 1,
        ];
        for mutate in transcript_mutations {
            let mut mutated = facts;
            mutate(&mut mutated);
            assert_eq!(
                validate_lifecycle_facts(
                    &mutated,
                    batch.transcript_sha256,
                    checked.payload_sha256()
                ),
                Err(MoeRoutingCompletionReadbackErrorV2::TranscriptMismatch)
            );
        }
    }

    #[test]
    fn lifecycle_rejects_cross_domain_partial_payload_and_identity_substitution() {
        let (routing, checked) = checked_reference();
        let inputs = exact_inputs(&routing, &checked);
        let batch = test_only_batch(&inputs);
        let facts = synthetic_facts(
            7_u64,
            11_u64,
            batch.transcript_sha256,
            checked.payload_sha256(),
        );

        let mut cross_context = facts;
        cross_context.readback_context = 8;
        assert_eq!(
            validate_lifecycle_facts(
                &cross_context,
                batch.transcript_sha256,
                checked.payload_sha256()
            ),
            Err(MoeRoutingCompletionReadbackErrorV2::CrossContext)
        );
        let mut cross_stream = facts;
        cross_stream.readback_stream = 12;
        assert_eq!(
            validate_lifecycle_facts(
                &cross_stream,
                batch.transcript_sha256,
                checked.payload_sha256()
            ),
            Err(MoeRoutingCompletionReadbackErrorV2::CrossStream)
        );
        let zero_mutations: &[(
            fn(&mut LifecycleFactsV2<u64, u64>),
            MoeRoutingCompletionReadbackErrorV2,
        )] = &[
            (
                |value| value.dispatch_identity.0 = [0; 16],
                MoeRoutingCompletionReadbackErrorV2::DispatchIdentity,
            ),
            (
                |value| value.completion_event_identity.0 = [0; 32],
                MoeRoutingCompletionReadbackErrorV2::CompletionEventIdentity,
            ),
            (
                |value| value.readback_identity.0 = [0; 32],
                MoeRoutingCompletionReadbackErrorV2::ReadbackIdentity,
            ),
            (
                |value| value.readback_event_identity.0 = [0; 32],
                MoeRoutingCompletionReadbackErrorV2::ReadbackEventIdentity,
            ),
            (
                |value| value.completion_readback_order_identity.0 = [0; 32],
                MoeRoutingCompletionReadbackErrorV2::CompletionReadbackOrderIdentity,
            ),
        ];
        for (mutate, expected) in zero_mutations {
            let mut mutated = facts;
            mutate(&mut mutated);
            assert_eq!(
                validate_lifecycle_facts(
                    &mutated,
                    batch.transcript_sha256,
                    checked.payload_sha256()
                ),
                Err(*expected)
            );
        }
        let mut wrong_profile = facts;
        wrong_profile.profile_sha256[0] ^= 1;
        assert_eq!(
            validate_lifecycle_facts(
                &wrong_profile,
                batch.transcript_sha256,
                checked.payload_sha256()
            ),
            Err(MoeRoutingCompletionReadbackErrorV2::Profile)
        );
        let mut partial = facts;
        partial.observed_fields ^= 1 << 4;
        assert_eq!(
            validate_lifecycle_facts(&partial, batch.transcript_sha256, checked.payload_sha256()),
            Err(MoeRoutingCompletionReadbackErrorV2::IncompleteObservation {
                observed: COMPLETE_ROUTING_OBSERVATION_MASK ^ (1 << 4)
            })
        );
        let mut stale = facts;
        stale.payload_sha256[0] ^= 1;
        assert_eq!(
            validate_lifecycle_facts(&stale, batch.transcript_sha256, checked.payload_sha256()),
            Err(MoeRoutingCompletionReadbackErrorV2::PayloadMismatch)
        );
        assert_eq!(
            validate_lifecycle_facts(&facts, identity(b"other-batch"), checked.payload_sha256()),
            Err(MoeRoutingCompletionReadbackErrorV2::RequestBatchIdentity)
        );
    }

    #[test]
    fn every_routing_array_drift_fails_before_a_completed_readback_can_issue() {
        let candidate = reference_candidate();
        let checked = check_host_observed_moe_routing_output_v1(candidate.as_v1()).unwrap();
        let exact_inputs = exact_inputs(&candidate, &checked);
        let batch = test_only_batch(&exact_inputs);
        let facts = synthetic_facts(
            7_u64,
            11_u64,
            batch.transcript_sha256,
            checked.payload_sha256(),
        );
        let mutations: &[fn(&mut MoeRoutingOutputCandidateV2)] = &[
            |value| value.top2_experts[0] ^= 1,
            |value| value.requested_counts[0] ^= 1,
            |value| value.admitted_counts[0] ^= 1,
            |value| value.expert_offsets[1] ^= 1,
            |value| value.route_slots[0] ^= 1,
            |value| value.permutation[0] ^= 1,
            |value| value.inverse[0] ^= 1,
        ];
        for mutate in mutations {
            let mut drifted = candidate;
            mutate(&mut drifted);
            match check_host_observed_moe_routing_output_v1(drifted.as_v1()) {
                Err(_) => {}
                Ok(drifted) => assert_eq!(
                    validate_lifecycle_facts(
                        &facts,
                        batch.transcript_sha256,
                        drifted.payload_sha256()
                    ),
                    Err(MoeRoutingCompletionReadbackErrorV2::PayloadMismatch)
                ),
            }
        }
    }

    #[test]
    fn request_batch_rejects_every_identity_and_transcript_mutation() {
        let (routing, checked) = checked_reference();
        let inputs = exact_inputs(&routing, &checked);
        let mutations: &[fn(&mut MoeRoutingExpertBatchIdentityV2)] = &[
            |value| value.routing_request_identity[0] ^= 1,
            |value| value.routing_logits_identity[0] ^= 1,
            |value| value.token_activations_identity[0] ^= 1,
            |value| value.route_weight_policy_identity[0] ^= 1,
            |value| value.model_expert_weight_artifact_identity[0] ^= 1,
            |value| value.transcript_sha256[0] ^= 1,
        ];
        for mutate in mutations {
            let mut batch = test_only_batch(&inputs);
            mutate(&mut batch);
            assert_eq!(
                validate_batch_identity(&batch),
                Err(MoeRoutingCompletionReadbackErrorV2::RequestBatchIdentity)
            );
        }
    }

    #[test]
    fn expert_inputs_reject_policy_activation_and_packing_drift() {
        let (routing, checked) = checked_reference();
        let exact = exact_inputs(&routing, &checked);
        let token_identity = token_activations_identity(&exact.token_activations);
        let weight_identity = route_weight_policy_identity(&exact.route_weights);
        assert!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                token_identity,
                weight_identity,
                &exact
            )
            .is_ok()
        );

        let mut invalid_weight = exact;
        invalid_weight.route_weights[0] = f32::NAN;
        assert_eq!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                token_identity,
                weight_identity,
                &invalid_weight
            ),
            Err(MoeExpertInputJoinErrorV2::RouteWeight { token: 0, rank: 0 })
        );
        let mut wrong_pair = exact;
        wrong_pair.route_weights[0] = 0.25;
        assert_eq!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                token_identity,
                weight_identity,
                &wrong_pair
            ),
            Err(MoeExpertInputJoinErrorV2::RouteWeightPair { token: 0 })
        );
        assert_eq!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                token_identity,
                identity(b"wrong-policy"),
                &exact
            ),
            Err(MoeExpertInputJoinErrorV2::RouteWeightPolicyIdentity)
        );
        assert_eq!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                identity(b"wrong-activations"),
                weight_identity,
                &exact
            ),
            Err(MoeExpertInputJoinErrorV2::TokenActivationIdentity)
        );
        let mut invalid_bf16 = exact;
        invalid_bf16.token_activations[17] = 0x7f80;
        assert_eq!(
            validate_and_pack_expert_inputs(
                &checked,
                &routing,
                token_identity,
                weight_identity,
                &invalid_bf16
            ),
            Err(MoeExpertInputJoinErrorV2::TokenActivation {
                index: 17,
                bits: 0x7f80
            })
        );
        for index in [0, 255, PACKED_ACTIVATION_ELEMENTS - 1] {
            let mut drifted = exact;
            drifted.packed_activation_tiles[index] ^= 1;
            assert!(matches!(
                validate_and_pack_expert_inputs(
                    &checked,
                    &routing,
                    token_identity,
                    weight_identity,
                    &drifted
                ),
                Err(MoeExpertInputJoinErrorV2::PackedActivation { index: actual, .. }) if actual == index
            ));
        }
    }

    #[test]
    fn upload_facts_reject_every_length_context_stream_and_alias_substitution() {
        let lengths = [PACKED_ACTIVATION_ELEMENTS, EXPERT_OFFSETS, ROUTES, ROUTES];
        let contexts = [7_u8; 4];
        let identities = [1_u8, 2, 3, 4];
        let validate = |lengths, contexts, identities, context, stream| {
            validate_completed_upload_facts(
                lengths, contexts, 7_u8, identities, 9_u8, context, stream,
            )
        };
        assert!(validate(lengths, contexts, identities, 7, 9).is_ok());
        for role in 0..4 {
            let mut mutated = lengths;
            mutated[role] -= 1;
            assert!(matches!(
                validate(mutated, contexts, identities, 7, 9),
                Err(MoeCompletedRoutingExpertUploadErrorV2::Length { .. })
            ));
            let mut mutated = contexts;
            mutated[role] = 8;
            assert!(matches!(
                validate(lengths, mutated, identities, 7, 9),
                Err(MoeCompletedRoutingExpertUploadErrorV2::Context { .. })
            ));
        }
        assert_eq!(
            validate(lengths, contexts, identities, 8, 9)
                .unwrap_err()
                .to_string(),
            MoeCompletedRoutingExpertUploadErrorV2::ProvenanceContext.to_string()
        );
        assert_eq!(
            validate(lengths, contexts, identities, 7, 8)
                .unwrap_err()
                .to_string(),
            MoeCompletedRoutingExpertUploadErrorV2::ProvenanceStream.to_string()
        );
        for left in 0..4 {
            for right in left + 1..4 {
                let mut mutated = identities;
                mutated[right] = mutated[left];
                assert!(matches!(
                    validate(lengths, contexts, mutated, 7, 9),
                    Err(MoeCompletedRoutingExpertUploadErrorV2::AliasedDestinations { .. })
                ));
            }
        }
    }

    #[test]
    fn weight_binding_transcript_covers_batch_artifact_allocation_and_region() {
        let batch = identity(b"batch");
        let artifact = identity(b"artifact");
        let baseline = weight_binding_sha256(batch, artifact, &17_u64, 32..2_080);
        assert_ne!(
            baseline,
            weight_binding_sha256(identity(b"other"), artifact, &17_u64, 32..2_080)
        );
        assert_ne!(
            baseline,
            weight_binding_sha256(batch, identity(b"other"), &17_u64, 32..2_080)
        );
        assert_ne!(
            baseline,
            weight_binding_sha256(batch, artifact, &18_u64, 32..2_080)
        );
        assert_ne!(
            baseline,
            weight_binding_sha256(batch, artifact, &17_u64, 34..2_080)
        );
        assert_ne!(
            baseline,
            weight_binding_sha256(batch, artifact, &17_u64, 32..2_082)
        );
    }
}
