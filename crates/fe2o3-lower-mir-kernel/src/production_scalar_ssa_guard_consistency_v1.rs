//! Inert, actual-N guard/control consistency. No source-proof custody or rewrite.
use super::*;
use fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1 as SourceReport;

#[path = "production_scalar_ssa_guard_analysis_v1.rs"]
mod analysis;
#[path = "production_scalar_ssa_bound_snapshot_guard_v1.rs"]
mod bound_snapshot;
pub use bound_snapshot::ProductionU32BoundSnapshotGuardRequestV1;
use bound_snapshot::{GuardCertificate, GuardRecurrence, GuardRequest, QueryOutcome};

/// One requested certificate from an existing typed source report.
/// Requests may be a unique subset, in any order; successful subset analysis
/// never establishes complete source/root/report coverage.
#[derive(Clone, Copy)]
pub struct ProductionU32GuardRequestV1<'report> {
    root: SemanticFunctionIdV1,
    report: &'report SourceReport,
    ordinal: usize,
}
impl<'report> ProductionU32GuardRequestV1<'report> {
    /// Constructs an inert query, not evidence that the requested join exists.
    pub const fn new(
        root: SemanticFunctionIdV1,
        report: &'report SourceReport,
        ordinal: usize,
    ) -> Self {
        Self {
            root,
            report,
            ordinal,
        }
    }
}

/// Unsupported mappings remain separate from contradiction/resource errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionU32GuardUnavailableV1 {
    /// The existing recurrence query could not join this actual capture.
    Recurrence(ProductionScalarSsaEmissionUnavailableV1),
    /// The guard is not the closed single-Compare emission recipe.
    GuardRecipe,
}

/// Fixed original-N coordinates, borrowing the exact retained source owner.
/// This is not a signed proof or authorization to modify any graph.
#[derive(Clone, Copy, Debug)]
pub struct ProductionU32GuardConsistencyFactV1<'source> {
    source: &'source ProductionPreRankedKirOwnerV1,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    ordinal: usize,
    recurrence: Recurrence,
    bound: Definition,
    condition: Definition,
    body: Block,
    exit: Block,
    then_edge: Edge,
    else_edge: Edge,
}
impl ProductionU32GuardConsistencyFactV1<'_> {
    /// Borrows the exact original source/N owner, never E or a later output.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source
    }
    /// Actual selected source root; distinct from physical function ordinal.
    pub fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Actual source function carrying the requested report.
    pub fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Ordinal in the original requested typed report.
    pub fn certificate_ordinal(&self) -> usize {
        self.ordinal
    }
    /// Exact previously checked source/N recurrence.
    pub fn recurrence(&self) -> Recurrence {
        self.recurrence
    }
    /// Actual U32 entry-bound definition in N.
    pub fn bound(&self) -> Definition {
        self.bound
    }
    /// Actual Bool Compare result consumed by the N branch.
    pub fn condition(&self) -> Definition {
        self.condition
    }
    /// Actual true-edge body entry.
    pub fn body(&self) -> Block {
        self.body
    }
    /// Actual false-edge loop exit.
    pub fn exit(&self) -> Block {
        self.exit
    }
    /// True N occurrence, not the source SwitchInt edge ordinal.
    pub fn then_edge(&self) -> Edge {
        self.then_edge
    }
    /// False N occurrence, not the source SwitchInt edge ordinal.
    pub fn else_edge(&self) -> Edge {
        self.else_edge
    }
    /// No fact from this analysis grants compiler transformation authority.
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

/// Requested join result, with unsupported distinct from successful consistency.
#[allow(
    clippy::large_enum_variant,
    reason = "Copy borrowed facts stay inline in prepaid report rows; boxing would add owning allocations and change the receipt contract"
)]
#[derive(Clone, Copy, Debug)]
pub enum ProductionU32GuardConsistencyV1<'source> {
    /// The requested actual capture does not expose the closed recipe.
    Unavailable(ProductionU32GuardUnavailableV1),
    /// The copied semantic claims were rechecked against actual N.
    Joined(ProductionU32GuardConsistencyFactV1<'source>),
}

