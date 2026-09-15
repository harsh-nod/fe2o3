//! Borrowed original Grid-to-SSA correspondence; no standalone source authority.
use super::*;
use crate::production::semantic_ssa::{
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
    ProductionSemanticSsaSourceQueryV1 as Query, ProductionSemanticSsaSourceSiteV1 as Site,
    ProductionSemanticSsaValueOriginV1 as Origin,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCapabilityMemoryAliasingV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDisjointIndexSpaceV1, SemanticStatementV1, SemanticTypeDeclV1,
};
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};

#[path = "index_body.rs"]
mod index_body;

pub struct ProductionGuardedGridSourceV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    query: Query<'a>,
    retained: &'a GuardedGridResultsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionGuardedGridActionKindV1 {
    Issuer,
    Some,
    Borrow,
}

/// Values are the actual retained SSA definitions/uses at a complete adapter
/// window. The private constructor always joins the Grid receipt and its view.
pub struct ProductionGuardedGridActionV1<'a> {
    query: Query<'a>,
    receipt: &'a ProductionGuardedGridLeaderResultV1,
    site: Site,
    kind: ProductionGuardedGridActionKindV1,
    source_local: SemanticLocalIdV1,
    source: SsaValueV1,
    created: Option<(SemanticLocalIdV1, SsaValueV1)>,
    output: (SemanticLocalIdV1, SsaValueV1),
}
impl ProductionGuardedGridActionV1<'_> {
    pub fn receipt(&self) -> &ProductionGuardedGridLeaderResultV1 {
        self.receipt
    }
    pub const fn site(&self) -> Site {
        self.site
    }
    pub const fn kind(&self) -> ProductionGuardedGridActionKindV1 {
        self.kind
    }
    pub const fn source(&self) -> (SemanticLocalIdV1, SsaValueV1) {
        (self.source_local, self.source)
    }
    pub const fn created(&self) -> Option<(SemanticLocalIdV1, SsaValueV1)> {
        self.created
    }
    pub const fn output(&self) -> (SemanticLocalIdV1, SsaValueV1) {
        self.output
    }
    pub fn belongs_to(&self, source: &ProductionGuardedGridSourceV1<'_>) -> bool {
        std::ptr::eq(self.query.function(), source.query.function())
            && std::ptr::eq(self.query.plan(), source.query.plan())
    }
}

/// Only the live owner pipeline may use this to construct an index witness.
/// Decoded MIR and a matching shape alone do not establish Rust source authority.
pub struct ProductionGuardedGridIndexV1<'a> {
    query: Query<'a>,
    receipt: &'a ProductionGuardedGridLeaderResultV1,
    returned_at: Site,
    returned_local: SemanticLocalIdV1,
    returned_value: SsaValueV1,
    consumed: (SemanticLocalIdV1, SsaValueV1),
    raw_site: Site,
    raw_operand: &'a SemanticOperandV1,
    raw_value: SsaValueV1,
    witness: SemanticTypeIdV1,
}
impl<'a> ProductionGuardedGridIndexV1<'a> {
    pub fn receipt(&self) -> &ProductionGuardedGridLeaderResultV1 {
        self.receipt
    }
    pub const fn return_site(&self) -> Site {
        self.returned_at
    }
    pub const fn returned(&self) -> (SemanticLocalIdV1, SsaValueV1) {
        (self.returned_local, self.returned_value)
    }
    pub const fn consumed(&self) -> (SemanticLocalIdV1, SsaValueV1) {
        self.consumed
    }
    pub const fn raw_site(&self) -> Site {
        self.raw_site
    }
    pub const fn raw_operand(&self) -> &'a SemanticOperandV1 {
        self.raw_operand
    }
    pub const fn raw_value(&self) -> SsaValueV1 {
        self.raw_value
    }
    pub const fn witness_type(&self) -> SemanticTypeIdV1 {
        self.witness
    }
    pub fn belongs_to(&self, source: &ProductionGuardedGridSourceV1<'_>) -> bool {
        std::ptr::eq(self.query.function(), source.query.function())
            && std::ptr::eq(self.query.plan(), source.query.plan())
    }
}

