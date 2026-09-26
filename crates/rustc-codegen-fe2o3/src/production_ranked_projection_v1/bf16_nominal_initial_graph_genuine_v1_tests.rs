//! Genuine complete-for-profile graph loan; actual owners only, never admission.
//! Existing telemetry remains an initial-row observation, not access readiness.
//! Source-rescan oracle is independent of populate_initial_graph_v1.
use super::*;
use crate::production_ranked_projection_v1::{
    CapabilityEdgeKindV1, CapabilityEdgeV1, PendingEnumPayloadLoadV1, PendingEnumPayloadStoreV1,
    SemanticAggregateKindV1, SemanticFunctionIdV1, SemanticLocalIdV1, SemanticRvalueKindV1,
    SemanticStatementKindV1,
    bf16_nominal_source_preparation_v1::{
        RichNominalSourceTablesV1, with_nominal_rich_source_preparation_v1,
    },
    canonical_assertion_facts_v1::{
        CanonicalAssertionErrorV1, CanonicalAssertionSessionV1, CanonicalSourceAssertionFactsV1,
        bf16_nominal_recipe_resources_v1::{
            NominalRecipeResourcesV1, with_nominal_recipe_resources_v1,
        },
        with_nominal_canonical_facts_observation_v1,
    },
    enum_payload_projection, raw_operand_place, transparent_operand_place,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1, CheckedBf16NominalCallV1,
    ProductionPreRankedKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1;
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

type QueryResult<T> = std::result::Result<T, QueryError>;
const HEADERS: usize = 16 * 1024;
const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;
const REFUSAL: &str = "genuine initial graph callback refusal";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Observe,
    Surplus,
    Error,
    Panic,
    Occupied,
    ForeignFunction,
    MissingFunction,
    ForeignCorrespondence,
    ForeignInventory,
    Masked,
    DenyWork,
    DenyStorage,
    Undercut,
    ReplaceLedger,
}
#[derive(Default)]
struct Events {
    prepaid: Cell<usize>,
    checked: Cell<usize>,
    rich: Cell<usize>,
    facts: Cell<usize>,
    context: Cell<usize>,
    loan: Cell<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    blocks: usize,
    locals: usize,
    edges: usize,
    aliases: usize,
    enum_edges: usize,
    stores: usize,
    loads: usize,
    borrowed: usize,
    calls: usize,
    owned: usize,
}
#[derive(Clone, Copy)]
struct Custody {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    work: usize,
    peak: usize,
}
impl Custody {
    fn take(budget: &Budget<'_>) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        }
    }
    fn check(self, budget: &Budget<'_>, owned: usize) -> QueryResult<()> {
        let floor = self.floor.checked_add(owned).ok_or(Resource::Arithmetic)?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
fn add(a: usize, b: usize) -> QueryResult<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn projection(error: Error) -> QueryError {
    match error {
        Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error)) => {
            QueryError::Resource(error)
        }
        Error::Incomplete(reason) | Error::Unsupported(reason) => QueryError::Unavailable(reason),
        Error::CanonicalAssertions(CanonicalAssertionErrorV1::Origin(_)) => {
            QueryError::Unavailable("genuine initial graph source correspondence refused")
        }
        _ => QueryError::Unavailable("genuine initial graph canonical query refused"),
    }
}
fn pay_statement(
    statement: &SemanticStatementV1,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
) -> Result<()> {
    context.with_resources(|resources| {
        resources.work(128)?;
        if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
            let place_work = |place: &crate::production_ranked_projection_v1::SemanticPlaceV1| {
                place
                    .projections()
                    .len()
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(16))
                    .ok_or_else(|| resource(Resource::Arithmetic))
            };
            resources.work(place_work(assignment.destination())?)?;
            assignment.value().kind().try_visit_operands(|operand| {
                resources.work(16)?;
                if let Some(place) = raw_operand_place(operand) {
                    resources.work(place_work(place)?)?;
                }
                Ok::<_, Error>(())
            })?;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } => {
                    resources.work(place_work(place)?)?
                }
                _ => {}
            }
        }
        Ok(())
    })
}
fn store_at(
    statement: &SemanticStatementV1,
    block: usize,
    ordinal: usize,
) -> Option<PendingEnumPayloadStoreV1> {
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return None;
    };
    if !assignment.destination().projections().is_empty() {
        return None;
    }
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return None;
    };
    let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() else {
        return None;
    };
    let [operand] = aggregate.operands() else {
        return None;
    };
    let place = transparent_operand_place(operand)?;
    Some(PendingEnumPayloadStoreV1 {
        carrier: assignment.destination().local().index() as usize,
        variant: *variant,
        source: place.local().index() as usize,
        construction_block: block,
        statement: ordinal,
    })
}

