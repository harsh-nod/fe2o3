// Source-reference identity and representation planning, never allocation authority.
include!("production_source_reference_boundaries_v29.rs");
include!("production_source_external_reference_v29.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSiteV29 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    statement: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceAnchorV29 {
    argument: u32,
    ty: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceCellNeedV29 {
    ReferentWrite,
    AddressObservation,
    UnrepresentedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceRepresentationV29 {
    // These choices describe representation obligations. Neither is a pointer permit.
    StableReferent,
    ExistingAllocationBinding(SourceReferenceAnchorV29),
    NeedsAddressable(SourceReferenceCellNeedV29),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceReferenceEffectsV29 {
    referent_reads: usize,
    referent_writes: usize,
    payload_reads: usize,
    payload_writes: usize,
    address_observations: usize,
}

#[derive(Debug)]
struct SourceReferenceOriginV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    value: usize,
    ty: SemanticTypeIdV1,
    projections: std::ops::Range<usize>,
    anchor: Option<SourceReferenceAnchorV29>,
}

#[derive(Debug)]
struct SourceReferenceLoanV29 {
    site: SourceReferenceSiteV29,
    source_type: SemanticTypeIdV1,
    kind: SemanticBorrowKindV1,
    origin: usize,
    parent: Option<usize>,
    effects: SourceReferenceEffectsV29,
    representation: SourceReferenceRepresentationV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceNodeKindV29 {
    Absent,
    Plain(Option<SourceReferenceAnchorV29>),
    Loan(usize),
    Address(usize),
    Aggregate { first: usize, count: usize },
    Enum { first: usize, count: usize },
    EnumView(usize),
    Discriminant(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceNodeV29 {
    ty: SemanticTypeIdV1,
    kind: SourceReferenceNodeKindV29,
    // Flattened original value identity; storage annotations are not value formations.
    value_origin: Option<usize>,
    storage: Option<SourceReferenceValueStorageV29>,
    inactive: Option<SourceReferenceInactiveShapeV29>,
    descriptor: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceValueStorageV29 {
    snapshot: usize,
    first: usize,
    count: usize,
    selector_source: Option<(SourceReferenceSiteV29, usize)>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceReferenceLocalV29 {
    // The storage generation changes on StorageLive, not on a value assignment.
    generation: u32,
    node: Option<usize>,
    storage: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceBlockV29 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    entry: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceAccessV29 {
    Read,
    ReadDiscriminant,
    Write,
    Borrow(SemanticBorrowKindV1),
    Address,
}

struct SourceReferencePlaceV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    value: usize,
    // Current holder representation, distinct from the loan's original value.
    representation_root: usize,
    node: usize,
    projections: Vec<SemanticProjectionV1>,
    selector_source: Option<(SourceReferenceSiteV29, usize)>,
    anchor: Option<SourceReferenceAnchorV29>,
    loan: Option<usize>,
    shared_path: bool,
    traversed: Vec<usize>,
}

struct SourceReferencePlanV29<'a, 'source> {
    instances: &'a ExecutionInstancesV29<'source>,
    source: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: ProductionCallInstanceIdV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    retained_floor: usize,
    failure: source_storage_v29::SourceReferenceFailureV29<'a>,
    storage_root: Option<source_storage_v29::SourceStorageRootCustodyViewV29<'a, 'source>>,
    storage_demands: Option<source_storage_demands_v29::RootStorageDemandsV29<'a, 'source>>,
    has_storage_demands: bool,
    descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'a>>,
    descriptor_sets: Vec<SourceDescriptorSetV29>,
    descriptor_origins: Vec<SourceDescriptorOriginV29>,
    descriptor_set_index: BTreeMap<SourceDescriptorSetKeyV29, Vec<usize>>,
    origins: Vec<SourceReferenceOriginV29>,
    projections: Vec<SemanticProjectionV1>,
    loans: Vec<SourceReferenceLoanV29>,
    external_borrows: Vec<SourceExternalReferenceBorrowV29>,
    nodes: Vec<SourceReferenceNodeV29>,
    selected_storage: Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>,
    representation_demands: Vec<SourceReferenceRepresentationDemandV29>,
    representation_demand_sites: BTreeMap<Box<SourceReferenceAccessIndexKeyV29>, Vec<usize>>,
    storage_activations: Vec<SourceReferenceStorageActivationV29>,
    storage_activation_sites: BTreeMap<(usize, u32, u32), usize>,
    boundary_values: Vec<SourceReferenceBoundaryValueV29>,
    boundary_sites: BTreeMap<SourceReferenceBoundaryKeyV29, usize>,
    children: Vec<usize>,
    enum_alternatives: Vec<SourceReferenceEnumAlternativeV29>,
    enum_alternative_types: BTreeMap<(u32, u32), Vec<usize>>,
    enum_members: Vec<usize>,
    enum_nodes: BTreeMap<u32, Vec<usize>>,
    enum_views: Vec<SourceReferenceEnumViewV29>,
    enum_view_nodes: BTreeMap<usize, Vec<usize>>,
    enum_observations: Vec<SourceReferenceEnumObservationV29>,
    // Retaining entry states makes the later ABI/CFG consumer use this exact plan.
    states: Vec<Vec<SourceReferenceLocalV29>>,
    storage_snapshots: Vec<source_storage_v29::SourceStorageSnapshotV29<'a>>,
    inactive_removals: Vec<SourceReferenceInactiveRemovalV29>,
    inactive_choices: Vec<usize>,
    blocks: Vec<SourceReferenceBlockV29>,
    entries: Vec<Option<usize>>,
    accesses: Vec<SourceReferenceAccessRecordV29>,
    access_sites: BTreeMap<Box<SourceReferenceAccessIndexKeyV29>, usize>,
    access_loans: Vec<usize>,
    selectors: Vec<SourceReferenceSelectorV29>,
    selector_sites: BTreeMap<SourceReferenceSelectorSiteV29, usize>,
    selector_values: BTreeMap<(usize, u32, SsaValueV1), usize>,
    selector_redefinitions: BTreeMap<(usize, u32), Vec<usize>>,
    descriptors: Vec<SourceReferenceDescriptorV29>,
    descriptor_sites: BTreeMap<SourceReferenceSelectorSiteV29, usize>,
    descriptor_holders: BTreeMap<(usize, usize), ()>,
    descriptor_values: BTreeMap<SourceReferenceSelectorSiteV29, SourceDescriptorValueV29>,
    descriptor_guards: Vec<SourceReferenceDescriptorGuardV29>,
    descriptor_guard_sites: BTreeMap<(usize, u32), usize>,
    epoch_sets: Vec<SourceReferenceEpochSetV29>,
    epoch_members: Vec<u32>,
    epoch_objects: BTreeMap<(usize, u32), Vec<usize>>,
    raw_origins: Vec<SourceReferenceRawOriginV29>,
    raw_origin_sites: BTreeMap<(usize, u32, usize, u32), usize>,
    raw_sets: Vec<SourceReferenceRawSetV29>,
    raw_choices: Vec<SourceReferenceRawChoiceV29>,
    raw_set_objects: BTreeMap<(usize, u32), Vec<usize>>,
    raw_nodes: BTreeMap<(u32, usize), usize>,
    raw_accesses: BTreeMap<Box<SourceReferenceRawAccessKeyV29>, Box<SourceReferenceRawAccessV29>>,
    address_observed: bool,
    returns: Vec<Option<usize>>,
    cells: SourceReferenceCellsV29,
    storage: SourceReferenceStorageV29,
}

struct SourceReferenceBuilderV29<'a, 'root, 'source> {
    plan: SourceReferencePlanV29<'a, 'source>,
    storage_requests: Option<&'a [source_storage_demands_v29::DemandV29]>,
    storage_root: Option<source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>>,
    storage_snapshot_indices: Vec<Option<usize>>,
    frames: Vec<Option<usize>>,
    visited: Vec<bool>,
    summaries: Vec<Option<SourceReferenceCallSummaryV29>>,
    epochs: Vec<Vec<usize>>,
    block_sites: BTreeMap<(usize, u32), usize>,
    loan_sites: BTreeMap<(usize, u32, usize), usize>,
    effect_sites: BTreeSet<(usize, u32, Option<usize>, usize, usize, u8)>,
    effect_site: Option<SourceReferenceSiteV29>,
    effect_ordinal: usize,
}

fn source_reference_error_v29(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

include!("production_source_reference_storage_transfer_v29.rs");
include!("production_source_reference_inactive_v29.rs");
include!("production_source_reference_construction_v29.rs");
include!("production_source_reference_memo_v29.rs");
include!("production_source_reference_enum_v29.rs");
include!("production_source_reference_enum_transport_v29.rs");
include!("production_source_reference_selectors_v29.rs");
include!("production_source_reference_descriptors_v29.rs");
include!("production_source_reference_epochs_v29.rs");
include!("production_source_reference_addresses_v29.rs");
include!("production_source_descriptor_facts_v29.rs");
include!("production_source_descriptor_transport_v29.rs");

fn source_reference_headers_v29<R>() -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    argument_sum_v1(&[
        size_of::<SourceReferenceBuilderV29<'_, '_, '_>>(),
        size_of::<SourceReferencePlanV29<'_, '_>>(),
        size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        // The promoted-only/cell-enabled entry wrappers share this custody body.
        size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        // Compatibility entry delegates to the original-demand-aware scope.
        size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>(),
        size_of::<Result<(), Box<dyn std::any::Any + Send>>>(),
        // emission_push -> emission_vec never reenters an owner traversal:
        // at most one replacement Vec header coexists with its paid owner.
        size_of::<Vec<usize>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        4 * size_of::<usize>(),
        size_of::<Option<ProductionSemanticKirErrorV1>>(),
        // The common body owns these envelopes even for a legacy None table.
        size_of::<Option<source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>>>(),
        size_of::<
            Result<
                source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>,
                ProductionSemanticKirErrorV1,
            >,
        >(),
        size_of::<Option<source_storage_v29::SourceStorageRootArenaV29<'_, '_>>>(),
        size_of::<Option<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>(),
        size_of::<Option<source_storage_v29::SourceStorageRootV29<'_, '_, '_>>>(),
        size_of::<Option<source_storage_v29::SourceStorageRootRefundV29<'_, '_>>>(),
        size_of::<(
            Option<ProductionSemanticKirErrorV1>,
            Result<
                source_storage_v29::SourceStorageRootRefundV29<'_, '_>,
                ProductionSemanticKirErrorV1,
            >,
        )>(),
        size_of::<Result<R, source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>>(),
        size_of::<
            Result<
                Result<R, source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>,
                Box<dyn std::any::Any + Send>,
            >,
        >(),
        size_of::<usize>(),
        size_of::<bool>(),
        source_reference_cleanup_headers_v29()?,
        // Only one owned payload iterator/type walk is live: Loan/EnumView
        // completes it before any recursive rebuild or consumer invocation.
        source_reference_owned_type_headers_v29()?,
    ])
}

const SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29: usize = 4;
const SOURCE_REFERENCE_ENTRY_WORK_V29: usize = 2 + 1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29;

fn source_reference_cleanup_headers_v29() -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    use std::panic::AssertUnwindSafe;
    type Payload = Box<dyn std::any::Any + Send>;
    argument_sum_v1(&[
        // Initial helper argument and protected move into its drop closure.
        size_of::<[Option<Payload>; 2]>(),
        size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>(),
        // A caught payload, its protected retry and the replacement result.
        argument_product_v1(2, size_of::<Payload>())?,
        size_of::<AssertUnwindSafe<Payload>>(),
        argument_product_v1(2, size_of::<Result<(), Payload>>())?,
        size_of::<std::ops::Range<usize>>(),
        size_of::<usize>(),
        size_of::<bool>(),
    ])
}

fn source_reference_discard_v29<T>(value: T) -> bool {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut result = catch_unwind(AssertUnwindSafe(|| drop(value)));
    let panicked = result.is_err();
    for _ in 0..SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29 {
        result = match result {
            Ok(()) => return panicked,
            Err(payload) => catch_unwind(AssertUnwindSafe(|| drop(payload))),
        };
    }
    if result.is_err() {
        // Returning would drop another hostile payload outside the fixed bound.
        // Reservations and the already selected first error remain live here.
        std::process::abort();
    }
    panicked
}

fn source_reference_scratch_v29<T>(
    capacity: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(std::mem::size_of::<Vec<T>>())?;
    emission_vec_v1(capacity, budget)
}

// Owner headers, backing and interrupted scratch remain paid through destruction.
// Valid callback-owned extra reservations stay paid. Lost custody never refunds
// caller credits. This private scope converts callback panics to a typed refusal.
fn with_source_reference_plan_v29<'work, R>(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_storage_scope_v29(
        instances,
        SourceReferenceStorageV29::PromotedOnly,
        None,
        budget,
        |plan, _, budget| consume(plan, budget).map_err(Into::into),
    )
}

fn with_source_reference_storage_plan_v29<'work, R>(
    instances: &ExecutionInstancesV29<'_>,
    storage: SourceReferenceStorageV29,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_storage_scope_v29(instances, storage, None, budget, |plan, _, budget| {
        consume(plan, budget).map_err(Into::into)
    })
}

fn with_source_reference_storage_scope_v29<'source, 'work, R>(
    instances: &ExecutionInstancesV29<'source>,
    storage: SourceReferenceStorageV29,
    layouts: Option<&mut source_storage_v29::SourceStorageLayoutsV29<'source>>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        Option<source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        R,
        source_storage_v29::SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_descriptor_scope_v29(instances, storage, layouts, None, budget, consume)
}

fn with_source_reference_descriptor_scope_v29<'source, 'work, R>(
    instances: &ExecutionInstancesV29<'source>,
    storage: SourceReferenceStorageV29,
    layouts: Option<&mut source_storage_v29::SourceStorageLayoutsV29<'source>>,
    descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        Option<source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        R,
        source_storage_v29::SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_descriptor_demands_scope_v29(
        instances,
        storage,
        layouts,
        descriptor_root,
        None,
        budget,
        consume,
    )
}

fn with_source_reference_descriptor_demands_scope_v29<'source, 'work, R>(
    instances: &ExecutionInstancesV29<'source>,
    storage: SourceReferenceStorageV29,
    layouts: Option<&mut source_storage_v29::SourceStorageLayoutsV29<'source>>,
    descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>,
    storage_demands: Option<source_storage_demands_v29::RootStorageDemandsV29<'_, 'source>>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        Option<source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        R,
        source_storage_v29::SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use source_storage_v29::{SourceStorageRootCallbackErrorV29, SourceStorageRootCheckpointV29};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    // Prepay all cleanup attempts atomically before construction or callbacks.
    let work = SOURCE_REFERENCE_ENTRY_WORK_V29
        .checked_add(if layouts.is_some() {
            SourceStorageRootCheckpointV29::additional_work()
        } else {
            0
        })
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const ArgumentBudgetV1<'_> as usize;
    let checkpoint = layouts
        .map(|layouts| SourceStorageRootCheckpointV29::begin(layouts, instances, budget))
        .transpose()?;
    let mut retained_floor = None;
    let mut first_failure = None;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(work)?;
        budget.reserve_storage(source_reference_headers_v29::<R>()?)?;
        let arena = checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.arena(budget))
            .transpose()?;
        // This local owns every C2 value. Its address stays stable while the
        // later-declared Plan and all callback views borrow it; it drops last.
        let mut plan = SourceReferenceBuilderV29::new_with_descriptor_demands(
            instances,
            storage,
            arena.as_ref().map(|arena| arena.view()),
            descriptor_root,
            storage_demands,
            budget,
        )?
        .build(budget)?;
        plan.retained_floor = budget.storage();
        retained_floor = Some(plan.retained_floor);
        if let Some(checkpoint) = &checkpoint {
            checkpoint.retained(plan.retained_floor, budget)?;
        }
        let result = match catch_unwind(AssertUnwindSafe(|| {
            consume(&plan, arena.as_ref().map(|arena| arena.view()), budget)
        })) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(SourceStorageRootCallbackErrorV29::Owned(error))) => Err(error),
            Ok(Err(SourceStorageRootCallbackErrorV29::Recorded(recorded))) => {
                if !plan.failure.matches_recorded(&recorded) {
                    Err(ArgumentResourceV1::Accounting.into())
                } else {
                    Err(plan
                        .failure
                        .first_error()
                        .unwrap_or_else(|| ArgumentResourceV1::Accounting.into()))
                }
            }
            Err(payload) => {
                payloads[0] = Some(payload);
                Err(source_reference_error_v29(
                    "source reference callback panicked",
                ))
            }
        };
        // Moving an owned legacy slot consumes its diagnostic. Integrated slots
        // remain owned by the checkpoint until every plan/arena backing is gone.
        first_failure = plan.failure.into_owned_first();
        result
    })) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(source_reference_error_v29(
                "source reference construction panicked",
            ))
        }
    };
    let mut root_refund = None;
    let mut root_lost = false;
    if let Some(checkpoint) = checkpoint {
        if let Err(error) = result {
            checkpoint.record(error);
            result = Err(checkpoint
                .observation()
                .unwrap_or_else(|| ArgumentResourceV1::Accounting.into()));
        }
        let (first, refund) = checkpoint.prepare_refund(budget);
        first_failure = first;
        match refund {
            Ok(refund) => root_refund = Some(refund),
            Err(error) => {
                root_lost = true;
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
    }
    let required = retained_floor.unwrap_or(budget.storage());
    let same = slot == budget as *const ArgumentBudgetV1<'_> as usize
        && ledger == budget.work_ledger_identity_v1();
    let lost = root_lost || !same || budget.storage() < required;
    if first_failure.is_some() || lost {
        // An earlier query failure wins; an existing callback error also survives
        // a later cleanup failure. No rejected R outlives its reservation.
        let replacement = first_failure;
        if replacement.is_some() || result.is_ok() {
            let rejected = std::mem::replace(
                &mut result,
                Err(replacement.unwrap_or_else(|| ArgumentResourceV1::Accounting.into())),
            );
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(payload);
            }
        }
    }
    // The first error is selected above before prepaid, bounded cleanup. No
    // unrelated ledger is charged and no reservation is refunded while cleanup
    // runs. A non-returning destructor still requires external supervision.
    if source_reference_discard_v29(payloads) {
        if result.is_ok() {
            result = Err(source_reference_error_v29(
                "source reference destructor panicked",
            ));
        }
    }
    if let Some(refund) = root_refund {
        if let Err(error) = refund.release(budget)
            && result.is_ok()
        {
            result = Err(error);
        }
    } else if same && !lost {
        let owned = required
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if let Err(error) = budget.release_storage(owned)
            && result.is_ok()
        {
            result = Err(error.into());
        }
    }
    result
}

