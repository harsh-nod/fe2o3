//! Failure-only source observations. Nothing returned here is proof authority.
use super::*;
use fe2o3_mir_model::{SsaValueV1, semantic_mir_v1::SemanticAssignmentV1};
use fe2o3_pliron::{
    ProductionSemanticSsaSourceSiteV1 as Site, ProductionSemanticSsaValueOriginV1 as Origin,
};
use std::io::{self, Write};

// This limits diagnostic reads only, never the proof/SSA owner's resources.
const MAX_QUERY_STEPS: usize = 128;
const MAX_PROJECTIONS: usize = 4;

pub(super) fn observe(
    error: &ProductionRankedProjectionErrorV1,
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    site: ProjectedSemanticAccessSiteV1,
    contracts: &ProjectionLocalContractsV1,
) {
    if !matches!(
        error,
        ProductionRankedProjectionErrorV1::MissingAllocationProvenance { .. }
    ) {
        return;
    }
    let enabled =
        index_rejection_v1::enabled(std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").as_deref());
    if !enabled {
        return;
    }
    // Preserve the original error even if stderr cannot accept the observation.
    let _ = write(
        error,
        owner,
        root,
        function,
        site,
        contracts,
        enabled,
        MAX_QUERY_STEPS,
        &mut std::io::stderr().lock(),
    );
}

#[allow(clippy::too_many_arguments)]
fn write(
    error: &ProductionRankedProjectionErrorV1,
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    site: ProjectedSemanticAccessSiteV1,
    contracts: &ProjectionLocalContractsV1,
    enabled: bool,
    query_steps: usize,
    out: &mut impl Write,
) -> io::Result<()> {
    if !enabled {
        return Ok(());
    }
    let ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
        local,
        ty,
        projections,
    } = error
    else {
        return Ok(());
    };
    let Ok(query) = owner.source_query_for_root(root, function) else {
        return Ok(());
    };
    let Some(view) = owner.execution_view_for_root(root) else {
        return Ok(());
    };
    let Some(statement_index) = site.statement else {
        return Ok(());
    };
    let Some(statement) = function
        .blocks()
        .get(site.block)
        .and_then(|b| b.statements().get(statement_index))
    else {
        return Ok(());
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(());
    };
    let (place, borrowed) = match assignment.value().kind() {
        SemanticRvalueKindV1::Borrow { place, .. } => (place, true),
        SemanticRvalueKindV1::AddressOf { place, .. } => (place, false),
        _ => return Ok(()),
    };
    if place.local().index() != *local
        || place.projections().len() != *projections
        || function
            .locals()
            .get(*local as usize)
            .map(|l| l.ty().index())
            != Some(*ty)
        || place.projections().first().map(|p| p.kind())
            != Some(SemanticProjectionKindV1::Dereference)
    {
        return Ok(());
    }
    let (Ok(block), Ok(index)) = (u32::try_from(site.block), u32::try_from(statement_index)) else {
        return Ok(());
    };
    let origin = view.block_origins().get(site.block);
    write!(
        out,
        "capability-address-rejection root={} body=",
        root.index()
    )?;
    for byte in function.identity().as_bytes() {
        write!(out, "{byte:02x}")?;
    }
    write!(out, " view=")?;
    for byte in view.identity() {
        write!(out, "{byte:02x}")?;
    }
    writeln!(
        out,
        " site=bb{block}:s{index} original_block={:?} original_statement={:?} original_local={:?} source={:?}",
        origin.map(|o| (
            o.instance().index(),
            o.function().index(),
            o.block().index()
        )),
        origin.and_then(|o| o.statements().get(statement_index)),
        view.local_origins().get(*local as usize),
        statement.source()
    )?;
    write!(out, "capability-address-place declared_type={ty} ")?;
    describe_assignment(out, assignment)?;
    write!(
        out,
        " allocation={:?} provenance={:?} checked={:?}",
        contracts
            .allocations
            .get(*local as usize)
            .copied()
            .flatten(),
        contracts
            .allocation_provenance
            .get(*local as usize)
            .copied()
            .flatten(),
        contracts
            .checked_references
            .origins
            .get(*local as usize)
            .copied()
            .flatten()
    )?;
    if let Some(SemanticTypeShapeV1::Pointer(pointer)) = owner
        .source_semantic()
        .types()
        .get(*ty as usize)
        .map(|ty| ty.shape())
    {
        write!(
            out,
            " pointer_kind={:?} pointee={} address_space={} mutability={:?}",
            pointer.kind(),
            pointer.pointee().index(),
            pointer.address_space(),
            pointer.mutability()
        )?;
    }
    writeln!(out)?;
    if !borrowed {
        return writeln!(out, "capability-address-use query=not-a-Borrow steps=0");
    }
    let mut remaining = query_steps.min(MAX_QUERY_STEPS);
    let mut charge = || match remaining.checked_sub(1) {
        Some(next) => {
            remaining = next;
            true
        }
        None => false,
    };
    let value = query.borrow_place_use(
        Site::new(SemanticBlockIdV1::from_index(block), Some(index)),
        place,
        &mut charge,
    );
    writeln!(out, "capability-address-use original_borrow={value:?}")?;
    if let Ok(SsaValueV1::Definition(id)) = value {
        write!(out, "capability-address-definition ")?;
        match query.definition_origin(id, &mut charge) {
            Ok((variable, origin)) => {
                write!(out, "variable={variable:?} ")?;
                match origin {
                    Origin::Entry { argument } => {
                        writeln!(out, "origin=Entry argument={argument}")?
                    }
                    Origin::Event { block, event, site } => {
                        let definition_origin =
                            view.block_origins().get(site.block().index() as usize);
                        write!(
                            out,
                            "origin=Event block={block:?} event={event} site={site:?} original_block={:?} original_statement={:?}",
                            definition_origin.map(|o| (
                                o.instance().index(),
                                o.function().index(),
                                o.block().index()
                            )),
                            site.statement()
                                .and_then(|s| definition_origin?.statements().get(s as usize)),
                        )?;
                        if let Some(statement) = site.statement().and_then(|s| {
                            function
                                .blocks()
                                .get(site.block().index() as usize)?
                                .statements()
                                .get(s as usize)
                        }) {
                            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                                write!(out, " ")?;
                                describe_assignment(out, assignment)?;
                            }
                        }
                        writeln!(out)?;
                    }
                    Origin::Edge { edge, definition } => writeln!(
                        out,
                        "origin=Edge edge={:?} role={:?} definition={definition}",
                        edge.id(),
                        edge.role()
                    )?,
                    Origin::BlockArgument(incoming) => writeln!(
                        out,
                        "origin=BlockArgument block={:?} edges={} external_entry={}",
                        incoming.block(),
                        incoming.edge_count(),
                        incoming.external_entry().is_some()
                    )?,
                }
            }
            Err(error) => writeln!(out, "query={error:?}")?,
        }
    }
    writeln!(
        out,
        "capability-address-observation steps={} remaining={remaining} proof_authority=false",
        query_steps.min(MAX_QUERY_STEPS) - remaining
    )
}

