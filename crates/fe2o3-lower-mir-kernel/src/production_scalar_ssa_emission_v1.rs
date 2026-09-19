//! Opt-in emission custody for an inert source/SSA/N recurrence consistency query.
//! This does not select a backend, reproduce the source no-overflow proof, or
//! authorize an optimization. The legacy materializer and owner stay unchanged.
//! Generic rows establish genuine emission custody and coverage, not a source
//! expression equivalence theorem. The existing production source proof/report
//! replay remains mandatory independently of this additional inert query.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CanonicalKirLoopErrorV1, CanonicalKirLoopLimitsV1,
    CanonicalKirLoopsV1 as Loops, CanonicalKirRecurrenceV1 as Recurrence,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeCoordinateV1 as Edge, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
};
use fe2o3_mir_model::{
    SemanticU32InductionNoOverflowCertificateV1 as Certificate, SsaEdgeIdV1, SsaResolvedEventV1,
    SsaVariableIdV1,
};
use fe2o3_pliron::{
    ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOccurrenceStorageV1,
};
use std::{
    mem::size_of,
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "production_scalar_ssa_emission_capture_v1.rs"]
mod capture;
#[path = "production_scalar_ssa_guard_consistency_v1.rs"]
mod guard_consistency;
#[path = "production_scalar_ssa_emission_query_v1.rs"]
mod query;
#[path = "production_scalar_ssa_emission_replay_v1.rs"]
mod replay;
#[path = "production_scalar_ssa_emission_resources_v1.rs"]
mod resources;
pub use guard_consistency::{
    ProductionU32BoundSnapshotGuardRequestV1, ProductionU32GuardConsistencyFactV1,
    ProductionU32GuardConsistencyV1, ProductionU32GuardReportV1, ProductionU32GuardRequestV1,
    ProductionU32GuardRowV1, ProductionU32GuardStorageV1, ProductionU32GuardUnavailableV1,
};
#[cfg(test)]
#[path = "production_scalar_ssa_emission_v1_tests.rs"]
mod tests;
pub use query::{
    ProductionScalarSsaEmissionQueryV1, ProductionU32BoundSnapshotRecurrenceFactV1,
    ProductionU32BoundSnapshotRecurrenceV1, ProductionU32RecurrenceConsistencyFactV1,
    ProductionU32RecurrenceConsistencyV1,
};
use resources::{append, charge_lookup, reserve, table_bytes};

const MAX_ROWS: usize = 1_048_576;

