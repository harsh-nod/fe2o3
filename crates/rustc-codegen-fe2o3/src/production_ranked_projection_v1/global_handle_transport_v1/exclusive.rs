//! Consuming custody for an owned product with one exclusive Global leaf.
//! This never extends the shared Capture/Product APIs or grants element access.
use super::*;
use std::sync::Arc;

const MAX_HANDLES: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Exclusive {
    function: usize,
    types: usize,
    type_count: usize,
    paths: Arc<[Capture]>,
}

impl Exclusive {
    pub(in super::super) fn storage_units(&self) -> usize {
        // The fixed owner and Arc bookkeeping fit in one old inline fact unit.
        1 + self.paths.len()
    }

    fn matches(&self, types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> bool {
        self.function == function as *const _ as usize
            && self.types == types.as_ptr() as usize
            && self.type_count == types.len()
    }
}

#[derive(Clone, Copy)]
struct Selection {
    paths: [Option<Capture>; MAX_HANDLES],
    len: usize,
}

impl Selection {
    fn empty() -> Self {
        Self {
            paths: [None; MAX_HANDLES],
            len: 0,
        }
    }

    fn push(&mut self, path: Capture) -> Option<()> {
        if self.len == MAX_HANDLES || self.paths[0].is_some_and(|first| first.ty != path.ty) {
            return None;
        }
        self.paths[self.len] = Some(path);
        self.len += 1;
        Some(())
    }

    fn leaf(path: Capture) -> Self {
        let mut result = Self::empty();
        result.push(path).expect("one bounded leaf");
        result
    }

    fn ty(&self) -> Option<SemanticTypeIdV1> {
        self.paths[0].map(|path| path.ty)
    }

    fn exclusive_count(&self) -> usize {
        self.paths
            .iter()
            .flatten()
            .filter(|path| path.view.borrow == Some(SemanticBorrowKindV1::Mutable))
            .count()
    }
}

fn leaf(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    view: ProjectedGlobalViewV1,
) -> Option<Capture> {
    let mutability = match view.borrow {
        Some(SemanticBorrowKindV1::Shared)
            if !view.allocation.writable
                && view.contract == SemanticCapabilityMemoryContractV1::global_read_only() =>
        {
            SemanticMutabilityV1::Immutable
        }
        Some(SemanticBorrowKindV1::Mutable)
            if view.allocation.writable
                && view.contract
                    == SemanticCapabilityMemoryContractV1::global_exclusive_read_write() =>
        {
            SemanticMutabilityV1::Mutable
        }
        _ => return None,
    };
    is_exact_reference_to_v1(types, ty, view.view, mutability).then_some(Capture {
        view,
        ty,
        steps: [None; MAX_STEPS],
        len: 0,
    })
}

fn read(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> Option<Selection> {
    let local = place.local().index() as usize;
    let ty = function.locals().get(local)?.ty();
    let mut selected = match state.get(&local)? {
        ProjectedCapabilityValueV1::ExclusiveGlobalCapture(value)
            if value.matches(types, function) =>
        {
            let mut selected = Selection::empty();
            for path in value.paths.iter().copied() {
                selected.push(path)?;
            }
            selected
        }
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))
            if place.projections().is_empty() =>
        {
            Selection::leaf(leaf(types, ty, *view)?)
        }
        _ => return None,
    };
    if selected.ty()? != ty || place.projections().len() > MAX_STEPS {
        return None;
    }
    for projection in place.projections() {
        let SemanticProjectionKindV1::Field(selected_field) = projection.kind() else {
            return None;
        };
        let input = selected.ty()?;
        let output = *fields(types, input, None)?.get(selected_field as usize)?;
        if projection.result_type() != output {
            return None;
        }
        let mut next = Selection::empty();
        for path in selected.paths.into_iter().flatten() {
            let step = path.steps[0]?;
            let Kind::Field(field) = step.kind else {
                return None;
            };
            if step.input != input
                || fields(types, input, None)?.get(field as usize) != Some(&step.output)
            {
                return None;
            }
            if field == selected_field {
                next.push(path.tail()?)?;
            }
        }
        if next.len == 0 || next.ty()? != output {
            return None;
        }
        selected = next;
    }
    (selected.ty()? == place.ty()).then_some(selected)
}

