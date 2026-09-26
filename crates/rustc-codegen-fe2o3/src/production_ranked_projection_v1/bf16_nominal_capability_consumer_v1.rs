//! Narrow real-facts consumer for C2. This is a child of canonical facts;
//! its only mutable ledger borrow is the one already retained by those facts.
use super::super::bf16_nominal_call_routing_v1::NominalCallVisitorV1;
use super::super::bf16_nominal_capabilities_v1::NominalCallerSiteV1;
use super::super::bf16_nominal_dense_v1::{NominalCapabilityConsumerV1, NominalCapabilityLedgerV1};
use super::super::{
    ProductionRankedProjectionErrorV1, SemanticDirectCallV1, SemanticSourceProvenanceV1,
    SemanticTerminatorKindV1,
};
use super::{
    CanonicalAssertionErrorV1, CanonicalSourceAssertionFactsV1, ProjectedAssertionFactsV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as Error;

type Result<T> = std::result::Result<T, Error>;
struct Consumer<'f, 'r, 'i, 'g, 'b, 'w> {
    facts: &'f mut CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w>,
}
impl NominalCapabilityConsumerV1 for Consumer<'_, '_, '_, '_, '_, '_> {
    fn charge_work_v1(&mut self, amount: usize) -> Result<()> {
        self.facts
            .budget
            .charge_work(amount)
            .map_err(Error::Resource)
    }
    fn reserve_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.facts
            .budget
            .reserve_storage(amount)
            .map_err(Error::Resource)
    }
    fn release_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.facts
            .budget
            .release_storage(amount)
            .map_err(Error::Resource)
    }
    fn ledger_v1(&self) -> NominalCapabilityLedgerV1 {
        let budget: &Budget<'_> = self.facts.budget;
        NominalCapabilityLedgerV1 {
            slot: budget as *const Budget<'_> as usize,
            identity: budget.work_ledger_identity_v1(),
            work: budget.work(),
            storage: budget.storage(),
            peak: budget.peak_storage(),
            denied_work: budget.failed_work().is_some(),
            denied_storage: budget.failed_storage().is_some(),
        }
    }
    fn with_nominal_call_v1(
        &mut self,
        block: usize,
        call: &SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
        visit: &mut NominalCallVisitorV1<'_>,
    ) -> Result<()> {
        // Exact existing canonical method, including its terminator provenance
        // join. Its only current error constructors are Resource and NominalCall.
        self.facts
            .with_nominal_call_v1(block, call, source, visit)
            .map_err(|error| match error {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(error),
                ) => Error::Resource(error),
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::NominalCall(error),
                ) => error,
                ProductionRankedProjectionErrorV1::Incomplete(reason)
                | ProductionRankedProjectionErrorV1::Unsupported(reason) => {
                    Error::Unavailable(reason)
                }
                _ => Error::Unavailable("nominal canonical facts forwarding contract changed"),
            })
    }
}

/// Real B4 facts only. No trait-object fallback, guarded decorator bypass,
/// masked facts, ranked owner or separately stored mutable Budget is accepted.
/// The result cannot retain the local site/consumer or any borrowed authority.
pub(in crate::production_ranked_projection_v1) fn with_nominal_capability_consumer_v1<
    R: Copy + 'static,
>(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    inspect: impl for<'s> FnOnce(
        &NominalCallerSiteV1<'s>,
        &mut dyn NominalCapabilityConsumerV1,
    ) -> Result<R>,
) -> Result<R> {
    if facts.masked.is_some() {
        return Err(Error::Unavailable(
            "nominal dense consumer requires its unmasked real facts scope",
        ));
    }
    let owner = facts.owner;
    let report = facts.report;
    let inventory = report.inventory();
    let emission = owner
        .bf16_call_instance_emission_v1()
        .ok_or(Error::Unavailable(
            "nominal dense source emission relation absent",
        ))?;
    let block = emission.source_call_block();
    let function = owner
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(facts.semantic_function.index() as usize)
        .ok_or(Error::Unavailable("nominal dense source function absent"))?;
    let actual = function
        .blocks()
        .get(block.index() as usize)
        .ok_or(Error::Unavailable("nominal dense source call block absent"))?;
    let SemanticTerminatorKindV1::Call(call) = actual.terminator().kind() else {
        return Err(Error::Unavailable(
            "nominal dense source call terminal absent",
        ));
    };
    let site = NominalCallerSiteV1::for_source(
        owner,
        inventory,
        facts.correspondence_owner,
        facts.semantic_function,
        block,
        call,
        actual.terminator().source(),
        facts.budget,
    )?;
    let mut consumer = Consumer { facts };
    inspect(&site, &mut consumer)
}
