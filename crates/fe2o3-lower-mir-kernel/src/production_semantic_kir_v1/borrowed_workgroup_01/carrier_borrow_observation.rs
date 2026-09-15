// Failure-only original-site observation. No value/definition lookup, new graph,
// promotion, or loan is produced; the original NoPromotedUse rejection is kept.
use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaResolvedEventV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError, ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceSiteV1,
};
use std::ffi::OsStr;
use std::io::{self, Write};

const PREFIX: usize = 16;
const QUERY_STEPS: usize = 128;

fn enabled(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

struct Observation<'a> {
    body: [u8; 32],
    view: [u8; 32],
    root: u32,
    site: Site,
    source_local: u32,
    source_type: Option<u32>,
    place_type: u32,
    reference_type: u32,
    same_body: bool,
    same_plan: bool,
    promoted: bool,
    source_block: Option<(u32, u32, u32)>,
    source_statement: Option<SemanticExpandedStatementOriginV1>,
    original_local: Option<(u32, u32, u32)>,
    place_path: &'a [SemanticProjectionV1],
    requested_path: &'a [SemanticProjectionV1],
    events: std::result::Result<&'a [(u32, SsaResolvedEventV1)], QueryError>,
    query_remaining: usize,
}

pub(super) fn emit(
    resolver: &WorkgroupSourceResolverV1<'_, '_>,
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    site: Site,
    place: &SemanticPlaceV1,
    destination: SemanticTypeIdV1,
    requested_path: &[SemanticProjectionV1],
) {
    if !enabled(std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").as_deref()) {
        return;
    }
    let view = resolver.view;
    let source_block = view.block_origins().get(site.block as usize);
    let same_body = std::ptr::eq(query.function(), resolver.graph.body)
        && std::ptr::eq(query.function(), view.body());
    let same_plan = std::ptr::eq(query.plan().plan(), resolver.graph.ssa);
    // One immutable window lookup, with its own fixed diagnostic-only bound.
    // It cannot debit/refund the proof Graph or provide a replacement source use.
    let mut remaining = QUERY_STEPS;
    let events = if same_body && same_plan {
        query.resolved_events_at(
            ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(site.block),
                site.statement,
            ),
            &mut || match remaining.checked_sub(1) {
                Some(next) => {
                    remaining = next;
                    true
                }
                None => false,
            },
        )
    } else {
        Err(QueryError::WrongOwner)
    };
    let observation = Observation {
        body: *view.body().identity().as_bytes(),
        view: *view.identity(),
        root: view.root().index(),
        site,
        source_local: place.local().index(),
        source_type: view
            .body()
            .locals()
            .get(place.local().index() as usize)
            .map(|local| local.ty().index()),
        place_type: place.ty().index(),
        reference_type: destination.index(),
        same_body,
        same_plan,
        promoted: query
            .plan()
            .plan()
            .promoted_variables()
            .binary_search(&SsaVariableIdV1::new(place.local().index()))
            .is_ok(),
        source_block: source_block.map(|origin| {
            (
                origin.instance().index(),
                origin.function().index(),
                origin.block().index(),
            )
        }),
        source_statement: source_block.and_then(|origin| {
            site.statement
                .and_then(|statement| origin.statements().get(statement as usize).copied())
        }),
        original_local: view
            .local_origins()
            .get(place.local().index() as usize)
            .map(|origin| {
                (
                    origin.instance().index(),
                    origin.function().index(),
                    origin.local().index(),
                )
            }),
        place_path: place.projections(),
        requested_path,
        events,
        query_remaining: remaining,
    };
    let _ = write_observation(&mut io::stderr().lock(), &observation);
}

fn hex(out: &mut impl Write, bytes: &[u8; 32]) -> io::Result<()> {
    for byte in bytes {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

fn write_observation(out: &mut impl Write, o: &Observation<'_>) -> io::Result<()> {
    write!(out, "WORKGROUP_BORROW_FAILURE body=")?;
    hex(out, &o.body)?;
    write!(out, " view=")?;
    hex(out, &o.view)?;
    writeln!(
        out,
        " root={} site={:?} same_body={} same_plan={} promoted={} error=NoPromotedUse diagnostic_only=true",
        o.root, o.site, o.same_body, o.same_plan, o.promoted
    )?;
    writeln!(
        out,
        "WORKGROUP_BORROW_SOURCE block={:?} statement={:?} local={:?} source_local={} source_type={:?} place_type={} reference_type={}",
        o.source_block,
        o.source_statement,
        o.original_local,
        o.source_local,
        o.source_type,
        o.place_type,
        o.reference_type
    )?;
    for (label, path) in [("place", o.place_path), ("requested", o.requested_path)] {
        writeln!(
            out,
            "WORKGROUP_BORROW_PATH kind={label} count={} truncated={} prefix={:?}",
            path.len(),
            path.len() > PREFIX,
            &path[..path.len().min(PREFIX)]
        )?;
    }
    match &o.events {
        Ok(events) => {
            writeln!(
                out,
                "WORKGROUP_BORROW_WINDOW count={} truncated={} query_remaining={}",
                events.len(),
                events.len() > PREFIX,
                o.query_remaining
            )?;
            for (event, value) in events.iter().take(PREFIX) {
                writeln!(
                    out,
                    "WORKGROUP_BORROW_EVENT event={event} resolved={value:?}"
                )?;
            }
        }
        Err(error) => writeln!(
            out,
            "WORKGROUP_BORROW_WINDOW error={error:?} query_remaining={}",
            o.query_remaining
        )?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("carrier_borrow_observation_tests.rs");
}
