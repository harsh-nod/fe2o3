//! Complete graph coverage and pending ranked obligations, never source or safety authority.
//! Candidates are inert; scoped checked views borrow the original graph and metadata.
use crate::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "canonical_ranked_view_build_v1.rs"]
mod build;
#[path = "canonical_ranked_view_check_v1.rs"]
mod check;
#[path = "canonical_ranked_view_control_v1.rs"]
mod control;
#[path = "canonical_ranked_view_effects_v1.rs"]
mod effects;
pub use build::build_canonical_ranked_candidate_v1;
pub use check::with_checked_canonical_ranked_view_v1;

/// Dense inventory positions, never sparse ValueId/BlockId allocation sizes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalRankedSubjectV1 {
    Module,
    Kernel(usize),
    Function(usize),
    Block(usize),
    Definition(usize),
    Operation(usize),
    Use(usize),
    Edge(usize),
    EdgeArgument(usize),
    Effect(usize),
    Call(usize),
    Requirement {
        owner: CanonicalRankedRequirementOwnerV1,
        ordinal: usize,
    },
    Metadata(usize),
}
use CanonicalRankedSubjectV1 as Subject;

/// Requirements retain their declaring owner and occurrence, without deduplication.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalRankedRequirementOwnerV1 {
    Module,
    Kernel(usize),
    Function(usize),
    Operation(usize),
    /// Function-context requirement derived from this operation's actual pointer type.
    AtomicPointer(usize),
}
use CanonicalRankedRequirementOwnerV1 as RequirementOwner;

/// Closed semantic family; exact operands/types/attributes remain in N.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedOperationClassV1 {
    Constant,
    Intrinsic,
    MemoryIntrinsic,
    Unary,
    Binary,
    Compare,
    Cast,
    Select,
    Call,
    Alloca,
    SliceLength,
    SliceData,
    GetElementPointer,
    Load,
    GuardedLoad,
    GuardedStore,
    Store,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    Matrix,
    LdsTranspose,
    Wave,
    InlineAssembly,
    VerificationContract,
    VectorLoad,
    VectorStore,
    VectorLayoutConvert,
}
use CanonicalRankedOperationClassV1 as OperationClass;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedTerminatorClassV1 {
    Branch,
    ConditionalBranch,
    Switch,
    IntegerSwitch,
    Return,
    Unreachable,
}
use CanonicalRankedTerminatorClassV1 as TerminatorClass;

/// Equal targets still have distinct occurrences and argument payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedEdgeClassV1 {
    Branch,
    True,
    False,
    SwitchCase(usize),
    SwitchDefault,
    IntegerCase(usize),
    IntegerDefault,
}
use CanonicalRankedEdgeClassV1 as EdgeClass;

/// None of these obligations is discharged by structural coverage or metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CanonicalRankedObligationV1 {
    ExactScalarSemantics,
    Control,
    Bounds,
    Provenance,
    Initialization,
    RaceFreedom,
    Lifetime,
    Ordering,
    Convergence,
    TrapBehavior,
    CallEffects,
    CallControl,
    Launch,
    Target,
    Tensor,
    Assembly,
    Contract,
    ReferenceRefinement,
    SourceMetadata,
}
use CanonicalRankedObligationV1 as Obligation;
const ALL_OBLIGATIONS: [Obligation; 19] = [
    Obligation::ExactScalarSemantics,
    Obligation::Control,
    Obligation::Bounds,
    Obligation::Provenance,
    Obligation::Initialization,
    Obligation::RaceFreedom,
    Obligation::Lifetime,
    Obligation::Ordering,
    Obligation::Convergence,
    Obligation::TrapBehavior,
    Obligation::CallEffects,
    Obligation::CallControl,
    Obligation::Launch,
    Obligation::Target,
    Obligation::Tensor,
    Obligation::Assembly,
    Obligation::Contract,
    Obligation::ReferenceRefinement,
    Obligation::SourceMetadata,
];

/// Duplicate-free fixed-size set of explicitly pending obligations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanonicalRankedObligationsV1(u32);
use CanonicalRankedObligationsV1 as Obligations;
impl Obligations {
    pub const NONE: Self = Self(0);
    pub const fn with(self, obligation: Obligation) -> Self {
        Self(self.0 | (1 << obligation as u8))
    }
    pub const fn contains(self, obligation: Obligation) -> bool {
        self.0 & (1 << obligation as u8) != 0
    }
    pub fn iter(self) -> impl Iterator<Item = Obligation> {
        ALL_OBLIGATIONS
            .into_iter()
            .filter(move |item| self.contains(*item))
    }
}

