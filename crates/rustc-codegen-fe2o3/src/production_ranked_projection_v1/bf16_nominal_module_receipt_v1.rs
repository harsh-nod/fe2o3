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

/// Private formal continuation: actual rows/source first, original phase last.
/// Neither the owner nor its account can be split out or replaced.
#[allow(dead_code)]
pub(crate) struct PrivateBf16FormalMemoryV1 {
    owner: fe2o3_lower_mir_kernel::ProductionPrivateBf16FormalMemoryOwnerV1,
    verification: AuthenticatedRankedVerificationRosterV1,
}

impl PrivateBf16AttachedRankedV1 {
    fn into_private_bf16_formal_memory_v1(
        mut self,
        requested_return: [u8; 4],
    ) -> Result<PrivateBf16FormalMemoryV1, E> {
        self.verification.phase.require_clean_v1()?;
        // Keep the original account stationary, declared before moved sources
        // and results so every error/unwind destroys those values first.
        let mut verification = self.verification;
        let attached = self.owner;
        let roots = &verification.roots;
        let result = verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata("private formal owner requires one actual root"));
            };
            let available = sum(
                std::mem::size_of::<PrivateBf16AttachedRankedV1>(),
                fe2o3_lower_mir_kernel::ProductionPrivateBf16FormalMemoryOwnerV1::retained_storage_v1(),
            )?;
            if std::mem::size_of::<PrivateBf16FormalMemoryV1>() > available {
                return Err(resource(Resource::Accounting));
            }
            attached.into_private_bf16_formal_memory_with_budget_v1(
                root.semantic_root, requested_return, budget,
            ).map_err(E::FormalMemory)
        });
        // A swallowed inner denial cannot escape as an accepted owning stage.
        verification.phase.require_clean_v1()?;
        let owner = result?;
        Ok(PrivateBf16FormalMemoryV1 {
            owner,
            verification,
        })
    }
}

impl ProductionRankedSemanticProgramV1 {
    /// Closed consuming continuation selected only by the private retained
    /// pipeline stage. All roster, module, nominal and formal gates still run.
    #[allow(dead_code)]
    pub(crate) fn into_private_bf16_formal_memory_v1(
        self,
        requested_return: [u8; 4],
    ) -> Result<PrivateBf16FormalMemoryV1, E> {
        self.into_verified_roster_receipt()?
            .into_private_bf16_module_verified_receipt_v1()?
            .into_private_bf16_attached_ranked_v1()?
            .into_private_bf16_formal_memory_v1(requested_return)
    }
}

impl PrivateBf16FormalMemoryV1 {
    /// Fresh nominal translation, analysis and guard proof use the SAME phase.
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_formal_memory_v1(
        &mut self,
        requested_return: [u8; 4],
    ) -> Result<(), E> {
        self.verification.phase.require_clean_v1()?;
        let owner = &self.owner;
        let roots = &self.verification.roots;
        let result = self.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata(
                    "private formal replay changed actual roots",
                ));
            };
            if owner.root_count() != 1 {
                return Err(E::RosterMetadata(
                    "private formal replay changed actual source",
                ));
            }
            owner
                .verify_private_bf16_formal_memory_with_budget_v1(
                    root.semantic_root,
                    requested_return,
                    budget,
                )
                .map_err(E::FormalMemory)
        });
        self.verification.phase.require_clean_v1()?;
        result
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod private_formal_owner_error_tests {
    use super::*;

    #[test]
    fn private_formal_owner_preserves_typed_formal_error_source() {
        use std::error::Error as _;
        let error = E::FormalMemory(
            fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::KernelCount { actual: 0 },
        );
        assert!(error.to_string().contains("private nominal formal memory"));
        assert!(error.source().is_some());
        assert!(matches!(
            error,
            E::FormalMemory(
                fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::KernelCount { actual: 0 }
            )
        ));
    }
}

#[cfg(test)]
mod formal_owner_observation {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity;

    // Non-Copy and confined to the surviving owner's single live phase loan.
    struct Observation {
        ledger: WorkIdentity,
        entry_work: usize,
        protected: usize,
        work: usize,
        storage: usize,
        peak: usize,
    }

    impl Observation {
        fn emit(self, requested: [u8; 4]) {
            eprintln!(
                "fe2o3-bf16-private-formal-owner-v1 retained=true raw_analysis=incomplete discharged_reasons=8 allocations=3 accesses=9 bounds=1 aliases=2 conflicts=0 permutation={} work={} storage={} peak={} owner_storage={} observation_storage={} same_account=true source_join=true fresh_full_replay=true exact_obligations=true intact_owner=true stage_retained=true wrong_return_refused=true cleanup_pending=true formal_admission=false normal_admission=false launch_authenticated=false",
                if requested == [0, 1, 2, 3] { "identity" } else { "swap01" },
                self.work, self.storage, self.peak,
                fe2o3_lower_mir_kernel::ProductionPrivateBf16FormalMemoryOwnerV1::retained_storage_v1(),
                std::mem::size_of::<Self>(),
            );
        }
    }

    impl PrivateBf16FormalMemoryV1 {
        /// Read-only observation immediately after the actual production-stage
        /// replay. It neither reruns a frontend nor reconstructs an analysis.
        pub(crate) fn observe_private_bf16_formal_owner_for_test_v1(
            &mut self,
            requested: [u8; 4],
        ) -> Result<(), E> {
            self.verification.phase.require_clean_v1()?;
            let owner = &self.owner;
            let roots = &self.verification.roots;
            self.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                let header = std::mem::size_of::<Observation>();
                let protected = budget.storage().checked_add(header)
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                budget.reserve_storage(header).map_err(resource)?;
                let mut row = Observation {
                    ledger: budget.work_ledger_identity_v1(),
                    entry_work: budget.work(), protected,
                    work: 0, storage: 0, peak: 0,
                };
                let reasons = owner.ranked_discharged_reasons();
                let report = owner.obligations();
                budget.charge_work(
                    64usize.checked_add(reasons.len().checked_mul(8)
                        .ok_or_else(|| resource(Resource::Arithmetic))?)
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
                ).map_err(resource)?;
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || roots.len() != 1 || owner.root_count() != 1
                    || owner.raw_analysis_is_complete()
                    || reasons.len() != 8
                    || report.allocations().len() != 3
                    || report.accesses().len() != 9
                    || report.bounds_requirements().len() != 1
                    || report.runtime_alias_requirements().len() != 2
                    || !report.inter_invocation_conflicts().is_empty()
                    || reasons.iter().any(|reason| !matches!(reason,
                        fe2o3_kernel_ir::FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }))
                    || owner.grants_artifact_or_launch_authority()
                {
                    return Err(E::RosterMetadata("actual retained formal owner rows differ"));
                }
                for (ordinal, reason) in reasons.iter().enumerate() {
                    eprintln!("fe2o3-bf16-private-formal-owner-reason-v1 ordinal={} value={:?}",
                        ordinal, reason);
                }
                budget.check_prior_denials_v1().map_err(resource)?;
                if budget.work_ledger_identity_v1() != row.ledger
                    || budget.storage() != row.protected
                    || budget.work() < row.entry_work
                    || budget.peak_storage() < budget.storage()
                {
                    return Err(resource(Resource::Accounting));
                }
                row.work = budget.work();
                row.storage = budget.storage();
                row.peak = budget.peak_storage();
                // Consuming emit destroys this paid record before exact refund.
                row.emit(requested);
                budget.release_storage(header).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                Ok(())
            })?;
            self.verification.phase.require_clean_v1()?;
            Ok(())
        }
    }
}

type PrivateBf16TargetErrorV1 = crate::production_pipeline::ProductionPipelineError;
type PrivateBf16TargetResultV1 = Result<PrivateBf16TargetPayloadV1, PrivateBf16TargetErrorV1>;

struct PrivateBf16TargetPayloadV1 {
    bound: dialect_amdgcn::ProductionTargetBoundKernelIrV1,
    geometry: crate::production_geometry_v1::ProductionGeometryV1,
}

/// Target data dies first; the actual source, formal obligations and original
/// projection phase stay unsplit. The caller retains materialization separately.
#[allow(dead_code)]
pub(crate) struct PrivateBf16TargetBoundV1 {
    target: PrivateBf16TargetPayloadV1,
    formal: PrivateBf16FormalMemoryV1,
}

fn private_bf16_target_resource_v1(error: Resource) -> PrivateBf16TargetErrorV1 {
    PrivateBf16TargetErrorV1::RankedVerification(resource(error))
}

fn private_bf16_target_closure_v1() -> PrivateBf16TargetErrorV1 {
    PrivateBf16TargetErrorV1::Geometry(
        crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
    )
}

fn private_bf16_target_names_v1(
    semantic_binding: &[u8; 32],
    descriptor_binding: &[u8; 32],
    names: [&[u8]; 5],
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16TargetErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    // Pay the five length visits and fixed closure checks before this count.
    budget
        .charge_work(8)
        .map_err(private_bf16_target_resource_v1)?;
    let bytes = names
        .iter()
        .try_fold(0usize, |sum, name| sum.checked_add(name.len()))
        .and_then(|sum| sum.checked_mul(2))
        .and_then(|sum| sum.checked_add(32))
        .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
    budget
        .charge_work(bytes)
        .map_err(private_bf16_target_resource_v1)?;
    if semantic_binding != descriptor_binding
        || names[0].is_empty()
        || names.iter().any(|name| *name != names[0])
    {
        return Err(private_bf16_target_closure_v1());
    }
    Ok(())
}

fn private_bf16_target_derive_v1(
    module: &fe2o3_kernel_ir::Module,
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    obligations: &fe2o3_kernel_ir::FormalMemoryObligations,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> PrivateBf16TargetResultV1 {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(4)
        .map_err(private_bf16_target_resource_v1)?;
    let [typed] = typed_roots else {
        return Err(private_bf16_target_closure_v1());
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(private_bf16_target_closure_v1());
    };
    let entry = function
        .kernel_entry()
        .ok_or_else(private_bf16_target_closure_v1)?;
    private_bf16_target_names_v1(
        entry.kernel_binding_identity().as_bytes(),
        &typed.kernel_binding_bytes(),
        [
            entry.export_symbol().as_bytes(),
            typed.entry_symbol().as_bytes(),
            kernel.id.as_str().as_bytes(),
            kernel.entry.as_str().as_bytes(),
            obligations.kernel().as_str().as_bytes(),
        ],
        budget,
    )?;
    let launch = typed
        .source_launch()
        .ok_or(PrivateBf16TargetErrorV1::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
        ))?;
    // Existing geometry trees/call closure and binder clone/verifier payloads
    // retain their inherited excluded domain. This is not a whole-engine bound.
    let geometry = crate::production_geometry_v1::derive_production_geometry_v1(
        module,
        typed.entry_symbol(),
        function,
        launch,
        profile.device_target(),
    )
    .map_err(PrivateBf16TargetErrorV1::Geometry)?;
    let bound = dialect_amdgcn::bind_production_target_v1(module, profile)
        .map_err(PrivateBf16TargetErrorV1::TargetBinding)?;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(4)
        .map_err(private_bf16_target_resource_v1)?;
    let [bound_kernel] = bound.module().kernels.as_slice() else {
        return Err(private_bf16_target_closure_v1());
    };
    let [bound_id] = bound.kernel_ids() else {
        return Err(private_bf16_target_closure_v1());
    };
    private_bf16_target_names_v1(
        entry.kernel_binding_identity().as_bytes(),
        &typed.kernel_binding_bytes(),
        [
            typed.entry_symbol().as_bytes(),
            bound_kernel.id.as_str().as_bytes(),
            bound_kernel.entry.as_str().as_bytes(),
            bound_id.as_str().as_bytes(),
            obligations.kernel().as_str().as_bytes(),
        ],
        budget,
    )?;
    if bound.profile() != profile {
        return Err(private_bf16_target_closure_v1());
    }
    let bound_geometry = crate::production_geometry_v1::derive_production_geometry_v1(
        bound.module(),
        typed.entry_symbol(),
        function,
        launch,
        profile.device_target(),
    )
    .map_err(PrivateBf16TargetErrorV1::Geometry)?;
    if geometry != bound_geometry {
        return Err(private_bf16_target_closure_v1());
    }
    Ok(PrivateBf16TargetPayloadV1 { bound, geometry })
}

fn private_bf16_target_retained_header_v1() -> usize {
    // Conservative complete destination header, in addition to existing formal
    // credits; not a claim about the clone's heap or incidental stack copies.
    std::mem::size_of::<PrivateBf16TargetBoundV1>()
}

fn private_bf16_target_scratch_v1() -> usize {
    std::mem::size_of::<Option<PrivateBf16TargetResultV1>>()
}

fn reserve_private_bf16_target_output_v1(
    retain_output: bool,
    budget: &mut Budget<'_>,
) -> Result<(usize, usize), E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget.charge_work(1).map_err(resource)?;
    let retained = if retain_output {
        private_bf16_target_retained_header_v1()
    } else {
        0
    };
    let scratch = private_bf16_target_scratch_v1();
    budget
        .reserve_storage(sum(retained, scratch)?)
        .map_err(resource)?;
    Ok((retained, scratch))
}

fn require_private_bf16_bound_match_v1(
    retained: &dialect_amdgcn::ProductionTargetBoundKernelIrV1,
    fresh: &dialect_amdgcn::ProductionTargetBoundKernelIrV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
) -> Result<(), PrivateBf16TargetErrorV1> {
    // Complete structural equality, not a digest or replay-compatible flag.
    // Like existing formal equality, this comparison is an excluded legacy
    // graph/payload domain rather than a new whole-graph budget claim.
    if retained.profile() != profile || fresh.profile() != profile || retained != fresh {
        return Err(private_bf16_target_closure_v1());
    }
    Ok(())
}

impl PrivateBf16FormalMemoryV1 {
    #[allow(dead_code)]
    pub(crate) fn bind_private_bf16_target_v1(
        mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<PrivateBf16TargetBoundV1, PrivateBf16TargetErrorV1> {
        self.verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        let owner = &self.owner;
        let roots = &self.verification.roots;
        let result = self.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata(
                    "private target requires one retained source root",
                ));
            };
            let (retained, scratch) = reserve_private_bf16_target_output_v1(true, budget)?;
            let mut output: Option<PrivateBf16TargetResultV1> = None;
            let loan = owner.with_private_bf16_target_source_v1(
                root.semantic_root,
                requested_return,
                budget,
                |module, function, obligations, budget| {
                    output = Some(private_bf16_target_derive_v1(
                        module,
                        function,
                        obligations,
                        typed_roots,
                        profile,
                        budget,
                    ));
                    Ok(())
                },
            );
            if let Err(error) = loan {
                drop(output);
                budget
                    .release_storage(sum(retained, scratch)?)
                    .map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                return Err(E::FormalMemory(error));
            }
            budget.check_prior_denials_v1().map_err(resource)?;
            let output = output.ok_or_else(|| resource(Resource::Accounting))?;
            match output {
                Ok(target) => {
                    // The moved target now uses its already-paid destination
                    // header. Its temporary Option/Result representation is gone.
                    budget.release_storage(scratch).map_err(resource)?;
                    Ok(Ok(target))
                }
                Err(error) => {
                    // No target survives; returned typed diagnostics are not
                    // reusable owner/receipt authority.
                    budget
                        .release_storage(sum(retained, scratch)?)
                        .map_err(resource)?;
                    Ok(Err(error))
                }
            }
        });
        // Resource refusal wins before a captured target error/value escapes.
        self.verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        let target = result.map_err(PrivateBf16TargetErrorV1::RankedVerification)??;
        Ok(PrivateBf16TargetBoundV1 {
            target,
            formal: self,
        })
    }
}

impl PrivateBf16TargetBoundV1 {
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_target_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16TargetErrorV1> {
        self.formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        let owner = &self.formal.owner;
        let roots = &self.formal.verification.roots;
        let target = &self.target;
        let result = self.formal.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata(
                    "private target replay changed retained roots",
                ));
            };
            let (_, scratch) = reserve_private_bf16_target_output_v1(false, budget)?;
            let mut fresh: Option<PrivateBf16TargetResultV1> = None;
            let loan = owner.with_private_bf16_target_source_v1(
                root.semantic_root,
                requested_return,
                budget,
                |module, function, obligations, budget| {
                    fresh = Some(private_bf16_target_derive_v1(
                        module,
                        function,
                        obligations,
                        typed_roots,
                        profile,
                        budget,
                    ));
                    Ok(())
                },
            );
            if let Err(error) = loan {
                drop(fresh);
                budget.release_storage(scratch).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                return Err(E::FormalMemory(error));
            }
            budget.check_prior_denials_v1().map_err(resource)?;
            let fresh = fresh.ok_or_else(|| resource(Resource::Accounting))?;
            let compared = match fresh {
                Ok(fresh) => {
                    let compared =
                        require_private_bf16_bound_match_v1(&target.bound, &fresh.bound, profile)
                            .and_then(|()| {
                                if target.geometry == fresh.geometry {
                                    Ok(())
                                } else {
                                    Err(private_bf16_target_closure_v1())
                                }
                            });
                    drop(fresh);
                    compared
                }
                Err(error) => Err(error),
            };
            budget.release_storage(scratch).map_err(resource)?;
            budget.check_prior_denials_v1().map_err(resource)?;
            Ok(compared)
        });
        self.formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16TargetErrorV1::RankedVerification)?
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod private_target_consumer_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn private_target_names_bind_all_five_roles_and_exact_binding() {
        let binding = [7u8; 32];
        for changed in 0..7 {
            let mut work = Work::new(512);
            let mut budget = Budget::new(&mut work, 1);
            let mut names: [&[u8]; 5] = [b"entry"; 5];
            if changed < 5 {
                names[changed] = b"other";
            }
            let descriptor = if changed == 5 { [8u8; 32] } else { binding };
            assert_eq!(
                private_bf16_target_names_v1(&binding, &descriptor, names, &mut budget,).is_ok(),
                changed == 6
            );
        }
    }

    #[test]
    fn private_target_names_prepaid_exact_and_one_short() {
        let exact = 8 + 32 + 2 * 5 * b"entry".len();
        for limit in [exact, exact - 1] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 1);
            let result =
                private_bf16_target_names_v1(&[7; 32], &[7; 32], [b"entry"; 5], &mut budget);
            assert_eq!(result.is_ok(), limit == exact);
            assert_eq!(budget.storage(), 0);
            if limit == exact {
                assert_eq!(budget.work(), exact);
            } else {
                assert!(budget.failed_work().is_some());
            }
        }
    }

    #[test]
    fn private_target_output_header_exact_and_one_short() {
        for retained in [false, true] {
            let total = private_bf16_target_scratch_v1()
                + if retained {
                    private_bf16_target_retained_header_v1()
                } else {
                    0
                };
            for limit in [7 + total, 7 + total - 1] {
                let mut work = Work::new(1);
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(7).unwrap();
                let result = reserve_private_bf16_target_output_v1(retained, &mut budget);
                assert_eq!(result.is_ok(), limit == 7 + total);
                if let Ok((header, scratch)) = result {
                    assert_eq!(header + scratch, total);
                    assert_eq!(budget.storage(), 7 + total);
                    budget.release_storage(total).unwrap();
                } else {
                    assert!(budget.failed_storage().is_some());
                }
                assert_eq!(budget.storage(), 7);
            }
        }
    }

    #[test]
    fn private_target_output_preserves_prior_work_and_storage_refusals() {
        for kind in 0..3 {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if kind != 1 {
                assert!(budget.charge_work(9).is_err());
            }
            if kind != 0 {
                assert!(budget.reserve_storage(1).is_err());
            }
            let original = budget.check_prior_denials_v1().unwrap_err();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(
                    matches!(reserve_private_bf16_target_output_v1(true, &mut budget),
                    Err(E::ConditionalResource(error)) if error == original)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
        }
    }

    // Inert real-binder control only, not a rustc/source qualification fixture.
    fn module() -> fe2o3_kernel_ir::Module {
        use fe2o3_kernel_ir::*;
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        let mut kernel = Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("private-bf16-target-binding-control");
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn private_target_replay_rejects_wrong_profile_kernel_and_graph() {
        use fe2o3_amd_target::ProductionAmdTargetProfileV1::{Gfx942, Gfx950};
        let source = module();
        let retained = dialect_amdgcn::bind_production_target_v1(&source, Gfx942).unwrap();
        let fresh = dialect_amdgcn::bind_production_target_v1(&source, Gfx942).unwrap();
        assert!(require_private_bf16_bound_match_v1(&retained, &fresh, Gfx942).is_ok());
        assert!(require_private_bf16_bound_match_v1(&retained, &fresh, Gfx950).is_err());
        let wrong = dialect_amdgcn::bind_production_target_v1(&source, Gfx950).unwrap();
        assert!(require_private_bf16_bound_match_v1(&retained, &wrong, Gfx942).is_err());
        let mut changed = source.clone();
        changed.kernels[0].id = fe2o3_kernel_ir::KernelId::new("other");
        let changed = dialect_amdgcn::bind_production_target_v1(&changed, Gfx942).unwrap();
        assert!(require_private_bf16_bound_match_v1(&retained, &changed, Gfx942).is_err());
        let mut changed = source;
        changed.kernels[0].domain = fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(2),
        };
        let changed = dialect_amdgcn::bind_production_target_v1(&changed, Gfx942).unwrap();
        assert!(require_private_bf16_bound_match_v1(&retained, &changed, Gfx942).is_err());
    }
}

