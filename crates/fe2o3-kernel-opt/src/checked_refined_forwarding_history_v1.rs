//! Fixed borrowed B-through-F semantics, not a wire or authenticated producer.
use crate::{
    CanonicalPolicy8CompositionErrorV1, CanonicalPolicy8SemanticInputsV1,
    ReplayedPolicy8SemanticRelationV1, check_published_policy8_semantic_relation_v1,
    private_cell_promotion_resources_v1 as resources,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingErrorV1 as ForwardingError,
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits,
    CanonicalKirCrossBlockForwardingOriginV1 as ForwardingOrigin,
    CanonicalKirInductionRefinementErrorV1 as RefinementError,
    CanonicalKirInductionRefinementOriginV1 as RefinementOrigin,
    CanonicalKirLicmErrorV1 as LicmError, CanonicalKirLicmOriginV1 as LicmOrigin,
    CanonicalKirLoopLimitsV1 as LoopLimits, CanonicalKirLoopPreheaderV1 as Preheader,
    CanonicalKirLoopPreheadersErrorV1 as PreheadersError,
    CanonicalKirPrivateCellCensusLimitsV1 as PromotionLimits,
    CanonicalKirPrivateCellOriginV1 as PromotionOrigin,
    CanonicalKirPrivateCellPromotionErrorV1 as PromotionError,
    CheckedCanonicalKirCrossBlockForwardingV1 as Forwarding,
    CheckedCanonicalKirInductionRefinementV1 as Refinement, CheckedCanonicalKirLicmV1 as Licm,
    CheckedCanonicalKirLoopPreheadersV1 as Preheaders,
    CheckedCanonicalKirPrivateCellPromotionV1 as Promotion,
    check_canonical_kir_cross_block_forwarding_v1, check_canonical_kir_induction_refinement_v1,
    check_canonical_kir_licm_v1, check_canonical_kir_loop_preheaders_v1,
    check_canonical_kir_private_cell_promotion_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Site, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

/// The two caller-selected limits retained by the actual R/F transactions.
/// Earlier promotion/preheader/LICM transactions have fixed analysis limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRefinedForwardingHistoryLimitsV1 {
    /// All seven exact induction-refinement limits.
    pub refinement: LoopLimits,
    /// Exact cross-block forwarding analysis limits.
    pub forwarding: ForwardingLimits,
}
type Limits = CanonicalRefinedForwardingHistoryLimitsV1;

/// Fixed semantic subjects and complete claims. No hashes replace actual owners.
/// Each tail's input is its predecessor's output, not a second selectable field.
#[derive(Clone, Copy)]
pub struct CanonicalRefinedForwardingHistoryInputsV1<'a> {
    /// Existing complete B/C/S/O/I/J/K history with its original policy identities.
    pub prefix: CanonicalPolicy8SemanticInputsV1<'a>,
    /// Actual private-cell promotion output.
    pub promoted: &'a Owner,
    /// Every selected allocation in the actual K input.
    pub selected_allocations: &'a [Site],
    /// Complete private-cell output origins.
    pub promotion_origins: &'a [PromotionOrigin],
    /// Actual neutral-preheader output.
    pub preheaders: &'a Owner,
    /// Complete selected preheader rows, including all external edge occurrences.
    pub preheader_rows: &'a [Preheader],
    /// Actual LICM output L.
    pub licm: &'a Owner,
    /// Complete original-operation-ordered LICM origins.
    pub licm_origins: &'a [LicmOrigin],
    /// Actual induction-refinement output R.
    pub refined: &'a Owner,
    /// Complete one-to-two refinement origins, including unchanged operations.
    pub refinement_origins: &'a [RefinementOrigin],
    /// Actual forwarding output F, never historical K or L.
    pub output: &'a Owner,
    /// Complete R-input-ordered forwarding origins.
    pub forwarding_origins: &'a [ForwardingOrigin],
    /// Exact R/F limits; no clamping or inferred execution caps.
    pub limits: Limits,
}
type Inputs<'a> = CanonicalRefinedForwardingHistoryInputsV1<'a>;

