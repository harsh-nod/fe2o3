//! Capture-only counting, fixed-capacity filling and ledger actions.

use super::super::adapter::{emission_v1 as grammar, prepared_v1 as prepared};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1;

pub(super) struct Meter<'b, 'w> {
    budget: &'b mut Budget<'w>,
    retained: usize,
}

impl Meter<'_, '_> {
    pub(super) fn work(&mut self, units: usize) -> CaptureResult<()> {
        self.budget.charge_work(units).map_err(Into::into)
    }

    pub(super) fn require(
        &mut self,
        units: usize,
        function: SemanticFunctionIdV1,
        block: Option<SsaBlockIdV1>,
        condition: impl FnOnce() -> bool,
    ) -> CaptureResult<()> {
        self.work(units)?;
        if condition() {
            Ok(())
        } else {
            Err(mismatch(function, block))
        }
    }

    fn reserve(&mut self, bytes: usize) -> CaptureResult<()> {
        let retained = self
            .retained
            .checked_add(bytes)
            .ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(bytes)?;
        self.retained = retained;
        Ok(())
    }

    fn array<T>(&mut self, count: usize) -> CaptureResult<Vec<T>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        self.work(1)?;
        let bytes = std::mem::size_of::<T>()
            .checked_mul(count)
            .ok_or(Resource::Arithmetic)?;
        if bytes == 0 {
            return Err(Resource::Accounting.into());
        }
        self.reserve(bytes)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        // Pinned nightly-2026-04-03 RawVec::grow_exact stores len+additional
        // as capacity; Global returns the requested layout length. These fresh
        // non-ZST vectors never grow, shrink or convert to boxes. Allocator
        // overhead is excluded. A different capacity is an UNQUALIFIED failure,
        // not a retrospective storage top-up. Re-audit on toolchain changes.
        if rows.capacity() != count {
            return Err(Resource::Accounting.into());
        }
        Ok(rows)
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct Counts {
    blocks: usize,
    events: usize,
    constants: usize,
    successors: usize,
    edge_definitions: usize,
    entries: usize,
    elisions: usize,
}

impl FunctionRows {
    fn allocate(
        function: SemanticFunctionIdV1,
        count: Counts,
        meter: &mut Meter<'_, '_>,
    ) -> CaptureResult<Self> {
        Ok(Self {
            function,
            blocks: meter.array(count.blocks)?,
            events: meter.array(count.events)?,
            constants: meter.array(count.constants)?,
            successors: meter.array(count.successors)?,
            edge_definitions: meter.array(count.edge_definitions)?,
            entries: Vec::new(),
            elisions: meter.array(count.elisions)?,
        })
    }
}

pub(super) struct CaptureDriver<'b, 'w> {
    meter: Meter<'b, 'w>,
    expected_functions: usize,
    functions: Vec<FunctionRows>,
    current: Option<FunctionRows>,
}

impl<'b, 'w> CaptureDriver<'b, 'w> {
    pub(super) fn new(budget: &'b mut Budget<'w>) -> Self {
        Self {
            meter: Meter {
                budget,
                retained: 0,
            },
            expected_functions: 0,
            functions: Vec::new(),
            current: None,
        }
    }

    pub(super) fn finish(mut self) -> CaptureResult<Attachment> {
        // Two shape checks here and the owner's final receipt/floor comparison.
        self.meter.work(3)?;
        if self.current.is_some() || self.functions.len() != self.expected_functions {
            return Err(Resource::Accounting.into());
        }
        Ok(Attachment {
            functions: self.functions,
            storage: ProductionSemanticSsaOccurrenceStorageV1 {
                retained_storage: self.meter.retained,
            },
        })
    }
}

impl ReplayDriver for CaptureDriver<'_, '_> {
    type Error = CaptureError;

    fn start(&mut self, functions: usize) -> CaptureResult<()> {
        self.meter.work(1)?;
        self.meter
            .reserve(std::mem::size_of::<Option<Attachment>>())?;
        self.functions = self.meter.array(functions)?;
        self.expected_functions = functions;
        Ok(())
    }

