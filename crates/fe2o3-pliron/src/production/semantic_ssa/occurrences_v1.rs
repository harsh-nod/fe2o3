//! Sealed optional source occurrences from one complete SSA replay.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::{SsaEdgeIdV1, SsaResolvedEventV1, SsaValueV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[path = "occurrences_emission_v1.rs"]
mod emission;
#[path = "occurrences_join_v1.rs"]
mod join;

/// Failure to capture occurrences from the exact retained source and SSA plans.
#[derive(Debug)]
pub enum ProductionSemanticSsaOccurrenceErrorV1 {
    /// The owner already contains a capture; no work or ledger change occurred.
    AlreadyCaptured,
    /// The existing complete source/SSA replay failed.
    Replay(ProductionSemanticSsaErrorV1),
    /// A capture-only work, storage, allocation or accounting operation failed.
    Resource(Resource),
    /// Actual emitted rows did not join exactly to this function's fresh input/plan.
    CaptureMismatch {
        /// Exact semantic function being captured.
        function: SemanticFunctionIdV1,
        /// Exact source block, when the mismatch is block-local.
        block: Option<SsaBlockIdV1>,
    },
}

impl fmt::Display for ProductionSemanticSsaOccurrenceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyCaptured => f.write_str("source SSA occurrences are already captured"),
            Self::Replay(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::CaptureMismatch { function, block } => write!(
                f,
                "source SSA occurrence mismatch in function {} at block {block:?}",
                function.index(),
            ),
        }
    }
}

impl Error for ProductionSemanticSsaOccurrenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::AlreadyCaptured | Self::CaptureMismatch { .. } => None,
        }
    }
}

impl From<ProductionSemanticSsaErrorV1> for ProductionSemanticSsaOccurrenceErrorV1 {
    fn from(error: ProductionSemanticSsaErrorV1) -> Self {
        Self::Replay(error)
    }
}

impl From<Resource> for ProductionSemanticSsaOccurrenceErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

type CaptureError = ProductionSemanticSsaOccurrenceErrorV1;
type CaptureResult<T> = Result<T, CaptureError>;

/// Transfer receipt for new occurrence payload, excluding preexisting MIR/SSA storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaOccurrenceStorageV1 {
    retained_storage: usize,
}

impl ProductionSemanticSsaOccurrenceStorageV1 {
    /// Logical bytes to reserve while the owning capture remains live.
    pub const fn retained_storage(self) -> usize {
        self.retained_storage
    }
}

/// Exact source syntax location; an event ordinal is a separate coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaOccurrenceSiteV1 {
    /// An original statement in the indicated source block.
    Statement {
        /// Source block, not a reverse-postorder ordinal.
        block: SsaBlockIdV1,
        /// Original statement ordinal.
        statement: u32,
    },
    /// The terminator of an original source block.
    Terminator {
        /// Source block, not a successor target.
        block: SsaBlockIdV1,
    },
}

/// Descriptive operand position in the actual adapter grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaOperandRoleV1 {
    /// Rvalue operand in original operand order.
    RvalueOperand(u32),
    /// A rvalue's place.
    RvaluePlace,
    /// An assignment destination.
    Destination,
    /// Explicit store value.
    StoreValue,
    /// Explicit store destination.
    StoreDestination,
    /// Atomic address.
    AtomicAddress,
    /// Atomic value.
    AtomicValue,
    /// Atomic expected value.
    AtomicExpected,
    /// Atomic replacement value.
    AtomicReplacement,
    /// Atomic result destination.
    AtomicDestination,
    /// Other statement place.
    StatementPlace,
    /// Assumption condition.
    Assume,
    /// Storage-live operation.
    StorageLive,
    /// Storage-dead operation.
    StorageDead,
    /// Call argument in original argument order.
    CallArgument(u32),
    /// Projected call-result address evaluated before argument moves.
    CallDestinationAddress,
    /// Tail-call argument in original argument order.
    TailCallArgument(u32),
    /// Switch discriminant.
    SwitchDiscriminant,
    /// Dropped place.
    DropPlace,
    /// Assertion condition.
    AssertCondition,
    /// Assertion diagnostic operand in original order.
    AssertMessage(u32),
    /// Non-ignored return value.
    ReturnValue,
    /// Destination of an actually elided authenticated borrow.
    ElidedBorrowDestination,
}

/// Descriptive subevent of an operand or destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaEventRoleV1 {
    /// Base local read.
    BaseUse,
    /// Index local read at this original projection ordinal.
    ProjectionIndexUse(u32),
    /// Kill after an unprojected Move's read.
    MoveKill,
    /// Unprojected destination definition.
    DestinationDefine,
    /// Storage lifetime kill.
    StorageKill,
}