/// Exact failed stage; boxed nested diagnostics are not returned receipt backing.
#[derive(Debug)]
pub enum CanonicalRefinedForwardingHistoryErrorV1 {
    /// Existing P8 semantic prefix or one of its fixed predecessor policies.
    Prefix(Box<CanonicalPolicy8CompositionErrorV1>),
    /// Exact K-to-promoted pair refusal.
    Promotion(PromotionError),
    /// Exact promoted-to-preheaders pair refusal.
    Preheaders(PreheadersError),
    /// Exact preheaders-to-L pair refusal.
    Licm(LicmError),
    /// Exact L-to-R pair refusal.
    Refinement(RefinementError),
    /// Exact R-to-F pair refusal.
    Forwarding(ForwardingError),
    /// Changed limits, even if they would admit the same graphs.
    LimitsMismatch,
    /// Cumulative caller work/storage/accounting refusal.
    Resource(Resource),
    /// Local replay unwound; no rejected receipt escapes.
    Panicked,
}
type Error = CanonicalRefinedForwardingHistoryErrorV1;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fixed B-through-F history: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Added logical wrapper and all nested receipt storage, returned unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRefinedForwardingHistoryStorageV1(usize);
impl CanonicalRefinedForwardingHistoryStorageV1 {
    /// Reserve until the borrowed receipt is dropped, before controlled work.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CanonicalRefinedForwardingHistoryStorageV1;

/// Independently checked fixed semantic chain, with every endpoint still borrowed.
/// Source, producer execution, descriptor, artifact and runtime authority are absent.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalRefinedForwardingHistoryV1 as History;
/// fn copy<'a>(v: &History<'a>) -> History<'a> { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::CheckedCanonicalRefinedForwardingHistoryV1 as History;
/// fn mutate(v: &mut History<'_>) { v.inputs.limits.refinement.operations = 0; }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalRefinedForwardingHistoryV1 as History;
/// fn escape<'a>(v: History<'a>) -> History<'static> { v }
/// ```
pub struct CheckedCanonicalRefinedForwardingHistoryV1<'a> {
    inputs: Inputs<'a>,
    prefix: ReplayedPolicy8SemanticRelationV1<'a>,
    promotion: Promotion<'a>,
    preheaders: Preheaders<'a>,
    licm: Licm<'a>,
    refinement: Refinement<'a>,
    forwarding: Forwarding<'a>,
    storage: Storage,
}
type History<'a> = CheckedCanonicalRefinedForwardingHistoryV1<'a>;
impl<'a> History<'a> {
    /// Independently replayed existing P8 prefix, still about K.
    pub const fn prefix(&self) -> &ReplayedPolicy8SemanticRelationV1<'a> {
        &self.prefix
    }
    /// Exact K-to-promoted relation.
    pub const fn promotion(&self) -> &Promotion<'a> {
        &self.promotion
    }
    /// Exact neutral preheader subdivision relation.
    pub const fn preheaders(&self) -> &Preheaders<'a> {
        &self.preheaders
    }
    /// Exact total-operation motion relation.
    pub const fn licm(&self) -> &Licm<'a> {
        &self.licm
    }
    /// Exact one-to-two L/R relation.
    pub const fn refinement(&self) -> &Refinement<'a> {
        &self.refinement
    }
    /// Exact R/F relation.
    pub const fn forwarding(&self) -> &Forwarding<'a> {
        &self.forwarding
    }
    /// The same actual F supplied to the last independent pair.
    pub const fn output(&self) -> &'a Owner {
        self.inputs.output
    }
    /// Exact stored limits, not a source admission or execution budget claim.
    pub const fn limits(&self) -> Limits {
        self.inputs.limits
    }
    /// Added logical receipt ownership, excluding caller-owned backing.
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    /// This semantic checker does not authenticate execution.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// No source, descriptor, publication or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Independently repeats every check against the same immutable endpoints.
    /// The original receipt and its caller-owned backing must remain live.
    pub fn replay(&self, limits: Limits, budget: &mut Budget<'_>) -> Result<(), Error> {
        if budget.storage() < self.storage.0 {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(
                size_of::<Limits>()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if limits != self.inputs.limits {
                return Err(Error::LimitsMismatch);
            }
            let checked =
                meter.derive(|b| check_canonical_refined_forwarding_history_v1(self.inputs, b))?;
            let retained = checked.storage().retained_storage();
            meter.reserve(retained)?;
            meter.work(1)?;
            drop(checked);
            meter.release(retained)?;
            Ok(())
        })
    }
}

