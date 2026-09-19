//! Separate in-process facts for a copied header bound; no legacy evidence conversion.
use super::*;
use std::{convert::Infallible, mem::size_of};

/// Hard cap on accepted coexisting table reservations, not allocator metadata/RSS.
pub const MAX_SEMANTIC_U32_BOUND_SNAPSHOT_STORAGE_V1: usize = 64 * 1024 * 1024;

/// Dependency-neutral live meter. Admission precedes work/allocation, and actual
/// excess capacity is charged before initialization. The caller owns cleanup of
/// all accepted reservations after partial/scratch values drop, including unwind.
pub trait SemanticU32InductionBoundSnapshotMeterV1 {
    /// Original caller resource failure, preserved without translation.
    type Error;
    /// Admits the next logical work before it is performed.
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;
    /// Reserves additional concurrently retained allocation payload.
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error>;
}

/// Analysis or exact external-meter denial, never an absent certificate.
#[derive(Debug, Eq, PartialEq)]
pub enum SemanticU32InductionBoundSnapshotErrorV1<E> {
    /// Closed source analysis, local limit or allocation denial.
    Analysis(SemanticU32InductionAnalysisErrorV1),
    /// Unchanged caller resource error.
    Meter(E),
}
impl<E: fmt::Display> fmt::Display for SemanticU32InductionBoundSnapshotErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(e) => e.fmt(f),
            Self::Meter(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for SemanticU32InductionBoundSnapshotErrorV1<E> {}

/// Exact new-family fact. The immutable entry and actual RHS snapshot are
/// distinct bindings; no conversion to the frozen V1/V2 evidence is provided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionBoundSnapshotCertificateV1 {
    common: SemanticU32InductionNoOverflowCertificateV1,
    guard_bound: SemanticU32InductionPlaceBindingV1,
    bound_snapshot: Option<SemanticU32InductionStatementSiteV1>,
}
macro_rules! common_getter {
    ($name:ident, $ty:ty, $doc:literal) => {
        #[doc = $doc]
        pub const fn $name(self) -> $ty {
            self.common.$name
        }
    };
}
impl SemanticU32InductionBoundSnapshotCertificateV1 {
    common_getter!(
        semantic_mir_sha256,
        InertSemanticMirSha256V1,
        "Exact admitted source identity."
    );
    common_getter!(
        function,
        SemanticFunctionIdV1,
        "Actual selected semantic function."
    );
    common_getter!(
        function_identity,
        SemanticFunctionIdentityV1,
        "Exact function identity."
    );
    common_getter!(
        induction,
        SemanticU32InductionPlaceBindingV1,
        "Actual twice-defined induction local."
    );
    common_getter!(
        guard_induction,
        SemanticU32InductionPlaceBindingV1,
        "Actual ordered left guard operand."
    );
    common_getter!(
        bound,
        SemanticU32InductionPlaceBindingV1,
        "Original immutable entry bound, not the snapshot local."
    );
    common_getter!(
        predicate,
        SemanticU32InductionPlaceBindingV1,
        "Exact Bool guard result."
    );
    common_getter!(
        checked_result,
        SemanticU32InductionPlaceBindingV1,
        "Exact checked-add tuple."
    );
    common_getter!(
        preheader,
        SemanticU32InductionBlockSiteV1,
        "Actual acyclic initialization predecessor."
    );
    common_getter!(
        header,
        SemanticU32InductionBlockSiteV1,
        "Actual source loop header."
    );
    common_getter!(
        body_entry,
        SemanticU32InductionBlockSiteV1,
        "Actual guard-true successor."
    );
    common_getter!(
        exit,
        SemanticU32InductionBlockSiteV1,
        "Actual guard-false successor."
    );
    common_getter!(
        initialization,
        SemanticU32InductionStatementSiteV1,
        "Exact initial zero assignment."
    );
    common_getter!(
        guard_induction_snapshot,
        Option<SemanticU32InductionStatementSiteV1>,
        "Optional left-operand header snapshot; never the bound snapshot."
    );
    common_getter!(
        guard,
        SemanticU32InductionStatementSiteV1,
        "Exact strict LessThan statement."
    );
    common_getter!(
        checked_addition,
        SemanticU32InductionStatementSiteV1,
        "Exact checked addition."
    );
    common_getter!(
        update,
        SemanticU32InductionStatementSiteV1,
        "Exact tuple-value writeback."
    );
    /// Actual ordered guard RHS binding, independently distinct from Entry.
    pub const fn guard_bound(self) -> SemanticU32InductionPlaceBindingV1 {
        self.guard_bound
    }
    /// Exact Copy statement when the RHS is a header snapshot.
    pub const fn bound_snapshot(self) -> Option<SemanticU32InductionStatementSiteV1> {
        self.bound_snapshot
    }
    /// Inert semantic fact only, not source/optimizer/native authority.
    pub const fn grants_authority(self) -> bool {
        false
    }
    /// A separate source/SSA/N join and production proof remain mandatory.
    pub const fn authorizes_compiler_transform(self) -> bool {
        false
    }
}

