//! Full decoded semantic history checked against retained genuine source.
//! Compatibility is not original source-file identity or observed execution.
#![allow(
    clippy::drop_non_drop,
    reason = "End borrowed checks before receipt release."
)]
#![allow(
    clippy::result_large_err,
    reason = "Preserve typed causes without allocation."
)]
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_kernel_opt::{DecodedExpandedHistoryV1, ExpandedHistoryErrorV1};
use std::panic::{AssertUnwindSafe, catch_unwind};
type Anchor<'a> = CanonicalOutputFormalSourceAnchorV1<'a>;

/// Full decoded relation/source refusal or the unchanged callback error.
#[derive(Debug)]
pub enum DecodedExpandedSourceErrorV1<E> {
    /// Work, storage, arithmetic, allocation or stationary-ledger custody failed.
    Resource(Resource),
    /// Complete decoded U/scalar semantic replay failed.
    History(ExpandedHistoryErrorV1),
    /// The retained source or final source/census/formal join failed.
    Source(ProductionLoopUnrollErrorV1),
    /// Inventory derivation of the actual decoded final graph failed.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// The decoded prefix differs from the closed production schedule.
    Policy,
    /// The original source does not retain the required connected executable.
    MissingConnectedSource,
    /// The callback returned its original typed failure.
    Callback(E),
    /// A checked scope or callback unwound.
    Panicked,
}
type Error<E> = DecodedExpandedSourceErrorV1<E>;
type ResultV1<T, E> = std::result::Result<T, Error<E>>;
impl<E> From<Resource> for Error<E> {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl<E: fmt::Debug> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decoded expanded source: {self:?}")
    }
}
impl<E: StdError + 'static> StdError for Error<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::History(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Callback(e) => Some(e),
            Self::Policy | Self::MissingConnectedSource | Self::Panicked => None,
        }
    }
}
struct Custody {
    ledger: Ledger,
    slot: usize,
    floor: usize,
}
impl Custody {
    fn check(&self, b: &Budget<'_>) -> std::result::Result<(), Resource> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.slot != b as *const Budget<'_> as usize
            || b.storage() < self.floor
        {
            return Err(Resource::Accounting);
        }
        Ok(())
    }
}
type Payloads = [Option<Box<dyn std::any::Any + Send>>; 2];
const GUARD: usize = size_of::<Custody>() + size_of::<Payloads>();

/// Temporary source-compatible decoded final subject, not an execution grant.
/// The anchor is retained genuine source; wire compatibility alone does not
/// authenticate its file identity. That comparison belongs to native/publication custody.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedDecodedExpandedSourceV1 as V;
/// fn clone(v: V<'_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedDecodedExpandedSourceV1 as V;
/// fn fake() -> V<'static> { Default::default() }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{CanonicalOutputFormalSourceAnchorV1 as A,
///     with_checked_decoded_expanded_source_v1 as check};
/// use fe2o3_kernel_opt::DecodedExpandedHistoryV1 as D;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as G,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// fn escape<'a>(a: A<'a>, d: &D<'_, '_>, b: &mut B<'_>) -> &'a G {
///     check(a, d, b, |v, b| v.output(b)).unwrap()
/// }
/// ```
pub struct CheckedDecodedExpandedSourceV1<'scope> {
    anchor: Anchor<'scope>,
    neutral: &'scope Graph,
    bound: &'scope Graph,
    output: &'scope Graph,
    origins: &'scope [ProductionExpandedSourceOriginV1],
    kernels: &'scope [FormalMemoryObligations],
    rounds: usize,
    custody: &'scope Custody,
}
impl CheckedDecodedExpandedSourceV1<'_> {
    fn query(&self, budget: &mut Budget<'_>) -> std::result::Result<(), Resource> {
        self.custody.check(budget)?;
        budget.charge_work(1)
    }
    /// Retains genuine Direct or Erased source, not a wire file-identity claim.
    pub fn source_anchor(&self, b: &mut Budget<'_>) -> std::result::Result<Anchor<'_>, Resource> {
        self.query(b)?;
        Ok(self.anchor)
    }
    /// Actual retained N/E before target binding.
    pub fn pre_bind(&self, b: &mut Budget<'_>) -> std::result::Result<&Graph, Resource> {
        self.query(b)?;
        Ok(self.neutral)
    }
    /// Actual decoded B, before the complete historical optimization prefix.
    pub fn bound_input(&self, b: &mut Budget<'_>) -> std::result::Result<&Graph, Resource> {
        self.query(b)?;
        Ok(self.bound)
    }
    /// Actual terminal decoded scalar graph, never historical U.
    pub fn output(&self, b: &mut Budget<'_>) -> std::result::Result<&Graph, Resource> {
        self.query(b)?;
        Ok(self.output)
    }
    /// Complete independently transported final source occurrences.
    pub fn origins(
        &self,
        b: &mut Budget<'_>,
    ) -> std::result::Result<&[ProductionExpandedSourceOriginV1], Resource> {
        self.query(b)?;
        Ok(self.origins)
    }
    /// Fresh final formal reports, in complete kernel order.
    pub fn kernels(
        &self,
        b: &mut Budget<'_>,
    ) -> std::result::Result<&[FormalMemoryObligations], Resource> {
        self.query(b)?;
        Ok(self.kernels)
    }
    /// Number of complete scalar rounds including the unchanged terminal round.
    pub fn round_count(&self, b: &mut Budget<'_>) -> std::result::Result<usize, Resource> {
        self.query(b)?;
        Ok(self.rounds)
    }
    /// Semantic compatibility is neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Decoded relation replay never attests that an optimizer executed.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
}
fn add(a: usize, b: usize) -> std::result::Result<usize, Resource> {
    a.checked_add(b).ok_or(Resource::Arithmetic)
}
fn scoped<'w, T, E>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>, &mut Payloads, &Custody) -> ResultV1<T, E>,
) -> ResultV1<T, E> {
    let custody = Custody {
        ledger: budget.work_ledger_identity_v1(),
        slot: budget as *const Budget<'w> as usize,
        floor: budget.storage(),
    };
    let paid = add(GUARD, size_of::<ResultV1<T, E>>())?;
    budget.reserve_storage(paid)?;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget, &mut payloads, &custody))) {
        Ok(v) => v,
        Err(p) => {
            payloads[0] = Some(p);
            Err(Error::Panicked)
        }
    };
    let same = custody.ledger == budget.work_ledger_identity_v1()
        && custody.slot == budget as *const Budget<'w> as usize;
    if !same || budget.storage() < add(custody.floor, paid)? {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        if let Err(p) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(p);
        }
    }
    // Inner dependents and rejected results have already dropped while paid.
    // Never replenish missing credits or release a replacement ledger.
    if same && budget.storage() >= custody.floor {
        if let Err(e) = budget.release_storage(budget.storage() - custody.floor) {
            let rejected = std::mem::replace(&mut result, Err(e.into()));
            if let Err(p) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(p);
            }
        }
    }
    drop(payloads);
    result
}

