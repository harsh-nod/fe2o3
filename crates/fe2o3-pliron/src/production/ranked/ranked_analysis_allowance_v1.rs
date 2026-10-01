//! Caller-selected analysis allowance for the unchanged closed ranked pipeline.
//!
//! This allowance covers the existing production-analysis account, including
//! occurrence bindings and the two mandatory verifier runs. It is NOT a budget
//! for recipe construction/transformation, initial recipe hashing, legacy graph
//! snapshot printing, dialect/context allocation, or all retained Pliron state.

use super::{
    ProductionConstructionV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_with_target_and_analysis_limits_v1,
    ranked_gfx942_atomic_target_v1,
};
use crate::production_analysis::ProductionAnalysisResourceLimitsV1;

/// A caller-selected restriction of the existing production-analysis ceiling.
///
/// Zero is a valid allowance and can cause the first positive-cost analysis
/// admission to refuse. This value grants no verification, compiler refinement,
/// artifact, or launch authority. It does not meter allocations made before the
/// existing analysis account, or establish a complete context-storage envelope.
///
/// Callers using a separate resource ledger must reserve this allowance before
/// calling the compilation endpoint. The returned lowering input owns its live
/// Pliron session; any enclosing retained-storage reservation must outlive that
/// input, including its use by the actual lowering consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRankedAnalysisAllowanceV1 {
    limits: ProductionAnalysisResourceLimitsV1,
}

impl ProductionRankedAnalysisAllowanceV1 {
    /// Restricts the two analysis dimensions without increasing either hard cap.
    ///
    /// Construction/transform/context costs require separate caller admission;
    /// this constructor only validates two scalar analysis limits.
    pub fn new(
        max_work: usize,
        max_peak_storage: usize,
    ) -> Result<Self, ProductionRankedAnalysisAllowanceErrorV1> {
        let ceiling = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        if max_work > ceiling.max_work() {
            return Err(ProductionRankedAnalysisAllowanceErrorV1::WorkAboveHardCeiling);
        }
        if max_peak_storage > ceiling.max_peak_storage() {
            return Err(ProductionRankedAnalysisAllowanceErrorV1::StorageAboveHardCeiling);
        }
        Ok(Self {
            limits: ProductionAnalysisResourceLimitsV1::new(max_work, max_peak_storage),
        })
    }

    /// Maximum cumulative work admitted by the existing analysis account.
    pub const fn max_work(self) -> usize {
        self.limits.max_work()
    }

    /// Maximum overlapping storage admitted by the existing analysis account.
    ///
    /// This is not the total heap size of the recipe, session, or process.
    pub const fn max_peak_storage(self) -> usize {
        self.limits.max_peak_storage()
    }
}

/// A requested analysis allowance would widen an existing production hard cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRankedAnalysisAllowanceErrorV1 {
    WorkAboveHardCeiling,
    StorageAboveHardCeiling,
}

impl std::fmt::Display for ProductionRankedAnalysisAllowanceErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::WorkAboveHardCeiling => "ranked analysis work allowance exceeds the hard ceiling",
            Self::StorageAboveHardCeiling => {
                "ranked analysis storage allowance exceeds the hard ceiling"
            }
        })
    }
}

impl std::error::Error for ProductionRankedAnalysisAllowanceErrorV1 {}

/// Runs the original closed ranked pipeline with a caller-restricted analysis
/// account, including both verifier runs and their report comparison.
///
/// The construction is already owned and validated: its earlier constructor
/// and three transformation passes are NOT retroactively admitted here. Initial
/// recipe hashing, legacy graph snapshots and the live Pliron context likewise
/// retain their existing separate resource boundary. No returned analysis
/// subtotal may be post-charged as if it covered these excluded costs.
///
/// This consumes the exact supplied construction. It creates a fresh closed
/// session, preserves normal-form/current-graph checks, and returns the original
/// move-only lowering input only after all mandatory verification transitions.
/// The input retains that session until the input is consumed or dropped.
/// Existing default compilation functions retain their hard-ceiling behavior.
/// No target capability is injected by this target-neutral entrypoint.
///
/// The construction cannot be reused after either success or refusal:
///
/// ```compile_fail
/// use fe2o3_pliron::{
///     compile_ranked_kernel_for_lowering_with_analysis_allowance_v1,
///     ProductionConstructionV1, ProductionRankedAnalysisAllowanceV1,
///     ProductionSessionLimitsV1,
/// };
/// fn reuse(c: ProductionConstructionV1, s: ProductionSessionLimitsV1,
///          a: ProductionRankedAnalysisAllowanceV1) {
///     let _ = compile_ranked_kernel_for_lowering_with_analysis_allowance_v1(c, s, a);
///     let _ = compile_ranked_kernel_for_lowering_with_analysis_allowance_v1(c, s, a);
/// }
/// ```
pub fn compile_ranked_kernel_for_lowering_with_analysis_allowance_v1(
    construction: ProductionConstructionV1,
    limits: ProductionSessionLimitsV1,
    allowance: ProductionRankedAnalysisAllowanceV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    compile_ranked_kernel_for_lowering_with_target_and_analysis_limits_v1(
        construction,
        limits,
        None,
        allowance.limits,
    )
}

