//! Private shared scalar reads in the SSA owner's checked execution view.
//!
//! This is a memory-effect classifier, not scalar substitution or capability
//! authority. The original borrow, capture, assignment, and call transfers stay
//! in semantic MIR and must still pass SSA/KIR lowering.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssignmentV1, SemanticStatementV1};
use fe2o3_mir_model::{SemanticExpandedRootV1, SemanticExpandedStatementOriginV1};

mod aggregate_read;
mod scalar_enum_result;
mod flow_failure;
mod neutral_transfer_v1;
#[cfg(test)]
mod flow_failure_tests;
#[cfg(test)]
mod empty_goto_census_tests;
pub(super) mod flow_observation;
mod state;
#[cfg(test)]
mod ordered_join_owner_tests;
use flow_failure::{Failure, Phase};
use state::{Budget, Flow, RetainedFlow, Storage, Value};

const MAX_WORK: usize = 1_000_000;
const MAX_STORAGE: usize = 131_072;
const MAX_FIELDS: usize = 16;
const MAX_DEPTH: usize = 8;
type Result<T> = std::result::Result<T, ()>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Site {
    block: usize,
    statement: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reference {
    target: u32,
    mutable: bool,
    borrow: Site,
}

fn record_read(
    budget: &mut Budget,
    flow: &Flow,
    reads: &mut BTreeMap<usize, Reference>,
    storage: &mut Storage,
    statement: usize,
    reference: Reference,
) -> Result<()> {
    budget.charge(2)?;
    if reads.contains_key(&statement) {
        return Err(());
    }
    budget.check_storage(&[flow.nodes(), 2])?;
    storage.resize(
        reads
            .len()
            .checked_add(1)
            .and_then(|len| len.checked_mul(2))
            .ok_or(())?,
    )?;
    reads.insert(statement, reference);
    Ok(())
}

pub(super) struct PrivateScalarReads<'a> {
    function: &'a SemanticFunctionDeclV1,
    conditions: global_enum_transport_v1::Conditions<'a>,
    reads: BTreeMap<usize, Reference>,
    observation: flow_observation::Observation,
}

impl<'a> PrivateScalarReads<'a> {
    pub(super) fn for_root(
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
    ) -> Option<Self> {
        let view = owner.execution_view_for_root(root)?;
        let plan = owner.execution_plan_for_root(root)?;
        if plan.function_identity() != view.body().identity()
            || !owner
                .execution_expansion()
                .root(root)
                .is_some_and(|checked| std::ptr::eq(checked, view))
        {
            return None;
        }
        let mut analysis = Analysis::new(owner.source_semantic().types(), view);
        let result = analysis.run();
        let reads = result.unwrap_or_default();
        Some(Self {
            function: view.body(),
            conditions: analysis.conditions?,
            reads,
            observation: analysis.observation,
        })
    }

    pub(super) fn contains(
        &self,
        function: &SemanticFunctionDeclV1,
        statement: &SemanticStatementV1,
    ) -> bool {
        std::ptr::eq(self.function, function)
            && self.reads.contains_key(&(statement as *const _ as usize))
    }

    pub(super) fn conditions_for(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Option<&global_enum_transport_v1::Conditions<'a>> {
        self.conditions
            .matches(types, function)
            .then_some(&self.conditions)
    }

    pub(super) fn observation_for(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Option<flow_observation::Observation> {
        std::ptr::eq(self.function, function).then_some(self.observation)
    }
}

struct Analysis<'a> {
    types: &'a [SemanticTypeDeclV1],
    view: &'a SemanticExpandedRootV1,
    budget: Budget,
    observation: flow_observation::Observation,
    conditions: Option<global_enum_transport_v1::Conditions<'a>>,
}

impl<'a> Analysis<'a> {
    fn new(types: &'a [SemanticTypeDeclV1], view: &'a SemanticExpandedRootV1) -> Self {
        Self {
            types,
            view,
            budget: Budget::new(MAX_WORK),
            conditions: global_enum_transport_v1::Conditions::new(types, view.body()).ok(),
            observation: flow_observation::Observation {
                locals: view.body().locals().len(),
                blocks: view.body().blocks().len(),
                ..Default::default()
            },
        }
    }

    fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.view.body()
    }

    fn scalar(&self, ty: SemanticTypeIdV1) -> bool {
        let Some(decl) = self.types.get(ty.index() as usize) else {
            return false;
        };
        let size = match decl.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => 1,
            SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Integer {
                    bits: bits @ (8 | 16 | 32 | 64),
                    ..
                }
                | SemanticScalarTypeV1::Float {
                    bits: bits @ (32 | 64),
                },
            ) => u64::from(*bits / 8),
            _ => return false,
        };
        !decl.layout().is_uninhabited() && decl.layout().size_bytes() == Some(size)
    }

    fn reference_type(&self, ty: SemanticTypeIdV1) -> Option<(SemanticTypeIdV1, bool)> {
        let SemanticTypeShapeV1::Pointer(pointer) = self.types.get(ty.index() as usize)?.shape()
        else {
            return None;
        };
        (pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.metadata() == SemanticPointerMetadataV1::None
            && pointer.address_space() == 0)
            .then_some((
                pointer.pointee(),
                pointer.mutability() == SemanticMutabilityV1::Mutable,
            ))
    }

    fn variant_fields(&self, ty: SemanticTypeIdV1, variant: u32) -> Option<&[SemanticTypeIdV1]> {
        let SemanticTypeShapeV1::Enum { variants, .. } =
            self.types.get(ty.index() as usize)?.shape()
        else {
            return None;
        };
        if variants.len() != 2 {
            return None;
        }
        let variant = variants.get(variant as usize)?;
        (!variant.is_uninhabited() && variant.fields().fields().len() <= MAX_FIELDS)
            .then_some(variant.fields().fields())
    }

    fn field_type(&self, ty: SemanticTypeIdV1, field: u32) -> Option<SemanticTypeIdV1> {
        let fields = match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => fields,
            _ => return None,
        };
        fields.fields().get(field as usize).copied()
    }

    fn candidate<'s>(&self, statement: &'s SemanticStatementV1) -> Option<&'s SemanticPlaceV1> {
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return None;
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return None;
        };
        let [projection] = place.projections() else {
            return None;
        };
        let base = self
            .function()
            .locals()
            .get(place.local().index() as usize)?
            .ty();
        (assignment.destination().projections().is_empty()
            && assignment.destination().ty() == place.ty()
            && assignment.value().result_type() == place.ty()
            && self.scalar(place.ty())
            && projection.kind() == SemanticProjectionKindV1::Dereference
            && projection.result_type() == place.ty()
            && self.reference_type(base) == Some((place.ty(), false)))
        .then_some(place)
    }

    fn checked_site(&self, site: Site) -> bool {
        let Some(origin) = self.view.block_origins().get(site.block) else {
            return false;
        };
        let Some(statement) = origin.statements().get(site.statement) else {
            return false;
        };
        let Some(frame) = self
            .view
            .instances()
            .get(origin.instance().index() as usize)
        else {
            return false;
        };
        if frame.function() != origin.function() {
            return false;
        }
        match statement {
            SemanticExpandedStatementOriginV1::Source { .. } => true,
            SemanticExpandedStatementOriginV1::ParameterTransfer { callee, .. }
            | SemanticExpandedStatementOriginV1::FrameStorageLive { callee, .. } => self
                .view
                .instances()
                .get(callee.index() as usize)
                .is_some_and(|frame| frame.parent() == Some(origin.instance())),
            SemanticExpandedStatementOriginV1::ReturnTransfer { callee }
            | SemanticExpandedStatementOriginV1::FrameStorageDead { callee, .. } => {
                *callee == origin.instance()
            }
        }
    }

    fn run(&mut self) -> Result<BTreeMap<usize, Reference>> {
        let result = self.run_inner();
        self.observation.completed = result.is_ok();
        self.observation.remaining_work = self.budget.remaining;
        self.observation.work_exhausted = self.budget.work_exhausted;
        self.observation.admitted_reads = result.as_ref().map_or(0, BTreeMap::len);
        if result.is_ok() {
            self.observation.phase = Phase::Complete;
        } else if let Some(failure) = self.budget.failure() {
            self.observation.failure.get_or_insert(Failure::Resource(failure));
        } else {
            self.observation.failure.get_or_insert(Failure::UnclassifiedTransfer);
        }
        result
    }

    fn run_inner(&mut self) -> Result<BTreeMap<usize, Reference>> {
        self.observation.phase = Phase::Conditions;
        let Some(conditions) = self.conditions.as_ref() else {
            return self.observation.reject(Failure::ConditionsUnavailable);
        };
        self.budget.charge(conditions.work_units())?;
        let function = self.function();
        self.observation.phase = Phase::CandidateScan;
        self.budget
            .charge(function.locals().len() + function.blocks().len())?;
        let mut has_candidate = false;
        for block in function.blocks() {
            self.budget.charge(block.statements().len() + 1)?;
            has_candidate |= block.statements().iter().any(|statement| {
                self.candidate(statement).is_some() || self.aggregate_candidate(statement).is_some()
            });
        }
        if !has_candidate {
            return Ok(BTreeMap::new());
        }
        let entry = function.entry().index() as usize;
        self.observation.phase = Phase::CfgPreflight;
        if entry >= function.blocks().len() {
            return self.observation.reject(Failure::InvalidEntry);
        }
        if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS {
            self.observation.empty_goto_census = Some(
                source_cfg_diagnostic_v1::empty_goto_census_v1::capture(function),
            );
            return self.observation.reject(Failure::BlockLimit {
                blocks: function.blocks().len(), limit: MAX_RANKED_BOUNDS_BLOCKS,
            });
        }
        // Entries, queued bits and pending slots each have block-sized capacity.
        // Successors have at most two distinct (target, return-local) pairs per
        // block. Keep their reservation live even while consuming the set.
        let _cfg_storage = self
            .budget
            .reserve(function.blocks().len().checked_mul(5).ok_or(())?)?;
        let mut initial = Flow::default();
        self.observation.phase = Phase::InitialState;
        let mut stored = initial.nodes();
        for block in function.blocks() {
            self.budget.charge(block.statements().len() + 1)?;
            for statement in block.statements() {
                if let SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) = statement.kind()
                {
                    if !initial.dead.contains(&local.index()) {
                        self.budget.check_storage(&[stored, 1])?;
                        initial.dead.insert(local.index());
                        stored += 1;
                    }
                }
            }
        }
        for (local, declaration) in function.locals().iter().enumerate() {
            self.budget.charge(1)?;
            if matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)) {
                self.budget.check_storage(&[stored, 2])?;
                initial.values.insert_metered(local as u32, Value::Opaque, &mut self.budget)?;
                stored += 2;
            }
        }
        let initial = RetainedFlow::new(initial, &mut self.budget)?;
        let mut entries: Vec<Option<RetainedFlow>> =
            (0..function.blocks().len()).map(|_| None).collect();
        entries[entry] = Some(initial);
        let mut pending = VecDeque::with_capacity(entries.len());
        pending.push_back(entry);
        let mut queued = vec![false; entries.len()];
        queued[entry] = true;
        while let Some(block) = pending.pop_front() {
            self.observation.block(block);
            self.observation.phase = Phase::EntryClone;
            self.budget.charge(1)?;
            queued[block] = false;
            let Some(source) = entries[block].as_ref() else {
                return self.observation.reject(Failure::MissingEntry);
            };
            let (outgoing, flow) = if neutral_transfer_v1::is_identity(
                &self.function().blocks()[block],
                &mut self.budget,
            )? {
                self.observe_terminator(block);
                let outgoing = self.terminator_edges(block, None)?;
                self.observation.phase = Phase::OutgoingReservation;
                self.budget.charge(1)?;
                (outgoing, source.share(&mut self.budget)?)
            } else {
                let mut flow = source.working(&mut self.budget)?;
                self.observation.phase = Phase::Statements;
                self.statements(block, &mut flow, None)?;
                let outgoing = self.terminator(block, &mut flow)?;
                self.observation.phase = Phase::OutgoingReservation;
                self.budget.charge(1)?;
                (outgoing, RetainedFlow::new(flow, &mut self.budget)?)
            };
            for (target, returned) in outgoing {
                self.observation.successor = Some((target, returned));
                self.observation.phase = Phase::SuccessorClone;
                if target >= entries.len() {
                    return self.observation.reject(Failure::InvalidSuccessor);
                }
                let next = if let Some(local) = returned {
                    let mut next = flow.working(&mut self.budget)?;
                    self.observation.phase = Phase::CallReturn;
                    let value = self.call_return_value(local, &next)?;
                    next.assign(local, Some(value), &mut self.budget)?;
                    RetainedFlow::new(next, &mut self.budget)?
                } else {
                    flow.share(&mut self.budget)?
                };
                self.observation.phase = Phase::Join;
                let merged = match &entries[target] {
                    Some(previous) => {
                        let merged = previous.join(&next, &mut self.budget)?;
                        drop(next);
                        merged
                    }
                    None => next,
                };
                self.observation.phase = Phase::StoreSuccessor;
                let changed = match entries[target].as_ref() {
                    Some(previous) => !previous.publication_eq(&merged, &mut self.budget)?,
                    None => {
                        self.budget.charge(merged.flow().nodes())?;
                        true
                    }
                };
                if changed {
                    // The new immutable payload and handle are already reserved.
                    // Replacing an entry releases its old handle, and its payload
                    // only after the final shared observer is gone.
                    entries[target] = Some(merged);
                    if !queued[target] {
                        queued[target] = true;
                        pending.push_back(target);
                    }
                }
            }
        }
        // Publish only after the fixed point: an early visit cannot authorize a
        // read invalidated by another predecessor or a loop backedge.
        let mut reads = BTreeMap::new();
        self.observation.phase = Phase::Publication;
        let mut read_storage = self.budget.reserve(0)?;
        for (block, entry) in entries.into_iter().enumerate() {
            self.observation.block(block);
            self.observation.phase = Phase::Publication;
            self.budget.charge(1)?;
            if let Some(entry) = entry {
                // The publication pass interprets statements only. No mutable
                // state or read reservation is needed to visit an empty list.
                self.budget.charge(1)?;
                if self.function().blocks()[block].statements().is_empty() {
                    continue;
                }
                let mut flow = entry.into_working(&mut self.budget)?;
                self.observation.phase = Phase::PublicationStatements;
                self.statements(block, &mut flow, Some((&mut reads, &mut read_storage)))?;
            }
        }
        Ok(reads)
    }

    fn statements(
        &mut self,
        block: usize,
        flow: &mut Flow,
        mut reads: Option<(&mut BTreeMap<usize, Reference>, &mut Storage)>,
    ) -> Result<()> {
        for (statement_index, statement) in self.function().blocks()[block]
            .statements()
            .iter()
            .enumerate()
        {
            self.budget.charge(1)?;
            let site = Site {
                block,
                statement: statement_index,
            };
            self.observation.last_block = Some(block);
            self.observation.last_statement = Some(statement_index);
            if reads.is_some() {
                if let Some(place) = self
                    .candidate(statement)
                    .or_else(|| self.aggregate_candidate(statement))
                {
                    self.observation.candidate(site, place, flow);
                }
            }
            if !self.checked_site(site) {
                return self.observation.reject(Failure::InvalidCheckedSite);
            }
            if let Some((reads, read_storage)) = reads.as_mut()
                && let Some(place) = self.candidate(statement)
                && let Some(Value::Reference(reference)) = flow
                    .values
                    .get_metered(place.local().index(), &mut self.budget)?
                && !reference.mutable
                && !flow.escaped.contains(&reference.target)
                && flow
                    .values
                    .get_metered(reference.target, &mut self.budget)?
                    == Some(&Value::Opaque)
                && self
                    .function()
                    .locals()
                    .get(reference.target as usize)
                    .is_some_and(|local| {
                        local.ty() == place.ty()
                            && !matches!(local.role(), SemanticLocalRoleV1::Argument(_))
                    })
            {
                record_read(
                    &mut self.budget,
                    flow,
                    reads,
                    read_storage,
                    statement as *const _ as usize,
                    *reference,
                )?;
            }
            if let Some((reads, read_storage)) = reads.as_mut()
                && let Some(place) = self.aggregate_candidate(statement)
                && let Some(reference) = self.aggregate_read_reference(place, flow)?
            {
                record_read(
                    &mut self.budget,
                    flow,
                    reads,
                    read_storage,
                    statement as *const _ as usize,
                    reference,
                )?;
            }
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    self.assignment(site, assignment, flow)?
                }
                SemanticStatementKindV1::StorageLive(local) => {
                    flow.kill(local.index(), &mut self.budget)?;
                    flow.dead.remove(&local.index());
                }
                SemanticStatementKindV1::StorageDead(local) => {
                    flow.kill(local.index(), &mut self.budget)?;
                    self.budget.check_storage(&[flow.nodes(), 1])?;
                    flow.dead.insert(local.index());
                }
                SemanticStatementKindV1::Deinitialize(place)
                | SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                    self.kill_place(place, flow)?
                }
                SemanticStatementKindV1::Assume(operand) => {
                    self.operand(operand, flow, block)?;
                }
                SemanticStatementKindV1::Nop => {}
                SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_) => {
                    flow.forget_references(&mut self.budget)?
                }
            }
            self.budget.charge(1)?;
            self.budget.check_storage(&[flow.nodes()])?;
        }
        Ok(())
    }

    fn read(
        &mut self,
        place: &SemanticPlaceV1,
        flow: &mut Flow,
        block: usize,
    ) -> Result<Option<Value>> {
        self.budget.charge(place.projections().len() + 1)?;
        let Some(local) = self.function().locals().get(place.local().index() as usize) else {
            return self.observation.reject(Failure::InvalidLocal);
        };
        let mut ty = local.ty();
        let mut value = flow.copy_value(place.local().index(), &mut self.budget)?;
        let mut active_variant = None;
        for (projection_index, projection) in place.projections().iter().enumerate() {
            value = match (projection.kind(), value) {
                (
                    SemanticProjectionKindV1::Downcast(variant),
                    Some(Value::Variants {
                        ty: enum_ty,
                        possible,
                        mut fields,
                    }),
                ) if projection_index == 0
                    && enum_ty == ty
                    && projection.result_type() == ty
                    && variant < 2
                    && fields.len() == 2
                    && possible & (1 << variant) != 0
                    && self.conditions.as_ref().is_some_and(|conditions| {
                        conditions.allows(
                            self.types,
                            self.function(),
                            place.local(),
                            variant,
                            block,
                        )
                    }) =>
                {
                    active_variant = Some(variant);
                    fields[variant as usize].take()
                }
                (SemanticProjectionKindV1::Downcast(_), Some(value))
                | (_, Some(value @ Value::Variants { .. })) => {
                    // Failed variant activation is unknown/uninitialized data,
                    // not an Opaque initialized scalar after another projection.
                    flow.escape_value(&value, &mut self.budget)?;
                    return Ok(None);
                }
                (SemanticProjectionKindV1::Dereference, Some(Value::Reference(reference))) => {
                    if self.reference_type(ty)
                        != Some((projection.result_type(), reference.mutable))
                        || flow.escaped.contains(&reference.target)
                        || self
                            .function()
                            .locals()
                            .get(reference.target as usize)
                            .is_none_or(|local| local.ty() != projection.result_type())
                    {
                        return Ok(None);
                    }
                    flow.copy_value(reference.target, &mut self.budget)?
                }
                (SemanticProjectionKindV1::Field(field), Some(Value::Fields(fields))) => {
                    let field_ty = if let Some(variant) = active_variant.take() {
                        self.variant_fields(ty, variant)
                            .and_then(|fields| fields.get(field as usize))
                            .copied()
                    } else {
                        self.field_type(ty, field)
                    };
                    if field_ty != Some(projection.result_type()) {
                        return Ok(None);
                    }
                    // Move the selected child; don't clone it while its entire
                    // captured environment is still allocated.
                    fields.into_iter().nth(field as usize).flatten()
                }
                (_, Some(Value::Opaque)) => Some(Value::Opaque),
                (_, Some(value)) => {
                    // Unsupported projections may observe data, but never retain
                    // a private reference origin through a cast/index/downcast.
                    self.budget.charge(value.nodes())?;
                    if value.contains_references() {
                        flow.escape_value(&value, &mut self.budget)?;
                    }
                    Some(Value::Opaque)
                }
                (_, None) => None,
            };
            ty = projection.result_type();
        }
        if ty != place.ty() || active_variant.is_some() {
            return Ok(None);
        }
        Ok(value)
    }

    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        flow: &mut Flow,
        block: usize,
    ) -> Result<Option<Value>> {
        match operand {
            SemanticOperandV1::Constant(_) => Ok(Some(Value::Opaque)),
            SemanticOperandV1::Copy(place) => self.read(place, flow, block),
            SemanticOperandV1::Move(place) => {
                let value = self.read(place, flow, block)?;
                let _value_storage = self
                    .budget
                    .reserve(value.as_ref().map_or(0, Value::nodes))?;
                self.kill_place(place, flow)?;
                Ok(value)
            }
        }
    }

    fn borrow_target(
        &mut self,
        place: &SemanticPlaceV1,
        flow: &Flow,
    ) -> Result<Option<(u32, bool)>> {
        self.budget.charge(1)?;
        if place.projections().is_empty() {
            return Ok(flow
                .values
                .get_metered(place.local().index(), &mut self.budget)?
                .is_some()
                .then_some((place.local().index(), true)));
        }
        let [projection] = place.projections() else {
            return Ok(None);
        };
        if projection.kind() != SemanticProjectionKindV1::Dereference {
            return Ok(None);
        }
        let Some(Value::Reference(reference)) = flow
            .values
            .get_metered(place.local().index(), &mut self.budget)?
        else {
            return Ok(None);
        };
        let base = self
            .function()
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(|| { self.observation.failure.get_or_insert(Failure::InvalidLocal); })?
            .ty();
        Ok(
            (self.reference_type(base) == Some((place.ty(), reference.mutable))
                && projection.result_type() == place.ty()
                && flow
                    .values
                    .get_metered(reference.target, &mut self.budget)?
                    .is_some())
            .then_some((reference.target, reference.mutable)),
        )
    }

    fn assignment(
        &mut self,
        site: Site,
        assignment: &SemanticAssignmentV1,
        flow: &mut Flow,
    ) -> Result<()> {
        let value = match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand) => self.operand(operand, flow, site.block)?,
            SemanticRvalueKindV1::Borrow { kind, place } => {
                let pair = self.reference_type(assignment.value().result_type());
                let expected_mutable = match kind {
                    SemanticBorrowKindV1::Shared => Some(false),
                    SemanticBorrowKindV1::Mutable => Some(true),
                    SemanticBorrowKindV1::Fake => None,
                };
                match (pair, expected_mutable, self.borrow_target(place, flow)?) {
                    (Some((pointee, mutable)), Some(expected), Some((target, writable)))
                        if pointee == place.ty()
                            && mutable == expected
                            && (!mutable || writable)
                            && !flow.escaped.contains(&target)
                            && !matches!(
                                self.function().locals()[target as usize].role(),
                                SemanticLocalRoleV1::Argument(_)
                            ) =>
                    {
                        if mutable {
                            flow.invalidate(target, &mut self.budget)?;
                        }
                        Some(Value::Reference(Reference {
                            target,
                            mutable,
                            borrow: site,
                        }))
                    }
                    _ => Some(Value::Opaque),
                }
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let variant = match aggregate.kind() {
                    SemanticAggregateKindV1::EnumVariant(variant)
                        if self
                            .variant_fields(assignment.value().result_type(), *variant)
                            .is_some() =>
                    {
                        Some(*variant)
                    }
                    _ => None,
                };
                let supported = matches!(
                    aggregate.kind(),
                    SemanticAggregateKindV1::Tuple | SemanticAggregateKindV1::Aggregate
                ) || variant.is_some();
                let supported = supported && aggregate.operands().len() <= MAX_FIELDS;
                let capacity = if supported {
                    aggregate.operands().len()
                } else {
                    0
                };
                self.budget.check_storage(&[flow.nodes(), capacity, 1])?;
                let mut field_nodes = capacity + 1;
                let mut field_storage = self.budget.reserve(field_nodes)?;
                let mut fields = Vec::with_capacity(capacity);
                for (field, operand) in aggregate.operands().iter().enumerate() {
                    self.budget.charge(1)?;
                    let value = self.operand(operand, flow, site.block)?;
                    let field_ty = if let Some(variant) = variant {
                        self.variant_fields(assignment.value().result_type(), variant)
                            .and_then(|fields| fields.get(field))
                            .copied()
                    } else {
                        self.field_type(assignment.value().result_type(), field as u32)
                    };
                    if supported && field_ty == Some(operand.ty()) {
                        let nodes = value.as_ref().map_or(1, Value::nodes);
                        self.budget.check_storage(&[flow.nodes(), nodes])?;
                        field_nodes = field_nodes.checked_add(nodes - 1).ok_or(())?;
                        field_storage.resize(field_nodes)?;
                        fields.push(value);
                    } else if let Some(value) = value {
                        flow.escape_value(&value, &mut self.budget)?;
                    }
                }
                if supported && fields.len() == aggregate.operands().len() {
                    if let Some(variant) = variant {
                        if self
                            .variant_fields(assignment.value().result_type(), variant)
                            .map(<[_]>::len)
                            != Some(fields.len())
                        {
                            return self.observation.reject(Failure::VariantFieldCount);
                        }
                        self.budget.check_storage(&[flow.nodes(), 3])?;
                        field_storage.resize(field_nodes.checked_add(3).ok_or(())?)?;
                        let mut alternatives = vec![None, None];
                        alternatives[variant as usize] = Some(Value::Fields(fields));
                        Some(Value::Variants {
                            ty: assignment.value().result_type(),
                            possible: 1 << variant,
                            fields: alternatives,
                        })
                    } else {
                        Some(Value::Fields(fields))
                    }
                } else {
                    Some(Value::Opaque)
                }
            }
            SemanticRvalueKindV1::AddressOf { place, .. } => {
                if let Some((target, _)) = self.borrow_target(place, flow)? {
                    flow.escape(target, &mut self.budget)?;
                } else {
                    flow.forget_references(&mut self.budget)?;
                }
                Some(Value::Opaque)
            }
            value => {
                let mut initialized = match value {
                    SemanticRvalueKindV1::Length(place)
                    | SemanticRvalueKindV1::Discriminant(place) => {
                        self.read(place, flow, site.block)?.is_some()
                    }
                    SemanticRvalueKindV1::Load(load) => {
                        self.read(load.source(), flow, site.block)?.is_some()
                    }
                    _ => true,
                };
                value.try_visit_operands(|operand| {
                    self.budget.charge(1)?;
                    let value = self.operand(operand, flow, site.block)?;
                    initialized &= value.is_some();
                    if let Some(value) = value {
                        flow.escape_value(&value, &mut self.budget)?;
                    }
                    Ok::<(), ()>(())
                })?;
                initialized.then_some(Value::Opaque)
            }
        };
        self.budget.charge(value.as_ref().map_or(1, Value::nodes))?;
        if value
            .as_ref()
            .is_some_and(|value| value.depth() > MAX_DEPTH)
        {
            return self.observation.reject(Failure::DepthLimit);
        }
        if assignment.destination().projections().is_empty() {
            flow.assign(
                assignment.destination().local().index(),
                value,
                &mut self.budget,
            )
        } else {
            if let Some(value) = &value {
                flow.escape_value(value, &mut self.budget)?;
            }
            let _value_storage = self
                .budget
                .reserve(value.as_ref().map_or(0, Value::nodes))?;
            self.kill_place(assignment.destination(), flow)
        }
    }

    fn kill_place(&mut self, place: &SemanticPlaceV1, flow: &mut Flow) -> Result<()> {
        self.budget.charge(place.projections().len() + 1)?;
        if place
            .projections()
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            if let Some((target, _)) = self.borrow_target(place, flow)? {
                flow.kill(target, &mut self.budget)
            } else {
                flow.forget_references(&mut self.budget)
            }
        } else {
            flow.kill(place.local().index(), &mut self.budget)
        }
    }

    fn observe_terminator(&mut self, block: usize) {
        self.observation.last_block = Some(block);
        self.observation.last_statement = None;
        self.observation.phase = Phase::TerminatorTransfer;
        let terminator = self.function().blocks()[block].terminator().kind();
        self.observation.terminator = Some(match terminator {
            SemanticTerminatorKindV1::Call(_) => "Call",
            SemanticTerminatorKindV1::SwitchInt { .. } => "SwitchInt",
            SemanticTerminatorKindV1::Assert { .. } => "Assert",
            SemanticTerminatorKindV1::Drop { .. } => "Drop",
            SemanticTerminatorKindV1::Goto(_) => "Goto",
            SemanticTerminatorKindV1::Return => "Return",
            SemanticTerminatorKindV1::UnwindResume => "UnwindResume",
            SemanticTerminatorKindV1::UnwindTerminate => "UnwindTerminate",
            SemanticTerminatorKindV1::Abort => "Abort",
            SemanticTerminatorKindV1::Unreachable => "Unreachable",
            SemanticTerminatorKindV1::TailCall(_) => "TailCall",
            SemanticTerminatorKindV1::FalseEdge { .. } => "FalseEdge",
        });
    }

    fn terminator(
        &mut self,
        block: usize,
        flow: &mut Flow,
    ) -> Result<BTreeSet<(usize, Option<u32>)>> {
        self.observe_terminator(block);
        let terminator = self.function().blocks()[block].terminator().kind();
        let mut returned = None;
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.budget.charge(1)?;
                    if let Some(value) = self.operand(operand, flow, block)? {
                        flow.escape_value(&value, &mut self.budget)?;
                    }
                }
                if let Some(destination) = call.destination() {
                    self.kill_place(destination.place(), flow)?;
                    if destination.place().projections().is_empty() {
                        returned = Some((
                            destination.edge().target().index() as usize,
                            destination.place().local().index(),
                        ));
                    }
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.operand(discriminant, flow, block)?;
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => {
                self.operand(condition, flow, block)?;
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.kill_place(place, flow)?,
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
            SemanticTerminatorKindV1::TailCall(_) | SemanticTerminatorKindV1::FalseEdge { .. } => {
                return self.observation.reject(Failure::UnsupportedTerminator);
            }
        }
        self.terminator_edges(block, returned)
    }

    fn terminator_edges(
        &mut self,
        block: usize,
        returned: Option<(usize, u32)>,
    ) -> Result<BTreeSet<(usize, Option<u32>)>> {
        let terminator = self.function().blocks()[block].terminator().kind();
        let mut successors = BTreeSet::new();
        self.observation.phase = Phase::TerminatorEdges;
        terminator.try_for_each_edge(|edge| {
            self.budget.charge(1)?;
            let target = edge.target().index() as usize;
            let returned_local = returned
                .filter(|(block, _)| {
                    *block == target && edge.role() == SemanticEdgeRoleV1::CallReturn
                })
                .map(|(_, local)| local);
            self.observation.edge(target, edge.role(), returned_local);
            if target >= self.function().blocks().len() {
                return self.observation.reject(Failure::InvalidSuccessor);
            }
            successors.insert((target, returned_local));
            Ok::<(), ()>(())
        })?;
        Ok(successors)
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn private_scalar_capture_final_reads_share_retained_and_working_storage_ceiling() {
        let flow = Flow {
            values: BTreeMap::from([(0, Value::Opaque)]).into(),
            ..Flow::default()
        };
        let mut budget = Budget::new(MAX_WORK);
        let _entries = budget.reserve(MAX_STORAGE - flow.nodes() - 2).unwrap();
        let mut read_storage = budget.reserve(0).unwrap();
        let mut reads = BTreeMap::new();
        let reference = Reference {
            target: 0,
            mutable: false,
            borrow: Site {
                block: 0,
                statement: 0,
            },
        };
        record_read(
            &mut budget,
            &flow,
            &mut reads,
            &mut read_storage,
            1,
            reference,
        )
        .unwrap();
        assert!(
            record_read(
                &mut budget,
                &flow,
                &mut reads,
                &mut read_storage,
                2,
                reference
            )
            .is_err()
        );
        assert_eq!(reads, BTreeMap::from([(1, reference)]));
        assert!(budget.check_storage(&[flow.nodes()]).is_ok());
        assert!(budget.check_storage(&[flow.nodes(), 1]).is_err());
    }
}
