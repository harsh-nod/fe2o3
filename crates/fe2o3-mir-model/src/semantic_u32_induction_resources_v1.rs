//! Strict original-ledger admission for the complete-CFG legacy V1 report.
//! No V2/SSA-plan, bound-snapshot-family, wire, or compiler-authority conversion.
use super::*;
use std::mem::size_of;

/// Exact resource failure for the strictly metered legacy report.
#[derive(Debug, Eq, PartialEq)]
pub enum SemanticU32InductionMeteredErrorV1<E> {
    /// Existing semantic refusal or independent legacy local limit.
    Analysis(SemanticU32InductionAnalysisErrorV1),
    /// Unmodified original caller denial.
    Meter(E),
    /// Fallible allocation or unexpected capacity.
    Allocation,
    /// Resource arithmetic overflow before allocation/copy/traversal.
    Arithmetic,
}
impl<E: fmt::Display> fmt::Display for SemanticU32InductionMeteredErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(f),
            Self::Meter(error) => error.fmt(f),
            Self::Allocation => f.write_str("u32 induction allocation refused"),
            Self::Arithmetic => f.write_str("u32 induction resource arithmetic overflow"),
        }
    }
}
impl<E: Error + 'static> Error for SemanticU32InductionMeteredErrorV1<E> {}

struct Adapter<'a, M: SemanticU32InductionBoundSnapshotMeterV1> {
    original: &'a mut M,
    failure: Option<SemanticU32InductionMeteredErrorV1<M::Error>>,
}
impl<M: SemanticU32InductionBoundSnapshotMeterV1> bound_snapshot::InternalMeter for Adapter<'_, M> {
    fn strict_resources(&self) -> bool {
        true
    }
    fn strict_failure(&mut self, arithmetic: bool) {
        if self.failure.is_none() {
            self.failure = Some(if arithmetic {
                SemanticU32InductionMeteredErrorV1::Arithmetic
            } else {
                SemanticU32InductionMeteredErrorV1::Allocation
            });
        }
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        if let Err(error) = self.original.charge_work(amount) {
            self.failure = Some(SemanticU32InductionMeteredErrorV1::Meter(error));
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        Ok(())
    }
    fn reserve_storage(
        &mut self,
        amount: usize,
    ) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        if let Err(error) = self.original.reserve_storage(amount) {
            self.failure = Some(SemanticU32InductionMeteredErrorV1::Meter(error));
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        Ok(())
    }
}

pub(super) fn frame_storage_v1<M: SemanticU32InductionBoundSnapshotMeterV1>() -> Option<usize> {
    type Report = SemanticU32InductionNoOverflowReportV1;
    [
        size_of::<Report>(),
        size_of::<SemanticCfgV1>(),
        size_of::<SemanticInventoryV1>(),
        size_of::<WorkBudgetV1<'_>>(),
        size_of::<Adapter<'_, M>>(),
        size_of::<Vec<SemanticU32InductionNoOverflowCertificateV1>>(),
        size_of::<CandidateProofContextV1<'_>>(),
        size_of::<ProvedCandidateV1>(),
        size_of::<Result<Option<ProvedCandidateV1>, SemanticU32InductionAnalysisErrorV1>>(),
        size_of::<Result<Option<ProvedCandidateV1>, SemanticU32InductionAnalysisErrorV1>>(),
        size_of::<
            Result<
                Option<SemanticU32InductionNoOverflowCertificateV1>,
                SemanticU32InductionAnalysisErrorV1,
            >,
        >(),
        size_of::<
            Result<
                Option<SemanticU32InductionNoOverflowCertificateV1>,
                SemanticU32InductionAnalysisErrorV1,
            >,
        >(),
        size_of::<Result<Report, SemanticU32InductionAnalysisErrorV1>>(),
        size_of::<Result<Report, SemanticU32InductionAnalysisErrorV1>>(),
        size_of::<Result<Report, SemanticU32InductionMeteredErrorV1<M::Error>>>(),
        size_of::<Result<Report, SemanticU32InductionMeteredErrorV1<M::Error>>>(),
        size_of::<Result<Report, SemanticU32InductionMeteredErrorV1<M::Error>>>(),
        size_of::<Result<(), M::Error>>(),
        size_of::<M::Error>(),
        size_of::<SemanticU32InductionMeteredErrorV1<M::Error>>(),
        // Only fixed, non-generic iterator/counter/borrow/local helper frames.
        8192,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| total.checked_add(bytes))
}

