//! Private nominal structural module conversion on the original phase.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionHelperSourcePolicyV1 as Policy,
    ProductionRankedSemanticProjectionRootV1 as LoweringRoot,
};

type E = ProductionRankedVerificationErrorV1;

fn resource(error: Resource) -> E {
    E::ConditionalResource(error)
}

fn sum(left: usize, right: usize) -> Result<usize, E> {
    left.checked_add(right)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn row_bytes<T>(count: usize) -> Result<usize, E> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))
}

// Requested live payload/header accounting, not allocator slack or all heap.
// Requiring the exact requested capacity keeps the later fixed conversion debit
// independent of allocator over-allocation. On failure the enclosing owning
// phase releases accepted credits only after all consumed source data is dead.
fn one_vec<T>(budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget.charge_work(1).map_err(resource)?;
    let storage = sum(std::mem::size_of::<Vec<T>>(), row_bytes::<T>(1)?)?;
    budget.reserve_storage(storage).map_err(resource)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(1)
        .map_err(|_| resource(Resource::Allocation))?;
    if rows.capacity() != 1 {
        return Err(resource(Resource::Accounting));
    }
    Ok((rows, storage))
}

fn box_one<T>(rows: Vec<T>, vector_storage: usize, budget: &mut Budget<'_>) -> Result<Box<[T]>, E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget.charge_work(1).map_err(resource)?;
    let payload = row_bytes::<T>(1)?;
    if rows.len() != 1
        || rows.capacity() != 1
        || vector_storage != sum(std::mem::size_of::<Vec<T>>(), payload)?
    {
        return Err(resource(Resource::Accounting));
    }
    // Conservatively cover a destination overlapping the still-live Vec even
    // when this allocator can reuse its buffer. Only our exact Vec reservation
    // is refunded after consuming it; original source credits are not inferred.
    budget.reserve_storage(payload).map_err(resource)?;
    let boxed = rows.into_boxed_slice();
    budget.release_storage(vector_storage).map_err(resource)?;
    Ok(boxed)
}

fn reserve_destination_maps(
    root: &ProductionRankedVerifiedRootCandidateV1,
    budget: &mut Budget<'_>,
) -> Result<usize, E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget
        .charge_work(sum(
            2,
            sum(
                root.access_sources.len(),
                root.executable_effect_sources.len(),
            )?,
        )?)
        .map_err(resource)?;
    let payload = sum(
        row_bytes::<ProductionRankedAccessSourceV1>(root.access_sources.len())?,
        row_bytes::<ProductionRankedExecutableEffectSourceV1>(
            root.executable_effect_sources.len(),
        )?,
    )?;
    // LoweringRoot::new consumes both actual Vecs into boxed maps. The original
    // phase floor continues covering their input buffers while this reserves
    // their possible destinations. No invented per-buffer old credit is freed.
    budget.reserve_storage(payload).map_err(resource)?;
    Ok(payload)
}

/// Source first, authenticated evidence and its original account last. There is
/// intentionally no public split, attachment, receipt mutation or token export.
#[allow(dead_code)]
pub(crate) struct PrivateBf16ModuleVerifiedReceiptV1 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    verification: AuthenticatedRankedVerificationRosterV1,
}