/// Owning new-family report, independent of frozen V1/V2 reports and encodings.
/// The caller must retain its capacity-derived reservation while this report
/// lives. The report neither releases nor owns the caller's meter. No wire or
/// from-parts API is provided.
#[derive(Debug, Eq, PartialEq)]
pub struct SemanticU32InductionBoundSnapshotReportV1 {
    semantic_mir_sha256: InertSemanticMirSha256V1,
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    checked_additions_examined: usize,
    certificates: Vec<SemanticU32InductionBoundSnapshotCertificateV1>,
    work_units: usize,
    retained: usize,
}
impl SemanticU32InductionBoundSnapshotReportV1 {
    /// Exact admitted source identity, not independently authenticated custody.
    pub const fn semantic_mir_sha256(&self) -> InertSemanticMirSha256V1 {
        self.semantic_mir_sha256
    }
    /// Actual selected source body.
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Exact original function identity.
    pub const fn function_identity(&self) -> SemanticFunctionIdentityV1 {
        self.function_identity
    }
    /// All checked-Add occurrences examined, including unsupported candidates.
    pub const fn checked_additions_examined(&self) -> usize {
        self.checked_additions_examined
    }
    /// Privately established, source-ordered facts in the new family only.
    pub fn certificates(&self) -> &[SemanticU32InductionBoundSnapshotCertificateV1] {
        &self.certificates
    }
    /// Accepted local logical work; the external meter was charged before work.
    pub const fn work_units(&self) -> usize {
        self.work_units
    }
    /// Actual report header plus retained vector capacity, excluding source.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Always false; no artifact/source proof or compiler authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// The new report cannot authorize a rewrite or bypass production evidence.
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

pub(super) trait InternalMeter {
    fn charge_work(&mut self, amount: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1>;
    fn reserve_storage(&mut self, amount: usize)
    -> Result<(), SemanticU32InductionAnalysisErrorV1>;
}
struct Adapter<'a, M: SemanticU32InductionBoundSnapshotMeterV1> {
    meter: &'a mut M,
    error: Option<M::Error>,
    storage: usize,
}
impl<M: SemanticU32InductionBoundSnapshotMeterV1> InternalMeter for Adapter<'_, M> {
    fn charge_work(&mut self, amount: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        if let Err(error) = self.meter.charge_work(amount) {
            self.error = Some(error);
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        Ok(())
    }
    fn reserve_storage(
        &mut self,
        amount: usize,
    ) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        let next = self
            .storage
            .checked_add(amount)
            .filter(|n| *n <= MAX_SEMANTIC_U32_BOUND_SNAPSHOT_STORAGE_V1)
            .ok_or(SemanticU32InductionAnalysisErrorV1::Storage)?;
        if let Err(error) = self.meter.reserve_storage(amount) {
            self.error = Some(error);
            return Err(SemanticU32InductionAnalysisErrorV1::Storage);
        }
        self.storage = next;
        Ok(())
    }
}
struct LocalMeter;
impl SemanticU32InductionBoundSnapshotMeterV1 for LocalMeter {
    type Error = Infallible;
    fn charge_work(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn reserve_storage(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}

/// Derives only the new in-process family, with fixed complete-CFG scope.
/// This does not change old direct-entry analysis or any portable evidence.
pub fn analyze_semantic_u32_induction_bound_snapshots_v1(
    source: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<SemanticU32InductionBoundSnapshotReportV1, SemanticU32InductionAnalysisErrorV1> {
    analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
        source,
        function,
        SemanticU32InductionAnalysisLimitsV1::default(),
        &mut LocalMeter,
    )
    .map_err(|error| match error {
        SemanticU32InductionBoundSnapshotErrorV1::Analysis(error) => error,
        SemanticU32InductionBoundSnapshotErrorV1::Meter(error) => match error {},
    })
}

/// New-family derivation with complete live work/capacity admission. Accepted
/// allocation reservations stay with the caller until scratch/partial values
/// drop. This conservatively retains every accepted allocation and reallocation
/// overlap until return, even when a temporary has already dropped. On success
/// preserve the report's exact receipt when restoring the caller floor. Old
/// analysis/evidence are never selected here.
pub fn analyze_semantic_u32_induction_bound_snapshots_with_meter_v1<
    M: SemanticU32InductionBoundSnapshotMeterV1,
>(
    source: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
    meter: &mut M,
) -> Result<
    SemanticU32InductionBoundSnapshotReportV1,
    SemanticU32InductionBoundSnapshotErrorV1<M::Error>,
> {
    let mut adapter = Adapter {
        meter,
        error: None,
        storage: 0,
    };
    let result = derive(source, function, limits, &mut adapter);
    if let Some(error) = adapter.error {
        Err(SemanticU32InductionBoundSnapshotErrorV1::Meter(error))
    } else {
        result.map_err(SemanticU32InductionBoundSnapshotErrorV1::Analysis)
    }
}

fn derive(
    source: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
    meter: &mut dyn InternalMeter,
) -> Result<SemanticU32InductionBoundSnapshotReportV1, SemanticU32InductionAnalysisErrorV1> {
    if limits.work_units > MAX_SEMANTIC_U32_INDUCTION_WORK_V1
        || limits.certificates > MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1
    {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits {
            requested_work: limits.work_units,
            maximum_work: MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
            requested_certificates: limits.certificates,
            maximum_certificates: MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1,
        });
    }
    let declaration = source.functions().get(function.index() as usize).ok_or(
        SemanticU32InductionAnalysisErrorV1::InvalidModel("snapshot function outside source"),
    )?;
    meter.reserve_storage(
        size_of::<SemanticU32InductionBoundSnapshotReportV1>()
            + size_of::<SemanticCfgV1>()
            + size_of::<SemanticInventoryV1>()
            + size_of::<LifetimeIndex>()
            + size_of::<WorkBudgetV1<'_>>(),
    )?;
    let mut budget = WorkBudgetV1 {
        used: 0,
        limit: limits.work_units,
        meter: Some(meter),
    };
    let mut reachable_count = 0;
    let graph =
        SemanticCfgV1::analyze(declaration, false, None, &mut budget, &mut reachable_count)?;
    let inventory = SemanticInventoryV1::analyze(declaration, &graph, &mut budget)?;
    let lifetimes = LifetimeIndex::derive(declaration, &mut budget)?;
    let mut certificates = Vec::new();
    budget.reserve_vec(
        &mut certificates,
        inventory.checked_additions.len().min(limits.certificates),
        true,
    )?;
    let context = CandidateProofContextV1 {
        types: source.types(),
        function: declaration,
        semantic_mir_sha256: source.semantic_sha256(),
        function_id: function,
        graph: &graph,
        inventory: &inventory,
    };
    for candidate in &inventory.checked_additions {
        budget.charge(1)?;
        if let Some(proved) = prove_candidate_with_bound_v1(
            &context,
            *candidate,
            BoundResolverV1::Snapshot(&lifetimes),
            &mut budget,
        )? {
            let actual = certificates.len().saturating_add(1);
            if actual > limits.certificates {
                return Err(SemanticU32InductionAnalysisErrorV1::CertificateLimit {
                    actual,
                    limit: limits.certificates,
                });
            }
            if certificates.len() == certificates.capacity() {
                return Err(SemanticU32InductionAnalysisErrorV1::Storage);
            }
            certificates.push(SemanticU32InductionBoundSnapshotCertificateV1 {
                common: proved.certificate,
                guard_bound: proved.guard_bound,
                bound_snapshot: proved.bound_snapshot,
            });
        }
    }
    let retained = certificates
        .capacity()
        .checked_mul(size_of::<SemanticU32InductionBoundSnapshotCertificateV1>())
        .and_then(|bytes| bytes.checked_add(size_of::<SemanticU32InductionBoundSnapshotReportV1>()))
        .ok_or(SemanticU32InductionAnalysisErrorV1::Storage)?;
    Ok(SemanticU32InductionBoundSnapshotReportV1 {
        semantic_mir_sha256: source.semantic_sha256(),
        function,
        function_identity: declaration.identity(),
        checked_additions_examined: inventory.checked_additions.len(),
        certificates,
        work_units: budget.used,
        retained,
    })
}

#[derive(Clone, Copy, Default)]
struct Life {
    lives: usize,
    live: Option<DefinitionSiteV1>,
    dead_count: usize,
    dead: [Option<DefinitionSiteV1>; 2],
}
pub(super) struct LifetimeIndex {
    locals: Vec<Life>,
}
impl LifetimeIndex {
    fn derive(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1<'_>,
    ) -> Result<Self, SemanticU32InductionAnalysisErrorV1> {
        let mut locals: Vec<Life> = budget.filled(function.locals().len(), Life::default())?;
        for (block, body) in function.blocks().iter().enumerate() {
            budget.charge(1)?;
            for (statement, value) in body.statements().iter().enumerate() {
                budget.charge(1)?;
                let (local, live) = match value.kind() {
                    SemanticStatementKindV1::StorageLive(local) => (*local, true),
                    SemanticStatementKindV1::StorageDead(local) => (*local, false),
                    _ => continue,
                };
                let row = locals.get_mut(local.index() as usize).ok_or(
                    SemanticU32InductionAnalysisErrorV1::InvalidModel(
                        "lifetime local outside source",
                    ),
                )?;
                let site = DefinitionSiteV1 {
                    block,
                    position: DefinitionPositionV1::Statement(statement),
                };
                if live {
                    row.lives = row.lives.saturating_add(1);
                    if row.live.is_none() {
                        row.live = Some(site);
                    }
                } else {
                    if row.dead_count < 2 {
                        row.dead[row.dead_count] = Some(site);
                    }
                    row.dead_count = row.dead_count.saturating_add(1);
                }
            }
        }
        Ok(Self { locals })
    }
}

pub(super) fn resolve(
    context: &CandidateProofContextV1<'_>,
    header: usize,
    guard: usize,
    place: &SemanticPlaceV1,
    lifetimes: &LifetimeIndex,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<
    Option<(SemanticLocalIdV1, Option<DefinitionSiteV1>)>,
    SemanticU32InductionAnalysisErrorV1,
> {
    let function = context.function;
    let inventory = context.inventory;
    let snapshot = place.local();
    let declaration = &function.locals()[snapshot.index() as usize];
    budget.charge(4)?;
    if declaration.role().is_entry_argument() {
        let life = lifetimes.locals[snapshot.index() as usize];
        return Ok((life.lives == 0 && life.dead_count == 0).then_some((snapshot, None)));
    }
    let Some(site) = definition(inventory, snapshot)?.first else {
        return Ok(None);
    };
    let DefinitionPositionV1::Statement(statement) = site.position else {
        return Ok(None);
    };
    if site.block != header
        || statement >= guard
        || declaration.role() != SemanticLocalRoleV1::Temporary
        || declaration.ty() != place.ty()
        || !is_exact_u32(context.types, place.ty())
        || !definition(inventory, snapshot)?.is_unique_at(site)
        || use_count(inventory, snapshot)? != 1
        || local(&inventory.address_or_projection_hazard, snapshot)?
        || local(&inventory.direct_copy_alias, snapshot)?
    {
        return Ok(None);
    }
    budget.charge(10)?;
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[header].statements()[statement].kind()
    else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(entry)) = assignment.value().kind()
    else {
        return Ok(None);
    };
    if !is_exact_destination(assignment.destination(), snapshot, place.ty())
        || assignment.value().result_type() != place.ty()
        || !entry.projections().is_empty()
        || entry.ty() != place.ty()
        || entry.local() == snapshot
    {
        return Ok(None);
    }
    let bound = entry.local();
    let bound_decl = &function.locals()[bound.index() as usize];
    let life = lifetimes.locals[bound.index() as usize];
    if bound_decl.ty() != place.ty()
        || !bound_decl.role().is_entry_argument()
        || definition(inventory, bound)?.count != 0
        || use_count(inventory, bound)? != 1
        || local(&inventory.address_or_projection_hazard, bound)?
        || life.lives != 0
        || life.dead_count != 0
    {
        return Ok(None);
    }
    Ok(Some((bound, Some(site))))
}

pub(super) fn lifetime_matches(
    lifetimes: &LifetimeIndex,
    local: SemanticLocalIdV1,
    definition: DefinitionSiteV1,
    header: usize,
    body: usize,
    exit: usize,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<bool, SemanticU32InductionAnalysisErrorV1> {
    budget.charge(5)?;
    let life = lifetimes.locals[local.index() as usize];
    if life.lives == 0 && life.dead_count == 0 {
        return Ok(true);
    }
    let Some(live) = life.live else {
        return Ok(false);
    };
    let (DefinitionPositionV1::Statement(live_statement), DefinitionPositionV1::Statement(defined)) =
        (live.position, definition.position)
    else {
        return Ok(false);
    };
    if life.lives != 1 || live.block != header || live_statement >= defined || life.dead_count != 2
    {
        return Ok(false);
    }
    let mut prior = None;
    for dead in life.dead.into_iter().flatten() {
        budget.charge(1)?;
        if (dead.block != body && dead.block != exit) || prior == Some(dead.block) {
            return Ok(false);
        }
        prior = Some(dead.block);
    }
    Ok(true)
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[derive(Default)]
    struct Count {
        work: usize,
        storage: usize,
    }
    impl SemanticU32InductionBoundSnapshotMeterV1 for Count {
        type Error = Infallible;
        fn charge_work(&mut self, n: usize) -> Result<(), Infallible> {
            self.work += n;
            Ok(())
        }
        fn reserve_storage(&mut self, n: usize) -> Result<(), Infallible> {
            self.storage += n;
            Ok(())
        }
    }

    #[test]
    fn real_capacity_is_prepaid_and_failed_count_arithmetic_never_allocates() {
        let mut count = Count::default();
        let mut adapter = Adapter {
            meter: &mut count,
            error: None,
            storage: 0,
        };
        let mut values = Vec::<u64>::new();
        {
            let mut work = WorkBudgetV1 {
                used: 0,
                limit: usize::MAX,
                meter: Some(&mut adapter),
            };
            work.reserve_vec(&mut values, 3, false).unwrap();
            let original = values.capacity();
            values.extend_from_slice(&[1, 2, 3]);
            assert!(work.meter.is_some());
            assert!(work.reserve_vec(&mut values, usize::MAX, true).is_err());
            assert_eq!(values.capacity(), original);
        }
        assert_eq!(adapter.storage, values.capacity() * size_of::<u64>());
        let accepted = adapter.storage;
        assert!(adapter.reserve_storage(usize::MAX).is_err());
        assert_eq!(adapter.storage, accepted);
        assert!(
            adapter
                .reserve_storage(MAX_SEMANTIC_U32_BOUND_SNAPSHOT_STORAGE_V1)
                .is_err()
        );
        assert_eq!(adapter.storage, accepted);
    }

    #[test]
    fn legacy_allocations_keep_the_previous_charge_sequence_without_a_meter() {
        let mut work = WorkBudgetV1::new(0);
        let mut values: Vec<u64> = work.filled(3, 0).unwrap();
        let _: Vec<Vec<usize>> = work.nested(5).unwrap();
        work.reserve_vec(&mut values, 4, false).unwrap();
        assert_eq!(work.used, 0);
        assert_eq!(
            work.charge(5),
            Err(SemanticU32InductionAnalysisErrorV1::WorkLimit {
                actual: 5,
                limit: 0
            })
        );
        assert_eq!(work.used, 5);
    }
}
