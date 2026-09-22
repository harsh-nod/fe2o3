//! Closed consuming source-to-J composition; not a new fixed policy or artifact.
use super::*;
use fe2o3_kernel_opt::{
    OwnedRedundantStoreContinuationV1, prepare_owned_redundant_store_continuation_v1,
};
#[path = "production_checked_output_owned_prefix_v1.rs"]
mod owned_prefix;
use owned_prefix::OwnedPrefix;

/// Additional J/metadata/report-row and heap-prefix receipt. The caller owns
/// all pre-existing Prefix6, separately reserved B and unrelated reservations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionOwnedRedundantStoreStorageV1(usize);
impl ProductionOwnedRedundantStoreStorageV1 {
    /// Reserve this added receipt before further controlled allocations.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct StoreData {
    continuation: OwnedRedundantStoreContinuationV1,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Retains the entire actual Direct Prefix6 once, actual J, and fresh J reports.
/// Historical I reports remain only inside Prefix6. No native artifact, policy
/// record, source/signed proof or publication authority is synthesized.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedRedundantStoreContinuationV1;
/// fn clone(value: ProductionOwnedRedundantStoreContinuationV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedRedundantStoreContinuationV1;
/// fn parts(value: ProductionOwnedRedundantStoreContinuationV1) { let _ = value.prefix; }
/// ```
pub struct ProductionOwnedRedundantStoreContinuationV1 {
    prefix: OwnedPrefix<ProductionCheckedOutputOwnerPolicy6V1>,
    data: StoreData,
}

/// UnitLocal counterpart retaining actual original N/E custody without storing
/// references into itself. J is never substituted for historical I or E.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedUnitLocalRedundantStoreContinuationV1;
/// fn clone(value: ProductionOwnedUnitLocalRedundantStoreContinuationV1) { let _ = value.clone(); }
/// ```
pub struct ProductionOwnedUnitLocalRedundantStoreContinuationV1 {
    prefix: OwnedPrefix<ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1>,
    data: StoreData,
}

fn header<W>() -> StoreResult<usize> {
    std::mem::size_of::<W>()
        .checked_sub(std::mem::size_of::<OwnedRedundantStoreContinuationV1>())
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn added_storage(data: &StoreData, wrapper: usize, prefix_backing: usize) -> StoreResult<usize> {
    data.continuation
        .retained_storage()
        .checked_add(wrapper)
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .and_then(|n| n.checked_add(prefix_backing))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn prepare_data(
    prefix: StorePrefix<'_>,
    wrapper: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> StoreResult<StoreData> {
    // Keep the established bridge replay intact as well; both charged replays
    // are deliberate until a separately reviewed factoring removes duplication.
    prefix.replay(budget)?;
    let continuation = prepare_owned_redundant_store_continuation_v1(prefix.output(), budget)
        .map_err(StoreError::Deletion)?;
    budget.reserve_storage(continuation.retained_storage())?;
    let kernels = check_store_output(
        prefix,
        StoreDeletionView::Owned {
            input: prefix.output(),
            continuation: &continuation,
        },
        budget,
    )?;
    budget.reserve_storage(wrapper)?;
    let mut data = StoreData {
        continuation,
        kernels,
        added: 0,
    };
    data.added = added_storage(&data, wrapper, 0)?;
    Ok(data)
}

fn required(
    prefix: StorePrefix<'_>,
    data: &StoreData,
    wrapper: usize,
    prefix_backing: usize,
) -> StoreResult<usize> {
    if added_storage(data, wrapper, prefix_backing)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn replay_data(
    prefix: StorePrefix<'_>,
    data: &StoreData,
    wrapper: usize,
    prefix_backing: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> StoreResult<()> {
    store_scope(
        required(prefix, data, wrapper, prefix_backing)?,
        budget,
        |budget| {
            let fresh = check_store_output(
                prefix,
                StoreDeletionView::Owned {
                    input: prefix.output(),
                    continuation: &data.continuation,
                },
                budget,
            )?;
            if fresh != data.kernels {
                return Err(
                    E::Formal(crate::ProductionFormalMemoryErrorV1::ObligationMismatch).into(),
                );
            }
            Ok(())
        },
    )
}

impl ProductionCheckedOutputOwnerPolicy6V1 {
    /// Consumes this exact Prefix6 and computes its closed redundant-store
    /// continuation. No external candidate can be attached. The whole caller
    /// entry floor is preserved on every exit, even after a consumed failure;
    /// the caller owns its pre-existing reservation cleanup. Success returns
    /// only the additional J/header/report-row/heap receipt unreserved. The new
    /// backing is fully paid; the inherited logical floor is not relocation credit.
    pub fn continue_redundant_private_stores_v1(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> StoreResult<(
        ProductionOwnedRedundantStoreContinuationV1,
        ProductionOwnedRedundantStoreStorageV1,
    )> {
        let required = StorePrefix::Direct(&self).floor()?;
        store_scope(required, budget, |budget| {
            let wrapper = header::<ProductionOwnedRedundantStoreContinuationV1>()?;
            let mut data = prepare_data(StorePrefix::Direct(&self), wrapper, budget)?;
            let prefix = OwnedPrefix::try_new(self, budget)?;
            data.added = added_storage(&data, wrapper, prefix.retained_storage()?)?;
            let receipt = ProductionOwnedRedundantStoreStorageV1(data.added);
            Ok((
                ProductionOwnedRedundantStoreContinuationV1 { prefix, data },
                receipt,
            ))
        })
    }
}
impl ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    /// Same additional-delta contract as Direct: original N/E and historical
    /// Prefix6 remain intact, and pre-existing caller reservations are never
    /// inferred or released. Both fresh source lifetimes and final J are checked.
    pub fn continue_redundant_private_stores_v1(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> StoreResult<(
        ProductionOwnedUnitLocalRedundantStoreContinuationV1,
        ProductionOwnedRedundantStoreStorageV1,
    )> {
        let required = StorePrefix::Erased(&self).floor()?;
        store_scope(required, budget, |budget| {
            let wrapper = header::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>()?;
            let mut data = prepare_data(StorePrefix::Erased(&self), wrapper, budget)?;
            let prefix = OwnedPrefix::try_new(self, budget)?;
            data.added = added_storage(&data, wrapper, prefix.retained_storage()?)?;
            let receipt = ProductionOwnedRedundantStoreStorageV1(data.added);
            Ok((
                ProductionOwnedUnitLocalRedundantStoreContinuationV1 { prefix, data },
                receipt,
            ))
        })
    }
}

impl ProductionOwnedRedundantStoreContinuationV1 {
    /// Actual retained Direct source/history; never reconstructed from J.
    pub const fn prefix(&self) -> &ProductionCheckedOutputOwnerPolicy6V1 {
        self.prefix.get()
    }
    /// Closed actual I/J continuation and complete inert deletion observations.
    pub const fn continuation(&self) -> &OwnedRedundantStoreContinuationV1 {
        &self.data.continuation
    }
    /// Actual J, not historical Policy6 I.
    pub fn output(&self) -> &StoreOwner {
        self.data.continuation.output()
    }
    /// Fresh checked J formal reports in J's physical kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.data.kernels
    }
    /// Additional J/header/report-row/heap receipt, excluding inherited storage.
    pub const fn additional_retained_storage_v1(&self) -> usize {
        self.data.added
    }
    /// This local continuation grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Inherited minimum plus the exact added receipt; B remains separately prepaid.
    pub fn retained_input_storage_floor_v1(&self) -> StoreResult<usize> {
        required(
            StorePrefix::Direct(self.prefix()),
            &self.data,
            header::<Self>()?,
            self.prefix.retained_storage()?,
        )
    }
    /// Replay actual source/history, I/J relation and fresh J safety/report equality.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> StoreResult<()> {
        replay_data(
            StorePrefix::Direct(self.prefix()),
            &self.data,
            header::<Self>()?,
            self.prefix.retained_storage()?,
            budget,
        )
    }
}
impl ProductionOwnedUnitLocalRedundantStoreContinuationV1 {
    /// Actual retained UnitLocal original N/E source and Policy6 history.
    pub const fn prefix(&self) -> &ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
        self.prefix.get()
    }
    /// Closed actual I/J continuation and complete inert deletion observations.
    pub const fn continuation(&self) -> &OwnedRedundantStoreContinuationV1 {
        &self.data.continuation
    }
    /// Actual J, never original N, erased E, or historical I.
    pub fn output(&self) -> &StoreOwner {
        self.data.continuation.output()
    }
    /// Fresh checked J formal reports in J's physical kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.data.kernels
    }
    /// Additional J/header/report-row/heap receipt, excluding inherited storage.
    pub const fn additional_retained_storage_v1(&self) -> usize {
        self.data.added
    }
    /// This local continuation grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Inherited minimum plus the exact added receipt; B remains separately prepaid.
    pub fn retained_input_storage_floor_v1(&self) -> StoreResult<usize> {
        required(
            StorePrefix::Erased(self.prefix()),
            &self.data,
            header::<Self>()?,
            self.prefix.retained_storage()?,
        )
    }
    /// Replay actual original-source custody, I/J relation and fresh J reports.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> StoreResult<()> {
        replay_data(
            StorePrefix::Erased(self.prefix()),
            &self.data,
            header::<Self>()?,
            self.prefix.retained_storage()?,
            budget,
        )
    }
}

#[cfg(test)]
#[path = "production_checked_output_owned_redundant_store_internal_v1_tests.rs"]
mod tests;