    fn input(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: Option<&[SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    ) -> CaptureResult<(SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize)> {
        self.meter.work(2)?;
        if self.current.is_some() || self.functions.len() >= self.expected_functions {
            return Err(Resource::Accounting.into());
        }
        let id = SemanticFunctionIdV1::from_index(checked_u32(self.functions.len())?);
        let mut count = Observer::new(&mut self.meter, id, None);
        let prepared = prepared::prepare_semantic_ssa_adapter_with_observer_v1(
            function,
            types,
            callables,
            transparent_borrows,
            &mut count,
        )?;
        prepared
            .emit_blocks(&mut CountBlocks::default(), &mut count)
            .map_err(flatten)?;
        let block_counts = count.finish()?;
        let mut rows = FunctionRows::allocate(id, block_counts, &mut self.meter)?;
        let entries = {
            let mut fill = Observer::new(&mut self.meter, id, Some(&mut rows));
            let entries = prepared.into_entries(&mut fill)?;
            let actual = fill.finish()?;
            self.meter.require(7, id, None, || actual == block_counts)?;
            entries
        };
        let mut count = Observer::new(&mut self.meter, id, None);
        entries
            .emit_entries(&mut CountEntries::default(), &mut count)
            .map_err(flatten)?;
        let entry_counts = count.finish()?;
        rows.entries = self.meter.array(entry_counts.entries)?;
        let input = {
            let mut fill = Observer::new(&mut self.meter, id, Some(&mut rows));
            let input = entries.finish(&mut fill)?;
            let actual = fill.finish()?;
            self.meter.require(7, id, None, || actual == entry_counts)?;
            input
        };
        self.current = Some(rows);
        Ok(input)
    }

    fn join(
        &mut self,
        input: &SsaConstructionInputV1,
        plan: &ProductionSemanticSsaFunctionPlanV1,
    ) -> CaptureResult<()> {
        self.meter.work(1)?;
        let mut rows = self.current.take().ok_or(Resource::Accounting)?;
        self.meter
            .require(1, rows.function, None, || rows.function == plan.function())?;
        join::join(&mut rows, input, plan.plan(), &mut self.meter)?;
        self.meter.work(1)?;
        append(&mut self.functions, rows)?;
        Ok(())
    }
}

fn append<T>(rows: &mut Vec<T>, row: T) -> CaptureResult<()> {
    // The caller prepays one row action, including this fixed cursor guard.
    if rows.len() >= rows.capacity() {
        return Err(Resource::Accounting.into());
    }
    rows.push(row);
    Ok(())
}

fn advance(value: &mut usize) -> CaptureResult<()> {
    *value = value.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}

fn flatten(error: grammar::SemanticSsaEmissionErrorV1<CaptureError, Resource>) -> CaptureError {
    match error {
        grammar::SemanticSsaEmissionErrorV1::Observer(error) => error,
        grammar::SemanticSsaEmissionErrorV1::Output(error) => error.into(),
    }
}

#[derive(Default)]
struct CountEvents(usize);

impl grammar::SemanticSsaEventBufferV1 for CountEvents {
    type Error = Resource;
    fn event_count(&self) -> usize {
        self.0
    }
    fn push_event(&mut self, _: SsaEventV1) -> Result<(), Resource> {
        self.0 = self.0.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    }
}

#[derive(Default)]
struct CountBlocks {
    successors: usize,
}

impl prepared::SemanticSsaBlockOutputV1 for CountBlocks {
    type Error = Resource;
    type Events = CountEvents;
    fn begin_block(&mut self) -> Result<CountEvents, Resource> {
        Ok(CountEvents::default())
    }
    fn begin_successors(&mut self, _: usize) -> Result<(), Resource> {
        self.successors = 0;
        Ok(())
    }
    fn successor_count(&self) -> usize {
        self.successors
    }
    fn push_successor(
        &mut self,
        _: SsaEdgeRoleV1,
        _: SsaBlockIdV1,
        _: Option<SsaVariableIdV1>,
    ) -> Result<(), Resource> {
        self.successors = self.successors.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    }
    fn finish_block(&mut self, _: CountEvents) -> Result<(), Resource> {
        Ok(())
    }
}

#[derive(Default)]
struct CountEntries(usize);

impl prepared::SemanticSsaEntryOutputV1 for CountEntries {
    type Error = Resource;
    fn entry_count(&self) -> usize {
        self.0
    }
    fn push_entry(&mut self, _: SsaVariableIdV1) -> Result<(), Resource> {
        self.0 = self.0.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    }
}

struct Observer<'m, 'b, 'w, 'r> {
    meter: &'m mut Meter<'b, 'w>,
    function: SemanticFunctionIdV1,
    rows: Option<&'r mut FunctionRows>,
    counts: Counts,
    block_events: usize,
    block_successors: usize,
    expected_elisions: Option<usize>,
}

impl<'m, 'b, 'w, 'r> Observer<'m, 'b, 'w, 'r> {
    fn new(
        meter: &'m mut Meter<'b, 'w>,
        function: SemanticFunctionIdV1,
        rows: Option<&'r mut FunctionRows>,
    ) -> Self {
        Self {
            meter,
            function,
            rows,
            counts: Counts::default(),
            block_events: 0,
            block_successors: 0,
            expected_elisions: None,
        }
    }