/// Derives the complete-CFG V1 report using the exact original caller meter.
///
/// Uses the same algorithm, semantic refusals, limits and report work_units as
/// the old V1 entry. Extra allocation/initialization/copy work is external only.
/// The caller owns ALL accepted reservations until reports and partial values
/// drop, including error/unwind: this API never releases/reset/replaces a meter.
/// Meter failures are terminal, never converted into empty certificates.
/// These inert reports retain their existing no-authority semantics.
///
/// Resource admission precedes source lookup/validation. With sufficient caller
/// resources, function lookup and semantic/local refusal ordering match V1.
/// V2/SSA-plan and bound-snapshot facts are not selected by this entry.
pub fn analyze_semantic_u32_induction_no_overflow_with_meter_v1<
    M: SemanticU32InductionBoundSnapshotMeterV1,
>(
    source: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
    meter: &mut M,
) -> Result<SemanticU32InductionNoOverflowReportV1, SemanticU32InductionMeteredErrorV1<M::Error>> {
    let Some(frame) = frame_storage_v1::<M>() else {
        return Err(SemanticU32InductionMeteredErrorV1::Arithmetic);
    };
    let mut adapter = Adapter {
        original: meter,
        failure: None,
    };
    let result = {
        let mut budget = WorkBudgetV1 {
            used: 0,
            limit: limits.work_units,
            meter: Some(&mut adapter),
        };
        (|| {
            budget.strict_storage(frame)?;
            budget.extra(128)?;
            let declaration = source.functions().get(function.index() as usize).ok_or(
                SemanticU32InductionAnalysisErrorV1::InvalidModel(
                    "the requested semantic function is outside the admitted function table",
                ),
            )?;
            analyze_function_in_scope_with_budget_v2(
                source.types(),
                declaration,
                source.semantic_sha256(),
                function,
                None,
                false,
                limits,
                &mut budget,
            )
        })()
    };
    match adapter.failure {
        Some(error) => Err(error),
        None => result.map_err(SemanticU32InductionMeteredErrorV1::Analysis),
    }
}

impl WorkBudgetV1<'_> {
    pub(super) fn strict_resources(&self) -> bool {
        self.meter
            .as_ref()
            .is_some_and(|meter| meter.strict_resources())
    }
    // This MUST NOT touch the old local diagnostic counter or old meter path.
    pub(super) fn extra(
        &mut self,
        amount: usize,
    ) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        if let Some(meter) = &mut self.meter
            && meter.strict_resources()
        {
            meter.charge_work(amount)?;
        }
        Ok(())
    }
    pub(super) fn strict_failure(
        &mut self,
        arithmetic: bool,
    ) -> SemanticU32InductionAnalysisErrorV1 {
        if let Some(meter) = &mut self.meter
            && meter.strict_resources()
        {
            meter.strict_failure(arithmetic);
        }
        SemanticU32InductionAnalysisErrorV1::Storage
    }
    pub(super) fn strict_storage(
        &mut self,
        bytes: usize,
    ) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        if let Some(meter) = &mut self.meter
            && meter.strict_resources()
        {
            meter.reserve_storage(bytes)?;
        }
        Ok(())
    }
    pub(super) fn strict_reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.strict_failure(true))?;
        if requested <= values.capacity() {
            return Ok(());
        }
        let bytes = requested
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.strict_failure(true))?;
        let copy = values
            .len()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.strict_failure(true))?;
        self.extra(copy)?;
        // Retain the old reservation: full new capacity pays coexistence.
        self.strict_storage(bytes)?;
        values
            .try_reserve_exact(additional)
            .map_err(|_| self.strict_failure(false))?;
        // No charge-after-allocation fallback and no initialization/loan of
        // unreserved allocator capacity.
        if size_of::<T>() != 0 && values.capacity() != requested {
            return Err(self.strict_failure(false));
        }
        Ok(())
    }
    pub(super) fn boxed<T>(
        &mut self,
        values: Vec<T>,
    ) -> Result<Box<[T]>, SemanticU32InductionAnalysisErrorV1> {
        if self.strict_resources() && values.len() != values.capacity() {
            let bytes = values
                .len()
                .checked_mul(size_of::<T>())
                .ok_or_else(|| self.strict_failure(true))?;
            self.extra(bytes)?;
            self.strict_storage(bytes)?;
        }
        Ok(values.into_boxed_slice())
    }
}