/// Runs the same gfx942 closed pipeline with a caller-restricted analysis account.
///
/// This shares the existing gfx942 atomic-target builder, capability profile and
/// fixed verifier sequence. The allowance does not cover recipe construction,
/// transformation, target setup, initial hashing, snapshots or the context.
/// It does not admit another target or establish nominal BF16 readiness. The
/// returned move-only input retains its original session and target context.
pub fn compile_ranked_kernel_for_gfx942_lowering_with_analysis_allowance_v1(
    construction: ProductionConstructionV1,
    limits: ProductionSessionLimitsV1,
    system_coherent_allocations: impl IntoIterator<Item = u64>,
    allowance: ProductionRankedAnalysisAllowanceV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    let target = ranked_gfx942_atomic_target_v1(system_coherent_allocations)?;
    compile_ranked_kernel_for_lowering_with_target_and_analysis_limits_v1(
        construction,
        limits,
        Some(target),
        allowance.limits,
    )
}

#[cfg(test)]
#[path = "ranked_analysis_allowance_v1_tests.rs"]
mod tests;

impl ProductionRankedAnalysisAllowanceV1 {
    /// Returns the existing analysis ceiling, without changing any default.
    /// This is a scalar allowance, not prepaid work or storage.
    pub fn production_hard_ceiling() -> Self {
        Self {
            limits: ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        }
    }
}

/// Caller-selected bounds for the existing two-pass presentation sink/hash.
/// These do not cover Display traversal/allocation, constructors, or Context.
/// A caller's separate account must prepay max_work before the session starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRankedSnapshotAllowanceV1 {
    max_bytes: usize,
    max_work: usize,
}

impl ProductionRankedSnapshotAllowanceV1 {
    pub fn new(
        max_bytes: usize,
        max_work: usize,
    ) -> Result<Self, ProductionRankedSnapshotAllowanceErrorV1> {
        if max_bytes > crate::HARD_MAX_OPERATION_IMPORT_BYTES {
            return Err(ProductionRankedSnapshotAllowanceErrorV1::BytesAboveHardCeiling);
        }
        if max_work > ProductionAnalysisResourceLimitsV1::production_hard_ceiling().max_work() {
            return Err(ProductionRankedSnapshotAllowanceErrorV1::WorkAboveHardCeiling);
        }
        Ok(Self {
            max_bytes,
            max_work,
        })
    }

    /// Existing policy ceilings, not a new memory envelope.
    pub fn production_hard_ceiling() -> Self {
        Self {
            max_bytes: crate::HARD_MAX_OPERATION_IMPORT_BYTES,
            max_work: ProductionAnalysisResourceLimitsV1::production_hard_ceiling().max_work(),
        }
    }

    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }
    pub const fn max_work(self) -> usize {
        self.max_work
    }

    fn into_policy(self) -> crate::graph_analysis_v1::snapshot_policy_v1::SnapshotPolicyV1 {
        crate::graph_analysis_v1::snapshot_policy_v1::SnapshotPolicyV1::new(
            self.max_bytes,
            self.max_work,
        )
        .expect("checked snapshot allowance preserves the existing ceilings")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRankedSnapshotAllowanceErrorV1 {
    BytesAboveHardCeiling,
    WorkAboveHardCeiling,
}
impl std::fmt::Display for ProductionRankedSnapshotAllowanceErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::BytesAboveHardCeiling => {
                "ranked snapshot byte allowance exceeds the hard ceiling"
            }
            Self::WorkAboveHardCeiling => "ranked snapshot work allowance exceeds the hard ceiling",
        })
    }
}
impl std::error::Error for ProductionRankedSnapshotAllowanceErrorV1 {}

/// Selects both existing resource policies on the unchanged target-neutral engine.
///
/// Callers with an enclosing account must precharge analysis.max_work() plus
/// snapshot.max_work() and retain at least analysis.max_peak_storage() while the
/// returned lowering input/session remains live. No unused work is refunded by
/// this endpoint. The snapshot byte limit bounds each presentation's emitted
/// UTF8, not a retained String or a reservation for all printer allocations.
///
/// The supplied construction already exists. Its constructor/transforms, initial
/// recipe hashing, Context/dialect allocation and Display internals are excluded
/// and require separate caller admission. This endpoint does not establish BF16
/// normal readiness or artifact/launch authority. Defaults and targets are unchanged.
pub fn compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1(
    construction: ProductionConstructionV1,
    limits: ProductionSessionLimitsV1,
    analysis: ProductionRankedAnalysisAllowanceV1,
    snapshot: ProductionRankedSnapshotAllowanceV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    super::compile_ranked_kernel_for_lowering_with_resource_policy_v1(
        construction,
        limits,
        None,
        analysis.limits,
        Some(snapshot.into_policy()),
    )
}