#[cfg(test)]
mod private_target_observation {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity;
    use std::cell::Cell;

    // Non-Copy, created only after every consuming transition has succeeded.
    struct Observation {
        ledger: WorkIdentity,
        entry_work: usize,
        protected: usize,
        work: usize,
        storage: usize,
        peak: usize,
        callback_calls: Cell<usize>,
        callback_drops: Cell<usize>,
        wrong_binding: [u8; 32],
    }

    struct Probe<'a> {
        calls: &'a Cell<usize>,
        drops: &'a Cell<usize>,
    }
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    fn require_closure_refusal<T>(
        result: Result<T, PrivateBf16TargetErrorV1>,
    ) -> Result<(), fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1> {
        match result {
            Err(PrivateBf16TargetErrorV1::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            )) => Ok(()),
            other => {
                drop(other);
                Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            }
        }
    }

    fn scratch() -> Result<usize, E> {
        sum(
            sum(
                std::mem::size_of::<Observation>(),
                std::mem::size_of::<fe2o3_kernel_ir::Module>(),
            )?,
            sum(
                std::mem::size_of::<PrivateBf16TargetResultV1>(),
                std::mem::size_of::<
                    Result<
                        dialect_amdgcn::ProductionTargetBoundKernelIrV1,
                        dialect_amdgcn::ProductionTargetBindingErrorV1,
                    >,
                >(),
            )?,
        )
    }

    impl Observation {
        fn emit(
            self,
            requested: [u8; 4],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
            geometry: &crate::production_geometry_v1::ProductionGeometryV1,
            observation_storage: usize,
        ) {
            let workgroup = geometry.workgroup();
            let grid = geometry.max_grid();
            eprintln!(
                "fe2o3-bf16-private-target-bound-v1 retained=true raw_analysis=incomplete discharged_reasons=8 allocations=3 accesses=9 bounds=1 aliases=2 conflicts=0 permutation={} profile={} rank={} workgroup_x={} workgroup_y={} workgroup_z={} max_grid_x={} max_grid_y={} max_grid_z={} max_flat_workgroup={} static_shared={} matrix={} workgroup_memory={} work={} storage={} peak={} target_storage={} observation_storage={} same_account=true source_join=true fresh_full_replay=true exact_obligations=true intact_owner=true stage_retained=true geometry_checked=true target_bound=true target_replayed=true wrong_return_refused=true unused_callback_dropped=true wrong_expected_target_refused=true wrong_kernel_refused=true wrong_entry_refused=true wrong_binding_refused=true changed_bound_refused=true cleanup_pending=true formal_admission=false normal_admission=false optimizer=false llvm=false launch_authenticated=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                profile.device_target(),
                geometry.rank(),
                workgroup[0],
                workgroup[1],
                workgroup[2],
                grid[0],
                grid[1],
                grid[2],
                geometry.max_flat_workgroup_size(),
                geometry.static_shared_memory_bytes(),
                geometry.allow_exact_tiled_matrix(),
                geometry.allow_workgroup_memory(),
                self.work,
                self.storage,
                self.peak,
                private_bf16_target_retained_header_v1(),
                observation_storage,
            );
        }
    }

    impl PrivateBf16TargetBoundV1 {
        /// Genuine negatives borrow this surviving target's actual source and
        /// retained bound graph. Mutated copies are never installed as owners.
        /// No paid observation crosses a consuming transition.
        pub(crate) fn observe_private_bf16_target_for_test_v1(
            &mut self,
            requested: [u8; 4],
            typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16TargetErrorV1> {
            self.formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            let owner = &self.formal.owner;
            let roots = &self.formal.verification.roots;
            let target = &self.target;
            let result = self.formal.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                let header = scratch()?;
                let protected = budget.storage().checked_add(header)
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                budget.reserve_storage(header).map_err(resource)?;
                let mut row = Observation {
                    ledger: budget.work_ledger_identity_v1(),
                    entry_work: budget.work(), protected,
                    work: 0, storage: 0, peak: 0,
                    callback_calls: Cell::new(0), callback_drops: Cell::new(0),
                    wrong_binding: [0; 32],
                };
                budget.charge_work(64).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata("actual target source root changed"));
                };
                if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
                    || target.bound.profile() != profile
                    || owner.root_count() != 1
                    || owner.raw_analysis_is_complete()
                    || owner.grants_artifact_or_launch_authority()
                {
                    return Err(E::RosterMetadata("actual target owner changed"));
                }
                let wrong = if requested == [0, 1, 2, 3] {
                    [1, 0, 2, 3]
                } else { [0, 1, 2, 3] };
                let probe = Probe { calls: &row.callback_calls, drops: &row.callback_drops };
                let refused = owner.with_private_bf16_target_source_v1(
                    root.semantic_root, wrong, budget,
                    move |_, _, _, _| {
                        probe.calls.set(probe.calls.get() + 1);
                        drop(probe);
                        Ok(())
                    },
                );
                match refused {
                    Err(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                    )) if row.callback_calls.get() == 0 && row.callback_drops.get() == 1 => {}
                    other => {
                        drop(other);
                        return Err(E::RosterMetadata(
                            "opposite Return did not refuse before dropping unused target callback",
                        ));
                    }
                }
                // Correct actual source replay remains possible after the typed
                // wrong-Return refusal; no replacement owner or budget is created.
                owner.with_private_bf16_target_source_v1(
                    root.semantic_root, requested, budget,
                    |module, function, obligations, budget| {
                        use fe2o3_amd_target::ProductionAmdTargetProfileV1::{Gfx942, Gfx950};
                        use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as SemanticError;
                        budget.check_prior_denials_v1()?;
                        budget.charge_work(64)?;
                        let opposite = if profile == Gfx942 { Gfx950 } else { Gfx942 };
                        require_closure_refusal(require_private_bf16_bound_match_v1(
                            &target.bound, &target.bound, opposite,
                        ))?;
                        let [typed] = typed_roots else {
                            return Err(SemanticError::CorrespondenceMismatch);
                        };
                        let [kernel] = module.kernels.as_slice() else {
                            return Err(SemanticError::CorrespondenceMismatch);
                        };
                        let entry = function.kernel_entry()
                            .ok_or(SemanticError::CorrespondenceMismatch)?;
                        row.wrong_binding = typed.kernel_binding_bytes();
                        row.wrong_binding[0] ^= 1;
                        require_closure_refusal(private_bf16_target_names_v1(
                            entry.kernel_binding_identity().as_bytes(), &row.wrong_binding,
                            [
                                entry.export_symbol().as_bytes(), typed.entry_symbol().as_bytes(),
                                kernel.id.as_str().as_bytes(), kernel.entry.as_str().as_bytes(),
                                obligations.kernel().as_str().as_bytes(),
                            ], budget,
                        ))?;
                        // Actual graph copies are inert negative arguments.
                        // Clone/engine payloads keep the declared inherited exclusion;
                        // their fixed Module/Result slots are prepaid above.
                        {
                            let mut changed = module.clone();
                            changed.kernels[0].id =
                                fe2o3_kernel_ir::KernelId::new("private_target_wrong_kernel");
                            require_closure_refusal(private_bf16_target_derive_v1(
                                &changed, function, obligations, typed_roots, profile, budget,
                            ))?;
                        }
                        {
                            let mut changed = module.clone();
                            changed.kernels[0].entry =
                                fe2o3_kernel_ir::FunctionId::new("private_target_wrong_entry");
                            require_closure_refusal(private_bf16_target_derive_v1(
                                &changed, function, obligations, typed_roots, profile, budget,
                            ))?;
                        }
                        {
                            let mut changed = module.clone();
                            changed.kernels[0].id =
                                fe2o3_kernel_ir::KernelId::new("private_target_changed_retained_state");
                            let changed = dialect_amdgcn::bind_production_target_v1(&changed, profile)
                                .map_err(|_| SemanticError::CorrespondenceMismatch)?;
                            require_closure_refusal(require_private_bf16_bound_match_v1(
                                &target.bound, &changed, profile,
                            ))?;
                        }
                        budget.check_prior_denials_v1()?;
                        Ok(())
                    },
                ).map_err(E::FormalMemory)?;
                let reasons = owner.ranked_discharged_reasons();
                let report = owner.obligations();
                budget.charge_work(
                    64usize.checked_add(reasons.len().checked_mul(8)
                        .ok_or_else(|| resource(Resource::Arithmetic))?)
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
                ).map_err(resource)?;
                if reasons.len() != 8 || report.allocations().len() != 3
                    || report.accesses().len() != 9 || report.bounds_requirements().len() != 1
                    || report.runtime_alias_requirements().len() != 2
                    || !report.inter_invocation_conflicts().is_empty()
                    || reasons.iter().any(|reason| !matches!(reason,
                        fe2o3_kernel_ir::FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }))
                {
                    return Err(E::RosterMetadata("actual target retained formal rows differ"));
                }
                for (ordinal, reason) in reasons.iter().enumerate() {
                    eprintln!("fe2o3-bf16-private-target-bound-reason-v1 ordinal={} value={:?}",
                        ordinal, reason);
                }
                budget.check_prior_denials_v1().map_err(resource)?;
                if budget.work_ledger_identity_v1() != row.ledger
                    || budget.storage() != row.protected
                    || budget.work() < row.entry_work
                    || budget.peak_storage() < budget.storage()
                {
                    return Err(resource(Resource::Accounting));
                }
                row.work = budget.work();
                row.storage = budget.storage();
                row.peak = budget.peak_storage();
                row.emit(requested, profile, &target.geometry, header);
                budget.release_storage(header).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                Ok(())
            });
            self.formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16TargetErrorV1::RankedVerification)
        }
    }
}

type PrivateBf16OptimizationErrorV1 = crate::production_pipeline::ProductionPipelineError;
type PrivateBf16OptimizationResultV1 =
    Result<PrivateBf16OptimizationPayloadV1, PrivateBf16OptimizationErrorV1>;

/// Both actual V12 endpoints remain owned. The checked result drops before B.
struct PrivateBf16OptimizationPayloadV1 {
    checked: fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    input: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    retained: usize,
}

/// This private B -> O continuation does not replace the nominal source owner
/// with a legacy Connected value. Target/formal/source custody and the original
/// projection account drop last; materialization remains owned by the caller.
#[allow(dead_code)]
pub(crate) struct PrivateBf16OptimizedV1 {
    optimization: PrivateBf16OptimizationPayloadV1,
    target: PrivateBf16TargetBoundV1,
}

fn private_bf16_optimization_scratch_v1() -> usize {
    std::mem::size_of::<Option<PrivateBf16OptimizationResultV1>>()
}

fn reserve_private_bf16_optimization_output_v1(
    budget: &mut Budget<'_>,
) -> Result<(usize, usize), E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget.charge_work(1).map_err(resource)?;
    // Conservative complete destination header in addition to separately paid
    // endpoint and payload headers. No existing header credit is reused here.
    let retained = std::mem::size_of::<PrivateBf16OptimizedV1>();
    let scratch = private_bf16_optimization_scratch_v1();
    budget
        .reserve_storage(sum(retained, scratch)?)
        .map_err(resource)?;
    Ok((retained, scratch))
}

/// Recheck the real B/O owners and immutable occurrence rows. This is only the
/// existing fixed local rewrite relation, not a new formal proof about O.
/// There are no callbacks or escaping borrowed views in this scratch scope.
fn recheck_private_bf16_optimization_v1(
    bound: &fe2o3_kernel_ir::Module,
    input: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16OptimizationErrorV1> {
    use fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1 as Replay;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(3).map_err(Replay::Resource)?;
        let bytes = input.canonical().canonical_bytes();
        let historical = checked.native_input_audit_bytes();
        // Canonical encoding injectively bounds the complete module comparison,
        // as in V12 admission; history is compared in full, never by digest.
        budget.charge_work(bytes.len()).map_err(Replay::Resource)?;
        if input.module() != bound {
            return Err(Replay::InputHistory);
        }
        let comparison = bytes
            .len()
            .checked_add(historical.len())
            .ok_or(Replay::Resource(Resource::Arithmetic))?;
        budget.charge_work(comparison).map_err(Replay::Resource)?;
        if bytes != historical {
            return Err(Replay::InputHistory);
        }
        let (input_inventory, input_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(input, budget)
                .map_err(Replay::Inventory)?;
        budget
            .reserve_storage(input_storage.retained_storage())
            .map_err(Replay::Resource)?;
        let (output_inventory, output_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(checked.owner(), budget)
                .map_err(Replay::Inventory)?;
        budget
            .reserve_storage(output_storage.retained_storage())
            .map_err(Replay::Resource)?;
        let (view, storage) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input_inventory,
            &output_inventory,
            checked.occurrences().candidate(),
            budget,
        )
        .map_err(Replay::Transition)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(Replay::Resource)?;
        drop(view);
        drop(output_inventory);
        drop(input_inventory);
        Ok(())
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Replay::Panicked)
        }
    };
    // Only this no-callback scratch can be above floor. On unwind all inventory
    // owners and the checked borrow have already dropped. Work/history persist.
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    restored.map_err(private_bf16_target_resource_v1)?;
    result.map_err(PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay)
}

/// Construction-only scope. Success keeps B, O and their paid payload alive;
/// failure drops every newly constructed owner before restoring this floor.
fn derive_private_bf16_optimization_v1(
    bound: &fe2o3_kernel_ir::Module,
    budget: &mut Budget<'_>,
) -> PrivateBf16OptimizationResultV1 {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget
            .charge_work(1)
            .map_err(private_bf16_target_resource_v1)?;
        let header = std::mem::size_of::<PrivateBf16OptimizationPayloadV1>();
        budget
            .reserve_storage(header)
            .map_err(private_bf16_target_resource_v1)?;
        let (input, input_storage) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::
                from_module_ref_with_verification_budget_v12(bound, budget)
                .map_err(PrivateBf16OptimizationErrorV1::PrivateBf16CanonicalInput)?;
        budget
            .reserve_storage(input_storage.retained_storage())
            .map_err(private_bf16_target_resource_v1)?;
        let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1(&input, budget)
            .map_err(PrivateBf16OptimizationErrorV1::PrivateBf16Optimization)?;
        budget
            .reserve_storage(checked.storage().retained_storage())
            .map_err(private_bf16_target_resource_v1)?;
        recheck_private_bf16_optimization_v1(bound, &input, &checked, budget)?;
        budget
            .charge_work(3)
            .map_err(private_bf16_target_resource_v1)?;
        let retained = header
            .checked_add(input_storage.retained_storage())
            .and_then(|bytes| bytes.checked_add(checked.storage().retained_storage()))
            .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
        if budget.storage().checked_sub(floor) != Some(retained) {
            return Err(private_bf16_target_resource_v1(Resource::Accounting));
        }
        Ok(PrivateBf16OptimizationPayloadV1 {
            checked,
            input,
            retained,
        })
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(
                PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay(
                    fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::Panicked,
                ),
            )
        }
    };
    let prior = budget.check_prior_denials_v1();
    if result.is_ok() && prior.is_ok() {
        return result;
    }
    // Consume a possible successful payload before any credit is refunded.
    let error = match result {
        Ok(payload) => {
            drop(payload);
            private_bf16_target_resource_v1(prior.expect_err("denial checked above"))
        }
        Err(error) => error,
    };
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    if let Err(error) = prior {
        return Err(private_bf16_target_resource_v1(error));
    }
    restored.map_err(private_bf16_target_resource_v1)?;
    Err(error)
}

impl PrivateBf16TargetBoundV1 {
    #[allow(dead_code)]
    pub(crate) fn optimize_private_bf16_target_v1(
        mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<PrivateBf16OptimizedV1, PrivateBf16OptimizationErrorV1> {
        // Fresh full nominal/formal replay and actual geometry/target rebinding
        // precede borrowing B. No earlier observation authorizes this phase.
        self.revalidate_private_bf16_target_v1(requested_return, typed_roots, profile)?;
        self.formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
        let bound = &self.target.bound;
        let result = self.formal.verification.phase.with_budget(|budget| {
            let (retained, scratch) = reserve_private_bf16_optimization_output_v1(budget)?;
            let output = derive_private_bf16_optimization_v1(bound.module(), budget);
            match output {
                Ok(optimization) => {
                    budget.release_storage(scratch).map_err(resource)?;
                    Ok(Ok(optimization))
                }
                Err(error) => {
                    budget
                        .release_storage(sum(retained, scratch)?)
                        .map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Err(error))
                }
            }
        });
        self.formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
        let optimization = result.map_err(PrivateBf16OptimizationErrorV1::RankedVerification)??;
        Ok(PrivateBf16OptimizedV1 {
            optimization,
            target: self,
        })
    }
}