fn describe_place(out: &mut impl Write, place: &SemanticPlaceV1) -> io::Result<()> {
    write!(
        out,
        "(local={},type={},projections={:?},projection_count={})",
        place.local().index(),
        place.ty().index(),
        &place.projections()[..place.projections().len().min(MAX_PROJECTIONS)],
        place.projections().len()
    )
}

fn describe_operand(out: &mut impl Write, value: &SemanticOperandV1) -> io::Result<()> {
    match value {
        SemanticOperandV1::Copy(place) => {
            write!(out, "Copy")?;
            describe_place(out, place)
        }
        SemanticOperandV1::Move(place) => {
            write!(out, "Move")?;
            describe_place(out, place)
        }
        SemanticOperandV1::Constant(_) => write!(out, "Constant(type={})", value.ty().index()),
    }
}

fn describe_assignment(out: &mut impl Write, assignment: &SemanticAssignmentV1) -> io::Result<()> {
    write!(out, "destination=")?;
    describe_place(out, assignment.destination())?;
    write!(
        out,
        " result_type={} ",
        assignment.value().result_type().index()
    )?;
    match assignment.value().kind() {
        SemanticRvalueKindV1::Use(value) => {
            write!(out, "rvalue=Use operand=")?;
            describe_operand(out, value)
        }
        SemanticRvalueKindV1::Borrow { kind, place } => {
            write!(out, "rvalue=Borrow kind={kind:?} place=")?;
            describe_place(out, place)
        }
        SemanticRvalueKindV1::AddressOf { mutability, place } => {
            write!(out, "rvalue=AddressOf mutability={mutability:?} place=")?;
            describe_place(out, place)
        }
        SemanticRvalueKindV1::Aggregate(value) => {
            write!(
                out,
                "rvalue=Aggregate kind={:?} arity={}",
                value.kind(),
                value.operands().len()
            )?;
            for (i, operand) in value.operands().iter().take(2).enumerate() {
                write!(out, " operand{i}=")?;
                describe_operand(out, operand)?;
            }
            write!(out, " operands_truncated={}", value.operands().len() > 2)
        }
        SemanticRvalueKindV1::Unary { operation, .. } => {
            write!(out, "rvalue=Unary operation={operation:?}")
        }
        SemanticRvalueKindV1::Binary { operation, .. } => {
            write!(out, "rvalue=Binary operation={operation:?}")
        }
        SemanticRvalueKindV1::CheckedBinary(value) => write!(
            out,
            "rvalue=CheckedBinary operation={:?}",
            value.operation()
        ),
        SemanticRvalueKindV1::UncheckedBinary(value) => write!(
            out,
            "rvalue=UncheckedBinary operation={:?}",
            value.operation()
        ),
        SemanticRvalueKindV1::Cast { kind, .. } => write!(out, "rvalue=Cast kind={kind:?}"),
        SemanticRvalueKindV1::Length(place) => {
            write!(out, "rvalue=Length place=")?;
            describe_place(out, place)
        }
        SemanticRvalueKindV1::Discriminant(place) => {
            write!(out, "rvalue=Discriminant place=")?;
            describe_place(out, place)
        }
        SemanticRvalueKindV1::Load(value) => {
            write!(
                out,
                "rvalue=Load volatility={:?} source=",
                value.volatility()
            )?;
            describe_place(out, value.source())
        }
    }
}

#[cfg(test)]
mod tests;