impl PrivateBf16ModuleVerifiedReceiptV1 {
    #[allow(dead_code)]
    pub(crate) fn root_count(&self) -> usize {
        self.receipt.root_count()
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionRankedSemanticProjectionRosterReceiptV1 {
    /// Closed nominal-only continuation. Source replay/proof engines keep their
    /// inherited limits. New singleton/container/map-conversion work and payload
    /// reservations use this same retained projection account, not UnitLocal's
    /// separate stage account. The caller's materialization account stays alive.
    #[allow(dead_code)]
    pub(crate) fn into_private_bf16_module_verified_receipt_v1(
        mut self,
    ) -> Result<PrivateBf16ModuleVerifiedReceiptV1, E> {
        self.phase.require_clean_v1()?;
        let materialized = &self.materialized;
        let source_order_roots = &self.source_order_roots;
        self.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(1).map_err(resource)?;
            if materialized.helper_source_policy_v1() != Policy::Bf16Nominal {
                return Err(E::Custody(
                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
                        LocalHelperSourceConsumerUnavailable {
                            consumer: "private nominal module conversion",
                        },
                ));
            }
            budget.charge_work(2).map_err(resource)?;
            if source_order_roots.len() != 1 {
                return Err(E::RosterMetadata(
                    "private nominal module requires one exact root",
                ));
            }
            if budget.storage()
                < materialized
                    .unit_local_source_storage_floor_v1()
                    .map_err(E::Custody)?
            {
                return Err(resource(Resource::Accounting));
            }
            Ok(())
        })?;
        // Preserve all source/binding, mandatory, functional, induction and
        // canonical identity/order checks before destroying the input roster.
        self.verify_equivalence()?;

        // Declared first: any later source/root locals die before this account.
        let mut phase = self.phase;
        let Self {
            materialized,
            source_order_roots,
            canonical_kernel_order,
            canonical_roster_identity,
            ..
        } = self;
        let (receipt, verification_roots) = phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let slot = budget as *const _ as usize;
            budget.charge_work(1).map_err(resource)?;
            let header = std::mem::size_of::<PrivateBf16ModuleVerifiedReceiptV1>();
            budget.reserve_storage(header).map_err(resource)?;
            let (mut lowering, lowering_storage) = one_vec::<LoweringRoot>(budget)?;
            let (mut verification, verification_storage) =
                one_vec::<AuthenticatedRankedVerificationRootV1>(budget)?;
            let mut map_storage = 0usize;
            // Exact one-row bound was checked above. The original boxed source
            // buffer remains prepaid by the retained floor until this phase dies.
            for root in source_order_roots.into_vec() {
                budget.charge_work(1).map_err(resource)?;
                map_storage = sum(map_storage, reserve_destination_maps(&root, budget)?)?;
                let (source, verified) = root.into_source_and_verification_v1();
                lowering.push(source);
                verification.push(verified);
            }
            let lowering = box_one(lowering, lowering_storage, budget)?;
            let verification = box_one(verification, verification_storage, budget)?;
            let receipt = ProductionMaterializedRankedModuleReceiptV1::
                from_private_bf16_projection_roster_with_budget_v1(
                    materialized, lowering, budget,
                ).map_err(E::Custody)?;
            if receipt.root_count() != 1 || verification.len() != receipt.root_count() {
                return Err(E::RosterMetadata(
                    "private nominal module changed the root roster",
                ));
            }
            let retained = sum(
                header,
                sum(
                    map_storage,
                    sum(
                        row_bytes::<LoweringRoot>(1)?,
                        row_bytes::<AuthenticatedRankedVerificationRootV1>(1)?,
                    )?,
                )?,
            )?;
            if budget as *const _ as usize != slot
                || budget.work_ledger_identity_v1() != ledger
                || budget.storage() != sum(floor, retained)?
            {
                return Err(resource(Resource::Accounting));
            }
            budget.check_prior_denials_v1().map_err(resource)?;
            Ok((receipt, verification))
        })?;
        let mut paired = PrivateBf16ModuleVerifiedReceiptV1 {
            receipt,
            verification: AuthenticatedRankedVerificationRosterV1 {
                roots: verification_roots,
                canonical_roster_identity,
                canonical_kernel_order,
                phase,
            },
        };
        paired.verification.phase.require_clean_v1()?;
        Ok(paired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn trial(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, bool, bool) {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = (|| {
            let (mut rows, storage) = one_vec::<u64>(&mut budget)?;
            rows.push(19);
            box_one(rows, storage, &mut budget)
        })();
        let ok = result.is_ok();
        if let Ok(rows) = &result {
            assert_eq!(rows.as_ref(), &[19]);
            assert_eq!(budget.storage(), 7 + std::mem::size_of::<u64>());
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        let state = (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_work().is_some(),
            budget.failed_storage().is_some(),
        );
        // The test owns these exact remaining credits; data dies before refund.
        drop(result);
        let extra = budget.storage() - 7;
        budget.release_storage(extra).unwrap();
        assert_eq!(budget.storage(), 7);
        state
    }

    #[test]
    fn private_nominal_module_destination_exact_and_one_short_boundaries() {
        let peak = 7 + std::mem::size_of::<Vec<u64>>() + 2 * std::mem::size_of::<u64>();
        assert_eq!(trial(2, peak), (true, 2, peak, false, false));
        let short_work = trial(1, peak);
        assert!(!short_work.0 && short_work.1 == 1 && short_work.3 && !short_work.4);
        let short_storage = trial(2, peak - 1);
        assert!(!short_storage.0 && short_storage.1 == 2 && !short_storage.3 && short_storage.4);
    }

    #[test]
    fn private_nominal_module_destinations_preserve_prior_denial_without_new_debit() {
        for storage_failure in [false, true] {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if storage_failure {
                assert!(budget.reserve_storage(1).is_err());
            } else {
                assert!(budget.charge_work(4).is_err());
            }
            let history = (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                let result = one_vec::<u64>(&mut budget);
                if storage_failure {
                    assert!(matches!(
                        result,
                        Err(E::ConditionalResource(Resource::Storage(_)))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(E::ConditionalResource(Resource::Work(_)))
                    ));
                }
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    history
                );
            }
        }
    }

    #[test]
    fn private_nominal_module_box_refuses_mismatched_credit_and_roster() {
        for mutation in 0..3 {
            let mut work = Work::new(10);
            let mut budget = Budget::new(&mut work, 1000);
            budget.reserve_storage(7).unwrap();
            let (mut rows, mut storage) = one_vec::<u64>(&mut budget).unwrap();
            if mutation != 0 {
                rows.push(19);
            }
            if mutation == 1 {
                storage += 1;
            }
            if mutation == 2 {
                storage -= 1;
            }
            let floor = budget.storage();
            assert!(matches!(
                box_one(rows, storage, &mut budget),
                Err(E::ConditionalResource(Resource::Accounting))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.check_prior_denials_v1().is_ok());
            budget.release_storage(floor - 7).unwrap();
        }
    }

    #[test]
    fn private_nominal_module_checked_size_arithmetic_refuses_overflow() {
        assert!(matches!(
            row_bytes::<u64>(usize::MAX),
            Err(E::ConditionalResource(Resource::Arithmetic))
        ));
        assert!(matches!(
            sum(usize::MAX, 1),
            Err(E::ConditionalResource(Resource::Arithmetic))
        ));
    }
}

#[cfg(test)]
mod module_observation {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity;

    // These snapshots never leave the single live phase loan that paid them.
    struct AccountSnapshot {
        account: WorkIdentity,
        storage: usize,
        work: usize,
    }
    struct ValidatedObservation {
        work: usize,
        storage: usize,
        retained: usize,
        peak: usize,
    }
    const OBSERVATION_STORAGE: usize =
        std::mem::size_of::<AccountSnapshot>() + std::mem::size_of::<ValidatedObservation>();

    impl ValidatedObservation {
        fn emit(self, requested: [u8; 4]) {
            eprintln!(
                "fe2o3-bf16-private-module-conversion-v1 completed=true roots=1 accesses=3 permutation={} work={} storage={} observation_storage={} retained_storage={} peak={} same_account=true source_join=true fresh_structural_validation=true cleanup_pending=true normal_admission=false attached=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                self.work,
                self.storage,
                OBSERVATION_STORAGE,
                self.retained,
                self.peak,
            );
        }
    }

    fn require_account(before: &AccountSnapshot, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(3).map_err(resource)?;
        if budget.work_ledger_identity_v1() != before.account
            || budget.storage() != before.storage
            || budget.work() <= before.work
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }

    impl ProductionRankedSemanticProgramV1 {
        /// Initial checks return no paid record across the consuming boundary.
        /// The actual owner and original phase, not detached observations,
        /// preserve source/account custody through the shipping conversion.
        pub(crate) fn observe_private_nominal_module_for_test_v1(
            self,
            requested: [u8; 4],
        ) -> Result<(), E> {
            let mut roster = self.into_verified_roster_receipt()?;
            roster.phase.require_clean_v1()?;
            let materialized = &roster.materialized;
            let roots = &roster.source_order_roots;
            roster.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(32 + 32 + 4 + 8).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "module observer requires one actual root",
                    ));
                };
                let emission =
                    materialized
                        .bf16_call_instance_emission_v1()
                        .ok_or(E::RosterMetadata(
                            "module observer has no actual nominal emission",
                        ))?;
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || !std::ptr::eq(emission.owner(), materialized)
                    || emission.root() != root.semantic_root
                    || emission.return_permutation() != requested
                    || root.access_sources.len() != 3
                    || !root.executable_effect_sources.is_empty()
                    || root.ranked_ir.is_empty()
                {
                    return Err(E::RosterMetadata(
                        "module observer actual source/maps/Return differ",
                    ));
                }
                Ok(())
            })?;
            consume_and_observe(roster, requested)
        }
    }

    pub(super) fn consume_and_observe(
        roster: ProductionRankedSemanticProjectionRosterReceiptV1,
        requested: [u8; 4],
    ) -> Result<(), E> {
        // No paid caller-local state exists if this consuming call returns Err.
        let mut paired = roster.into_private_bf16_module_verified_receipt_v1()?;
        paired.verification.phase.require_clean_v1()?;
        let receipt = &paired.receipt;
        let roots = &paired.verification.roots;
        let observation = paired.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget
                .reserve_storage(OBSERVATION_STORAGE)
                .map_err(resource)?;
            let before = AccountSnapshot {
                account: budget.work_ledger_identity_v1(),
                storage: budget.storage(),
                work: budget.work(),
            };
            let retained = sum(
                std::mem::size_of::<PrivateBf16ModuleVerifiedReceiptV1>(),
                sum(
                    row_bytes::<LoweringRoot>(1)?,
                    sum(
                        row_bytes::<AuthenticatedRankedVerificationRootV1>(1)?,
                        row_bytes::<ProductionRankedAccessSourceV1>(3)?,
                    )?,
                )?,
            )?;
            budget.charge_work(32 + 4).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata("module owning pair changed root count"));
            };
            if receipt.root_count() != 1 {
                return Err(E::RosterMetadata(
                    "module owning pair changed authenticated identity",
                ));
            }
            // Real retained source vs actual authenticated root and requested
            // Return; complete structural checks, not a copied pre-move SHA.
            receipt
                .verify_private_bf16_module_retained_source_with_budget_v1(
                    root.semantic_root,
                    requested,
                    budget,
                )
                .map_err(E::Custody)?;
            require_account(&before, budget)?;
            drop(before);
            Ok(ValidatedObservation {
                work: budget.work(),
                storage: budget.storage(),
                retained,
                peak: budget.peak_storage(),
            })
        })?;
        // Paid output dies before refund; phase postflight already completed.
        observation.emit(requested);
        paired.verification.phase.require_clean_v1()?;
        paired.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget
                .release_storage(OBSERVATION_STORAGE)
                .map_err(resource)
        })?;
        drop(paired);
        Ok(())
    }

    #[test]
    fn module_observation_rejects_other_account_and_changed_retained_balance() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let mut original_work = Work::new(100);
        let mut original = Budget::new(&mut original_work, 1000);
        original.reserve_storage(117).unwrap();
        let before = AccountSnapshot {
            account: original.work_ledger_identity_v1(),
            storage: 117,
            work: original.work(),
        };
        assert!(require_account(&before, &mut original).is_ok());
        original.release_storage(1).unwrap();
        assert!(matches!(
            require_account(&before, &mut original),
            Err(E::ConditionalResource(Resource::Accounting))
        ));
        let mut other_work = Work::new(100);
        let mut other = Budget::new(&mut other_work, 1000);
        other.reserve_storage(117).unwrap();
        assert!(matches!(
            require_account(&before, &mut other),
            Err(E::ConditionalResource(Resource::Accounting))
        ));
        assert_eq!(original.storage(), 116);
        assert_eq!(other.storage(), 117);
    }
}

