//! Exact retained environment transfer and field read in one checked expansion.
//! This is correspondence, not a loan proof. It allocates no graph or roster.
use super::*;
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedRootV1,
    SemanticExpandedTerminatorOriginV1 as TerminatorOrigin,
};

pub(super) struct Request<'a> {
    pub closure_source: &'a SemanticFunctionDeclV1,
    pub wrapper: SemanticCallInstanceIdV1,
    pub closure: SemanticCallInstanceIdV1,
    pub invoke_block: SemanticBlockIdV1,
    pub environment: SemanticLocalIdV1,
    pub environment_value: SsaValueV1,
    pub field: u32,
    pub destination: &'a SemanticPlaceV1,
}

pub(super) fn field_value(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    request: Request<'_>,
    work: &mut usize,
) -> PhaseResult<SsaValueV1> {
    spend(work, 12)?;
    let frame = view
        .instances()
        .get(request.closure.index() as usize)
        .ok_or_else(|| rejected("phase capture has no exact closure frame"))?;
    let environment = query
        .function()
        .locals()
        .get(request.environment.index() as usize)
        .ok_or_else(|| rejected("phase capture environment is outside its source body"))?;
    let environment_origin = view
        .local_origins()
        .get(request.environment.index() as usize)
        .ok_or_else(|| rejected("phase capture environment has no original owner"))?;
    let destination_origin = view
        .local_origins()
        .get(request.destination.local().index() as usize)
        .ok_or_else(|| rejected("phase capture destination has no original owner"))?;
    if !std::ptr::eq(query.function(), view.body())
        || frame.function_identity() != request.closure_source.identity()
        || frame.parent() != Some(request.wrapper)
        || frame.call_block() != Some(request.invoke_block)
        || environment_origin.instance() != request.wrapper
        || destination_origin.instance() != request.closure
        || destination_origin.function() != frame.function()
        || !request.destination.projections().is_empty()
        || request.closure_source.abi().extern_abi() != SemanticExternAbiV1::RustCall
        || request.closure_source.abi().source_input_types().len() != 2
        || request.closure_source.abi().source_input_types()[0] != environment.ty()
        || request
            .closure_source
            .abi()
            .source_argument_ownership()
            .first()
            != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
    {
        return Err(rejected(
            "phase capture substituted its closure, caller, ABI or local owner",
        ));
    }
    let call_block =
        phase_argument::mapped_block(view, request.wrapper, request.invoke_block, work)?;
    let call_origin = &view.block_origins()[call_block.index() as usize];
    if call_origin.terminator()
        != (TerminatorOrigin::CallEntry {
            callee: request.closure,
        })
    {
        return Err(rejected("phase capture lost its exact closure call entry"));
    }
    let mut parameter = None;
    for (statement, marker) in call_origin.statements().iter().enumerate() {
        spend(work, 1)?;
        if *marker
            != (Origin::ParameterTransfer {
                callee: request.closure,
                argument: 0,
            })
        {
            continue;
        }
        let site = Site::new(call_block, Some(statement as u32));
        let a = phase_argument::assignment(query, site)?;
        let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
            return Err(rejected(
                "phase capture parameter is not its retained transfer",
            ));
        };
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(rejected(
                "phase capture parameter cannot originate in a constant",
            ));
        };
        let origin = view
            .local_origins()
            .get(a.destination().local().index() as usize)
            .ok_or_else(|| rejected("phase capture parameter left its original frame"))?;
        let original = request
            .closure_source
            .locals()
            .get(origin.local().index() as usize)
            .ok_or_else(|| rejected("phase capture parameter has no source declaration"))?;
        if place.local() != request.environment
            || !place.projections().is_empty()
            || place.ty() != environment.ty()
            || a.destination().ty() != environment.ty()
            || !a.destination().projections().is_empty()
            || origin.instance() != request.closure
            || origin.function() != frame.function()
            || original.role() != SemanticLocalRoleV1::Argument(0)
            || original.ty() != environment.ty()
        {
            return Err(rejected(
                "phase capture parameter substituted the exact environment",
            ));
        }
        let used = query
            .operand_use(site, operand, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase capture transfer has no exact promoted source use"))?;
        if !used.belongs_to(query)
            || used.value() != request.environment_value
            || (matches!(operand, SemanticOperandV1::Move(_))
                && value_at(
                    query,
                    site,
                    SsaVariableIdV1::new(place.local().index()),
                    Event::Kill,
                    work,
                )? != used.value())
        {
            return Err(rejected(
                "phase capture parameter changed its environment SSA identity",
            ));
        }
        let value = value_at(
            query,
            site,
            SsaVariableIdV1::new(a.destination().local().index()),
            Event::Define,
            work,
        )?;
        if parameter
            .replace((a.destination().local(), value))
            .is_some()
        {
            return Err(rejected("phase capture parameter transfer was duplicated"));
        }
    }
    let (parameter, parameter_value) = parameter
        .ok_or_else(|| rejected("phase capture lost its closure environment parameter"))?;
    let mut result = None;
    for (block, origin) in view.block_origins().iter().enumerate() {
        spend(work, 1)?;
        if origin.instance() != request.closure {
            continue;
        }
        if origin.function() != frame.function() {
            return Err(rejected("phase capture read changed its original function"));
        }
        for (statement, marker) in origin.statements().iter().enumerate() {
            spend(work, 1)?;
            if !matches!(marker, Origin::Source { .. }) {
                continue;
            }
            let item = &query.function().blocks()[block].statements()[statement];
            let SemanticStatementKindV1::Assign(a) = item.kind() else {
                continue;
            };
            if a.destination() != request.destination {
                continue;
            }
            let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
                return Err(rejected(
                    "phase capture read was replaced by another definition",
                ));
            };
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Err(rejected(
                    "phase capture read cannot originate in a constant",
                ));
            };
            if place.local() != parameter
                || place.ty() != request.destination.ty()
                || !matches!(place.projections(), [field]
                    if field.kind() == SemanticProjectionKindV1::Field(request.field)
                        && field.result_type() == request.destination.ty())
                || a.value().result_type() != request.destination.ty()
            {
                return Err(rejected(
                    "phase capture read substituted the selected environment field",
                ));
            }
            let site = Site::new(
                SemanticBlockIdV1::from_index(block as u32),
                Some(statement as u32),
            );
            // The public operand query intentionally accepts whole locals only.
            // This exact retained field read uses the aggregate's existing Use;
            // it does not create a separate field SSA value or a partial Kill.
            if value_at(
                query,
                site,
                SsaVariableIdV1::new(parameter.index()),
                Event::Use,
                work,
            )? != parameter_value
            {
                return Err(rejected(
                    "phase capture read changed its environment SSA identity",
                ));
            }
            let value = value_at(
                query,
                site,
                SsaVariableIdV1::new(request.destination.local().index()),
                Event::Define,
                work,
            )?;
            if result.replace(value).is_some() {
                return Err(rejected(
                    "phase capture destination was defined more than once",
                ));
            }
        }
    }
    result.ok_or_else(|| rejected("phase capture has no exact retained field read"))
}