impl PrivateBf16OptimizedV1 {
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_optimization_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16OptimizationErrorV1> {
        self.target
            .revalidate_private_bf16_target_v1(requested_return, typed_roots, profile)?;
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
        let bound = &self.target.target.bound;
        let optimization = &self.optimization;
        let result = self.target.formal.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            Ok(recheck_private_bf16_optimization_v1(
                bound.module(),
                &optimization.input,
                &optimization.checked,
                budget,
            ))
        });
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod private_checked_v12_optimizer_tests {
    use super::*;
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, *};

    fn limits() -> (usize, usize) {
        (
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        )
    }

    // A real binder/optimizer component control, not a nominal rustc proof.
    fn module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("private-checked-v12-optimizer-control");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn private_checked_output_header_exact_and_one_short() {
        let total =
            std::mem::size_of::<PrivateBf16OptimizedV1>() + private_bf16_optimization_scratch_v1();
        for cap in [7 + total, 7 + total - 1] {
            let mut work = Work::new(1);
            let mut budget = Budget::new(&mut work, cap);
            budget.reserve_storage(7).unwrap();
            let result = reserve_private_bf16_optimization_output_v1(&mut budget);
            assert_eq!(result.is_ok(), cap == 7 + total);
            if let Ok((retained, scratch)) = result {
                assert_eq!(retained + scratch, total);
                budget.release_storage(total).unwrap();
            } else {
                assert!(budget.failed_storage().is_some());
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_checked_entry_preserves_exhausted_work_and_prior_denial_precedence() {
        for kind in 0..3 {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if kind != 1 {
                assert!(budget.charge_work(9).is_err());
            }
            if kind != 0 {
                assert!(budget.reserve_storage(1).is_err());
            }
            let original = budget.check_prior_denials_v1().unwrap_err();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(
                    matches!(reserve_private_bf16_optimization_output_v1(&mut budget),
                    Err(E::ConditionalResource(error)) if error == original)
                );
                assert!(
                    matches!(derive_private_bf16_optimization_v1(&module(), &mut budget),
                    Err(PrivateBf16OptimizationErrorV1::RankedVerification(
                        E::ConditionalResource(error))) if error == original)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
        }
    }

    #[test]
    fn private_checked_real_binder_changed_output_and_fresh_replay_keep_original_account() {
        for profile in [
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let bound = dialect_amdgcn::bind_production_target_v1(&module(), profile).unwrap();
            let (work_limit, storage_limit) = limits();
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let payload = derive_private_bf16_optimization_v1(bound.module(), &mut budget).unwrap();
            assert_eq!(budget.storage(), 7 + payload.retained);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_ne!(
                payload.input.canonical().identity(),
                payload.checked.owner().canonical().identity()
            );
            assert_eq!(
                payload.input.module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations
                    .len(),
                1
            );
            assert!(
                payload.checked.owner().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations
                    .is_empty()
            );
            let floor = budget.storage();
            let before = budget.work();
            recheck_private_bf16_optimization_v1(
                bound.module(),
                &payload.input,
                &payload.checked,
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(!payload.checked.grants_authority());
            let retained = payload.retained;
            drop(payload);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 7);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
        }
    }

    #[test]
    fn private_checked_exact_and_one_short_construction_boundaries_restore_floor() {
        let bound = dialect_amdgcn::bind_production_target_v1(
            &module(),
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        let (work_limit, storage_limit) = limits();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let payload = derive_private_bf16_optimization_v1(bound.module(), &mut budget).unwrap();
        let exact_work = budget.work();
        let exact_storage = budget.peak_storage();
        let retained = payload.retained;
        drop(payload);
        budget.release_storage(retained).unwrap();
        for (work_cap, storage_cap, accepted) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
        ] {
            let mut work = Work::new(work_cap);
            let mut budget = Budget::new(&mut work, storage_cap);
            budget.reserve_storage(7).unwrap();
            let result = derive_private_bf16_optimization_v1(bound.module(), &mut budget);
            assert_eq!(result.is_ok(), accepted);
            if let Ok(payload) = result {
                let retained = payload.retained;
                drop(payload);
                budget.release_storage(retained).unwrap();
            } else {
                assert!(budget.check_prior_denials_v1().is_err());
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_checked_replay_refuses_changed_target_or_historical_input() {
        let profile = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942;
        let source = module();
        let bound = dialect_amdgcn::bind_production_target_v1(&source, profile).unwrap();
        let mut changed = source;
        changed.id = fe2o3_kernel_ir::ModuleId::new("changed-input");
        let other = dialect_amdgcn::bind_production_target_v1(&changed, profile).unwrap();
        let (work_limit, storage_limit) = limits();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let payload = derive_private_bf16_optimization_v1(bound.module(), &mut budget).unwrap();
        let other_payload =
            derive_private_bf16_optimization_v1(other.module(), &mut budget).unwrap();
        let floor = budget.storage();
        for (actual_bound, actual_checked) in [
            (other.module(), &payload.checked),
            (bound.module(), &other_payload.checked),
        ] {
            assert!(matches!(
                recheck_private_bf16_optimization_v1(
                    actual_bound,
                    &payload.input,
                    actual_checked,
                    &mut budget,
                ),
                Err(
                    PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay(
                        fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::InputHistory
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
        }
        let retained = payload.retained + other_payload.retained;
        drop(other_payload);
        drop(payload);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn private_checked_replay_preserves_prior_resource_error_and_borrowed_owners() {
        let bound = dialect_amdgcn::bind_production_target_v1(
            &module(),
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        let (work_limit, storage_limit) = limits();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let payload = derive_private_bf16_optimization_v1(bound.module(), &mut budget).unwrap();
        assert!(budget.charge_work(work_limit).is_err());
        let original = budget.check_prior_denials_v1().unwrap_err();
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        for _ in 0..2 {
            assert!(matches!(recheck_private_bf16_optimization_v1(
                bound.module(), &payload.input, &payload.checked, &mut budget,
            ), Err(PrivateBf16OptimizationErrorV1::RankedVerification(
                E::ConditionalResource(error))) if error == original));
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
        }
        let retained = payload.retained;
        drop(payload);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn private_checked_malformed_input_is_refused_without_fallback_or_lost_credit() {
        let mut source = module();
        source.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
        let (work_limit, storage_limit) = limits();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        assert!(matches!(
            derive_private_bf16_optimization_v1(&source, &mut budget),
            Err(PrivateBf16OptimizationErrorV1::PrivateBf16CanonicalInput(_))
        ));
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn private_checked_typed_errors_retain_display_and_source() {
        use std::error::Error;
        let error = PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay(
            fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::InputHistory,
        );
        assert!(error.to_string().contains("input history differs"));
        assert!(error.source().is_some());
    }
}

#[cfg(test)]
mod private_checked_v12_genuine_observer {
    use super::*;
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;

    struct Observation {
        ledger: CanonicalKernelIrWorkLedgerIdentityV1,
        protected: usize,
        entry_work: usize,
        work: usize,
        storage: usize,
        peak: usize,
        input_bytes: usize,
        output_bytes: usize,
        input_functions: usize,
        output_functions: usize,
        input_operations: usize,
        output_operations: usize,
        input_sha: [u8; 32],
        output_sha: [u8; 32],
    }
    struct Hex<'a>(&'a [u8; 32]);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    impl Observation {
        fn emit(self, requested: [u8; 4], retained: usize, scratch: usize) {
            eprintln!(
                "fe2o3-bf16-private-checked-optimizer-v1 permutation={} input_version=12 output_version=12 input_sha256={} output_sha256={} input_bytes={} output_bytes={} input_functions={} output_functions={} input_operations={} output_operations={} work={} storage={} peak={} retained_optimizer_storage={} observation_storage={} same_account=true intact_owner=true checked_optimizer=true fresh_relation=true source_join=true wrong_return_refused=true raw_analysis=incomplete discharged_reasons=8 allocations=3 accesses=9 bounds=1 aliases=2 conflicts=0 cleanup_pending=true formal_admission=false normal_admission=false llvm=false launch_authenticated=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                Hex(&self.input_sha),
                Hex(&self.output_sha),
                self.input_bytes,
                self.output_bytes,
                self.input_functions,
                self.output_functions,
                self.input_operations,
                self.output_operations,
                self.work,
                self.storage,
                self.peak,
                retained,
                scratch,
            );
        }
    }

    impl PrivateBf16OptimizedV1 {
        /// Observe only inside the already-surviving owning stage. No paid
        /// observation crosses its consuming constructor or replay transitions.
        pub(crate) fn observe_private_bf16_optimization_for_test_v1(
            &mut self,
            requested: [u8; 4],
        ) -> Result<(), PrivateBf16OptimizationErrorV1> {
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
            let target = &self.target.target;
            let formal = &self.target.formal.owner;
            let optimization = &self.optimization;
            let result = self.target.formal.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                let floor = budget.storage();
                let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                budget.charge_work(256).map_err(resource)?;
                let scratch = std::mem::size_of::<Observation>();
                let protected = budget.storage().checked_add(scratch)
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                budget.reserve_storage(scratch).map_err(resource)?;
                let mut row = Observation {
                    ledger: budget.work_ledger_identity_v1(), protected,
                    entry_work: budget.work(), work: 0, storage: 0, peak: 0,
                    input_bytes: 0, output_bytes: 0, input_functions: 0, output_functions: 0,
                    input_operations: 0, output_operations: 0, input_sha: [0;32], output_sha: [0;32],
                };
                if !matches!(requested, [0,1,2,3] | [1,0,2,3])
                    || formal.root_count() != 1 || formal.raw_analysis_is_complete()
                    || formal.grants_artifact_or_launch_authority()
                    || optimization.checked.grants_authority()
                {
                    return Err(E::RosterMetadata("actual optimized owner domain differs"));
                }
                // A fresh actual B/O relation, not a prior boolean marker.
                if let Err(error) = recheck_private_bf16_optimization_v1(
                    target.bound.module(), &optimization.input, &optimization.checked, budget,
                ) { return Ok(Err(error)); }
                let (input, input_storage) = match
                    CanonicalKirInventoryV1::derive(&optimization.input, budget) {
                        Ok(value) => value,
                        Err(error) => return Ok(Err(PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay(
                            fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::Inventory(error),
                        ))),
                    };
                budget.reserve_storage(input_storage.retained_storage()).map_err(resource)?;
                let (output, output_storage) = match
                    CanonicalKirInventoryV1::derive(optimization.checked.owner(), budget) {
                        Ok(value) => value,
                        Err(error) => return Ok(Err(PrivateBf16OptimizationErrorV1::PrivateBf16OptimizationReplay(
                            fe2o3_kernel_opt::KernelIrCheckedOptimizationReceiptErrorV1::Inventory(error),
                        ))),
                    };
                budget.reserve_storage(output_storage.retained_storage()).map_err(resource)?;
                budget.charge_work(256).map_err(resource)?;
                if !input.belongs_to(&optimization.input)
                    || !output.belongs_to(optimization.checked.owner())
                    || input.kernels().len() != 1 || output.kernels().len() != 1
                {
                    return Err(E::RosterMetadata("actual optimizer inventory custody differs"));
                }
                row.input_bytes = optimization.input.canonical().canonical_bytes().len();
                row.output_bytes = optimization.checked.owner().canonical().canonical_bytes().len();
                row.input_sha = *optimization.input.canonical().identity().digest();
                row.output_sha = *optimization.checked.owner().canonical().identity().digest();
                row.input_functions = input.functions().len();
                row.output_functions = output.functions().len();
                row.input_operations = input.operations().len();
                row.output_operations = output.operations().len();
                drop(output);
                drop(input);
                budget.release_storage(sum(input_storage.retained_storage(), output_storage.retained_storage())?)
                    .map_err(resource)?;
                let reasons = formal.ranked_discharged_reasons();
                let obligations = formal.obligations();
                budget.charge_work(64usize.checked_add(reasons.len().checked_mul(8)
                    .ok_or_else(|| resource(Resource::Arithmetic))?)
                    .ok_or_else(|| resource(Resource::Arithmetic))?).map_err(resource)?;
                if reasons.len() != 8 || obligations.allocations().len() != 3
                    || obligations.accesses().len() != 9 || obligations.bounds_requirements().len() != 1
                    || obligations.runtime_alias_requirements().len() != 2
                    || !obligations.inter_invocation_conflicts().is_empty()
                    || reasons.iter().any(|reason| !matches!(reason,
                        fe2o3_kernel_ir::FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }))
                {
                    return Err(E::RosterMetadata("actual optimized owner lost source obligations"));
                }
                for (ordinal, reason) in reasons.iter().enumerate() {
                    eprintln!("fe2o3-bf16-private-checked-optimizer-reason-v1 ordinal={} value={:?}",
                        ordinal, reason);
                }
                budget.check_prior_denials_v1().map_err(resource)?;
                if row.ledger != budget.work_ledger_identity_v1()
                    || budget.storage() != row.protected || budget.work() <= row.entry_work
                    || budget.peak_storage() < budget.storage()
                { return Err(resource(Resource::Accounting)); }
                row.work = budget.work();
                row.storage = budget.storage();
                row.peak = budget.peak_storage();
                row.emit(requested, optimization.retained, scratch);
                budget.release_storage(scratch).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                Ok(Ok(()))
                }));
                // This observer returns only unit/diagnostics and lends no
                // callback. All paid rows and inventory borrows have dropped.
                let result = match attempted {
                    Ok(result) => result,
                    Err(payload) => {
                        drop(payload);
                        Err(E::RosterMetadata("private optimizer observer panicked"))
                    }
                };
                let release = budget.storage().checked_sub(floor)
                    .ok_or_else(|| resource(Resource::Accounting))?;
                budget.release_storage(release).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                result
            });
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16OptimizationErrorV1::RankedVerification)?
        }
    }
}

// Actual checked-output safety is deliberately separate from source guarded
// admission. No historical ranked reason is imported into O's fresh analysis.
type PrivateBf16OutputFormalResultV1<'o> = Result<
    fe2o3_lower_mir_kernel::CheckedOutputFormalMemoryAnalysisV1<'o>,
    fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1,
>;

fn reserve_private_bf16_output_formal_frame_v1(
    budget: &mut Budget<'_>,
) -> Result<usize, PrivateBf16TargetErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(1)
        .map_err(private_bf16_target_resource_v1)?;
    let storage = std::mem::size_of::<PrivateBf16OutputFormalResultV1<'_>>();
    budget
        .reserve_storage(storage)
        .map_err(private_bf16_target_resource_v1)?;
    Ok(storage)
}

/// Existing complete-only formal analysis of the actual checked O. Its graph,
/// trees and obligation payload retain the inherited formal-engine exclusion.
/// Only this selected Result header and explicit join work use the original
/// canonical account. On unwind the owning phase retains the accepted credit
/// until its locals/owner drop; this helper is always called inside that phase.
fn check_private_bf16_output_formal_v1(
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16TargetErrorV1> {
    let storage = reserve_private_bf16_output_formal_frame_v1(budget)?;
    let analyzed = fe2o3_lower_mir_kernel::analyze_checked_output_formal_memory_v1(checked);
    let result = match analyzed {
        Ok(report) => {
            let joined = (|| {
                budget
                    .check_prior_denials_v1()
                    .map_err(private_bf16_target_resource_v1)?;
                // Pay fixed selections before accessing the singleton rosters.
                budget
                    .charge_work(6)
                    .map_err(private_bf16_target_resource_v1)?;
                let [kernel] = checked.owner().module().kernels.as_slice() else {
                    return Err(private_bf16_target_closure_v1());
                };
                let [obligations] = report.kernels() else {
                    return Err(private_bf16_target_closure_v1());
                };
                let lengths = kernel
                    .id
                    .as_str()
                    .len()
                    .checked_add(kernel.entry.as_str().len())
                    .and_then(|n| n.checked_add(obligations.kernel().as_str().len()))
                    .and_then(|n| n.checked_add(obligations.entry().as_str().len()))
                    .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
                budget
                    .charge_work(lengths)
                    .map_err(private_bf16_target_resource_v1)?;
                if !std::ptr::eq(report.output(), checked.owner())
                    || obligations.kernel() != &kernel.id
                    || obligations.entry() != &kernel.entry
                    || obligations.index_width() != fe2o3_kernel_ir::FormalIndexWidth::Bits64
                    || !obligations.inter_invocation_conflicts().is_empty()
                {
                    return Err(private_bf16_target_closure_v1());
                }
                Ok(())
            })();
            drop(report);
            joined
        }
        // Keep the original typed Incomplete/Analysis/conflict payload. It is
        // excluded diagnostic data, never a borrowed owner or proof receipt.
        Err(error) => Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(error)),
    };
    // The paid analysis Result/report is consumed before this exact refund.
    budget
        .release_storage(storage)
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    result
}

fn require_private_bf16_output_bound_match_v1(
    output: &fe2o3_kernel_ir::Module,
    fresh: &dialect_amdgcn::ProductionTargetBoundKernelIrV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
) -> Result<(), PrivateBf16TargetErrorV1> {
    // This is an idempotence/exact-target check, not a replacement executable.
    // Full equality and binder clone retain their existing excluded domain.
    if fresh.profile() != profile || fresh.module() != output {
        return Err(private_bf16_target_closure_v1());
    }
    Ok(())
}

fn check_private_bf16_output_target_v1(
    output: &fe2o3_kernel_ir::Module,
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    source_obligations: &fe2o3_kernel_ir::FormalMemoryObligations,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16TargetErrorV1> {
    let (_, scratch) = reserve_private_bf16_target_output_v1(false, budget)
        .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
    // Source obligations supply only the already-authenticated kernel-name
    // join in this existing geometry/binder routine, not O memory proof.
    let derived = private_bf16_target_derive_v1(
        output,
        function,
        source_obligations,
        typed_roots,
        profile,
        budget,
    );
    let result = match derived {
        Ok(fresh) => {
            let result = require_private_bf16_output_bound_match_v1(output, &fresh.bound, profile);
            drop(fresh);
            result
        }
        Err(error) => Err(error),
    };
    // Geometry and cloned bound candidate have died; O is never replaced.
    budget
        .release_storage(scratch)
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    result
}

impl PrivateBf16OptimizedV1 {
    /// Private unit-only consumer of fresh O safety. Even success grants no
    /// ordinary, LLVM, artifact, descriptor or authenticated-launch authority.
    /// Source guards cannot discharge O here: the existing complete-only
    /// analyzer's exact typed refusal is returned unchanged.
    #[allow(dead_code)]
    pub(crate) fn verify_private_bf16_output_safety_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16TargetErrorV1> {
        self.revalidate_private_bf16_optimization_v1(requested_return, typed_roots, profile)?;
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        let checked = &self.optimization.checked;
        let owner = &self.target.formal.owner;
        let roots = &self.target.formal.verification.roots;
        let result = self.target.formal.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata(
                    "private output safety changed source roots",
                ));
            };
            let storage = std::mem::size_of::<Option<Result<(), PrivateBf16TargetErrorV1>>>();
            budget.reserve_storage(storage).map_err(resource)?;
            let mut observed: Option<Result<(), PrivateBf16TargetErrorV1>> = None;
            let loan = owner.with_private_bf16_target_source_v1(
                root.semantic_root,
                requested_return,
                budget,
                |_source_module, function, source_obligations, budget| {
                    observed = Some(
                        check_private_bf16_output_target_v1(
                            checked.owner().module(),
                            function,
                            source_obligations,
                            typed_roots,
                            profile,
                            budget,
                        )
                        .and_then(|()| check_private_bf16_output_formal_v1(checked, budget)),
                    );
                    Ok(())
                },
            );
            if let Err(error) = loan {
                drop(observed);
                budget.release_storage(storage).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                return Err(E::FormalMemory(error));
            }
            // Consume the paid Option/Result before refund; only unit or a
            // typed diagnostic moves out, never a report, owner or receipt.
            let observed = observed.ok_or_else(|| resource(Resource::Accounting))?;
            match observed {
                Ok(()) => {
                    budget.release_storage(storage).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Ok(()))
                }
                Err(error) => {
                    budget.release_storage(storage).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Err(error))
                }
            }
        });
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16TargetErrorV1::RankedVerification)?
    }
}