#[cfg(test)]
pub(super) fn refuse_module_observation_conversion_fixture_v1(
    roster: ProductionRankedSemanticProjectionRosterReceiptV1,
) -> Result<(), E> {
    module_observation::consume_and_observe(roster, [0, 1, 2, 3])
}

/// Full nominal validation stays attached to the intact materialized owner.
/// The authenticated roster contains the SAME original projection phase and
/// drops last. This is not a legacy Connected or ordinary pipeline attachment.
#[allow(dead_code)]
pub(crate) struct PrivateBf16AttachedRankedV1 {
    owner: fe2o3_lower_mir_kernel::ProductionPrivateBf16AttachedRankedOwnerV1,
    verification: AuthenticatedRankedVerificationRosterV1,
}

impl PrivateBf16ModuleVerifiedReceiptV1 {
    #[allow(dead_code)]
    pub(crate) fn into_private_bf16_attached_ranked_v1(
        mut self,
    ) -> Result<PrivateBf16AttachedRankedV1, E> {
        self.verification.phase.require_clean_v1()?;
        // The account is declared first so later consumed source/result locals
        // always die before it, including callback failure and postflight error.
        let mut verification = self.verification;
        let receipt = self.receipt;
        let owner = verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(1).map_err(resource)?;
            // The original Stage A wrapper credit is kept; the lowerer prepays
            // its new fixed owner. Together those actual credits conservatively
            // cover this moved wrapper. No old header/buffer credit is guessed.
            let available = sum(
                std::mem::size_of::<PrivateBf16ModuleVerifiedReceiptV1>(),
                fe2o3_lower_mir_kernel::ProductionPrivateBf16AttachedRankedOwnerV1::
                    retained_storage_v1(),
            )?;
            if std::mem::size_of::<PrivateBf16AttachedRankedV1>() > available {
                return Err(resource(Resource::Accounting));
            }
            receipt
                .into_private_bf16_attached_ranked_owner_with_budget_v1(budget)
                .map_err(E::Custody)
        })?;
        verification.phase.require_clean_v1()?;
        Ok(PrivateBf16AttachedRankedV1 {
            owner,
            verification,
        })
    }
}

