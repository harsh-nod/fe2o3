//! A small immutable product of independently authenticated shared paths.
//! No field is selected by type, and a product is never an unconditional origin.
use super::*;
use std::sync::Arc;

const MAX_HANDLES: usize = 4;

fn admitted_path(path: &Capture) -> bool {
    path.is_unconditional()
        && path.view.borrow == Some(SemanticBorrowKindV1::Shared)
        && path.view.contract == SemanticCapabilityMemoryContractV1::global_read_only()
        && !path.view.allocation.writable
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Product(Arc<[Capture]>);

impl Product {
    // Count shared storage again in every retained state. This overestimates
    // sharing and includes one ordinary entry for the Arc allocation header.
    pub(in super::super) fn storage_units(&self) -> usize {
        1 + self.0.len()
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

    fn one(path: Capture) -> Self {
        let mut result = Self::empty();
        result.push(path).expect("one fixed path");
        result
    }

    fn from_value(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        value: &ProjectedCapabilityValueV1,
    ) -> Option<Self> {
        if let ProjectedCapabilityValueV1::CapturedGlobalProduct(product) = value {
            let mut result = Self::empty();
            for path in product.0.iter().copied() {
                if path.ty != ty || !admitted_path(&path) {
                    return None;
                }
                result.push(path)?;
            }
            (result.len >= 2).then_some(result)
        } else {
            super::from_value(types, ty, value).map(Self::one)
        }
    }

    fn ty(&self) -> Option<SemanticTypeIdV1> {
        self.paths[0].map(|path| path.ty)
    }

    fn single(self) -> Option<Capture> {
        (self.len == 1).then_some(self.paths[0]).flatten()
    }

    fn prepend(self, input: SemanticTypeIdV1, kind: Kind) -> Option<Self> {
        let mut result = Self::empty();
        for path in self.paths.into_iter().flatten() {
            result.push(path.prepend(input, kind)?)?;
        }
        Some(result)
    }

    fn value(self) -> Option<ProjectedCapabilityValueV1> {
        if let Some(path) = self.single() {
            return Some(path.value());
        }
        if self.len < 2 || self.paths.iter().flatten().any(|path| !admitted_path(path)) {
            return None;
        }
        // Repeated aliases of one handle are outside this first product subset;
        // retain the old duplicate-capture rejection rather than selecting one.
        for left in 0..self.len {
            for right in left + 1..self.len {
                if self.paths[left]?.view == self.paths[right]?.view {
                    return None;
                }
            }
        }
        let mut paths = Vec::new();
        paths.try_reserve_exact(self.len).ok()?;
        paths.extend(self.paths.into_iter().flatten());
        Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(Product(
            paths.into(),
        )))
    }

    fn prefix(
        mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        state: &ProjectedCapabilityStateV1,
        place: &SemanticPlaceV1,
        count: usize,
    ) -> Option<Self> {
        if count > MAX_STEPS * 2
            || self.ty()? != function.locals().get(place.local().index() as usize)?.ty()
        {
            return None;
        }
        for projection in place.projections().get(..count)? {
            let ty = self.ty()?;
            let mut next = Self::empty();
            match projection.kind() {
                SemanticProjectionKindV1::Field(selected) => {
                    let output = *fields(types, ty, None)?.get(selected as usize)?;
                    if output != projection.result_type() {
                        return None;
                    }
                    for path in self.paths.into_iter().flatten() {
                        let step = path.steps[0]?;
                        let Kind::Field(field) = step.kind else {
                            return None;
                        };
                        if step.input != ty
                            || fields(types, ty, None)?.get(field as usize) != Some(&step.output)
                        {
                            return None;
                        }
                        if field == selected {
                            next.push(path.tail()?)?;
                        }
                    }
                }
                SemanticProjectionKindV1::Dereference => {
                    let output = projection.result_type();
                    if !is_exact_shared_reference_to_v1(types, ty, output) {
                        return None;
                    }
                    let mut source = None;
                    for path in self.paths.into_iter().flatten() {
                        let step = path.steps[0]?;
                        let Kind::SharedReference(local) = step.kind else {
                            return None;
                        };
                        if step.input != ty
                            || step.output != output
                            || source.is_some_and(|old| old != local)
                        {
                            return None;
                        }
                        source = Some(local);
                        next.push(path.tail()?)?;
                    }
                    let source = source?;
                    if function.locals().get(source.index() as usize)?.ty() != output {
                        return None;
                    }
                    let current =
                        Self::from_value(types, output, state.get(&(source.index() as usize))?)?;
                    // Compare the entire borrowed product before choosing a field.
                    // Rebinding even an unselected handle revokes this snapshot.
                    if current.len != next.len || current.paths != next.paths {
                        return None;
                    }
                }
                _ => return None,
            }
            if next.len == 0 || next.ty()? != projection.result_type() {
                return None;
            }
            self = next;
        }
        Some(self)
    }
}

fn read(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    count: usize,
) -> Option<Selection> {
    let local = place.local().index() as usize;
    let value = state.get(&local)?;
    if matches!(value, ProjectedCapabilityValueV1::CapturedGlobalProduct(_)) {
        Selection::from_value(types, function.locals().get(local)?.ty(), value)?
            .prefix(types, function, state, place, count)
    } else {
        super::read_prefix(types, function, state, place, count).map(Selection::one)
    }
}

pub(super) fn read_single(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    count: usize,
) -> Option<Capture> {
    let local = place.local().index() as usize;
    Selection::from_value(
        types,
        function.locals().get(local)?.ty(),
        state.get(&local)?,
    )?
    .prefix(types, function, state, place, count)?
    .single()
}

pub(super) fn preserves_shared_copy(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> bool {
    let Some(selected) = read(types, function, state, place, place.projections().len()) else {
        return false;
    };
    selected.ty() == Some(place.ty())
        && matches!(types.get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Pointer(pointer))
                if is_exact_shared_reference_to_v1(types, place.ty(), pointer.pointee()))
        && selected
            .paths
            .iter()
            .flatten()
            .all(|path| path.view.borrow == Some(SemanticBorrowKindV1::Shared))
}

pub(super) fn use_value(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    destination: SemanticTypeIdV1,
) -> Option<ProjectedCapabilityValueV1> {
    let selected = read(types, function, state, place, place.projections().len())?;
    if selected.ty()? != place.ty() || place.ty() != destination {
        return None;
    }
    selected.value()
}

pub(super) fn borrow(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    destination: SemanticTypeIdV1,
) -> Option<ProjectedCapabilityValueV1> {
    if !place.projections().is_empty()
        || !is_exact_shared_reference_to_v1(types, destination, place.ty())
    {
        return None;
    }
    let selected = read(types, function, state, place, 0)?;
    if selected.ty()? != place.ty() {
        return None;
    }
    selected
        .prepend(destination, Kind::SharedReference(place.local()))?
        .value()
}

pub(super) fn aggregate(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    destination: SemanticTypeIdV1,
    aggregate: &fe2o3_mir_model::semantic_mir_v1::SemanticAggregateRvalueV1,
) -> Option<ProjectedCapabilityValueV1> {
    let variant = match aggregate.kind() {
        SemanticAggregateKindV1::Aggregate | SemanticAggregateKindV1::Tuple => None,
        SemanticAggregateKindV1::EnumVariant(variant) => Some(*variant),
        _ => return None,
    };
    let field_types = fields(types, destination, variant)?;
    if field_types.len() != aggregate.operands().len() || field_types.len() > MAX_FIELDS {
        return None;
    }
    let mut result = Selection::empty();
    for (field, (ty, operand)) in field_types.iter().zip(aggregate.operands()).enumerate() {
        if *ty != operand.ty() {
            return None;
        }
        let Some(place) = raw_operand_place(operand) else {
            continue;
        };
        let Some(selected) = read(types, function, state, place, place.projections().len()) else {
            // A failed product projection cannot be silently ignored as a scalar sibling.
            if matches!(
                state.get(&(place.local().index() as usize)),
                Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
            ) {
                return None;
            }
            continue;
        };
        if selected.ty()? != place.ty() {
            return None;
        }
        for path in selected.paths.into_iter().flatten() {
            let kind = match variant {
                Some(variant) => Kind::VariantField {
                    variant,
                    field: field as u32,
                },
                None => Kind::Field(field as u32),
            };
            result.push(path.prepend(destination, kind)?)?;
        }
    }
    result.value()
}

pub(super) fn initialized_scalar_sibling(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    field: u32,
) -> bool {
    let Some(selected) = read(types, function, state, place, 1) else {
        return false;
    };
    selected.len > 1 && fields(types, selected.ty().unwrap(), None).and_then(|fields| fields.get(field as usize)) == Some(&place.ty())
        && selected.paths.iter().flatten().all(|path| {
            path.view.borrow == Some(SemanticBorrowKindV1::Shared)
                && matches!(path.steps[0], Some(Step { kind: Kind::Field(captured), .. }) if captured != field)
        })
}

// A work unit is a bounded Capture-path operation (at most MAX_STEPS), as in
// the single-path state. These are logical units, not allocator/hash-table or
// machine-instruction bounds. Charge the product's additional paths on every
// statement read and its bounded pairwise alias check before construction.
fn charge_assignment(
    state: &ProjectedCapabilityStateV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut handles = 0usize;
    let mut charge_place = |place: &SemanticPlaceV1| {
        match state.get(&(place.local().index() as usize)) {
            Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(product)) => {
                handles += product.0.len();
                // Selection, whole-source reauthentication, and consumption
                // each process at most the input product's bounded paths.
                charge_capability_dataflow_work_v1(work, 3 * product.0.len())
            }
            Some(ProjectedCapabilityValueV1::CapturedGlobal(_))
            | Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(_))) =>
            {
                handles += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    };
    match assignment.value().kind() {
        SemanticRvalueKindV1::Aggregate(aggregate) if aggregate.operands().len() <= MAX_FIELDS => {
            for operand in aggregate.operands() {
                if let Some(place) = raw_operand_place(operand) {
                    charge_place(place)?;
                }
            }
            if handles > 1 {
                // Includes rejected oversized/aliasing candidates; this is an
                // upper bound because field selection can discard input paths.
                charge_capability_dataflow_work_v1(work, handles * (handles + 1))?;
            }
        }
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => charge_place(place)?,
        value => value.try_visit_operands(|operand| {
            if let Some(place) = raw_operand_place(operand) {
                charge_place(place)?;
            }
            Ok::<_, ProductionRankedProjectionErrorV1>(())
        })?,
    }
    Ok(())
}

fn charge_operand(
    state: &ProjectedCapabilityStateV1,
    operand: &SemanticOperandV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if let Some(place) = raw_operand_place(operand)
        && let Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(product)) =
            state.get(&(place.local().index() as usize))
    {
        charge_capability_dataflow_work_v1(work, 3 * product.0.len())?;
    }
    Ok(())
}

pub(in super::super) fn charge_statement(
    state: &ProjectedCapabilityStateV1,
    statement: &SemanticStatementKindV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => charge_assignment(state, assignment, work),
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
    let operands = match terminator {
        SemanticTerminatorKindV1::Call(call) => call.arguments(),
        SemanticTerminatorKindV1::TailCall(call) => call.arguments(),
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            std::slice::from_ref(discriminant)
        }
        SemanticTerminatorKindV1::Assert { condition, .. } => std::slice::from_ref(condition),
        _ => &[],
    };
    for operand in operands {
        charge_operand(state, operand, work)?;
    }
    Ok(())
}