fn wrapper() -> Result<usize, Error> {
    size_of::<History<'_>>()
        .checked_sub(size_of::<ReplayedPolicy8SemanticRelationV1<'_>>())
        .and_then(|n| n.checked_sub(size_of::<Promotion<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<Preheaders<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<Licm<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<Refinement<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<Forwarding<'_>>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}

/// Replays one fixed complete B-through-F chain with the existing independent
/// semantic/pair engines. Every next input is the preceding actual output.
/// The historical promotion/preheader/LICM transactions fix their analysis limits
/// at compile time; their default values here match those exact owner APIs.
/// Existing P8 records retain their fixed policy identities. No pass is executed.
///
/// All nested work is cumulative. Caller graph/row/record backing remains live
/// once; the new wrapper minus embedded headers and all six nested receipts are
/// reserved through final use. No graph copy, additional allocator domain or RSS
/// claim is introduced. Failure/unwind drops locals before same-ledger cleanup;
/// work/peak/first-denial history is never reset. Success returns an unreserved
/// borrowed receipt that must be reserved before later controlled work.
pub fn check_canonical_refined_forwarding_history_v1<'a>(
    inputs: Inputs<'a>,
    budget: &mut Budget<'_>,
) -> Result<History<'a>, Error> {
    resources::scoped(budget, |meter| {
        let mut retained = wrapper()?;
        meter.reserve(retained)?;
        let prefix = meter.derive(|b| {
            check_published_policy8_semantic_relation_v1(inputs.prefix, b)
                .map_err(|e| Error::Prefix(Box::new(e)))
        })?;
        let ps = prefix.storage().retained_storage();
        meter.reserve(ps)?;
        retained = retained.checked_add(ps).ok_or(Resource::Arithmetic)?;
        let (promotion, ps) = meter.derive(|b| {
            check_canonical_kir_private_cell_promotion_v1(
                prefix.output(),
                inputs.promoted,
                inputs.selected_allocations,
                inputs.promotion_origins,
                PromotionLimits::default(),
                b,
            )
            .map_err(Error::Promotion)
        })?;
        meter.reserve(ps.retained_storage())?;
        retained = retained
            .checked_add(ps.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let (preheaders, ps) = meter.derive(|b| {
            check_canonical_kir_loop_preheaders_v1(
                promotion.output(),
                inputs.preheaders,
                inputs.preheader_rows,
                LoopLimits::default(),
                b,
            )
            .map_err(Error::Preheaders)
        })?;
        meter.reserve(ps.retained_storage())?;
        retained = retained
            .checked_add(ps.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let (licm, ps) = meter.derive(|b| {
            check_canonical_kir_licm_v1(
                preheaders.output(),
                inputs.licm,
                inputs.licm_origins,
                LoopLimits::default(),
                b,
            )
            .map_err(Error::Licm)
        })?;
        meter.reserve(ps.retained_storage())?;
        retained = retained
            .checked_add(ps.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let (refinement, ps) = meter.derive(|b| {
            check_canonical_kir_induction_refinement_v1(
                licm.output(),
                inputs.refined,
                inputs.refinement_origins,
                inputs.limits.refinement,
                b,
            )
            .map_err(Error::Refinement)
        })?;
        meter.reserve(ps.retained_storage())?;
        retained = retained
            .checked_add(ps.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let (forwarding, ps) = meter.derive(|b| {
            check_canonical_kir_cross_block_forwarding_v1(
                refinement.output(),
                inputs.output,
                inputs.forwarding_origins,
                inputs.limits.forwarding,
                b,
            )
            .map_err(Error::Forwarding)
        })?;
        meter.reserve(ps.retained_storage())?;
        retained = retained
            .checked_add(ps.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        meter.work(1)?;
        Ok(History {
            inputs,
            prefix,
            promotion,
            preheaders,
            licm,
            refinement,
            forwarding,
            storage: CanonicalRefinedForwardingHistoryStorageV1(retained),
        })
    })
}