/// Replays the ENTIRE decoded history and checks actual retained source against it.
/// Source, wire, framing and decoded-graph backing must already be reserved.
/// The callback can return owned values under their own transfer-receipt protocol;
/// checked borrows cannot escape. Every query and callback return enforces the
/// original ledger, exact Budget slot and complete live callback backing.
/// This does not authenticate original file identity, optimizer execution or signing.
pub fn with_checked_decoded_expanded_source_v1<'w, T, E>(
    anchor: Anchor<'_>,
    decoded: &DecodedExpandedHistoryV1<'_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'s> FnOnce(
        CheckedDecodedExpandedSourceV1<'s>,
        &mut Budget<'w>,
    ) -> std::result::Result<T, E>,
) -> ResultV1<T, E> {
    let minimum = match anchor {
        Anchor::Direct(v) => v
            .pre_ranked_retained_analysis_storage_v1()
            .ok_or(Error::MissingConnectedSource)?,
        Anchor::Erased(v) => v.retained_storage_floor_v1(),
    };
    let minimum = add(
        minimum,
        add(
            decoded.storage().retained_storage(),
            add(
                decoded.frame().storage().retained_storage(),
                decoded.frame().canonical_bytes().len(),
            )?,
        )?,
    )?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    scoped(budget, |budget, payloads, outer| {
        budget.charge_work(31)?;
        let replayed = decoded.check_semantics(budget).map_err(Error::History)?;
        budget.reserve_storage(replayed.storage().retained_storage())?;
        outer.check(budget)?;
        let history = replayed.prefix();
        if history.limits() != UNROLL
            || history.prefix().limits().refinement != LOOPS
            || history.prefix().limits().forwarding != FORWARDING
        {
            return Err(Error::Policy);
        }
        let (inventory, receipt) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(replayed.output(), budget)
                .map_err(Error::Inventory)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let count = inventory.operations().len();
        let bytes = count
            .checked_mul(size_of::<ProductionExpandedSourceOriginV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(add(
            size_of::<Vec<ProductionExpandedSourceOriginV1>>(),
            bytes,
        )?)?;
        let mut origins = Vec::new();
        origins
            .try_reserve_exact(count)
            .map_err(|_| Error::Resource(Resource::Allocation))?;
        budget.reserve_storage(
            origins
                .capacity()
                .checked_sub(count)
                .ok_or(Resource::Accounting)?
                .checked_mul(size_of::<ProductionExpandedSourceOriginV1>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let reports_extent = replayed
            .output()
            .module()
            .kernels
            .len()
            .checked_mul(size_of::<FormalMemoryObligations>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(add(
            size_of::<Box<[FormalMemoryObligations]>>(),
            reports_extent,
        )?)?;
        let required = budget.storage();
        let kernels = ProductionOwnedLoopUnrollContinuationV1::check_replayed_expanded_sites_v1(
            anchor,
            history,
            ExpandedScalarViewV1::Decoded(replayed.scalar()),
            &mut origins,
            required,
            budget,
        )
        .map_err(Error::Source)?;
        outer.check(budget)?;
        let neutral = match anchor {
            Anchor::Direct(v) => v
                .pre_ranked_executable()
                .ok_or(Error::MissingConnectedSource)?,
            Anchor::Erased(v) => v.erased(),
        };
        let bound = history
            .prefix()
            .prefix()
            .policy7_relation()
            .policy6_relation()
            .policy5_relation()
            .policy4_relation()
            .policy3_relation()
            .semantic_receipt()
            .input();
        budget.reserve_storage(add(
            size_of::<CheckedDecodedExpandedSourceV1<'_>>(),
            size_of::<Custody>(),
        )?)?;
        let custody = Custody {
            ledger: outer.ledger,
            slot: outer.slot,
            floor: budget.storage(),
        };
        custody.check(budget)?;
        let view = CheckedDecodedExpandedSourceV1 {
            anchor,
            neutral,
            bound,
            output: replayed.output(),
            origins: &origins,
            kernels: &kernels,
            rounds: replayed.scalar().rounds().len(),
            custody: &custody,
        };
        let mut result = callback(view, budget).map_err(Error::Callback);
        if let Err(e) = custody.check(budget) {
            let rejected = std::mem::replace(&mut result, Err(Error::Resource(e)));
            if let Err(p) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(p);
            }
        }
        drop(kernels);
        drop(origins);
        drop(inventory);
        drop(replayed);
        result
    })
}