// The oracle's equality primitive is independently exercised on inert rows.
// It does not call the graph builder or turn these rows into a graph authority.
fn require_expected_edge_at(
    rows: &[Vec<CapabilityEdgeV1>],
    source: usize,
    ordinal: usize,
    expected: CapabilityEdgeV1,
) -> Result<()> {
    let edge = rows
        .get(source)
        .and_then(|row| row.get(ordinal))
        .ok_or(Error::Incomplete(
            "genuine graph expected source edge absent",
        ))?;
    if *edge != expected {
        return Err(Error::Incomplete("genuine graph source edge differs"));
    }
    Ok(())
}
fn require_exhausted_row(row: &[CapabilityEdgeV1], consumed: usize) -> Result<()> {
    if row.len() != consumed {
        return Err(Error::Incomplete(
            "genuine graph unconsumed or missing source edge",
        ));
    }
    Ok(())
}

fn expect_edge(
    view: &NominalCompleteForProfileGraphV1<'_>,
    cursors: &mut [usize],
    source: usize,
    expected: CapabilityEdgeV1,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
) -> Result<()> {
    context.with_resources(|resources| resources.work(64))?;
    let ordinal = *cursors
        .get(source)
        .ok_or(Error::Incomplete("genuine graph source row absent"))?;
    require_expected_edge_at(view.edges(), source, ordinal, expected)?;
    cursors[source] = ordinal
        .checked_add(1)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok(())
}
fn family(operation: &SemanticCompilerIntrinsicOperationV1) -> Option<&'static str> {
    Some(match operation {
        SemanticCompilerIntrinsicOperationV1::Trap => "Trap",
        SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { .. } => "MatrixContextCurrent",
        SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { .. } => "WaveLaneCurrent",
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor { .. } => {
            "Bf16MatrixViewRowMajor"
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewColumnMajor { .. } => {
            "Bf16MatrixViewColumnMajor"
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. } => "Bf16MatrixLoad",
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. } => {
            "Bf16MatrixLoadZeroFilledV2"
        }
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. } => {
            "F32MatrixAccumulatorZero"
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => "ThreadIndex1d",
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. } => "DisjointSliceGetMut",
        _ => return None,
    })
}

