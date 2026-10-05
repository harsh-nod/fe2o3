//! Fixed actual-owner V18 access to the unchanged formal extraction engine.
//! This is an inert prerequisite, not an admission or shared-ledger interface.

use super::*;
use crate::VerifiedCanonicalKernelIrModuleV18;

/// Standalone input/replay bounds, not allocation receipts or solver budgets.
/// Callers may lower these caps, never raise their fixed default maxima.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalOwnerFormalLimitsV18 {
    /// Exact canonical bytes, including all unreachable code and unused metadata.
    pub canonical_bytes: usize,
    /// Complete function roster, including declarations and non-root helpers.
    pub functions: usize,
    /// Cumulative root queries in one scope, including rejected queries.
    pub queries: usize,
    /// Conservative access-pair cap checked before effect/formal report allocation.
    pub candidate_pairs: usize,
}
impl Default for CanonicalOwnerFormalLimitsV18 {
    fn default() -> Self {
        Self {
            canonical_bytes: 262_144,
            functions: crate::MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1,
            queries: 1_024,
            candidate_pairs: MAX_FORMAL_MEMORY_CANDIDATE_PAIRS_V1,
        }
    }
}

/// A refusal before, or from, the actual-owner formal engine.
#[derive(Debug)]
pub enum CanonicalOwnerFormalErrorV18 {
    /// Potential quadratic report growth was refused before extraction.
    CandidatePairs(FormalMemoryCandidatePairErrorV1),
    /// A fixed standalone input/replay bound was exceeded.
    Limit {
        /// The bounded input or query dimension.
        resource: &'static str,
        /// The observed input or attempted query count.
        actual: usize,
        /// The fixed or caller-lowered limit.
        limit: usize,
    },
    /// The genuine storage-aware effect or formal engine refused the input.
    Formal(FormalMemoryObligationError),
}
impl fmt::Display for CanonicalOwnerFormalErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "actual-owner formal V18: {self:?}")
    }
}
impl Error for CanonicalOwnerFormalErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CandidatePairs(error) => Some(error),
            Self::Formal(error) => Some(error),
            Self::Limit { .. } => None,
        }
    }
}
type ResultV18<T> = Result<T, CanonicalOwnerFormalErrorV18>;

/// Fresh formal observations retaining the actual immutable V18 owner.
/// The launch is an explicit analysis input, not authenticated runtime state.
/// The V1 report, including incomplete reasons and every conflict, is unchanged.
/// This wrapper cannot be decoded from bytes or assembled from claimed hashes.
#[derive(Debug)]
#[must_use = "formal observations do not establish admission without their live owner"]
pub struct CanonicalOwnerFormalAnalysisV18<'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    launch: ExplicitLaunchExtent,
    analysis: FormalMemoryObligationAnalysis,
}
impl<'owner> CanonicalOwnerFormalAnalysisV18<'owner> {
    /// The very owner used to derive the effect summary and formal rows.
    pub const fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }
    /// Pointer identity, never equality of canonical identities or module bytes.
    pub fn belongs_to(&self, owner: &VerifiedCanonicalKernelIrModuleV18) -> bool {
        std::ptr::eq(self.owner, owner)
    }
    /// Original caller-supplied analysis launch, without runtime authority.
    pub const fn launch(&self) -> ExplicitLaunchExtent {
        self.launch
    }
    /// Unmodified fresh formal obligations and completeness reasons.
    pub const fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        &self.analysis
    }
}

/// Exact nominal V18 entrance sharing the existing storage-aware effects and
/// authenticated-module formal engines. It neither erases storage metadata nor
/// obtains a legacy verification token. Calls, pointers, ordinary memory and
/// control flow reach the existing engine instead of a scalar-only filter.
/// Every unsupported effect remains unsupported; no V19/ordered-composition
/// exemptions are supplied, and an empty report is never manufactured.
///
/// The canonical-byte cap covers the entire authenticated transport, including
/// nested type/layout/operand metadata and unreachable bodies, without another
/// traversal or serialization. Effects, CFG and guarded analysis retain their
/// existing independent resource limits. This is NOT a production shared-ledger
/// census, retained-storage receipt or operation-work budget. Integration still
/// requires metering all formal indexes/rows and binding actual source/output
/// custody, ABI, launch, ledger and versioned conflict-discharge receipts.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::*;
/// fn escape(owner: VerifiedCanonicalKernelIrModuleV18)
///     -> CanonicalOwnerFormalAnalysisV18<'static>
/// {
///     let mut scope = CanonicalOwnerFormalScopeV18::new(&owner, Default::default()).unwrap();
///     scope.derive(&KernelId::new("root"), ExplicitLaunchExtent::Unknown,
///         FormalIndexWidth::Bits64).unwrap()
/// }
/// ```
pub struct CanonicalOwnerFormalScopeV18<'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    effects: crate::InterproceduralEffectAnalysisV1,
    queries: usize,
    query_limit: usize,
    identity_limit: usize,
}
impl<'owner> CanonicalOwnerFormalScopeV18<'owner> {
    /// Retains the real owner and derives its real storage-aware effect summary.
    pub fn new(
        owner: &'owner VerifiedCanonicalKernelIrModuleV18,
        limits: CanonicalOwnerFormalLimitsV18,
    ) -> ResultV18<Self> {
        let cap = CanonicalOwnerFormalLimitsV18::default();
        bound(
            "canonical bytes",
            owner.canonical_bytes().len(),
            limits.canonical_bytes.min(cap.canonical_bytes),
        )?;
        bound(
            "functions",
            owner.module().functions.len(),
            limits.functions.min(cap.functions),
        )?;
        candidate_pair_bound_v1::bound_candidate_pairs_for_module_v1(
            owner.module(),
            limits.candidate_pairs,
        )
        .map_err(CanonicalOwnerFormalErrorV18::CandidatePairs)?;
        let effects =
            crate::interprocedural_effects::analyze_interprocedural_effects_from_storage_v18(
                owner.verified_storage_module_ref_v1(),
            )
            .map_err(|error| {
                CanonicalOwnerFormalErrorV18::Formal(FormalMemoryObligationError::InvalidModule(
                    error,
                ))
            })?;
        Ok(Self {
            owner,
            effects,
            queries: 0,
            query_limit: limits.queries.min(cap.queries),
            identity_limit: owner.canonical_bytes().len(),
        })
    }

    /// Exact actual owner, not an independently reconstructed equal-byte graph.
    pub const fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }

    /// Derives fresh observations for a root in this actual owner's module.
    /// Replays consume the same finite query allowance even when they fail.
    pub fn derive(
        &mut self,
        kernel: &KernelId,
        launch: ExplicitLaunchExtent,
        index_width: FormalIndexWidth,
    ) -> ResultV18<CanonicalOwnerFormalAnalysisV18<'owner>> {
        let actual = self.queries.saturating_add(1);
        bound("queries", actual, self.query_limit)?;
        self.queries = actual;
        bound("query identity", kernel.as_str().len(), self.identity_limit)?;
        let analysis = derive_kernel_memory_obligations_from_authenticated_module(
            self.owner.module(),
            kernel,
            launch,
            index_width,
            None,
            None,
            &self.effects,
        )
        .map_err(CanonicalOwnerFormalErrorV18::Formal)?;
        Ok(CanonicalOwnerFormalAnalysisV18 {
            owner: self.owner,
            launch,
            analysis,
        })
    }
}
fn bound(resource: &'static str, actual: usize, limit: usize) -> ResultV18<()> {
    if actual > limit {
        Err(CanonicalOwnerFormalErrorV18::Limit {
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "actual_owner_v18_tests.rs"]
mod tests;
