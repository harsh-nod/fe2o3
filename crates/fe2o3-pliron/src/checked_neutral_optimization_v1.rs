//! Consuming adoption of the actual observed neutral successor after the fixed
//! independent occurrence checker. This is local-rule custody, not authority.

use super::{
    KirBridgeOptimizedReceiptV1, KirNeutralOccurrenceRowsV1, KirNeutralOptimizationOutputV1,
    KirOptimizationMapV12, PlironOptimizationReportV1, output_wrapper_storage_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, CanonicalKirTransitionErrorV1,
    CheckedCanonicalKirTransitionV1, check_canonical_kir_transition_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    convert::Infallible,
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub enum KirCheckedNeutralOptimizationErrorV1<E = Infallible> {
    Inventory(CanonicalKirInventoryErrorV1),
    Transition(CanonicalKirTransitionErrorV1),
    Resource(Resource),
    Origin(E),
    /// The origin adapter did not restore its incoming storage floor, or its
    /// transfer omitted the inline owned result. Such a result is never adopted.
    OriginAccounting,
    Panicked,
}
impl<E: fmt::Display> fmt::Display for KirCheckedNeutralOptimizationErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(f),
            Self::Transition(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Origin(error) => write!(f, "checked neutral origin transport failed: {error}"),
            Self::OriginAccounting => {
                f.write_str("checked neutral origin storage accounting failed")
            }
            Self::Panicked => f.write_str("checked neutral adoption panicked"),
        }
    }
}
impl<E: Error + 'static> Error for KirCheckedNeutralOptimizationErrorV1<E> {}
impl<E> From<Resource> for KirCheckedNeutralOptimizationErrorV1<E> {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

/// Move-only custody of the actual optimized owner, checked under the fixed
/// named scalar/CFG rules. The native input bytes are historical audit data;
/// they are not a second connected executable and cannot replace `owner()`.
/// This establishes no ranked proof, general equivalence or runtime authority.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
/// fn duplicate(owner: CheckedNeutralKernelIrOwnerV1) {
///     let _ = owner.clone();
/// }
/// ```
pub struct CheckedNeutralKernelIrOwnerV1 {
    owner: Owner,
    report: PlironOptimizationReportV1,
    bridge: KirBridgeOptimizedReceiptV1,
    map: KirOptimizationMapV12,
    occurrences: KirNeutralOccurrenceRowsV1,
    input_history: Vec<u8>,
    storage: KirCheckedNeutralOptimizationStorageV1,
}

/// Inert logical storage receipt for the checked owner, all retained history,
/// and its wrapper. It does not account for a separately returned origin owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirCheckedNeutralOptimizationStorageV1 {
    retained: usize,
}
impl KirCheckedNeutralOptimizationStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Inert transfer receipt for the callback's owned result plus this receipt's
/// inline header. It certifies neither the origin contents nor their semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirNeutralOwnedOriginStorageV1 {
    retained: usize,
}
impl KirNeutralOwnedOriginStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
    pub const fn grants_authority(self) -> bool {
        false
    }
}

impl CheckedNeutralKernelIrOwnerV1 {
    pub const fn owner(&self) -> &Owner {
        &self.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.report
    }
    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapV12 {
        &self.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.occurrences
    }
    /// Exact native V12 input bytes retained only for historical import audit.
    pub fn native_input_audit_bytes(&self) -> &[u8] {
        &self.input_history
    }
    /// Reserve before any subsequent controlled allocation while this owner
    /// lives, and release only after drop or another explicit custody transfer.
    pub const fn storage(&self) -> KirCheckedNeutralOptimizationStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn checked_wrapper_storage_v1() -> Option<usize> {
    let mut remainder = size_of::<CheckedNeutralKernelIrOwnerV1>();
    for header in [
        size_of::<Owner>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeOptimizedReceiptV1>(),
        size_of::<KirOptimizationMapV12>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ] {
        remainder = remainder.checked_sub(header)?;
    }
    Some(remainder)
}

fn restore_floor(budget: &mut Budget<'_>, floor: usize) -> Result<(), Resource> {
    if budget.storage() < floor {
        // A rejected adapter may have released a caller-owned reservation.
        // Restoring a previously admitted floor cannot require a larger peak.
        budget.reserve_storage(floor - budget.storage())
    } else {
        budget.release_storage(budget.storage() - floor)
    }
}

impl KirNeutralOptimizationOutputV1<'_> {
    /// Consume observed output into owned fixed-rule checked custody, without
    /// a source-origin adapter. All resource behavior matches the scoped method.
    pub fn try_check_and_finish_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<CheckedNeutralKernelIrOwnerV1, KirCheckedNeutralOptimizationErrorV1> {
        self.try_check_and_finish_with_v1(budget, |_, _| Ok::<_, Infallible>(((), 0)))
            .map(|(owner, (), _)| owner)
    }