/// Why the actual adapter emitted an entry definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaEntryOriginV1 {
    /// Original source argument role, not a native or ranked parameter ordinal.
    Argument(u32),
    /// Authenticated implicit capability local.
    ImplicitCapability,
}

/// One original adapter event and its exact fresh-plan resolution.
#[derive(Debug)]
pub struct ProductionSemanticSsaEventOccurrenceV1 {
    site: ProductionSemanticSsaOccurrenceSiteV1,
    operand: ProductionSemanticSsaOperandRoleV1,
    role: ProductionSemanticSsaEventRoleV1,
    ordinal: u32,
    event: SsaEventV1,
    reachable: bool,
    promoted: bool,
    resolved: Option<SsaResolvedEventV1>,
}

impl ProductionSemanticSsaEventOccurrenceV1 {
    /// Exact original syntax location.
    pub const fn site(&self) -> ProductionSemanticSsaOccurrenceSiteV1 {
        self.site
    }
    /// Actual operand position.
    pub const fn operand(&self) -> ProductionSemanticSsaOperandRoleV1 {
        self.operand
    }
    /// Actual operand subevent.
    pub const fn role(&self) -> ProductionSemanticSsaEventRoleV1 {
        self.role
    }
    /// Original block-event ordinal, including unpromoted events.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    /// Exact original kind and variable.
    pub const fn event(&self) -> SsaEventV1 {
        self.event
    }
    /// Whether the actual fresh plan reaches the source block.
    pub const fn is_reachable(&self) -> bool {
        self.reachable
    }
    /// Whether the actual input marks the event's variable promotable.
    pub const fn is_promoted(&self) -> bool {
        self.promoted
    }
    /// Exact retained resolution; absence is not successful discharge.
    pub const fn resolved(&self) -> Option<SsaResolvedEventV1> {
        self.resolved
    }
}

/// A constant operand's locator, without a copied constant payload or SSA event.
#[derive(Debug)]
pub struct ProductionSemanticSsaConstantOccurrenceV1 {
    site: ProductionSemanticSsaOccurrenceSiteV1,
    operand: ProductionSemanticSsaOperandRoleV1,
    next_event: u32,
    ty: SemanticTypeIdV1,
}

impl ProductionSemanticSsaConstantOccurrenceV1 {
    /// Exact original syntax location.
    pub const fn site(&self) -> ProductionSemanticSsaOccurrenceSiteV1 {
        self.site
    }
    /// Actual operand position.
    pub const fn operand(&self) -> ProductionSemanticSsaOperandRoleV1 {
        self.operand
    }
    /// Next event ordinal; the constant itself emits no event.
    pub const fn next_event(&self) -> u32 {
        self.next_event
    }
    /// Type in the same borrowed source owner; no payload is reinterpreted.
    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }
}

/// One exact source successor occurrence, including repeated targets.
#[derive(Debug)]
pub struct ProductionSemanticSsaSuccessorOccurrenceV1 {
    id: SsaEdgeIdV1,
    edge: SemanticControlFlowEdgeV1,
    definitions: std::ops::Range<usize>,
}

impl ProductionSemanticSsaSuccessorOccurrenceV1 {
    /// Original source block and successor ordinal.
    pub const fn id(&self) -> SsaEdgeIdV1 {
        self.id
    }
    /// Actual semantic target and edge role.
    pub const fn edge(&self) -> SemanticControlFlowEdgeV1 {
        self.edge
    }
}

/// Definition attached to one exact conceptual successor.
#[derive(Debug)]
pub struct ProductionSemanticSsaEdgeDefinitionOccurrenceV1 {
    edge: SsaEdgeIdV1,
    ordinal: u32,
    variable: SsaVariableIdV1,
    reachable: bool,
    promoted: bool,
    value: Option<SsaValueV1>,
}

impl ProductionSemanticSsaEdgeDefinitionOccurrenceV1 {
    /// Exact source block and successor ordinal.
    pub const fn edge(&self) -> SsaEdgeIdV1 {
        self.edge
    }
    /// Original edge-definition ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    /// Exact source variable.
    pub const fn variable(&self) -> SsaVariableIdV1 {
        self.variable
    }
    /// Whether the source block is reachable.
    pub const fn is_reachable(&self) -> bool {
        self.reachable
    }
    /// Whether the actual input promotes the variable.
    pub const fn is_promoted(&self) -> bool {
        self.promoted
    }
    /// Exact edge definition, never inferred from the target alone.
    pub const fn value(&self) -> Option<SsaValueV1> {
        self.value
    }
}

/// One actual external entry definition and its category.
#[derive(Debug)]
pub struct ProductionSemanticSsaEntryDefinitionOccurrenceV1 {
    ordinal: u32,
    variable: SsaVariableIdV1,
    origin: ProductionSemanticSsaEntryOriginV1,
    value: Option<SsaValueV1>,
}

