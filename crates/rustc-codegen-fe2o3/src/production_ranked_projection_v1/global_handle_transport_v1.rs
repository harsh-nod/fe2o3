//! Closed transport of authenticated shared Global handles through fields.
//! These facts retain custody only; they neither read elements nor prove memory
//! equivalence. Reference steps must still name the same live stored fact.

use super::*;

#[path = "global_handle_transport_v1/exclusive.rs"]
pub(super) mod exclusive;

#[path = "global_handle_transport_v1/product.rs"]
mod product;
pub(super) use product::Product;
pub(super) use product::{
    charge_statement as charge_product_statement, charge_terminator as charge_product_terminator,
};

#[cfg(test)]
#[path = "global_handle_transport_v1/product_tests.rs"]
mod product_tests;

const MAX_STEPS: usize = 8;
const MAX_FIELDS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Field(u32),
    VariantField { variant: u32, field: u32 },
    SharedReference(SemanticLocalIdV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Step {
    input: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Capture {
    view: ProjectedGlobalViewV1,
    ty: SemanticTypeIdV1,
    steps: [Option<Step>; MAX_STEPS],
    len: u8,
}

impl Capture {
    pub(super) fn is_unconditional(&self) -> bool {
        !self.steps[..usize::from(self.len)]
            .iter()
            .flatten()
            .any(|step| matches!(step.kind, Kind::VariantField { .. }))
    }

    pub(super) fn in_variant(self, ty: SemanticTypeIdV1, variant: u32, field: u32) -> Option<Self> {
        self.prepend(ty, Kind::VariantField { variant, field })
    }

    fn prepend(mut self, input: SemanticTypeIdV1, kind: Kind) -> Option<Self> {
        let len = usize::from(self.len);
        if len == MAX_STEPS {
            return None;
        }
        self.steps.copy_within(..len, 1);
        self.steps[0] = Some(Step {
            input,
            output: self.ty,
            kind,
        });
        self.ty = input;
        self.len += 1;
        Some(self)
    }

    fn tail(mut self) -> Option<Self> {
        let step = self.steps[0]?;
        let len = usize::from(self.len);
        self.steps.copy_within(1..len, 0);
        self.steps[len - 1] = None;
        self.len -= 1;
        self.ty = step.output;
        Some(self)
    }

    fn value(self) -> ProjectedCapabilityValueV1 {
        if self.len == 0 {
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(self.view))
        } else {
            ProjectedCapabilityValueV1::CapturedGlobal(self)
        }
    }
}

fn from_value(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    value: &ProjectedCapabilityValueV1,
) -> Option<Capture> {
    match value {
        ProjectedCapabilityValueV1::CapturedGlobal(capture) if capture.ty == ty => Some(*capture),
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))
            if view.borrow == Some(SemanticBorrowKindV1::Shared)
                && is_exact_shared_reference_to_v1(types, ty, view.view) =>
        {
            Some(Capture {
                view: *view,
                ty,
                steps: [None; MAX_STEPS],
                len: 0,
            })
        }
        _ => None,
    }
}

fn fields(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    variant: Option<u32>,
) -> Option<&[SemanticTypeIdV1]> {
    match (types.get(ty.index() as usize)?.shape(), variant) {
        (SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields), None) => {
            Some(fields.fields())
        }
        (SemanticTypeShapeV1::Enum { variants, .. }, Some(variant)) => {
            let variant = variants.get(variant as usize)?;
            (!variant.is_uninhabited()).then_some(variant.fields().fields())
        }
        _ => None,
    }
}

fn read_prefix(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    count: usize,
) -> Option<Capture> {
    let local = place.local().index() as usize;
    if matches!(
        state.get(&local),
        Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
    ) {
        return product::read_single(types, function, state, place, count);
    }
    let capture = from_value(
        types,
        function.locals().get(local)?.ty(),
        state.get(&local)?,
    )?;
    read_capture_prefix(types, function, state, place, count, capture)
}

fn read_capture_prefix(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    count: usize,
    mut capture: Capture,
) -> Option<Capture> {
    if count > MAX_STEPS * 2
        || function.locals().get(place.local().index() as usize)?.ty() != capture.ty
    {
        return None;
    }
    let mut projections = place.projections().get(..count)?.iter();
    while let Some(projection) = projections.next() {
        let step = capture.steps[0]?;
        if step.input != capture.ty {
            return None;
        }
        let tail = capture.tail()?;
        let last = match step.kind {
            Kind::Field(field) => {
                if projection.kind() != SemanticProjectionKindV1::Field(field)
                    || fields(types, step.input, None)?.get(field as usize) != Some(&step.output)
                {
                    return None;
                }
                projection
            }
            Kind::VariantField { variant, field } => {
                if projection.kind() != SemanticProjectionKindV1::Downcast(variant)
                    || projection.result_type() != step.input
                    || fields(types, step.input, Some(variant))?.get(field as usize)
                        != Some(&step.output)
                {
                    return None;
                }
                let field_projection = projections.next()?;
                if field_projection.kind() != SemanticProjectionKindV1::Field(field) {
                    return None;
                }
                field_projection
            }
            Kind::SharedReference(source) => {
                if projection.kind() != SemanticProjectionKindV1::Dereference
                    || !is_exact_shared_reference_to_v1(types, step.input, step.output)
                    || function.locals().get(source.index() as usize)?.ty() != step.output
                    || from_value(types, step.output, state.get(&(source.index() as usize))?)?
                        != tail
                {
                    return None;
                }
                projection
            }
        };
        if last.result_type() != step.output {
            return None;
        }
        capture = tail;
    }
    Some(capture)
}