// Independent source rescan: no graph builder, stored pending-store table or
// sort is reused. Exact row order and every source association are checked.
fn observe_view(
    view: &NominalCompleteForProfileGraphV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    cursors: &mut [usize],
    telemetry: bool,
) -> Result<Observation> {
    context.with_resources(|resources| resources.work(128))?;
    let function = view.function();
    assert!(std::ptr::eq(function, rich.function()));
    assert!(std::ptr::eq(function, context.function()));
    assert_eq!(cursors.len(), function.locals().len());
    assert_eq!(view.edges().len(), function.locals().len());
    let callables = checked
        .emission()
        .owner()
        .semantic_ssa()
        .source_semantic()
        .callables();
    let mut observation = Observation {
        blocks: function.blocks().len(),
        locals: function.locals().len(),
        edges: 0,
        aliases: 0,
        enum_edges: 0,
        stores: 0,
        loads: 0,
        borrowed: 0,
        calls: 0,
        owned: 0,
    };
    for (block_index, block) in function.blocks().iter().enumerate() {
        context.with_resources(|resources| resources.work(64))?;
        for (ordinal, statement) in block.statements().iter().enumerate() {
            pay_statement(statement, context)?;
            if let Some(store) = store_at(statement, block_index, ordinal) {
                let mut matches = 0usize;
                for retained in &view.initial.graph.stores {
                    context.with_resources(|resources| resources.work(64))?;
                    matches += usize::from(*retained == store);
                }
                assert_eq!(matches, 1, "each original store retained exactly once");
                observation.stores += 1;
                continue;
            }
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let destination = assignment.destination().local().index() as usize;
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    if let Some((carrier, variant)) =
                        raw_operand_place(operand).and_then(enum_payload_projection)
                    {
                        context.with_resources(|resources| resources.work(64))?;
                        assert_eq!(
                            view.initial.graph.loads.get(observation.loads),
                            Some(&PendingEnumPayloadLoadV1 {
                                carrier,
                                variant,
                                destination,
                                use_block: block_index,
                                statement: ordinal,
                            })
                        );
                        observation.loads += 1;
                    }
                    transparent_operand_place(operand)
                }
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.projections().is_empty() =>
                {
                    context.with_resources(|resources| resources.work(64))?;
                    assert_eq!(
                        view.initial.graph.borrowed.get(observation.borrowed),
                        Some(&(place.local().index() as usize, block_index))
                    );
                    observation.borrowed += 1;
                    Some(place)
                }
                _ => None,
            };
            if let Some(source) = source {
                expect_edge(
                    view,
                    cursors,
                    source.local().index() as usize,
                    CapabilityEdgeV1 {
                        destination,
                        use_block: block_index,
                        kind: CapabilityEdgeKindV1::Alias,
                    },
                    context,
                )?;
                observation.edges += 1;
                observation.aliases += 1;
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            context.with_resources(|resources| resources.work(64))?;
            let label = match callables.get(call.callee().index() as usize) {
                Some(SemanticCallableDeclV1::Defined { .. })
                    if std::ptr::eq(call, checked.source_call()) =>
                {
                    "Defined"
                }
                Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => {
                    family(operation).ok_or(Error::Incomplete(
                        "genuine graph unexpected source intrinsic",
                    ))?
                }
                _ => {
                    return Err(Error::Incomplete(
                        "genuine graph unexpected source callable",
                    ));
                }
            };
            observation.calls += 1;
            if telemetry {
                // Capped test telemetry; formatting/I/O is not compiler-ledger work.
                eprintln!(
                    "fe2o3-initial-graph-call-v1 block={} family={}",
                    block_index, label
                );
            }
        }
    }
    // Reconstruct enum joins by scanning ORIGINAL source for each original load.
    // No sort/store list from the implementation is reused as an oracle.
    for (block_index, block) in function.blocks().iter().enumerate() {
        context.with_resources(|resources| resources.work(64))?;
        for (ordinal, statement) in block.statements().iter().enumerate() {
            pay_statement(statement, context)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                continue;
            };
            let Some((carrier, variant)) =
                raw_operand_place(operand).and_then(enum_payload_projection)
            else {
                continue;
            };
            let mut found = None;
            for (store_block, block) in function.blocks().iter().enumerate() {
                context.with_resources(|resources| resources.work(64))?;
                for (store_ordinal, statement) in block.statements().iter().enumerate() {
                    pay_statement(statement, context)?;
                    if let Some(store) = store_at(statement, store_block, store_ordinal)
                        && store.carrier == carrier
                        && store.variant == variant
                        && found.replace(store).is_some()
                    {
                        return Err(Error::Incomplete(
                            "genuine graph source enum store is ambiguous",
                        ));
                    }
                }
            }
            let Some(store) = found else {
                continue;
            };
            context.with_resources(|resources| resources.work(4 * usize::BITS as usize + 64))?;
            let kind = if store.construction_block == block_index && store.statement < ordinal {
                observation.aliases += 1;
                CapabilityEdgeKindV1::Alias
            } else {
                if rich.scalar_counts().get(carrier).copied() != Some(1) {
                    continue;
                }
                let Some(availability) = rich
                    .enum_payload_dominance()
                    .availability(SemanticLocalIdV1::from_index(carrier as u32), variant)
                else {
                    continue;
                };
                observation.enum_edges += 1;
                CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block: store.construction_block,
                    availability,
                }
            };
            expect_edge(
                view,
                cursors,
                store.source,
                CapabilityEdgeV1 {
                    destination: assignment.destination().local().index() as usize,
                    use_block: block_index,
                    kind,
                },
                context,
            )?;
            observation.edges += 1;
        }
    }
    let mut logged = 0usize;
    for (source, row) in view.edges().iter().enumerate() {
        context.with_resources(|resources| resources.work(64))?;
        require_exhausted_row(row, cursors[source])?;
        for (ordinal, edge) in row.iter().enumerate() {
            context.with_resources(|resources| resources.work(64))?;
            if telemetry && logged < 32 {
                match edge.kind {
                    CapabilityEdgeKindV1::Alias => eprintln!(
                        "fe2o3-initial-graph-edge-v1 source_local={} row={} destination={} use_block={} kind=alias",
                        source, ordinal, edge.destination, edge.use_block
                    ),
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                        construction_block, ..
                    } => eprintln!(
                        "fe2o3-initial-graph-edge-v1 source_local={} row={} destination={} use_block={} kind=enum_payload construction_block={}",
                        source, ordinal, edge.destination, edge.use_block, construction_block
                    ),
                    _ => return Err(Error::Incomplete("genuine graph noninitial edge kind")),
                }
                logged += 1;
            }
        }
    }
    assert_eq!(observation.edges, view.edge_count());
    assert_eq!(observation.stores, view.initial.graph.stores.len());
    for pair in view.initial.graph.stores.windows(2) {
        context.with_resources(|resources| resources.work(64))?;
        assert!((pair[0].carrier, pair[0].variant) <= (pair[1].carrier, pair[1].variant));
    }
    assert_eq!(observation.loads, view.initial.graph.loads.len());
    assert_eq!(observation.borrowed, view.initial.graph.borrowed.len());
    Ok(observation)
}