impl ProductionSemanticSsaEntryDefinitionOccurrenceV1 {
    /// Original input entry-definition ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    /// Exact source variable.
    pub const fn variable(&self) -> SsaVariableIdV1 {
        self.variable
    }
    /// Argument versus authenticated implicit capability.
    pub const fn origin(&self) -> ProductionSemanticSsaEntryOriginV1 {
        self.origin
    }
    /// Actual promoted entry definition; absent for unpromoted variables.
    pub const fn value(&self) -> Option<SsaValueV1> {
        self.value
    }
}

/// Read-only capture bound to the same live source/SSA owner.
///
/// Captured event slices cannot outlive their owner:
///
/// ```compile_fail,E0505
/// use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
/// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
/// fn cannot_detach(owner: ProductionSemanticSsaOwnerV1) {
///     let view = owner.occurrences_v1().unwrap();
///     let function = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
///     let events = function.events();
///     drop(owner);
///     let _ = events.len();
/// }
/// ```
///
/// A live event slice also excludes mutable capture on its owner:
///
/// ```compile_fail,E0502
/// use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
/// fn cannot_recapture(
///     owner: &mut ProductionSemanticSsaOwnerV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     let view = owner.occurrences_v1().unwrap();
///     let function = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
///     let events = function.events();
///     let _ = owner.try_capture_occurrences_with_budget_v1(budget);
///     let _ = events.len();
/// }
/// ```
pub struct ProductionSemanticSsaOccurrenceViewV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    attachment: &'a Attachment,
}

impl<'a> ProductionSemanticSsaOccurrenceViewV1<'a> {
    /// Number of captured semantic functions, without filtering source blocks.
    pub fn function_count(&self) -> usize {
        self.attachment.functions.len()
    }
    /// O(1) lookup by exact semantic function ID, not public root or kernel ordinal.
    pub fn function(
        &self,
        id: SemanticFunctionIdV1,
    ) -> Option<ProductionSemanticSsaFunctionOccurrencesV1<'a>> {
        let rows = self.attachment.functions.get(id.index() as usize)?;
        (rows.function == id).then_some(ProductionSemanticSsaFunctionOccurrencesV1 {
            owner: self.owner,
            rows,
        })
    }
}

/// Ordered descriptive rows borrowing one exact captured semantic function.
pub struct ProductionSemanticSsaFunctionOccurrencesV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    rows: &'a FunctionRows,
}

impl ProductionSemanticSsaFunctionOccurrencesV1<'_> {
    /// Exact function ID in this owner.
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.rows.function
    }
    /// The same source/SSA owner; no graph or detached capture is constructed.
    pub const fn owner(&self) -> &ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    /// Events in original block/event order.
    pub fn events(&self) -> &[ProductionSemanticSsaEventOccurrenceV1] {
        &self.rows.events
    }
    /// Constant operand occurrences in original visitation order.
    pub fn constants(&self) -> &[ProductionSemanticSsaConstantOccurrenceV1] {
        &self.rows.constants
    }
    /// Successor occurrences in original block/successor order.
    pub fn successors(&self) -> &[ProductionSemanticSsaSuccessorOccurrenceV1] {
        &self.rows.successors
    }
    /// Definitions in original block/successor/definition order.
    pub fn edge_definitions(&self) -> &[ProductionSemanticSsaEdgeDefinitionOccurrenceV1] {
        &self.rows.edge_definitions
    }
    /// Actual external entry definitions in local order.
    pub fn entry_definitions(&self) -> &[ProductionSemanticSsaEntryDefinitionOccurrenceV1] {
        &self.rows.entries
    }
    /// Exactly the authenticated borrows elided by the actual prepared adapter.
    pub fn elisions(&self) -> &[ProductionSemanticSsaOccurrenceSiteV1] {
        &self.rows.elisions
    }
}

struct BlockRows {
    block: SsaBlockIdV1,
    events: std::ops::Range<usize>,
    successors: std::ops::Range<usize>,
}

struct FunctionRows {
    function: SemanticFunctionIdV1,
    blocks: Vec<BlockRows>,
    events: Vec<ProductionSemanticSsaEventOccurrenceV1>,
    constants: Vec<ProductionSemanticSsaConstantOccurrenceV1>,
    successors: Vec<ProductionSemanticSsaSuccessorOccurrenceV1>,
    edge_definitions: Vec<ProductionSemanticSsaEdgeDefinitionOccurrenceV1>,
    entries: Vec<ProductionSemanticSsaEntryDefinitionOccurrenceV1>,
    elisions: Vec<ProductionSemanticSsaOccurrenceSiteV1>,
}

pub(super) struct Attachment {
    functions: Vec<FunctionRows>,
    storage: ProductionSemanticSsaOccurrenceStorageV1,
}