/// One output in exactly the input request order.
#[derive(Clone, Copy, Debug)]
pub struct ProductionU32GuardRowV1<'source> {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    ordinal: usize,
    outcome: ProductionU32GuardConsistencyV1<'source>,
}
impl<'source> ProductionU32GuardRowV1<'source> {
    /// Actual requested source root.
    pub fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Actual requested source function.
    pub fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Actual requested certificate ordinal.
    pub fn certificate_ordinal(&self) -> usize {
        self.ordinal
    }
    /// Inert checked result for this request only.
    pub fn outcome(&self) -> ProductionU32GuardConsistencyV1<'source> {
        self.outcome
    }
}

/// Header and actual row capacities, excluding borrowed source/report storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionU32GuardStorageV1(usize);
impl ProductionU32GuardStorageV1 {
    /// Reserve while the returned report is retained by a ledger-controlled phase.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only inert result borrowing one genuine C owner. No request/report or
/// temporary CFG/analysis borrow is retained, and there is no raw constructor.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionScalarSsaEmissionOwnerV1,
///     ProductionU32GuardRequestV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn cannot_detach(owner: ProductionScalarSsaEmissionOwnerV1,
///     requests: &[ProductionU32GuardRequestV1<'_>], budget: &mut Budget<'_>) {
///     let (report, _) = owner.analyze_u32_guard_consistency_v1(
///         requests, Default::default(), budget).unwrap();
///     drop(owner);
///     let _ = report.rows();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionU32GuardReportV1;
/// fn cannot_copy(report: ProductionU32GuardReportV1<'_>) {
///     let _ = report.clone();
/// }
/// ```
pub struct ProductionU32GuardReportV1<'source> {
    owner: &'source ProductionScalarSsaEmissionOwnerV1,
    rows: Vec<ProductionU32GuardRowV1<'source>>,
    retained: usize,
}
impl<'source> ProductionU32GuardReportV1<'source> {
    /// Borrows the exact C capture; no copy of N or source is retained.
    pub fn owner(&self) -> &'source ProductionScalarSsaEmissionOwnerV1 {
        self.owner
    }
    /// Exact requested subset in original request order, not full-source coverage.
    pub fn rows(&self) -> &[ProductionU32GuardRowV1<'source>] {
        &self.rows
    }
    /// Complete new report receipt; source/report inputs remain borrowed.
    pub fn storage(&self) -> ProductionU32GuardStorageV1 {
        ProductionU32GuardStorageV1(self.retained)
    }
    /// Always false, including when every requested row is Joined.
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

impl ProductionScalarSsaEmissionOwnerV1 {
    /// Checks a unique requested subset against actual source/SSA/N custody.
    /// Duplicated `(root, function, certificate ordinal)` requests reject.
    /// The source/C receipt must be live on entry; requested report storage is
    /// caller-owned/external. Success transfers only the returned report receipt
    /// UNRESERVED and restores the entry floor. Errors/panics drop scratch before
    /// valid-ledger cleanup. No source no-overflow analyzer or optimizer is run.
    /// When C retained a preexisting SSA occurrence attachment, its separate
    /// receipt must also remain reserved, exactly as for the recurrence query.
    /// Root-qualified helper reports are permitted only with an exact C alias;
    /// this query neither selects a kernel body nor claims complete root coverage.
    /// Existing source replay retains its local-limit domain. All new metadata,
    /// actual-N inventory/loop replay and CFG queries use the supplied live budget.
    ///
    /// A successful subset is not complete report coverage, retained source-proof
    /// custody, signed authority, final-graph transport or rewrite permission.
    pub fn analyze_u32_guard_consistency_v1<'source>(
        &'source self,
        requests: &[ProductionU32GuardRequestV1<'_>],
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(
        ProductionU32GuardReportV1<'source>,
        ProductionU32GuardStorageV1,
    )> {
        analysis::derive(self, requests, limits, budget)
    }
}