#[allow(clippy::too_many_arguments)]
fn enter_context<'w>(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, 'w>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    source: &CheckedBf16CallInstanceV1<'_>,
    mode: Mode,
    owned: &mut usize,
    pending: &mut PendingNominalInitialGraphV1,
    cursors: &mut Option<Vec<usize>>,
    events: &Events,
    replacement: &mut Option<Budget<'w>>,
    storage_limit: usize,
    protected: &Cell<usize>,
) -> Result<Observation> {
    with_nominal_recipe_resources_v1(facts, rich, owned, |context| {
        events.context.set(events.context.get() + 1);
        // These are NEGATIVE-ONLY shadows of real facts, disposed by inspect_facts.
        // Modify no live production facts and construct no counterfeit graph.
        match mode {
            Mode::ForeignFunction => context.facts.semantic_function = source.helper(),
            Mode::MissingFunction => {
                context.facts.semantic_function = SemanticFunctionIdV1::from_index(u32::MAX)
            }
            Mode::ForeignCorrespondence => context.facts.correspondence_owner = source.helper(),
            _ => {}
        }
        assert!(cursors.is_none());
        context.with_resources(|resources| {
            *cursors = Some(Vec::new());
            let cursor_values = cursors
                .as_mut()
                .expect("empty outer cursors just installed");
            let count = rich.function().locals().len();
            resources.work(count)?;
            resources.reserve(cursor_values, count)?;
            cursor_values.resize(count, 0usize);
            Ok(())
        })?;
        let observed = context.with_complete_for_profile_graph_v1(
            checked,
            rich,
            pending,
            |view, context| {
                events.loan.set(events.loan.get() + 1);
                let observation = observe_view(
                    &view,
                    rich,
                    checked,
                    context,
                    cursors.as_mut().expect("paid outer cursors"),
                    mode == Mode::Observe,
                )?;
                // Work, storage floor and identity are checked at the actual loan.
                protected.set(context.facts.budget.storage());
                if matches!(mode, Mode::Surplus | Mode::Error | Mode::Panic) {
                    context.with_facts(|facts| {
                        facts.reserve_scalar_private_storage_v1(23)?;
                        facts.charge_private_array_work(17)
                    })?;
                }
                match mode {
                    Mode::Error => return Err(Error::Incomplete(REFUSAL)),
                    Mode::Panic => panic!("genuine initial graph callback panic"),
                    Mode::DenyWork => {
                        context.with_resources(|resources| resources.work(PROBE_WORK + 1))?
                    }
                    Mode::DenyStorage => context.with_resources(|resources| {
                        resources.reserve_storage(
                            storage_limit
                                .checked_add(1)
                                .ok_or_else(|| resource(Resource::Arithmetic))?,
                        )
                    })?,
                    Mode::Undercut => context.facts.budget.release_storage(1).map_err(resource)?,
                    Mode::ReplaceLedger => {
                        *context.facts.budget =
                            replacement.take().expect("negative-only foreign ledger");
                    }
                    _ => {}
                }
                Ok(observation)
            },
        )?;
        if mode == Mode::Occupied {
            // No graph replacement or second loan, even on the same source/ledger.
            return context.with_complete_for_profile_graph_v1(checked, rich, pending, |_, _| {
                panic!("occupied pending graph was loaned twice");
            });
        }
        Ok(observed)
    })
}

