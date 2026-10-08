//! Private outer-owner retention; no counter transition establishes native proof.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrOriginalAccountRetentionLoanV1 as Loan;

pub(in super::super) struct EpochAnchor {
    active: Cell<bool>,
    carriers: Cell<usize>,
}
impl EpochAnchor {
    pub(in super::super) fn new() -> Self {
        Self {
            active: Cell::new(false),
            carriers: Cell::new(0),
        }
    }
    pub(in super::super) fn require_idle(&self) -> Result<()> {
        require(
            !self.active.get() && self.carriers.get() == 0,
            "native invocation epoch remains active",
        )
    }
    pub(super) fn begin<'scope, 'work>(
        &'scope self,
        failed: &'scope Cell<bool>,
        budget: &mut Budget<'work>,
    ) -> Result<Epoch<'scope, 'work>> {
        self.require_idle()?;
        require(!failed.get(), "native invocation custody is terminal")?;
        let loan = budget.retain_original_account_v1()?;
        self.active.set(true);
        Ok(Epoch {
            anchor: self,
            failed,
            loan: Some(loan),
            closed: false,
        })
    }
}
impl Drop for EpochAnchor {
    fn drop(&mut self) {
        if self.active.get() || self.carriers.get() != 0 {
            std::process::abort();
        }
    }
}

pub(super) struct Epoch<'scope, 'work> {
    anchor: &'scope EpochAnchor,
    failed: &'scope Cell<bool>,
    loan: Option<Loan<'work>>,
    closed: bool,
}
impl Epoch<'_, '_> {
    pub(super) const LOAN_STORAGE: usize = Loan::RETAINED_STORAGE;

    pub(super) fn retain_carrier(&self) -> Result<CarrierRetention<'_>> {
        require(
            self.anchor.active.get() && !self.closed && !self.failed.get(),
            "native carrier outside its original active epoch",
        )?;
        let next = self
            .anchor
            .carriers
            .get()
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?;
        require(
            next <= fe2o3_runtime::MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1,
            "native invocation carrier limit",
        )?;
        self.anchor.carriers.set(next);
        Ok(CarrierRetention {
            anchor: self.anchor,
        })
    }

    pub(super) fn close(&mut self) -> Result<()> {
        require(
            self.anchor.active.get() && !self.closed && self.anchor.carriers.get() == 0,
            "native invocation retains unsettled or forgotten carriers",
        )?;
        self.closed = true;
        self.anchor.active.set(false);
        drop(self.loan.take());
        Ok(())
    }
}
impl Drop for Epoch<'_, '_> {
    fn drop(&mut self) {
        if self.anchor.carriers.get() != 0 {
            // Runs before the private account loan or any borrowed proof owner
            // can retire, even if the runtime future itself was forgotten.
            std::process::abort();
        }
        if !self.closed {
            self.failed.set(true);
            self.anchor.active.set(false);
        }
    }
}

pub(super) struct CarrierRetention<'scope> {
    anchor: &'scope EpochAnchor,
}
impl Drop for CarrierRetention<'_> {
    fn drop(&mut self) {
        let count = self.anchor.carriers.get();
        if !self.anchor.active.get() || count == 0 {
            std::process::abort();
        }
        // The private authority is destroyed on effect-free preparation refusal,
        // definite pre-adoption cancellation, actual original unpublished
        // native/Context retirement without decoding, or native/decoder settlement.
        // Cancellation does not claim a decoder result or native completion.
        self.anchor.carriers.set(count - 1);
    }
}

#[cfg(test)]
mod tests;