    fn finish(self) -> CaptureResult<Counts> {
        self.meter.require(1, self.function, None, || {
            self.expected_elisions
                .is_none_or(|expected| expected == self.counts.elisions)
        })?;
        Ok(self.counts)
    }
}

impl grammar::SemanticSsaEmissionObserverV1 for Observer<'_, '_, '_, '_> {
    type Error = CaptureError;

    fn block_pass_begin(&mut self, elisions: usize) -> CaptureResult<()> {
        let work = elisions
            .checked_mul(4)
            .and_then(|n| n.checked_add(3))
            .ok_or(Resource::Arithmetic)?;
        self.meter.work(work)?;
        self.expected_elisions = Some(elisions);
        Ok(())
    }

    fn entry_pass_begin(&mut self, _: usize, implicit: usize) -> CaptureResult<()> {
        let work = implicit
            .checked_mul(2)
            .and_then(|n| n.checked_add(3))
            .ok_or(Resource::Arithmetic)?;
        self.meter.work(work)
    }

    fn visit(
        &mut self,
        kind: grammar::SemanticSsaVisitV1,
        _: grammar::SemanticSsaEmissionSiteV1,
    ) -> CaptureResult<()> {
        self.meter.work(1)?;
        if matches!(kind, grammar::SemanticSsaVisitV1::EntryCandidate) {
            self.meter.work(1)?;
        }
        Ok(())
    }

    fn statement_elision_lookup(
        &mut self,
        _: grammar::SemanticSsaEmissionSiteV1,
        _: usize,
    ) -> CaptureResult<()> {
        self.meter.work(2)
    }

    fn event(
        &mut self,
        site: grammar::SemanticSsaEmissionSiteV1,
        operand: grammar::SemanticSsaOperandRoleV1,
        role: grammar::SemanticSsaEventRoleV1,
        ordinal: usize,
        event: SsaEventV1,
    ) -> CaptureResult<()> {
        self.meter.require(1, self.function, None, || {
            self.counts.events.checked_sub(self.block_events) == Some(ordinal)
        })?;
        self.meter.work(1)?;
        advance(&mut self.counts.events)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            append(
                &mut rows.events,
                ProductionSemanticSsaEventOccurrenceV1 {
                    site: source_site(site)?,
                    operand: operand_role(operand)?,
                    role: event_role(role)?,
                    ordinal: checked_u32(ordinal)?,
                    event,
                    reachable: false,
                    promoted: false,
                    resolved: None,
                },
            )?;
        }
        Ok(())
    }