fn value(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    selected: Selection,
    work: &mut usize,
) -> Result<Option<ProjectedCapabilityValueV1>, ProductionRankedProjectionErrorV1> {
    if selected.len == 1
        && let Some(path) = selected.paths[0]
        && path.len == 0
    {
        return Ok(leaf(types, path.ty, path.view).map(|_| {
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(path.view))
        }));
    }
    if selected.exclusive_count() != 1 {
        return Ok(None);
    }
    let provenance = selected.paths[0].expect("selected prefix").view.provenance;
    if selected
        .paths
        .iter()
        .flatten()
        .any(|path| path.view.provenance != provenance)
    {
        return Ok(None);
    }
    for left in 0..selected.len {
        for right in left + 1..selected.len {
            let a = selected.paths[left].expect("selected prefix");
            let b = selected.paths[right].expect("selected prefix");
            if a.view.allocation.allocation_origin == b.view.allocation.allocation_origin {
                return Ok(None);
            }
        }
    }
    // Bound the new vector and Arc payload while the working source still lives.
    // Retained states separately count the immutable payload on every reference.
    // HashMap iteration visits buckets, including holes left by a must-meet.
    // This new preflight scan is charged to the caller's existing work owner.
    charge_capability_dataflow_work_v1(work, state.capacity())?;
    checked_capability_stored_entries_v1(
        capability_state_storage_units_v1(state)?,
        2 * (1 + selected.len),
    )?;
    let mut paths = Vec::new();
    paths.try_reserve_exact(selected.len).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "exclusive Global capture storage cannot be reserved",
        )
    })?;
    paths.extend(selected.paths.into_iter().flatten());
    Ok(Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(
        Exclusive {
            function: function as *const _ as usize,
            types: types.as_ptr() as usize,
            type_count: types.len(),
            paths: paths.into(),
        },
    )))
}

fn is_exclusive(value: Option<&ProjectedCapabilityValueV1>) -> bool {
    match value {
        Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_)) => true,
        Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))) => {
            // A partially conflicting record is a rejected exclusive candidate,
            // not an opportunity to fall back to shared-only aggregation.
            view.borrow == Some(SemanticBorrowKindV1::Mutable)
                || view.allocation.writable
                || view.contract
                    == SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
        }
        _ => false,
    }
}

pub(in super::super) fn assignment(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    work: &mut usize,
) -> Result<Option<ProjectedCapabilityValueV1>, ProductionRankedProjectionErrorV1> {
    let candidate = match assignment.value().kind() {
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            // The candidate scan is bounded before examining operands and charged
            // even if the candidate is rejected or contains no exclusive leaf.
            if aggregate.operands().len() > MAX_FIELDS {
                return Ok(None);
            }
            charge_capability_dataflow_work_v1(work, aggregate.operands().len() + 1)?;
            aggregate
                .operands()
                .iter()
                .filter_map(raw_operand_place)
                .any(|place| is_exclusive(state.get(&(place.local().index() as usize))))
        }
        SemanticRvalueKindV1::Use(operand) => raw_operand_place(operand).is_some_and(|place| {
            matches!(
                state.get(&(place.local().index() as usize)),
                Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
            )
        }),
        _ => false,
    };
    if !candidate {
        return Ok(None);
    }
    // At most sixteen operands with four paths and eight field steps each.
    // Includes reads, prefix checking, duplicate checks and construction.
    // Operand consumption is charged separately by charge_statement.
    let operands = match assignment.value().kind() {
        SemanticRvalueKindV1::Aggregate(aggregate) => aggregate.operands().len(),
        _ => 1,
    };
    charge_capability_dataflow_work_v1(
        work,
        1 + operands * MAX_HANDLES * (MAX_STEPS + 3) + MAX_HANDLES * MAX_HANDLES,
    )?;
    let destination = assignment.destination();
    let selected = (|| {
        if !destination.projections().is_empty()
            || function
                .locals()
                .get(destination.local().index() as usize)?
                .ty()
                != destination.ty()
            || assignment.value().result_type() != destination.ty()
        {
            return None;
        }
        match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand) => {
                read(types, function, state, raw_operand_place(operand)?)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                if !matches!(
                    aggregate.kind(),
                    SemanticAggregateKindV1::Aggregate | SemanticAggregateKindV1::Tuple
                ) {
                    return None;
                }
                let field_types = fields(types, destination.ty(), None)?;
                if field_types.len() != aggregate.operands().len() {
                    return None;
                }
                let mut result = Selection::empty();
                for (field, (ty, operand)) in
                    field_types.iter().zip(aggregate.operands()).enumerate()
                {
                    if *ty != operand.ty() {
                        return None;
                    }
                    let Some(place) = raw_operand_place(operand) else {
                        continue;
                    };
                    let Some(selected) = read(types, function, state, place) else {
                        if is_exclusive(state.get(&(place.local().index() as usize))) {
                            return None;
                        }
                        continue;
                    };
                    for path in selected.paths.into_iter().flatten() {
                        result.push(path.prepend(destination.ty(), Kind::Field(field as u32))?)?;
                    }
                }
                Some(result)
            }
            _ => None,
        }
    })();
    let origin = match selected {
        Some(selected) if selected.ty() == Some(destination.ty()) => {
            value(types, function, state, selected, work)?
        }
        _ => None,
    };
    // A rejected exclusive candidate must not fall back to shared aggregation
    // and silently omit the unsupported exclusive sibling.
    Ok(Some(origin.unwrap_or(ProjectedCapabilityValueV1::Invalid)))
}

