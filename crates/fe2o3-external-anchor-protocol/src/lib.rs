//! Bounded external anti-rollback anchor protocol foundation.
//!
//! This crate defines canonical fixed-width messages, strict Ed25519 response verification, and a
//! move-only local state machine for one hash-chain transition. A [`PreparedAnchorAdvanceV1`] can
//! issue either an advance challenge or, after modeled crash recovery, an observation challenge.
//! A commit decision is constructible only after a caller-pinned key verifies a signed observation
//! of the exact proposed sequence and head. A signed observation of the exact prior sequence and
//! head deterministically produces an abort decision.
//!
//! `AUTHORITY=none`: this crate does not establish public-key provenance, generate or persist fresh
//! nonces, authenticate a channel, persist local state, implement a monotonic anchor, publish an
//! artifact, or integrate with the broker service. Single-use transition tokens reject stale
//! responses from another nonce or phase and prevent replay through the same safe token. Reusing a
//! caller nonce after process loss cannot be detected without external durable state. The state
//! machine models crash decisions; it does not make local state or publication atomic.
//! Observations other than the exact prior or proposed position fail closed; multi-writer
//! reconciliation and anchor-key rotation are not modeled.
//!
//! Transition tokens are intentionally not cloneable:
//!
//! ```compile_fail
//! use fe2o3_external_anchor_protocol::PreparedAnchorAdvanceV1;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<PreparedAnchorAdvanceV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_external_anchor_protocol::PendingAnchorTransitionV1;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<PendingAnchorTransitionV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_external_anchor_protocol::AnchoredCommitDecisionV1;
//!
//! fn require_copy<T: Copy>() {}
//! require_copy::<AnchoredCommitDecisionV1>();
//! ```

mod protocol;
mod state;

#[cfg(test)]
mod tests;

pub use protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_SIGNED_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
    AnchorChallengeV1, AnchorKeyIdentityV1, AnchorPositionV1, AnchorProtocolErrorV1, CallerNonceV1,
    ChallengeKindV1, EXTERNAL_ANCHOR_AUTHORITY_V1, EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1,
    HashChainHeadV1, PinnedAnchorKeyV1, TRANSACTION_IDENTITY_MAX_LEN_V1, TransactionDigestV1,
    UnsignedAnchorObservationV1, derive_proposed_head_v1, derive_transaction_digest_v1,
};
pub use state::{
    AnchorDecisionV1, AnchoredAbortDecisionV1, AnchoredCommitDecisionV1, AnchoredStateV1,
    PendingAnchorTransitionV1, PreparedAnchorAdvanceV1,
};
