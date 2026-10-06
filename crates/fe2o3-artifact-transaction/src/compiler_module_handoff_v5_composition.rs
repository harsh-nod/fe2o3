//! Explicit custody composition on a larger ORIGINAL owned resource account.
//! Bounds canonical payloads and counted logical storage, not every filesystem
//! allocation. Ordinary-path opening inherits environment/path-guard allocation
//! before its later capacity check. The native root uses its retained proc-FD
//! path, which does not consult that guard configuration. A future path-free
//! entry can reuse RetainedDurableDirectoryV1 and the same recovery/mint engine.
use super::*;
use crate::ArtifactLockRetirementBarrierV1 as Barrier;
use crate::{
    CompilerExecutionSubjectErrorV3 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V3 as SUBJECT_SCRATCH,
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as AccountIdentity,
    CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
};
use std::sync::Weak;

type Quote = CompilerModuleHandoffCurrentnessCustodyQuoteV5;
type Lease = CompilerModuleHandoffCurrentnessLeaseV5;
type Token = CompilerModuleHandoffConsumptionTokenV5;
type Charge = CompilerModuleHandoffStorageV5;

/// Additional fixed work for EACH composition window, above ordinary V5 quotes.
pub const COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_WORK_V5: usize =
    Budget::STORAGE_WINDOW_WORK_V1 + 8 + 4 * 1088;
/// Additional fixed scratch for EACH window; the complete quote is ALSO charged.
pub const COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_SCRATCH_V5: usize =
    Budget::STORAGE_WINDOW_SCRATCH_V1
        + 4 * size_of::<CompilerModuleHandoffCustodyResourcesV5>()
        + 4 * size_of::<CompilerModuleHandoffCustodyScopeV5<'static, 'static, 'static>>()
        + 4096;
const WORK: usize = COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_WORK_V5;
const FRAME: usize = COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_SCRATCH_V5;