/// Explicit role for every covered item; no wildcard "safe" role exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedRoleV1 {
    Module,
    Kernel,
    Definition,
    Use,
    EdgeArgument,
    Effect,
    Call,
    Requirement,
    Metadata,
    DefinedFunction,
    Declaration,
    Block(TerminatorClass),
    Operation(OperationClass),
    Edge(EdgeClass),
}
use CanonicalRankedRoleV1 as Role;

/// Public mutable candidate data, not a proof object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRankedCoverageRowV1 {
    pub subject: Subject,
    pub role: Role,
    pub obligations: Obligations,
}
use CanonicalRankedCoverageRowV1 as Row;

/// Source-only claims are inert here; later lowerer authentication is mandatory.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalRankedMetadataKindV1 {
    Launch,
    Memory,
    Lifetime,
    Tensor,
    Pipeline,
    Numerical,
    Refinement,
    Assembly,
}

/// Closed inert values may reference N, not supply another CFG or scalar tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRankedMetadataFactV1 {
    Unsigned(u64),
    Signed(i64),
    Definition(usize),
    Effect(usize),
    Identity([u8; 32]),
}
use CanonicalRankedMetadataFactV1 as Fact;

#[derive(Debug)]
pub struct CanonicalRankedMetadataRowV1<'m> {
    pub subject: Subject,
    pub kind: CanonicalRankedMetadataKindV1,
    pub facts: &'m [Fact],
}

/// Graph-bound claims; construction does not authenticate or prove their values.
#[derive(Debug)]
pub struct CanonicalRankedMetadataV1<'g, 'm> {
    owner: &'g Owner,
    rows: &'m [CanonicalRankedMetadataRowV1<'m>],
}
use CanonicalRankedMetadataV1 as Metadata;
impl<'g, 'm> Metadata<'g, 'm> {
    pub const fn new(owner: &'g Owner, rows: &'m [CanonicalRankedMetadataRowV1<'m>]) -> Self {
        Self { owner, rows }
    }
    pub const fn rows(&self) -> &'m [CanonicalRankedMetadataRowV1<'m>] {
        self.rows
    }
    /// Logical borrowed extent, excluding allocator slack and retained source owners.
    pub fn storage_extent(&self, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(add(1, self.rows.len())?)?;
        let mut bytes = add(
            size_of::<Self>(),
            payload::<CanonicalRankedMetadataRowV1<'_>>(self.rows.len())?,
        )?;
        for row in self.rows {
            bytes = add(bytes, payload::<Fact>(row.facts.len())?)?;
        }
        Ok(bytes)
    }
}

