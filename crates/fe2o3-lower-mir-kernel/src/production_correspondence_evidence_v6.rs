//! Original source -> checked execution views -> exact production KIR.
//!
//! Decoding is deliberately inert. Only live replay composes the expansion,
//! complete per-root induction reports, SSA and lowering relations. V4/V5 are
//! neither constructed nor reinterpreted by this format.

use super::*;
use fe2o3_mir_model::{
    InertCanonicalSemanticCallExpansionEvidenceV1, InertCanonicalSemanticU32InductionEvidenceV1,
    InertCanonicalSemanticU32InductionEvidenceV2, SemanticCallExpansionEvidenceErrorV1,
    SemanticU32InductionAnalysisLimitsV1, SemanticU32InductionEvidenceErrorV1,
    SemanticU32InductionEvidenceErrorV2, SemanticU32InductionNoOverflowReportV1,
    analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1,
};

#[path = "production_correspondence_evidence_v6/codec.rs"]
mod codec;
use codec::{decode, encode};

#[cfg(test)]
#[path = "production_correspondence_evidence_v6/tests.rs"]
mod tests;

/// Wire version composing checked call expansion with production lowering.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V6: u16 = 6;
/// Closed replay policy; decoded records never establish equivalence themselves.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V6: u16 = 1;
/// Aggregate byte cap, including the expansion and all induction records.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6: usize = 4 * 1024 * 1024;
/// Aggregate bound on correspondence rows and parameter projection components.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_RECORDS_V6: usize = 262_144;
/// Aggregate bound on nested expansion replay and induction-analysis reservations.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_REPLAY_WORK_V6: usize = 64 * 1024 * 1024;

const DOMAIN: &[u8] = b"FE2O3/COMPOSED-EXECUTION-MIR-TO-KIR-CORRESPONDENCE/V6\0";
type Result<T> = std::result::Result<T, ProductionCorrespondenceEvidenceErrorV6>;

/// Fail-closed V6 decoding, ownership, or independent replay failure.
#[derive(Debug)]
pub enum ProductionCorrespondenceEvidenceErrorV6 {
    /// Invalid version, policy, reserved bits, or discriminant.
    InvalidHeader,
    /// Truncated, overflowing, or trailing bytes.
    InvalidLength,
    /// A byte, row, or composite replay budget was exceeded.
    LimitExceeded,
    /// A bounded allocation failed.
    AllocationFailure,
    /// The wire record has an alternate encoding or stale identity.
    NonCanonical,
    /// Missing, duplicated, reordered, or substituted root/function ownership.
    RootRoster,
    /// An execution coordinate or record shape does not match its declared owner.
    Correspondence,
    /// Original source, SSA, KIR, or lowering limits differ from the live owner.
    OwnerMismatch,
    /// The complete induction report differs from independent analysis.
    ReportMismatch,
    /// An expanded root was paired with source-only induction, or conversely.
    InductionCoordinateSpace,
    /// Checked expansion evidence failed.
    Expansion(SemanticCallExpansionEvidenceErrorV1),
    /// Original-coordinate induction evidence failed.
    InductionV1(SemanticU32InductionEvidenceErrorV1),
    /// Execution-coordinate induction evidence failed.
    InductionV2(SemanticU32InductionEvidenceErrorV2),
    /// Source/SSA/lowering or induction replay rejected the live inputs.
    LiveReplay(String),
}

impl fmt::Display for ProductionCorrespondenceEvidenceErrorV6 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V6 source/execution/KIR correspondence failed: {self:?}")
    }
}
impl Error for ProductionCorrespondenceEvidenceErrorV6 {}

/// Coordinate-tagged, complete induction evidence for one root.
#[derive(Debug, Eq, PartialEq)]
pub enum MirToKirInductionEvidenceV6 {
    /// Only for a checked call-free identity view, including a selected Result body.
    Original(InertCanonicalSemanticU32InductionEvidenceV1),
    /// Bound to the exact expansion evidence and this root's execution view.
    Expanded(InertCanonicalSemanticU32InductionEvidenceV2),
}

/// One physical root and its independently replayable induction report.
#[derive(Debug, Eq, PartialEq)]
pub struct MirToKirRootCorrespondenceEvidenceV6 {
    root: SemanticFunctionIdV1,
    source_body: SemanticFunctionIdV1,
    execution_view_identity: [u8; 32],
    induction: MirToKirInductionEvidenceV6,
}