impl PrivateBf16AttachedRankedV1 {
    /// Later consumers can replay full nominal translation without destroying
    /// the only intact source owner or creating a substitute resource account.
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_attached_ranked_v1(
        &mut self,
        semantic_sha256: &[u8; 32],
        root: SemanticFunctionIdV1,
        requested_return: [u8; 4],
    ) -> Result<(), E> {
        self.verification.phase.require_clean_v1()?;
        let owner = &self.owner;
        let result = self.verification.phase.with_budget(|budget| {
            owner
                .verify_private_bf16_attached_ranked_with_budget_v1(
                    semantic_sha256,
                    root,
                    requested_return,
                    budget,
                )
                .map_err(E::Custody)
        });
        self.verification.phase.require_clean_v1()?;
        result
    }

    #[allow(dead_code)]
    pub(crate) fn root_count(&self) -> usize {
        self.owner.root_count()
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Negative controls supply an actual legacy RawEmpty receipt and its actual
/// account. No test helper can manufacture a nominal source or success report.
#[cfg(test)]
pub(super) fn refuse_legacy_private_attachment_fixture_v1(
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    verification: AuthenticatedRankedVerificationRosterV1,
) -> Result<(), E> {
    let pair = PrivateBf16ModuleVerifiedReceiptV1 {
        receipt,
        verification,
    };
    match pair.into_private_bf16_attached_ranked_v1() {
        Err(error) => Err(error),
        Ok(owner) => {
            drop(owner);
            panic!("negative legacy fixture unexpectedly gained nominal custody");
        }
    }
}

#[cfg(test)]
mod attached_observation {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity;

    // These snapshots never leave the single live phase loan that paid them.
    struct AccountSnapshot {
        account: WorkIdentity,
        storage: usize,
        work: usize,
    }
    struct ValidatedObservation {
        work: usize,
        storage: usize,
        module_retained: usize,
        attachment_retained: usize,
        peak: usize,
    }
    const OBSERVATION_STORAGE: usize =
        std::mem::size_of::<AccountSnapshot>() + std::mem::size_of::<ValidatedObservation>();

    impl ValidatedObservation {
        fn emit(self, requested: [u8; 4]) {
            eprintln!(
                "fe2o3-bf16-private-intact-attachment-v1 completed=true roots=1 accesses=3 permutation={} work={} storage={} observation_storage={} module_storage={} attachment_storage={} peak={} same_account=true source_join=true fresh_full_validation=true fresh_full_replay=true intact_owner=true cleanup_pending=true normal_admission=false legacy_connected=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                self.work,
                self.storage,
                OBSERVATION_STORAGE,
                self.module_retained,
                self.attachment_retained,
                self.peak,
            );
        }
    }

    fn require_account(before: &AccountSnapshot, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(3).map_err(resource)?;
        if budget.work_ledger_identity_v1() != before.account
            || budget.storage() != before.storage
            || budget.work() <= before.work
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }

    impl ProductionRankedSemanticProgramV1 {
        /// Initial checks return no paid record across the consuming boundary.
        /// The actual owner and original phase, not detached observations,
        /// preserve source/account custody through the shipping conversion.
        pub(crate) fn observe_private_nominal_attached_for_test_v1(
            self,
            requested: [u8; 4],
        ) -> Result<(), E> {
            let mut roster = self.into_verified_roster_receipt()?;
            roster.phase.require_clean_v1()?;
            let materialized = &roster.materialized;
            let roots = &roster.source_order_roots;
            roster.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(32 + 32 + 4 + 8).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "module observer requires one actual root",
                    ));
                };
                let emission =
                    materialized
                        .bf16_call_instance_emission_v1()
                        .ok_or(E::RosterMetadata(
                            "module observer has no actual nominal emission",
                        ))?;
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || !std::ptr::eq(emission.owner(), materialized)
                    || emission.root() != root.semantic_root
                    || emission.return_permutation() != requested
                    || root.access_sources.len() != 3
                    || !root.executable_effect_sources.is_empty()
                    || root.ranked_ir.is_empty()
                {
                    return Err(E::RosterMetadata(
                        "module observer actual source/maps/Return differ",
                    ));
                }
                Ok(())
            })?;
            consume_and_observe(roster, requested)
        }
    }

    pub(super) fn consume_and_observe(
        roster: ProductionRankedSemanticProjectionRosterReceiptV1,
        requested: [u8; 4],
    ) -> Result<(), E> {
        // Both consuming transitions run with no paid caller-local observer.
        let module = roster.into_private_bf16_module_verified_receipt_v1()?;
        consume_module_and_observe(module, requested)
    }

    pub(super) fn consume_module_and_observe(
        module: PrivateBf16ModuleVerifiedReceiptV1,
        requested: [u8; 4],
    ) -> Result<(), E> {
        let mut paired = module.into_private_bf16_attached_ranked_v1()?;
        paired.verification.phase.require_clean_v1()?;
        let owner = &paired.owner;
        let roots = &paired.verification.roots;
        let observation = paired.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget
                .reserve_storage(OBSERVATION_STORAGE)
                .map_err(resource)?;
            let before = AccountSnapshot {
                account: budget.work_ledger_identity_v1(),
                storage: budget.storage(),
                work: budget.work(),
            };
            let module_retained = sum(
                std::mem::size_of::<PrivateBf16ModuleVerifiedReceiptV1>(),
                sum(
                    row_bytes::<LoweringRoot>(1)?,
                    sum(
                        row_bytes::<AuthenticatedRankedVerificationRootV1>(1)?,
                        row_bytes::<ProductionRankedAccessSourceV1>(3)?,
                    )?,
                )?,
            )?;
            let attachment_retained = fe2o3_lower_mir_kernel::
                ProductionPrivateBf16AttachedRankedOwnerV1::retained_storage_v1();
            budget.charge_work(32 + 4).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata("module owning pair changed root count"));
            };
            if owner.root_count() != 1 {
                return Err(E::RosterMetadata(
                    "module owning pair changed authenticated identity",
                ));
            }
            // Fresh full replay of actual intact owner/maps on its stored
            // original Work identity; exact new-vs-retained report equality.
            // Actual authenticated root/requested Return are not self-joins.
            owner
                .verify_private_bf16_attached_retained_source_with_budget_v1(
                    root.semantic_root,
                    requested,
                    budget,
                )
                .map_err(E::Custody)?;
            require_account(&before, budget)?;
            drop(before);
            Ok(ValidatedObservation {
                work: budget.work(),
                storage: budget.storage(),
                module_retained,
                attachment_retained,
                peak: budget.peak_storage(),
            })
        })?;
        // Paid output dies before refund; phase postflight already completed.
        observation.emit(requested);
        paired.verification.phase.require_clean_v1()?;
        paired.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget
                .release_storage(OBSERVATION_STORAGE)
                .map_err(resource)
        })?;
        drop(paired);
        Ok(())
    }

    #[test]
    fn attached_observation_rejects_other_account_and_changed_retained_balance() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let mut original_work = Work::new(100);
        let mut original = Budget::new(&mut original_work, 1000);
        original.reserve_storage(117).unwrap();
        let before = AccountSnapshot {
            account: original.work_ledger_identity_v1(),
            storage: 117,
            work: original.work(),
        };
        assert!(require_account(&before, &mut original).is_ok());
        original.release_storage(1).unwrap();
        assert!(matches!(
            require_account(&before, &mut original),
            Err(E::ConditionalResource(Resource::Accounting))
        ));
        let mut other_work = Work::new(100);
        let mut other = Budget::new(&mut other_work, 1000);
        other.reserve_storage(117).unwrap();
        assert!(matches!(
            require_account(&before, &mut other),
            Err(E::ConditionalResource(Resource::Accounting))
        ));
        assert_eq!(original.storage(), 116);
        assert_eq!(other.storage(), 117);
    }
}

