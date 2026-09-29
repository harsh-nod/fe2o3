//! Actual V18 producer; only the total-scalar selection algorithm is shared.
use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirLicmV18 as Pair18, check_canonical_kir_licm_v18 as check_pair,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayStorageV18 as OutputStorage18,
    VerifiedCanonicalKernelIrIdentityV18 as Identity18,
    VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

/// Move-only V18 output with every input operation's exact output coordinate.
/// Storage layouts and execution/memory/call operations remain present. A source
/// continuation must retain its authentic input and revalidate source/control
/// relocation plus final native conditions; this component grants none of those.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::{OwnedLicmContinuationV1, OwnedLicmContinuationV18};
/// fn relabel(value: OwnedLicmContinuationV18) -> OwnedLicmContinuationV1 { value }
/// ```
pub struct OwnedLicmContinuationV18 {
    output: Owner18,
    output_storage: OutputStorage18,
    input_identity: Identity18,
    origins: Vec<Row>,
    retained: usize,
}
impl OwnedLicmContinuationV18 {
    pub const fn output(&self) -> &Owner18 {
        &self.output
    }
    pub const fn input_identity(&self) -> &Identity18 {
        &self.input_identity
    }
    pub fn origins(&self) -> &[Row] {
        &self.origins
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Rechecks complete actual endpoints and all movement conditions. The stored
    /// input identity is only an early mismatch check, never a replay substitute.
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
            meter.derive(|b| {
                Ok(check_pair(
                    input,
                    &self.output,
                    &self.origins,
                    Limits::default(),
                    b,
                )?)
            })
        })
    }
}

/// Hoists the existing closed total-integer grammar on the actual storage-capable
/// graph. Reuses the FIFO def-use selector and move-only materializer, admits a
/// fresh V18 output, and independently checks both exact endpoints. Original and
/// final CFG/loop/MemorySSA analyses belong to distinct owners and cannot be reused
/// across this boundary. No numbered policy or default-pipeline activation.
/// The caller forwards its captured V18 storage-layout admission limits; they
/// are used unchanged for the fresh output, without a permissive fallback.
pub fn prepare_owned_licm_v18(
    input: &Owner18,
    layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedLicmContinuationV18> {
    resources::scoped(budget, |meter| {
        meter.reserve(header_v18()?)?;
        meter.reserve(size_of::<fe2o3_kernel_ir::StorageLayoutLimitsV1>())?;
        let (inventory, is) = meter.derive(|b| Ok(Inventory::derive_v18(input, b)?))?;
        meter.reserve(is.retained_storage())?;
        let (loops, ls) =
            meter.derive(|b| Ok(Loops::derive_v18(&inventory, Limits::default(), b)?))?;
        meter.reserve(ls.retained_storage())?;
        meter.derive(|b| Ok(loops.replay(&inventory, Limits::default(), b)?))?;
        let origins = plan(&inventory, &loops, meter)?;
        let (mut candidate, cs) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v18(b)?))?;
        meter.reserve(cs.retained_storage())?;
        let extra_candidate = materialize(&inventory, &origins, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner18::from_module_ref_with_verification_budget_v18(
                &candidate, layouts, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let ps = {
            let (_pair, ps) = meter
                .derive(|b| Ok(check_pair(input, &output, &origins, Limits::default(), b)?))?;
            meter.reserve(ps.retained_storage())?;
            ps
        };
        meter.release(ps.retained_storage())?;
        let retained = retained_v18(output_storage, &origins)?;
        drop(candidate);
        meter.release(
            cs.retained_storage()
                .checked_add(extra_candidate)
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(loops);
        meter.release(ls.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        meter.work(1)?;
        Ok(OwnedLicmContinuationV18 {
            output,
            output_storage,
            input_identity: *input.identity(),
            origins,
            retained,
        })
    })
}

fn header_v18() -> Result<usize> {
    size_of::<OwnedLicmContinuationV18>()
        .checked_sub(size_of::<Owner18>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained_v18(output: OutputStorage18, rows: &Vec<Row>) -> Result<usize> {
    header_v18()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}