impl MirToKirRootCorrespondenceEvidenceV6 {
    /// Original physical kernel root, never an expanded function-table index.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Exactly authenticated original body selected for this root.
    pub const fn source_body(&self) -> SemanticFunctionIdV1 {
        self.source_body
    }
    /// Identity scoping every execution block, statement and local for this root.
    pub const fn execution_view_identity(&self) -> &[u8; 32] {
        &self.execution_view_identity
    }
    /// Complete report in its explicitly tagged coordinate space.
    pub const fn induction(&self) -> &MirToKirInductionEvidenceV6 {
        &self.induction
    }
}

/// Exact materialized entry, scoped by original root and selected source body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirToKirFunctionCorrespondenceEvidenceV6 {
    record: SemanticKirFunctionCorrespondenceV1,
    kernel_ir_function_ordinal: u32,
}

impl MirToKirFunctionCorrespondenceEvidenceV6 {
    /// Original physical root owning this execution view.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.record.correspondence_owner
    }
    /// Original selected body; IDs within its spans are execution-view coordinates.
    pub const fn source_body(&self) -> SemanticFunctionIdV1 {
        self.record.semantic_function
    }
    /// Exact ordinal in the canonical KIR module, not in the source function table.
    pub const fn kernel_ir_function_ordinal(&self) -> u32 {
        self.kernel_ir_function_ordinal
    }
    /// Exact KIR function identity.
    pub const fn kernel_ir_function(&self) -> &FunctionId {
        &self.record.kernel_ir_function
    }
    /// Closed materialized function role.
    pub const fn role(&self) -> SemanticKirFunctionRoleV1 {
        self.record.role
    }
}

/// Canonical, authority-free composition of original source and execution-to-KIR records.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalMirToKirCorrespondenceEvidenceV6 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    semantic_ssa_identity: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    lowering_limits: ProductionSemanticKirLimitsV1,
    expansion: InertCanonicalSemanticCallExpansionEvidenceV1,
    roots: Box<[MirToKirRootCorrespondenceEvidenceV6]>,
    functions: Box<[MirToKirFunctionCorrespondenceEvidenceV6]>,
    correspondence: SemanticKirCorrespondenceV1,
}

/// Borrowed replay result: cannot be decoded, detached, or manufactured by callers.
#[derive(Debug)]
pub struct ReplayedMirToKirCorrespondenceV6<'a> {
    evidence: &'a InertCanonicalMirToKirCorrespondenceEvidenceV6,
    owner: &'a ProductionSemanticKirOwnerV1,
    reports: Box<[(SemanticFunctionIdV1, SemanticU32InductionNoOverflowReportV1)]>,
}