#[cfg(test)]
pub(super) fn refuse_attached_observation_conversion_fixture_v1(
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    verification: AuthenticatedRankedVerificationRosterV1,
) -> Result<(), E> {
    // Negative fixture has only a real legacy receipt and its retained phase.
    // It cannot manufacture nominal source or a successful attached report.
    let module = PrivateBf16ModuleVerifiedReceiptV1 {
        receipt,
        verification,
    };
    attached_observation::consume_module_and_observe(module, [0, 1, 2, 3])
}

#[cfg(test)]
mod formal_diagnostic {
    use super::*;
    use fe2o3_kernel_ir::{
        FormalMemoryIncompleteReason as Reason, FormalMemoryObligationAnalysis as Analysis,
        FormalMemoryObligationError as FormalError,
    };
    use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as LowerError;

    // Selected row/field visits and variable text bytes are prepaid before
    // formatting. Standard stderr/formatting machinery is the same inherited
    // harness exclusion; outer byte/process/deadline limits remain mandatory.
    // No report vectors are cloned and no full obligations dump is emitted.
    fn prepay_text(budget: &mut Budget<'_>, bytes: usize) -> Result<(), LowerError> {
        budget.check_prior_denials_v1()?;
        budget.charge_work(bytes)?;
        Ok(())
    }

