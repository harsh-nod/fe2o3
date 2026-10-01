//! Exact original immutable witness loans; scalar payloads do not imply pointers.

use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceReferenceCarrierV38 as Carrier;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBorrowKindV1 as BorrowKind, SemanticDisjointIndexSpaceV1 as IndexSpace,
};
use fe2o3_mir_model::{SsaResolvedEventV1 as SsaEvent, SsaValueV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as EventRole, ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Borrow {
    destination: usize,
    origin: usize,
    source_type: u32,
    generation: u32,
    instance: usize,
    block: usize,
    statement: usize,
}

fn key(site: Site) -> [u32; 2] {
    match site {
        Site::Statement { block, statement } => [block.get(), statement],
        Site::Terminator { block } => [block.get(), u32::MAX],
    }
}

pub(in super::super) fn original_value(
    slots: &SourceSlots<'_, '_>,
    function: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    block: usize,
    statement: Option<usize>,
    operand: OperandRole,
    local: u32,
    define: bool,
    out: &mut Writer<'_, '_>,
) -> Result<SsaValueV1> {
    out.budget.charge_work(5)?;
    let source = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_ssa(out.budget)?;
    let occurrences = source
        .occurrences_v1()
        .and_then(|rows| rows.function(function))
        .ok_or_else(mismatch)?;
    let events = occurrences.events();
    let selected = [
        u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
        statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| Resource::Arithmetic)?
            .unwrap_or(u32::MAX),
    ];
    let (mut lo, mut hi) = (0, events.len());
    while lo < hi {
        out.budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        if key(events[middle].site()) < selected {
            lo = middle + 1
        } else {
            hi = middle
        }
    }
    let role = if define {
        EventRole::DestinationDefine
    } else {
        EventRole::BaseUse
    };
    let mut result = None;
    for event in &events[lo..] {
        out.budget.charge_work(8)?;
        if key(event.site()) != selected {
            break;
        }
        // Authenticated transparent borrow elision keeps the destination Define
        // under its dedicated role; the caller still checks the exact loan/site.
        let selected_operand = event.operand() == operand
            || (define
                && operand == OperandRole::Destination
                && event.operand() == OperandRole::ElidedBorrowDestination);
        if !selected_operand || event.role() != role {
            continue;
        }
        let (variable, value) = match (define, event.resolved()) {
            (true, Some(SsaEvent::Define { variable, value }))
            | (false, Some(SsaEvent::Use { variable, value })) => (variable, value),
            _ => return Err(mismatch()),
        };
        if variable.get() != local
            || !event.is_promoted()
            || !event.is_reachable()
            || result.replace(value).is_some()
        {
            return Err(mismatch());
        }
    }
    result.ok_or_else(mismatch)
}

impl Borrow {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        function: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
        block: usize,
        statement: usize,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(8)?;
        let Rvalue::Borrow {
            kind: BorrowKind::Shared,
            place,
        } = assignment.value().kind()
        else {
            return Ok(None);
        };
        let Some((_, _, _, IndexSpace::Index1d)) = context.slots.witness_class(place.ty(), out)?
        else {
            return Ok(None);
        };
        let destination = assignment.destination();
        if !place.projections().is_empty()
            || !destination.projections().is_empty()
            || destination.ty() != assignment.value().result_type()
            || context
                .slots
                .legacy_descriptor_by_source(
                    context.root,
                    context.instance,
                    place.local().index(),
                    out,
                )?
                .is_some()
            || context
                .slots
                .legacy_descriptor_by_source(
                    context.root,
                    context.instance,
                    destination.local().index(),
                    out,
                )?
                .is_some()
        {
            return Err(unsupported());
        }
        let value = original_value(
            context.slots,
            function,
            block,
            Some(statement),
            OperandRole::Destination,
            destination.local().index(),
            true,
            out,
        )?;
        let relation = context.slots.correspondence(out)?;
        let endpoint =
            relation.ssa_typed_endpoint_v36(context.root, context.instance, value, out.budget)?;
        let reference = endpoint.reference(out.budget)?.ok_or_else(mismatch)?;
        if endpoint.source_type(out.budget)? != destination.ty()
            || endpoint.source_local(out.budget)? != destination.local()
            || endpoint.source_function(out.budget)? != function
            || reference.carrier(out.budget)? != Carrier::StableScalar
            || reference.origin_instance(out.budget)? != context.instance
            || reference.origin_function(out.budget)? != function
            || reference.origin_local(out.budget)? != place.local()
            || reference.origin_type(out.budget)? != place.ty()
            || reference.borrow_site(out.budget)?
                != (
                    context.instance,
                    fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                        u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
                    ),
                    Some(statement),
                )
        {
            return Err(mismatch());
        }
        Ok(Some(Self {
            destination: context.local(destination.local().index())?,
            origin: context.local(place.local().index())?,
            source_type: place.ty().index(),
            generation: reference.origin_generation(out.budget)?,
            instance: context.instance,
            block,
            statement,
        }))
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::WitnessBorrow {{ destination: {}, origin: {}, source_type: {}, generation: {}, instance: {}, block: {}, statement: {} }}",
            self.destination, self.origin, self.source_type, self.generation, self.instance,
            self.block, self.statement).map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<Borrow>()
        + size_of::<Result<Option<Borrow>>>()
        + size_of::<SsaValueV1>()
        + size_of::<Result<SsaValueV1>>()
        + size_of::<[u32; 2]>() * 2
        + size_of::<Option<SsaValueV1>>()
        + size_of::<Site>()
        + size_of::<OperandRole>()
        + size_of::<EventRole>()
        + size_of::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + size_of::<Result<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>>()
        + 18 * size_of::<usize>()
        + 20 * size_of::<&()>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>()
        + size_of::<
            Result<Option<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>>,
        >()
}