impl<'a> ReplayedMirToKirCorrespondenceV6<'a> {
    /// Exact inert bytes whose complete composition was replayed.
    pub const fn evidence(&self) -> &'a InertCanonicalMirToKirCorrespondenceEvidenceV6 {
        self.evidence
    }
    /// Exact still-live lowering owner used by replay, including ranked checks when retained.
    pub const fn owner(&self) -> &'a ProductionSemanticKirOwnerV1 {
        self.owner
    }
    /// Independently recomputed complete reports, in original root order.
    pub fn induction_reports(
        &self,
    ) -> &[(SemanticFunctionIdV1, SemanticU32InductionNoOverflowReportV1)] {
        &self.reports
    }
    /// This relation does not grant artifact, publication, or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl InertCanonicalMirToKirCorrespondenceEvidenceV6 {
    /// Replays the live owner and every supplied complete report in exact original-root order.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
        reports: &[(
            SemanticFunctionIdV1,
            &SemanticU32InductionNoOverflowReportV1,
        )],
        limits: SemanticU32InductionAnalysisLimitsV1,
    ) -> Result<Self> {
        let mut budget = preflight_replay(owner, limits)?;
        if reports
            .iter()
            .map(|(root, _)| root)
            .ne(owner.semantic().semantic().roots())
        {
            return Err(ProductionCorrespondenceEvidenceErrorV6::RootRoster);
        }
        owner.verify_equivalence().map_err(live_error)?;
        let source = owner.semantic().semantic();
        let checked = owner.semantic_ssa.execution_expansion();
        let expansion =
            InertCanonicalSemanticCallExpansionEvidenceV1::from_checked_expansion(source, checked)
                .map_err(ProductionCorrespondenceEvidenceErrorV6::Expansion)?;
        let mut roots = reserve(reports.len())?;
        for &(root, report) in reports {
            let limits = budget.analysis_limits(limits);
            let view = checked
                .root(root)
                .ok_or(ProductionCorrespondenceEvidenceErrorV6::RootRoster)?;
            let induction = if view.has_expanded_calls() {
                MirToKirInductionEvidenceV6::Expanded(
                    InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
                        source, checked, &expansion, root, report, limits,
                    )
                    .map_err(ProductionCorrespondenceEvidenceErrorV6::InductionV2)?,
                )
            } else {
                let replayed = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
                    source, checked, root, limits,
                )
                .map_err(live_error)?;
                if report != &replayed {
                    return Err(ProductionCorrespondenceEvidenceErrorV6::ReportMismatch);
                }
                MirToKirInductionEvidenceV6::Original(
                    InertCanonicalSemanticU32InductionEvidenceV1::from_report(&replayed)
                        .map_err(ProductionCorrespondenceEvidenceErrorV6::InductionV1)?,
                )
            };
            budget.charge(report.work_units())?;
            roots.push(MirToKirRootCorrespondenceEvidenceV6 {
                root,
                source_body: view.source_body(),
                execution_view_identity: *view.identity(),
                induction,
            });
        }
        let mut functions = reserve(owner.correspondence.lowered_functions.len())?;
        let ordinals = owner
            .module
            .functions
            .iter()
            .enumerate()
            .map(|(ordinal, function)| (&function.id, ordinal))
            .collect::<BTreeMap<_, _>>();
        for record in &owner.correspondence.lowered_functions {
            let ordinal = *ordinals
                .get(&record.kernel_ir_function)
                .ok_or(ProductionCorrespondenceEvidenceErrorV6::Correspondence)?;
            functions.push(MirToKirFunctionCorrespondenceEvidenceV6 {
                record: record.clone(),
                kernel_ir_function_ordinal: u32::try_from(ordinal)
                    .map_err(|_| ProductionCorrespondenceEvidenceErrorV6::LimitExceeded)?,
            });
        }
        let candidate = Self {
            canonical_bytes: Box::new([]),
            identity: [0; 32],
            semantic_ssa_identity: *owner.semantic_ssa_identity().as_bytes(),
            canonical_kernel_ir: owner.canonical_kernel_ir_identity(),
            lowering_limits: owner.limits,
            expansion,
            roots: roots.into_boxed_slice(),
            functions: functions.into_boxed_slice(),
            correspondence: owner.correspondence.clone(),
        };
        Self::decode(&encode(&candidate)?)
    }

    /// Strict bounded canonical decoding only; does not establish any semantic claim.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode(bytes)
    }

    /// Checks retained bytes/fields, without granting the live replay result.
    pub fn revalidate(&self) -> Result<()> {
        if Self::decode(&self.canonical_bytes)? != *self {
            return Err(ProductionCorrespondenceEvidenceErrorV6::NonCanonical);
        }
        Ok(())
    }

    /// Replays the original source, expansion, complete induction, SSA and exact KIR.
    ///
    /// A digest or a decoded correspondence cannot replace the live owner. The returned
    /// reports are independently analyzed, never facts released from decoded bytes.
    pub fn verify_replay<'a>(
        &'a self,
        owner: &'a ProductionSemanticKirOwnerV1,
        limits: SemanticU32InductionAnalysisLimitsV1,
    ) -> Result<ReplayedMirToKirCorrespondenceV6<'a>> {
        let mut budget = preflight_replay(owner, limits)?;
        self.revalidate()?;
        if self.semantic_ssa_identity != *owner.semantic_ssa_identity().as_bytes()
            || self.canonical_kernel_ir != owner.canonical_kernel_ir_identity()
            || self.lowering_limits != owner.limits
            || self.semantic_sha256() != owner.correspondence.semantic_sha256()
            || self.correspondence != owner.correspondence
        {
            return Err(ProductionCorrespondenceEvidenceErrorV6::OwnerMismatch);
        }
        owner.verify_equivalence().map_err(live_error)?;
        let source = owner.semantic().semantic();
        let checked = owner.semantic_ssa.execution_expansion();
        self.expansion
            .verify_against_checked_expansion(source, checked)
            .map_err(ProductionCorrespondenceEvidenceErrorV6::Expansion)?;
        if self
            .roots
            .iter()
            .map(|row| row.root)
            .ne(source.roots().iter().copied())
        {
            return Err(ProductionCorrespondenceEvidenceErrorV6::RootRoster);
        }
        for function in &self.functions {
            let actual = owner
                .module
                .functions
                .get(function.kernel_ir_function_ordinal as usize)
                .ok_or(ProductionCorrespondenceEvidenceErrorV6::Correspondence)?;
            if actual.id != function.record.kernel_ir_function
                || actual.body.is_none()
                || actual.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            {
                return Err(ProductionCorrespondenceEvidenceErrorV6::Correspondence);
            }
        }
        let mut reports = reserve(self.roots.len())?;
        for root in &self.roots {
            let limits = budget.analysis_limits(limits);
            let report = match &root.induction {
                MirToKirInductionEvidenceV6::Expanded(evidence) => evidence
                    .verify_replay(source, checked, &self.expansion, limits)
                    .map_err(ProductionCorrespondenceEvidenceErrorV6::InductionV2)?,
                MirToKirInductionEvidenceV6::Original(evidence) => {
                    let report =
                        analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
                            source, checked, root.root, limits,
                        )
                        .map_err(live_error)?;
                    let expected =
                        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&report)
                            .map_err(ProductionCorrespondenceEvidenceErrorV6::InductionV1)?;
                    if &expected != evidence {
                        return Err(ProductionCorrespondenceEvidenceErrorV6::ReportMismatch);
                    }
                    report
                }
            };
            budget.charge(report.work_units())?;
            reports.push((root.root, report));
        }
        Ok(ReplayedMirToKirCorrespondenceV6 {
            evidence: self,
            owner,
            reports: reports.into_boxed_slice(),
        })
    }

    /// Canonical evidence bytes, not a serialized live owner.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Domain-separated identity of every retained field.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Original admitted source hash, never a second expanded MIR document.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        self.expansion.source_semantic_sha256()
    }
    /// Exact checked SSA identity consumed by lowering.
    pub const fn semantic_ssa_identity(&self) -> &[u8; 32] {
        &self.semantic_ssa_identity
    }
    /// Exact version-bound canonical KIR identity.
    pub const fn canonical_kernel_ir(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }
    /// Original-source to per-call execution correspondence, with its own limits and origins.
    pub const fn expansion(&self) -> &InertCanonicalSemanticCallExpansionEvidenceV1 {
        &self.expansion
    }
    /// Complete original-root roster, including call-free roots.
    pub fn roots(&self) -> &[MirToKirRootCorrespondenceEvidenceV6] {
        &self.roots
    }
    /// Exact execution-view entry to KIR-function roster.
    pub fn functions(&self) -> &[MirToKirFunctionCorrespondenceEvidenceV6] {
        &self.functions
    }
    /// Complete execution-to-KIR trace. Interpret coordinates through `expansion()`,
    /// scoped by each row's `correspondence_owner`, never as original source IDs.
    pub const fn execution_correspondence(&self) -> &SemanticKirCorrespondenceV1 {
        &self.correspondence
    }
    /// Decoding alone grants no proof, publication or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn live_error(error: impl fmt::Display) -> ProductionCorrespondenceEvidenceErrorV6 {
    ProductionCorrespondenceEvidenceErrorV6::LiveReplay(error.to_string())
}