impl<'a, 'root, 'source> SourceReferenceBuilderV29<'a, 'root, 'source> {
    fn new(
        instances: &'a ExecutionInstancesV29<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_storage(instances, SourceReferenceStorageV29::PromotedOnly, budget)
    }

    fn new_with_storage(
        instances: &'a ExecutionInstancesV29<'source>,
        storage: SourceReferenceStorageV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_root(instances, storage, None, budget)
    }

    fn new_with_root(
        instances: &'a ExecutionInstancesV29<'source>,
        storage: SourceReferenceStorageV29,
        storage_root: Option<source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_descriptors(instances, storage, storage_root, None, budget)
    }

    fn new_with_descriptors(
        instances: &'a ExecutionInstancesV29<'source>,
        storage: SourceReferenceStorageV29,
        storage_root: Option<source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>>,
        descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'a>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_descriptor_demands(
            instances,
            storage,
            storage_root,
            descriptor_root,
            None,
            budget,
        )
    }

    fn new_with_descriptor_demands(
        instances: &'a ExecutionInstancesV29<'source>,
        storage: SourceReferenceStorageV29,
        storage_root: Option<source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>>,
        descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'a>>,
        storage_demands: Option<source_storage_demands_v29::RootStorageDemandsV29<'a, 'source>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let storage_requests = if let Some(demands) = storage_demands {
            if storage_root.is_none() {
                return Err(source_reference_error_v29(
                    "original storage demands require their source layout root",
                ));
            }
            Some(demands.requests(instances, budget)?.0)
        } else {
            None
        };
        let has_storage_demands = storage_requests.is_some_and(|requests| !requests.is_empty());
        if let Some(profile) = descriptor_root {
            profile.check_instances(instances, budget)?;
        }
        let count = instances.instances().len();
        let mut entries = emission_vec_v1(count, budget)?;
        let mut frames = emission_vec_v1(count, budget)?;
        let mut visited = emission_vec_v1(count, budget)?;
        let mut summaries = emission_vec_v1(count, budget)?;
        let mut epochs = emission_vec_v1(count, budget)?;
        let mut returns = if storage == SourceReferenceStorageV29::ScalarCells {
            emission_vec_v1(count, budget)?
        } else {
            Vec::new()
        };
        budget.charge_work(argument_product_v1(count, 5)?)?;
        entries.resize(count, None);
        frames.resize(count, None);
        visited.resize(count, false);
        summaries.resize_with(count, || None);
        epochs.resize_with(count, Vec::new);
        if storage == SourceReferenceStorageV29::ScalarCells {
            budget.charge_work(count)?;
            returns.resize(count, None);
        }
        Ok(Self {
            plan: SourceReferencePlanV29 {
                instances,
                source: *instances.owner().source_semantic_sha256(),
                ssa: instances.owner().identity(),
                root: instances.root(),
                ledger: budget.work_ledger_identity_v1(),
                slot: budget as *const ArgumentBudgetV1<'_> as usize,
                retained_floor: budget.storage(),
                failure: storage_root.as_ref().map_or_else(
                    source_storage_v29::SourceReferenceFailureV29::owned,
                    |root| root.custody_view().failure(),
                ),
                storage_root: storage_root.as_ref().map(|root| root.custody_view()),
                storage_demands,
                has_storage_demands,
                descriptor_root,
                descriptor_sets: Vec::new(),
                descriptor_origins: Vec::new(),
                descriptor_set_index: BTreeMap::new(),
                origins: Vec::new(),
                projections: Vec::new(),
                loans: Vec::new(),
                external_borrows: Vec::new(),
                nodes: Vec::new(),
                children: Vec::new(),
                enum_alternatives: Vec::new(),
                enum_alternative_types: BTreeMap::new(),
                enum_members: Vec::new(),
                enum_nodes: BTreeMap::new(),
                enum_views: Vec::new(),
                enum_view_nodes: BTreeMap::new(),
                enum_observations: Vec::new(),
                selected_storage: Vec::new(),
                representation_demands: Vec::new(),
                representation_demand_sites: BTreeMap::new(),
                storage_activations: Vec::new(),
                storage_activation_sites: BTreeMap::new(),
                boundary_values: Vec::new(),
                boundary_sites: BTreeMap::new(),
                states: Vec::new(),
                storage_snapshots: Vec::new(),
                inactive_removals: Vec::new(),
                inactive_choices: Vec::new(),
                blocks: Vec::new(),
                entries,
                accesses: Vec::new(),
                access_sites: BTreeMap::new(),
                access_loans: Vec::new(),
                selectors: Vec::new(),
                selector_sites: BTreeMap::new(),
                selector_values: BTreeMap::new(),
                selector_redefinitions: BTreeMap::new(),
                descriptors: Vec::new(),
                descriptor_sites: BTreeMap::new(),
                descriptor_holders: BTreeMap::new(),
                descriptor_values: BTreeMap::new(),
                descriptor_guards: Vec::new(),
                descriptor_guard_sites: BTreeMap::new(),
                epoch_sets: Vec::new(),
                epoch_members: Vec::new(),
                epoch_objects: BTreeMap::new(),
                raw_origins: Vec::new(),
                raw_origin_sites: BTreeMap::new(),
                raw_sets: Vec::new(),
                raw_choices: Vec::new(),
                raw_set_objects: BTreeMap::new(),
                raw_nodes: BTreeMap::new(),
                raw_accesses: BTreeMap::new(),
                address_observed: false,
                returns,
                cells: SourceReferenceCellsV29::default(),
                storage,
            },
            storage_root,
            storage_requests,
            storage_snapshot_indices: Vec::new(),
            frames,
            visited,
            summaries,
            epochs,
            block_sites: BTreeMap::new(),
            loan_sites: BTreeMap::new(),
            effect_sites: BTreeSet::new(),
            effect_site: None,
            effect_ordinal: 0,
        })
    }

    fn build(
        mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferencePlanV29<'a, 'source>, ProductionSemanticKirErrorV1> {
        self.collect_storage_selectors(budget)?;
        self.function(self.plan.root, None, budget)?;
        budget.charge_work(self.visited.len())?;
        for (ordinal, &visited) in self.visited.iter().enumerate() {
            let instance = self
                .plan
                .instances
                .id_at(ordinal)
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            if self.plan.instances.instance_reachable(instance) != Some(visited) {
                return Err(source_reference_error_v29(
                    "source reference call roster is incomplete",
                ));
            }
        }
        #[cfg(test)]
        if let Some(audit) = SOURCE_EXTERNAL_METADATA_AUDIT_V30.get() {
            audit(&mut self.plan, budget)?;
        }
        check_source_external_metadata_complete_v30(&self.plan, budget)?;
        self.finish_effects(budget)?;
        if self.plan.storage == SourceReferenceStorageV29::ScalarCells {
            self.plan_scalar_cells(budget)?;
        }
        // All scratch capacities were charged on construction and remain charged
        // through this owner's callback. No dropped scratch is reused as credit.
        Ok(self.plan)
    }

    fn node(
        &mut self,
        ty: SemanticTypeIdV1,
        kind: SourceReferenceNodeKindV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let id = self.plan.nodes.len();
        emission_push_v1(
            &mut self.plan.nodes,
            SourceReferenceNodeV29 {
                ty,
                kind,
                value_origin: None,
                storage: None,
                inactive: None,
                descriptor: None,
            },
            budget,
        )?;
        Ok(id)
    }

    fn plain(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.node(ty, SourceReferenceNodeKindV29::Plain(None), budget)
    }

    fn clone_state(
        &mut self,
        id: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let source = self.plan.states.get(id).ok_or_else(|| {
            source_reference_error_v29("source reference state is outside its owner")
        })?;
        let mut state = source_reference_scratch_v29(source.len(), budget)?;
        budget.charge_work(source.len())?;
        state.extend_from_slice(source);
        let index = self.plan.states.len();
        emission_push_v1(&mut self.plan.states, state, budget)?;
        Ok(index)
    }

    fn frame(
        &self,
        instance: ProductionCallInstanceIdV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.frames
            .get(instance.index())
            .copied()
            .flatten()
            .ok_or_else(|| {
                source_reference_error_v29("source reference instance has no live frame")
            })
    }

    fn local(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
    ) -> Result<SourceReferenceLocalV29, ProductionSemanticKirErrorV1> {
        self.plan.states[self.frame(instance)?]
            .get(local.index() as usize)
            .copied()
            .ok_or_else(|| {
                source_reference_error_v29("source reference local is outside its frame")
            })
    }

    fn set_local(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        value: SourceReferenceLocalV29,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let state = self.frame(instance)?;
        let target = self.plan.states[state]
            .get_mut(local.index() as usize)
            .ok_or_else(|| {
                source_reference_error_v29("source reference local is outside its frame")
            })?;
        *target = value;
        Ok(())
    }
}

