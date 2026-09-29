//! Address-observation analysis for closed, same-block shared primitive loans.
//! This supplies storage candidates only; source events and lifetime/SSA replay
//! remain mandatory. It does not authorize raw pointers or cross-CFG loans.
use super::super::nominal_reference_effects_v29::{BorrowWork, Meter};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssignmentV1, SemanticBorrowKindV1, SemanticMutabilityV1,
    SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticProjectionV1, SemanticRvalueV1,
};

type Error = ProductionSemanticSsaErrorV1;
type Result<T> = std::result::Result<T, Error>;

#[path = "adapter_shared_primitive_liveness_v1.rs"]
mod liveness;

pub(in super::super) fn liveness_scratch_words(function: &SemanticFunctionDeclV1) -> Result<usize> {
    liveness::scratch_words(function)
}

struct Candidate {
    site: SemanticTransparentBorrowSiteV1,
    source: u32,
    pointee: SemanticTypeIdV1,
    reference: SemanticTypeIdV1,
    live: usize,
    valid: bool,
    read: bool,
}

struct Alias {
    candidate: usize,
    fields: Vec<u32>,
    live: bool,
}

#[derive(Clone, Copy)]
struct Path<'a> {
    fields: &'a [SemanticProjectionV1],
    dereference: bool,
    selected: SemanticTypeIdV1,
}

fn primitive(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|declaration| {
        matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        )
    })
}

fn candidate(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    assignment: &SemanticAssignmentV1,
) -> Option<(u32, SemanticTypeIdV1, SemanticTypeIdV1)> {
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return None;
    };
    let destination = assignment.destination();
    let reference = assignment.value().result_type();
    if !place.projections().is_empty()
        || !destination.projections().is_empty()
        || reference != destination.ty()
        || function
            .locals()
            .get(destination.local().index() as usize)?
            .ty()
            != reference
        || function.locals().get(place.local().index() as usize)?.ty() != place.ty()
        || !primitive(types, place.ty())
    {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(reference.index() as usize)?.shape()
    else {
        return None;
    };
    (pointer.kind() == SemanticPointerKindV1::Reference
        && pointer.mutability() == SemanticMutabilityV1::Immutable
        && pointer.metadata() == SemanticPointerMetadataV1::None
        && pointer.pointee() == place.ty())
    .then_some((place.local().index(), place.ty(), reference))
}

