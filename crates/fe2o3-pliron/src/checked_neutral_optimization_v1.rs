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
        let KirNeutralOptimizationOutputV1 {
            input,
            owner,
            report,
            bridge,
            map,
            occurrences,
            storage,
        } = self;
        check_and_finish_parts(
            ObservedParts {
                input,
                owner,
                report,
                bridge,
                map,
                occurrences,
                storage,
                extra: (),
            },
            budget,
            origins,
            output_wrapper_storage_v1,
            checked_wrapper_storage_v1,
            crate::fixed_policy_v3::FixedPolicy::Historical2,
        )
        .map(|(parts, origin, receipt)| {
            let CheckedParts {
                owner,
                report,
                bridge,
                map,
                occurrences,
                input_history,
                storage,
                extra: (),
            } = parts;
            (
                CheckedNeutralKernelIrOwnerV1 {
                    owner,
                    report,
                    bridge,
                    map,
                    occurrences,
                    input_history,
                    storage,
                },
                origin,
                receipt,
            )
        })
    }
}

// Private owned fields shared by two distinct public owner types. The unit
// historical extra never carries a policy-3 execution record through V1 custody.
pub(super) struct ObservedParts<'input, M, X, O = Owner, B = KirBridgeOptimizedReceiptV1> {
    pub(super) input: &'input O,
    pub(super) owner: O,
    pub(super) report: PlironOptimizationReportV1,
    pub(super) bridge: B,
    pub(super) map: M,
    pub(super) occurrences: KirNeutralOccurrenceRowsV1,
    pub(super) storage: super::KirNeutralOptimizationStorageV1,
    pub(super) extra: X,
}
pub(super) struct CheckedParts<M, X, O = Owner, B = KirBridgeOptimizedReceiptV1> {
    pub(super) owner: O,
    pub(super) report: PlironOptimizationReportV1,
    pub(super) bridge: B,
    pub(super) map: M,
    pub(super) occurrences: KirNeutralOccurrenceRowsV1,
    pub(super) input_history: Vec<u8>,
    pub(super) storage: KirCheckedNeutralOptimizationStorageV1,
    pub(super) extra: X,
}
pub(super) fn check_and_finish_parts<M, X, T: 'static, E: 'static, F>(
    parts: ObservedParts<'_, M, X>,
    budget: &mut Budget<'_>,
    origins: F,
    observed_wrapper: fn() -> Option<usize>,
    checked_wrapper: fn() -> Option<usize>,
    policy: crate::fixed_policy_v3::FixedPolicy,
) -> Result<
    (CheckedParts<M, X>, T, KirNeutralOwnedOriginStorageV1),
    KirCheckedNeutralOptimizationErrorV1<E>,
>
where
    F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
        &'view CheckedCanonicalKirTransitionV1<'inventory, 'input, 'output, 'rows>,
        &mut Budget<'work>,
    ) -> Result<(T, usize), E>,
{
    check_and_finish_parts_typed(
        parts, budget, origins, observed_wrapper, checked_wrapper, policy,
        AdoptionProfile {
            bytes: |owner| owner.canonical().canonical_bytes(),
            inventory: |owner, budget| CanonicalKirInventoryV1::derive(owner, budget),
            transition: check_canonical_kir_transition_v1,
            bounded_cleanup: false,
        },
    )
}

// Closed nominal facades select these adapters; callers cannot provide one.
pub(super) struct AdoptionProfile<O> {
    pub(super) bytes: for<'a> fn(&'a O) -> &'a [u8],
    pub(super) inventory: for<'g, 'b, 'w> fn(
        &'g O, &'b mut Budget<'w>,
    ) -> Result<(CanonicalKirInventoryV1<'g, O>, fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1), CanonicalKirInventoryErrorV1>,
    pub(super) transition: for<'a, 'i, 'o, 'r, 'b, 'w> fn(
        &'a CanonicalKirInventoryV1<'i, O>, &'a CanonicalKirInventoryV1<'o, O>,
        fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'r>, &'b mut Budget<'w>,
    ) -> Result<(CheckedCanonicalKirTransitionV1<'a, 'i, 'o, 'r, O>, fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1), CanonicalKirTransitionErrorV1>,
    pub(super) bounded_cleanup: bool,
}

