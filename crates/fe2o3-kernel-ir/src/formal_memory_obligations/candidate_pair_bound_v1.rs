use super::*;

/// Maximum candidate pairs before the standalone legacy formal report allocates.
/// This does not establish production shared-ledger work or storage credit.
pub const MAX_FORMAL_MEMORY_CANDIDATE_PAIRS_V1: usize = 1_024;

/// Failure of a conservative, pre-extraction report-growth bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalMemoryCandidatePairErrorV1 {
    /// Candidate pairs already exceed the permitted report-shape envelope.
    Limit {
        /// The first conservative pair count above the limit.
        actual: usize,
        /// The caller's limit clamped to the fixed maximum.
        limit: usize,
    },
    /// The candidate count or triangular pair count could not be represented.
    Arithmetic,
}
impl fmt::Display for FormalMemoryCandidatePairErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "formal memory candidate-pair preflight: {self:?}")
    }
}
impl Error for FormalMemoryCandidatePairErrorV1 {}

/// Descriptive preflight census, not an owner binding or an admission receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalMemoryCandidatePairBoundV1 {
    accesses: usize,
    pairs: usize,
}
impl FormalMemoryCandidatePairBoundV1 {
    /// All potential access-producing instructions, including private/dead ones.
    pub const fn accesses(self) -> usize {
        self.accesses
    }
    /// Checked A*(A+1)/2, including every possible self-pair.
    pub const fn pairs(self) -> usize {
        self.pairs
    }
}

/// Counts potential report growth before calling formal extraction on a genuine
/// legacy verified module. A separate fixed V18 entrance uses the same internal
/// traversal on its actual owner's module; no token conversion is performed.
///
/// The current ordinary engine appends at most one access for Load, GuardedLoad,
/// Store, GuardedStore or Atomic. A guarded fallback replaces the failed primary
/// derivation; it cannot add a second row. Other arms append no accesses. All
/// functions/blocks are counted, even unreachable or private instructions. Thus
/// A bounds entry access rows and A*(A+1)/2 bounds conflict-pair candidates;
/// alias envelopes are at most A and alias pairs at most A*(A-1)/2. This is an
/// intentionally conservative preflight, not a claim that every pair conflicts.
///
/// The caller's cap is clamped to the fixed maximum. No report/index allocation,
/// effects analysis or formal extraction occurs here. Scan work and later formal
/// allocations are not paid on a production ledger by this standalone helper.
pub fn bound_formal_memory_candidate_pairs_v1(
    verified: VerifiedKernelIrModuleV1<'_>,
    limit: usize,
) -> Result<FormalMemoryCandidatePairBoundV1, FormalMemoryCandidatePairErrorV1> {
    bound_candidate_pairs_for_module_v1(verified.module(), limit)
}

pub(super) fn bound_candidate_pairs_for_module_v1(
    module: &Module,
    limit: usize,
) -> Result<FormalMemoryCandidatePairBoundV1, FormalMemoryCandidatePairErrorV1> {
    let limit = limit.min(MAX_FORMAL_MEMORY_CANDIDATE_PAIRS_V1);
    let mut result = FormalMemoryCandidatePairBoundV1 {
        accesses: 0,
        pairs: 0,
    };
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                if matches!(
                    operation.kind,
                    OperationKind::Load { .. }
                        | OperationKind::GuardedLoad { .. }
                        | OperationKind::Store { .. }
                        | OperationKind::GuardedStore { .. }
                        | OperationKind::Atomic(_)
                ) {
                    result.accesses = result
                        .accesses
                        .checked_add(1)
                        .ok_or(FormalMemoryCandidatePairErrorV1::Arithmetic)?;
                    // T(n) = T(n-1) + n avoids an overflowing multiplication.
                    result.pairs = result
                        .pairs
                        .checked_add(result.accesses)
                        .ok_or(FormalMemoryCandidatePairErrorV1::Arithmetic)?;
                    if result.pairs > limit {
                        return Err(FormalMemoryCandidatePairErrorV1::Limit {
                            actual: result.pairs,
                            limit,
                        });
                    }
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "candidate_pair_bound_v1_tests.rs"]
mod tests;
