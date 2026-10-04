//! Shared, private profile custody for the paired release/broker transports.
//!
//! Reservations are deliberately monotone for the lifetime of an account. No
//! owner drop refunds storage, so aliases, unwinding and dropping a transfer
//! inside a budget callback cannot refund a live source or recursively lock it.
//! The receiver borrows the caller's original account; it never resets one.
use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3 as Capability;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3, COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3,
    CompilerExecutionClientProfileV3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fs::File,
    mem::size_of,
    sync::{Arc, Mutex},
};

pub(crate) type ClientProfileAccountV3 = Arc<Mutex<OwnedBudget>>;

// Conservative complete logical owner headers, including the account allocation
// and Arc counters. Capability internals are separately charged by their API.
const PROFILE_OWNER_STORAGE: usize =
    size_of::<FundedClientProfileV3>() + size_of::<Mutex<OwnedBudget>>() + 2 * size_of::<usize>();
const TRANSFER_OWNER_STORAGE: usize = size_of::<FundedProfileFileV3>();

/// Profile admission including owner headers and all six incoming/socket File
/// owners prepaid by receive_v4. This excludes executable hashing and the root
/// request's transport work, which the caller funds on this same account.
pub(crate) fn client_profile_receive_quota_v3() -> Result<(usize, usize), String> {
    let work = Capability::IO_WORK.checked_add(COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3);
    let storage = Capability::IO_STORAGE
        .checked_add(COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3)
        .and_then(|n| n.checked_add(Capability::FILE_STORAGE.checked_mul(7)?))
        .and_then(|n| n.checked_add(PROFILE_OWNER_STORAGE))
        .and_then(|n| n.checked_add(TRANSFER_OWNER_STORAGE));
    Ok((
        work.ok_or("native profile work quota overflow")?,
        storage.ok_or("native profile storage quota overflow")?,
    ))
}

/// A move-only profile and the exact persistent account that funds it. Public
/// within this binary for wrapper composition, not a profile approval API.
pub(crate) struct FundedClientProfileV3 {
    capability: Capability,
    account: ClientProfileAccountV3,
}

/// This distinct transferred descriptor stays paid alongside its source.
pub(crate) struct FundedProfileFileV3 {
    file: File,
    account: ClientProfileAccountV3,
}

impl FundedProfileFileV3 {
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
}

impl FundedClientProfileV3 {
    pub(super) fn new_release_account() -> Result<ClientProfileAccountV3, String> {
        // A finite release/broker envelope sized for this many complete profile
        // decodes. Cheaper operations consume their actual quotes cumulatively.
        // A denied invocation returns without resetting this ledger.
        const OPERATIONS: usize = 4096;
        let (decode_work, decode_storage) = client_profile_receive_quota_v3()?;
        let work = decode_work
            .checked_mul(OPERATIONS)
            .and_then(|n| n.checked_add(Capability::PRODUCTION_WORK))
            .ok_or("native release profile work overflow")?;
        let storage = Capability::IO_STORAGE
            .checked_mul(OPERATIONS)
            .and_then(|n| n.checked_add(decode_storage))
            .and_then(|n| n.checked_add(Capability::PRODUCTION_STORAGE))
            .ok_or("native release profile storage overflow")?;
        Ok(Arc::new(Mutex::new(OwnedBudget::new(
            Work::new(work),
            storage,
        ))))
    }

    pub(super) fn from_production_profile() -> Result<Self, String> {
        let account = Self::new_release_account()?;
        let capability = with_account(&account, |budget| {
            budget
                .reserve_storage(PROFILE_OWNER_STORAGE)
                .map_err(|error| error.to_string())?;
            let (capability, growth) =
                Capability::from_production_profile(budget).map_err(|error| error.to_string())?;
            budget
                .reserve_storage(growth.additional_storage())
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(capability)
        })?;
        Ok(Self {
            capability,
            account,
        })
    }

    pub(super) fn from_inherited(
        receive: impl FnOnce() -> Result<File, String>,
    ) -> Result<Self, String> {
        let account = Self::new_release_account()?;
        // The inherited FD remains live while its separately retained duplicate
        // is normalized/admitted; pay both on this child's one admission account.
        with_account(&account, |budget| {
            budget
                .reserve_storage(Capability::FILE_STORAGE)
                .map_err(|error| error.to_string())
        })?;
        Self::from_received_with(account, receive)
    }

    /// The inherited/received File is prepaid before its native constructor.
    /// The supplied account is the original invocation's account, never a quote.
    #[cfg(test)]
    pub(crate) fn from_received_file(
        file: File,
        account: ClientProfileAccountV3,
    ) -> Result<Self, String> {
        Self::from_received_with(account, || Ok(file))
    }

    pub(crate) fn from_received_with(
        account: ClientProfileAccountV3,
        receive: impl FnOnce() -> Result<File, String>,
    ) -> Result<Self, String> {
        with_account(&account, |budget| {
            budget
                .reserve_storage(
                    Capability::FILE_STORAGE
                        .checked_add(TRANSFER_OWNER_STORAGE)
                        .ok_or("native profile transfer storage overflow")?,
                )
                .map_err(|error| error.to_string())
        })?;
        let file = receive()?;
        Self::from_funded_file(FundedProfileFileV3 { file, account })
    }

    pub(super) fn from_funded_file(input: FundedProfileFileV3) -> Result<Self, String> {
        let FundedProfileFileV3 { file, account } = input;
        let capability = with_account(&account, |budget| {
            budget
                .reserve_storage(PROFILE_OWNER_STORAGE)
                .map_err(|error| error.to_string())?;
            let (capability, growth) =
                Capability::from_file(file, budget).map_err(|error| error.to_string())?;
            budget
                .reserve_storage(growth.additional_storage())
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(capability)
        })?;
        Ok(Self {
            capability,
            account,
        })
    }

    pub(crate) const fn profile(&self) -> &CompilerExecutionClientProfileV3 {
        self.capability.profile()
    }

    pub(crate) fn with_profile_budget<T>(
        &self,
        operation: impl for<'a> FnOnce(&Capability, &mut Budget<'a>) -> T,
    ) -> T {
        with_account(&self.account, |budget| operation(&self.capability, budget))
    }

    pub(crate) fn try_clone_for_transfer(&self) -> Result<FundedProfileFileV3, String> {
        let file = self.with_profile_budget(|capability, budget| {
            budget
                .reserve_storage(TRANSFER_OWNER_STORAGE)
                .map_err(|error| error.to_string())?;
            let (file, growth) = capability
                .try_clone_for_transfer(budget)
                .map_err(|error| error.to_string())?;
            budget
                .reserve_storage(growth.additional_storage())
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(file)
        })?;
        Ok(FundedProfileFileV3 {
            file,
            account: Arc::clone(&self.account),
        })
    }

    pub(crate) fn try_clone_retained(&self) -> Result<Self, String> {
        Self::from_funded_file(self.try_clone_for_transfer()?)
    }
}

fn with_account<T>(
    account: &ClientProfileAccountV3,
    operation: impl for<'a> FnOnce(&mut Budget<'a>) -> T,
) -> T {
    // Poisoning does not reset the accepted prefix or refund reservations.
    account
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .with_budget(operation)
}

#[cfg(test)]
#[path = "authority_release_profile_tests.rs"]
pub(crate) mod tests;