#[cfg(test)]
mod private_checked_output_safety_tests {
    use super::*;
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, *};

    fn limits() -> (usize, usize) {
        (
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        )
    }

    // Actual binder/checked optimizer fixtures, not nominal source admission.
    fn module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
        let arguments = (0..parameters.len())
            .map(|i| ValueId(u32::try_from(i).unwrap()))
            .collect();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = operations;
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("private-output-safety-component");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            arguments,
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn with_checked(
        source: Module,
        inspect: impl FnOnce(&fe2o3_pliron::CheckedNeutralKernelIrOwnerV1, &mut Budget<'_>),
    ) {
        let bound = dialect_amdgcn::bind_production_target_v1(
            &source,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        let (work_limit, storage_limit) = limits();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let payload = derive_private_bf16_optimization_v1(bound.module(), &mut budget).unwrap();
        let retained = payload.retained;
        inspect(&payload.checked, &mut budget);
        drop(payload);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn private_output_formal_frame_exact_and_one_short() {
        let frame = std::mem::size_of::<PrivateBf16OutputFormalResultV1<'_>>();
        for cap in [7 + frame, 7 + frame - 1] {
            let mut work = Work::new(1);
            let mut budget = Budget::new(&mut work, cap);
            budget.reserve_storage(7).unwrap();
            let result = reserve_private_bf16_output_formal_frame_v1(&mut budget);
            assert_eq!(result.is_ok(), cap == 7 + frame);
            if let Ok(storage) = result {
                assert_eq!(storage, frame);
                budget.release_storage(storage).unwrap();
            } else {
                assert!(budget.failed_storage().is_some());
            }
            assert_eq!(budget.work(), 1);
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_output_formal_frame_preserves_prior_work_storage_and_precedence() {
        for kind in 0..3 {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if kind != 1 {
                assert!(budget.charge_work(9).is_err());
            }
            if kind != 0 {
                assert!(budget.reserve_storage(1).is_err());
            }
            let prior = budget.check_prior_denials_v1().unwrap_err();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(
                    matches!(reserve_private_bf16_output_formal_frame_v1(&mut budget),
                    Err(PrivateBf16TargetErrorV1::RankedVerification(E::ConditionalResource(error)))
                    if error == prior)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
        }
    }

    #[test]
    fn private_output_complete_analysis_uses_actual_checked_owner_and_exact_selected_work() {
        with_checked(module(vec![], vec![]), |checked, budget| {
            let floor = budget.storage();
            let work = budget.work();
            let ledger = budget.work_ledger_identity_v1();
            check_private_bf16_output_formal_v1(checked, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), work + 1 + 6 + 4 * "entry".len());
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
            let fresh =
                fe2o3_lower_mir_kernel::analyze_checked_output_formal_memory_v1(checked).unwrap();
            assert!(std::ptr::eq(fresh.output(), checked.owner()));
            let [obligations] = fresh.kernels() else {
                panic!("one actual output kernel");
            };
            assert_eq!(obligations.kernel().as_str(), "entry");
            assert_eq!(obligations.entry().as_str(), "entry");
            assert_eq!(obligations.accesses().len(), 0);
            assert_eq!(obligations.index_width(), FormalIndexWidth::Bits64);
            drop(fresh);
        });
    }

    #[test]
    fn private_output_incomplete_retains_exact_unknown_call_reason() {
        let mut source = module(
            vec![],
            vec![Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("unknown_external"),
                    arguments: vec![],
                },
            )],
        );
        source.functions.push(Function::declaration(
            "unknown_external",
            Signature::new(vec![], vec![]),
        ));
        with_checked(source, |checked, budget| {
            let floor = budget.storage();
            let work = budget.work();
            let result = check_private_bf16_output_formal_v1(checked, budget);
            let Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
                fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::Incomplete { reasons },
            )) = result
            else {
                panic!("actual unknown call must stay incomplete");
            };
            assert!(matches!(reasons.as_ref(),
                [FormalMemoryIncompleteReason::CallEffectsUnavailable { location, callee }]
                    if location.block == BlockId(0) && location.operation_index == 0
                        && callee.as_str() == "unknown_external"));
            drop(reasons);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), work + 1);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
        });
    }

    #[test]
    fn private_output_conflicts_are_typed_refusals_not_complete_admission() {
        let source = module(
            vec![
                Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
                Type::F32,
            ],
            vec![Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )],
        );
        with_checked(source, |checked, budget| {
            let floor = budget.storage();
            let result = check_private_bf16_output_formal_v1(checked, budget);
            let Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
                fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::InterInvocationConflicts {
                    conflicts,
                },
            )) = result
            else {
                panic!("actual constant-address write must refuse");
            };
            assert_eq!(conflicts.len(), 1);
            drop(conflicts);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.failed_storage(), None);
        });
    }

    #[test]
    fn private_output_formal_refuses_before_analysis_on_original_exhausted_account() {
        with_checked(module(vec![], vec![]), |checked, budget| {
            let (limit, _) = limits();
            assert!(budget.charge_work(limit).is_err());
            let prior = budget.check_prior_denials_v1().unwrap_err();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(
                    matches!(check_private_bf16_output_formal_v1(checked, budget),
                    Err(PrivateBf16TargetErrorV1::RankedVerification(E::ConditionalResource(error)))
                    if error == prior)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
        });
    }

    #[test]
    fn private_output_target_is_exact_and_never_replaced_by_idempotent_clone() {
        use fe2o3_amd_target::ProductionAmdTargetProfileV1::{Gfx942, Gfx950};
        let source = module(vec![], vec![]);
        let output = dialect_amdgcn::bind_production_target_v1(&source, Gfx942).unwrap();
        let fresh = dialect_amdgcn::bind_production_target_v1(output.module(), Gfx942).unwrap();
        require_private_bf16_output_bound_match_v1(output.module(), &fresh, Gfx942).unwrap();
        assert!(!std::ptr::eq(output.module(), fresh.module()));
        assert!(
            require_private_bf16_output_bound_match_v1(output.module(), &fresh, Gfx950).is_err()
        );
        // Missing target requirements cannot be repaired by adopting the clone.
        assert!(require_private_bf16_output_bound_match_v1(&source, &fresh, Gfx942).is_err());
        let mut changed = output.module().clone();
        changed.kernels[0].id = KernelId::new("different");
        assert!(require_private_bf16_output_bound_match_v1(&changed, &fresh, Gfx942).is_err());
        let mut changed = output.module().clone();
        changed.functions[0].id = FunctionId::new("different");
        assert!(require_private_bf16_output_bound_match_v1(&changed, &fresh, Gfx942).is_err());
        let mut changed = output.module().clone();
        changed.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(7),
        };
        assert!(require_private_bf16_output_bound_match_v1(&changed, &fresh, Gfx942).is_err());
    }
}

#[cfg(test)]
mod private_checked_output_diagnostic {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, FormalMemoryIncompleteReason as Reason,
        FormalMemoryObligationAnalysis as Analysis, FormalMemoryObligationError as FormalError,
    };
    type Attempt = Result<Analysis, FormalError>;

    // Selected fixed live observation, created after every consuming stage map.
    // Engine payload, typed consumer diagnostics and formatter/control locals
    // retain their explicitly inherited excluded domains.
    struct Observation {
        ledger: Ledger,
        floor: usize,
        initial_work: usize,
        output_sha: [u8; 32],
        output_bytes: u64,
        work: usize,
        storage: usize,
        peak: usize,
    }

    fn prepay_error(error: &FormalError, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(32).map_err(resource)?;
        match error {
            FormalError::InvalidModule(errors) => {
                budget
                    .charge_work(errors.diagnostics().len())
                    .map_err(resource)?;
                for row in errors.diagnostics() {
                    budget.charge_work(32).map_err(resource)?;
                    let bytes = row
                        .location
                        .module
                        .as_str()
                        .len()
                        .checked_add(
                            row.location
                                .function
                                .as_ref()
                                .map_or(0, |v| v.as_str().len()),
                        )
                        .and_then(|n| {
                            n.checked_add(
                                row.location.kernel.as_ref().map_or(0, |v| v.as_str().len()),
                            )
                        })
                        .and_then(|n| n.checked_add(row.message.len()))
                        .ok_or_else(|| resource(Resource::Arithmetic))?;
                    budget.charge_work(bytes).map_err(resource)?;
                }
            }
            FormalError::MissingKernel { kernel } => {
                budget
                    .charge_work(kernel.as_str().len())
                    .map_err(resource)?;
            }
            FormalError::InvalidInvocationRange(_) | FormalError::GuardedResource(_) => {}
        }
        Ok(())
    }

    fn prepay_attempt(attempt: &Attempt, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(32).map_err(resource)?;
        match attempt {
            Err(error) => prepay_error(error, budget),
            Ok(analysis) => {
                let o = analysis.obligations();
                let rows = o
                    .allocations()
                    .len()
                    .checked_add(o.accesses().len())
                    .and_then(|n| n.checked_add(o.bounds_requirements().len()))
                    .and_then(|n| n.checked_add(o.runtime_alias_requirements().len()))
                    .and_then(|n| n.checked_add(o.inter_invocation_conflicts().len()))
                    .and_then(|n| n.checked_add(analysis.incomplete_reasons().len()))
                    .and_then(|n| n.checked_mul(128))
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                // Known row counts are paid before any traversal/comparison.
                // This selected field-visit allowance is not formatter/RSS work.
                budget.charge_work(rows).map_err(resource)?;
                budget
                    .charge_work(sum(o.kernel().as_str().len(), o.entry().as_str().len())?)
                    .map_err(resource)?;
                for reason in analysis.incomplete_reasons() {
                    if let Reason::CallEffectsUnavailable { callee, .. } = reason {
                        budget
                            .charge_work(callee.as_str().len())
                            .map_err(resource)?;
                    }
                }
                Ok(())
            }
        }
    }

    fn prepay_consumer(
        consumer: &Result<(), PrivateBf16TargetErrorV1>,
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        use fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1 as F;
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(32).map_err(resource)?;
        match consumer {
            Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::Incomplete { reasons })) => {
                budget
                    .charge_work(
                        reasons
                            .len()
                            .checked_mul(128)
                            .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )
                    .map_err(resource)?;
                for reason in reasons.iter() {
                    if let Reason::CallEffectsUnavailable { callee, .. } = reason {
                        budget
                            .charge_work(callee.as_str().len())
                            .map_err(resource)?;
                    }
                }
            }
            Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::InterInvocationConflicts {
                conflicts,
            })) => {
                budget
                    .charge_work(
                        conflicts
                            .len()
                            .checked_mul(128)
                            .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )
                    .map_err(resource)?;
            }
            Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::Analysis(error))) => {
                prepay_error(error, budget)?
            }
            _ => {}
        }
        Ok(())
    }

    fn consumer_join(
        consumer: &Result<(), PrivateBf16TargetErrorV1>,
        attempt: &Attempt,
    ) -> Result<&'static str, E> {
        use fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1 as F;
        match (consumer, attempt) {
            (Ok(()), Ok(Analysis::Complete(o))) if o.inter_invocation_conflicts().is_empty() => {
                Ok("complete")
            }
            (
                Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::Incomplete {
                    reasons: expected,
                })),
                Ok(Analysis::Incomplete { reasons, .. }),
            ) if expected.as_ref() == reasons.as_slice() => Ok("incomplete"),
            (
                Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::InterInvocationConflicts {
                    conflicts,
                })),
                Ok(Analysis::Complete(o)),
            ) if !conflicts.is_empty() && conflicts.as_ref() == o.inter_invocation_conflicts() => {
                Ok("conflict")
            }
            (
                Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(F::Analysis(expected))),
                Err(actual),
            ) if expected == actual => Ok("analysis_error"),
            _ => Err(E::RosterMetadata(
                "actual checked O consumer and fresh raw analysis differ",
            )),
        }
    }

    fn emit_attempt(attempt: &Attempt, consumer: &str) {
        match attempt {
            Ok(analysis) => {
                let o = analysis.obligations();
                eprintln!(
                    "fe2o3-bf16-private-checked-output-summary-v1 consumer={} analysis={} allocations={} accesses={} bounds={} aliases={} conflicts={} incomplete_reasons={} optimized_formal_admission=false",
                    consumer,
                    if analysis.is_complete() {
                        "complete"
                    } else {
                        "incomplete"
                    },
                    o.allocations().len(),
                    o.accesses().len(),
                    o.bounds_requirements().len(),
                    o.runtime_alias_requirements().len(),
                    o.inter_invocation_conflicts().len(),
                    analysis.incomplete_reasons().len(),
                );
                eprintln!(
                    "fe2o3-bf16-private-checked-output-header-v1 kernel={:?} entry={:?} index_width={:?} basis={:?} invocations={:?}",
                    o.kernel(),
                    o.entry(),
                    o.index_width(),
                    o.analysis_basis(),
                    o.invocations(),
                );
                for (i, row) in o.allocations().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-allocation-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.accesses().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-access-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.bounds_requirements().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-bounds-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.runtime_alias_requirements().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-alias-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.inter_invocation_conflicts().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-conflict-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in analysis.incomplete_reasons().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-reason-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
            }
            Err(FormalError::InvalidModule(errors)) => {
                eprintln!(
                    "fe2o3-bf16-private-checked-output-error-v1 consumer={} kind=InvalidModule diagnostics={}",
                    consumer,
                    errors.diagnostics().len()
                );
                for (i, row) in errors.diagnostics().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-verifier-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
            }
            Err(FormalError::MissingKernel { kernel }) => eprintln!(
                "fe2o3-bf16-private-checked-output-error-v1 consumer={} kind=MissingKernel value={:?}",
                consumer, kernel
            ),
            Err(FormalError::InvalidInvocationRange(error)) => eprintln!(
                "fe2o3-bf16-private-checked-output-error-v1 consumer={} kind=InvalidInvocationRange value={:?}",
                consumer, error
            ),
            Err(FormalError::GuardedResource(error)) => eprintln!(
                "fe2o3-bf16-private-checked-output-error-v1 consumer={} kind=GuardedResource value={:?}",
                consumer, error
            ),
        }
    }

    impl Observation {
        fn emit(self, requested: [u8; 4], consumer: &str, scratch: usize) {
            struct Hex<'a>(&'a [u8; 32]);
            impl std::fmt::Display for Hex<'_> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    for byte in self.0 {
                        write!(f, "{byte:02x}")?;
                    }
                    Ok(())
                }
            }
            eprintln!(
                "fe2o3-bf16-private-checked-output-collected-v1 permutation={} consumer={} output_version=12 output_sha256={} output_bytes={} work={} storage={} peak={} observation_storage={} same_account=true actual_checked_output=true fresh_raw_analysis=true consumer_join=true geometry_target_completed=true cleanup_pending=true optimized_formal_admission=false normal_admission=false llvm=false launch_authenticated=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                consumer,
                Hex(&self.output_sha),
                self.output_bytes,
                self.work,
                self.storage,
                self.peak,
                scratch,
            );
        }
    }

    impl PrivateBf16OptimizedV1 {
        pub(crate) fn observe_private_bf16_checked_output_for_test_v1(
            &mut self,
            requested: [u8; 4],
            consumer: &Result<(), PrivateBf16TargetErrorV1>,
        ) -> Result<(), PrivateBf16TargetErrorV1> {
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            let checked = &self.optimization.checked;
            let result = self.target.formal.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(16).map_err(resource)?;
                let [kernel] = checked.owner().module().kernels.as_slice() else {
                    return Err(E::RosterMetadata(
                        "raw output diagnostic requires one actual kernel",
                    ));
                };
                let mut extents = [1_u64; 3];
                for (axis, extent) in kernel.domain.extents().enumerate() {
                    extents[axis] = match extent {
                        fe2o3_kernel_ir::LaunchExtent::Static(n) => u64::from(n),
                        fe2o3_kernel_ir::LaunchExtent::Dynamic => {
                            fe2o3_lower_mir_kernel::PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1
                        }
                    };
                }
                let scratch = sum(
                    std::mem::size_of::<Observation>(),
                    std::mem::size_of::<Attempt>(),
                )?;
                let floor = budget.storage();
                budget.reserve_storage(scratch).map_err(resource)?;
                let mut observation = Observation {
                    ledger: budget.work_ledger_identity_v1(),
                    floor,
                    initial_work: budget.work(),
                    output_sha: *checked.owner().canonical().identity().digest(),
                    output_bytes: checked.owner().canonical().identity().canonical_length(),
                    work: 0,
                    storage: 0,
                    peak: 0,
                };
                let attempt = fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                    checked.owner().module(),
                    &kernel.id,
                    fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                        rank: kernel.domain.rank(),
                        extents,
                    },
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                );
                let inspected = (|| {
                    prepay_attempt(&attempt, budget)?;
                    prepay_consumer(consumer, budget)?;
                    // Raw rows and the original typed consumer error are
                    // compared, not reconstructed from source guard locations.
                    let classification = consumer_join(consumer, &attempt)?;
                    budget.charge_work(64).map_err(resource)?;
                    if budget.work_ledger_identity_v1() != observation.ledger
                        || budget.storage() != sum(observation.floor, scratch)?
                        || budget.work() <= observation.initial_work
                    {
                        return Err(resource(Resource::Accounting));
                    }
                    emit_attempt(&attempt, classification);
                    observation.work = budget.work();
                    observation.storage = budget.storage();
                    observation.peak = budget.peak_storage();
                    // Non-Copy observation is consumed while both selected
                    // headers are reserved. No account-paid scalar escapes.
                    observation.emit(requested, classification, scratch);
                    Ok(())
                })();
                drop(attempt);
                // On an early failure the unused observation is dropped when
                // the FnOnce inspection closure returns, before this refund.
                budget.release_storage(scratch).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                inspected
            });
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16TargetErrorV1::RankedVerification)
        }
    }

    // Parser/join component controls: these do not substitute for the distinct
    // genuine endpoint, which retains the actual checked O and both accounts.
    fn raw_control(kind: u8) -> Attempt {
        use fe2o3_kernel_ir::*;
        let parameters = if kind == 2 {
            vec![
                Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
                Type::F32,
            ]
        } else {
            vec![]
        };
        let arguments = (0..parameters.len()).map(|i| ValueId(i as u32)).collect();
        let mut block = BasicBlock::new(BlockId(0));
        if kind == 1 {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("unknown"),
                    arguments: vec![],
                },
            ));
        } else if kind == 2 {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("raw-output-observer-control");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            arguments,
            vec![block],
        ));
        if kind == 1 {
            module.functions.push(Function::declaration(
                "unknown",
                Signature::new(vec![], vec![]),
            ));
        }
        module.kernels.push(Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        derive_kernel_memory_obligations_for_launch(
            &module,
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [2, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
    }

    #[test]
    fn private_output_raw_join_distinguishes_complete_incomplete_and_conflict() {
        use fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1 as F;
        for kind in 0..3 {
            let raw = raw_control(kind);
            let analysis = raw.as_ref().unwrap();
            let (consumer, expected) = match kind {
                0 => (Ok(()), "complete"),
                1 => (
                    Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
                        F::Incomplete {
                            reasons: analysis.incomplete_reasons().to_vec().into_boxed_slice(),
                        },
                    )),
                    "incomplete",
                ),
                _ => (
                    Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
                        F::InterInvocationConflicts {
                            conflicts: analysis
                                .obligations()
                                .inter_invocation_conflicts()
                                .to_vec()
                                .into_boxed_slice(),
                        },
                    )),
                    "conflict",
                ),
            };
            assert_eq!(consumer_join(&consumer, &raw).unwrap(), expected);
            if kind != 0 {
                assert!(consumer_join(&Ok(()), &raw).is_err());
            }
        }
    }

    #[test]
    fn private_output_raw_join_rejects_changed_reasons_and_keeps_exact_engine_error() {
        use fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1 as F;
        let raw = raw_control(1);
        let wrong = Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
            F::Incomplete {
                reasons: vec![Reason::GuardedAccessRequiresRankedProof {
                    location: fe2o3_kernel_ir::FunctionOperationLocation {
                        block: fe2o3_kernel_ir::BlockId(999),
                        operation_index: 7,
                    },
                }]
                .into_boxed_slice(),
            },
        ));
        assert!(consumer_join(&wrong, &raw).is_err());
        let error = FormalError::MissingKernel {
            kernel: fe2o3_kernel_ir::KernelId::new("missing"),
        };
        let consumer = Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
            F::Analysis(error.clone()),
        ));
        assert_eq!(
            consumer_join(&consumer, &Err(error)).unwrap(),
            "analysis_error"
        );
        assert!(
            consumer_join(
                &consumer,
                &Err(FormalError::MissingKernel {
                    kernel: fe2o3_kernel_ir::KernelId::new("other"),
                })
            )
            .is_err()
        );
    }

    #[test]
    fn private_output_raw_work_is_prepaid_at_exact_and_one_short_boundary() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        use fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1 as F;
        let raw = raw_control(1);
        let consumer = Err(PrivateBf16TargetErrorV1::FormalMemoryAdmission(
            F::Incomplete {
                reasons: raw
                    .as_ref()
                    .unwrap()
                    .incomplete_reasons()
                    .to_vec()
                    .into_boxed_slice(),
            },
        ));
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        prepay_attempt(&raw, &mut budget).unwrap();
        prepay_consumer(&consumer, &mut budget).unwrap();
        let exact = budget.work();
        for cap in [exact, exact - 1] {
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            let result = prepay_attempt(&raw, &mut budget)
                .and_then(|()| prepay_consumer(&consumer, &mut budget));
            assert_eq!(result.is_ok(), cap == exact);
            assert_eq!(budget.storage(), 7);
            if cap != exact {
                let prior = budget.check_prior_denials_v1().unwrap_err();
                assert!(matches!(prepay_attempt(&raw, &mut budget),
                    Err(E::ConditionalResource(error)) if error == prior));
            }
        }
    }

    #[test]
    fn private_output_raw_prior_storage_refusal_wins_before_new_work() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let raw = raw_control(0);
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert!(budget.reserve_storage(1).is_err());
        budget.charge_work(1).unwrap();
        let prior = budget.check_prior_denials_v1().unwrap_err();
        let before = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        for _ in 0..2 {
            assert!(matches!(prepay_attempt(&raw, &mut budget),
                Err(E::ConditionalResource(error)) if error == prior));
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
        }
    }
}

