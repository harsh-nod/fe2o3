//! Storage-capable checked transaction using the same exact Store consensus.
use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirCrossBlockForwardingV18 as Pair18,
    check_canonical_kir_cross_block_forwarding_v18 as check_pair,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayStorageV18 as OutputStorage18, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrIdentityV18 as Identity18,
    VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

/// Actual V18 output and full same-site correspondence. Only proved direct
/// private scalar Loads are replaced; no object-memory alias rule is implied.
pub struct OwnedCrossBlockForwardingV18 {
    output: Owner18,
    output_storage: OutputStorage18,
    input_identity: Identity18,
    origins: Vec<Row>,
    limits: Limits,
    retained: usize,
}
impl OwnedCrossBlockForwardingV18 {
    /// Fresh actual final owner.
    pub const fn output(&self) -> &Owner18 {
        &self.output
    }
    /// Exact original identity, only an early replay mismatch check.
    pub const fn input_identity(&self) -> &Identity18 {
        &self.input_identity
    }
    /// Complete input-ordered same-site operation correspondence.
    pub fn origins(&self) -> &[Row] {
        &self.origins
    }
    /// Exact analysis ceilings reused by independent replay.
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    /// Reserve this unreserved credit while retaining the owning output.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This component does not confer source, native or execution authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Rebuilds analysis and checks every actual endpoint and lineage row.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner18,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair18<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(
                size_of::<Identity18>()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained_v18(self.output_storage, &self.origins)? {
                return Err(Resource::Accounting.into());
            }
            if input.identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|budget| {
                Ok(check_pair(
                    input,
                    &self.output,
                    &self.origins,
                    self.limits,
                    budget,
                )?)
            })
        })
    }
}

/// Runs the shared consensus rule on an actual V18 owner, admits the private
/// candidate with the caller's unchanged layout ceilings, then independently
/// checks it. Storage tables and unsupported operations are preserved exactly.
pub fn prepare_owned_cross_block_forwarding_v18(
    input: &Owner18,
    limits: Limits,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedCrossBlockForwardingV18> {
    resources::scoped(budget, |meter| {
        meter.reserve(header_v18()?)?;
        meter.reserve(size_of::<StorageLayoutLimitsV1>())?;
        let (inventory, size) = meter.derive(|b| Ok(Inventory::derive_v18(input, b)?))?;
        meter.reserve(size.retained_storage())?;
        let (memory, size) =
            meter.derive(|b| Ok(Memory::derive_v18(&inventory, limits.memory, b)?))?;
        meter.reserve(size.retained_storage())?;
        let origins = plan(&inventory, &memory, limits, meter)?;
        let (mut candidate, size) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v18(b)?))?;
        meter.reserve(size.retained_storage())?;
        materialize(&inventory, &origins, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner18::from_module_ref_with_verification_budget_v18(
                &candidate, layouts, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let pair_size = {
            let (_pair, pair_size) =
                meter.derive(|b| Ok(check_pair(input, &output, &origins, limits, b)?))?;
            meter.reserve(pair_size.retained_storage())?;
            pair_size
        };
        meter.release(pair_size.retained_storage())?;
        let retained = retained_v18(output_storage, &origins)?;
        // Scratch remains credited until the outer scope drops its backing.
        meter.work(1)?;
        Ok(OwnedCrossBlockForwardingV18 {
            output,
            output_storage,
            input_identity: *input.identity(),
            origins,
            limits,
            retained,
        })
    })
}
fn header_v18() -> Result<usize> {
    size_of::<OwnedCrossBlockForwardingV18>()
        .checked_sub(size_of::<Owner18>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained_v18(output: OutputStorage18, rows: &Vec<Row>) -> Result<usize> {
    header_v18()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}