fn read(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> Option<Capture> {
    let capture = read_prefix(types, function, state, place, place.projections().len())?;
    (capture.ty == place.ty()).then_some(capture)
}

pub(super) fn capture_operand(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> Option<Capture> {
    read(types, function, state, place)
}

pub(super) fn read_conditional_payload(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
    conditions: &global_enum_transport_v1::Conditions<'_>,
    block: usize,
) -> Option<ProjectedCapabilityValueV1> {
    let ProjectedCapabilityValueV1::GlobalEnum(capture) =
        state.get(&(place.local().index() as usize))?
    else {
        return None;
    };
    let capture = capture.active_payload(types, function, place, conditions, block)?;
    let capture = read_capture_prefix(
        types,
        function,
        state,
        place,
        place.projections().len(),
        capture,
    )?;
    (capture.ty == place.ty()).then(|| capture.value())
}

pub(super) fn preserves_initialized_scalar_copy(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
    reads: Option<&private_scalar_capture_v1::PrivateScalarReads<'_>>,
) -> bool {
    let Some(reads) = reads else {
        return false;
    };
    if reads.conditions_for(types, function).is_none() || !reads.contains(function, statement) {
        return false;
    }
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return false;
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        return false;
    };
    let [dereference, field] = place.projections() else {
        return false;
    };
    let SemanticProjectionKindV1::Field(field_index) = field.kind() else {
        return false;
    };
    if dereference.kind() != SemanticProjectionKindV1::Dereference
        || !matches!(
            types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_))
        )
    {
        return false;
    }
    if matches!(
        state.get(&(place.local().index() as usize)),
        Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
    ) {
        return field.result_type() == place.ty()
            && product::initialized_scalar_sibling(types, function, state, place, field_index);
    }
    let Some(capture) = read_prefix(types, function, state, place, 1) else {
        return false;
    };
    let Some(Step {
        kind: Kind::Field(captured_field),
        ..
    }) = capture.steps[0]
    else {
        return false;
    };
    capture.view.borrow == Some(SemanticBorrowKindV1::Shared)
        && captured_field != field_index
        && fields(types, capture.ty, None).and_then(|fields| fields.get(field_index as usize))
            == Some(&place.ty())
        && field.result_type() == place.ty()
}

pub(super) fn preserves_projected_copy(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> bool {
    if matches!(
        state.get(&(place.local().index() as usize)),
        Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
    ) {
        return product::preserves_shared_copy(types, function, state, place);
    }
    if place.projections().is_empty() {
        return false;
    }
    let Some(capture) = read(types, function, state, place) else {
        return false;
    };
    // A captured aggregate can contain non-Copy fields. Only the current shared
    // reference value is duplicable; custody for its surrounding owner is not.
    matches!(
        types.get(capture.ty.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if capture.view.borrow == Some(SemanticBorrowKindV1::Shared)
                && is_exact_shared_reference_to_v1(types, capture.ty, pointer.pointee())
    )
}

pub(super) fn assignment(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> Option<ProjectedCapabilityValueV1> {
    let destination = assignment.destination();
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
    let capture = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(operand) => {
            let place = raw_operand_place(operand)?;
            if matches!(
                state.get(&(place.local().index() as usize)),
                Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
            ) {
                return product::use_value(types, function, state, place, destination.ty());
            }
            read(types, function, state, raw_operand_place(operand)?)?
        }
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } if place.projections().is_empty()
            && is_exact_shared_reference_to_v1(types, destination.ty(), place.ty()) =>
        {
            if matches!(
                state.get(&(place.local().index() as usize)),
                Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
            ) {
                return product::borrow(types, function, state, place, destination.ty());
            }
            read(types, function, state, place)?
                .prepend(destination.ty(), Kind::SharedReference(place.local()))?
        }
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } => {
            let last = place.projections().last()?;
            let capture =
                read_prefix(types, function, state, place, place.projections().len() - 1)?;
            if capture.len != 0
                || last.kind() != SemanticProjectionKindV1::Dereference
                || last.result_type() != capture.view.view
                || place.ty() != capture.view.view
                || destination.ty() != capture.ty
            {
                return None;
            }
            capture
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            return product::aggregate(types, function, state, destination.ty(), aggregate);
        }
        _ => return None,
    };
    (capture.ty == destination.ty()).then(|| capture.value())
}

pub(super) fn metadata_observation(
    value: &SemanticRvalueKindV1,
    origin: Option<ProjectedCapabilityValueV1>,
) -> bool {
    matches!(
        value,
        SemanticRvalueKindV1::Use(_)
            | SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                ..
            }
    ) && matches!(
        origin,
        Some(ProjectedCapabilityValueV1::CapturedGlobal(_))
            | Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
            | Some(ProjectedCapabilityValueV1::GlobalEnum(_))
            | Some(ProjectedCapabilityValueV1::Known(
                ProjectedCapabilityOriginV1::GlobalView(ProjectedGlobalViewV1 {
                    borrow: Some(SemanticBorrowKindV1::Shared),
                    ..
                })
            ))
    )
}

pub(super) fn invalidates_reference_storage(
    state: &ProjectedCapabilityStateV1,
    place: &SemanticPlaceV1,
) -> bool {
    match state.get(&(place.local().index() as usize)) {
        Some(ProjectedCapabilityValueV1::CapturedGlobal(_))
        | Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
        | Some(ProjectedCapabilityValueV1::GlobalEnum(_)) => true,
        Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))) => {
            place.projections().is_empty() && view.borrow == Some(SemanticBorrowKindV1::Shared)
        }
        _ => false,
    }
}