impl PrivateBf16OptimizedV1 {
    /// Separate actual-O structural guard continuation. The original raw
    /// diagnostic and complete-only consumer keep their original refusal.
    /// Source/O/target custody and both accounts remain intact; no report escapes.
    #[allow(dead_code)]
    pub(crate) fn verify_private_bf16_output_guarded_safety_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16TargetErrorV1> {
        self.revalidate_private_bf16_optimization_v1(requested_return, typed_roots, profile)?;
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        let checked = &self.optimization.checked;
        let owner = &self.target.formal.owner;
        let roots = &self.target.formal.verification.roots;
        let result = self.target.formal.verification.phase.with_budget(|budget| {
            budget.check_prior_denials_v1().map_err(resource)?;
            budget.charge_work(2).map_err(resource)?;
            let [root] = roots.as_ref() else {
                return Err(E::RosterMetadata(
                    "private guarded output safety changed source roots",
                ));
            };
            let storage = std::mem::size_of::<Option<Result<(), PrivateBf16TargetErrorV1>>>();
            budget.reserve_storage(storage).map_err(resource)?;
            let mut observed: Option<Result<(), PrivateBf16TargetErrorV1>> = None;
            let loan = owner.with_private_bf16_target_source_v1(
                root.semantic_root,
                requested_return,
                budget,
                |_source_module, function, source_obligations, budget| {
                    // Actual O geometry and exact target idempotence are checked
                    // independently; source obligations provide no O guard proof.
                    observed = Some(
                        check_private_bf16_output_target_v1(
                            checked.owner().module(),
                            function,
                            source_obligations,
                            typed_roots,
                            profile,
                            budget,
                        )
                        .and_then(|()| {
                            owner
                                .with_private_bf16_checked_output_guarded_formal_v1(
                                    checked.owner(),
                                    root.semantic_root,
                                    requested_return,
                                    budget,
                                    |_actual_output_attempt, _guard_result, _budget| Ok(()),
                                )
                                .map_err(PrivateBf16TargetErrorV1::FormalMemoryAdmission)
                        }),
                    );
                    Ok(())
                },
            );
            if let Err(error) = loan {
                drop(observed);
                budget.release_storage(storage).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                return Err(E::FormalMemory(error));
            }
            let observed = observed.ok_or_else(|| resource(Resource::Accounting))?;
            match observed {
                Ok(()) => {
                    budget.release_storage(storage).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Ok(()))
                }
                Err(error) => {
                    budget.release_storage(storage).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Err(error))
                }
            }
        });
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16TargetErrorV1::RankedVerification)?
    }
}

#[cfg(test)]
mod private_checked_output_guard_observer {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, FormalMemoryIncompleteReason as Reason,
        FormalMemoryObligationAnalysis as Analysis, FormalMemoryObligationError as FormalError,
    };
    type Attempt = Result<Analysis, FormalError>;
    fn prepay_error(error: &FormalError, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(32).map_err(resource)?;
        match error {
            FormalError::InvalidModule(errors) => {
                budget
                    .charge_work(errors.diagnostics().len())
                    .map_err(resource)?;
                for row in errors.diagnostics() {
                    budget.charge_work(32).map_err(resource)?;
                    let bytes = row
                        .location
                        .module
                        .as_str()
                        .len()
                        .checked_add(
                            row.location
                                .function
                                .as_ref()
                                .map_or(0, |v| v.as_str().len()),
                        )
                        .and_then(|n| {
                            n.checked_add(
                                row.location.kernel.as_ref().map_or(0, |v| v.as_str().len()),
                            )
                        })
                        .and_then(|n| n.checked_add(row.message.len()))
                        .ok_or_else(|| resource(Resource::Arithmetic))?;
                    budget.charge_work(bytes).map_err(resource)?;
                }
            }
            FormalError::MissingKernel { kernel } => {
                budget
                    .charge_work(kernel.as_str().len())
                    .map_err(resource)?;
            }
            FormalError::InvalidInvocationRange(_) | FormalError::GuardedResource(_) => {}
        }
        Ok(())
    }

    fn prepay_attempt(attempt: &Attempt, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        budget.charge_work(32).map_err(resource)?;
        match attempt {
            Err(error) => prepay_error(error, budget),
            Ok(analysis) => {
                let o = analysis.obligations();
                let rows = o
                    .allocations()
                    .len()
                    .checked_add(o.accesses().len())
                    .and_then(|n| n.checked_add(o.bounds_requirements().len()))
                    .and_then(|n| n.checked_add(o.runtime_alias_requirements().len()))
                    .and_then(|n| n.checked_add(o.inter_invocation_conflicts().len()))
                    .and_then(|n| n.checked_add(analysis.incomplete_reasons().len()))
                    .and_then(|n| n.checked_mul(128))
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                // Known row counts are paid before any traversal/comparison.
                // This selected field-visit allowance is not formatter/RSS work.
                budget.charge_work(rows).map_err(resource)?;
                budget
                    .charge_work(sum(o.kernel().as_str().len(), o.entry().as_str().len())?)
                    .map_err(resource)?;
                for reason in analysis.incomplete_reasons() {
                    if let Reason::CallEffectsUnavailable { callee, .. } = reason {
                        budget
                            .charge_work(callee.as_str().len())
                            .map_err(resource)?;
                    }
                }
                Ok(())
            }
        }
    }

    fn emit_attempt(attempt: &Attempt, consumer: &str) {
        match attempt {
            Ok(analysis) => {
                let o = analysis.obligations();
                eprintln!(
                    "fe2o3-bf16-private-checked-output-guard-summary-v1 consumer={} analysis={} allocations={} accesses={} bounds={} aliases={} conflicts={} incomplete_reasons={} optimized_formal_admission=false",
                    consumer,
                    if analysis.is_complete() {
                        "complete"
                    } else {
                        "incomplete"
                    },
                    o.allocations().len(),
                    o.accesses().len(),
                    o.bounds_requirements().len(),
                    o.runtime_alias_requirements().len(),
                    o.inter_invocation_conflicts().len(),
                    analysis.incomplete_reasons().len(),
                );
                eprintln!(
                    "fe2o3-bf16-private-checked-output-guard-header-v1 kernel={:?} entry={:?} index_width={:?} basis={:?} invocations={:?}",
                    o.kernel(),
                    o.entry(),
                    o.index_width(),
                    o.analysis_basis(),
                    o.invocations(),
                );
                for (i, row) in o.allocations().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-allocation-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.accesses().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-access-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.bounds_requirements().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-bounds-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.runtime_alias_requirements().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-alias-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in o.inter_invocation_conflicts().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-conflict-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
                for (i, row) in analysis.incomplete_reasons().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-reason-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
            }
            Err(FormalError::InvalidModule(errors)) => {
                eprintln!(
                    "fe2o3-bf16-private-checked-output-guard-error-v1 consumer={} kind=InvalidModule diagnostics={}",
                    consumer,
                    errors.diagnostics().len()
                );
                for (i, row) in errors.diagnostics().iter().enumerate() {
                    eprintln!(
                        "fe2o3-bf16-private-checked-output-guard-verifier-v1 ordinal={} value={:?}",
                        i, row
                    );
                }
            }
            Err(FormalError::MissingKernel { kernel }) => eprintln!(
                "fe2o3-bf16-private-checked-output-guard-error-v1 consumer={} kind=MissingKernel value={:?}",
                consumer, kernel
            ),
            Err(FormalError::InvalidInvocationRange(error)) => eprintln!(
                "fe2o3-bf16-private-checked-output-guard-error-v1 consumer={} kind=InvalidInvocationRange value={:?}",
                consumer, error
            ),
            Err(FormalError::GuardedResource(error)) => eprintln!(
                "fe2o3-bf16-private-checked-output-guard-error-v1 consumer={} kind=GuardedResource value={:?}",
                consumer, error
            ),
        }
    }

    // Non-Copy record only exists inside the surviving original phase loan.
    struct Observation {
        ledger: Ledger,
        floor: usize,
        initial_work: usize,
        output_sha: [u8; 32],
        output_bytes: u64,
        discharged_reasons: usize,
        work: usize,
        storage: usize,
        peak: usize,
    }
    impl Observation {
        fn emit(self, requested: [u8; 4], scratch: usize) {
            struct Hex<'a>(&'a [u8; 32]);
            impl std::fmt::Display for Hex<'_> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    for byte in self.0 {
                        write!(f, "{byte:02x}")?;
                    }
                    Ok(())
                }
            }
            eprintln!(
                "fe2o3-bf16-private-checked-output-guard-collected-v1 permutation={} consumer=guarded output_version=12 output_sha256={} output_bytes={} discharged_reasons={} work={} storage={} peak={} observation_storage={} same_account=true actual_checked_output=true fresh_raw_analysis=true actual_output_guard_proved=true raw_incomplete_preserved=true geometry_target_completed=true cleanup_pending=true optimized_formal_admission=false normal_admission=false llvm=false launch_authenticated=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                Hex(&self.output_sha),
                self.output_bytes,
                self.discharged_reasons,
                self.work,
                self.storage,
                self.peak,
                scratch,
            );
        }
    }

    impl PrivateBf16OptimizedV1 {
        pub(crate) fn observe_private_bf16_checked_output_guard_for_test_v1(
            &mut self,
            requested: [u8; 4],
        ) -> Result<(), PrivateBf16TargetErrorV1> {
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            let checked = &self.optimization.checked;
            let owner = &self.target.formal.owner;
            let roots = &self.target.formal.verification.roots;
            let result = self.target.formal.verification.phase.with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                budget.charge_work(16).map_err(resource)?;
                let [root] = roots.as_ref() else {
                    return Err(E::RosterMetadata("actual O guard observer changed source roots"));
                };
                let slot_storage = std::mem::size_of::<Option<Result<(), E>>>();
                budget.reserve_storage(slot_storage).map_err(resource)?;
                let mut observed: Option<Result<(), E>> = None;
                // This is another fresh, independently charged actual O analysis
                // and structural proof, not replay of the shipping unit result.
                let loan = owner.with_private_bf16_checked_output_guarded_formal_v1(
                    checked.owner(), root.semantic_root, requested, budget,
                    |attempt, proof, budget| {
                        observed = Some((|| {
                            prepay_attempt(attempt, budget)?;
                            budget.charge_work(64).map_err(resource)?;
                            emit_attempt(attempt, "guarded");
                            if let Err(detail) = proof {
                                eprintln!("fe2o3-bf16-private-checked-output-guard-refusal-v1 detail={:?}", detail);
                                return Err(E::RosterMetadata("actual O structural proof refused"));
                            }
                            let analysis = attempt.as_ref().map_err(|_| E::RosterMetadata(
                                "actual O guard observer requires a fresh raw analysis",
                            ))?;
                            if analysis.is_complete() || analysis.incomplete_reasons().is_empty()
                                || !analysis.obligations().inter_invocation_conflicts().is_empty()
                                || analysis.incomplete_reasons().iter().any(|r| !matches!(r,
                                    Reason::GuardedAccessRequiresRankedProof { .. }))
                            {
                                return Err(E::RosterMetadata("actual O raw guarded reason roster changed"));
                            }
                            let scratch = std::mem::size_of::<Observation>();
                            let floor = budget.storage();
                            budget.reserve_storage(scratch).map_err(resource)?;
                            let mut observation = Observation {
                                ledger: budget.work_ledger_identity_v1(), floor,
                                initial_work: budget.work(),
                                output_sha: *checked.owner().canonical().identity().digest(),
                                output_bytes: checked.owner().canonical().identity().canonical_length(),
                                discharged_reasons: analysis.incomplete_reasons().len(),
                                work: 0, storage: 0, peak: 0,
                            };
                            let inspected = (|| {
                                budget.charge_work(64).map_err(resource)?;
                                if budget.work_ledger_identity_v1() != observation.ledger
                                    || budget.storage() != sum(observation.floor, scratch)?
                                    || budget.work() <= observation.initial_work
                                {
                                    return Err(resource(Resource::Accounting));
                                }
                                observation.work = budget.work();
                                observation.storage = budget.storage();
                                observation.peak = budget.peak_storage();
                                observation.emit(requested, scratch);
                                Ok(())
                            })();
                            // The capture dies on success or early returned error
                            // before its exact refund. Raw/proof remain lent and
                            // paid by the unchanged inner lowerer frame.
                            budget.release_storage(scratch).map_err(resource)?;
                            budget.check_prior_denials_v1().map_err(resource)?;
                            inspected
                        })());
                        Ok(())
                    },
                );
                if let Err(error) = loan {
                    drop(observed);
                    budget.release_storage(slot_storage).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    return Err(E::FormalMemory(error));
                }
                let observed = observed.ok_or_else(|| resource(Resource::Accounting))?;
                match observed {
                    Ok(()) => {
                        budget.release_storage(slot_storage).map_err(resource)?;
                        budget.check_prior_denials_v1().map_err(resource)?;
                        Ok(())
                    }
                    Err(error) => {
                        budget.release_storage(slot_storage).map_err(resource)?;
                        budget.check_prior_denials_v1().map_err(resource)?;
                        Err(error)
                    }
                }
            });
            self.target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16TargetErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16TargetErrorV1::RankedVerification)
        }
    }
}

type PrivateBf16LlvmErrorV1 = crate::production_pipeline::ProductionPipelineError;
type PrivateBf16LlvmResultV1 = Result<PrivateBf16LlvmPayloadV1, PrivateBf16LlvmErrorV1>;

struct PrivateBf16LlvmPayloadV1 {
    worker: String,
    dialect: String,
    retained: usize,
}

/// Both LLVM strings drop before their actual checked O/B/nominal/formal/target
/// owner and original projection phase. Caller retains materialization separately.
/// This private stage is inert text, not descriptor/Worker/artifact/launch authority.
#[allow(dead_code)]
pub(crate) struct PrivateBf16LlvmV1 {
    llvm: PrivateBf16LlvmPayloadV1,
    optimized: PrivateBf16OptimizedV1,
}

fn private_bf16_llvm_context_v1(reason: &'static str) -> PrivateBf16LlvmErrorV1 {
    PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm(
        dialect_amdgcn::PrivateBf16NativeLlvmErrorV1::Context(reason),
    )
}

fn private_bf16_llvm_caps_v1() -> Result<usize, E> {
    sum(
        dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
        dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1,
    )
}

fn reserve_private_bf16_llvm_output_v1(budget: &mut Budget<'_>) -> Result<(usize, usize), E> {
    budget.check_prior_denials_v1().map_err(resource)?;
    budget.charge_work(1).map_err(resource)?;
    let retained = std::mem::size_of::<PrivateBf16LlvmV1>();
    let scratch = std::mem::size_of::<Option<PrivateBf16LlvmResultV1>>();
    budget
        .reserve_storage(sum(retained, scratch)?)
        .map_err(resource)?;
    Ok((retained, scratch))
}

/// Construction-only scope with no callbacks. The existing two text ceilings
/// are reserved before either emitter or binder can allocate a returned String.
/// Their inherited trees/formatters/internal allocations remain excluded; this
/// is logical retained text/header accounting, not an entire-engine/RSS bound.
fn derive_private_bf16_llvm_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    requested_return: [u8; 4],
    budget: &mut Budget<'_>,
) -> PrivateBf16LlvmResultV1 {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget
            .charge_work(3)
            .map_err(private_bf16_target_resource_v1)?;
        let header = std::mem::size_of::<PrivateBf16LlvmPayloadV1>();
        let caps =
            private_bf16_llvm_caps_v1().map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        budget
            .reserve_storage(
                header
                    .checked_add(caps)
                    .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
            )
            .map_err(private_bf16_target_resource_v1)?;
        let dialect = dialect_amdgcn::lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
            output,
            requested_return,
            budget,
        )
        .map_err(PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm)?;
        if dialect.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES {
            return Err(private_bf16_llvm_context_v1(
                "dialect text exceeded its existing ceiling",
            ));
        }
        // Pay selected complete text scans and copying before the existing
        // exact canonical-header/unique-header LLVM22 layout binder runs.
        budget
            .charge_work(
                dialect
                    .len()
                    .checked_mul(8)
                    .and_then(|n| {
                        n.checked_add(
                            fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
                                .len()
                                .checked_mul(2)?,
                        )
                    })
                    .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
            )
            .map_err(private_bf16_target_resource_v1)?;
        let worker = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&dialect)
            .map_err(PrivateBf16LlvmErrorV1::UpstreamLlvmLayoutBinding)?;
        if worker.len() > dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1 {
            return Err(private_bf16_llvm_context_v1(
                "worker text exceeded its existing ceiling",
            ));
        }
        let text = dialect
            .len()
            .checked_add(worker.len())
            .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
        let retained = header
            .checked_add(text)
            .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
        budget
            .release_storage(
                caps.checked_sub(text)
                    .ok_or_else(|| private_bf16_target_resource_v1(Resource::Accounting))?,
            )
            .map_err(private_bf16_target_resource_v1)?;
        if budget.storage().checked_sub(floor) != Some(retained) {
            return Err(private_bf16_target_resource_v1(Resource::Accounting));
        }
        Ok(PrivateBf16LlvmPayloadV1 {
            worker,
            dialect,
            retained,
        })
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm(
                dialect_amdgcn::PrivateBf16NativeLlvmErrorV1::Panicked,
            ))
        }
    };
    let prior = budget.check_prior_denials_v1();
    if result.is_ok() && prior.is_ok() {
        return result;
    }
    let error = match result {
        Ok(payload) => {
            drop(payload);
            private_bf16_target_resource_v1(prior.expect_err("prior denial checked"))
        }
        Err(error) => error,
    };
    // Failure has dropped all newly created strings/context before refund.
    // No callback or preexisting owner's reservation can be above this floor.
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    if let Err(error) = prior {
        return Err(private_bf16_target_resource_v1(error));
    }
    restored.map_err(private_bf16_target_resource_v1)?;
    Err(error)
}

