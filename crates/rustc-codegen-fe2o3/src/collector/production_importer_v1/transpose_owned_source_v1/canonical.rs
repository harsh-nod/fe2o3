//! Complete live-source/footer join. An inert decoded row never constructs a
//! MappedPlan; only source observation and full replay do that.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticOwnedSourceCallSiteV1 as CallSite, SemanticOwnedSourceStatementSiteV1 as StatementSite,
    SemanticTransposeOwnedFlowSitesV1 as FlowSites, SemanticTransposeOwnedFlowV1 as Flow,
};

#[cfg(test)]
#[path = "canonical_tests.rs"]
pub(super) mod tests;

fn schema(error: fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1) -> PlanError {
    PlanError::Replay(ProductionSemanticImportErrorV1::SemanticSchema(error))
}

fn site((function, block): (SemanticFunctionIdV1, SemanticBlockIdV1)) -> CallSite {
    CallSite { function, block }
}

impl<'a> MappedPlan<'a> {
    /// Consume the complete live roster into inert canonical records. The
    /// request owner still performs full structural admission and live replay.
    pub(in super::super) fn into_canonical_rows(
        self,
        fragment_bytes: &mut u64,
        work: &mut usize,
    ) -> PlanResult<Vec<Flow>> {
        let mut rows = Vec::new();
        for flow in self.flows {
            bounded::reserve(&mut rows, 1, work)?;
            rows.push(row(self.source, &flow, fragment_bytes, work)?);
        }
        Ok(rows)
    }

    /// A production consumer must enter through this method, with the live
    /// replay plan rebuilt against the SAME admitted owner carrying the footer.
    pub(in super::super) fn bind_canonical_footer(
        self,
        owner: &'a fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        fragment_bytes: &mut u64,
        work: &mut usize,
    ) -> PlanResult<BoundPlan<'a>> {
        if !std::ptr::eq(self.source, owner.source_semantic()) {
            return Err(Error::Source("transpose footer and SSA source owners differ").into());
        }
        self.check_canonical_footer(fragment_bytes, work)?;
        self.bind(owner, work)
    }

    pub(super) fn check_canonical_footer(
        &self,
        fragment_bytes: &mut u64,
        work: &mut usize,
    ) -> PlanResult<()> {
        let rows = self.source.transpose_owned_flows();
        bounded::charge(work, 1)?;
        if rows.len() != self.flows.len() {
            return Err(Error::Source("transpose canonical source roster changed").into());
        }
        for (flow, actual) in self.flows.iter().zip(rows) {
            bounded::charge(work, std::mem::size_of::<Flow>().div_ceil(8))?;
            let expected = row(self.source, flow, fragment_bytes, work)?;
            let words = expected
                .workgroup_borrows()
                .len()
                .checked_add(204_usize.div_ceil(8))
                .ok_or(Error::Work)?;
            bounded::charge(work, words)?;
            if &expected != actual {
                return Err(Error::Source("transpose canonical source row changed").into());
            }
        }
        Ok(())
    }
}

fn row(
    source: &AdmittedInertSemanticMirV1,
    flow: &MappedFlow<'_>,
    fragment_bytes: &mut u64,
    work: &mut usize,
) -> PlanResult<Flow> {
    for (function, body) in flow.bodies {
        bounded::charge(work, 1)?;
        if source
            .functions()
            .get(function.index() as usize)
            .is_none_or(|retained| !std::ptr::eq(retained, body))
        {
            return Err(Error::Source("transpose canonical body owner changed").into());
        }
    }
    let mut borrows = Vec::new();
    for &borrow in &flow.workgroup_borrows {
        bounded::charge(work, 1)?;
        bounded::push(&mut borrows, site(borrow), work)?;
    }
    let sites = FlowSites {
        issue: site(flow.issue),
        capture: StatementSite {
            function: flow.capture.0,
            block: flow.capture.1,
            statement: flow.capture.2,
        },
        capture_field: flow.capture_field,
        matrix_call: site(flow.matrix_call),
        closure_call: site(flow.closure_call),
        stage: site(flow.stage),
        publish: site(flow.publish),
        workgroup_local: flow.workgroup_local,
    };
    // Bound encoding before it runs. All rows share both allowances. On error,
    // conservatively consume its granted allowance; a failed hash cannot be
    // retried against a refunded source ledger. The original schema error stays.
    if *work == 0 {
        return Err(Error::Work.into());
    }
    let allowance =
        (*fragment_bytes).min(u64::try_from(*work).unwrap_or(u64::MAX).saturating_mul(8));
    let built = Flow::for_retained_source(
        source.functions(),
        sites,
        borrows,
        flow.source_binding,
        allowance,
    );
    let charged_bytes = match &built {
        Ok((_, bytes)) => u64::try_from(*bytes).map_err(|_| Error::Work)?,
        Err(_) => allowance,
    };
    bounded::charge(
        work,
        usize::try_from(charged_bytes.div_ceil(8)).map_err(|_| Error::Work)?,
    )?;
    *fragment_bytes = fragment_bytes
        .checked_sub(charged_bytes)
        .ok_or(Error::Work)?;
    let (row, _) = built.map_err(schema)?;
    Ok(row)
}