    fn constant(
        &mut self,
        site: grammar::SemanticSsaEmissionSiteV1,
        operand: grammar::SemanticSsaOperandRoleV1,
        next_event: usize,
        constant: &SemanticConstantV1,
    ) -> CaptureResult<()> {
        self.meter.require(1, self.function, None, || {
            self.counts.events.checked_sub(self.block_events) == Some(next_event)
        })?;
        self.meter.work(1)?;
        advance(&mut self.counts.constants)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            append(
                &mut rows.constants,
                ProductionSemanticSsaConstantOccurrenceV1 {
                    site: source_site(site)?,
                    operand: operand_role(operand)?,
                    next_event: checked_u32(next_event)?,
                    ty: constant.ty(),
                },
            )?;
        }
        Ok(())
    }

    fn successor(
        &mut self,
        block: usize,
        ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
    ) -> CaptureResult<()> {
        self.meter.require(2, self.function, None, || {
            block == self.counts.blocks
                && self.counts.successors.checked_sub(self.block_successors) == Some(ordinal)
        })?;
        self.meter.work(1)?;
        advance(&mut self.counts.successors)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            append(
                &mut rows.successors,
                ProductionSemanticSsaSuccessorOccurrenceV1 {
                    id: SsaEdgeIdV1::new(
                        SsaBlockIdV1::new(checked_u32(block)?),
                        checked_u32(ordinal)?,
                    ),
                    edge,
                    definitions: self.counts.edge_definitions..self.counts.edge_definitions,
                },
            )?;
        }
        Ok(())
    }

    fn edge_definition(
        &mut self,
        block: usize,
        edge_ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
        definition_ordinal: usize,
        variable: SsaVariableIdV1,
    ) -> CaptureResult<()> {
        self.meter.work(1)?;
        advance(&mut self.counts.edge_definitions)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            let id = SsaEdgeIdV1::new(
                SsaBlockIdV1::new(checked_u32(block)?),
                checked_u32(edge_ordinal)?,
            );
            let successor = rows.successors.last_mut().ok_or(Resource::Accounting)?;
            self.meter
                .require(5, self.function, Some(id.source()), || {
                    successor.id == id
                        && successor.edge == edge
                        && successor.definitions.len() == definition_ordinal
                })?;
            successor.definitions.end = self.counts.edge_definitions;
            append(
                &mut rows.edge_definitions,
                ProductionSemanticSsaEdgeDefinitionOccurrenceV1 {
                    edge: id,
                    ordinal: checked_u32(definition_ordinal)?,
                    variable,
                    reachable: false,
                    promoted: false,
                    value: None,
                },
            )?;
        }
        Ok(())
    }

    fn entry_definition(
        &mut self,
        ordinal: usize,
        variable: SsaVariableIdV1,
        origin: grammar::SemanticSsaEntryOriginV1,
    ) -> CaptureResult<()> {
        self.meter
            .require(1, self.function, None, || self.counts.entries == ordinal)?;
        self.meter.work(1)?;
        advance(&mut self.counts.entries)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            let origin = match origin {
                grammar::SemanticSsaEntryOriginV1::Argument(n) => {
                    ProductionSemanticSsaEntryOriginV1::Argument(n)
                }
                grammar::SemanticSsaEntryOriginV1::ImplicitCapability => {
                    ProductionSemanticSsaEntryOriginV1::ImplicitCapability
                }
            };
            append(
                &mut rows.entries,
                ProductionSemanticSsaEntryDefinitionOccurrenceV1 {
                    ordinal: checked_u32(ordinal)?,
                    variable,
                    origin,
                    value: None,
                },
            )?;
        }
        Ok(())
    }

    fn elided_borrow(&mut self, site: grammar::SemanticSsaEmissionSiteV1) -> CaptureResult<()> {
        self.meter.work(1)?;
        advance(&mut self.counts.elisions)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            append(&mut rows.elisions, source_site(site)?)?;
        }
        Ok(())
    }

    fn block_complete(
        &mut self,
        block: usize,
        events: usize,
        successors: usize,
    ) -> CaptureResult<()> {
        // Also prepay the prepared driver's later per-block elision tail check.
        self.meter.require(4, self.function, None, || {
            block == self.counts.blocks
                && self.block_events.checked_add(events) == Some(self.counts.events)
                && self.block_successors.checked_add(successors) == Some(self.counts.successors)
        })?;
        self.meter.work(1)?;
        advance(&mut self.counts.blocks)?;
        if let Some(rows) = self.rows.as_deref_mut() {
            append(
                &mut rows.blocks,
                BlockRows {
                    block: SsaBlockIdV1::new(checked_u32(block)?),
                    events: self.block_events..self.counts.events,
                    successors: self.block_successors..self.counts.successors,
                },
            )?;
        }
        self.block_events = self.counts.events;
        self.block_successors = self.counts.successors;
        Ok(())
    }

    fn input_complete(&mut self, blocks: usize, entries: usize) -> CaptureResult<()> {
        self.meter
            .require(1, self.function, None, || entries == self.counts.entries)?;
        if let Some(rows) = self.rows.as_deref() {
            self.meter
                .require(1, self.function, None, || blocks == rows.blocks.len())?;
        }
        Ok(())
    }
}