fn recheck_private_bf16_llvm_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    retained: &PrivateBf16LlvmPayloadV1,
    requested_return: [u8; 4],
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let fresh = derive_private_bf16_llvm_v1(output, requested_return, budget)?;
        let joined = (|| {
            budget
                .charge_work(
                    retained
                        .dialect
                        .len()
                        .checked_add(retained.worker.len())
                        .and_then(|n| n.checked_add(fresh.dialect.len()))
                        .and_then(|n| n.checked_add(fresh.worker.len()))
                        .and_then(|n| n.checked_add(3))
                        .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
                )
                .map_err(private_bf16_target_resource_v1)?;
            if retained.retained != fresh.retained
                || retained.dialect != fresh.dialect
                || retained.worker != fresh.worker
            {
                return Err(private_bf16_llvm_context_v1(
                    "retained LLVM bytes differ from actual O replay",
                ));
            }
            Ok(())
        })();
        drop(fresh);
        joined
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm(
                dialect_amdgcn::PrivateBf16NativeLlvmErrorV1::Panicked,
            ))
        }
    };
    // This no-callback scratch scope retains no new payload on any path.
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    restored.map_err(private_bf16_target_resource_v1)?;
    result
}

impl PrivateBf16OptimizedV1 {
    #[allow(dead_code)]
    pub(crate) fn lower_private_bf16_llvm_v1(
        mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<PrivateBf16LlvmV1, PrivateBf16LlvmErrorV1> {
        // Replays actual source/Return/target/B→O and freshly discharges actual
        // O guards. Neither a prior unit result nor source guard coordinates
        // authorize this consuming step. Runtime bounds/aliases remain duties.
        self.verify_private_bf16_output_guarded_safety_v1(requested_return, typed_roots, profile)?;
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        let output = self.optimization.checked.owner();
        let result = self.target.formal.verification.phase.with_budget(|budget| {
            let (retained, scratch) = reserve_private_bf16_llvm_output_v1(budget)?;
            match derive_private_bf16_llvm_v1(output, requested_return, budget) {
                Ok(llvm) => {
                    budget.release_storage(scratch).map_err(resource)?;
                    Ok(Ok(llvm))
                }
                Err(error) => {
                    budget
                        .release_storage(sum(retained, scratch)?)
                        .map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    Ok(Err(error))
                }
            }
        });
        self.target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        let llvm = result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)??;
        Ok(PrivateBf16LlvmV1 {
            llvm,
            optimized: self,
        })
    }
}

impl PrivateBf16LlvmV1 {
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_llvm_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        self.optimized
            .verify_private_bf16_output_guarded_safety_v1(requested_return, typed_roots, profile)?;
        self.optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        let output = self.optimized.optimization.checked.owner();
        let retained = &self.llvm;
        let result = self
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                budget.check_prior_denials_v1().map_err(resource)?;
                Ok(recheck_private_bf16_llvm_v1(
                    output,
                    retained,
                    requested_return,
                    budget,
                ))
            });
        self.optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod private_bf16_llvm_owner_controls {
    use super::*;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
    };
    fn source(permutation: [u8; 4], trapped: bool) -> Module {
        let parameters = (0..12)
            .map(|i| {
                if i < 8 {
                    Type::Scalar(ScalarType::Bf16)
                } else {
                    Type::F32
                }
            })
            .collect::<Vec<_>>();
        let mut call = BasicBlock::new(BlockId(0));
        call.operations.push(Operation::new(
            (12..16)
                .map(|i| ValueDef::new(ValueId(i), Type::F32))
                .collect(),
            OperationKind::Call {
                callee: FunctionId::new("mfma"),
                arguments: (0..12).map(ValueId).collect(),
            },
        ));
        if trapped {
            call.operations
                .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        }
        call.terminator = Some(if trapped {
            Terminator::Unreachable
        } else {
            Terminator::Return { values: vec![] }
        });
        let mut blocks = vec![call];
        if trapped {
            // A distinct reachable branch supplies the root's sole normal Return.
            let mut start = BasicBlock::new(BlockId(2));
            start.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(16), Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            ));
            start.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(16),
                then_target: BlockId(0),
                then_arguments: vec![],
                else_target: BlockId(1),
                else_arguments: vec![],
            });
            let mut done = BasicBlock::new(BlockId(1));
            done.terminator = Some(Terminator::Return { values: vec![] });
            blocks = vec![start, blocks.pop().unwrap(), done];
        }
        let mut root = Function::kernel_entry(
            "entry",
            Signature::new(parameters.clone(), vec![]),
            (0..12).map(ValueId).collect(),
            blocks,
        );
        root.required_capabilities = root.derived_capabilities();
        // Calls do not derive the capability of their narrow-float arguments.
        root.required_capabilities
            .insert(TargetCapability::BFloat16);
        let mut body = BasicBlock::new(BlockId(0));
        body.operations.push(Operation::new(
            (12..16)
                .map(|i| ValueDef::new(ValueId(i), Type::F32))
                .collect(),
            OperationKind::Matrix(
                MatrixOperation::multiply_accumulate(
                    [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                    [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                    [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
                )
                .with_declared_tensor_layout(
                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                        .with_zero_filled_predicate_inputs(),
                ),
            ),
        ));
        body.terminator = Some(Terminator::Return {
            values: permutation
                .into_iter()
                .map(|i| ValueId(12 + u32::from(i)))
                .collect(),
        });
        let mut helper = Function::internal_helper(
            "mfma",
            Signature::new(parameters, vec![Type::F32; 4]),
            (0..12).map(ValueId).collect(),
            vec![body],
        );
        helper.required_capabilities = helper.derived_capabilities();
        let mut module = Module::new("private_bf16_native_control");
        module.functions = vec![root, helper];
        if trapped {
            module
                .functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration());
        }
        let mut kernel = Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn with_owner(source: &Module, inspect: impl FnOnce(&Owner, &mut Budget<'_>)) {
        let bound = dialect_amdgcn::bind_production_target_v1(source, Profile::Gfx942).unwrap();
        let mut work = Work::new(1_000_000_000_000);
        let mut budget = Budget::new(&mut work, 128 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let (owner, storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        inspect(&owner, &mut budget);
        drop(owner);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 7);
    }
    #[test]
    fn llvm_owner_header_exact_short_and_original_prior_denial() {
        let bytes = std::mem::size_of::<PrivateBf16LlvmV1>()
            + std::mem::size_of::<Option<PrivateBf16LlvmResultV1>>();
        for cap in [7 + bytes, 6 + bytes] {
            let mut work = Work::new(1);
            let mut budget = Budget::new(&mut work, cap);
            budget.reserve_storage(7).unwrap();
            let result = reserve_private_bf16_llvm_output_v1(&mut budget);
            assert_eq!(result.is_ok(), cap == 7 + bytes);
            if let Ok((retained, scratch)) = result {
                assert_eq!(retained + scratch, bytes);
                budget.release_storage(bytes).unwrap();
            } else {
                let prior = budget.check_prior_denials_v1().unwrap_err();
                let before = (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                );
                for _ in 0..2 {
                    assert!(matches!(reserve_private_bf16_llvm_output_v1(&mut budget),
                        Err(E::ConditionalResource(error)) if error==prior));
                    assert_eq!(
                        (
                            budget.work(),
                            budget.storage(),
                            budget.failed_work(),
                            budget.failed_storage()
                        ),
                        before
                    );
                }
            }
            assert_eq!(budget.storage(), 7);
        }
    }
    #[test]
    fn actual_model_and_worker_texts_are_retained_compared_and_dropped_before_refund() {
        for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
            with_owner(&source(permutation, false), |owner, budget| {
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let mut payload = derive_private_bf16_llvm_v1(owner, permutation, budget).unwrap();
                let live = payload.retained;
                assert_eq!(budget.storage(), floor + live);
                assert_eq!(
                    live,
                    std::mem::size_of::<PrivateBf16LlvmPayloadV1>()
                        + payload.dialect.len()
                        + payload.worker.len()
                );
                assert!(
                    payload
                        .worker
                        .contains(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1)
                );
                recheck_private_bf16_llvm_v1(owner, &payload, permutation, budget).unwrap();
                assert_eq!(budget.storage(), floor + live);
                payload.worker.replace_range(0..1, "X");
                assert!(matches!(
                    recheck_private_bf16_llvm_v1(owner, &payload, permutation, budget),
                    Err(PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm(
                        dialect_amdgcn::PrivateBf16NativeLlvmErrorV1::Context(
                            "retained LLVM bytes differ from actual O replay"
                        )
                    ))
                ));
                payload.worker.replace_range(0..1, "t");
                assert!(
                    recheck_private_bf16_llvm_v1(owner, &payload, [0, 0, 2, 3], budget).is_err()
                );
                assert_eq!(budget.storage(), floor + live);
                assert!(budget.work_ledger_identity_v1() == ledger);
                drop(payload);
                budget.release_storage(live).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
    #[test]
    fn malformed_actual_owner_does_not_publish_partial_llvm_or_retained_credit() {
        let mut module = source([0, 1, 2, 3], false);
        module.functions[1].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(
                0,
                Operation::effect_free(
                    ValueDef::new(ValueId(16), Type::F32),
                    OperationKind::Constant(Constant::F32Bits(0)),
                ),
            );
        with_owner(&module, |owner, budget| {
            let floor = budget.storage();
            assert!(matches!(
                derive_private_bf16_llvm_v1(owner, [0, 1, 2, 3], budget),
                Err(PrivateBf16LlvmErrorV1::PrivateBf16NativeLlvm(
                    dialect_amdgcn::PrivateBf16NativeLlvmErrorV1::Context("extra helper operation")
                ))
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
    #[test]
    fn both_text_ceilings_are_prepaid_before_emission_on_original_account() {
        with_owner(&source([0, 1, 2, 3], false), |owner, budget| {
            let needed = std::mem::size_of::<PrivateBf16LlvmPayloadV1>()
                + private_bf16_llvm_caps_v1().unwrap();
            let padding = 128 * 1024 * 1024 - budget.storage() - (needed - 1);
            budget.reserve_storage(padding).unwrap();
            let floor = budget.storage();
            assert!(derive_private_bf16_llvm_v1(owner, [0, 1, 2, 3], budget).is_err());
            assert_eq!(budget.storage(), floor);
            let prior = budget.check_prior_denials_v1().unwrap_err();
            assert!(matches!(prior, Resource::Storage(_)));
            let before = (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(
                    matches!(derive_private_bf16_llvm_v1(owner,[0,1,2,3],budget),
                    Err(PrivateBf16LlvmErrorV1::RankedVerification(E::ConditionalResource(error))) if error==prior)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
            budget.release_storage(padding).unwrap();
        });
    }
}

#[cfg(test)]
mod private_owning_llvm_observer_v1 {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    use sha2::{Digest, Sha256};

    // Non-Copy paid row: all hashes/counters are consumed before exact refund.
    struct Observation {
        ledger: Ledger,
        protected: usize,
        initial_work: usize,
        output_sha: [u8; 32],
        output_bytes: u64,
        dialect_sha: [u8; 32],
        worker_sha: [u8; 32],
        dialect_bytes: usize,
        worker_bytes: usize,
        retained: usize,
        work: usize,
        storage: usize,
        peak: usize,
    }
    struct Hex<'a>(&'a [u8; 32]);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    impl Observation {
        fn emit(self, requested: [u8; 4], scratch: usize) {
            eprintln!(
                "fe2o3-bf16-private-owning-llvm-collected-v1 requested={},{},{},{} output_sha256={} output_bytes={} dialect_sha256={} dialect_bytes={} worker_sha256={} worker_bytes={} retained_llvm_storage={} observer_storage={} work={} storage={} peak={} same_ledger=true actual_output_owner=true full_text_replayed=true worker_layout_bound=true mfma_calls=1 anchor_absence_records=1 cleanup_pending=true llvm_emitted=true worker_invoked=false descriptor_constructed=false normal_admission=false launch_authenticated=false artifact_authority=false",
                requested[0],
                requested[1],
                requested[2],
                requested[3],
                Hex(&self.output_sha),
                self.output_bytes,
                Hex(&self.dialect_sha),
                self.dialect_bytes,
                Hex(&self.worker_sha),
                self.worker_bytes,
                self.retained,
                scratch,
                self.work,
                self.storage,
                self.peak,
            );
        }
    }
    fn inspect_text_v1(llvm: &PrivateBf16LlvmPayloadV1, budget: &mut Budget<'_>) -> Result<(), E> {
        const CALL: &str = " = call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(";
        const ABSENCE: &str = "!fe2o3.semantic_anchor.absence.v1 =";
        budget.check_prior_denials_v1().map_err(resource)?;
        let bytes = llvm
            .dialect
            .len()
            .checked_add(llvm.worker.len())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        // Selected full-text passes, including later two hashes. These do not
        // claim that all inherited formatter/control stack storage is metered.
        budget
            .charge_work(
                bytes
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )
            .map_err(resource)?;
        if llvm.dialect.is_empty()
            || llvm.worker.is_empty()
            || llvm.dialect.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
            || llvm.worker.len() > dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1
            || llvm.retained != sum(std::mem::size_of::<PrivateBf16LlvmPayloadV1>(), bytes)?
            || llvm.dialect.matches(CALL).count() != 1
            || llvm.worker.matches(CALL).count() != 1
            || llvm.dialect.matches(ABSENCE).count() != 1
            || llvm.worker.matches(ABSENCE).count() != 1
            || llvm.dialect.contains("call void @llvm.pseudoprobe")
            || llvm.worker.contains("call void @llvm.pseudoprobe")
            || llvm
                .worker
                .matches(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1)
                .count()
                != 1
        {
            return Err(E::RosterMetadata("actual retained LLVM shape differs"));
        }
        Ok(())
    }

    impl PrivateBf16LlvmV1 {
        /// Called only after the actual returned owner has completed full
        /// revalidation. This paid row has no source/descriptor/launch authority.
        pub(crate) fn observe_private_bf16_llvm_for_test_v1(
            &mut self,
            requested: [u8; 4],
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            let output = self.optimized.optimization.checked.owner();
            let llvm = &self.llvm;
            let result = self
                .optimized
                .target
                .formal
                .verification
                .phase
                .with_budget(|budget| {
                    budget.check_prior_denials_v1().map_err(resource)?;
                    budget.charge_work(4).map_err(resource)?;
                    if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3]) {
                        return Err(E::RosterMetadata("unsupported actual LLVM Return request"));
                    }
                    let scratch = std::mem::size_of::<Observation>();
                    let floor = budget.storage();
                    budget.reserve_storage(scratch).map_err(resource)?;
                    let mut row = Observation {
                        ledger: budget.work_ledger_identity_v1(),
                        protected: sum(floor, scratch)?,
                        initial_work: budget.work(),
                        output_sha: *output.canonical().identity().digest(),
                        output_bytes: output.canonical().identity().canonical_length(),
                        dialect_sha: [0; 32],
                        worker_sha: [0; 32],
                        dialect_bytes: llvm.dialect.len(),
                        worker_bytes: llvm.worker.len(),
                        retained: llvm.retained,
                        work: 0,
                        storage: 0,
                        peak: 0,
                    };
                    let inspected = (|| {
                        inspect_text_v1(llvm, budget)?;
                        row.dialect_sha = Sha256::digest(llvm.dialect.as_bytes()).into();
                        row.worker_sha = Sha256::digest(llvm.worker.as_bytes()).into();
                        budget.check_prior_denials_v1().map_err(resource)?;
                        if budget.work_ledger_identity_v1() != row.ledger
                            || budget.storage() != row.protected
                            || budget.work() <= row.initial_work
                            || budget.peak_storage() < budget.storage()
                        {
                            return Err(resource(Resource::Accounting));
                        }
                        row.work = budget.work();
                        row.storage = budget.storage();
                        row.peak = budget.peak_storage();
                        row.emit(requested, scratch);
                        Ok(())
                    })();
                    // FnOnce capture is consumed on success or dropped on refusal;
                    // neither the paid row nor a Copy counter escapes the loan.
                    budget.release_storage(scratch).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    inspected
                });
            self.optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)
        }
    }

    #[test]
    fn llvm_observer_prepaid_shape_refusal_preserves_same_account_floor_and_sticky_error() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let retained = std::mem::size_of::<PrivateBf16LlvmPayloadV1>() + 16;
        let exact = 8 * 16 + 128;
        for cap in [exact, exact - 1] {
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, retained + 7);
            budget.reserve_storage(retained + 7).unwrap();
            let payload = PrivateBf16LlvmPayloadV1 {
                worker: "not LLVM".into(),
                dialect: "not LLVM".into(),
                retained,
            };
            let id = budget.work_ledger_identity_v1();
            let result = inspect_text_v1(&payload, &mut budget);
            if cap == exact {
                assert!(matches!(
                    result,
                    Err(E::RosterMetadata("actual retained LLVM shape differs"))
                ));
            } else {
                let prior = budget.check_prior_denials_v1().unwrap_err();
                assert!(matches!(result,Err(E::ConditionalResource(error)) if error==prior));
                let before = (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                );
                assert!(
                    matches!(inspect_text_v1(&payload,&mut budget),Err(E::ConditionalResource(error)) if error==prior)
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
            assert!(budget.work_ledger_identity_v1() == id);
            assert_eq!(budget.storage(), retained + 7);
            drop(payload);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 7);
        }
    }
}

// Private descriptor continuation: neither checked-output policy admission nor
// a signed-source Worker stage. The source/formal and actual O owners stay live.
struct PrivateBf16DescriptorPayloadV1 {
    canonical_descriptor: Box<[u8]>,
    final_llvm: Box<str>,
    retained: usize,
}

#[allow(dead_code)]
pub(crate) struct PrivateBf16DescriptorV1 {
    // New bytes die before both LLVM strings, actual O/B/source/formal owners,
    // and their original projection account.
    descriptor: PrivateBf16DescriptorPayloadV1,
    llvm: PrivateBf16LlvmV1,
}

type PrivateBf16DescriptorResultV1 = Result<PrivateBf16DescriptorPayloadV1, PrivateBf16LlvmErrorV1>;

fn private_bf16_descriptor_context_v1(reason: &'static str) -> PrivateBf16LlvmErrorV1 {
    PrivateBf16LlvmErrorV1::DescriptorEvidence(
        crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(reason),
    )
}

fn private_bf16_descriptor_caps_v1() -> Result<usize, E> {
    sum(
        dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
        fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES,
    )
}

/// The caller prepays the two returned boxed payload ceilings before lending
/// source or O. The inherited descriptor/geometry/module-symbol engines and
/// their temporary trees remain excluded; this is not total stack/heap/RSS.
#[allow(clippy::too_many_arguments)]
fn construct_private_bf16_descriptor_payload_prepaid_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    worker: &str,
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    source_launch: &fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
    obligations: &fe2o3_kernel_ir::FormalMemoryObligations,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> PrivateBf16DescriptorResultV1 {
    use crate::production_worker_handoff::ProductionWorkerHandoffError as HandoffError;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(
            worker
                .len()
                .checked_add(8)
                .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
        )
        .map_err(private_bf16_target_resource_v1)?;
    #[cfg(test)]
    private_descriptor_constructor_failure_controls::constructor_entered();
    let target = fe2o3_compiler_ffi::DeviceTargetV1::parse("gfx942:xnack-")
        .expect("closed private BF16 target");
    let compiler_module =
        crate::kernel_ir_codegen::retain_private_bf16_checked_compiler_module_text_v1(
            output,
            worker.to_owned(),
        )
        .map_err(|error| {
            PrivateBf16LlvmErrorV1::WorkerHandoff(HandoffError::CompilerModule(error))
        })?;
    let envelope = crate::production_worker_handoff::derive_production_compiler_ffi_envelope(
        target,
        output.module(),
        &compiler_module,
        None,
        *output.canonical().identity().digest(),
    )
    .map_err(PrivateBf16LlvmErrorV1::WorkerHandoff)?;
    let descriptor =
        crate::compiler_descriptor::private_bf16_v1::construct_private_bf16_descriptor_source_v1(
            &envelope,
            &compiler_module,
            typed_roots,
            semantic,
            source_launch,
            output,
            obligations,
        )
        .map_err(PrivateBf16LlvmErrorV1::DescriptorEvidence)?;
    if descriptor.canonical_bytes().len() > fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(private_bf16_descriptor_context_v1(
            "private BF16 descriptor byte ceiling",
        ));
    }
    budget
        .charge_work(
            descriptor
                .canonical_bytes()
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(compiler_module.llvm_ir().len()))
                .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
        )
        .map_err(private_bf16_target_resource_v1)?;
    let compiler_module =
        crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(compiler_module, &descriptor)
            .map_err(|error| {
                PrivateBf16LlvmErrorV1::WorkerHandoff(HandoffError::CompilerModule(error))
            })?;
    if compiler_module.llvm_ir().len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES {
        return Err(private_bf16_descriptor_context_v1(
            "private BF16 final LLVM byte ceiling",
        ));
    }
    budget
        .charge_work(
            descriptor
                .canonical_bytes()
                .len()
                .checked_add(compiler_module.llvm_ir().len())
                .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?,
        )
        .map_err(private_bf16_target_resource_v1)?;
    let canonical_descriptor = descriptor.canonical_bytes().to_vec().into_boxed_slice();
    let final_llvm = compiler_module.llvm_ir().to_owned().into_boxed_str();
    let retained = std::mem::size_of::<PrivateBf16DescriptorV1>()
        .checked_add(canonical_descriptor.len())
        .and_then(|n| n.checked_add(final_llvm.len()))
        .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
    // The decoded table and temporary compiler module drop before this boxed
    // inert payload escapes. Neither is an admitted owner or runtime witness.
    #[cfg(test)]
    private_descriptor_constructor_failure_controls::payload_made();
    Ok(PrivateBf16DescriptorPayloadV1 {
        canonical_descriptor,
        final_llvm,
        retained,
    })
}

/// No caller callback and no generic budget-scoped returning wrapper: retained
/// reservations remain charged continuously while either payload is live.
#[allow(clippy::too_many_arguments)]
fn derive_private_bf16_descriptor_v1(
    owner: &fe2o3_lower_mir_kernel::ProductionPrivateBf16FormalMemoryOwnerV1,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    semantic_root: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    requested_return: [u8; 4],
    worker: &str,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> PrivateBf16DescriptorResultV1 {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let mut prepaid = 0usize;
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepaid = reserve_private_bf16_descriptor_output_v1(budget)?;
        let mut observed: Option<PrivateBf16DescriptorResultV1> = None;
        let source_loan = owner.with_private_bf16_descriptor_source_v1(
            semantic_root,
            requested_return,
            budget,
            |_source, _function, _source_obligations, semantic, source_launch, budget| {
                let output_loan = owner.with_private_bf16_checked_output_guarded_formal_v1(
                    output,
                    semantic_root,
                    requested_return,
                    budget,
                    |attempt, guard, budget| {
                        if let (Ok(analysis), Ok(())) = (attempt, guard) {
                            // Exactly the fresh raw report's obligations; no
                            // Complete relabel and no source-coordinate substitute.
                            observed = Some(construct_private_bf16_descriptor_payload_prepaid_v1(
                                output,
                                worker,
                                semantic,
                                source_launch,
                                analysis.obligations(),
                                typed_roots,
                                budget,
                            ));
                        }
                        Ok(())
                    },
                );
                if let Err(error) = output_loan {
                    drop(observed.take());
                    observed = Some(Err(PrivateBf16LlvmErrorV1::FormalMemoryAdmission(error)));
                }
                Ok(())
            },
        );
        if let Err(error) = source_loan {
            drop(observed);
            return Err(PrivateBf16LlvmErrorV1::FormalMemoryAdmission(error));
        }
        #[cfg(test)]
        if observed.as_ref().is_some_and(Result::is_ok) {
            // Both borrowed source/actual-O guard frames have closed. A test
            // panic now must drop the captured real payload before reconciliation.
            private_descriptor_constructor_failure_controls::after_capture();
        }
        observed.ok_or_else(|| private_bf16_target_resource_v1(Resource::Accounting))?
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(private_bf16_descriptor_context_v1(
                "private BF16 descriptor construction panicked",
            ))
        }
    };
    finish_private_bf16_descriptor_attempt_v1(result, floor, ledger, slot, prepaid, budget)
}

// Exact private result reconciliation, extracted unchanged for denial/drop tests.
// It accepts only this inert payload Result; no callback, owner or generic escape.
fn finish_private_bf16_descriptor_attempt_v1(
    result: PrivateBf16DescriptorResultV1,
    floor: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    prepaid: usize,
    budget: &mut Budget<'_>,
) -> PrivateBf16DescriptorResultV1 {
    let prior = budget.check_prior_denials_v1();
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage().checked_sub(floor) != Some(prepaid)
    {
        drop(result);
        prior.map_err(private_bf16_target_resource_v1)?;
        // Never refund an unknown callback surplus or a foreign account.
        return Err(private_bf16_target_resource_v1(Resource::Accounting));
    }
    match (result, prior) {
        (Ok(payload), Ok(())) => {
            let Some(unused) = prepaid.checked_sub(payload.retained) else {
                drop(payload);
                budget
                    .release_storage(prepaid)
                    .map_err(private_bf16_target_resource_v1)?;
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            };
            if let Err(error) = budget.release_storage(unused) {
                drop(payload);
                return Err(private_bf16_target_resource_v1(error));
            }
            Ok(payload)
        }
        (result, prior) => {
            let error = match (result, prior) {
                (Ok(payload), Err(error)) => {
                    drop(payload);
                    private_bf16_target_resource_v1(error)
                }
                (Err(error), Ok(())) => error,
                (Err(_), Err(error)) => private_bf16_target_resource_v1(error),
                (Ok(_), Ok(())) => unreachable!("success handled above"),
            };
            // Every new box/callback has already dropped before this refund.
            budget
                .release_storage(prepaid)
                .map_err(private_bf16_target_resource_v1)?;
            Err(error)
        }
    }
}

fn compare_private_bf16_descriptor_payload_v1(
    retained: &PrivateBf16DescriptorPayloadV1,
    fresh: &PrivateBf16DescriptorPayloadV1,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let work = retained
        .canonical_descriptor
        .len()
        .checked_add(fresh.canonical_descriptor.len())
        .and_then(|n| n.checked_add(retained.final_llvm.len()))
        .and_then(|n| n.checked_add(fresh.final_llvm.len()))
        .and_then(|n| n.checked_add(3))
        .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
    budget
        .charge_work(work)
        .map_err(private_bf16_target_resource_v1)?;
    if retained.retained != fresh.retained
        || retained.canonical_descriptor != fresh.canonical_descriptor
        || retained.final_llvm != fresh.final_llvm
    {
        return Err(private_bf16_descriptor_context_v1(
            "private BF16 descriptor/final LLVM bytes differ from owned replay",
        ));
    }
    Ok(())
}

impl PrivateBf16LlvmV1 {
    #[allow(dead_code)]
    pub(crate) fn prepare_private_bf16_descriptor_v1(
        mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<PrivateBf16DescriptorV1, PrivateBf16LlvmErrorV1> {
        self.revalidate_private_bf16_llvm_v1(requested_return, typed_roots, profile)?;
        let owner = &self.optimized.target.formal.owner;
        let output = self.optimized.optimization.checked.owner();
        let worker = &self.llvm.worker;
        let [root] = self.optimized.target.formal.verification.roots.as_ref() else {
            return Err(private_bf16_descriptor_context_v1(
                "private BF16 descriptor root custody",
            ));
        };
        let semantic_root = root.semantic_root;
        let result = self
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                Ok(derive_private_bf16_descriptor_v1(
                    owner,
                    output,
                    semantic_root,
                    requested_return,
                    worker,
                    typed_roots,
                    budget,
                ))
            });
        self.optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        let descriptor = result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)??;
        Ok(PrivateBf16DescriptorV1 {
            descriptor,
            llvm: self,
        })
    }
}

impl PrivateBf16DescriptorV1 {
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_descriptor_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        self.llvm
            .revalidate_private_bf16_llvm_v1(requested_return, typed_roots, profile)?;
        let owner = &self.llvm.optimized.target.formal.owner;
        let output = self.llvm.optimized.optimization.checked.owner();
        let worker = &self.llvm.llvm.worker;
        let retained = &self.descriptor;
        let [root] = self
            .llvm
            .optimized
            .target
            .formal
            .verification
            .roots
            .as_ref()
        else {
            return Err(private_bf16_descriptor_context_v1(
                "private BF16 descriptor replay roots",
            ));
        };
        let semantic_root = root.semantic_root;
        let result = self
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                let replay_floor = budget.storage();
                let replay_ledger = budget.work_ledger_identity_v1();
                let replay_slot = budget as *const Budget<'_> as usize;
                let fresh = match derive_private_bf16_descriptor_v1(
                    owner,
                    output,
                    semantic_root,
                    requested_return,
                    worker,
                    typed_roots,
                    budget,
                ) {
                    Ok(fresh) => fresh,
                    Err(error) => return Ok(Err(error)),
                };
                let joined = compare_private_bf16_descriptor_payload_v1(retained, &fresh, budget);
                let storage = fresh.retained;
                drop(fresh);
                budget.release_storage(storage).map_err(resource)?;
                budget.check_prior_denials_v1().map_err(resource)?;
                if budget.storage() != replay_floor
                    || budget.work_ledger_identity_v1() != replay_ledger
                    || budget as *const Budget<'_> as usize != replay_slot
                {
                    return Err(resource(Resource::Accounting));
                }
                Ok(joined)
            });
        self.llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod private_bf16_descriptor_byte_controls {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    // Pure byte-comparison controls. These deliberately construct no source,
    // LLVM, descriptor or compilation owner and establish no source admission.
    fn bytes() -> PrivateBf16DescriptorPayloadV1 {
        let canonical_descriptor = vec![0x31, 0x32, 0x33].into_boxed_slice();
        let final_llvm = "owned llvm plus descriptor".to_owned().into_boxed_str();
        let retained = std::mem::size_of::<PrivateBf16DescriptorV1>()
            + canonical_descriptor.len()
            + final_llvm.len();
        PrivateBf16DescriptorPayloadV1 {
            canonical_descriptor,
            final_llvm,
            retained,
        }
    }

    #[test]
    fn private_descriptor_comparison_requires_both_complete_bytes_and_storage() {
        for mutation in 0..4 {
            let a = bytes();
            let mut b = bytes();
            match mutation {
                1 => b.canonical_descriptor[1] ^= 1,
                2 => b.final_llvm = "owned llvm plus descriptoR".to_owned().into_boxed_str(),
                3 => b.retained += 1,
                _ => {}
            }
            let mut work = Work::new(10_000);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            let before = budget.storage();
            let result = compare_private_bf16_descriptor_payload_v1(&a, &b, &mut budget);
            assert_eq!(result.is_ok(), mutation == 0);
            if mutation != 0 {
                assert!(
                    matches!(result, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::
                        ProductionDescriptorMismatch(
                            "private BF16 descriptor/final LLVM bytes differ from owned replay",
                        ),
                )))
                );
            }
            assert_eq!(budget.storage(), before);
        }
    }

    #[test]
    fn private_descriptor_comparison_exact_work_and_sticky_one_short() {
        let a = bytes();
        let b = bytes();
        let exact = a.canonical_descriptor.len()
            + b.canonical_descriptor.len()
            + a.final_llvm.len()
            + b.final_llvm.len()
            + 3;
        for cap in [exact, exact - 1] {
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            let result = compare_private_bf16_descriptor_payload_v1(&a, &b, &mut budget);
            assert_eq!(result.is_ok(), cap == exact);
            assert_eq!(budget.storage(), 7);
            if cap != exact {
                let before = (budget.work(), budget.failed_work(), budget.failed_storage());
                assert!(compare_private_bf16_descriptor_payload_v1(&a, &b, &mut budget).is_err());
                assert_eq!(
                    (budget.work(), budget.failed_work(), budget.failed_storage()),
                    before
                );
            }
        }
    }

    #[test]
    fn private_descriptor_prior_storage_denial_precedes_comparison_work() {
        let a = bytes();
        let b = bytes();
        let mut work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert!(budget.reserve_storage(1).is_err());
        let before = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        for _ in 0..2 {
            assert!(compare_private_bf16_descriptor_payload_v1(&a, &b, &mut budget).is_err());
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
        }
    }
}

