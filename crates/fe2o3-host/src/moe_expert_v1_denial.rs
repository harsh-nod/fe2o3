//! Fail-closed boundary for the bounded MoE expert host binding.
//!
//! This module deliberately defines no execution lifecycle. The denial token
//! retains the typed host binding, but exposes no artifact load, device copy,
//! kernel dispatch, completion, or unload operation.

use crate::GeneratedMoeExpertV1HostAdapterV1;
use std::fmt;

const AUTHORITY_BLOCKER: &str = "compiler/finalizer artifact authority and an authenticated compact-pack execution path are unavailable";

/// A retained MoE expert binding for which every execution operation is denied.
///
/// This token is not a model of a future runtime. Dropping it only releases the
/// Rust borrows retained by the host binding; it says nothing about GPU
/// execution or quiescence because no GPU operation can be started through it.
#[must_use = "the denial token retains the admitted MoE buffer leases"]
pub struct MoeExpertExecutionDeniedV1<B> {
    _binding: B,
}

impl<B> MoeExpertExecutionDeniedV1<B> {
    pub const fn reason(&self) -> &'static str {
        AUTHORITY_BLOCKER
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_copy_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }
}

impl<B> fmt::Debug for MoeExpertExecutionDeniedV1<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeExpertExecutionDeniedV1")
            .field("reason", &AUTHORITY_BLOCKER)
            .finish_non_exhaustive()
    }
}

/// Consumes the exact typed binding and retains it behind a denial-only API.
///
/// This function performs no load, copy, dispatch, synchronization, or unload.
pub fn deny_moe_expert_execution_v1<
    'activations,
    'weights,
    'offsets,
    'inverse,
    'route_weights,
    'expert_output,
    'compact_output,
    'combined_output,
>(
    binding: GeneratedMoeExpertV1HostAdapterV1<
        'activations,
        'weights,
        'offsets,
        'inverse,
        'route_weights,
        'expert_output,
        'compact_output,
        'combined_output,
    >,
) -> MoeExpertExecutionDeniedV1<
    GeneratedMoeExpertV1HostAdapterV1<
        'activations,
        'weights,
        'offsets,
        'inverse,
        'route_weights,
        'expert_output,
        'compact_output,
        'combined_output,
    >,
> {
    MoeExpertExecutionDeniedV1 { _binding: binding }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denial_token_has_no_authority() {
        let denied = MoeExpertExecutionDeniedV1 { _binding: () };
        assert!(denied.reason().contains("authority"));
        assert!(!denied.grants_artifact_authority());
        assert!(!denied.grants_copy_authority());
        assert!(!denied.grants_load_authority());
        assert!(!denied.grants_dispatch_authority());
    }
}