// This census grants no representation or pointer authority. A negative result
// lets the existing lowering path run without creating any SourceReference.
fn source_reference_borrows_present_v29(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut present = false;
    for row in instances.instances() {
        budget.charge_work(1)?;
        for (block, declaration) in row.declaration().blocks().iter().enumerate() {
            budget.charge_work(1)?;
            if !row.ssa().plan().is_reachable(SsaBlockIdV1::new(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            )) {
                continue;
            }
            for statement in declaration.statements() {
                budget.charge_work(1)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        present |= match assignment.value().kind() {
                            SemanticRvalueKindV1::Borrow { .. } => true,
                            SemanticRvalueKindV1::Use(_)
                            | SemanticRvalueKindV1::Unary { .. }
                            | SemanticRvalueKindV1::Binary { .. }
                            | SemanticRvalueKindV1::CheckedBinary(_)
                            | SemanticRvalueKindV1::UncheckedBinary(_)
                            | SemanticRvalueKindV1::Cast { .. }
                            | SemanticRvalueKindV1::AddressOf { .. }
                            | SemanticRvalueKindV1::Length(_)
                            | SemanticRvalueKindV1::Discriminant(_)
                            | SemanticRvalueKindV1::Aggregate(_)
                            | SemanticRvalueKindV1::Load(_) => false,
                        };
                    }
                    SemanticStatementKindV1::Store(_)
                    | SemanticStatementKindV1::AtomicRmw(_)
                    | SemanticStatementKindV1::AtomicCompareExchange(_)
                    | SemanticStatementKindV1::SetDiscriminant { .. }
                    | SemanticStatementKindV1::Deinitialize(_)
                    | SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Assume(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            budget.charge_work(1)?;
            match declaration.terminator().kind() {
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::SwitchInt { .. }
                | SemanticTerminatorKindV1::Call(_)
                | SemanticTerminatorKindV1::TailCall(_)
                | SemanticTerminatorKindV1::Drop { .. }
                | SemanticTerminatorKindV1::Assert { .. }
                | SemanticTerminatorKindV1::FalseEdge { .. }
                | SemanticTerminatorKindV1::Return
                | SemanticTerminatorKindV1::UnwindResume
                | SemanticTerminatorKindV1::UnwindTerminate
                | SemanticTerminatorKindV1::Abort
                | SemanticTerminatorKindV1::Unreachable => {}
            }
        }
    }
    Ok(present)
}

include!("production_source_reference_cfg_v29.rs");

impl SourceReferencePlanV29<'_, '_> {
    // This constant-size predicate is shared with the prepaid, consuming abort
    // path. Ordinary queries must still go through check_owner and its debit.
    fn retains_custody(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        budget: &ArgumentBudgetV1<'_>,
    ) -> bool {
        self.slot == budget as *const ArgumentBudgetV1<'_> as usize
            && self.ledger == budget.work_ledger_identity_v1()
            && budget.storage() >= self.retained_floor
            && std::ptr::eq(self.instances, instances)
            && self.root == instances.root()
            && self.source == *instances.owner().source_semantic_sha256()
            && self.ssa == instances.owner().identity()
            && self
                .storage_root
                .as_ref()
                .is_none_or(|arena| arena.retains_custody(instances, &self.failure, budget))
    }

    fn check_owner(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(error) = self.failure.first_error() {
            return Err(error);
        }
        if !self.retains_custody(instances, budget) {
            self.failure.record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.charge(5, budget)?;
        Ok(())
    }

    fn charge(
        &self,
        work: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(error) = self.failure.first_error() {
            return Err(error);
        }
        budget.charge_work(work).map_err(|error| {
            self.failure.record_resource(error);
            error.into()
        })
    }

    fn loan_at(
        &self,
        site: SourceReferenceSiteV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&SourceReferenceLoanV29>, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        self.charge(self.loans.len(), budget)?;
        Ok(self.loans.iter().find(|loan| loan.site == site))
    }

    fn require_promoted(
        &self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceRepresentationV29, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        self.charge(1, budget)?;
        match self.loans.get(loan).map(|loan| loan.representation) {
            Some(SourceReferenceRepresentationV29::NeedsAddressable(_)) => {
                Err(source_reference_error_v29(
                    "source reference requires checked addressable storage and writeback",
                ))
            }
            Some(representation) => Ok(representation),
            None => Err(source_reference_error_v29(
                "source reference loan is outside its owner",
            )),
        }
    }
}
