//! New-family guard entry; frozen legacy requests never select report rederivation.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirSha256V1, SemanticFunctionIdentityV1};
use fe2o3_mir_model::{
    SemanticU32InductionBlockSiteV1 as BlockSite,
    SemanticU32InductionBoundSnapshotCertificateV1 as SnapshotCertificate,
    SemanticU32InductionBoundSnapshotErrorV1 as SnapshotError,
    SemanticU32InductionBoundSnapshotMeterV1 as SnapshotMeter,
    SemanticU32InductionBoundSnapshotReportV1 as SnapshotReport,
    SemanticU32InductionPlaceBindingV1 as PlaceBinding,
    SemanticU32InductionStatementSiteV1 as StatementSite,
    analyze_semantic_u32_induction_bound_snapshots_with_meter_v1,
};

#[cfg(test)]
#[path = "production_scalar_ssa_bound_snapshot_guard_v1_tests.rs"]
mod tests;

/// An inert request from the separate source bound-snapshot report family.
/// This does not convert into a legacy no-overflow report or portable evidence.
#[derive(Clone, Copy)]
pub struct ProductionU32BoundSnapshotGuardRequestV1<'report> {
    root: SemanticFunctionIdV1,
    report: &'report SnapshotReport,
    ordinal: usize,
}
impl<'report> ProductionU32BoundSnapshotGuardRequestV1<'report> {
    /// Binds actual root/report/ordinal; all custody and recipe checks remain ahead.
    pub const fn new(
        root: SemanticFunctionIdV1,
        report: &'report SnapshotReport,
        ordinal: usize,
    ) -> Self {
        Self {
            root,
            report,
            ordinal,
        }
    }
}

// These private subjects preserve the original typed input, not an old-family
// certificate reconstructed from new fields. They cannot be selected by callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GuardCertificate {
    Legacy(Certificate),
    Snapshot(SnapshotCertificate),
}
macro_rules! getter {
    ($name:ident, $ty:ty) => {
        pub(super) fn $name(self) -> $ty {
            match self {
                Self::Legacy(fact) => fact.$name(),
                Self::Snapshot(fact) => fact.$name(),
            }
        }
    };
}
impl GuardCertificate {
    getter!(function, SemanticFunctionIdV1);
    getter!(function_identity, SemanticFunctionIdentityV1);
    getter!(semantic_mir_sha256, InertSemanticMirSha256V1);
    getter!(induction, PlaceBinding);
    getter!(guard_induction, PlaceBinding);
    getter!(bound, PlaceBinding);
    getter!(predicate, PlaceBinding);
    getter!(header, BlockSite);
    getter!(body_entry, BlockSite);
    getter!(exit, BlockSite);
    getter!(guard, StatementSite);
    getter!(guard_induction_snapshot, Option<StatementSite>);
    pub(super) fn guard_bound(self) -> PlaceBinding {
        match self {
            Self::Legacy(fact) => fact.bound(),
            Self::Snapshot(fact) => fact.guard_bound(),
        }
    }
    pub(super) fn bound_snapshot(self) -> Option<StatementSite> {
        match self {
            Self::Legacy(_) => None,
            Self::Snapshot(fact) => fact.bound_snapshot(),
        }
    }
}
#[allow(
    clippy::large_enum_variant,
    reason = "Closed borrowed facts preserve allocation-free legacy layout"
)]
pub(super) enum GuardRecurrence<'s> {
    Legacy(ProductionU32RecurrenceConsistencyFactV1<'s>),
    Snapshot(ProductionU32BoundSnapshotRecurrenceFactV1<'s>),
}
impl<'s> GuardRecurrence<'s> {
    pub(super) fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        match self {
            Self::Legacy(fact) => fact.source(),
            Self::Snapshot(fact) => fact.source(),
        }
    }
    pub(super) fn root(&self) -> SemanticFunctionIdV1 {
        match self {
            Self::Legacy(fact) => fact.root(),
            Self::Snapshot(fact) => fact.root(),
        }
    }
    pub(super) fn certificate(&self) -> GuardCertificate {
        match self {
            Self::Legacy(fact) => GuardCertificate::Legacy(fact.certificate()),
            Self::Snapshot(fact) => GuardCertificate::Snapshot(fact.certificate()),
        }
    }
    pub(super) fn recurrence(&self) -> Recurrence {
        match self {
            Self::Legacy(fact) => fact.recurrence(),
            Self::Snapshot(fact) => fact.recurrence(),
        }
    }
    pub(super) fn authorizes_compiler_transform(&self) -> bool {
        match self {
            Self::Legacy(fact) => fact.authorizes_compiler_transform(),
            Self::Snapshot(fact) => fact.authorizes_compiler_transform(),
        }
    }
}
#[allow(
    clippy::large_enum_variant,
    reason = "Closed borrowed facts preserve allocation-free legacy layout"
)]
pub(super) enum QueryOutcome<'s> {
    Unavailable(ProductionScalarSsaEmissionUnavailableV1),
    Joined(GuardRecurrence<'s>),
}