pub(in super::super) fn has_candidates(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    meter: &mut impl BorrowWork,
) -> Result<bool> {
    for block in function.blocks() {
        meter.work(1)?;
        for statement in block.statements() {
            meter.work(24)?;
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if candidate(function, types, assignment).is_some() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(in super::super) trait ReadObserver {
    fn read(
        &mut self,
        borrow: SemanticTransparentBorrowSiteV1,
        source: u32,
        site: (u32, u32),
        place: &SemanticPlaceV1,
        meter: &mut impl BorrowWork,
    ) -> Result<()>;
}

struct NoReads;
impl ReadObserver for NoReads {
    fn read(
        &mut self,
        _: SemanticTransparentBorrowSiteV1,
        _: u32,
        _: (u32, u32),
        _: &SemanticPlaceV1,
        _: &mut impl BorrowWork,
    ) -> Result<()> {
        Ok(())
    }
}

struct Analysis<'a, 'b, M: BorrowWork, O: ReadObserver> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    meter: &'b mut M,
    observer: &'b mut O,
    site: Option<SemanticTransparentBorrowSiteV1>,
    candidates: Vec<Candidate>,
    holders: BTreeMap<u32, Vec<Alias>>,
    active: BTreeMap<u32, BTreeSet<usize>>,
    alias_words: usize,
    cap: usize,
    tree_work: usize,
    exhausted: bool,
}

impl<M: BorrowWork, O: ReadObserver> Analysis<'_, '_, M, O> {
    fn tree(&mut self) -> Result<()> {
        self.meter.work(self.tree_work)
    }

    fn reserve(&mut self, words: usize) -> Result<bool> {
        // Prepay teardown too, including resource-error or expansion fallback.
        self.meter
            .work(words.checked_add(8).ok_or(Error::ResourceOverflow)?)?;
        let next = self
            .alias_words
            .checked_add(words)
            .ok_or(Error::ResourceOverflow)?;
        if next > self.cap {
            self.exhausted = true;
            return Ok(false);
        }
        self.alias_words = next;
        Ok(true)
    }

    fn add_live(&mut self, index: usize) -> Result<()> {
        self.tree()?;
        let candidate = &mut self.candidates[index];
        if candidate.live == 0 {
            self.active
                .entry(candidate.source)
                .or_default()
                .insert(index);
        }
        candidate.live = candidate
            .live
            .checked_add(1)
            .ok_or(Error::ResourceOverflow)?;
        Ok(())
    }

    fn release(&mut self, alias: Alias) -> Result<()> {
        let mut alias = alias;
        self.deactivate(&mut alias)?;
        self.meter.work(alias.fields.len())?;
        self.alias_words = self
            .alias_words
            .checked_sub(1 + alias.fields.len())
            .ok_or(Error::ReplayMismatch)?;
        Ok(())
    }

    fn deactivate(&mut self, alias: &mut Alias) -> Result<()> {
        self.meter.work(1)?;
        if !alias.live {
            return Ok(());
        }
        self.tree()?;
        let candidate = &mut self.candidates[alias.candidate];
        candidate.live = candidate.live.checked_sub(1).ok_or(Error::ReplayMismatch)?;
        if candidate.live == 0 {
            let active = self
                .active
                .get_mut(&candidate.source)
                .ok_or(Error::ReplayMismatch)?;
            if !active.remove(&alias.candidate) {
                return Err(Error::ReplayMismatch);
            }
            if active.is_empty() {
                self.active.remove(&candidate.source);
            }
        }
        alias.live = false;
        Ok(())
    }

    fn discard(&mut self, aliases: Vec<Alias>, poison: bool) -> Result<()> {
        for alias in aliases {
            self.meter.work(1)?;
            if poison {
                self.candidates[alias.candidate].valid = false;
            }
            self.release(alias)?;
        }
        Ok(())
    }

    fn source_write(&mut self, local: u32) -> Result<()> {
        self.tree()?;
        if let Some(active) = self.active.get(&local) {
            for &index in active {
                self.meter.work(2)?;
                self.candidates[index].valid = false;
            }
        }
        Ok(())
    }

    fn poison_holder(&mut self, local: u32) -> Result<()> {
        self.tree()?;
        if let Some(aliases) = self.holders.get(&local) {
            for alias in aliases {
                self.meter.work(2)?;
                self.candidates[alias.candidate].valid = false;
            }
        }
        Ok(())
    }

    fn end_local(&mut self, local: u32) -> Result<()> {
        self.source_write(local)?;
        self.tree()?;
        if let Some(mut aliases) = self.holders.remove(&local) {
            for alias in &mut aliases {
                self.deactivate(alias)?;
            }
            self.tree()?;
            self.holders.insert(local, aliases);
        }
        Ok(())
    }

    fn path<'p>(&mut self, place: &'p SemanticPlaceV1) -> Result<Option<Path<'p>>> {
        self.meter.work(4)?;
        let Some(local) = self.function.locals().get(place.local().index() as usize) else {
            return Ok(None);
        };
        let mut ty = local.ty();
        for (index, projection) in place.projections().iter().enumerate() {
            self.meter.work(12)?;
            let Some(declaration) = self.types.get(ty.index() as usize) else {
                return Ok(None);
            };
            match (projection.kind(), declaration.shape()) {
                (
                    SemanticProjectionKindV1::Field(field),
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                ) => {
                    let Some(&field_type) = fields.fields().get(field as usize) else {
                        return Ok(None);
                    };
                    if field_type != projection.result_type() {
                        return Ok(None);
                    }
                    ty = field_type;
                }
                (SemanticProjectionKindV1::Dereference, SemanticTypeShapeV1::Pointer(pointer))
                    if index + 1 == place.projections().len()
                        && pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::None
                        && pointer.pointee() == projection.result_type()
                        && primitive(self.types, pointer.pointee())
                        && place.ty() == pointer.pointee() =>
                {
                    return Ok(Some(Path {
                        fields: &place.projections()[..index],
                        dereference: true,
                        selected: ty,
                    }));
                }
                _ => return Ok(None),
            }
        }
        Ok((ty == place.ty()).then_some(Path {
            fields: place.projections(),
            dereference: false,
            selected: ty,
        }))
    }

    fn matches(&mut self, fields: &[u32], prefix: &[SemanticProjectionV1]) -> Result<bool> {
        self.meter.work(
            (1 + fields.len().min(prefix.len()))
                .checked_mul(4)
                .ok_or(Error::ResourceOverflow)?,
        )?;
        Ok(fields.len() >= prefix.len()
            && fields.iter().zip(prefix).all(|(&field, projection)| {
                projection.kind() == SemanticProjectionKindV1::Field(field)
            }))
    }

    fn read_place(&mut self, place: &SemanticPlaceV1, consume: bool) -> Result<Vec<Alias>> {
        let local = place.local().index();
        let Some(path) = self.path(place)? else {
            self.poison_holder(local)?;
            self.source_write(local)?;
            return Ok(Vec::new());
        };
        if consume && !path.dereference {
            self.source_write(local)?;
        }
        self.tree()?;
        let mut retained = Vec::new();
        let mut selected = Vec::new();
        if let Some(aliases) = self.holders.remove(&local) {
            for mut alias in aliases {
                if self.matches(&alias.fields, path.fields)? {
                    if !alias.live {
                        self.candidates[alias.candidate].valid = false;
                    } else if path.dereference {
                        let candidate = &mut self.candidates[alias.candidate];
                        if alias.fields.len() != path.fields.len()
                            || consume
                            || candidate.reference != path.selected
                            || candidate.pointee != place.ty()
                        {
                            candidate.valid = false;
                        } else {
                            candidate.read = true;
                            if let Some(site) = self.site {
                                self.observer.read(
                                    candidate.site,
                                    candidate.source,
                                    (site.block, site.statement),
                                    place,
                                    self.meter,
                                )?;
                            }
                        }
                    } else {
                        let length = alias.fields.len() - path.fields.len();
                        if self.reserve(1 + length)? {
                            self.meter.work(length)?;
                            self.add_live(alias.candidate)?;
                            selected.push(Alias {
                                candidate: alias.candidate,
                                fields: alias.fields[path.fields.len()..].to_vec(),
                                live: true,
                            });
                            if consume {
                                self.deactivate(&mut alias)?;
                            }
                        }
                    }
                }
                retained.push(alias);
            }
        }
        if !retained.is_empty() {
            self.tree()?;
            self.holders.insert(local, retained);
        }
        Ok(selected)
    }

    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<Vec<Alias>> {
        self.meter.work(1)?;
        match operand {
            SemanticOperandV1::Copy(place) => self.read_place(place, false),
            SemanticOperandV1::Move(place) => self.read_place(place, true),
            SemanticOperandV1::Constant(_) => Ok(Vec::new()),
        }
    }

    fn scalar_operand(&mut self, operand: &SemanticOperandV1) -> Result<()> {
        let aliases = self.operand(operand)?;
        self.discard(aliases, true)
    }

    fn write_place<'p>(&mut self, place: &'p SemanticPlaceV1) -> Result<Option<Path<'p>>> {
        let local = place.local().index();
        self.source_write(local)?;
        let Some(path) = self.path(place)? else {
            self.poison_holder(local)?;
            return Ok(None);
        };
        if path.dereference {
            self.poison_holder(local)?;
            return Ok(None);
        }
        self.tree()?;
        let mut retained = Vec::new();
        if let Some(aliases) = self.holders.remove(&local) {
            for alias in aliases {
                if self.matches(&alias.fields, path.fields)? {
                    self.release(alias)?;
                } else {
                    retained.push(alias);
                }
            }
        }
        if !retained.is_empty() {
            self.tree()?;
            self.holders.insert(local, retained);
        }
        Ok(Some(path))
    }

    fn install(
        &mut self,
        place: &SemanticPlaceV1,
        path: Path<'_>,
        aliases: Vec<Alias>,
    ) -> Result<()> {
        if aliases.is_empty() {
            return Ok(());
        }
        self.tree()?;
        let mut installed = self
            .holders
            .remove(&place.local().index())
            .unwrap_or_default();
        for mut alias in aliases {
            if !self.reserve(path.fields.len())? {
                self.release(alias)?;
                continue;
            }
            self.meter.work(path.fields.len() + alias.fields.len())?;
            let mut fields = Vec::with_capacity(path.fields.len() + alias.fields.len());
            for projection in path.fields {
                let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                    return Err(Error::ReplayMismatch);
                };
                fields.push(field);
            }
            fields.append(&mut alias.fields);
            alias.fields = fields;
            installed.push(alias);
        }
        if !installed.is_empty() {
            self.tree()?;
            self.holders.insert(place.local().index(), installed);
        }
        Ok(())
    }

    fn deinitialize(&mut self, place: &SemanticPlaceV1) -> Result<()> {
        let local = place.local().index();
        self.source_write(local)?;
        let Some(path) = self.path(place)?.filter(|path| !path.dereference) else {
            return self.poison_holder(local);
        };
        self.tree()?;
        if let Some(mut aliases) = self.holders.remove(&local) {
            for alias in &mut aliases {
                if self.matches(&alias.fields, path.fields)? {
                    self.deactivate(alias)?;
                }
            }
            self.tree()?;
            self.holders.insert(local, aliases);
        }
        Ok(())
    }

    fn rvalue(&mut self, value: &SemanticRvalueV1) -> Result<Vec<Alias>> {
        self.meter.work(4)?;
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                let aliases = self.operand(operand)?;
                if operand.ty() == value.result_type() {
                    Ok(aliases)
                } else {
                    self.discard(aliases, true)?;
                    Ok(Vec::new())
                }
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let fields = self
                    .types
                    .get(value.result_type().index() as usize)
                    .and_then(
                        |declaration| match (aggregate.kind(), declaration.shape()) {
                            (
                                SemanticAggregateKindV1::Tuple,
                                SemanticTypeShapeV1::Tuple(fields),
                            )
                            | (
                                SemanticAggregateKindV1::Aggregate,
                                SemanticTypeShapeV1::Aggregate(fields),
                            ) => Some(fields.fields()),
                            _ => None,
                        },
                    );
                let mut exact =
                    fields.is_some_and(|fields| fields.len() == aggregate.operands().len());
                for (index, operand) in aggregate.operands().iter().enumerate() {
                    self.meter.work(3)?;
                    exact &= fields.is_some_and(|fields| fields.get(index) == Some(&operand.ty()));
                }
                let mut result = Vec::new();
                for (index, operand) in aggregate.operands().iter().enumerate() {
                    self.meter.work(3)?;
                    let aliases = self.operand(operand)?;
                    if !exact {
                        self.discard(aliases, true)?;
                        continue;
                    }
                    for mut alias in aliases {
                        if !self.reserve(1)? {
                            self.release(alias)?;
                            continue;
                        }
                        self.meter.work(1 + alias.fields.len())?;
                        alias.fields.insert(
                            0,
                            u32::try_from(index).map_err(|_| Error::ResourceOverflow)?,
                        );
                        result.push(alias);
                    }
                }
                Ok(result)
            }
            SemanticRvalueKindV1::Load(load) => {
                // Explicit memory loads still require a physical pointer in
                // the production lowerer, even when the access is nonvolatile.
                self.poison_holder(load.source().local().index())?;
                self.source_write(load.source().local().index())?;
                Ok(Vec::new())
            }
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => {
                self.poison_holder(place.local().index())?;
                self.source_write(place.local().index())?;
                Ok(Vec::new())
            }
            _ => {
                value
                    .kind()
                    .try_visit_operands(|operand| self.scalar_operand(operand))?;
                Ok(Vec::new())
            }
        }
    }

    fn statement(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        statement: &SemanticStatementKindV1,
    ) -> Result<()> {
        self.site = Some(site);
        self.meter.work(24)?;
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = assignment.destination();
                if let Some((source, pointee, reference)) =
                    candidate(self.function, self.types, assignment)
                {
                    let path = self
                        .write_place(destination)?
                        .ok_or(Error::ReplayMismatch)?;
                    let index = self.candidates.len();
                    self.candidates.push(Candidate {
                        site,
                        source,
                        pointee,
                        reference,
                        live: 0,
                        valid: true,
                        read: false,
                    });
                    if self.reserve(1)? {
                        self.add_live(index)?;
                        self.install(
                            destination,
                            path,
                            vec![Alias {
                                candidate: index,
                                fields: Vec::new(),
                                live: true,
                            }],
                        )?;
                    }
                } else {
                    let aliases = self.rvalue(assignment.value())?;
                    let path = self.write_place(destination)?;
                    if let Some(path) =
                        path.filter(|_| assignment.value().result_type() == destination.ty())
                    {
                        self.install(destination, path, aliases)?;
                    } else {
                        self.discard(aliases, true)?;
                    }
                }
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => self.end_local(local.index())?,
            SemanticStatementKindV1::Deinitialize(place) => self.deinitialize(place)?,
            SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                self.poison_holder(place.local().index())?;
                self.source_write(place.local().index())?;
            }
            SemanticStatementKindV1::Store(store) => {
                self.scalar_operand(store.value())?;
                self.write_place(store.destination())?;
            }
            SemanticStatementKindV1::AtomicRmw(operation) => {
                self.poison_holder(operation.address().local().index())?;
                self.source_write(operation.address().local().index())?;
                self.scalar_operand(operation.value())?;
                self.write_place(operation.destination())?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                self.poison_holder(operation.address().local().index())?;
                self.source_write(operation.address().local().index())?;
                self.scalar_operand(operation.expected())?;
                self.scalar_operand(operation.replacement())?;
                self.write_place(operation.destination())?;
            }
            SemanticStatementKindV1::Assume(operand) => self.scalar_operand(operand)?,
            SemanticStatementKindV1::Nop => {}
        }
        Ok(())
    }

    fn close_block(&mut self, terminator: &SemanticTerminatorKindV1) -> Result<()> {
        self.site = None;
        self.meter.work(1)?;
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in call.arguments() {
                    self.scalar_operand(argument)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for argument in call.arguments() {
                    self.scalar_operand(argument)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.scalar_operand(discriminant)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                self.poison_holder(place.local().index())?
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.scalar_operand(condition)?;
                partial_moves::visit_assert_operands_v1(message, &mut |operand| {
                    self.scalar_operand(operand)
                })?;
            }
            _ => {}
        }
        if matches!(terminator, SemanticTerminatorKindV1::Return) {
            for (&local, aliases) in &self.holders {
                self.meter.work(2)?;
                if self
                    .function
                    .locals()
                    .get(local as usize)
                    .is_some_and(|local| local.role() == SemanticLocalRoleV1::Return)
                {
                    for alias in aliases {
                        self.meter.work(1)?;
                        self.candidates[alias.candidate].valid = false;
                    }
                }
            }
        } else {
            // No alias crosses an edge or reaches a call/drop/return operand.
            for candidate in &mut self.candidates {
                self.meter.work(1)?;
                if candidate.live != 0 {
                    candidate.valid = false;
                }
            }
        }
        Ok(())
    }
}