pub(super) fn check_and_finish_parts_typed<O, B, M, X, T: 'static, E: 'static, F>(
    parts: ObservedParts<'_, M, X, O, B>,
    budget: &mut Budget<'_>,
    origins: F,
    observed_wrapper: fn() -> Option<usize>,
    checked_wrapper: fn() -> Option<usize>,
    policy: crate::fixed_policy_v3::FixedPolicy,
    admission: AdoptionProfile<O>,
) -> Result<(CheckedParts<M, X, O, B>, T, KirNeutralOwnedOriginStorageV1), KirCheckedNeutralOptimizationErrorV1<E>>
where
    F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
        &'view CheckedCanonicalKirTransitionV1<'inventory, 'input, 'output, 'rows, O>,
        &mut Budget<'work>,
    ) -> Result<(T, usize), E>,
{
    let ledger = budget.work_ledger_identity_v1();
    let observed_storage = parts.storage.retained_storage();
    let Some(floor) = budget.storage().checked_sub(observed_storage) else {
        drop(parts);
        return Err(Resource::Accounting.into());
    };
    let mut active_callback_floor = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        if admission.bounded_cleanup {
            budget.charge_work(1 + crate::kir_bridge_v1::BOUNDED_PAYLOAD_CLEANUP_ATTEMPTS_V1)?;
            budget.reserve_storage(bounded_adoption_headers::<O, B, M, X, T, E, F>()?)?;
        }
        budget.charge_work(1)?;
        let scratch_floor = budget.storage();
        let (origin_owner, origin_storage) = {
            let (input, input_storage) = (admission.inventory)(parts.input, budget)
                .map_err(KirCheckedNeutralOptimizationErrorV1::Inventory)?;
            budget.reserve_storage(input_storage.retained_storage())?;
            let (output, output_storage) = (admission.inventory)(&parts.owner, budget)
                .map_err(KirCheckedNeutralOptimizationErrorV1::Inventory)?;
            budget.reserve_storage(output_storage.retained_storage())?;
            let (checked, checked_storage) = (admission.transition)(
                &input,
                &output,
                parts.occurrences.candidate(),
                budget,
            )
            .map_err(KirCheckedNeutralOptimizationErrorV1::Transition)?;
            budget.reserve_storage(checked_storage.retained_storage())?;
            let callback_floor = budget.storage();
            active_callback_floor = Some(callback_floor);
            let callback = origins(&checked, budget);
            if budget.work_ledger_identity_v1() != ledger {
                drop(callback);
                return Err(Resource::Accounting.into());
            }
            if budget.storage() != callback_floor {
                return match callback {
                    Err(error) if admission.bounded_cleanup => {
                        Err(KirCheckedNeutralOptimizationErrorV1::Origin(error))
                    }
                    callback => {
                        drop(callback);
                        Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting)
                    }
                };
            }
            active_callback_floor = None;
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
        let wrapper = checked_wrapper().ok_or(Resource::Arithmetic)?;
        // The old observed wrapper still coexists until its fields move.
        budget.reserve_storage(wrapper)?;
        let bytes = (admission.bytes)(parts.input);
        budget.charge_work(bytes.len())?;
        budget.reserve_storage(bytes.len())?;
        let mut input_history = Vec::new();
        if policy != crate::fixed_policy_v3::FixedPolicy::Historical2 {
            budget.charge_work(2)?;
        }
        input_history
            .try_reserve_exact(bytes.len())
            .map_err(|_| Resource::Allocation)?;
        if input_history.capacity() != bytes.len() {
            if policy != crate::fixed_policy_v3::FixedPolicy::Historical2 {
                let excess = input_history
                    .capacity()
                    .checked_sub(bytes.len())
                    .ok_or(Resource::Accounting)?;
                budget.reserve_storage(excess)?;
            }
            return Err(Resource::Accounting.into());
        }
        input_history.extend_from_slice(bytes);
        let retained = observed_storage
            .checked_sub(observed_wrapper().ok_or(Resource::Arithmetic)?)
            .and_then(|amount| amount.checked_add(wrapper))
            .and_then(|amount| amount.checked_add(input_history.capacity()))
            .ok_or(Resource::Arithmetic)?;
        let ObservedParts {
            input: _,
            owner,
            report,
            bridge,
            map,
            occurrences,
            storage: _,
            extra,
        } = parts;
        Ok((
            CheckedParts {
                owner,
                extra,
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
            if admission.bounded_cleanup {
                crate::kir_bridge_v1::discard_bounded_payload_v1(payload);
            } else {
                drop(payload);
            }
            Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
        }
    };
    // A hostile callback may replace the borrowed ledger before returning or
    // panicking. Refuse custody without charging or releasing that foreign meter.
    if budget.work_ledger_identity_v1() != ledger {
        drop_adoption_result(result, admission.bounded_cleanup);
        return Err(Resource::Accounting.into());
    }
    // A mismatched callback floor may represent a nested sticky refusal. Its
    // residual reservations cannot be refunded by this enclosing transaction.
    // A balanced callback panic still permits normal cleanup after unwind.
    if admission.bounded_cleanup
        && active_callback_floor.is_some_and(|expected| budget.storage() != expected)
    {
        return result;
    }
    // No allocation or callback may intervene after this explicit output
    // transfer. Rejected owners were dropped by the unwound/returned scope.
    if let Err(error) = restore_floor(budget, floor) {
        drop_adoption_result(result, admission.bounded_cleanup);
        return Err(error.into());
    }
    result
}

fn drop_adoption_result<T>(value: T, bounded: bool) {
    if bounded {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
            crate::kir_bridge_v1::discard_bounded_payload_v1(payload);
        }
    } else {
        drop(value);
    }
}

fn bounded_adoption_headers<O, B, M, X, T, E, F>() -> Result<usize, Resource> {
    type Payload = Box<dyn std::any::Any + Send>;
    let slots = [
        (2, size_of::<ObservedParts<'_, M, X, O, B>>()),
        (2, size_of::<CheckedParts<M, X, O, B>>()),
        (8, size_of::<Result<(CheckedParts<M, X, O, B>, T, KirNeutralOwnedOriginStorageV1), KirCheckedNeutralOptimizationErrorV1<E>>>()),
        (2, size_of::<std::thread::Result<Result<(CheckedParts<M, X, O, B>, T, KirNeutralOwnedOriginStorageV1), KirCheckedNeutralOptimizationErrorV1<E>>>>()),
        (4, size_of::<Payload>()),
        (1, size_of::<AssertUnwindSafe<Payload>>()),
        (2, size_of::<std::thread::Result<()>>()),
        (1, size_of::<AdoptionProfile<O>>()),
        (1, size_of::<F>()),
        (2, size_of::<Result<(T, usize), E>>()),
        (4, size_of::<usize>()),
        (1, size_of::<Option<usize>>()),
    ];
    slots.into_iter().try_fold(0usize, |total, (count, size)| {
        size.checked_mul(count).and_then(|bytes| total.checked_add(bytes))
            .ok_or(Resource::Arithmetic)
    })
}

#[cfg(test)]
#[path = "checked_neutral_optimization_v1_tests.rs"]
mod tests;