pub(super) trait GuardRequest: Copy {
    fn root(self) -> SemanticFunctionIdV1;
    fn function(self) -> SemanticFunctionIdV1;
    fn function_identity(self) -> SemanticFunctionIdentityV1;
    fn semantic(self) -> InertSemanticMirSha256V1;
    fn ordinal(self) -> usize;
    fn certificate(self) -> Option<GuardCertificate>;
    fn grants_authority(self) -> bool;
    fn authorizes_compiler_transform(self) -> bool;
    fn query<'s>(
        self,
        query: &mut ProductionScalarSsaEmissionQueryV1<'_, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<QueryOutcome<'s>>;
}
macro_rules! request_fields {
    () => {
        fn root(self) -> SemanticFunctionIdV1 {
            self.root
        }
        fn function(self) -> SemanticFunctionIdV1 {
            self.report.function()
        }
        fn function_identity(self) -> SemanticFunctionIdentityV1 {
            self.report.function_identity()
        }
        fn semantic(self) -> InertSemanticMirSha256V1 {
            self.report.semantic_mir_sha256()
        }
        fn ordinal(self) -> usize {
            self.ordinal
        }
        fn grants_authority(self) -> bool {
            self.report.grants_authority()
        }
        fn authorizes_compiler_transform(self) -> bool {
            self.report.authorizes_compiler_transform()
        }
    };
}
impl GuardRequest for ProductionU32GuardRequestV1<'_> {
    request_fields!();
    fn certificate(self) -> Option<GuardCertificate> {
        self.report
            .certificates()
            .get(self.ordinal)
            .copied()
            .map(GuardCertificate::Legacy)
    }
    fn query<'s>(
        self,
        query: &mut ProductionScalarSsaEmissionQueryV1<'_, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<QueryOutcome<'s>> {
        Ok(
            match query.check_u32_certificate_v1(
                self.root,
                &self.report.certificates()[self.ordinal],
                budget,
            )? {
                ProductionU32RecurrenceConsistencyV1::Unavailable(reason) => {
                    QueryOutcome::Unavailable(reason)
                }
                ProductionU32RecurrenceConsistencyV1::Joined(fact) => {
                    QueryOutcome::Joined(GuardRecurrence::Legacy(fact))
                }
            },
        )
    }
}
impl GuardRequest for ProductionU32BoundSnapshotGuardRequestV1<'_> {
    request_fields!();
    fn certificate(self) -> Option<GuardCertificate> {
        self.report
            .certificates()
            .get(self.ordinal)
            .copied()
            .map(GuardCertificate::Snapshot)
    }
    fn query<'s>(
        self,
        query: &mut ProductionScalarSsaEmissionQueryV1<'_, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<QueryOutcome<'s>> {
        Ok(
            match query.check_u32_bound_snapshot_certificate_v1(
                self.root,
                &self.report.certificates()[self.ordinal],
                budget,
            )? {
                ProductionU32BoundSnapshotRecurrenceV1::Unavailable(reason) => {
                    QueryOutcome::Unavailable(reason)
                }
                ProductionU32BoundSnapshotRecurrenceV1::Joined(fact) => {
                    QueryOutcome::Joined(GuardRecurrence::Snapshot(fact))
                }
            },
        )
    }
}