fn reserve_private_bf16_descriptor_output_v1(
    budget: &mut Budget<'_>,
) -> Result<usize, PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(3)
        .map_err(private_bf16_target_resource_v1)?;
    let amount = private_bf16_descriptor_caps_v1()
        .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
        .checked_add(std::mem::size_of::<PrivateBf16DescriptorV1>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Option<PrivateBf16DescriptorResultV1>>()))
        .ok_or_else(|| private_bf16_target_resource_v1(Resource::Arithmetic))?;
    budget
        .reserve_storage(amount)
        .map_err(private_bf16_target_resource_v1)?;
    Ok(amount)
}

#[cfg(test)]
mod private_descriptor_constructor_reservation_controls {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn constructor_prepayment_exact_storage_and_one_short_do_not_run_engines() {
        let exact = private_bf16_descriptor_caps_v1().unwrap()
            + std::mem::size_of::<PrivateBf16DescriptorV1>()
            + std::mem::size_of::<Option<PrivateBf16DescriptorResultV1>>();
        for available in [exact, exact - 1] {
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 7 + available);
            budget.reserve_storage(7).unwrap();
            let result = reserve_private_bf16_descriptor_output_v1(&mut budget);
            if available == exact {
                assert_eq!(result.unwrap(), exact);
                assert_eq!(budget.storage(), 7 + exact);
                budget.release_storage(exact).unwrap();
                assert_eq!(budget.storage(), 7);
            } else {
                assert!(result.is_err());
                assert_eq!(budget.storage(), 7);
                let before = (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                );
                assert!(reserve_private_bf16_descriptor_output_v1(&mut budget).is_err());
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
        }
    }

    #[test]
    fn constructor_prepayment_exact_work_and_one_short_preserve_entry_storage() {
        let storage = private_bf16_descriptor_caps_v1().unwrap()
            + std::mem::size_of::<PrivateBf16DescriptorV1>()
            + std::mem::size_of::<Option<PrivateBf16DescriptorResultV1>>();
        for cap in [3, 2] {
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, 7 + storage);
            budget.reserve_storage(7).unwrap();
            let result = reserve_private_bf16_descriptor_output_v1(&mut budget);
            assert_eq!(result.is_ok(), cap == 3);
            if let Ok(reserved) = result {
                budget.release_storage(reserved).unwrap();
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn constructor_prepayment_prior_denial_precedes_any_new_charge() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert!(budget.reserve_storage(1).is_err());
        let before = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        for _ in 0..2 {
            assert!(reserve_private_bf16_descriptor_output_v1(&mut budget).is_err());
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
        }
    }
}

#[cfg(test)]
mod private_owning_descriptor_observer_v1 {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    use sha2::{Digest, Sha256};

