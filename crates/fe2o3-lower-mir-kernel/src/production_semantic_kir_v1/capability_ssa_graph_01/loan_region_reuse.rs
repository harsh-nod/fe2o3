use std::sync::Arc;

// CFG geometry only. Owner values and ordered invalidations are checked by
// loan_live_uncached for each distinct loan and consumer, including cache hits.
struct CapabilityLoanRegionV1 {
    blocks: Vec<bool>,
    acyclic: bool,
}

impl CapabilitySsaGraphV1<'_> {
    fn loan_region(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Arc<CapabilityLoanRegionV1>, ProductionSemanticKirErrorV1> {
        endpoint_scc::loan_region(self, from, to)
    }
}