struct Meter<'a, 'w>(&'a mut Budget<'w>);
impl SnapshotMeter for Meter<'_, '_> {
    type Error = Resource;
    fn charge_work(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.charge_work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.reserve_storage(amount)?;
        #[cfg(test)]
        tests::after_reservation(self.0)?;
        Ok(())
    }
}
fn model_error(error: SnapshotError<Resource>) -> Error {
    match error {
        SnapshotError::Analysis(error) => Error::BoundSnapshotAnalysis(error),
        SnapshotError::Meter(error) => Error::Resource(error),
    }
}

impl ProductionScalarSsaEmissionOwnerV1 {
    /// Rederives the new source family once per requested actual function, then
    /// joins its exact Statement-to-Entry Copy and ordered N guard/control.
    /// All distinct input reports must already be reserved alongside the source;
    /// scratch cannot stand in for their incoming reservation. Equal requests
    /// remain duplicate errors. The existing inert guard-coordinate output is
    /// transferred UNRESERVED with its receipt; it contains no old/new source
    /// report conversion or portable proof. All original guard exclusions remain.
    pub fn analyze_u32_bound_snapshot_guard_consistency_v1<'s>(
        &'s self,
        requests: &[ProductionU32BoundSnapshotGuardRequestV1<'_>],
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(ProductionU32GuardReportV1<'s>, ProductionU32GuardStorageV1)> {
        let inherited = analysis::require_inherited(self, budget)?;
        budget.charge_work(1)?;
        if requests.len() > MAX_ROWS || requests.len() > limits.rows {
            return Err(Error::Limit);
        }
        let entry = budget.storage();
        resources::scoped(budget, |budget| {
            budget.reserve_storage(size_of::<Vec<(u32, usize, usize)>>())?;
            let mut index = Vec::new();
            reserve(&mut index, requests.len(), budget)?;
            for (ordinal, request) in requests.iter().enumerate() {
                analysis::check_request(self, *request, budget)?;
                analysis::push(
                    &mut index,
                    (
                        request.function().index(),
                        request.report as *const SnapshotReport as usize,
                        ordinal,
                    ),
                    budget,
                )?;
            }
            resources::sort_work(index.len(), budget)?;
            index.sort_unstable();
            let mut required = inherited;
            let mut previous = None;
            for &(function, pointer, ordinal) in &index {
                budget.charge_work(2)?;
                if previous != Some((function, pointer)) {
                    required = required
                        .checked_add(requests[ordinal].report.retained_storage())
                        .ok_or(Resource::Arithmetic)?;
                    previous = Some((function, pointer));
                }
            }
            if entry < required {
                return Err(Resource::Accounting.into());
            }
            let mut start = 0;
            while start < index.len() {
                budget.charge_work(1)?;
                let function = index[start].0;
                let mut end = start + 1;
                while end < index.len() {
                    budget.charge_work(1)?;
                    if index[end].0 != function {
                        break;
                    }
                    end += 1;
                }
                let scratch_floor = budget.storage();
                let fresh = analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
                    self.original().semantic_ssa().source_semantic(),
                    SemanticFunctionIdV1::from_index(function),
                    Default::default(),
                    &mut Meter(budget),
                )
                .map_err(model_error)?;
                #[cfg(test)]
                tests::derived();
                let mut prior_pointer = None;
                for &(_, pointer, ordinal) in &index[start..end] {
                    budget.charge_work(1)?;
                    if prior_pointer == Some(pointer) {
                        continue;
                    }
                    prior_pointer = Some(pointer);
                    let expected = requests[ordinal].report;
                    let bytes = expected
                        .certificates()
                        .len()
                        .checked_mul(size_of::<SnapshotCertificate>())
                        .and_then(|n| n.checked_add(size_of::<SnapshotReport>()))
                        .ok_or(Resource::Arithmetic)?;
                    budget.charge_work(bytes)?;
                    if expected != &fresh {
                        return Err(Error::Mismatch(
                            "new-family exact same-source report replay",
                        ));
                    }
                }
                drop(fresh);
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(scratch_floor)
                        .ok_or(Resource::Accounting)?,
                )?;
                start = end;
            }
            Ok(())
        })?;
        analysis::derive(self, requests, limits, budget)
    }
}