    fn require_byte_refusal(
        result: Result<(), PrivateBf16LlvmErrorV1>,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        match result {
            Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                    "private BF16 descriptor/final LLVM bytes differ from owned replay",
                ),
            )) => Ok(()),
            Err(error) => Err(error),
            Ok(()) => Err(private_bf16_descriptor_context_v1(
                "mutated private BF16 descriptor owner unexpectedly replayed",
            )),
        }
    }

    impl PrivateBf16DescriptorV1 {
        /// Genuine-owner controls: mutate only the already retained boxed bytes,
        /// run the complete owning replay, restore bytes before inspecting any
        /// result (including unwind), then demand a clean complete replay.
        /// There is no replacement owner, target graph, account or proof token.
        pub(crate) fn exercise_private_bf16_descriptor_bytes_for_test_v1(
            &mut self,
            requested: [u8; 4],
            typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            if self.descriptor.canonical_descriptor.is_empty() {
                return Err(private_bf16_descriptor_context_v1(
                    "empty owned descriptor control",
                ));
            }
            let last = self.descriptor.canonical_descriptor.len() - 1;
            self.descriptor.canonical_descriptor[last] ^= 1;
            let changed_descriptor = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.revalidate_private_bf16_descriptor_v1(requested, typed_roots, profile)
            }));
            self.descriptor.canonical_descriptor[last] ^= 1;
            let changed_descriptor = match changed_descriptor {
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    return Err(private_bf16_descriptor_context_v1(
                        "owned descriptor mutation replay panicked after bytes were restored",
                    ));
                }
            };
            require_byte_refusal(changed_descriptor)?;

            // Mutating this ASCII substring in-place preserves allocation and
            // UTF-8, while changing actual retained final LLVM target text.
            let start = self.descriptor.final_llvm.find("gfx942").ok_or_else(|| {
                private_bf16_descriptor_context_v1("owned final LLVM target text absent")
            })?;
            let end = start + "gfx942".len();
            self.descriptor
                .final_llvm
                .get_mut(start..end)
                .expect("matched ASCII target")
                .make_ascii_uppercase();
            let changed_text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.revalidate_private_bf16_descriptor_v1(requested, typed_roots, profile)
            }));
            self.descriptor
                .final_llvm
                .get_mut(start..end)
                .expect("same-width ASCII target")
                .make_ascii_lowercase();
            let changed_text = match changed_text {
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    return Err(private_bf16_descriptor_context_v1(
                        "owned final LLVM mutation replay panicked after bytes were restored",
                    ));
                }
            };
            require_byte_refusal(changed_text)?;
            self.revalidate_private_bf16_descriptor_v1(requested, typed_roots, profile)
        }
    }

    // Non-Copy row remains prepaid until consumed; no detached report grants
    // source, descriptor, Worker, artifact, runtime-allocation or launch authority.
    struct Observation {
        ledger: Ledger,
        protected: usize,
        initial_work: usize,
        output_sha: [u8; 32],
        output_bytes: u64,
        descriptor_sha: [u8; 32],
        final_sha: [u8; 32],
        pre_worker_sha: [u8; 32],
        descriptor_bytes: usize,
        final_bytes: usize,
        pre_worker_bytes: usize,
        retained_descriptor: usize,
        retained_llvm: usize,
        work: usize,
        storage: usize,
        peak: usize,
    }
    struct Hex<'a>(&'a [u8; 32]);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    impl Observation {
        fn emit(self, requested: [u8; 4], scratch: usize) {
            eprintln!(
                "fe2o3-bf16-private-owning-descriptor-collected-v1 requested={},{},{},{} output_sha256={} output_bytes={} descriptor_sha256={} descriptor_bytes={} final_llvm_sha256={} final_llvm_bytes={} pre_descriptor_worker_sha256={} pre_descriptor_worker_bytes={} retained_descriptor_storage={} retained_llvm_storage={} observer_storage={} work={} storage={} peak={} same_ledger=true actual_output_owner=true full_descriptor_replayed=true full_text_replayed=true descriptor_bytes_mutation_refused=true descriptor_text_mutation_refused=true replay_storage_restored=true runtime_bounds_alias_duties_preserved=true cleanup_pending=true llvm_emitted=true descriptor_constructed=true worker_invoked=false normal_admission=false launch_authenticated=false artifact_authority=false descriptor_authority=false",
                requested[0],
                requested[1],
                requested[2],
                requested[3],
                Hex(&self.output_sha),
                self.output_bytes,
                Hex(&self.descriptor_sha),
                self.descriptor_bytes,
                Hex(&self.final_sha),
                self.final_bytes,
                Hex(&self.pre_worker_sha),
                self.pre_worker_bytes,
                self.retained_descriptor,
                self.retained_llvm,
                scratch,
                self.work,
                self.storage,
                self.peak,
            );
        }
    }

    fn inspect_bytes(
        descriptor: &PrivateBf16DescriptorPayloadV1,
        llvm: &PrivateBf16LlvmPayloadV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        budget.check_prior_denials_v1().map_err(resource)?;
        let bytes = descriptor
            .canonical_descriptor
            .len()
            .checked_add(descriptor.final_llvm.len())
            .and_then(|n| n.checked_add(llvm.worker.len()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget
            .charge_work(
                bytes
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )
            .map_err(resource)?;
        let retained = std::mem::size_of::<PrivateBf16DescriptorV1>()
            .checked_add(descriptor.canonical_descriptor.len())
            .and_then(|n| n.checked_add(descriptor.final_llvm.len()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if llvm.worker.is_empty()
            || descriptor.canonical_descriptor.is_empty()
            || descriptor.canonical_descriptor.len()
                > fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES
            || descriptor.final_llvm.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
            || descriptor.final_llvm.len() <= llvm.worker.len()
            || descriptor.retained != retained
            || !descriptor.final_llvm.starts_with(llvm.worker.as_str())
            || descriptor
                .final_llvm
                .matches(".section .fe2o3.kd.v1")
                .count()
                != 1
        {
            return Err(E::RosterMetadata(
                "actual retained descriptor byte shape differs",
            ));
        }
        Ok(())
    }

    impl PrivateBf16DescriptorV1 {
        /// The private genuine endpoint calls this only after opposite Return,
        /// two actual-owner byte mutation refusals, and clean full replay.
        pub(crate) fn observe_private_bf16_descriptor_for_test_v1(
            &mut self,
            requested: [u8; 4],
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            let output = self.llvm.optimized.optimization.checked.owner();
            let llvm = &self.llvm.llvm;
            let descriptor = &self.descriptor;
            let result = self
                .llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .with_budget(|budget| {
                    budget.check_prior_denials_v1().map_err(resource)?;
                    budget.charge_work(4).map_err(resource)?;
                    if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3]) {
                        return Err(E::RosterMetadata(
                            "unsupported actual descriptor Return request",
                        ));
                    }
                    let scratch = std::mem::size_of::<Observation>();
                    let floor = budget.storage();
                    budget.reserve_storage(scratch).map_err(resource)?;
                    let mut row = Observation {
                        ledger: budget.work_ledger_identity_v1(),
                        protected: sum(floor, scratch)?,
                        initial_work: budget.work(),
                        output_sha: *output.canonical().identity().digest(),
                        output_bytes: output.canonical().identity().canonical_length(),
                        descriptor_sha: [0; 32],
                        final_sha: [0; 32],
                        pre_worker_sha: [0; 32],
                        descriptor_bytes: descriptor.canonical_descriptor.len(),
                        final_bytes: descriptor.final_llvm.len(),
                        pre_worker_bytes: llvm.worker.len(),
                        retained_descriptor: descriptor.retained,
                        retained_llvm: llvm.retained,
                        work: 0,
                        storage: 0,
                        peak: 0,
                    };
                    let inspected = (|| {
                        inspect_bytes(descriptor, llvm, budget)?;
                        row.descriptor_sha =
                            Sha256::digest(descriptor.canonical_descriptor.as_ref()).into();
                        row.final_sha = Sha256::digest(descriptor.final_llvm.as_bytes()).into();
                        row.pre_worker_sha = Sha256::digest(llvm.worker.as_bytes()).into();
                        budget.check_prior_denials_v1().map_err(resource)?;
                        if budget.work_ledger_identity_v1() != row.ledger
                            || budget.storage() != row.protected
                            || budget.work() <= row.initial_work
                            || budget.peak_storage() < budget.storage()
                        {
                            return Err(resource(Resource::Accounting));
                        }
                        row.work = budget.work();
                        row.storage = budget.storage();
                        row.peak = budget.peak_storage();
                        row.emit(requested, scratch);
                        Ok(())
                    })();
                    budget.release_storage(scratch).map_err(resource)?;
                    budget.check_prior_denials_v1().map_err(resource)?;
                    inspected
                });
            self.llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)
        }
    }

    #[test]
    fn descriptor_refusal_control_rejects_success_and_unrelated_errors() {
        assert!(
            require_byte_refusal(Err(private_bf16_descriptor_context_v1(
                "private BF16 descriptor/final LLVM bytes differ from owned replay",
            )))
            .is_ok()
        );
        assert!(require_byte_refusal(Ok(())).is_err());
        assert!(matches!(
            require_byte_refusal(Err(private_bf16_target_resource_v1(Resource::Accounting))),
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Accounting)
            ))
        ));
        assert!(require_byte_refusal(Err(private_bf16_descriptor_context_v1("other"))).is_err());
    }

    #[test]
    fn descriptor_observer_meter_exact_and_one_short_preserve_original_floor() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        // Inert malformed shape, deliberately no real descriptor/LLVM owner.
        let descriptor = PrivateBf16DescriptorPayloadV1 {
            canonical_descriptor: vec![1u8, 2, 3].into_boxed_slice(),
            final_llvm: "not LLVM".to_owned().into_boxed_str(),
            retained: std::mem::size_of::<PrivateBf16DescriptorV1>() + 3 + 8,
        };
        let llvm = PrivateBf16LlvmPayloadV1 {
            worker: "worker".into(),
            dialect: "dialect".into(),
            retained: 0,
        };
        let exact = 8 * (3 + 8 + 6) + 128;
        for cap in [exact, exact - 1] {
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = inspect_bytes(&descriptor, &llvm, &mut budget);
            if cap == exact {
                assert!(matches!(
                    result,
                    Err(E::RosterMetadata(
                        "actual retained descriptor byte shape differs",
                    ))
                ));
            } else {
                let prior = budget.check_prior_denials_v1().unwrap_err();
                assert!(matches!(result, Err(E::ConditionalResource(error)) if error == prior));
                let before = (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                );
                assert!(inspect_bytes(&descriptor, &llvm, &mut budget).is_err());
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.failed_work(),
                        budget.failed_storage()
                    ),
                    before
                );
            }
            assert_eq!(budget.storage(), 7);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}

#[cfg(test)]
mod private_descriptor_constructor_failure_controls {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::cell::Cell;

    // Test-harness state only, never source evidence or a replacement account.
    // No hooks exist in non-test builds; no public selector enables them.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct State {
        entered: usize,
        made: usize,
        dropped: usize,
        panic_after_capture: bool,
        panics: usize,
    }
    std::thread_local! {
        static STATE: Cell<Option<State>> = const { Cell::new(None) };
    }
    struct Scope;
    impl Scope {
        fn enter(panic_after_capture: bool) -> Self {
            STATE.with(|state| {
                assert!(state.get().is_none(), "no nested descriptor fault scope");
                state.set(Some(State {
                    panic_after_capture,
                    ..State::default()
                }));
            });
            Self
        }
        fn snapshot(&self) -> State {
            STATE.with(|state| state.get().unwrap())
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            STATE.with(|state| state.set(None));
        }
    }
    pub(super) fn constructor_entered() {
        STATE.with(|cell| {
            if let Some(mut state) = cell.get() {
                state.entered += 1;
                cell.set(Some(state));
            }
        });
    }
    pub(super) fn payload_made() {
        STATE.with(|cell| {
            if let Some(mut state) = cell.get() {
                state.made += 1;
                cell.set(Some(state));
            }
        });
    }
    pub(super) fn after_capture() {
        let panic_now = STATE.with(|cell| {
            let Some(mut state) = cell.get() else {
                return false;
            };
            if !state.panic_after_capture {
                return false;
            }
            state.panic_after_capture = false;
            state.panics += 1;
            cell.set(Some(state));
            true
        });
        if panic_now {
            // Bypass rustc's global panic hook; exercise only unwind/catch/drop.
            // This boxed inert test payload is explicitly harness-excluded.
            std::panic::resume_unwind(Box::new("descriptor control after closed loans"));
        }
    }
    impl Drop for PrivateBf16DescriptorPayloadV1 {
        fn drop(&mut self) {
            STATE.with(|cell| {
                if let Some(mut state) = cell.get() {
                    state.dropped += 1;
                    cell.set(Some(state));
                }
            });
        }
    }
    // Rust completes both boxed-field destructors before an explicit drop(...)
    // returns. The counter records the payload Drop; the tested caller still
    // must sequence that complete drop before any reservation refund.
    fn inert_payload() -> PrivateBf16DescriptorPayloadV1 {
        payload_made();
        PrivateBf16DescriptorPayloadV1 {
            canonical_descriptor: vec![1, 2, 3].into_boxed_slice(),
            final_llvm: "not LLVM".to_owned().into_boxed_str(),
            retained: std::mem::size_of::<PrivateBf16DescriptorV1>() + 3 + 8,
        }
    }
    fn is_accounting(result: &PrivateBf16DescriptorResultV1) -> bool {
        matches!(
            result,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Accounting)
            ))
        )
    }

    #[test]
    fn descriptor_result_reconciliation_retains_live_bytes_until_explicit_drop() {
        let scope = Scope::enter(false);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = &budget as *const Budget<'_> as usize;
        let prepaid = reserve_private_bf16_descriptor_output_v1(&mut budget).unwrap();
        let payload = finish_private_bf16_descriptor_attempt_v1(
            Ok(inert_payload()),
            7,
            ledger,
            slot,
            prepaid,
            &mut budget,
        )
        .unwrap();
        let retained = payload.retained;
        assert_eq!(budget.storage(), 7 + retained);
        assert_eq!((scope.snapshot().made, scope.snapshot().dropped), (1, 0));
        drop(payload);
        assert_eq!(scope.snapshot().dropped, 1);
        assert_eq!(budget.storage(), 7 + retained);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn descriptor_result_typed_refusal_and_zero_prepayment_restore_only_known_credit() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        for charged in [false, true] {
            let ledger = budget.work_ledger_identity_v1();
            let slot = &budget as *const Budget<'_> as usize;
            let prepaid = if charged {
                reserve_private_bf16_descriptor_output_v1(&mut budget).unwrap()
            } else {
                0
            };
            let result = finish_private_bf16_descriptor_attempt_v1(
                Err(private_bf16_descriptor_context_v1(
                    "inert constructor refusal",
                )),
                7,
                ledger,
                slot,
                prepaid,
                &mut budget,
            );
            assert!(
                matches!(result, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                    "inert constructor refusal"))))
            );
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn descriptor_result_original_denial_drops_payload_then_refunds_known_prepayment() {
        for kind in 0..3 {
            let scope = Scope::enter(false);
            let mut work = Work::new(3);
            let storage = private_bf16_descriptor_caps_v1().unwrap()
                + std::mem::size_of::<PrivateBf16DescriptorV1>()
                + std::mem::size_of::<Option<PrivateBf16DescriptorResultV1>>();
            let mut budget = Budget::new(&mut work, 7 + storage);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let slot = &budget as *const Budget<'_> as usize;
            let prepaid = reserve_private_bf16_descriptor_output_v1(&mut budget).unwrap();
            let payload = inert_payload();
            if kind != 1 {
                assert!(budget.charge_work(1).is_err());
            }
            if kind != 0 {
                assert!(budget.reserve_storage(1).is_err());
            }
            let prior = budget.check_prior_denials_v1().unwrap_err();
            let before = (
                budget.work(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            let result = finish_private_bf16_descriptor_attempt_v1(
                Ok(payload),
                7,
                ledger,
                slot,
                prepaid,
                &mut budget,
            );
            assert!(
                matches!(result, Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(error))) if error == prior)
            );
            assert_eq!(scope.snapshot().dropped, 1);
            assert_eq!(budget.storage(), 7);
            assert_eq!(
                (
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                before
            );
            // Sticky original refusal remains; never clear it to resume a phase.
            assert_eq!(budget.check_prior_denials_v1().unwrap_err(), prior);
        }
    }

    #[test]
    fn descriptor_result_foreign_ledger_slot_or_surplus_never_guesses_a_refund() {
        for fault in 0..3 {
            let scope = Scope::enter(false);
            let mut work = Work::new(100);
            let mut foreign_work = Work::new(100);
            let foreign = Budget::new(&mut foreign_work, 64 * 1024 * 1024);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(7).unwrap();
            let ledger = if fault == 0 {
                foreign.work_ledger_identity_v1()
            } else {
                budget.work_ledger_identity_v1()
            };
            let slot = &budget as *const Budget<'_> as usize + usize::from(fault == 1);
            let prepaid = reserve_private_bf16_descriptor_output_v1(&mut budget).unwrap();
            if fault == 2 {
                budget.reserve_storage(1).unwrap();
            }
            let before = budget.storage();
            let result = finish_private_bf16_descriptor_attempt_v1(
                Ok(inert_payload()),
                7,
                ledger,
                slot,
                prepaid,
                &mut budget,
            );
            assert!(is_accounting(&result));
            assert_eq!(scope.snapshot().dropped, 1);
            assert_eq!(budget.storage(), before);
            assert!(budget.check_prior_denials_v1().is_ok());
            // Isolated fixture teardown knows its own credit. Production did
            // not infer or release any source/callback/foreign reservation.
            budget.release_storage(before - 7).unwrap();
        }
    }

    #[test]
    fn descriptor_result_overstated_retention_drops_before_exact_credit_release() {
        let scope = Scope::enter(false);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = &budget as *const Budget<'_> as usize;
        let prepaid = reserve_private_bf16_descriptor_output_v1(&mut budget).unwrap();
        let mut payload = inert_payload();
        payload.retained = prepaid + 1;
        let result = finish_private_bf16_descriptor_attempt_v1(
            Ok(payload),
            7,
            ledger,
            slot,
            prepaid,
            &mut budget,
        );
        assert!(is_accounting(&result));
        assert_eq!(scope.snapshot().dropped, 1);
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn descriptor_fault_scope_restores_on_unwind() {
        let run = std::panic::catch_unwind(|| {
            let _scope = Scope::enter(true);
            let payload = inert_payload();
            after_capture();
            drop(payload);
        });
        assert!(run.is_err());
        let scope = Scope::enter(false);
        assert_eq!(scope.snapshot(), State::default());
    }

    struct Capture<'a>(&'a Cell<bool>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    impl PrivateBf16LlvmV1 {
        /// Genuine-source controls on the intact owner and original account.
        /// This is called only by the ignored private descriptor endpoint.
        /// Test counters/panic machinery remain excluded harness state, not
        /// production source, allocator, engine or whole-stack accounting.
        pub(crate) fn exercise_private_bf16_descriptor_constructor_for_test_v1(
            &mut self,
            requested: [u8; 4],
            typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.revalidate_private_bf16_llvm_v1(requested, typed_roots, profile)?;
            let owner = &self.optimized.target.formal.owner;
            let output = self.optimized.optimization.checked.owner();
            let worker = &self.llvm.worker;
            let [root] = self.optimized.target.formal.verification.roots.as_ref() else {
                return Err(private_bf16_descriptor_context_v1(
                    "constructor control root custody",
                ));
            };
            let semantic_root = root.semantic_root;
            let wrong = if requested == [0, 1, 2, 3] {
                [1, 0, 2, 3]
            } else {
                [0, 1, 2, 3]
            };
            let result = self.optimized.target.formal.verification.phase.with_budget(|budget| {
                let run = (|| -> Result<(), PrivateBf16LlvmErrorV1> {
                    let floor = budget.storage();
                    let ledger = budget.work_ledger_identity_v1();
                    let slot = budget as *const Budget<'_> as usize;
                    // An unused captured callback must drop even if source/
                    // Return replay refuses before lending anything.
                    let captured_dropped = Cell::new(false);
                    let called = Cell::new(false);
                    let captured = Capture(&captured_dropped);
                    let called_ref = &called;
                    let refused = owner.with_private_bf16_descriptor_source_v1(
                        semantic_root, wrong, budget,
                        move |_, _, _, _, _, _| {
                            let _held = captured;
                            called_ref.set(true);
                            Ok(())
                        },
                    );
                    assert!(matches!(refused, Err(
                        fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch))));
                    assert!(captured_dropped.get() && !called.get());
                    assert_eq!(budget.storage(), floor);
                    // Direct source-loan callback unwind occurs after replay,
                    // with no nested guard frame to pretend has been refunded.
                    let captured_dropped = Cell::new(false);
                    let called = Cell::new(false);
                    let called_ref = &called;
                    let captured = Capture(&captured_dropped);
                    let refused = owner.with_private_bf16_descriptor_source_v1(
                        semantic_root, requested, budget,
                        move |_, _, _, _, _, _| {
                            let _held = captured;
                            called_ref.set(true);
                            std::panic::resume_unwind(Box::new("descriptor source callback control"));
                        },
                    );
                    assert!(matches!(refused, Err(
                        fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::MirPlironTranslation(_)))));
                    assert!(captured_dropped.get() && called.get());
                    assert_eq!(budget.storage(), floor);
                    budget.check_prior_denials_v1().map_err(private_bf16_target_resource_v1)?;
                    for fault in 0..4 {
                        let scope = Scope::enter(fault == 2);
                        let result = derive_private_bf16_descriptor_v1(
                            owner, output, semantic_root,
                            if fault == 0 { wrong } else { requested },
                            worker, if fault == 1 { &[] } else { typed_roots }, budget,
                        );
                        match (fault, result) {
                            (0, Err(PrivateBf16LlvmErrorV1::FormalMemoryAdmission(
                                fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch)))) => {
                                assert_eq!((scope.snapshot().entered, scope.snapshot().made, scope.snapshot().dropped), (0, 0, 0));
                            }
                            (1, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                                    "private BF16 complete singleton descriptor roster")))) => {
                                assert_eq!((scope.snapshot().entered, scope.snapshot().made, scope.snapshot().dropped), (1, 0, 0));
                            }
                            (2, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                                    "private BF16 descriptor construction panicked")))) => {
                                assert_eq!((scope.snapshot().entered, scope.snapshot().made, scope.snapshot().dropped, scope.snapshot().panics), (1, 1, 1, 1));
                            }
                            (3, Ok(payload)) => {
                                assert_eq!((scope.snapshot().entered, scope.snapshot().made, scope.snapshot().dropped), (1, 1, 0));
                                let retained = payload.retained;
                                assert_eq!(budget.storage(), floor + retained);
                                drop(payload);
                                assert_eq!(scope.snapshot().dropped, 1);
                                assert_eq!(budget.storage(), floor + retained);
                                budget.release_storage(retained).map_err(private_bf16_target_resource_v1)?;
                            }
                            (_, Err(error)) => return Err(error),
                            (_, Ok(payload)) => {
                                drop(payload);
                                return Err(private_bf16_descriptor_context_v1(
                                    "constructor control unexpectedly returned payload"));
                            }
                        }
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        assert_eq!(budget as *const Budget<'_> as usize, slot);
                        budget.check_prior_denials_v1().map_err(private_bf16_target_resource_v1)?;
                    }
                    Ok(())
                })();
                Ok(run)
            });
            self.optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
        }
    }
}

#[path = "bf16_private_worker_handoff_v1.rs"]
mod private_worker_handoff_v1;
pub(crate) use private_worker_handoff_v1::PrivateBf16WorkerHandoffV1;