impl ProductionSemanticSsaOwnerV1 {
    /// Like SourceQuery, this borrows one existing owner and does not replay it
    /// or create a new allowance. Consumers retain their verify_replay/live
    /// source boundary before using these observations for lowering.
    pub fn guarded_grid_source_for_root<'a>(
        &'a self,
        root: SemanticFunctionIdV1,
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<ProductionGuardedGridSourceV1<'a>, QueryError> {
        let query = self.source_query_for_root(root, function)?;
        let view = self
            .execution_view_for_root(root)
            .ok_or(QueryError::WrongOwner)?;
        let retained = &query.plan().guarded_grid_results;
        if !retained.entries.is_empty() && retained.view != Some(*view.identity()) {
            return Err(QueryError::WrongOwner);
        }
        Ok(ProductionGuardedGridSourceV1 {
            owner: self,
            view,
            query,
            retained,
        })
    }
}

fn spend(charge: &mut impl FnMut(usize) -> bool, work: usize) -> Result<(), QueryError> {
    if charge(work) {
        Ok(())
    } else {
        Err(QueryError::WorkLimit)
    }
}
fn mismatch<T>() -> Result<T, QueryError> {
    Err(QueryError::UnsupportedOperand)
}
fn plain(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place)
        }
        _ => None,
    }
}
fn assignment_at(
    function: &SemanticFunctionDeclV1,
    site: Site,
) -> Result<&SemanticAssignmentV1, QueryError> {
    let statement = site.statement().ok_or(QueryError::InvalidSite)?;
    match function
        .blocks()
        .get(site.block().index() as usize)
        .and_then(|b| b.statements().get(statement as usize))
        .map(SemanticStatementV1::kind)
    {
        Some(SemanticStatementKindV1::Assign(a)) => Ok(a),
        _ => Err(QueryError::InvalidSite),
    }
}