    fn prepay_reason(budget: &mut Budget<'_>, reason: &Reason) -> Result<(), LowerError> {
        budget.check_prior_denials_v1()?;
        budget.charge_work(32)?;
        // All other reason payloads are fixed-size indices/locations/enums.
        if let Reason::CallEffectsUnavailable { callee, .. } = reason {
            prepay_text(budget, callee.as_str().len())?;
        }
        Ok(())
    }

    pub(super) fn emit_attempt(
        attempt: &Result<Analysis, FormalError>,
        budget: &mut Budget<'_>,
        requested: [u8; 4],
    ) -> Result<(), LowerError> {
        budget.check_prior_denials_v1()?;
        budget.charge_work(32)?;
        match attempt {
            Ok(analysis) => {
                let obligations = analysis.obligations();
                let reasons = analysis.incomplete_reasons();
                // Charge the known row count before the first traversal.
                budget.charge_work(reasons.len())?;
                eprintln!(
                    "fe2o3-bf16-private-formal-summary-v1 analysis={} basis=compiler_ir_unauthenticated_launch allocations={} accesses={} bounds={} aliases={} conflicts={} incomplete_reasons={} formal_admission=false",
                    if analysis.is_complete() {
                        "complete"
                    } else {
                        "incomplete"
                    },
                    obligations.allocations().len(),
                    obligations.accesses().len(),
                    obligations.bounds_requirements().len(),
                    obligations.runtime_alias_requirements().len(),
                    obligations.inter_invocation_conflicts().len(),
                    reasons.len(),
                );
                for (ordinal, reason) in reasons.iter().enumerate() {
                    prepay_reason(budget, reason)?;
                    // Debug prints the exact enum payload, including the actual
                    // source operation/callee. String escaping keeps one row.
                    eprintln!(
                        "fe2o3-bf16-private-formal-reason-v1 ordinal={} value={:?}",
                        ordinal, reason,
                    );
                }
            }
            Err(FormalError::InvalidModule(errors)) => {
                budget.charge_work(errors.diagnostics().len())?;
                eprintln!(
                    "fe2o3-bf16-private-formal-error-v1 kind=InvalidModule diagnostics={}",
                    errors.diagnostics().len(),
                );
                for (ordinal, diagnostic) in errors.diagnostics().iter().enumerate() {
                    budget.charge_work(32)?;
                    prepay_text(budget, diagnostic.location.module.as_str().len())?;
                    if let Some(function) = &diagnostic.location.function {
                        prepay_text(budget, function.as_str().len())?;
                    }
                    if let Some(kernel) = &diagnostic.location.kernel {
                        prepay_text(budget, kernel.as_str().len())?;
                    }
                    prepay_text(budget, diagnostic.message.len())?;
                    eprintln!(
                        "fe2o3-bf16-private-formal-verifier-row-v1 ordinal={} value={:?}",
                        ordinal, diagnostic,
                    );
                }
            }
            Err(FormalError::MissingKernel { kernel }) => {
                prepay_text(budget, kernel.as_str().len())?;
                eprintln!(
                    "fe2o3-bf16-private-formal-error-v1 kind=MissingKernel value={:?}",
                    kernel,
                );
            }
            Err(FormalError::InvalidInvocationRange(error)) => {
                // RegionValidationError contains only fixed numeric/enum data.
                eprintln!(
                    "fe2o3-bf16-private-formal-error-v1 kind=InvalidInvocationRange value={:?}",
                    error,
                );
            }
            Err(FormalError::GuardedResource(error)) => {
                eprintln!(
                    "fe2o3-bf16-private-formal-error-v1 kind=GuardedResource value={:?}",
                    error,
                );
            }
        }
        budget.check_prior_denials_v1()?;
        // This borrowed callback has not yet dropped the attempt or its intact
        // owner. Only the later outer owning-entry marker confirms cleanup and
        // materialization postflight. Even Complete is not formal acceptance.
        eprintln!(
            "fe2o3-bf16-private-formal-collected-v1 collection_complete=true permutation={} work={} storage={} peak={} same_account=true source_join=true fresh_full_replay=true intact_owner=true cleanup_pending=true formal_admission=false normal_admission=false launch_authenticated=false",
            if requested == [0, 1, 2, 3] {
                "identity"
            } else {
                "swap01"
            },
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        );
        Ok(())
    }