pub(in super::super) fn preserves_shared_leaf_copy(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> bool {
    if !matches!(
        state.get(&(place.local().index() as usize)),
        Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
    ) {
        return false;
    }
    let Some(selected) = read(types, function, state, place) else {
        return false;
    };
    selected.len == 1
        && selected.paths[0].is_some_and(|path| {
            path.len == 0 && path.view.borrow == Some(SemanticBorrowKindV1::Shared)
        })
}

pub(in super::super) fn invalidates_borrow(
    state: &ProjectedCapabilityStateV1,
    value: &SemanticRvalueKindV1,
) -> Option<SemanticLocalIdV1> {
    let place = match value {
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => place,
        _ => return None,
    };
    matches!(
        state.get(&(place.local().index() as usize)),
        Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
    )
    .then_some(place.local())
}

pub(in super::super) fn metadata_observation(
    value: &SemanticRvalueKindV1,
    origin: Option<&ProjectedCapabilityValueV1>,
) -> bool {
    matches!(value, SemanticRvalueKindV1::Use(_))
        && matches!(
            origin,
            Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
                | Some(ProjectedCapabilityValueV1::Known(
                    ProjectedCapabilityOriginV1::GlobalView(ProjectedGlobalViewV1 {
                        borrow: Some(SemanticBorrowKindV1::Mutable),
                        ..
                    })
                ))
        )
}

pub(in super::super) fn invalidate_store(
    state: &mut ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) {
    let local = place.local().index() as usize;
    if matches!(
        state.get(&local),
        Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
    ) {
        invalidate_capability_local_v1(state, local);
    }
}

fn charge_operand(
    state: &ProjectedCapabilityStateV1,
    operand: &SemanticOperandV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if raw_operand_place(operand).is_some_and(|place| {
        matches!(
            state.get(&(place.local().index() as usize)),
            Some(ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_))
        )
    }) {
        charge_capability_dataflow_work_v1(work, MAX_HANDLES * (MAX_STEPS + 1))?;
    }
    Ok(())
}

pub(in super::super) fn charge_statement(
    state: &ProjectedCapabilityStateV1,
    statement: &SemanticStatementKindV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => assignment
            .value()
            .kind()
            .try_visit_operands(|operand| charge_operand(state, operand, work)),
        SemanticStatementKindV1::Store(store) => charge_operand(state, store.value(), work),
        SemanticStatementKindV1::AtomicRmw(atomic) => charge_operand(state, atomic.value(), work),
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            charge_operand(state, atomic.expected(), work)?;
            charge_operand(state, atomic.replacement(), work)
        }
        SemanticStatementKindV1::Assume(operand) => charge_operand(state, operand, work),
        _ => Ok(()),
    }
}

pub(in super::super) fn charge_terminator(
    state: &ProjectedCapabilityStateV1,
    terminator: &SemanticTerminatorKindV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut charge = |operand: &SemanticOperandV1| charge_operand(state, operand, work);
    match terminator {
        SemanticTerminatorKindV1::Call(call) => {
            for operand in call.arguments() {
                charge(operand)?;
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for operand in call.arguments() {
                charge(operand)?;
            }
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. }
        | SemanticTerminatorKindV1::Assert {
            condition: discriminant,
            ..
        } => charge(discriminant)?,
        _ => (),
    }
    Ok(())
}
