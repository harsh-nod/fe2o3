//! Fixed-size, in-process runtime observations. No serialization or cross-run identity.
//!
//! One context accompanies one existing debug record when its sink opts in. No
//! per-frame vector, source-variable binding, physical resource or completion
//! proof is supplied. The receiving sink owns and must bound any retention.

use crate::{SimulationDebugSiteV1, SimulationInvocationV1};

/// One attempted operation in one activation of an actual interpreter frame.
///
/// Numeric tokens are local to the live simulation and full invocation. The
/// same tokens can recur in another simulation; these are not persisted owners,
/// authenticated source identities, or globally unique debugger handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugOperationOriginV1 {
    invocation: SimulationInvocationV1,
    site: SimulationDebugSiteV1,
    activation: u64,
    attempt: u64,
}

impl SimulationDebugOperationOriginV1 {
    pub(crate) fn from_runtime(value: crate::debug_identity_state::OperationIdentity) -> Self {
        Self {
            invocation: value.frame().invocation(),
            site: value.site(),
            activation: value.frame().activation().get(),
            attempt: value.attempt().get(),
        }
    }

    /// Exact invocation of the accompanying record, not only its global index.
    pub const fn invocation(self) -> SimulationInvocationV1 {
        self.invocation
    }

    /// Exact static site of this attempt, not the next operation in a snapshot.
    pub const fn site(self) -> SimulationDebugSiteV1 {
        self.site
    }

    /// Monotone, nonzero helper/root activation within the invocation.
    pub const fn activation(self) -> u64 {
        self.activation
    }

    /// Monotone, nonzero attempt within that activation; not a loop-trip count.
    pub const fn attempt(self) -> u64 {
        self.attempt
    }
}

/// Why a record cannot carry one exact runtime operation origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugOriginUnavailableV1 {
    /// Context was not requested by the sink.
    NotRequested,
    /// There is no active operation, or its invocation/site does not match.
    NoMatchingOperation,
    /// A workgroup/wave release describes multiple participants, not one lane.
    AggregateRecord,
    /// Checked bookkeeping failed; later contexts in this run remain unavailable.
    IdentityInvariant,
}

/// One fixed-size context delivered atomically with the legacy record callback.
///
/// This does not change SimulationDebugRecordV1, SimulationDebugFrameV1, or any
/// serialized trace/resource grammar. An available attempt does not imply
/// successful execution or a captured stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugOriginContextV1 {
    Available(SimulationDebugOperationOriginV1),
    Unavailable(SimulationDebugOriginUnavailableV1),
}

// Public callback payload has no allocation. Existing resident census accounts
// for fixed engine/frame/machine fields via their actual size_of values.
// A future collector must charge its own compact rows/capacities; retaining
// this full context per record is NOT included in the old V1 value/byte limits.
const _: () = assert!(std::mem::size_of::<SimulationDebugOriginContextV1>() <= 256);