    /// Independently check this observed output against its exact original
    /// input, then optionally transport source origins through a scoped view.
    /// The adapter cannot substitute a graph, select rules, or establish this
    /// typestate without the fixed checker succeeding first.
    ///
    /// The caller must already reserve the input, this observed output's exact
    /// receipt, and any separately retained graph session. This method consumes
    /// the observed reservation. Every exit restores the incoming floor MINUS
    /// that reservation after dropping scratch and rejected owners. Success
    /// transfers the returned checked owner and owned origin result: reserve
    /// BOTH returned receipts before any subsequent controlled allocation.
    /// Work, peak and first resource failure remain on this same ledger.
    ///
    /// The adapter must reserve its own scratch/result before allocation,
    /// restore its incoming floor on every returned exit, and return `(owned, storage)`
    /// with storage including `size_of::<T>()` and all retained logical payload.
    /// Its transfer is reserved immediately, before this method allocates audit
    /// bytes. A changed callback floor or undersized result receipt rejects.
    /// `T: 'static` excludes both borrowed checked-view data and captured input
    /// borrows; no self-referential inventory or checked view is retained.
    /// Unwinding drops callback locals before this method restores the floor.
    /// Host allocator overhead and caller error diagnostics remain excluded.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::KirNeutralOptimizationOutputV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// fn escape(
    ///     observed: KirNeutralOptimizationOutputV1<'_>,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    /// ) {
    ///     let _ = observed.try_check_and_finish_with_v1(budget, |checked, _| {
    ///         Ok::<_, ()>((checked.output(), std::mem::size_of::<usize>()))
    ///     });
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::KirNeutralOptimizationOutputV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// fn escape_original(
    ///     observed: KirNeutralOptimizationOutputV1<'_>,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    /// ) {
    ///     let input = observed.input();
    ///     let _ = observed.try_check_and_finish_with_v1(budget, |_, _| {
    ///         Ok::<_, ()>((input, std::mem::size_of::<usize>()))
    ///     });
    /// }
    /// ```
    pub fn try_check_and_finish_with_v1<T: 'static, E: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        origins: F,
    ) -> Result<
        (
            CheckedNeutralKernelIrOwnerV1,
            T,
            KirNeutralOwnedOriginStorageV1,
        ),
        KirCheckedNeutralOptimizationErrorV1<E>,
    >
    where
        F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
            &'view CheckedCanonicalKirTransitionV1<'inventory, 'input, 'output, 'rows>,
            &mut Budget<'work>,
        )
            -> Result<(T, usize), E>,
    {
        let observed_storage = self.storage.retained_storage();
        let Some(floor) = budget.storage().checked_sub(observed_storage) else {
            drop(self);
            return Err(Resource::Accounting.into());
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.charge_work(1)?;
            let scratch_floor = budget.storage();
            let (origin_owner, origin_storage) = {
                let (input, input_storage) = CanonicalKirInventoryV1::derive(self.input, budget)
                    .map_err(KirCheckedNeutralOptimizationErrorV1::Inventory)?;
                budget.reserve_storage(input_storage.retained_storage())?;
                let (output, output_storage) = CanonicalKirInventoryV1::derive(&self.owner, budget)
                    .map_err(KirCheckedNeutralOptimizationErrorV1::Inventory)?;
                budget.reserve_storage(output_storage.retained_storage())?;
                let (checked, checked_storage) = check_canonical_kir_transition_v1(
                    &input,
                    &output,
                    self.occurrences.candidate(),
                    budget,
                )
                .map_err(KirCheckedNeutralOptimizationErrorV1::Transition)?;
                budget.reserve_storage(checked_storage.retained_storage())?;
                let callback_floor = budget.storage();
                let callback = origins(&checked, budget);
                if budget.storage() != callback_floor {
                    drop(callback);
                    return Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting);
                }
                let (owner, retained) =
                    callback.map_err(KirCheckedNeutralOptimizationErrorV1::Origin)?;
                if retained < size_of::<T>() {
                    drop(owner);
                    return Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting);
                }
                budget.reserve_storage(retained)?;
                (owner, retained)
            };
            // Inventories and the checked borrow have ended, but the callback's
            // owned transfer stays reserved throughout subsequent allocation.
            let scratch = budget
                .storage()
                .checked_sub(scratch_floor)
                .and_then(|live| live.checked_sub(origin_storage))
                .ok_or(Resource::Accounting)?;
            budget.release_storage(scratch)?;
            budget.reserve_storage(size_of::<KirNeutralOwnedOriginStorageV1>())?;
            let origin_storage = origin_storage
                .checked_add(size_of::<KirNeutralOwnedOriginStorageV1>())
                .ok_or(Resource::Arithmetic)?;
            let wrapper = checked_wrapper_storage_v1().ok_or(Resource::Arithmetic)?;
            // The old observed wrapper still coexists until its fields move.
            budget.reserve_storage(wrapper)?;
            let bytes = self.input.canonical().canonical_bytes();
            budget.charge_work(bytes.len())?;
            budget.reserve_storage(bytes.len())?;
            let mut input_history = Vec::new();
            input_history
                .try_reserve_exact(bytes.len())
                .map_err(|_| Resource::Allocation)?;
            if input_history.capacity() != bytes.len() {
                return Err(Resource::Accounting.into());
            }
            input_history.extend_from_slice(bytes);
            let retained = observed_storage
                .checked_sub(output_wrapper_storage_v1().ok_or(Resource::Arithmetic)?)
                .and_then(|amount| amount.checked_add(wrapper))
                .and_then(|amount| amount.checked_add(input_history.capacity()))
                .ok_or(Resource::Arithmetic)?;
            let KirNeutralOptimizationOutputV1 {
                input: _,
                owner,
                report,
                bridge,
                map,
                occurrences,
                storage: _,
            } = self;
            Ok((
                CheckedNeutralKernelIrOwnerV1 {
                    owner,
                    report,
                    bridge,
                    map,
                    occurrences,
                    input_history,
                    storage: KirCheckedNeutralOptimizationStorageV1 { retained },
                },
                origin_owner,
                KirNeutralOwnedOriginStorageV1 {
                    retained: origin_storage,
                },
            ))
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
            }
        };
        // No allocation or callback may intervene after this explicit output
        // transfer. Rejected owners were dropped by the unwound/returned scope.
        if let Err(error) = restore_floor(budget, floor) {
            drop(result);
            return Err(error.into());
        }
        result
    }
}

#[cfg(test)]
#[path = "checked_neutral_optimization_v1_tests.rs"]
mod tests;