#[allow(clippy::too_many_arguments)]
fn inspect_facts<'w>(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, 'w>,
    rich: &RichNominalSourceTablesV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    checked: &CheckedBf16NominalCallV1<'_>,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    facts_inventory: &CanonicalKirInventoryV1<'_>,
    mode: Mode,
    owned: &mut usize,
    pending: &mut PendingNominalInitialGraphV1,
    cursors: &mut Option<Vec<usize>>,
    events: &Events,
    before: Custody,
    replacement: &mut Option<Budget<'w>>,
    storage_limit: usize,
    protected: &Cell<usize>,
) -> QueryResult<Observation> {
    events.facts.set(events.facts.get() + 1);
    assert_eq!(facts.budget as *const Budget<'_> as usize, before.slot);
    assert!(facts.budget.work_ledger_identity_v1() == before.ledger);
    assert!(std::ptr::eq(facts.owner, owner));
    assert!(facts.report.belongs_to(facts_inventory));
    assert!(checked.belongs_to(inventory));
    assert!(std::ptr::eq(checked.emission().owner(), owner));
    assert!(std::ptr::eq(checked.source_call(), source.source_call()));
    let result = match mode {
        Mode::ForeignFunction | Mode::MissingFunction | Mode::ForeignCorrespondence => {
            let mut shadow = CanonicalSourceAssertionFactsV1 {
                owner: facts.owner,
                origins: facts.origins,
                report: facts.report,
                budget: &mut *facts.budget,
                correspondence_owner: facts.correspondence_owner,
                semantic_function: facts.semantic_function,
                masked: None,
            };
            enter_context(
                &mut shadow,
                rich,
                checked,
                source,
                mode,
                owned,
                pending,
                cursors,
                events,
                replacement,
                storage_limit,
                protected,
            )
        }
        Mode::Masked => {
            let mut session = CanonicalAssertionSessionV1 {
                owner: facts.owner,
                origins: facts.origins,
                report: facts.report,
                budget: &mut *facts.budget,
            };
            session.with_source_masked_assertions_v1(source.root(), source.root(), |masked| {
                enter_context(
                    masked,
                    rich,
                    checked,
                    source,
                    mode,
                    owned,
                    pending,
                    cursors,
                    events,
                    replacement,
                    storage_limit,
                    protected,
                )
            })
        }
        _ => enter_context(
            facts,
            rich,
            checked,
            source,
            mode,
            owned,
            pending,
            cursors,
            events,
            replacement,
            storage_limit,
            protected,
        ),
    };
    result.map_err(projection)
}

#[allow(clippy::too_many_arguments)]
fn run<'w>(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    facts_inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'w>,
    mode: Mode,
    events: &Events,
    replacement: &mut Option<Budget<'w>>,
    storage_limit: usize,
    protected: &Cell<usize>,
) -> QueryResult<Observation> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget);
        // Common entry prepayment covers fixed harness construction/transfers;
        // every variable graph/cursor/source-rescan operation is charged below.
        budget.charge_work(4 * HEADERS)?;
        events.prepaid.set(events.prepaid.get() + 1);
        budget.reserve_storage(HEADERS)?;
        let mut owned = HEADERS;
        let mut pending = PendingNominalInitialGraphV1::new();
        let mut cursors = None;
        // Both payload owners outlive checked/rich/facts/context postflights.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |checked, budget| {
                    events.checked.set(events.checked.get() + 1);
                    assert!(checked.belongs_to(inventory));
                    assert!(std::ptr::eq(checked.emission().owner(), owner));
                    with_nominal_rich_source_preparation_v1(
                        owner,
                        inventory,
                        source.root(),
                        source.root(),
                        source.call_block(),
                        source.source_call(),
                        budget,
                        |rich, budget| {
                            events.rich.set(events.rich.get() + 1);
                            with_nominal_canonical_facts_observation_v1(
                                owner,
                                facts_inventory,
                                source.root(),
                                source.root(),
                                source.call_block(),
                                source.source_call(),
                                budget,
                                |facts| {
                                    inspect_facts(
                                        facts,
                                        rich,
                                        owner,
                                        checked,
                                        source,
                                        inventory,
                                        facts_inventory,
                                        mode,
                                        &mut owned,
                                        &mut pending,
                                        &mut cursors,
                                        events,
                                        before,
                                        replacement,
                                        storage_limit,
                                        protected,
                                    )
                                },
                            )
                        },
                    )
                },
            )
        }));
        let mut result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(QueryError::CallbackPanicked)
            }
        };
        // Dispose ALL graph/cursor/error/panic payloads before own-only refund.
        drop(pending);
        drop(cursors);
        before.check(budget, owned)?;
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            result = Err(Resource::Accounting.into());
        }
        if let Ok(observation) = &mut result {
            observation.owned = owned;
        }
        budget.release_storage(owned)?;
        result
    })
}
fn stages(events: &Events) -> (usize, usize, usize, usize, usize) {
    (
        events.checked.get(),
        events.rich.get(),
        events.facts.get(),
        events.context.get(),
        events.loan.get(),
    )
}
pub(in crate::production_ranked_projection_v1) fn observe_nominal_initial_graph_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> QueryResult<()> {
    let before = Custody::take(budget);
    let events = Events::default();
    let observed = run(
        owner,
        source,
        inventory,
        inventory,
        budget,
        Mode::Observe,
        &events,
        &mut None,
        0,
        &Cell::new(0),
    )?;
    before.check(budget, 0)?;
    assert_eq!(budget.storage(), before.floor);
    assert_eq!(stages(&events), (1, 1, 1, 1, 1));
    assert_eq!(events.prepaid.get(), 1);
    // ACTUAL counts, not expected constants or origin/ranked readiness.
    eprintln!(
        "fe2o3-initial-graph-observation-v1 root={} call_block={} permutation={:?} blocks={} locals={} edges={} aliases={} enum_edges={} stores={} loads={} borrowed={} calls={} owned_credits={} logged_edges={} truncated={} logical_work={} peak_before={} peak_after={}",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        observed.blocks,
        observed.locals,
        observed.edges,
        observed.aliases,
        observed.enum_edges,
        observed.stores,
        observed.loads,
        observed.borrowed,
        observed.calls,
        observed.owned,
        observed.edges.min(32),
        observed.edges > 32,
        budget
            .work()
            .checked_sub(before.work)
            .ok_or(Resource::Arithmetic)?,
        before.peak,
        budget.peak_storage()
    );
    Ok(())
}
fn with_headers(
    budget: &mut Budget<'_>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'_>) -> QueryResult<()>,
) -> QueryResult<()> {
    budget.reserve_storage(bytes)?;
    let before = Custody::take(budget);
    let outcome = catch_unwind(AssertUnwindSafe(|| body(budget)));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    before.check(budget, 0)?;
    budget.release_storage(bytes)?;
    result
}

