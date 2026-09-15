//! An explicitly selected statement, independent of the all-use history cap.
//! Reads retained classifier choices only; cannot publish or debit proof work.
use super::*;

const SELECTOR_ENV: &str = "FE2O3_TRACE_CARRIER_SITE";
const MAX_OPERANDS: usize = 16;
const TYPE_WORK: usize = 4096;

pub(in super::super) struct Observation<'a> {
    selected: Option<(&'a SemanticExpandedRootV1, Site)>,
}

impl<'a> Observation<'a> {
    pub(in super::super) fn new(view: Option<&'a SemanticExpandedRootV1>) -> Self {
        let raw = std::env::var(SELECTOR_ENV).ok();
        Self::with_setting(view, raw.as_deref())
    }

    fn with_setting(view: Option<&'a SemanticExpandedRootV1>, raw: Option<&str>) -> Self {
        // Reuse the bounded body/local/block/statement selector. Here local is
        // the assignment destination, not the borrowed owner of the other trace.
        let selected = (|| {
            let view = view?;
            let selection = StorageObservation::selected(view.body(), raw?)?;
            let (local, site) = selection.selected_site(view.body())?;
            let SemanticStatementKindV1::Assign(a) = view.body().blocks()
                .get(site.block as usize)?.statements().get(site.statement as usize)?.kind()
            else { return None };
            (a.destination().local().index() == local).then_some((view, site))
        })();
        Self { selected }
    }

    pub(in super::super) fn emit(
        &mut self,
        site: Site,
        stage: &'static str,
        checked_secondary_fields: math_capture_flow_v1::CheckedFields,
        routes: &math_capture_flow_v1::Routes<'_>,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &[SemanticBorrowCandidateV1],
        policy_pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    ) {
        if !self.selected.is_some_and(|(_, selected)| selected == site) { return }
        let (view, _) = self.selected.take().unwrap();
        if !routes.observation_body_matches(view.body()) { return }
        let mut out = Output { bytes: [0; MAX_OUTPUT], len: 0, truncated: false };
        let complete = Self::write(&mut out, view, site, stage, checked_secondary_fields,
            routes, by_reference, candidates, policy_pairs).is_ok();
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(&out.bytes[..out.len]);
        let _ = writeln!(stderr, "\ncarrier-site-end diagnostic-only=true complete={complete} payload_truncated={}", out.truncated);
    }

    fn write<W: Write>(
        out: &mut W,
        view: &SemanticExpandedRootV1,
        site: Site,
        stage: &str,
        checked_secondary_fields: math_capture_flow_v1::CheckedFields,
        routes: &math_capture_flow_v1::Routes<'_>,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &[SemanticBorrowCandidateV1],
        policy_pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    ) -> io::Result<()> {
        write!(out, "carrier-site body=")?;
        for byte in view.body().identity().as_bytes() { write!(out, "{byte:02x}")?; }
        writeln!(out, " site={}:{} stage={stage} checked_secondary_fields={checked_secondary_fields:?} diagnostic-only=true",
            site.block, site.statement)?;
        let SemanticStatementKindV1::Assign(a) = view.body().blocks()[site.block as usize]
            .statements()[site.statement as usize].kind() else { return Ok(()) };
        let mut remaining = TYPE_WORK;
        let mut type_complete = true;
        let mut place = |out: &mut W, label: &str, p: &SemanticPlaceV1| -> io::Result<()> {
            write_place(out, label, view, p, &mut |ty| routes.owned(ty).copied())?;
            write_candidate(out, p.local().index(), by_reference, candidates)?;
            writeln!(out, "checked-policy-pair reference={} owned={:?}", p.ty().index(),
                policy_pairs.get(&p.ty()).map(|ty| ty.index()))?;
            type_complete &= routes.write_type_observation(out, p.ty(), &mut remaining)?;
            Ok(())
        };
        place(out, "destination", a.destination())?;
        writeln!(out, "rvalue_kind={:?} result_ty={}",
            std::mem::discriminant(a.value().kind()), a.value().result_type().index())?;
        if let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() {
            writeln!(out, "aggregate_kind={:?} operand_count={}", aggregate.kind(), aggregate.operands().len())?;
        }
        if let SemanticRvalueKindV1::Borrow { kind, .. } = a.value().kind() {
            writeln!(out, "borrow_kind={kind:?}")?;
        }
        let mut count = 0;
        let complete = a.value().kind().try_visit_operands(|operand| {
            if count == MAX_OPERANDS { return Err(io::ErrorKind::Interrupted.into()) }
            writeln!(out, "operand_index={count}")?;
            count += 1;
            match operand {
                SemanticOperandV1::Copy(p) => place(out, "Copy", p),
                SemanticOperandV1::Move(p) => place(out, "Move", p),
                SemanticOperandV1::Constant(_) => writeln!(out, "Constant ty={}", operand.ty().index()),
            }
        }).is_ok();
        if let SemanticRvalueKindV1::Borrow { place: p, .. }
            | SemanticRvalueKindV1::AddressOf { place: p, .. } = a.value().kind()
        { place(out, "borrow-place", p)?; }
        writeln!(out, "operand_prefix_truncated={} type_work_remaining={remaining} type_details_truncated={}", !complete, !type_complete)
    }
}

fn write_candidate(
    out: &mut impl Write,
    local: u32,
    by_reference: &BTreeMap<u32, usize>,
    candidates: &[SemanticBorrowCandidateV1],
) -> io::Result<()> {
    let index = by_reference.get(&local).copied();
    writeln!(out, "candidate-local={local} index={index:?}")?;
    if let Some((index, c)) = index.and_then(|i| candidates.get(i).map(|c| (i, c))) {
        writeln!(out, "candidate={index} site={}:{} source_local={} source_type={} source_reference={:?} parent_index={:?} source_kind={:?} value_alias={} valid={} consumers={} intrinsic_consumer={}",
            c.site.block, c.site.statement, c.source_local, c.source_type.index(),
            c.source_reference, c.source_reference.and_then(|local| by_reference.get(&local)),
            c.source_kind, c.value_alias, c.valid, c.consumers, c.intrinsic_consumer)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "carrier_site_tests.rs"]
mod tests;