fn reserve<T>(count: usize) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| ProductionCorrespondenceEvidenceErrorV6::AllocationFailure)?;
    Ok(result)
}

struct ReplayBudget {
    remaining: usize,
}
impl ReplayBudget {
    fn charge(&mut self, work: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(work)
            .ok_or(ProductionCorrespondenceEvidenceErrorV6::LimitExceeded)?;
        Ok(())
    }
    fn analysis_limits(
        &self,
        limits: SemanticU32InductionAnalysisLimitsV1,
    ) -> SemanticU32InductionAnalysisLimitsV1 {
        SemanticU32InductionAnalysisLimitsV1::new(
            limits.work_units().min(self.remaining),
            limits.certificates(),
        )
    }
}

fn preflight_replay(
    owner: &ProductionSemanticKirOwnerV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
) -> Result<ReplayBudget> {
    let source = owner.semantic().semantic();
    let roots = source.roots().len();
    let maximum = SemanticU32InductionAnalysisLimitsV1::default();
    if roots == 0
        || roots > SemanticCallExpansionLimitsV1::HARD_MAX.roots
        || limits.work_units() > maximum.work_units()
        || limits.certificates() > maximum.certificates()
    {
        return Err(ProductionCorrespondenceEvidenceErrorV6::LimitExceeded);
    }
    // Each V2 report performs two expansion replays. Charge these in advance;
    // each analyzer receives at most the remaining aggregate work allowance.
    // Owner replay retains its independent hard caps.
    let expansion = owner.semantic_ssa.execution_expansion();
    let work = roots
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(expansion.work_units()))
        .ok_or(ProductionCorrespondenceEvidenceErrorV6::LimitExceeded)?;
    if work > MAX_MIR_TO_KIR_CORRESPONDENCE_REPLAY_WORK_V6 {
        return Err(ProductionCorrespondenceEvidenceErrorV6::LimitExceeded);
    }
    codec::validate_record_counts(&owner.correspondence)?;
    Ok(ReplayBudget {
        remaining: MAX_MIR_TO_KIR_CORRESPONDENCE_REPLAY_WORK_V6 - work,
    })
}