fn source_site(
    site: grammar::SemanticSsaEmissionSiteV1,
) -> CaptureResult<ProductionSemanticSsaOccurrenceSiteV1> {
    Ok(match site {
        grammar::SemanticSsaEmissionSiteV1::Statement { block, statement } => {
            ProductionSemanticSsaOccurrenceSiteV1::Statement {
                block: SsaBlockIdV1::new(checked_u32(block)?),
                statement: checked_u32(statement)?,
            }
        }
        grammar::SemanticSsaEmissionSiteV1::Terminator { block } => {
            ProductionSemanticSsaOccurrenceSiteV1::Terminator {
                block: SsaBlockIdV1::new(checked_u32(block)?),
            }
        }
        _ => return Err(Resource::Accounting.into()),
    })
}

fn operand_role(
    role: grammar::SemanticSsaOperandRoleV1,
) -> CaptureResult<ProductionSemanticSsaOperandRoleV1> {
    use ProductionSemanticSsaOperandRoleV1 as P;
    use grammar::SemanticSsaOperandRoleV1 as G;
    Ok(match role {
        G::RvalueOperand(n) => P::RvalueOperand(checked_u32(n)?),
        G::RvaluePlace => P::RvaluePlace,
        G::Destination => P::Destination,
        G::StoreValue => P::StoreValue,
        G::StoreDestination => P::StoreDestination,
        G::AtomicAddress => P::AtomicAddress,
        G::AtomicValue => P::AtomicValue,
        G::AtomicExpected => P::AtomicExpected,
        G::AtomicReplacement => P::AtomicReplacement,
        G::AtomicDestination => P::AtomicDestination,
        G::StatementPlace => P::StatementPlace,
        G::Assume => P::Assume,
        G::StorageLive => P::StorageLive,
        G::StorageDead => P::StorageDead,
        G::CallArgument(n) => P::CallArgument(checked_u32(n)?),
        G::CallDestinationAddress => P::CallDestinationAddress,
        G::TailCallArgument(n) => P::TailCallArgument(checked_u32(n)?),
        G::SwitchDiscriminant => P::SwitchDiscriminant,
        G::DropPlace => P::DropPlace,
        G::AssertCondition => P::AssertCondition,
        G::AssertMessage(n) => P::AssertMessage(checked_u32(n)?),
        G::ReturnValue => P::ReturnValue,
        G::ElidedBorrowDestination => P::ElidedBorrowDestination,
    })
}

fn event_role(
    role: grammar::SemanticSsaEventRoleV1,
) -> CaptureResult<ProductionSemanticSsaEventRoleV1> {
    use ProductionSemanticSsaEventRoleV1 as P;
    use grammar::SemanticSsaEventRoleV1 as G;
    Ok(match role {
        G::BaseUse => P::BaseUse,
        G::ProjectionIndexUse(n) => P::ProjectionIndexUse(checked_u32(n)?),
        G::MoveKill => P::MoveKill,
        G::DestinationDefine => P::DestinationDefine,
        G::StorageKill => P::StorageKill,
    })
}