/// Capture or consistency failure, separate from an unsupported mapping.
#[derive(Debug)]
pub enum ProductionScalarSsaEmissionErrorV1 {
    /// The live verification ledger rejected work, storage, or accounting.
    Resource(Resource),
    /// The genuine PreRanked materializer rejected the input.
    Materialization(ProductionPreRankedKirErrorV1),
    /// Capturing the actual source SSA occurrence inventory failed.
    Occurrences(fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1),
    /// Replaying the retained source SSA owner failed.
    Source(fe2o3_pliron::ProductionSemanticSsaErrorV1),
    /// Deriving or checking the actual canonical N inventory failed.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// Deriving or independently replaying canonical loop facts failed.
    Loops(CanonicalKirLoopErrorV1),
    /// The separate bound-snapshot source analyzer refused its exact model or limit.
    BoundSnapshotAnalysis(fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1),
    /// A claimed supported emission row or consistency relation disagreed.
    Mismatch(&'static str),
    /// The bounded emission inventory exceeded its row limit.
    Limit,
    /// A protected constructor, setup, or callback operation unwound.
    Panicked,
}
type Error = ProductionScalarSsaEmissionErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalKirLoopErrorV1> for Error {
    fn from(value: CanonicalKirLoopErrorV1) -> Self {
        Self::Loops(value)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1> for Error {
    fn from(value: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scalar SSA emission consistency: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    Scalar(ScalarType),
    CheckedAdd(ScalarType),
}
impl Shape {
    fn width(self) -> usize {
        match self {
            Self::Scalar(_) => 1,
            Self::CheckedAdd(_) => 2,
        }
    }
    fn scalar(self, component: usize) -> ScalarType {
        match (self, component) {
            (Self::CheckedAdd(_), 1) => ScalarType::Bool,
            (Self::Scalar(ty) | Self::CheckedAdd(ty), _) => ty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSite {
    Entry,
    Header(u32),
    Event(Site),
    Edge(SsaEdgeIdV1),
}
impl SourceSite {
    fn key(self) -> (u8, u32, u32) {
        match self {
            Self::Entry => (0, 0, 0),
            Self::Header(block) => (1, block, 0),
            Self::Event(Site::Statement { block, statement }) => (2, block.get(), statement),
            Self::Event(Site::Terminator { block }) => (3, block.get(), 0),
            Self::Edge(edge) => (4, edge.source().get(), edge.ordinal()),
        }
    }
}

/// One supported source definition. This roster is prepared independently of
/// the emitted values and replayed from actual source/SSA occurrences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Expected {
    function: SemanticFunctionIdV1,
    value: SsaValueV1,
    variable: SsaVariableIdV1,
    site: SourceSite,
    shape: Shape,
}
impl Expected {
    fn key(self) -> (u32, SsaValueV1) {
        (self.function.index(), self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EmittedDefinition {
    expected: usize,
    values: [ValueId; 2],
    definitions: [Option<Definition>; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EmittedEdge {
    source: SsaEdgeIdV1,
    target: u32,
    variable: SsaVariableIdV1,
    incoming: SsaValueV1,
    raw_source: BlockId,
    raw_target: BlockId,
    argument: u32,
    value: ValueId,
    edge: Option<Edge>,
}

#[derive(Debug)]
struct EmittedFunction {
    root: SemanticFunctionIdV1,
    source: SemanticFunctionIdV1,
    name: String,
    coordinate: Option<FunctionCoordinate>,
    unsupported_placement: bool,
    definitions: Range<usize>,
    edges: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Alias {
    root: SemanticFunctionIdV1,
    source: SemanticFunctionIdV1,
    emitted: usize,
}
impl Alias {
    fn key(self) -> (u32, u32) {
        (self.root.index(), self.source.index())
    }
}

/// Used only by the actual opt-in lowering path. No raw-parts constructor is
/// public and no test observation or reconstructed graph can fill these rows.
#[derive(Debug)]
pub(super) struct Recorder {
    expected: Vec<Expected>,
    functions: Vec<EmittedFunction>,
    definitions: Vec<EmittedDefinition>,
    edges: Vec<EmittedEdge>,
}

#[derive(Debug)]
struct Sealed {
    capture: Recorder,
    aliases: Vec<Alias>,
    statements: Vec<StatementIndex>,
    sites: Vec<DefinitionSiteIndex>,
    first_operands: Vec<FirstOperandIndex>,
    retained: usize,
}

/// Index of actual promoted BaseUse events at RvalueOperand(0), not an inferred
/// expression. C uses these exact source SSA reads for checked Add and update.
#[derive(Clone, Copy, Debug)]
struct FirstOperandIndex {
    key: (u32, u32, u32),
    ordinal: usize,
}

#[derive(Clone, Copy, Debug)]
struct DefinitionSiteIndex {
    key: (u32, u8, u32, u32, u32),
    expected: usize,
}
impl Expected {
    fn site_key(self) -> (u32, u8, u32, u32, u32) {
        let (tag, block, ordinal) = self.site.key();
        (
            self.function.index(),
            tag,
            block,
            ordinal,
            self.variable.get(),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct StatementIndex {
    key: (u32, u32, u32, u32),
    ordinal: usize,
}

/// Why this inert analysis is unavailable; none is successful admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScalarSsaEmissionUnavailableV1 {
    /// The legacy owner was created without an opt-in emission attachment.
    NotCaptured,
    /// Expanded execution or placement has no supported exact emission mapping.
    ExpandedPlacement,
    /// The actual source or transport lies outside the supported scalar fragment.
    UnsupportedFragment,
    /// The unchanged N graph has no exact admitted header-parameter recurrence.
    NoExactRecurrence,
}

/// The original genuine PreRanked owner plus one opt-in emission attachment.
/// This is not a second source or executable graph, and the legacy owner layout
/// and constructor receipts are unchanged. This type has no Clone/raw constructor.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionOwnerV1;
/// fn duplicate(value: ProductionScalarSsaEmissionOwnerV1) { let _ = value.clone(); }
/// ```
#[derive(Debug)]
pub struct ProductionScalarSsaEmissionOwnerV1 {
    original: ProductionPreRankedKirOwnerV1,
    emission: Sealed,
    captured_occurrences: Option<ProductionSemanticSsaOccurrenceStorageV1>,
    retained: usize,
}

impl ProductionScalarSsaEmissionOwnerV1 {
    /// Captures from the same genuine materialization path exactly once.
    /// Newly captured source occurrences and the attachment are metered and
    /// transferred once. Existing source/SSA replay, legacy lowering scratch,
    /// launch and canonical legacy bytes retain the materializer's documented
    /// local-limit/storage exclusions; this is not an allocator/RSS guarantee.
    /// The new outer scope restores its original ledger floor after failed
    /// values drop, including on unwind; success transfers an unreserved owner.
    /// It cannot repair excluded allocator
    /// aborts or attribute their cost to the new attachment's byte receipt.
    pub fn try_materialize_with_budget_v1(
        mut semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        resources::scoped(budget, |budget| {
            let header = size_of::<Self>()
                .checked_sub(size_of::<ProductionPreRankedKirOwnerV1>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let captured_occurrences = if semantic_ssa.occurrence_storage().is_none() {
                let receipt = semantic_ssa
                    .try_capture_occurrences_with_budget_v1(budget)
                    .map_err(Error::Occurrences)?;
                budget.reserve_storage(receipt.retained_storage())?;
                Some(receipt)
            } else {
                let receipt = semantic_ssa
                    .occurrence_storage()
                    .ok_or(Error::Mismatch("source occurrence receipt"))?;
                if budget.storage()
                    < receipt
                        .retained_storage()
                        .checked_add(header)
                        .ok_or(Resource::Arithmetic)?
                {
                    return Err(Resource::Accounting.into());
                }
                None
            };
            let recorder = Recorder::prepare(&semantic_ssa, budget)?;
            let (original, recorder) =
                ProductionPreRankedKirOwnerV1::try_materialize_origins_with_scalar_capture_v1(
                    semantic_ssa,
                    source_launch,
                    limits,
                    Some(recorder),
                    budget,
                )
                .map_err(Error::Materialization)?;
            let recorder = recorder.ok_or(Error::Mismatch("actual lowering discarded capture"))?;
            let emission = Sealed::seal(&original, recorder, budget)?;
            let retained = original
                .retained_analysis_storage_v1()
                .checked_add(header)
                .and_then(|n| n.checked_add(emission.retained))
                .and_then(|n| {
                    n.checked_add(captured_occurrences.map_or(0, |r| r.retained_storage()))
                })
                .ok_or(Resource::Arithmetic)?;
            Ok(Self {
                original,
                emission,
                captured_occurrences,
                retained,
            })
        })
    }

    /// Borrows the unchanged genuine source/SSA/N owner retained by this wrapper.
    pub fn original(&self) -> &ProductionPreRankedKirOwnerV1 {
        &self.original
    }
    /// Combined, UNRESERVED transfer: original graph/origin/helper receipt,
    /// this attachment's actual capacities, and newly captured occurrences once.
    /// An incoming occurrence receipt remains separately caller-reserved.
    pub fn retained_analysis_storage_v1(&self) -> usize {
        self.retained
    }
    /// Always false: emission custody alone grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Legacy construction does not implicitly enable this opt-in analysis.
    pub const fn scalar_ssa_emission_unavailable_v1(
        &self,
    ) -> ProductionScalarSsaEmissionUnavailableV1 {
        ProductionScalarSsaEmissionUnavailableV1::NotCaptured
    }
}

fn fixed_scalar(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<ScalarType> {
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) =
        types.get(ty.index() as usize)?.shape()
    else {
        return None;
    };
    Some(match (*signed, *bits) {
        (false, 8) => ScalarType::U8,
        (false, 16) => ScalarType::U16,
        (false, 32) => ScalarType::U32,
        (false, 64) => ScalarType::U64,
        (true, 8) => ScalarType::I8,
        (true, 16) => ScalarType::I16,
        (true, 32) => ScalarType::I32,
        (true, 64) => ScalarType::I64,
        _ => return None,
    })
}
