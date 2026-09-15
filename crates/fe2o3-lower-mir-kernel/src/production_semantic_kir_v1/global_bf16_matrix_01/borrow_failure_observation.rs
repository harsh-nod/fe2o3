// Failure-only metadata. No alternative value, definition or loan is queried.
use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaVariableIdV1};
use std::ffi::OsStr;
use std::io::{self, Write};

const PREFIX: usize = 8;

fn enabled(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

struct Observation<'a> {
    body: [u8; 32],
    target: &'static str,
    site: CapabilityDefinitionSiteV1,
    consumer: CapabilityDefinitionSiteV1,
    value: SsaValueV1,
    root: Option<u32>,
    view: Option<[u8; 32]>,
    query: &'static str,
    same_plan: Option<bool>,
    promoted: Option<bool>,
    source_block: Option<(u32, u32, u32)>,
    source_statement: Option<SemanticExpandedStatementOriginV1>,
    source_local: Option<(u32, u32, u32)>,
    owner_local: u32,
    owner_type: Option<u32>,
    owner_role: Option<SemanticLocalRoleV1>,
    place_type: u32,
    reference_type: u32,
    events: Option<usize>,
    place_path: &'a [SemanticProjectionV1],
    requested_path: &'a [SemanticProjectionKindV1],
}

pub(super) fn emit(
    resolver: &Resolver<'_, '_>,
    site: CapabilityDefinitionSiteV1,
    value: SsaValueV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    place: &SemanticPlaceV1,
    requested_path: &[SemanticProjectionKindV1],
) {
    if !enabled(std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").as_deref()) {
        return;
    }
    let body = resolver.graph.body;
    let local = body.locals().get(place.local().index() as usize);
    let mut observation = Observation {
        body: *body.identity().as_bytes(),
        target: match resolver.target {
            ResolveTarget::Global => "Global",
            ResolveTarget::MatrixConstruction => "MatrixConstruction",
        },
        site,
        consumer: resolver.consumer,
        value,
        root: None,
        view: None,
        query: "unavailable",
        same_plan: None,
        promoted: None,
        source_block: None,
        source_statement: None,
        source_local: None,
        owner_local: place.local().index(),
        owner_type: local.map(|local| local.ty().index()),
        owner_role: local.map(|local| local.role()),
        place_type: place.ty().index(),
        reference_type: assignment.value().result_type().index(),
        events: resolver
            .graph
            .ssa
            .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(site.block))
            .map(|events| events.len()),
        place_path: place.projections(),
        requested_path,
    };
    if let Some((owner, root)) = resolver.observation_source {
        observation.root = Some(root.index());
        // Existing membership query only: no replay, event-use query, or budget.
        match owner.source_query_for_root(root, body) {
            Ok(query) => {
                observation.query = "checked";
                observation.same_plan = Some(std::ptr::eq(query.plan().plan(), resolver.graph.ssa));
                observation.promoted = Some(
                    query
                        .plan()
                        .plan()
                        .promoted_variables()
                        .binary_search(&SsaVariableIdV1::new(place.local().index()))
                        .is_ok(),
                );
                if let Some(view) = owner.execution_view_for_root(root) {
                    observation.view = Some(*view.identity());
                    if let Some(origin) = view.block_origins().get(site.block as usize) {
                        observation.source_block = Some((
                            origin.instance().index(),
                            origin.function().index(),
                            origin.block().index(),
                        ));
                        observation.source_statement = site.statement.and_then(|statement| {
                            origin.statements().get(statement as usize).copied()
                        });
                    }
                    observation.source_local = view
                        .local_origins()
                        .get(place.local().index() as usize)
                        .map(|origin| {
                            (
                                origin.instance().index(),
                                origin.function().index(),
                                origin.local().index(),
                            )
                        });
                }
            }
            Err(_) => observation.query = "wrong-owner",
        }
    }
    let _ = write_observation(&mut io::stderr().lock(), &observation);
}

fn hex(out: &mut impl Write, bytes: &[u8; 32]) -> io::Result<()> {
    for byte in bytes {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

fn write_observation(out: &mut impl Write, o: &Observation<'_>) -> io::Result<()> {
    write!(out, "BF16_BORROW_BOUNDARY body=")?;
    hex(out, &o.body)?;
    writeln!(
        out,
        " target={} definition={:?} reference_value={:?} consumer={:?} diagnostic_only=true",
        o.target, o.site, o.value, o.consumer
    )?;
    write!(out, "BF16_BORROW_OWNER root={:?} view=", o.root)?;
    match o.view {
        Some(bytes) => hex(out, &bytes)?,
        None => write!(out, "unavailable")?,
    }
    writeln!(
        out,
        " query={} same_plan={:?} promoted={:?} events={:?}",
        o.query, o.same_plan, o.promoted, o.events
    )?;
    writeln!(
        out,
        "BF16_BORROW_SOURCE block={:?} statement={:?} local={:?} owner_local={} owner_type={:?} owner_role={:?} place_type={} reference_type={}",
        o.source_block,
        o.source_statement,
        o.source_local,
        o.owner_local,
        o.owner_type,
        o.owner_role,
        o.place_type,
        o.reference_type
    )?;
    writeln!(
        out,
        "BF16_BORROW_PLACE count={} truncated={} prefix={:?}",
        o.place_path.len(),
        o.place_path.len() > PREFIX,
        &o.place_path[..o.place_path.len().min(PREFIX)]
    )?;
    writeln!(
        out,
        "BF16_BORROW_REQUEST count={} truncated={} prefix={:?}",
        o.requested_path.len(),
        o.requested_path.len() > PREFIX,
        &o.requested_path[..o.requested_path.len().min(PREFIX)]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("borrow_failure_observation_tests.rs");
}