impl<'a> ProductionGuardedGridSourceV1<'a> {
    pub const fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.query.function()
    }
    pub fn types(&self) -> &'a [SemanticTypeDeclV1] {
        self.owner.source_semantic().types()
    }

    pub fn event_count(&self) -> usize {
        self.retained.events.len()
    }
    pub fn event_site(&self, index: usize) -> Option<Site> {
        self.retained
            .events
            .get(index)
            .map(|e| Site::new(SemanticBlockIdV1::from_index(e.block), Some(e.statement)))
    }

    pub fn action_at(
        &self,
        site: Site,
        charge: &mut impl FnMut(usize) -> bool,
    ) -> Result<Option<ProductionGuardedGridActionV1<'a>>, QueryError> {
        let Some(statement) = site.statement() else {
            return Ok(None);
        };
        let key = (site.block().index(), statement);
        let (mut low, mut high) = (0, self.retained.events.len());
        while low < high {
            spend(charge, 1)?;
            let middle = low + (high - low) / 2;
            let e = &self.retained.events[middle];
            if (e.block, e.statement) < key {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let Some(event) = self
            .retained
            .events
            .get(low)
            .filter(|e| (e.block, e.statement) == key)
        else {
            return Ok(None);
        };
        let origin = self
            .view
            .block_origins()
            .get(site.block().index() as usize)
            .ok_or(QueryError::InvalidSite)?;
        let getter = match event.kind {
            Kind::Issuer => self
                .view
                .instances()
                .get(origin.instance().index() as usize)
                .and_then(|i| i.parent()),
            Kind::Some => Some(origin.instance()),
            Kind::Borrow => None,
        };
        spend(charge, self.retained.entries.len())?;
        let mut matches = self
            .retained
            .entries
            .iter()
            .filter(|entry| match event.kind {
                Kind::Borrow => entry.option == event.source,
                _ => Some(entry.getter) == getter,
            });
        let receipt = matches.next().ok_or(QueryError::MissingDefinition)?;
        if matches.next().is_some() || receipt.contract.provenance().root() != self.query.root() {
            return mismatch();
        }
        let assignment = assignment_at(self.query.function(), site)?;
        let output_local = assignment.destination().local();
        if !assignment.destination().projections().is_empty() {
            return mismatch();
        }
        let events = self.query.resolved_events_at(site, &mut || charge(1))?;
        spend(charge, events.len())?;
        let (source, created, output, kind) = match (event.kind, events) {
            (
                Kind::Issuer,
                [
                    (
                        _,
                        SsaResolvedEventV1::Use {
                            variable: s,
                            value: source,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Define {
                            variable: c,
                            value: created,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Use {
                            variable: u,
                            value: used,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Kill {
                            variable: k,
                            previous: Some(killed),
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Define {
                            variable: d,
                            value: output,
                        },
                    ),
                ],
            ) if s.get() == event.source.index()
                && c.get() == event.destination.index()
                && c == u
                && u == k
                && created == used
                && used == killed
                && d.get() == output_local.index()
                && matches!(assignment.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                        if p.local()==event.destination && p.projections().is_empty()
                            && p.ty()==receipt.contract.types().leader)
                && assignment.value().result_type() == receipt.contract.types().leader =>
            {
                (
                    *source,
                    Some((event.destination, *created)),
                    *output,
                    ProductionGuardedGridActionKindV1::Issuer,
                )
            }
            (
                Kind::Some,
                [
                    (
                        _,
                        SsaResolvedEventV1::Use {
                            variable: s,
                            value: source,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Kill {
                            variable: k,
                            previous: Some(killed),
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Define {
                            variable: d,
                            value: output,
                        },
                    ),
                ],
            ) if s.get() == event.source.index()
                && s == k
                && source == killed
                && d.get() == output_local.index()
                && output_local == event.destination
                && assignment.value().result_type() == receipt.contract.types().leader_option
                && matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(a)
                    if *a.kind() == SemanticAggregateKindV1::EnumVariant(1)
                        && matches!(a.operands(), [SemanticOperandV1::Constant(c)]
                            if c.ty() == receipt.contract.types().leader
                                && matches!(c.value(), SemanticConstantValueV1::ZeroSized))) =>
            {
                (
                    *source,
                    None,
                    *output,
                    ProductionGuardedGridActionKindV1::Some,
                )
            }
            (
                Kind::Borrow,
                [
                    (
                        _,
                        SsaResolvedEventV1::Use {
                            variable: s,
                            value: source,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Define {
                            variable: c,
                            value: created,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Use {
                            variable: u,
                            value: used,
                        },
                    ),
                    (
                        _,
                        SsaResolvedEventV1::Define {
                            variable: d,
                            value: output,
                        },
                    ),
                ],
            ) if s.get() == event.source.index()
                && c.get() == event.destination.index()
                && c == u
                && created == used
                && d.get() == output_local.index()
                && matches!(assignment.value().kind(),SemanticRvalueKindV1::Borrow {kind:SemanticBorrowKindV1::Shared,place}
                        if place.local()==event.destination && place.projections().is_empty()
                            && place.ty()==receipt.contract.types().leader) =>
            {
                (
                    *source,
                    Some((event.destination, *created)),
                    *output,
                    ProductionGuardedGridActionKindV1::Borrow,
                )
            }
            _ => return mismatch(),
        };
        Ok(Some(ProductionGuardedGridActionV1 {
            query: self.query,
            receipt,
            site,
            kind,
            source_local: event.source,
            source,
            created,
            output: (output_local, output),
        }))
    }

    fn parameter(
        &self,
        callee: SemanticCallInstanceIdV1,
        argument: u32,
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
        charge: &mut impl FnMut(usize) -> bool,
    ) -> Result<(Site, &'a SemanticOperandV1), QueryError> {
        let instance = self
            .view
            .instances()
            .get(callee.index() as usize)
            .ok_or(QueryError::InvalidSite)?;
        let expected_local = instance
            .local_start()
            .checked_add(local.index())
            .ok_or(QueryError::InvalidSite)?;
        let mut found = None;
        // One bounded original-origin roster scan, not guessed call-entry or
        // definition ordinals. The caller's same work allowance pays every row.
        for (block, origin) in self.view.block_origins().iter().enumerate() {
            spend(charge, 1)?;
            for (statement, marker) in origin.statements().iter().enumerate() {
                spend(charge, 1)?;
                if *marker
                    != (SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument })
                {
                    continue;
                }
                let site = Site::new(
                    SemanticBlockIdV1::from_index(
                        u32::try_from(block).map_err(|_| QueryError::InvalidSite)?,
                    ),
                    Some(u32::try_from(statement).map_err(|_| QueryError::InvalidSite)?),
                );
                let a = assignment_at(self.query.function(), site)?;
                let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
                    return mismatch();
                };
                if a.destination().local().index() != expected_local
                    || a.destination().ty() != ty
                    || !a.destination().projections().is_empty()
                    || a.value().result_type() != ty
                    || operand.ty() != ty
                    || found.replace((site, operand)).is_some()
                {
                    return mismatch();
                }
            }
        }
        found.ok_or(QueryError::MissingDefinition)
    }

    pub fn index_for_store(
        &self,
        block: SemanticBlockIdV1,
        charge: &mut impl FnMut(usize) -> bool,
    ) -> Result<Option<ProductionGuardedGridIndexV1<'a>>, QueryError> {
        spend(charge, 1)?;
        let semantic = self.owner.source_semantic();
        let body = self.query.function();
        let Some(SemanticTerminatorKindV1::Call(call)) = body
            .blocks()
            .get(block.index() as usize)
            .map(|b| b.terminator().kind())
        else {
            return Ok(None);
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore {
                    witness,
                    contract,
                    provenance,
                    ..
                },
            ..
        }) = semantic.callables().get(call.callee().index() as usize)
        else {
            return Ok(None);
        };
        if contract.aliasing()
            != SemanticCapabilityMemoryAliasingV1::Disjoint(
                SemanticDisjointIndexSpaceV1::GridExclusive,
            )
        {
            return Ok(None);
        }
        let [_, operand, _] = call.arguments() else {
            return mismatch();
        };
        if provenance.root() != self.query.root()
            || operand.ty() != *witness
            || !matches!(operand,SemanticOperandV1::Move(p) if p.projections().is_empty())
        {
            return mismatch();
        }
        let mut site = Site::new(block, None);
        let mut operand = operand;
        let original_use = self.query.operand_use(site, operand, &mut || charge(1))?;
        let consumed = (
            SemanticLocalIdV1::from_index(original_use.variable().get()),
            original_use.value(),
        );
        loop {
            spend(charge, 1)?;
            let place = plain(operand).ok_or(QueryError::UnsupportedOperand)?;
            let value = self.query.operand_use(site, operand, &mut || charge(1))?;
            let Origin::Event {
                site: definition, ..
            } = self
                .query
                .value_origin(&value.retained_value(), &mut || charge(1))?
            else {
                return mismatch();
            };
            let a = assignment_at(body, definition)?;
            if a.destination().local() != place.local()
                || a.destination().ty() != *witness
                || !a.destination().projections().is_empty()
                || a.value().result_type() != *witness
            {
                return mismatch();
            }
            let origin = self
                .view
                .block_origins()
                .get(definition.block().index() as usize)
                .ok_or(QueryError::InvalidSite)?;
            let marker = origin
                .statements()
                .get(definition.statement().ok_or(QueryError::InvalidSite)? as usize)
                .ok_or(QueryError::InvalidSite)?;
            if let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = *marker {
                let instance = self
                    .view
                    .instances()
                    .get(callee.index() as usize)
                    .ok_or(QueryError::InvalidSite)?;
                let f = semantic
                    .functions()
                    .get(instance.function().index() as usize)
                    .ok_or(QueryError::InvalidSite)?;
                if origin.instance() != callee || f.identity() != instance.function_identity() {
                    return mismatch();
                }
                if let [reference, raw] = f.abi().source_input_types() {
                    let Some(SemanticTypeShapeV1::Pointer(pointer)) = semantic
                        .types()
                        .get(reference.index() as usize)
                        .map(SemanticTypeDeclV1::shape)
                    else {
                        return mismatch();
                    };
                    let types = index_body::IndexTypes {
                        leader: pointer.pointee(),
                        reference: *reference,
                        raw: *raw,
                        witness: *witness,
                    };
                    if let Some(recipe) = index_body::observe(
                        f,
                        semantic.functions(),
                        semantic.callables(),
                        semantic.types(),
                        types,
                        charge,
                    )
                    .map_err(|()| QueryError::WorkLimit)?
                    {
                        let expected_return = instance
                            .local_start()
                            .checked_add(recipe.returned.index())
                            .ok_or(QueryError::InvalidSite)?;
                        if !matches!(a.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                            if p.local().index()==expected_return && p.ty()==*witness && p.projections().is_empty())
                        {
                            return mismatch();
                        }
                        let (reference_site, reference_operand) =
                            self.parameter(callee, 0, recipe.receiver, *reference, charge)?;
                        let receipt =
                            self.leader_reference(reference_site, reference_operand, charge)?;
                        if receipt.contract.types().leader != types.leader
                            || receipt.contract.provenance() != *provenance
                        {
                            return mismatch();
                        }
                        // The constructor parameter transfer is the exact scalar
                        // use retained from index's original Copy(raw) call.
                        spend(charge, self.view.instances().len())?;
                        let mut constructor = None;
                        for (i, child) in self.view.instances().iter().enumerate() {
                            if child.parent() != Some(callee) {
                                continue;
                            }
                            if child.function() != recipe.constructor
                                || child.call_block() != Some(f.entry())
                                || constructor.replace(i).is_some()
                            {
                                return mismatch();
                            }
                        }
                        let constructor = constructor.ok_or(QueryError::MissingDefinition)?;
                        // Obtain the opaque instance ID from its retained origin.
                        let child = &self.view.instances()[constructor];
                        let child_origin = self
                            .view
                            .block_origins()
                            .get(child.block_start() as usize)
                            .ok_or(QueryError::InvalidSite)?;
                        if child_origin.instance().index() as usize != constructor {
                            return mismatch();
                        }
                        let (raw_site, raw_operand) = self.parameter(
                            child_origin.instance(),
                            0,
                            recipe.constructor_raw,
                            *raw,
                            charge,
                        )?;
                        let expected_raw = instance
                            .local_start()
                            .checked_add(recipe.raw.index())
                            .ok_or(QueryError::InvalidSite)?;
                        if !matches!(raw_operand,SemanticOperandV1::Copy(p)
                            if p.local().index()==expected_raw && p.ty()==*raw && p.projections().is_empty())
                        {
                            return mismatch();
                        }
                        let raw_value = self
                            .query
                            .operand_use(raw_site, raw_operand, &mut || charge(1))?
                            .value();
                        return Ok(Some(ProductionGuardedGridIndexV1 {
                            query: self.query,
                            receipt,
                            returned_at: definition,
                            returned_local: a.destination().local(),
                            returned_value: value.value(),
                            consumed,
                            raw_site,
                            raw_operand,
                            raw_value,
                            witness: *witness,
                        }));
                    }
                }
            }
            // Closed representation-preserving witness transport only. In
            // particular Copy, projected payloads, arbitrary aggregates and
            // phi selections cannot manufacture a new consuming witness.
            let SemanticRvalueKindV1::Use(next @ SemanticOperandV1::Move(_)) = a.value().kind()
            else {
                return mismatch();
            };
            if plain(next).is_none_or(|p| p.ty() != *witness) {
                return mismatch();
            }
            site = definition;
            operand = next;
        }
    }

    /// Follow exact immutable reference definitions, never a same-typed local.
    /// Each step uses SourceQuery's retained definition and original operand.
    fn leader_reference(
        &self,
        mut site: Site,
        mut operand: &'a SemanticOperandV1,
        charge: &mut impl FnMut(usize) -> bool,
    ) -> Result<&'a ProductionGuardedGridLeaderResultV1, QueryError> {
        loop {
            spend(charge, 1)?;
            let place = plain(operand).ok_or(QueryError::UnsupportedOperand)?;
            let value = self.query.operand_use(site, operand, &mut || charge(1))?;
            let Origin::Event {
                site: definition, ..
            } = self
                .query
                .value_origin(&value.retained_value(), &mut || charge(1))?
            else {
                return mismatch();
            };
            let assignment = assignment_at(self.query.function(), definition)?;
            if assignment.destination().local() != place.local()
                || assignment.destination().ty() != place.ty()
                || !assignment.destination().projections().is_empty()
                || assignment.value().result_type() != place.ty()
            {
                return mismatch();
            }
            if let Some(action) = self.action_at(definition, charge)? {
                if action.kind != ProductionGuardedGridActionKindV1::Borrow
                    || action.output != (place.local(), value.value())
                {
                    return mismatch();
                }
                return Ok(action.receipt);
            }
            let SemanticRvalueKindV1::Use(next) = assignment.value().kind() else {
                return mismatch();
            };
            if plain(next).is_none_or(|p| p.ty() != place.ty()) {
                return mismatch();
            }
            site = definition;
            operand = next;
        }
    }
}
