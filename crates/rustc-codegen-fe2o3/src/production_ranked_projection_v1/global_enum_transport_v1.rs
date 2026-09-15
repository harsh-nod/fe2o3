//! Bounded conditional custody for one shared Global, never an unconditional
//! origin. Variant absence is retained only from an explicit enum constructor.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticEnumVariantV1;

const MAX_VARIANTS: usize = 2;
const MAX_FIELDS: usize = 16;
const MAX_DEPTH: usize = 8;
const MAX_TYPE_NODES: usize = MAX_FIELDS * MAX_DEPTH;

pub(super) struct Conditions<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    dominance: SemanticEnumPayloadDominanceV1,
}

impl<'a> Conditions<'a> {
    pub(super) fn new(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Self, ()> {
        Self::analyze(types, function).map_err(|_| ())
    }

    pub(super) fn analyze(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Self, fe2o3_mir_model::SemanticOptionDominanceErrorV1> {
        Ok(Self {
            types,
            function,
            dominance: SemanticEnumPayloadDominanceV1::analyze(function, types)?,
        })
    }

    pub(super) fn dominance(&self) -> &SemanticEnumPayloadDominanceV1 {
        &self.dominance
    }

    pub(super) fn into_dominance(self) -> SemanticEnumPayloadDominanceV1 {
        self.dominance
    }

    pub(super) fn work_units(&self) -> usize {
        self.dominance.work_units()
    }

    pub(super) fn matches(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> bool {
        std::ptr::eq(self.types, types) && std::ptr::eq(self.function, function)
    }

    pub(super) fn allows(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        local: SemanticLocalIdV1,
        variant: u32,
        block: usize,
    ) -> bool {
        self.matches(types, function)
            && u32::try_from(block).ok().is_some_and(|block| {
                self.dominance
                    .availability(local, variant)
                    .is_some_and(|availability| {
                        self.dominance
                            .allows(availability, SemanticBlockIdV1::from_index(block))
                    })
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Capture {
    ty: SemanticTypeIdV1,
    possible: u8,
    // At most one variant carries a Global. Different simultaneous fields and
    // different captured variants remain outside this transport subset.
    payload: Option<(u8, global_handle_transport_v1::Capture)>,
}

impl Capture {
    pub(super) fn active_payload(
        self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        place: &SemanticPlaceV1,
        conditions: &Conditions<'_>,
        block: usize,
    ) -> Option<global_handle_transport_v1::Capture> {
        let (variant, payload) = self.payload?;
        if function.locals().get(place.local().index() as usize)?.ty() != self.ty
            || self.possible & (1 << variant) == 0
            || place.projections().first()?.kind()
                != SemanticProjectionKindV1::Downcast(u32::from(variant))
            || !conditions.allows(types, function, place.local(), u32::from(variant), block)
        {
            return None;
        }
        Some(payload)
    }

    pub(super) fn merge(self, incoming: Self) -> Option<Self> {
        if self.ty != incoming.ty || self.possible == 0 || incoming.possible == 0 {
            return None;
        }
        let payload = match (self.payload, incoming.payload) {
            (Some(left), Some(right)) if left == right => Some(left),
            (None, None) => None,
            (Some((variant, capture)), None) if incoming.possible & (1 << variant) == 0 => {
                Some((variant, capture))
            }
            (None, Some((variant, capture))) if self.possible & (1 << variant) == 0 => {
                Some((variant, capture))
            }
            // An unknown/different payload on the same variant is not absence
            // of that variant. Never recover it from another predecessor.
            _ => return None,
        };
        Some(Self {
            ty: self.ty,
            possible: self.possible | incoming.possible,
            payload,
        })
    }
}

// This is only a duplicability/shape filter for the inert carrier, never
// initialization or Global authority. Those require independent live facts.
fn shared_copy_shape(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    depth: usize,
    remaining: &mut usize,
) -> Option<bool> {
    *remaining = remaining.checked_sub(1)?;
    if depth == MAX_DEPTH {
        return None;
    }
    let declaration = types.get(ty.index() as usize)?;
    if declaration.layout().is_uninhabited() {
        return None;
    }
    match declaration.shape() {
        SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Scalar(_) => Some(false),
        SemanticTypeShapeV1::Pointer(pointer)
            if is_exact_shared_reference_to_v1(types, ty, pointer.pointee()) =>
        {
            Some(true)
        }
        SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
            shared_copy_fields(types, fields.fields(), depth, remaining)
        }
        SemanticTypeShapeV1::Enum { variants, .. } if variants.len() <= MAX_FIELDS => {
            let mut shared = false;
            for variant in variants.iter().filter(|variant| !variant.is_uninhabited()) {
                shared |= shared_copy_fields(types, variant.fields().fields(), depth, remaining)?;
            }
            Some(shared)
        }
        _ => None,
    }
}

fn shared_copy_fields(
    types: &[SemanticTypeDeclV1],
    fields: &[SemanticTypeIdV1],
    depth: usize,
    remaining: &mut usize,
) -> Option<bool> {
    if fields.len() > MAX_FIELDS {
        return None;
    }
    let mut shared = false;
    for field in fields {
        shared |= shared_copy_shape(types, *field, depth + 1, remaining)?;
    }
    Some(shared)
}

fn variants(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<&[SemanticEnumVariantV1]> {
    let declaration = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return None;
    };
    let mut remaining = MAX_TYPE_NODES;
    (variants.len() == MAX_VARIANTS
        && variants.iter().all(|variant| !variant.is_uninhabited())
        && shared_copy_shape(types, ty, 0, &mut remaining) == Some(true))
    .then_some(variants)
}

pub(super) fn assignment(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    state: &ProjectedCapabilityStateV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    block: usize,
    statement: usize,
    conditions: Option<&Conditions<'_>>,
) -> Option<ProjectedCapabilityValueV1> {
    let conditions = conditions.filter(|conditions| conditions.matches(types, function))?;
    let SemanticStatementKindV1::Assign(retained) = function
        .blocks()
        .get(block)?
        .statements()
        .get(statement)?
        .kind()
    else {
        return None;
    };
    if !std::ptr::eq(retained, assignment) {
        return None;
    }
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
    match assignment.value().kind() {
        SemanticRvalueKindV1::Use(operand) => {
            let place = raw_operand_place(operand)?;
            let ProjectedCapabilityValueV1::GlobalEnum(capture) =
                state.get(&(place.local().index() as usize))?
            else {
                return None;
            };
            if function.locals().get(place.local().index() as usize)?.ty() != capture.ty {
                return None;
            }
            if place.projections().is_empty() {
                return (place.ty() == capture.ty && destination.ty() == capture.ty)
                    .then_some(ProjectedCapabilityValueV1::GlobalEnum(*capture));
            }
            global_handle_transport_v1::read_conditional_payload(
                types, function, state, place, conditions, block,
            )
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            let SemanticAggregateKindV1::EnumVariant(variant) = *aggregate.kind() else {
                return None;
            };
            let alternatives = variants(types, destination.ty())?;
            let fields = alternatives.get(variant as usize)?.fields().fields();
            if fields.len() != aggregate.operands().len() {
                return None;
            }
            let mut payload = None;
            for (field, (ty, operand)) in fields.iter().zip(aggregate.operands()).enumerate() {
                if *ty != operand.ty() {
                    return None;
                }
                let Some(place) = raw_operand_place(operand) else {
                    continue;
                };
                let candidate =
                    global_handle_transport_v1::capture_operand(types, function, state, place);
                if let Some(candidate) = candidate {
                    if payload.is_some() || !candidate.is_unconditional() {
                        return None;
                    }
                    payload = Some((
                        variant as u8,
                        candidate.in_variant(destination.ty(), variant, field as u32)?,
                    ));
                } else if state.contains_key(&(place.local().index() as usize)) {
                    // Do not intercept scalar load results, other capabilities,
                    // or a nested conditional payload that this domain cannot hold.
                    return None;
                }
            }
            Some(ProjectedCapabilityValueV1::GlobalEnum(Capture {
                ty: destination.ty(),
                possible: 1 << variant,
                payload,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "global_enum_transport_v1/tests.rs"]
pub(super) mod tests;
