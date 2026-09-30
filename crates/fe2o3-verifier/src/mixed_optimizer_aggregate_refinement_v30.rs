//! Canonical aggregate-step obligations. This owner is deliberately not a
//! source-owned Policy12 receipt; the production chain must retain and join it.
use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirAggregateSsaV18 as Pair;

const DOMAIN: &[u8] = b"FE2O3/V18/CONCRETE-PRIVATE-MEMORY-CFG/V30\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateMemoryCfgSubjectV30 {
    input: VerifiedCanonicalKernelIrIdentityV18,
    output: VerifiedCanonicalKernelIrIdentityV18,
    statement: [u8; 32],
    blocks: usize,
    slots: usize,
    parameters: usize,
}
impl AggregateMemoryCfgSubjectV30 {
    pub const fn input(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.input
    }
    pub const fn output(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.output
    }
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    pub const fn modeled_blocks(self) -> usize {
        self.blocks
    }
    pub const fn modeled_slots(self) -> usize {
        self.slots
    }
    pub const fn appended_parameters(self) -> usize {
        self.parameters
    }
}

/// Move-only bounded compiler-generated obligation retaining independently
/// checked actual endpoints. There is no text/digest/row-report constructor.
/// This canonical step is not an original semantic-MIR chain or an executed
/// refinement. The caller pays all borrowed owners and this unreserved receipt.
pub struct AggregateMemoryCfgObligationV30<'graph> {
    pair: Pair<'graph>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: AggregateMemoryCfgSubjectV30,
    retained: usize,
}
impl<'graph> AggregateMemoryCfgObligationV30<'graph> {
    pub const fn checked_pair(&self) -> &Pair<'graph> {
        &self.pair
    }
    pub const fn subject(&self) -> AggregateMemoryCfgSubjectV30 {
        self.subject
    }
    pub fn generated_source(&self) -> &[u8] {
        self.generated.source()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    pub const fn authenticates_original_source(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Generates concrete selected-private-memory and actual CFG obligations from
/// the independently checked immutable endpoint relation. Uses the existing
/// shared arithmetic/Select emitter and finite-trace lemma. No Verus is run.
/// The existing same-ledger scope restores entry storage on return/refusal or
/// unwind; success returns an unreserved receipt for the retained generated text.
pub fn prepare_aggregate_memory_cfg_obligation_v30<'graph>(
    pair: Pair<'graph>,
    budget: &mut Budget<'_>,
) -> Result<AggregateMemoryCfgObligationV30<'graph>> {
    let floor = budget.storage();
    let headers = aggregate_memory_cfg_headers_v30()?;
    budget.with_prepaid_scope(floor, 1, 1, headers, move |budget| {
        let (input, input_storage) = Inventory::derive_v18(pair.input(), budget)?;
        budget.reserve_storage(input_storage.retained_storage())?;
        let (output, output_storage) = Inventory::derive_v18(pair.output(), budget)?;
        budget.reserve_storage(output_storage.retained_storage())?;
        let mut writer = Writer::new(budget)?;
        let blocks =
            semantics::generate_aggregate_memory_cfg_v30(&input, &output, &pair, &mut writer)?;
        let text = writer.finish()?;
        budget.charge_work(text.len().checked_mul(3).ok_or(Resource::Arithmetic)?)?;
        let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())?;
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        for owner in [pair.input(), pair.output()] {
            digest.update(owner.identity().canonical_length().to_le_bytes());
            digest.update(owner.identity().digest());
        }
        digest.update(generated.identity().as_bytes());
        let subject = AggregateMemoryCfgSubjectV30 {
            input: *pair.input().identity(),
            output: *pair.output().identity(),
            statement: digest.finalize().into(),
            blocks,
            slots: pair.witness().memory_slots().len(),
            parameters: pair.witness().parameters().len(),
        };
        let retained = size_of::<AggregateMemoryCfgObligationV30<'_>>()
            .checked_add(generated.source().len())
            .ok_or(Resource::Arithmetic)?;
        drop(input);
        drop(output);
        Ok(AggregateMemoryCfgObligationV30 {
            pair,
            generated,
            subject,
            retained,
        })
    })
}

fn aggregate_memory_cfg_headers_v30() -> Result<usize> {
    // The String and canonical boxed source can coexist during conversion.
    [
        SOURCE_LIMIT,
        SOURCE_LIMIT,
        size_of::<AggregateMemoryCfgObligationV30<'_>>(),
        size_of::<Result<AggregateMemoryCfgObligationV30<'_>>>(),
        size_of::<std::thread::Result<Result<AggregateMemoryCfgObligationV30<'_>>>>(),
        size_of::<Pair<'_>>(),
        size_of::<(
            Inventory<'_>,
            fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        )>(),
        size_of::<
            std::result::Result<
                (
                    Inventory<'_>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
                ),
                CanonicalKirInventoryErrorV1,
            >,
        >(),
        size_of::<(
            Inventory<'_>,
            fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        )>(),
        size_of::<
            std::result::Result<
                (
                    Inventory<'_>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
                ),
                CanonicalKirInventoryErrorV1,
            >,
        >(),
        size_of::<Writer<'_, '_>>(),
        size_of::<Result<Writer<'_, '_>>>(),
        size_of::<String>(),
        size_of::<Result<String>>(),
        size_of::<
            std::result::Result<
                CanonicalGeneratedVerusProofInputV3,
                GeneratedVerusProofInputErrorV3,
            >,
        >(),
        size_of::<Sha256>(),
        size_of::<AggregateMemoryCfgSubjectV30>(),
        size_of::<[&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18; 2]>(),
        size_of::<(
            usize,
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            std::result::Result<(), Resource>,
        )>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total.checked_add(bytes).ok_or(Resource::Arithmetic.into())
    })
}

#[cfg(test)]
#[path = "mixed_optimizer_aggregate_refinement_v30_tests.rs"]
mod tests;