/// Candidate bound to exact inventory and metadata objects, not claimed hashes.
/// Borrows prevent moves/mutation; admission additionally checks object identity.
#[derive(Debug)]
pub struct CanonicalRankedCandidateV1<'i, 'g, 'm> {
    inventory: &'i Inventory<'g>,
    metadata: &'i Metadata<'g, 'm>,
    rows: Vec<Row>,
}
use CanonicalRankedCandidateV1 as Candidate;
impl<'i, 'g, 'm> Candidate<'i, 'g, 'm> {
    /// Adopts caller-owned inert rows without checking them or allocating.
    pub fn from_rows(
        inventory: &'i Inventory<'g>,
        metadata: &'i Metadata<'g, 'm>,
        rows: Vec<Row>,
    ) -> Self {
        Self {
            inventory,
            metadata,
            rows,
        }
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn rows_mut(&mut self) -> &mut [Row] {
        &mut self.rows
    }
    pub fn into_rows(self) -> Vec<Row> {
        self.rows
    }
    /// Actual vector capacity plus header; not RSS or allocator overhead.
    pub fn retained_storage(&self) -> Result<usize> {
        add(size_of::<Self>(), payload::<Row>(self.rows.capacity())?)
    }
}

/// Payload transferred with a successful candidate, to reserve while it lives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRankedCandidateStorageV1(usize);
impl CanonicalRankedCandidateStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRankedViewErrorV1 {
    Resource(Resource),
    ForeignInventory,
    ForeignMetadata,
    MetadataOrder {
        ordinal: usize,
    },
    MetadataSubject {
        ordinal: usize,
    },
    MetadataFact {
        row: usize,
        fact: usize,
    },
    MissingRow {
        ordinal: usize,
        expected: Row,
    },
    MismatchedRow {
        ordinal: usize,
        expected: Row,
        actual: Row,
    },
    ExtraRows {
        first: usize,
    },
    /// Coverage-row table index, not a coordinate in the underlying graph.
    InvalidRow {
        ordinal: usize,
    },
    InvalidCoordinate(Subject),
    UnsupportedOperation {
        ordinal: usize,
        wire_version: u8,
    },
    InconsistentInventory,
    Panicked,
}
use CanonicalRankedViewErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical ranked view: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

/// Scoped structural coverage only: not source, numerical, memory, convergence,
/// launch or native safety. Every obligation remains pending.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalRankedViewV1;
/// fn forge() { let _ = CheckedCanonicalRankedViewV1 {}; }
/// ```
pub struct CheckedCanonicalRankedViewV1<'scope, 'i, 'g, 'm> {
    candidate: &'i Candidate<'i, 'g, 'm>,
    accounting: &'scope mut control::Accounting,
}
impl<'g> CheckedCanonicalRankedViewV1<'_, '_, 'g, '_> {
    /// Reuses the exact input inventory. Traversal beyond this O(1) query remains
    /// the consumer's metered work; it is not a free complete graph analysis.
    pub fn inventory(&mut self, budget: &mut Budget<'_>) -> Result<&Inventory<'g>> {
        self.accounting.charge(budget, 1)?;
        Ok(self.candidate.inventory)
    }
    /// O(1), with no source or prior-stage replay.
    pub fn row(&mut self, ordinal: usize, budget: &mut Budget<'_>) -> Result<&Row> {
        self.accounting.charge(budget, 1)?;
        match self.candidate.rows.get(ordinal) {
            Some(row) => Ok(row),
            None => Err(self.accounting.fail(Error::InvalidRow { ordinal })),
        }
    }
    pub fn row_count(&mut self, budget: &mut Budget<'_>) -> Result<usize> {
        self.accounting.charge(budget, 1)?;
        Ok(self.candidate.rows.len())
    }
    /// Existing N operation, not a reconstructed scalar expression.
    pub fn operation(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&'g fe2o3_kernel_ir::Operation> {
        self.accounting.charge(budget, 1)?;
        match self.candidate.inventory.operations().get(ordinal) {
            Some(row) => Ok(row.operation),
            None => Err(self
                .accounting
                .fail(Error::InvalidCoordinate(Subject::Operation(ordinal)))),
        }
    }
    pub fn edge(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&crate::CanonicalKirEdgeRefV1<'g>> {
        self.accounting.charge(budget, 1)?;
        match self.candidate.inventory.edges().get(ordinal) {
            Some(row) => Ok(row),
            None => Err(self
                .accounting
                .fail(Error::InvalidCoordinate(Subject::Edge(ordinal)))),
        }
    }
    pub fn edge_argument(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&crate::CanonicalKirEdgeArgumentRefV1> {
        self.accounting.charge(budget, 1)?;
        match self.candidate.inventory.edge_arguments().get(ordinal) {
            Some(row) => Ok(row),
            None => Err(self
                .accounting
                .fail(Error::InvalidCoordinate(Subject::EdgeArgument(ordinal)))),
        }
    }
    pub fn metadata(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&CanonicalRankedMetadataRowV1<'_>> {
        self.accounting.charge(budget, 1)?;
        match self.candidate.metadata.rows.get(ordinal) {
            Some(row) => Ok(row),
            None => Err(self
                .accounting
                .fail(Error::InvalidCoordinate(Subject::Metadata(ordinal)))),
        }
    }
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Resource::Arithmetic.into())
}
fn payload<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}
fn row(subject: Subject, role: Role, obligations: Obligations) -> Row {
    Row {
        subject,
        role,
        obligations,
    }
}

#[cfg(test)]
#[path = "canonical_ranked_view_hostile_v1_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "canonical_ranked_view_v1_tests.rs"]
mod tests;