pub(in crate::production_ranked_projection_v1) fn nominal_initial_graph_controls_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> QueryResult<()> {
    let before_controls = Custody::take(original);
    let original_runs = Cell::new(0usize);
    let original_prepaid = Cell::new(0usize);
    let negative_runs = Cell::new(0usize);
    let negative_prepaid = Cell::new(0usize);
    let result = owner.with_bf16_nominal_entry_resources_v1(inventory, original, |original| {
        with_headers(original, HEADERS, |original| {
            for mode in [Mode::Surplus, Mode::Error, Mode::Panic, Mode::Occupied] {
                original.charge_work(512)?;
                let before = Custody::take(original);
                let events = Events::default();
                original_runs.set(original_runs.get() + 1);
                let result = run(
                    owner,
                    source,
                    inventory,
                    inventory,
                    original,
                    mode,
                    &events,
                    &mut None,
                    0,
                    &Cell::new(0),
                );
                original_prepaid.set(original_prepaid.get() + events.prepaid.get());
                assert_eq!(stages(&events), (1, 1, 1, 1, 1));
                match mode {
                    Mode::Surplus => assert!(result.is_ok()),
                    Mode::Error => assert_eq!(result, Err(QueryError::Unavailable(REFUSAL))),
                    Mode::Panic => assert_eq!(result, Err(QueryError::CallbackPanicked)),
                    _ => assert_eq!(
                        result,
                        Err(QueryError::Unavailable(
                            "nominal initial graph pending owner is already occupied"
                        ))
                    ),
                }
                before.check(original, 0)?;
                let surplus = if mode == Mode::Occupied { 0 } else { 23 };
                assert_eq!(original.storage(), before.floor + surplus);
                assert_eq!(
                    (original.failed_work(), original.failed_storage()),
                    (None, None)
                );
                original.release_storage(surplus)?;
            }
            for mode in [
                Mode::ForeignFunction,
                Mode::MissingFunction,
                Mode::ForeignCorrespondence,
                Mode::Masked,
            ] {
                original.charge_work(512)?;
                let before = Custody::take(original);
                let events = Events::default();
                original_runs.set(original_runs.get() + 1);
                let result = run(
                    owner,
                    source,
                    inventory,
                    inventory,
                    original,
                    mode,
                    &events,
                    &mut None,
                    0,
                    &Cell::new(0),
                );
                original_prepaid.set(original_prepaid.get() + events.prepaid.get());
                assert_eq!(
                    stages(&events),
                    (1, 1, 1, usize::from(mode != Mode::Masked), 0)
                );
                let reason = if mode == Mode::Masked {
                    "nominal recipe resources require unmasked real facts"
                } else {
                    "nominal initial graph owner inventory or root differs"
                };
                assert_eq!(result, Err(QueryError::Unavailable(reason)));
                before.check(original, 0)?;
                assert_eq!(original.storage(), before.floor);
                assert_eq!(
                    (original.failed_work(), original.failed_storage()),
                    (None, None)
                );
            }
            // Real same-owner inventory, freshly derived on ORIGINAL. Different
            // inventory identity must refuse even when graph bytes are equal.
            let before_inventory = Custody::take(original);
            let (foreign_inventory, receipt) =
                CanonicalKirInventoryV1::derive(owner.executable(), original).map_err(|_| {
                    QueryError::Unavailable("genuine second inventory derivation refused")
                })?;
            assert_eq!(original.storage(), before_inventory.floor);
            let retained = receipt.retained_storage();
            if let Err(error) = original.reserve_storage(retained) {
                drop(foreign_inventory);
                return Err(error.into());
            }
            let inventory_result = catch_unwind(AssertUnwindSafe(|| {
                original.charge_work(512)?;
                let events = Events::default();
                original_runs.set(original_runs.get() + 1);
                let result = run(
                    owner,
                    source,
                    inventory,
                    &foreign_inventory,
                    original,
                    Mode::ForeignInventory,
                    &events,
                    &mut None,
                    0,
                    &Cell::new(0),
                );
                original_prepaid.set(original_prepaid.get() + events.prepaid.get());
                assert_eq!(stages(&events), (1, 1, 1, 1, 0));
                assert_eq!(
                    result,
                    Err(QueryError::Unavailable(
                        "nominal initial graph owner inventory or root differs"
                    ))
                );
                Ok::<_, QueryError>(())
            }));
            let inventory_result = match inventory_result {
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    Err(QueryError::CallbackPanicked)
                }
            };
            drop(foreign_inventory);
            before_inventory.check(original, retained)?;
            original.release_storage(retained)?;
            inventory_result?;
            assert_eq!(original.storage(), before_inventory.floor);
            assert_eq!(
                (original.failed_work(), original.failed_storage()),
                (None, None)
            );

            let occurrence = owner
                .semantic_ssa()
                .occurrence_storage()
                .ok_or(QueryError::Unavailable(
                    "genuine initial graph occurrence storage absent",
                ))?
                .retained_storage();
            let floor = add(
                add(owner.retained_analysis_storage_v1(), occurrence)?,
                inventory_storage,
            )?;
            let short = floor.checked_sub(1).ok_or(Resource::Arithmetic)?;
            // All independent accounts below are NEGATIVE ONLY, with their
            // finite complete work/scratch envelopes prepaid on ORIGINAL.
            with_headers(original, PROBE_SCRATCH, |original| {
                for which in 0..3 {
                    original.charge_work(PROBE_WORK + 512)?;
                    let mut work = Work::new(if which == 1 { 0 } else { PROBE_WORK });
                    let limit = if which == 2 {
                        floor
                    } else {
                        add(floor, PROBE_SCRATCH)?
                    };
                    let mut probe = Budget::new(&mut work, limit);
                    let incoming = if which == 0 { short } else { floor };
                    probe.reserve_storage(incoming)?;
                    let events = Events::default();
                    negative_runs.set(negative_runs.get() + 1);
                    let result = run(
                        owner,
                        source,
                        inventory,
                        inventory,
                        &mut probe,
                        Mode::Observe,
                        &events,
                        &mut None,
                        limit,
                        &Cell::new(0),
                    );
                    negative_prepaid.set(negative_prepaid.get() + events.prepaid.get());
                    assert_eq!(stages(&events), (0, 0, 0, 0, 0));
                    assert_eq!(probe.storage(), incoming);
                    match which {
                        0 => {
                            assert_eq!(result, Err(Resource::Accounting.into()));
                            assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
                        }
                        1 => {
                            assert!(matches!(
                                result,
                                Err(QueryError::Resource(Resource::Work(_)))
                            ));
                            assert!(probe.failed_work().is_some());
                        }
                        _ => {
                            assert!(matches!(
                                result,
                                Err(QueryError::Resource(Resource::Storage(_)))
                            ));
                            assert!(probe.failed_storage().is_some());
                        }
                    }
                }
                for mode in [
                    Mode::DenyWork,
                    Mode::DenyStorage,
                    Mode::Undercut,
                    Mode::ReplaceLedger,
                ] {
                    original.charge_work(PROBE_WORK + 512)?;
                    let mut work = Work::new(PROBE_WORK);
                    let mut foreign_work = Work::new(PROBE_WORK);
                    let limit = add(floor, PROBE_SCRATCH)?;
                    let mut probe = Budget::new(&mut work, limit);
                    probe.reserve_storage(floor)?;
                    let original_ledger = probe.work_ledger_identity_v1();
                    let mut replacement = Some(Budget::new(&mut foreign_work, limit));
                    let events = Events::default();
                    let protected = Cell::new(0);
                    negative_runs.set(negative_runs.get() + 1);
                    let result = run(
                        owner,
                        source,
                        inventory,
                        inventory,
                        &mut probe,
                        mode,
                        &events,
                        &mut replacement,
                        limit,
                        &protected,
                    );
                    negative_prepaid.set(negative_prepaid.get() + events.prepaid.get());
                    assert_eq!(
                        stages(&events),
                        (1, 1, 1, 1, 1),
                        "negative must reach the actual graph loan before refusing"
                    );
                    assert_eq!(result, Err(Resource::Accounting.into()));
                    if mode == Mode::ReplaceLedger {
                        assert!(probe.work_ledger_identity_v1() != original_ledger);
                        assert_eq!((probe.storage(), probe.work()), (0, 0));
                        assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
                    } else {
                        assert!(probe.work_ledger_identity_v1() == original_ledger);
                        if mode == Mode::Undercut {
                            assert!(probe.storage() >= floor && probe.storage() < protected.get());
                            assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
                        } else {
                            assert_eq!(probe.storage(), floor);
                            assert_eq!(probe.failed_work().is_some(), mode == Mode::DenyWork);
                            assert_eq!(probe.failed_storage().is_some(), mode == Mode::DenyStorage);
                        }
                    }
                }
                Ok(())
            })
        })
    });
    before_controls.check(original, 0)?;
    result?;
    assert_eq!(original.storage(), before_controls.floor);
    assert_eq!(
        (original.failed_work(), original.failed_storage()),
        (None, None)
    );
    eprintln!(
        "fe2o3-initial-graph-controls-v1 original_runs={} original_common_precharges={} negative_runs={} negative_common_precharges={} negative_prepaid_work={} logical_work={} peak_before={} peak_after={}",
        original_runs.get(),
        original_prepaid.get(),
        negative_runs.get(),
        negative_prepaid.get(),
        negative_runs
            .get()
            .checked_mul(PROBE_WORK + 512)
            .ok_or(Resource::Arithmetic)?,
        original
            .work()
            .checked_sub(before_controls.work)
            .ok_or(Resource::Arithmetic)?,
        before_controls.peak,
        original.peak_storage()
    );
    Ok(())
}