    impl ProductionRankedSemanticProgramV1 {
        /// Distinct diagnostic-only continuation. No account-paid observation
        /// survives either consuming conversion, and no formal report escapes.
        pub(crate) fn observe_private_nominal_formal_for_test_v1(
            self,
            requested: [u8; 4],
        ) -> Result<(), E> {
            let mut roster = self.into_verified_roster_receipt()?;
            roster.phase.require_clean_v1()?;
            let materialized = &roster.materialized;
            let roots = &roster.source_order_roots;
            roster.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(32 + 32 + 4 + 8).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "formal diagnostic requires one actual root",
                    ));
                };
                let emission =
                    materialized
                        .bf16_call_instance_emission_v1()
                        .ok_or(E::RosterMetadata(
                            "formal diagnostic has no actual nominal emission",
                        ))?;
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || !std::ptr::eq(emission.owner(), materialized)
                    || emission.root() != root.semantic_root
                    || emission.return_permutation() != requested
                    || root.access_sources.len() != 3
                    || !root.executable_effect_sources.is_empty()
                    || root.ranked_ir.is_empty()
                {
                    return Err(E::RosterMetadata(
                        "formal diagnostic actual source/maps/Return differ",
                    ));
                }
                Ok(())
            })?;
            let module = roster.into_private_bf16_module_verified_receipt_v1()?;
            let mut paired = module.into_private_bf16_attached_ranked_v1()?;
            paired.verification.phase.require_clean_v1()?;
            let owner = &paired.owner;
            let roots = &paired.verification.roots;
            paired.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(2).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "formal diagnostic owning pair changed roots",
                    ));
                };
                if owner.root_count() != 1 {
                    return Err(E::RosterMetadata(
                        "formal diagnostic owning pair changed identity",
                    ));
                }
                owner
                    .with_private_bf16_formal_diagnostic_v1(
                        root.semantic_root,
                        requested,
                        budget,
                        |attempt, budget| emit_attempt(attempt, budget, requested),
                    )
                    .map_err(E::Custody)
            })?;
            paired.verification.phase.require_clean_v1()?;
            // Source/report first, original projection phase last; caller still
            // retains the separate original materialization account.
            drop(paired);
            Ok(())
        }
    }

    #[test]
    fn formal_reason_fixed_work_exact_and_one_short_preserve_denial() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        for limit in [31, 32] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            let result = prepay_reason(&mut budget, &Reason::LaunchExtentUnknown);
            assert_eq!(result.is_ok(), limit == 32);
            assert_eq!(budget.work(), if limit == 32 { 32 } else { 0 });
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            if limit == 31 {
                let first = budget.failed_work();
                assert!(matches!(
                    result,
                    Err(LowerError::ArgumentCorrespondenceResource(Resource::Work(
                        _
                    )))
                ));
                assert!(prepay_text(&mut budget, 0).is_err());
                assert_eq!(budget.failed_work(), first);
                assert_eq!(budget.work(), 0);
            }
        }
    }

    #[test]
    fn formal_text_visits_exact_and_one_short_do_not_copy_payload() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        for limit in [4, 5] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            assert_eq!(
                prepay_text(&mut budget, "a\nb\\c".len()).is_ok(),
                limit == 5
            );
            assert_eq!(budget.work(), if limit == 5 { 5 } else { 0 });
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        }
    }

    #[test]
    fn formal_callee_bytes_are_prepaid_without_losing_exact_reason() {
        use fe2o3_kernel_ir::{
            BlockId, CanonicalKernelIrWorkBudgetV1 as Work, FunctionId, FunctionOperationLocation,
        };
        let reason = Reason::CallEffectsUnavailable {
            location: FunctionOperationLocation::new(BlockId(7), 11),
            callee: FunctionId::new("a\nb\\c"),
        };
        for limit in [36, 37] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            let result = prepay_reason(&mut budget, &reason);
            assert_eq!(result.is_ok(), limit == 37);
            assert_eq!(budget.work(), if limit == 37 { 37 } else { 32 });
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            if limit == 36 {
                let first = budget.failed_work();
                assert!(prepay_reason(&mut budget, &reason).is_err());
                assert_eq!(budget.failed_work(), first);
                assert_eq!(budget.work(), 32);
            }
        }
        assert!(
            matches!(&reason, Reason::CallEffectsUnavailable { location, callee }
            if location.block == BlockId(7) && location.operation_index == 11
                && callee.as_str() == "a\nb\\c")
        );
    }
}

