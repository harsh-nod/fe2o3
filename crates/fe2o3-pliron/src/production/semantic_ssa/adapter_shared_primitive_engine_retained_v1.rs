//! Original statement producers over retained, separately attached arenas.
use super::*;
use fe2o3_mir_model::semantic_mir_v1 as model;

#[derive(Clone, Copy)]
enum Output {
    Rhs,
    Installed,
    Candidates,
    Replacement,
    CurrentFields,
}

impl<'s, 'a, 'w> RetainedAliasSessionV1<'s, 'a, 'w> {
    fn install(&mut self, place: &'a SemanticPlaceV1, path: Path<'a>) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.current.is_some()
                || self.owner.drain.is_some()
                || self.owner.removed.is_some()
                || self.owner.installed.is_some()
                || !self.owner.replacement.is_empty()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.place = Some(place);
            self.owner.path = Some(path);
            if self.owner.rhs.is_empty() {
                return Ok(());
            }
            self.tree()?;
            self.owner.installed = Some(
                self.owner
                    .data
                    .holders
                    .remove(&place.local().index())
                    .unwrap_or_default(),
            );
            self.owner.drain = Some(std::mem::take(&mut self.owner.rhs).into_iter());
            loop {
                self.owner.current = self
                    .owner
                    .drain
                    .as_mut()
                    .ok_or(Error::ReplayMismatch)?
                    .next();
                if self.owner.current.is_none() {
                    break;
                }
                if !self.reserve(path.fields.len())? {
                    self.release_current()?;
                    continue;
                }
                let length = path.fields.len() + self.alias(Slot::Current)?.fields.len();
                alias_work(self.budget, &mut self.owner.failure, length)?;
                self.producer_capacity(Output::Replacement, length)?;
                for projection in path.fields {
                    let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                        return Err(Error::ReplayMismatch);
                    };
                    self.owner.replacement.push(field);
                }
                self.owner.replacement.append(
                    &mut self
                        .owner
                        .current
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?
                        .fields,
                );
                std::mem::swap(
                    &mut self
                        .owner
                        .current
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?
                        .fields,
                    &mut self.owner.replacement,
                );
                let needed = self
                    .owner
                    .installed
                    .as_ref()
                    .ok_or(Error::ReplayMismatch)?
                    .len()
                    .checked_add(1)
                    .ok_or(Error::ResourceOverflow)?;
                self.producer_capacity(Output::Installed, needed)?;
                let alias = self.owner.current.take().ok_or(Error::ReplayMismatch)?;
                self.owner
                    .installed
                    .as_mut()
                    .ok_or(Error::ReplayMismatch)?
                    .push(alias);
            }
            self.owner.drain = None;
            if !self
                .owner
                .installed
                .as_ref()
                .ok_or(Error::ReplayMismatch)?
                .is_empty()
            {
                self.tree()?;
                self.reserve_storage(size_of::<(u32, Vec<Alias>)>())
                    .map_err(|_| Error::ResourceOverflow)?;
                let installed = self.owner.installed.take().ok_or(Error::ReplayMismatch)?;
                self.owner
                    .data
                    .holders
                    .insert(place.local().index(), installed);
            } else {
                // Original successfully exhausted empty installed owner. No fallible
                // operation follows this retirement inside the primitive.
                self.owner.installed = None;
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn discard_rhs(&mut self, poison: bool) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.drain.is_some() || self.owner.current.is_some() {
                return Err(Error::ReplayMismatch);
            }
            self.owner.drain = Some(std::mem::take(&mut self.owner.rhs).into_iter());
            self.discard_pending(poison)
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn rvalue(&mut self, value: &'a SemanticRvalueV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if !self.owner.rhs.is_empty()
                || !self.owner.selected.is_empty()
                || self.owner.drain.is_some()
                || self.owner.current.is_some()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.value = Some(value);
            alias_work(self.budget, &mut self.owner.failure, 4)?;
            match value.kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    self.operand(operand)?;
                    std::mem::swap(&mut self.owner.rhs, &mut self.owner.selected);
                    if operand.ty() != value.result_type() {
                        self.discard_rhs(true)?;
                    }
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    let source = self.owner.source.ok_or(Error::ReplayMismatch)?;
                    let fields = source
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
                        alias_work(self.budget, &mut self.owner.failure, 3)?;
                        exact &=
                            fields.is_some_and(|fields| fields.get(index) == Some(&operand.ty()));
                    }
                    for (index, operand) in aggregate.operands().iter().enumerate() {
                        alias_work(self.budget, &mut self.owner.failure, 3)?;
                        self.operand(operand)?;
                        self.owner.drain =
                            Some(std::mem::take(&mut self.owner.selected).into_iter());
                        if !exact {
                            self.discard_pending(true)?;
                            continue;
                        }
                        loop {
                            self.owner.current = self
                                .owner
                                .drain
                                .as_mut()
                                .ok_or(Error::ReplayMismatch)?
                                .next();
                            if self.owner.current.is_none() {
                                break;
                            }
                            if !self.reserve(1)? {
                                self.release_current()?;
                                continue;
                            }
                            let needed = 1 + self.alias(Slot::Current)?.fields.len();
                            alias_work(self.budget, &mut self.owner.failure, needed)?;
                            let field =
                                u32::try_from(index).map_err(|_| Error::ResourceOverflow)?;
                            self.producer_capacity(Output::CurrentFields, needed)?;
                            self.owner
                                .current
                                .as_mut()
                                .ok_or(Error::ReplayMismatch)?
                                .fields
                                .insert(0, field);
                            let needed = self
                                .owner
                                .rhs
                                .len()
                                .checked_add(1)
                                .ok_or(Error::ResourceOverflow)?;
                            self.producer_capacity(Output::Rhs, needed)?;
                            let alias = self.owner.current.take().ok_or(Error::ReplayMismatch)?;
                            self.owner.rhs.push(alias);
                        }
                        self.owner.drain = None;
                    }
                }
                SemanticRvalueKindV1::Load(load) => {
                    self.poison_holder(load.source().local().index())?;
                    self.source_write(load.source().local().index())?;
                }
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                | SemanticRvalueKindV1::Length(place)
                | SemanticRvalueKindV1::Discriminant(place) => {
                    self.poison_holder(place.local().index())?;
                    self.source_write(place.local().index())?;
                }
                _ => self.scalar_rvalue_operands(value.kind())?,
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    // The original generic visitor does not expose its source lifetime. This
    // fixed traversal preserves its exact branch/order while retaining &'a.
    fn scalar_rvalue_operands(&mut self, kind: &'a SemanticRvalueKindV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = match kind {
            SemanticRvalueKindV1::Use(operand)
            | SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast { operand, .. } => self.scalar_operand(operand),
            SemanticRvalueKindV1::Binary { left, right, .. } => self
                .scalar_operand(left)
                .and_then(|()| self.scalar_operand(right)),
            SemanticRvalueKindV1::CheckedBinary(checked) => self
                .scalar_operand(checked.left())
                .and_then(|()| self.scalar_operand(checked.right())),
            SemanticRvalueKindV1::UncheckedBinary(unchecked) => self
                .scalar_operand(unchecked.left())
                .and_then(|()| self.scalar_operand(unchecked.right())),
            SemanticRvalueKindV1::Aggregate(aggregate) => (|| {
                for operand in aggregate.operands() {
                    self.scalar_operand(operand)?;
                }
                Ok(())
            })(),
            SemanticRvalueKindV1::Borrow { .. }
            | SemanticRvalueKindV1::AddressOf { .. }
            | SemanticRvalueKindV1::Length(_)
            | SemanticRvalueKindV1::Discriminant(_)
            | SemanticRvalueKindV1::Load(_) => Ok(()),
        };
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn statement(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        statement: &'a SemanticStatementKindV1,
    ) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if !self.owner.rhs.is_empty()
                || self.owner.installed.is_some()
                || !self.owner.replacement.is_empty()
                || !self.owner.selected.is_empty()
                || !self.owner.fields.is_empty()
                || !self.owner.output.is_empty()
                || self.owner.current.is_some()
                || self.owner.drain.is_some()
                || self.owner.removed.is_some()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.statement = Some(statement);
            self.owner.site = Some(site);
            alias_work(self.budget, &mut self.owner.failure, 24)?;
            match statement {
                SemanticStatementKindV1::Assign(assignment) => {
                    let destination = assignment.destination();
                    let source = self.owner.source.ok_or(Error::ReplayMismatch)?;
                    if let Some((source, pointee, reference)) =
                        candidate(source.function, source.types, assignment)
                    {
                        let path = self
                            .write_place(destination)?
                            .ok_or(Error::ReplayMismatch)?;
                        let index = self.owner.data.candidates.len();
                        self.producer_capacity(
                            Output::Candidates,
                            index.checked_add(1).ok_or(Error::ResourceOverflow)?,
                        )?;
                        self.owner.data.candidates.push(Candidate {
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
                            self.producer_capacity(Output::Rhs, 1)?;
                            self.owner.rhs.push(Alias {
                                candidate: index,
                                fields: Vec::new(),
                                live: true,
                            });
                            self.install(destination, path)?;
                        }
                    } else {
                        self.rvalue(assignment.value())?;
                        let path = self.write_place(destination)?;
                        if let Some(path) =
                            path.filter(|_| assignment.value().result_type() == destination.ty())
                        {
                            self.install(destination, path)?;
                        } else {
                            self.discard_rhs(true)?;
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
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn close_block(&mut self, terminator: &'a SemanticTerminatorKindV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if !self.owner.rhs.is_empty()
                || self.owner.installed.is_some()
                || !self.owner.replacement.is_empty()
                || !self.owner.selected.is_empty()
                || !self.owner.fields.is_empty()
                || !self.owner.output.is_empty()
                || self.owner.current.is_some()
                || self.owner.drain.is_some()
                || self.owner.removed.is_some()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.terminator = Some(terminator);
            self.owner.site = None;
            alias_work(self.budget, &mut self.owner.failure, 1)?;
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
                    self.scalar_assert_operands(message)?;
                }
                _ => {}
            }
            if matches!(terminator, SemanticTerminatorKindV1::Return) {
                let source = self.owner.source.ok_or(Error::ReplayMismatch)?;
                for (&local, aliases) in &self.owner.data.holders {
                    alias_work(self.budget, &mut self.owner.failure, 2)?;
                    if source
                        .function
                        .locals()
                        .get(local as usize)
                        .is_some_and(|local| local.role() == SemanticLocalRoleV1::Return)
                    {
                        for alias in aliases {
                            alias_work(self.budget, &mut self.owner.failure, 1)?;
                            self.owner.data.candidates[alias.candidate].valid = false;
                        }
                    }
                }
            } else {
                for candidate in &mut self.owner.data.candidates {
                    alias_work(self.budget, &mut self.owner.failure, 1)?;
                    if candidate.live != 0 {
                        candidate.valid = false;
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn scalar_assert_operands(
        &mut self,
        message: &'a model::SemanticAssertMessageV1,
    ) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        use model::SemanticAssertMessageV1 as Message;
        let result = match message {
            Message::BoundsCheck { length, index } => self
                .scalar_operand(length)
                .and_then(|()| self.scalar_operand(index)),
            Message::Overflow { left, right, .. } => self
                .scalar_operand(left)
                .and_then(|()| self.scalar_operand(right)),
            Message::DivisionByZero(operand) | Message::RemainderByZero(operand) => {
                self.scalar_operand(operand)
            }
            Message::MisalignedPointerDereference {
                required_alignment,
                found_alignment,
            } => self
                .scalar_operand(required_alignment)
                .and_then(|()| self.scalar_operand(found_alignment)),
            Message::NullPointerDereference
            | Message::ResumedAfterReturn
            | Message::ResumedAfterPanic => Ok(()),
        };
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(in super::super) fn expiry_work(&mut self, units: usize) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = alias_work(self.budget, &mut self.owner.failure, units);
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
    pub(in super::super) fn expiry_result(&mut self, result: Result<()>) -> Result<()> {
        if self.owner.phase != Phase::Active && result.is_ok() {
            return Err(Error::ReplayMismatch);
        }
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
    pub(in super::super) fn expiry_drop_local(&mut self, local: u32) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.current.is_some() || self.owner.drain.is_some() {
                return Err(Error::ReplayMismatch);
            }
            self.tree()?;
            self.owner.drain = self.owner.data.holders.remove(&local).map(Vec::into_iter);
            if self.owner.drain.is_some() {
                self.discard_pending(false)?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn producer_capacity(&mut self, output: Output, needed: usize) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            let (length, old, element) = match output {
                Output::Rhs => (
                    self.owner.rhs.len(),
                    self.owner.rhs.capacity(),
                    size_of::<Alias>(),
                ),
                Output::Installed => {
                    let vector = self.owner.installed.as_ref().ok_or(Error::ReplayMismatch)?;
                    (vector.len(), vector.capacity(), size_of::<Alias>())
                }
                Output::Candidates => (
                    self.owner.data.candidates.len(),
                    self.owner.data.candidates.capacity(),
                    size_of::<Candidate>(),
                ),
                Output::Replacement => (
                    self.owner.replacement.len(),
                    self.owner.replacement.capacity(),
                    size_of::<u32>(),
                ),
                Output::CurrentFields => {
                    let vector = &self
                        .owner
                        .current
                        .as_ref()
                        .ok_or(Error::ReplayMismatch)?
                        .fields;
                    (vector.len(), vector.capacity(), size_of::<u32>())
                }
            };
            if old >= needed {
                return Ok(());
            }
            let growth = (needed - old)
                .checked_mul(element)
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(growth)
                .map_err(|_| Error::ResourceOverflow)?;
            let additional = needed.checked_sub(length).ok_or(Error::ReplayMismatch)?;
            let allocation = match output {
                Output::Rhs => self.owner.rhs.try_reserve_exact(additional),
                Output::Installed => self
                    .owner
                    .installed
                    .as_mut()
                    .ok_or(Error::ReplayMismatch)?
                    .try_reserve_exact(additional),
                Output::Candidates => self.owner.data.candidates.try_reserve_exact(additional),
                Output::Replacement => self.owner.replacement.try_reserve_exact(additional),
                Output::CurrentFields => self
                    .owner
                    .current
                    .as_mut()
                    .ok_or(Error::ReplayMismatch)?
                    .fields
                    .try_reserve_exact(additional),
            };
            allocation.map_err(|_| {
                self.failed(Resource::Allocation);
                Error::ResourceOverflow
            })?;
            let actual = match output {
                Output::Rhs => self.owner.rhs.capacity(),
                Output::Installed => self
                    .owner
                    .installed
                    .as_ref()
                    .ok_or(Error::ReplayMismatch)?
                    .capacity(),
                Output::Candidates => self.owner.data.candidates.capacity(),
                Output::Replacement => self.owner.replacement.capacity(),
                Output::CurrentFields => self
                    .owner
                    .current
                    .as_ref()
                    .ok_or(Error::ReplayMismatch)?
                    .fields
                    .capacity(),
            };
            let excess = actual
                .checked_sub(needed)
                .ok_or(Resource::Accounting)
                .and_then(|count| count.checked_mul(element).ok_or(Resource::Arithmetic))
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(excess)
                .map_err(|_| Error::ResourceOverflow)?;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnginePhase {
    Fresh,
    Terminal,
    Complete,
}

/// Source-driven retained analysis. The actual Shared wrapper establishes source/SSA
/// membership and original discovery/sizing before entering this owner.
pub(in super::super::super::super) struct RetainedSharedEngineV1<'a> {
    phase: EnginePhase,
    source: Option<(&'a SemanticFunctionDeclV1, &'a [SemanticTypeDeclV1])>,
    aliases: RetainedAliasStateV1<'a>,
    liveness: liveness::RetainedSharedLivenessV1<'a>,
    retired: Vec<AliasData>,
    accepted: BTreeSet<SemanticTransparentBorrowSiteV1>,
    empty_result: BTreeSet<SemanticTransparentBorrowSiteV1>,
    block: usize,
    statement: usize,
    candidate: usize,
    exhausted: bool,
}
impl<'a> RetainedSharedEngineV1<'a> {
    pub(in super::super::super::super) fn new() -> Self {
        Self {
            phase: EnginePhase::Fresh,
            source: None,
            aliases: RetainedAliasStateV1::new(),
            liveness: liveness::RetainedSharedLivenessV1::new(),
            retired: Vec::new(),
            accepted: BTreeSet::new(),
            empty_result: BTreeSet::new(),
            block: 0,
            statement: 0,
            candidate: 0,
            exhausted: false,
        }
    }
    /// Still-private, unadmitted row preparation seam. The actual wrapper must
    /// supply the original has_candidates/scan_size/scratch chronology.
    pub(in super::super::super::super) fn prepare_observer_into(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        scratch: usize,
        units: usize,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> RetainedResult<()> {
        let fresh = self.phase == EnginePhase::Fresh;
        self.phase = EnginePhase::Terminal;
        if !fresh {
            return Err(self.aliases.failure.unwrap_or(Resource::Accounting).into());
        }
        self.source = Some((function, types));
        self.aliases
            .prepare_observer_into(function, types, scratch, units, budget, owned)?;
        self.phase = EnginePhase::Fresh;
        Ok(())
    }
    pub(in super::super::super::super) fn analyze_into(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        cap: usize,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> RetainedResult<()> {
        let fresh = self.phase == EnginePhase::Fresh;
        self.phase = EnginePhase::Terminal;
        if !fresh {
            return Err(self.aliases.failure.unwrap_or(Resource::Accounting).into());
        }
        // Retain the exact borrowed source before arithmetic or pair refusal.
        self.source = Some((function, types));
        self.aliases
            .observer
            .postflight_for(function, types, budget, owned, &self.aliases.failure)
            .map_err(|error| *self.aliases.failure.get_or_insert(error))?;
        let height = (usize::BITS - cap.max(1).leading_zeros()) as usize + 1;
        let tree_work = height
            .checked_mul(32)
            .ok_or(RetainedAliasErrorV1::Original(Error::ResourceOverflow))?;
        let mut session = RetainedAliasSessionV1::begin(
            &mut self.aliases,
            function,
            types,
            cap,
            tree_work,
            budget,
            owned,
        )?;
        self.liveness
            .prepare_into(function, session.budget, session.owned)
            .map_err(|error| match error {
                liveness::RetainedLivenessErrorV1::Resource(error) => {
                    RetainedAliasErrorV1::Resource(session.failed(error))
                }
                liveness::RetainedLivenessErrorV1::Original(error) => session.map_error(error),
            })?;
        let result = (|| {
            for (block_index, block) in function.blocks().iter().enumerate() {
                self.block = block_index;
                self.statement = 0;
                self.candidate = 0;
                session.expiry_work(1)?;
                if block_index != 0 {
                    // Both old and replacement DATA remain attached until the
                    // archive's real capacity has been admitted.
                    retired_capacity(&mut self.retired, &mut session)?;
                    let next = AliasData {
                        candidates: Vec::new(),
                        holders: BTreeMap::new(),
                        active: BTreeMap::new(),
                        alias_words: 0,
                        cap,
                        tree_work,
                        exhausted: false,
                    };
                    let previous = std::mem::replace(&mut session.owner.data, next);
                    self.retired.push(previous);
                    session.owner.site = None;
                }
                for (statement_index, statement) in block.statements().iter().enumerate() {
                    self.statement = statement_index;
                    let site = SemanticTransparentBorrowSiteV1 {
                        block: u32::try_from(block_index).map_err(|_| Error::ResourceOverflow)?,
                        statement: u32::try_from(statement_index)
                            .map_err(|_| Error::ResourceOverflow)?,
                    };
                    session.statement(site, statement.kind())?;
                    let schedule = self
                        .liveness
                        .completed_for(function, session.budget, session.owned)
                        .map_err(|error| match error {
                            liveness::RetainedLivenessErrorV1::Resource(error) => {
                                session.failed(error);
                                Error::ResourceOverflow
                            }
                            liveness::RetainedLivenessErrorV1::Original(error) => error,
                        })?;
                    schedule.completed_retained(&mut session, site, statement.kind())?;
                    if session.owner.data.exhausted {
                        self.exhausted = true;
                        return Ok(());
                    }
                }
                session.close_block(block.terminator().kind())?;
                if session.owner.data.exhausted {
                    self.exhausted = true;
                    return Ok(());
                }
                let teardown = session
                    .owner
                    .data
                    .alias_words
                    .checked_mul(4)
                    .ok_or(Error::ResourceOverflow)?;
                session.expiry_work(teardown)?;
                while self.candidate < session.owner.data.candidates.len() {
                    // Index rather than an owning temporary retains all candidate
                    // DATA at every original per-candidate denial.
                    session.expiry_work(tree_work)?;
                    let candidate = &session.owner.data.candidates[self.candidate];
                    if candidate.valid && candidate.read {
                        let site = candidate.site;
                        session
                            .reserve_storage(size_of::<SemanticTransparentBorrowSiteV1>())
                            .map_err(|_| Error::ResourceOverflow)?;
                        self.accepted.insert(site);
                    }
                    self.candidate += 1;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            return Err(session.map_error(error));
        }
        session.finish()?;
        self.phase = EnginePhase::Complete;
        Ok(())
    }

    /// Concrete original observer postprocessing. The source-driven analysis and
    /// every retained owner are checked before the allocation-free row transfer.
    pub(in super::super::super::super) fn finish_rows_into(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        promoted: &[bool],
        output: &mut super::super::super::super::shared_primitive_reads_v1::RetainedSharedRowsV1,
        budget: &mut Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<()> {
        let ready = self
            .accepted_for(function, types, budget, owned)
            .map(|_| ());
        self.phase = EnginePhase::Terminal;
        ready?;
        let accepted = if self.exhausted {
            &self.empty_result
        } else {
            &self.accepted
        };
        self.aliases
            .observer
            .finish_rows_into(
                function,
                types,
                accepted,
                promoted,
                output,
                budget,
                owned,
                &mut self.aliases.failure,
            )
            .map_err(RetainedAliasErrorV1::Resource)
    }
    pub(in super::super::super::super) fn accepted_for(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &Budget<'_>,
        owned: &usize,
    ) -> RetainedResult<&BTreeSet<SemanticTransparentBorrowSiteV1>> {
        if self.phase != EnginePhase::Complete {
            return Err(Resource::Accounting.into());
        }
        let (bound_function, bound_types) = self.source.ok_or(Resource::Accounting)?;
        if !std::ptr::eq(bound_function, function) || !std::ptr::eq(bound_types, types) {
            return Err(Resource::Accounting.into());
        }
        self.aliases
            .postflight_for(function, types, budget, owned)?;
        self.liveness
            .completed_for(function, budget, owned)
            .map_err(|error| match error {
                liveness::RetainedLivenessErrorV1::Resource(error) => {
                    RetainedAliasErrorV1::Resource(error)
                }
                liveness::RetainedLivenessErrorV1::Original(error) => {
                    RetainedAliasErrorV1::Original(error)
                }
            })?;
        Ok(if self.exhausted {
            &self.empty_result
        } else {
            &self.accepted
        })
    }
}
fn retired_capacity(
    output: &mut Vec<AliasData>,
    session: &mut RetainedAliasSessionV1<'_, '_, '_>,
) -> Result<()> {
    if session.owner.phase != Phase::Active {
        return Err(Error::ReplayMismatch);
    }
    let result = (|| {
        if output.len() < output.capacity() {
            return Ok(());
        }
        let needed = output
            .len()
            .checked_add(1)
            .ok_or(Resource::Arithmetic)
            .map_err(|error| {
                session.failed(error);
                Error::ResourceOverflow
            })?;
        let growth = needed
            .checked_sub(output.capacity())
            .and_then(|count| count.checked_mul(size_of::<AliasData>()))
            .ok_or(Resource::Arithmetic)
            .map_err(|error| {
                session.failed(error);
                Error::ResourceOverflow
            })?;
        session
            .reserve_storage(growth)
            .map_err(|_| Error::ResourceOverflow)?;
        output.try_reserve_exact(1).map_err(|_| {
            session.failed(Resource::Allocation);
            Error::ResourceOverflow
        })?;
        let excess = output
            .capacity()
            .checked_sub(needed)
            .ok_or(Resource::Accounting)
            .and_then(|count| {
                count
                    .checked_mul(size_of::<AliasData>())
                    .ok_or(Resource::Arithmetic)
            })
            .map_err(|error| {
                session.failed(error);
                Error::ResourceOverflow
            })?;
        session
            .reserve_storage(excess)
            .map_err(|_| Error::ResourceOverflow)?;
        Ok(())
    })();
    if result.is_err() {
        session.owner.phase = Phase::Terminal;
    }
    result
}

include!("adapter_shared_primitive_engine_frame_retained_v1.rs");

#[cfg(test)]
#[path = "adapter_shared_primitive_engine_retained_v1_tests.rs"]
mod tests;