#[cfg(test)]
pub(super) fn capture_resource_row_sizes_for_test() -> [usize; 3] {
    [
        std::mem::size_of::<Option<Attachment>>(),
        std::mem::size_of::<FunctionRows>(),
        std::mem::size_of::<BlockRows>(),
    ]
}

pub(super) trait ReplayDriver {
    type Error: From<ProductionSemanticSsaErrorV1>;
    fn start(&mut self, functions: usize) -> Result<(), Self::Error>;
    fn input(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: Option<&[SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    ) -> Result<(SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize), Self::Error>;
    fn join(
        &mut self,
        input: &SsaConstructionInputV1,
        plan: &ProductionSemanticSsaFunctionPlanV1,
    ) -> Result<(), Self::Error>;
}

pub(super) struct PlainReplay;

impl ReplayDriver for PlainReplay {
    type Error = ProductionSemanticSsaErrorV1;
    fn start(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn input(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: Option<&[SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    ) -> Result<(SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize), Self::Error> {
        Ok(semantic_function_ssa_input_v1(
            function,
            types,
            callables,
            transparent_borrows,
        ))
    }
    fn join(
        &mut self,
        _: &SsaConstructionInputV1,
        _: &ProductionSemanticSsaFunctionPlanV1,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ProductionSemanticSsaOwnerV1 {
    /// Performs one full replay and atomically captures actual source occurrences.
    ///
    /// Capture-only logical work/storage excludes existing source equivalence,
    /// classification, planner and identity-hashing costs. The pinned allocation
    /// assumption is documented in the private capture allocator. On success,
    /// storage returns to its entry floor and the returned receipt must be
    /// reserved before further controlled allocation. Returned errors and unwind
    /// drop uninstalled rows before restoring a valid floor. Capture-resource
    /// errors may precede later old replay errors; ordinary replay is unchanged.
    /// An already captured owner is rejected without any ledger/history change.
    pub fn try_capture_occurrences_with_budget_v1(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> CaptureResult<ProductionSemanticSsaOccurrenceStorageV1> {
        if self.occurrences.is_some() {
            return Err(CaptureError::AlreadyCaptured);
        }
        let floor = budget.storage();
        let attachment = capture_pending_with_cleanup(budget, |budget| {
            let mut driver = emission::CaptureDriver::new(budget);
            self.replay_with_driver_v1(&mut driver)?;
            driver.finish()
        })?;
        let receipt = attachment.storage;
        if budget.storage().checked_sub(floor) != Some(receipt.retained_storage) {
            drop(attachment);
            restore_floor(budget, floor)?;
            return Err(Resource::Accounting.into());
        }
        // This is an explicit transfer, immediately followed by the
        // infallible owner move. No allocation/check occurs after publish.
        budget.release_storage(receipt.retained_storage)?;
        self.occurrences = Some(attachment);
        Ok(receipt)
    }

    /// Returns only borrowed ordered rows; no replay, search or allocation occurs.
    pub fn occurrences_v1(&self) -> Option<ProductionSemanticSsaOccurrenceViewV1<'_>> {
        self.occurrences
            .as_ref()
            .map(|attachment| ProductionSemanticSsaOccurrenceViewV1 {
                owner: self,
                attachment,
            })
    }

    /// Returns the receipt owned by the capture, without changing any ledger.
    ///
    /// Read this before consuming the SSA owner if the caller must release its
    /// reserved capture bytes after `into_source_owner` drops the attachment.
    pub fn occurrence_storage(&self) -> Option<ProductionSemanticSsaOccurrenceStorageV1> {
        self.occurrences
            .as_ref()
            .map(|attachment| attachment.storage)
    }
}

pub(super) fn capture_pending_with_cleanup<T>(
    budget: &mut Budget<'_>,
    build: impl FnOnce(&mut Budget<'_>) -> CaptureResult<T>,
) -> CaptureResult<T> {
    let floor = budget.storage();
    match catch_unwind(AssertUnwindSafe(|| build(budget))) {
        Ok(Ok(pending)) => Ok(pending),
        Ok(Err(error)) => {
            restore_floor(budget, floor)?;
            Err(error)
        }
        Err(payload) => {
            // Scope unwinding has dropped pending owners before this release.
            // Invalid-ledger cleanup is not a reason to replace the panic value.
            let _ = restore_floor(budget, floor);
            resume_unwind(payload)
        }
    }
}

fn restore_floor(budget: &mut Budget<'_>, floor: usize) -> Result<(), Resource> {
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)
}

fn mismatch(function: SemanticFunctionIdV1, block: Option<SsaBlockIdV1>) -> CaptureError {
    CaptureError::CaptureMismatch { function, block }
}

fn checked_u32(value: usize) -> CaptureResult<u32> {
    u32::try_from(value).map_err(|_| Resource::Arithmetic.into())
}
