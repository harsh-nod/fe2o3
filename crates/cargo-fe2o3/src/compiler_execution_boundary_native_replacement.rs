//! Exact success-only retirement of a consumed artifact on its original account.
use super::{FRAME, Resource};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};

// No Drop/refund guard: the caller consumes actual custody before recovery,
// reserves the returned owner, then validates it before calling finish.
pub(super) struct Replacement {
    entry: usize,
    address: usize,
    ledger: Ledger,
    retired: usize,
}

impl Replacement {
    pub(super) fn begin(retired: usize, b: &mut Budget<'_>) -> Result<Self, Resource> {
        b.charge_work(8)?;
        let entry = b.storage();
        if entry < retired {
            return Err(Resource::Accounting);
        }
        let result = Self {
            entry,
            address: b as *const Budget<'_> as usize,
            ledger: b.work_ledger_identity_v1(),
            retired,
        };
        b.reserve_storage(FRAME)?;
        Ok(result)
    }

    pub(super) fn reserve(&self, charge: usize, b: &mut Budget<'_>) -> Result<(), Resource> {
        self.require(b, 0)?;
        b.reserve_storage(charge)
    }

    pub(super) fn finish(
        self,
        charge: usize,
        header: usize,
        b: &mut Budget<'_>,
    ) -> Result<(), Resource> {
        b.charge_work(8)?;
        self.require(b, charge)?;
        let scratch = FRAME.checked_sub(header).ok_or(Resource::Accounting)?;
        let release = self
            .retired
            .checked_add(scratch)
            .ok_or(Resource::Arithmetic)?;
        b.release_storage(release)
    }

    fn require(&self, b: &Budget<'_>, charge: usize) -> Result<(), Resource> {
        let expected = self
            .entry
            .checked_add(FRAME)
            .and_then(|n| n.checked_add(charge))
            .ok_or(Resource::Arithmetic)?;
        if b as *const Budget<'_> as usize != self.address
            || b.work_ledger_identity_v1() != self.ledger
            || b.storage() != expected
        {
            return Err(Resource::Accounting);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "compiler_execution_boundary_native_replacement_tests.rs"]
mod tests;