#[cfg(test)]
mod ranked_formal_observation {
    use super::*;
    use fe2o3_kernel_ir::{
        FormalMemoryIncompleteReason as Reason, FormalMemoryObligationAnalysis as Analysis,
        FormalMemoryObligationError as FormalError,
    };
    use fe2o3_lower_mir_kernel::{
        ProductionMemoryDischargeFailureV1 as Failure, ProductionSemanticKirErrorV1 as LowerError,
    };

    fn emit_proof(
        attempt: &Result<Analysis, FormalError>,
        proof: &Result<(), Failure>,
        budget: &mut Budget<'_>,
        requested: [u8; 4],
    ) -> Result<(), LowerError> {
        // Preserve exact raw analysis and original reason rows. This does not
        // label the core Incomplete result Complete or erase runtime obligations.
        super::formal_diagnostic::emit_attempt(attempt, budget, requested)?;
        budget.check_prior_denials_v1()?;
        budget.charge_work(40)?;
        if let Err(failure) = proof {
            let detail = match failure {
                Failure::Stage(detail)
                | Failure::Access { detail, .. }
                | Failure::GuardedBound { detail, .. } => *detail,
            };
            budget.charge_work(detail.len())?;
            eprintln!(
                "fe2o3-bf16-private-ranked-formal-refused-v1 value={:?}",
                failure
            );
            // The lowerer still returns refusal even when this diagnostic
            // callback returns Ok. No completion marker is emitted here.
            return Ok(());
        }
        let Ok(analysis) = attempt else {
            return Err(LowerError::CorrespondenceMismatch);
        };
        let report = analysis.obligations();
        let reasons = analysis.incomplete_reasons();
        budget.charge_work(reasons.len())?;
        if analysis.is_complete()
            || reasons.len() != 8
            || reasons
                .iter()
                .any(|reason| !matches!(reason, Reason::GuardedAccessRequiresRankedProof { .. }))
            || report.allocations().len() != 3
            || report.accesses().len() != 9
            || report.bounds_requirements().len() != 1
            || report.runtime_alias_requirements().len() != 2
            || !report.inter_invocation_conflicts().is_empty()
        {
            return Err(LowerError::CorrespondenceMismatch);
        }
        budget.check_prior_denials_v1()?;
        eprintln!(
            "fe2o3-bf16-private-ranked-formal-v1 discharged=true raw_analysis=incomplete discharged_reasons={} allocations={} accesses={} bounds={} aliases={} conflicts={} permutation={} work={} storage={} peak={} same_account=true source_join=true fresh_full_replay=true intact_owner=true cleanup_pending=true formal_admission=false normal_admission=false launch_authenticated=false",
            reasons.len(),
            report.allocations().len(),
            report.accesses().len(),
            report.bounds_requirements().len(),
            report.runtime_alias_requirements().len(),
            report.inter_invocation_conflicts().len(),
            if requested == [0, 1, 2, 3] {
                "identity"
            } else {
                "swap01"
            },
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        );
        Ok(())
    }

    impl ProductionRankedSemanticProgramV1 {
        /// Distinct test-only ranked guard continuation. No account-paid observation
        /// survives either consuming conversion, and no formal report escapes.
        pub(crate) fn observe_private_nominal_ranked_formal_for_test_v1(
            self,
            requested: [u8; 4],
        ) -> Result<(), E> {
            let mut roster = self.into_verified_roster_receipt()?;
            roster.phase.require_clean_v1()?;
            let materialized = &roster.materialized;
            let roots = &roster.source_order_roots;
            roster.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(32 + 32 + 4 + 8).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "ranked formal proof requires one actual root",
                    ));
                };
                let emission =
                    materialized
                        .bf16_call_instance_emission_v1()
                        .ok_or(E::RosterMetadata(
                            "ranked formal proof has no actual nominal emission",
                        ))?;
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || !std::ptr::eq(emission.owner(), materialized)
                    || emission.root() != root.semantic_root
                    || emission.return_permutation() != requested
                    || root.access_sources.len() != 3
                    || !root.executable_effect_sources.is_empty()
                    || root.ranked_ir.is_empty()
                {
                    return Err(E::RosterMetadata(
                        "ranked formal proof actual source/maps/Return differ",
                    ));
                }
                Ok(())
            })?;
            let module = roster.into_private_bf16_module_verified_receipt_v1()?;
            let mut paired = module.into_private_bf16_attached_ranked_v1()?;
            paired.verification.phase.require_clean_v1()?;
            let owner = &paired.owner;
            let roots = &paired.verification.roots;
            paired.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(2).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata(
                        "ranked formal proof owning pair changed roots",
                    ));
                };
                if owner.root_count() != 1 {
                    return Err(E::RosterMetadata(
                        "ranked formal proof owning pair changed identity",
                    ));
                }
                owner
                    .with_private_bf16_ranked_formal_v1(
                        root.semantic_root,
                        requested,
                        budget,
                        |attempt, proof, budget| emit_proof(attempt, proof, budget, requested),
                    )
                    .map_err(E::Custody)
            })?;
            paired.verification.phase.require_clean_v1()?;
            // Source/report first, original projection phase last; caller still
            // retains the separate original materialization account.
            drop(paired);
            Ok(())
        }
    }
}
