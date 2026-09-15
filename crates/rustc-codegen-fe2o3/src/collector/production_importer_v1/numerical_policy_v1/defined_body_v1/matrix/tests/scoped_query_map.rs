//! Test-only mapping of an observed failed query onto its retained view.
//! No query resolver, new Graph, custody claim, or definition fallback.
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticExpandedRootV1, SsaBlockIdV1};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
use std::io::{self, Write};

const OUTPUT_BYTES: usize = 16_384;
const STATEMENTS: usize = 64;
const OPERANDS_PER_STATEMENT: usize = 16;
const EVENTS: usize = 256;

pub(super) fn inspect_recorded(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    recorded: &[(&str, u32, u32)],
    out: &mut impl Write,
) -> io::Result<()> {
    // Fixed measured-record prefix, never a source acceptance selector. A
    // changed body is reported instead of treating stale coordinates as valid.
    for &(digest, block, local) in recorded.iter().take(4) {
        let Some(bytes) = decode_digest(digest) else {
            return writeln!(out, "query-map rejected malformed observation digest");
        };
        if view.body().identity().as_bytes() == &bytes {
            return inspect(owner, view, &bytes, block, local, out);
        }
    }
    writeln!(
        out,
        "query-map no matching observed body; recorded_prefix_truncated={}",
        recorded.len() > 4
    )
}

fn decode_digest(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let digit = |b| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let mut result = [0; 32];
    for (out, pair) in result.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *out = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Some(result)
}

pub(super) fn inspect(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    observed_body: &[u8; 32],
    block: u32,
    local: u32,
    out: &mut impl Write,
) -> io::Result<()> {
    let mut capped = Capped {
        out,
        remaining: OUTPUT_BYTES,
        truncated: false,
    };
    let result = map(owner, view, observed_body, block, local, &mut capped);
    let truncated = capped.truncated;
    // The fixed trailer is outside the payload ceiling, including when the
    // writer cap interrupts a line. No interrupted output is called complete.
    writeln!(
        capped.out,
        "\nquery-map end payload_truncated={truncated} diagnostic_only=true"
    )?;
    if truncated { Ok(()) } else { result }
}

