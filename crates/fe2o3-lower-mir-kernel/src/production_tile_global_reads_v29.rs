//! Local Global-read conditions on the actual scalar candidate. All source,
//! allocation, launch, collective and unrelated-effect obligations remain live.

use crate::{
    ProductionCheckedTileScalarTransportV29 as Transport,
    ProductionTileAttachmentFamilyV29 as AttachmentFamily,
    ProductionTileAttachmentTargetV29 as AttachmentTarget,
    ProductionTileExpansionStageV29 as Stage, ProductionTileOccurrenceCoordinateV29 as Occurrence,
    ProductionTilePendingKindV29 as PendingKind, ProductionTilePendingObligationV29,
    ProductionTileScalarTransportErrorV29, ProductionTileSourceCoordinateV29 as SourceCoordinate,
    ProductionTileTargetCoordinateV29 as TargetCoordinate,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::{
    AddressSpace, Axis, BinaryOp, CanonicalGuardedGlobalReadErrorV1 as GraphError,
    CanonicalGuardedGlobalReadLimitsV1, CanonicalGuardedGlobalReadOutcomeV18 as ReadOutcome,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate,
    CheckedCanonicalGuardedGlobalReadsV18 as Graph, ComparePredicate, Constant,
    ExecutionOperationV15, FormalRuntimeSliceReadDomainV1, IndexKind, IntrinsicKind, Module,
    Operation, OperationKind, ScalarType, Type, ValueId, with_canonical_guarded_global_reads_v18,
};
use std::{cell::RefCell, mem::size_of, ops::Range};

/// A source/graph join failure is not permission to skip an obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionTileGlobalReadErrorV29 {
    /// Actual pending-to-current transport refused its query.
    Transport(ProductionTileScalarTransportErrorV29),
    /// Shared graph engine or its paid query refused.
    Graph(GraphError),
    /// Source-join live work/storage or custody refusal.
    Resource(Resource),
    /// Complete original/current source association did not match.
    Binding {
        /// Located pending-row ordinal, when the mismatch has one.
        obligation: Option<usize>,
        /// Closed diagnostic for the failed exact association.
        reason: &'static str,
    },
    /// The actual occurrence lacks a local graph proof.
    Unproved {
        /// Exact actual operation lacking a required fact.
        operation: Coordinate,
        /// Closed local-condition diagnostic.
        reason: &'static str,
    },
    /// A callback or rejected payload unwound.
    Panicked,
}
impl From<Resource> for ProductionTileGlobalReadErrorV29 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<GraphError> for ProductionTileGlobalReadErrorV29 {
    fn from(error: GraphError) -> Self {
        Self::Graph(error)
    }
}
impl From<ProductionTileScalarTransportErrorV29> for ProductionTileGlobalReadErrorV29 {
    fn from(error: ProductionTileScalarTransportErrorV29) -> Self {
        Self::Transport(error)
    }
}
impl std::fmt::Display for ProductionTileGlobalReadErrorV29 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tile Global read: {self:?}")
    }
}
impl std::error::Error for ProductionTileGlobalReadErrorV29 {}
type Failure = ProductionTileGlobalReadErrorV29;
type R<T> = Result<T, Failure>;

/// Exact local conditions attached to one generated physical read occurrence.
/// Runtime slice allocation/extent and launch validity are still pending.
pub struct ProductionTileGlobalReadAdmissionV29 {
    obligation: usize,
    operation: Coordinate,
    root: usize,
    input: ValueId,
    base: ValueId,
    domain: FormalRuntimeSliceReadDomainV1,
    checked_operations: usize,
    aliases: Range<usize>,
}
impl ProductionTileGlobalReadAdmissionV29 {
    /// Index in the complete unchanged pending obligation roster.
    pub const fn obligation(&self) -> usize {
        self.obligation
    }
    /// Exact current physical operation.
    pub const fn operation(&self) -> Coordinate {
        self.operation
    }
    /// Exact source-selected root ordinal.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Current SSA input joined from the original input definition.
    pub const fn input(&self) -> ValueId {
        self.input
    }
    /// Current SSA base joined from the original base definition.
    pub const fn base(&self) -> ValueId {
        self.base
    }
    /// Descriptive locally bounded domain, not detached proof authority.
    pub const fn domain(&self) -> &FormalRuntimeSliceReadDomainV1 {
        &self.domain
    }
    /// Number of actual checked arithmetic occurrences independently proved.
    pub const fn checked_operations(&self) -> usize {
        self.checked_operations
    }
    /// Every source/attachment/piece association, including repeated aliases.
    pub fn aliases(&self) -> Range<usize> {
        self.aliases.clone()
    }
}

/// One unmerged actual source-to-read association. No private-memory grant.
pub struct ProductionTileGlobalReadAliasV29 {
    read: usize,
    source_alias: usize,
    attachment: usize,
    piece: usize,
}
impl ProductionTileGlobalReadAliasV29 {
    /// Dense local read row.
    pub const fn read(&self) -> usize {
        self.read
    }
    /// Original complete transport alias row.
    pub const fn source_alias(&self) -> usize {
        self.source_alias
    }
    /// Exact projected attachment occurrence.
    pub const fn attachment(&self) -> usize {
        self.attachment
    }
    /// Exact expansion-piece occurrence; equal coordinates are not deduplicated.
    pub const fn piece(&self) -> usize {
        self.piece
    }
}