#[derive(Clone, Copy)]
struct Account {
    storage: AccountIdentity,
    work: WorkIdentity,
    address: usize,
}
impl Account {
    fn capture(b: &Budget<'_>) -> Result<Self> {
        Ok(Self {
            storage: b
                .storage_account_identity_v1()
                .ok_or(Resource::Accounting)?,
            work: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
        })
    }
    fn require(&self, b: &Budget<'_>) -> Result<()> {
        if b.storage_account_identity_v1() != Some(self.storage)
            || b.work_ledger_identity_v1() != self.work
            || b as *const Budget<'_> as usize != self.address
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

/// Move-only resource composition for one exact quoted lease/token acquisition.
/// Not a reservation or authority. The caller keeps the full quote prepaid in
/// its actual custody owner. Every operation additionally prepays that SAME full
/// quote as conservative temporary overlap inside an artifact-local <=256 MiB
/// window. No existing reservation is subtracted or reclassified as external.
///
/// Acquisition is one-shot even on refusal/unwind. Only the exact Current created
/// here may supply its token or later revalidation. Keep the original account,
/// Budget address and Work borrow alive for operations. Drop performs no budget
/// access and can accompany independently funded terminal custody.
///
/// This is counted-account composition, not a universal preallocation bound for
/// ordinary paths or environment inputs. The existing ordinary-path guard may
/// allocate configuration before checking its retained capacity. Native root
/// custody uses a retained proc-FD path and bypasses that configuration path.
pub struct CompilerModuleHandoffCustodyResourcesV5 {
    quote: Quote,
    account: Account,
    current: Option<Weak<currentness::Current<Schema>>>,
    started: bool,
    lease_attempted: bool,
    token_attempted: bool,
    token_created: bool,
}

impl CompilerModuleHandoffCustodyResourcesV5 {
    /// Fixed metered configuration work, separately included by enclosing callers.
    pub const PREPARE_WORK: usize = 8;

    /// Retains inert resource coordinates only. No lock or compiler is admitted.
    /// Self's header is additional to quote.retained_storage(). All operations
    /// require an original Owned::with_budget view, not a fresh borrowed budget.
    pub fn prepare(quote: Quote, b: &mut Budget<'_>) -> Result<Self> {
        b.charge_work(Self::PREPARE_WORK)?;
        let account = Account::capture(b)?;
        acquisition_allowance(quote)?;
        Ok(Self {
            quote,
            account,
            current: None,
            started: false,
            lease_attempted: false,
            token_attempted: false,
            token_created: false,
        })
    }

    pub const fn quote(&self) -> Quote {
        self.quote
    }

    /// ONE nonrenewable acquisition window covers both constructors and their
    /// accumulating retained charges. Install each returned owner in already
    /// funded custody BEFORE reserve_retained or any other fallible operation.
    /// All outputs are unreserved after this callback's scratch scope exits;
    /// the independently funded enclosing owner must cover them on every exit.
    /// The borrowed barrier outlives every constructor/scope rollback.
    pub fn with_acquisition<T>(
        &mut self,
        barrier: &Barrier,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&mut CompilerModuleHandoffCustodyScopeV5<'_, '_, '_>) -> Result<T>,
    ) -> Result<T> {
        self.account.require(b)?;
        if self.started || !barrier.guards_artifact_locks() {
            return Err(Resource::Accounting.into());
        }
        self.started = true;
        let quote = self.quote;
        with_overlap(
            b,
            quote.retained_storage(),
            acquisition_allowance(quote)?,
            |b| {
                let mut scope = CompilerModuleHandoffCustodyScopeV5 {
                    owner: self,
                    barrier,
                    budget: b,
                };
                operation(&mut scope)
            },
        )
    }

    /// Rechecks the actual pair on its original account, including the FULL
    /// stored quote in the local allowance. Never imports equal receipt bytes.
    pub fn revalidate(&self, lease: &Lease, token: &Token, b: &mut Budget<'_>) -> Result<()> {
        self.account.require(b)?;
        self.require_pair(lease, token)?;
        let scratch = self.quote.currentness_revalidation_quota()?.scratch();
        let allowance = checked_allowance(&[self.quote.retained_storage(), FRAME, scratch])?;
        with_overlap(b, self.quote.retained_storage(), allowance, |b| {
            entry_composed(b, self.quote.retained_storage(), |r| {
                token.revalidate_locked_in(r)
            })
        })
    }

    /// Reconstructs only inert subject bytes from this exact live pair, using
    /// the ordinary codec/fees plus complete quoted-owner overlap. The result
    /// is additional and unreserved; native association and POST-join locked
    /// currentness checks remain mandatory at the caller's existing boundary.
    pub fn subject(
        &self,
        lease: &Lease,
        token: &Token,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(Subject, SubjectStorage), SubjectError> {
        self.account.require(b).map_err(|_| Resource::Accounting)?;
        self.require_pair(lease, token)
            .map_err(|_| Resource::Accounting)?;
        let allowance = checked_allowance(&[self.quote.retained_storage(), FRAME, SUBJECT_SCRATCH])
            .map_err(|error| match error {
                Error::Resource(r) => r,
                _ => Resource::Accounting,
            })?;
        with_overlap(b, self.quote.retained_storage(), allowance, |b| {
            Subject::from_publication_in_custody(self.quote.receipt(), token.handoff(), b)
        })
    }

    fn require_lease(&self, lease: &Lease) -> Result<()> {
        if !self
            .current
            .as_ref()
            .is_some_and(|current| std::ptr::eq(current.as_ptr(), Arc::as_ptr(&lease.binding)))
            || lease.receipt() != self.quote.receipt()
            || lease.storage() != self.quote.lease_storage()
        {
            return Err(Error::MismatchedCurrentnessToken);
        }
        Ok(())
    }
    fn require_pair(&self, lease: &Lease, token: &Token) -> Result<()> {
        self.require_lease(lease)?;
        if !self.token_created || token.storage() != self.quote.token_storage() {
            return Err(Error::MismatchedCurrentnessToken);
        }
        lease.validate_current_token(token)
    }
}

/// Scoped operations only; no Budget, base, allowance selector or lock extraction.
pub struct CompilerModuleHandoffCustodyScopeV5<'scope, 'budget, 'work> {
    owner: &'scope mut CompilerModuleHandoffCustodyResourcesV5,
    barrier: &'scope Barrier,
    budget: &'budget mut Budget<'work>,
}
impl CompilerModuleHandoffCustodyScopeV5<'_, '_, '_> {
    pub fn acquire_lease(
        &mut self,
        output: &Path,
        producer: &ProducerIdentity,
    ) -> Result<(Lease, Charge)> {
        self.owner.account.require(self.budget)?;
        if self.owner.lease_attempted {
            return Err(Resource::Accounting.into());
        }
        self.owner.lease_attempted = true;
        let quote = self.owner.quote;
        let result = entry_composed(self.budget, quote.retained_storage(), |r| {
            custody::acquire_quoted(output, producer, &quote, self.barrier, r)
        })?;
        self.owner.current = Some(Arc::downgrade(&result.0.binding));
        Ok(result)
    }
    pub fn acquire_token(&mut self, lease: &Lease) -> Result<(Token, Charge)> {
        self.owner.account.require(self.budget)?;
        self.owner.require_lease(lease)?;
        if self.owner.token_attempted {
            return Err(Resource::Accounting.into());
        }
        self.owner.token_attempted = true;
        let quote = self.owner.quote;
        let result = entry_composed(self.budget, quote.retained_storage(), |r| {
            lease.acquire_quoted_in(&quote, self.barrier, r)
        })?;
        self.owner.token_created = true;
        Ok(result)
    }
    /// Call only after installing the returned owner in independently funded custody.
    pub fn reserve_retained(&mut self, charge: Charge) -> Result<()> {
        self.owner.account.require(self.budget)?;
        if charge != self.owner.quote.lease_storage() && charge != self.owner.quote.token_storage()
        {
            return Err(Resource::Accounting.into());
        }
        self.budget.reserve_storage(charge.retained_storage())?;
        Ok(())
    }
    pub fn revalidate(&mut self, lease: &Lease, token: &Token) -> Result<()> {
        self.owner.account.require(self.budget)?;
        self.owner.require_pair(lease, token)?;
        entry_composed(self.budget, self.owner.quote.retained_storage(), |r| {
            token.revalidate_locked_in(r)
        })
    }
}

fn checked_allowance(parts: &[usize]) -> Result<usize> {
    let total = parts.iter().try_fold(0usize, |n, part| {
        n.checked_add(*part).ok_or(Resource::Arithmetic)
    })?;
    if total > MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 {
        return Err(Resource::Accounting.into());
    }
    Ok(total)
}
fn acquisition_allowance(quote: Quote) -> Result<usize> {
    checked_allowance(&[
        quote.retained_storage(),
        FRAME,
        // Both explicit returned charges coexist inside this SINGLE window.
        quote.retained_storage(),
        quote
            .lease_acquisition_quota()?
            .scratch()
            .max(quote.token_acquisition_quota()?.scratch())
            .max(quote.currentness_revalidation_quota()?.scratch()),
    ])
}
fn with_overlap<T, E: From<Resource>>(
    b: &mut Budget<'_>,
    owners: usize,
    allowance: usize,
    f: impl FnOnce(&mut Budget<'_>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    let floor = b.storage();
    b.with_additional_storage_window_v1(allowance, |b| {
        b.with_prepaid_scope(
            floor,
            0,
            WORK - Budget::STORAGE_WINDOW_WORK_V1,
            owners.checked_add(FRAME).ok_or(Resource::Arithmetic)?,
            f,
        )
    })
}

// Only the closed composition calls this entry. Legacy entry retains its total
// limit check, even when called inside a kernel storage window.
pub(super) fn entry_composed<T>(
    b: &mut Budget<'_>,
    floor: usize,
    f: impl FnOnce(&mut Resources<'_, '_>) -> Result<T>,
) -> Result<T> {
    Resources::Metered(b)
        .scoped(|r| {
            r.require::<Schema>()?;
            r.work(8)?;
            if r.storage() < floor {
                return Err(Resource::Accounting.into());
            }
            r.reserve(FRAME_STORAGE)?;
            Ok(f(r))
        })
        .map_err(Error::from)?
}

/// Explicit limited recovery composition on an original owned root account.
/// Uses the existing engine and an artifact-local <=256 MiB additional window;
/// original total affordability, work, peaks and denial history remain enforced.
/// The ceiling bounds V5 payload reading/decoding, not ordinary-path or environment
/// allocations inherited from the filesystem engine. The native root's retained
/// proc-FD path does not consult ordinary-path guard configuration.
pub fn try_recover_compiler_module_handoff_receipt_in_root_budget_v5(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    maximum_handoff_bytes: usize,
    barrier: &Barrier,
    b: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    let quota =
        compiler_module_handoff_try_recovery_quota_v5(output, producer, maximum_handoff_bytes)?;
    let dynamic = custody::dynamic_bound(output, producer)?;
    let allowance = checked_allowance(&[FRAME, quota.scratch()])?;
    with_overlap(b, 0, allowance, |b| {
        entry_composed(b, 0, |r| {
            custody::recover_limited(
                output,
                producer,
                attempt,
                maximum_handoff_bytes,
                dynamic,
                barrier,
                r,
            )
        })
    })
}

#[cfg(test)]
#[path = "compiler_module_handoff_v5_composition_tests.rs"]
mod tests;