fn map(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    observed_body: &[u8; 32],
    block: u32,
    local: u32,
    out: &mut impl Write,
) -> io::Result<()> {
    if !owner
        .execution_view_for_root(view.root())
        .is_some_and(|v| std::ptr::eq(v, view))
        || view.body().identity().as_bytes() != observed_body
    {
        return writeln!(
            out,
            "query-map rejected owner/view/body coordinate mismatch"
        );
    }
    let Some(plan) = owner.execution_plan_for_root(view.root()) else {
        return writeln!(out, "query-map rejected absent retained plan");
    };
    if plan.function_identity() != view.body().identity() {
        return writeln!(out, "query-map rejected retained plan/body mismatch");
    }
    let (Some(data), Some(origin), Some(decl), Some(local_origin)) = (
        view.body().blocks().get(block as usize),
        view.block_origins().get(block as usize),
        view.body().locals().get(local as usize),
        view.local_origins().get(local as usize),
    ) else {
        return writeln!(
            out,
            "query-map rejected block/local outside retained inventories"
        );
    };
    if data.statements().len() != origin.statements().len() {
        return writeln!(
            out,
            "query-map rejected statement origin inventory mismatch"
        );
    }
    writeln!(
        out,
        "query-map root={} source_body={} block={block} local={local} local_ty={} block_origin=(instance:{} function:{} block:{}) local_origin=(instance:{} function:{} local:{}) terminator_origin={:?}",
        view.root().index(),
        view.source_body().index(),
        decl.ty().index(),
        origin.instance().index(),
        origin.function().index(),
        origin.block().index(),
        local_origin.instance().index(),
        local_origin.function().index(),
        local_origin.local().index(),
        origin.terminator(),
    )?;
    match plan.plan().resolved_events(SsaBlockIdV1::new(block)) {
        None => writeln!(out, "retained_events=unavailable")?,
        Some(events) => {
            // Existing plan only. A printed Define is not a candidate fallback.
            for (index, event) in events.iter().take(EVENTS) {
                writeln!(out, "event={index} {event:?}")?;
            }
            writeln!(
                out,
                "event_total={} event_prefix_truncated={}",
                events.len(),
                events.len() > EVENTS
            )?;
        }
    }
    for (index, (statement, marker)) in data
        .statements()
        .iter()
        .zip(origin.statements())
        .take(STATEMENTS)
        .enumerate()
    {
        writeln!(out, "statement={index} marker={marker:?}")?;
        for (label, source) in [
            ("expansion", statement.source().expansion()),
            ("call_site", statement.source().call_site()),
        ] {
            if let Some(source) = source {
                write!(out, "{label} file=")?;
                for byte in source.file().as_bytes() {
                    write!(out, "{byte:02x}")?;
                }
                writeln!(
                    out,
                    " start={:?} end={:?}",
                    source.start_coordinate(),
                    source.end_coordinate()
                )?;
            }
        }
        match statement.kind() {
            SemanticStatementKindV1::Assign(a) => {
                show_place(out, "destination", a.destination())?;
                writeln!(
                    out,
                    "rvalue_kind={:?} result_ty={}",
                    std::mem::discriminant(a.value().kind()),
                    a.value().result_type().index()
                )?;
                let mut seen = 0;
                let result = a.value().kind().try_visit_operands(|operand| {
                    if seen == OPERANDS_PER_STATEMENT {
                        return Err(None);
                    }
                    seen += 1;
                    show_operand(out, operand).map_err(Some)
                });
                match result {
                    Ok(()) => {}
                    Err(None) => writeln!(out, "operand_prefix_truncated=true")?,
                    Err(Some(error)) => return Err(error),
                }
                match a.value().kind() {
                    SemanticRvalueKindV1::Borrow { kind, place } => {
                        writeln!(out, "borrow_kind={kind:?}")?;
                        show_place(out, "borrow_place", place)?;
                    }
                    SemanticRvalueKindV1::AddressOf { place, .. }
                    | SemanticRvalueKindV1::Length(place)
                    | SemanticRvalueKindV1::Discriminant(place) => {
                        show_place(out, "place_only", place)?
                    }
                    _ => {}
                }
            }
            SemanticStatementKindV1::StorageLive(id) => {
                writeln!(out, "StorageLive local={}", id.index())?
            }
            SemanticStatementKindV1::StorageDead(id) => {
                writeln!(out, "StorageDead local={}", id.index())?
            }
            SemanticStatementKindV1::Deinitialize(place) => show_place(out, "Deinitialize", place)?,
            kind => writeln!(
                out,
                "other_statement_kind={:?}",
                std::mem::discriminant(kind)
            )?,
        }
    }
    writeln!(
        out,
        "statement_total={} statement_prefix_truncated={}",
        data.statements().len(),
        data.statements().len() > STATEMENTS
    )?;
    if let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() {
        writeln!(
            out,
            "terminator=Call callee={} argument_count={} argument_prefix_truncated={}",
            call.callee().index(),
            call.arguments().len(),
            call.arguments().len() > OPERANDS_PER_STATEMENT
        )?;
        for operand in call.arguments().iter().take(OPERANDS_PER_STATEMENT) {
            show_operand(out, operand)?;
        }
        if let Some(destination) = call.destination() {
            show_place(out, "call_destination", destination.place())?;
        }
    } else {
        writeln!(
            out,
            "other_terminator_kind={:?}",
            std::mem::discriminant(data.terminator().kind())
        )?;
    }
    Ok(())
}

fn show_operand(out: &mut impl Write, operand: &SemanticOperandV1) -> io::Result<()> {
    match operand {
        SemanticOperandV1::Copy(place) => show_place(out, "Copy", place),
        SemanticOperandV1::Move(place) => show_place(out, "Move", place),
        SemanticOperandV1::Constant(_) => writeln!(out, "Constant ty={}", operand.ty().index()),
    }
}

fn show_place(out: &mut impl Write, label: &str, place: &SemanticPlaceV1) -> io::Result<()> {
    writeln!(
        out,
        "{label} local={} ty={} projections={} projection_prefix_truncated={}",
        place.local().index(),
        place.ty().index(),
        place.projections().len(),
        place.projections().len() > 16
    )?;
    for projection in place.projections().iter().take(16) {
        writeln!(out, "projection={projection:?}")?;
    }
    Ok(())
}

struct Capped<'a, W> {
    out: &'a mut W,
    remaining: usize,
    truncated: bool,
}

impl<W: Write> Write for Capped<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.truncated = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "query-map payload ceiling",
            ));
        }
        let n = self.out.write(&bytes[..bytes.len().min(self.remaining)])?;
        self.remaining -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_map_observation_digest_is_exact_and_bounded() {
        assert_eq!(decode_digest(&"ab".repeat(32)), Some([0xab; 32]));
        for invalid in ["ab".repeat(31), "ab".repeat(33), "gg".repeat(32)] {
            assert!(decode_digest(&invalid).is_none());
        }
    }

    #[test]
    fn query_map_payload_cap_has_no_extra_write_or_silent_completeness() {
        let mut bytes = Vec::new();
        let mut out = Capped {
            out: &mut bytes,
            remaining: 4,
            truncated: false,
        };
        out.write_all(b"1234").unwrap();
        assert!(!out.truncated);
        assert_eq!(
            out.write_all(b"5").unwrap_err().kind(),
            io::ErrorKind::WriteZero
        );
        assert!(out.truncated);
        assert_eq!(bytes, b"1234");
    }
}