struct Joined {
    reads: Vec<ProductionTileGlobalReadAdmissionV29>,
    aliases: Vec<ProductionTileGlobalReadAliasV29>,
}

/// Sealed source transport and fresh same-graph local conditions. No conversion
/// to fixed-nine safety, a private certificate, artifact or launch authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedTileGlobalReadsV29;
/// fn detach<'s>(view: &ProductionCheckedTileGlobalReadsV29<'s>)
///     -> ProductionCheckedTileGlobalReadsV29<'s> {
///     view.clone()
/// }
/// ```
pub struct ProductionCheckedTileGlobalReadsV29<'scope> {
    transport: &'scope Transport<'scope>,
    graph: &'scope Graph<'scope, 'scope>,
    joined: &'scope Joined,
    gate: &'scope Gate,
}
impl ProductionCheckedTileGlobalReadsV29<'_> {
    /// Complete unchanged transport, retaining every pending family.
    pub fn transport(&self, budget: &mut Budget<'_>) -> R<&Transport<'_>> {
        self.gate.query(budget)?;
        Ok(self.transport)
    }
    /// Fresh exact-owner graph view, not a separately editable recipe.
    pub fn graph(&self, budget: &mut Budget<'_>) -> R<&Graph<'_, '_>> {
        self.gate.query(budget)?;
        Ok(self.graph)
    }
    /// Generated physical reads, not all ordinary source memory effects.
    pub fn read_count(&self, budget: &mut Budget<'_>) -> R<usize> {
        self.gate.query(budget)?;
        Ok(self.joined.reads.len())
    }
    /// Paid, bounded local condition row.
    pub fn read(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> R<&ProductionTileGlobalReadAdmissionV29> {
        self.gate.query(budget)?;
        self.gate.save(
            self.joined
                .reads
                .get(ordinal)
                .ok_or_else(|| binding(None, "read ordinal")),
        )
    }
    /// Complete non-deduplicated source-to-read association count.
    pub fn alias_count(&self, budget: &mut Budget<'_>) -> R<usize> {
        self.gate.query(budget)?;
        Ok(self.joined.aliases.len())
    }
    /// Paid association query.
    pub fn alias(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> R<&ProductionTileGlobalReadAliasV29> {
        self.gate.query(budget)?;
        self.gate.save(
            self.joined
                .aliases
                .get(ordinal)
                .ok_or_else(|| binding(None, "alias ordinal")),
        )
    }
    /// The full pending roster remains present, including the GlobalRead subjects.
    pub fn pending_obligation_count(&self, budget: &mut Budget<'_>) -> R<usize> {
        self.gate.query(budget)?;
        self.gate.save(
            self.transport
                .pending_obligation_count(budget)
                .map_err(Into::into),
        )
    }
    /// No original Source/Collective/Launch/Attachment/GlobalRead is erased.
    pub fn pending_obligation(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> R<&ProductionTilePendingObligationV29> {
        self.gate.query(budget)?;
        self.gate.save(
            self.transport
                .pending_obligation(ordinal, budget)
                .map_err(Into::into),
        )
    }
    /// Local read conditions never authorize an artifact or a runtime launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Reuses the actual transport inventory and shared graph engine. No caller truth
/// or selected safe-read subset. The transport's source/current owners stay live.
///
/// Construction has an inherited CFG logical-cell domain plus actual-capacity
/// bytes for new buffers. No mixed storage total is a whole-path byte/RSS bound.
/// Callback scratch must be dropped/refunded before return; local facts cannot
/// escape their exact borrowed source/graph lifetime.
pub fn with_checked_tile_global_reads_v29<'w, T>(
    transport: &Transport<'_>,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(
        &ProductionCheckedTileGlobalReadsV29<'scope>,
        &mut Budget<'w>,
    ) -> R<T>,
) -> R<T> {
    let inventory = transport.current_inventory(budget)?;
    let output = inventory.owner();
    let nested = with_canonical_guarded_global_reads_v18(
        output,
        CanonicalGuardedGlobalReadLimitsV1::default(),
        budget,
        |graph, budget| {
            Ok(source_scope(budget, |budget| {
                if !std::ptr::eq(graph.owner(budget)?, output) {
                    return Err(binding(None, "foreign actual graph"));
                }
                let joined = join(transport, inventory, graph, budget)?;
                let gate = Gate::new(budget);
                let view = ProductionCheckedTileGlobalReadsV29 {
                    transport,
                    graph,
                    joined: &joined,
                    gate: &gate,
                };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    consume(&view, budget)
                }));
                let result = match caught {
                    Ok(mut result) => {
                        if let Err(error) = gate.postflight(budget) {
                            drain(std::mem::replace(&mut result, Err(error)));
                        }
                        result
                    }
                    Err(payload) => {
                        drain(payload);
                        // Keep the first query error through unwind. Paid callback
                        // scratch is refunded by source_scope after backing drops.
                        Err(if !gate.valid(budget) {
                            Resource::Accounting.into()
                        } else {
                            gate.failure.borrow().clone().unwrap_or(Failure::Panicked)
                        })
                    }
                };
                drop(joined);
                result
            }))
        },
    )?;
    nested
}

include!("production_tile_global_reads_resources_v29.rs");
include!("production_tile_global_reads_source_v29.rs");