#[test]
fn genuine_initial_graph_header_covers_fixed_owners_and_probe_accounts() {
    assert!(
        4 * size_of::<Custody>()
            + 4 * size_of::<Events>()
            + 4 * size_of::<Observation>()
            + 4 * size_of::<PendingNominalInitialGraphV1>()
            + 4 * size_of::<Option<Vec<usize>>>()
            + 8 * size_of::<QueryResult<Observation>>()
            + 4 * size_of::<Budget<'static>>()
            + 4 * size_of::<Work>()
            + 4096
            <= HEADERS
    );
}

#[test]
fn complete_profile_oracle_rejects_missing_extra_reordered_and_late_payload_edges() {
    let first = CapabilityEdgeV1 {
        destination: 1,
        use_block: 0,
        kind: CapabilityEdgeKindV1::Alias,
    };
    let second = CapabilityEdgeV1 {
        destination: 2,
        use_block: 1,
        kind: CapabilityEdgeKindV1::Alias,
    };
    let rows = vec![vec![first, second], vec![], vec![]];
    assert!(require_expected_edge_at(&rows, 0, 0, first).is_ok());
    assert!(require_expected_edge_at(&rows, 0, 1, second).is_ok());
    assert!(require_exhausted_row(&rows[0], 2).is_ok());
    assert!(require_expected_edge_at(&rows, 1, 0, first).is_err());
    assert!(require_expected_edge_at(&rows, 0, 2, first).is_err());
    assert!(require_exhausted_row(&rows[0], 1).is_err());
    assert!(require_exhausted_row(&rows[0], 3).is_err());
    let reordered = vec![vec![second, first], vec![], vec![]];
    assert!(require_expected_edge_at(&reordered, 0, 0, first).is_err());
    let missing = vec![vec![first], vec![], vec![]];
    assert!(require_expected_edge_at(&missing, 0, 1, second).is_err());
    let mut late = first;
    late.kind = CapabilityEdgeKindV1::AuthenticatedOptionPayload;
    let replaced = vec![vec![late, second], vec![], vec![]];
    assert!(require_expected_edge_at(&replaced, 0, 0, first).is_err());
    let extra = vec![vec![first, second, late], vec![], vec![]];
    assert!(require_exhausted_row(&extra[0], 2).is_err());
}

// Reuse the independent original-source rescan, not production graph population,
// inside a later SAME graph loan. The caller owns paid cursor storage OUTER.
pub(in crate::production_ranked_projection_v1) fn check_source_rows_for_prefix_for_test_v1(
    view: &NominalCompleteForProfileGraphV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    cursors: &mut [usize],
) -> Result<()> {
    observe_view(view, rich, checked, context, cursors, false).map(|_| ())
}