pub(in super::super) fn analyze(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    cap: usize,
    meter: &mut Meter<'_>,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>> {
    analyze_observed(function, types, cap, meter, &mut NoReads)
}

pub(in super::super) fn analyze_observed(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    cap: usize,
    meter: &mut impl BorrowWork,
    observer: &mut impl ReadObserver,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>> {
    let height = (usize::BITS - cap.max(1).leading_zeros()) as usize + 1;
    let tree_work = height.checked_mul(32).ok_or(Error::ResourceOverflow)?;
    let mut result = BTreeSet::new();
    let liveness = liveness::Schedule::new(function, meter)?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        meter.work(1)?;
        let mut analysis = Analysis {
            function,
            types,
            meter,
            observer,
            site: None,
            candidates: Vec::new(),
            holders: BTreeMap::new(),
            active: BTreeMap::new(),
            alias_words: 0,
            cap,
            tree_work,
            exhausted: false,
        };
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: u32::try_from(block_index).map_err(|_| Error::ResourceOverflow)?,
                statement: u32::try_from(statement_index).map_err(|_| Error::ResourceOverflow)?,
            };
            analysis.statement(site, statement.kind())?;
            liveness.completed(&mut analysis, site, statement.kind())?;
            if analysis.exhausted {
                return Ok(BTreeSet::new());
            }
        }
        analysis.close_block(block.terminator().kind())?;
        if analysis.exhausted {
            return Ok(BTreeSet::new());
        }
        analysis.meter.work(
            analysis
                .alias_words
                .checked_mul(4)
                .ok_or(Error::ResourceOverflow)?,
        )?;
        for candidate in analysis.candidates {
            analysis.meter.work(tree_work)?;
            if candidate.valid && candidate.read {
                result.insert(candidate.site);
            }
        }
    }
    Ok(result)
}

// Inert retained alias-state primitives; no ordinary analysis route change.
#[allow(dead_code)]
#[path = "adapter_shared_primitive_alias_retained_v1.rs"]
mod retained_alias_state;
#[allow(unused_imports)]
use retained_alias_state::{RetainedAliasErrorV1, RetainedAliasSessionV1, RetainedAliasStateV1};
