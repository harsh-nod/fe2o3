//! Private accounting coordinates, never Worker/compiler execution authority.
use crate::{LinkOptionV1, MAX_LINK_INPUTS, MAX_LINK_OPTIONS, PinnedWorkerV1, WorkerInputV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageIdentity,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
};
use std::mem::size_of;

/// Logical Rust storage of the actual borrowed configuration owners, including
/// spare capacities. This is an inert quote, not Worker or compiler admission;
/// sealed executable pages and child limits remain separate accounting domains.
#[allow(clippy::ptr_arg)]
pub fn conditional_worker_configuration_storage_v2(
    worker: &PinnedWorkerV1,
    providers: &Vec<WorkerInputV1>,
    options: &Vec<LinkOptionV1>,
    b: &mut Budget<'_>,
) -> Result<usize, Resource> {
    b.charge_work(8)?;
    if providers.len() > MAX_LINK_INPUTS || options.len() > MAX_LINK_OPTIONS {
        return Err(Resource::Accounting);
    }
    b.charge_work(
        providers
            .len()
            .checked_add(options.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut storage = worker
        .rust_storage()
        .ok_or(Resource::Arithmetic)?
        .checked_add(2 * size_of::<Vec<u8>>())
        .and_then(|n| {
            n.checked_add(
                providers
                    .capacity()
                    .checked_mul(size_of::<WorkerInputV1>())?,
            )
        })
        .and_then(|n| n.checked_add(options.capacity().checked_mul(size_of::<LinkOptionV1>())?))
        .ok_or(Resource::Arithmetic)?;
    for input in providers {
        storage = storage
            .checked_add(input.backing_capacity())
            .ok_or(Resource::Arithmetic)?;
    }
    for option in options {
        storage = storage
            .checked_add(option.backing_capacity().ok_or(Resource::Arithmetic)?)
            .ok_or(Resource::Arithmetic)?;
    }
    Ok(storage)
}

#[allow(clippy::ptr_arg)]
pub(crate) fn replay_input_storage(
    providers: &Vec<Vec<u8>>,
    b: &mut Budget<'_>,
) -> Result<usize, Resource> {
    b.charge_work(8)?;
    if providers.len() > MAX_LINK_INPUTS {
        return Err(Resource::Accounting);
    }
    b.charge_work(providers.len())?;
    providers.iter().try_fold(
        providers
            .capacity()
            .checked_mul(size_of::<Vec<u8>>())
            .and_then(|n| n.checked_add(size_of::<Vec<Vec<u8>>>()))
            .ok_or(Resource::Arithmetic)?,
        |n, input| n.checked_add(input.capacity()).ok_or(Resource::Arithmetic),
    )
}

#[derive(Clone, Copy)]
struct Original {
    storage: StorageIdentity,
    work: WorkIdentity,
    address: usize,
}

/// Kept only in actual move-only preflight/evidence owners. No public constructor
/// or admission claim; original account comparisons cannot approve a Worker.
#[derive(Clone, Copy)]
pub(crate) struct AccountMode(Option<Original>);
impl AccountMode {
    pub(crate) const LEGACY: Self = Self(None);
    pub(crate) fn original(b: &mut Budget<'_>) -> Result<Self, Resource> {
        b.charge_work(8)?;
        Ok(Self(Some(Original {
            storage: b
                .storage_account_identity_v1()
                .ok_or(Resource::Accounting)?,
            work: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
        })))
    }
    pub(crate) const fn is_original(self) -> bool {
        self.0.is_some()
    }
    pub(crate) fn run<T, E: From<Resource>>(
        self,
        b: &mut Budget<'_>,
        inputs: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let Some(original) = self.0 else {
            return operation(b);
        };
        b.charge_work(8)?;
        if b.storage_account_identity_v1() != Some(original.storage)
            || b.work_ledger_identity_v1() != original.work
            || b as *const Budget<'_> as usize != original.address
        {
            return Err(Resource::Accounting.into());
        }
        let floor = b.storage();
        let overlap = inputs
            .checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1)
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<Self>()))
            .ok_or(Resource::Arithmetic)?;
        b.with_additional_storage_window_v1(
            fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1,
            |b| b.with_prepaid_scope(floor.max(inputs), 0, 0, overlap, operation),
        )
    }
}

#[cfg(test)]
#[path = "conditional_worker_account_tests.rs"]
mod tests;
