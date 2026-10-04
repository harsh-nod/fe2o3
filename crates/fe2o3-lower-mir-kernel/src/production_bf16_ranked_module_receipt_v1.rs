//! Closed nominal structural receipt construction; not ranked attachment.
use super::*;

fn require_private_bf16_module_source_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    roots: &[ProductionRankedSemanticProjectionRootV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    if materialized.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::Bf16Nominal {
        return Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "private nominal module receipt",
            },
        );
    }
    budget.charge_work(3)?;
    if roots.len() != 1 || materialized.source_launch().roots().len() != 1 {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    if budget.storage() < materialized.unit_local_source_storage_floor_v1()? {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(3)?;
    let emission = materialized
        .bf16_call_instance_emission_v1()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !std::ptr::eq(emission.owner(), materialized)
        || emission.root() != roots[0].selected_root()
        || emission.root() != materialized.source_launch().roots()[0].selected_root()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Moves the exact private nominal source and already-boxed singleton roots.
    ///
    /// The caller retains the original materialization account and the original
    /// projection account, including the source floor and all incoming root/map
    /// payload reservations, until the returned receipt is dropped. This entry
    /// allocates no new root container and never replaces or resets that ledger.
    /// Source replay and legacy ranked/source checks keep their existing limits;
    /// only the new fixed nominal entry/join work is charged here.
    ///
    /// This is structural custody, not translation validation or attachment.
    /// Existing RawEmpty constructors and attachment guards remain unchanged
    /// and refuse this nominal receipt. No output admission is authorized here.
    #[doc(hidden)]
    pub fn from_private_bf16_projection_roster_with_budget_v1(
        materialized: ProductionPreRankedKirOwnerV1,
        roots: Box<[ProductionRankedSemanticProjectionRootV1]>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const _ as usize;
        require_private_bf16_module_source_v1(&materialized, &roots, budget)?;
        // Do not substitute the earlier projection's observation or report.
        // These are the same complete structural checks used by the old receipt.
        let validation = validate_source_ranked_roster_v1(
            &materialized.semantic_ssa,
            &materialized.source_launch,
            &roots,
        );
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() != floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.check_prior_denials_v1()?;
        validation?;
        Ok(Self {
            materialized,
            roots,
        })
    }
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Rechecks this intact private nominal structural receipt on its caller's
    /// original projection account. Expected source/root/Return values are only
    /// untrusted comparisons: no source owner, token or validation report escapes.
    /// This does not perform full translation validation or attach ranked checks.
    #[doc(hidden)]
    pub fn verify_private_bf16_module_roster_with_budget_v1(
        &self,
        expected_semantic_sha256: &[u8; 32],
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const _ as usize;
        require_private_bf16_module_source_v1(&self.materialized, &self.roots, budget)?;
        // Fixed SHA, Return and selected-root comparisons, prepaid before use.
        budget.charge_work(32 + 4 + 2)?;
        let emission = self
            .materialized
            .bf16_call_instance_emission_v1()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !matches!(expected_return, [0, 1, 2, 3] | [1, 0, 2, 3])
            || self
                .materialized
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
                != expected_semantic_sha256
            || self.roots[0].selected_root() != expected_root
            || emission.return_permutation() != expected_return
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let validation = validate_source_ranked_roster_v1(
            &self.materialized.semantic_ssa,
            &self.materialized.source_launch,
            &self.roots,
        );
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() != floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.check_prior_denials_v1()?;
        validation
    }
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Revalidates the actual retained private nominal source against the actual
    /// authenticated root and requested Return, without exporting or requiring a
    /// detached source snapshot. Source identity is retained by the move-only
    /// receipt; the internal SHA self-comparison is not a substitution proof.
    /// All existing structural/source checks and original-account postflights
    /// remain mandatory. This grants no attachment or ordinary admission.
    #[doc(hidden)]
    pub fn verify_private_bf16_module_retained_source_with_budget_v1(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.verify_private_bf16_module_roster_with_budget_v1(
            self.materialized
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            expected_root,
            expected_return,
            budget,
        )
    }
}

/// Private nominal source, actual ranked roots and a freshly checked full
/// translation report retained together. The intact pre-ranked owner is kept
/// for later nominal replay; no legacy Connected owner is reconstructed here.
///
/// This type grants neither ordinary admission nor target/launch authority.
/// Its caller retains the original materialization account and the same
/// projection account until this value is dropped.
#[doc(hidden)]
#[must_use = "dropping the private attachment abandons source and checked roots"]
pub struct ProductionPrivateBf16AttachedRankedOwnerV1 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    validation: ProductionMirPlironTranslationValidationV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

fn reserve_private_bf16_attachment_header_v1(
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    let header = std::mem::size_of::<ProductionPrivateBf16AttachedRankedOwnerV1>();
    budget.reserve_storage(header)?;
    Ok(header)
}

fn private_bf16_attachment_panic_error_v1(
    budget: &ArgumentBudgetV1<'_>,
) -> ProductionSemanticKirErrorV1 {
    // A panic is never allowed to replace an earlier original resource denial.
    // This gate adds no new debit and preserves existing Work-first precedence.
    match budget.check_prior_denials_v1() {
        Err(error) => error.into(),
        Ok(()) => ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::KernelShape,
        ),
    }
}

fn check_private_bf16_attachment_ledger_v1(
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    if budget.work_ledger_identity_v1() != ledger {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

fn check_private_bf16_attached_report_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    validation: &ProductionMirPlironTranslationValidationV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(32 + 4)?;
    if validation.semantic_sha256()
        != materialized
            .semantic_ssa
            .source_semantic()
            .semantic_sha256()
            .as_bytes()
        || validation.tensor_operations() != 1
        || validation.claims_indexed_address_equivalence()
        || validation.claims_complete_operational_equivalence()
        || validation.reconciled_projection_remains_trusted()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Retains a NEW full nominal translation report with this intact source.
    /// Old RawEmpty attachment and ordinary dispatch are deliberately unchanged.
    /// The fixed returned header is newly prepaid; all incoming source, root and
    /// map reservations remain caller-owned. No source/header credit is guessed
    /// or released during these ownership moves.
    #[doc(hidden)]
    pub fn into_private_bf16_attached_ranked_owner_with_budget_v1(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionPrivateBf16AttachedRankedOwnerV1, ProductionSemanticKirErrorV1> {
        require_private_bf16_module_source_v1(&self.materialized, &self.roots, budget)?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const _ as usize;
        let protected = floor
            .checked_add(std::mem::size_of::<
                ProductionPrivateBf16AttachedRankedOwnerV1,
            >())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let header = reserve_private_bf16_attachment_header_v1(budget)?;
        // Borrow the intact source for the entire fresh validation. Neither the
        // owner nor its sealed emission is reconstructed from a report or token.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validate_source_ranked_roster_v1(
                &self.materialized.semantic_ssa,
                &self.materialized.source_launch,
                &self.roots,
            )?;
            let root = &self.roots[0];
            let validation = self
                .materialized
                .verify_private_bf16_nominal_candidate_translation_with_budget_v1(
                    root.selected_root,
                    &root.lowering,
                    &root.access_sources,
                    &root.executable_effect_sources,
                    budget,
                )?;
            check_private_bf16_attached_report_v1(&self.materialized, &validation, budget)?;
            budget.check_prior_denials_v1()?;
            Ok::<_, ProductionSemanticKirErrorV1>(validation)
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(private_bf16_attachment_panic_error_v1(budget))
            }
        };
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() != protected
        {
            // A broken accounting floor is never repaired by broad refund.
            // The owning caller drops its exact phase only after these objects.
            drop(result);
            drop(self);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        match result {
            Ok(validation) => Ok(ProductionPrivateBf16AttachedRankedOwnerV1 {
                receipt: self,
                validation,
                ledger,
            }),
            Err(error) => {
                drop(self);
                budget.release_storage(header)?;
                Err(error)
            }
        }
    }
}

impl ProductionPrivateBf16AttachedRankedOwnerV1 {
    /// Logical newly retained fixed header; incoming source/root credits remain
    /// retained separately. This inert size is not ownership or admission.
    pub const fn retained_storage_v1() -> usize {
        std::mem::size_of::<Self>()
    }

    /// Returns the actual retained singleton root count.
    pub fn root_count(&self) -> usize {
        self.receipt.root_count()
    }

    /// Private custody alone never authorizes an artifact or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// A usable continuation: replay the full nominal checker on the STILL
    /// INTACT owner and actual ranked roots, then compare the newly derived
    /// report with the retained one. Expected joins are untrusted comparisons.
    /// No source owner, report, or detachable receipt escapes this loan.
    #[doc(hidden)]
    pub fn verify_private_bf16_attached_ranked_with_budget_v1(
        &self,
        expected_semantic_sha256: &[u8; 32],
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        check_private_bf16_attachment_ledger_v1(self.ledger, budget)?;
        let retained_floor = self
            .receipt
            .materialized
            .unit_local_source_storage_floor_v1()?
            .checked_add(Self::retained_storage_v1())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if budget.storage() < retained_floor {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.receipt
            .verify_private_bf16_module_roster_with_budget_v1(
                expected_semantic_sha256,
                expected_root,
                expected_return,
                budget,
            )?;
        let floor = budget.storage();
        let slot = budget as *const _ as usize;
        let scratch = std::mem::size_of::<ProductionMirPlironTranslationValidationV1>();
        let protected = floor
            .checked_add(scratch)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.reserve_storage(scratch)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let root = &self.receipt.roots[0];
            let fresh = self
                .receipt
                .materialized
                .verify_private_bf16_nominal_candidate_translation_with_budget_v1(
                    root.selected_root,
                    &root.lowering,
                    &root.access_sources,
                    &root.executable_effect_sources,
                    budget,
                )?;
            check_private_bf16_attached_report_v1(&self.receipt.materialized, &fresh, budget)?;
            // Exact fixed report equality, prepaid before field comparisons.
            budget.charge_work(32 + 5)?;
            let same = fresh == self.validation;
            drop(fresh);
            budget.check_prior_denials_v1()?;
            if !same {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok::<_, ProductionSemanticKirErrorV1>(())
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(private_bf16_attachment_panic_error_v1(budget))
            }
        };
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() != protected
        {
            drop(result);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        // No successful report or Copy counter survives this scratch refund.
        budget.release_storage(scratch)?;
        result
    }
}

#[cfg(test)]
mod private_attachment_resource_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    #[test]
    fn private_attachment_fixed_header_exact_and_one_short() {
        let header = ProductionPrivateBf16AttachedRankedOwnerV1::retained_storage_v1();
        assert!(header > std::mem::size_of::<ProductionMirPlironTranslationValidationV1>());
        for (work_limit, storage_limit, accepted) in [
            (1, 7 + header, true),
            (0, 7 + header, false),
            (1, 7 + header - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = reserve_private_bf16_attachment_header_v1(&mut budget);
            assert_eq!(result.is_ok(), accepted);
            assert!(budget.work_ledger_identity_v1() == ledger);
            if accepted {
                assert_eq!(result.unwrap(), header);
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (1, 7 + header, 7 + header)
                );
                budget.release_storage(header).unwrap();
            } else if work_limit == 0 {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            Resource::Work(_)
                        )
                    )
                ));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (0, 7, 7)
                );
                assert!(budget.failed_work().is_some());
                assert!(budget.failed_storage().is_none());
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            Resource::Storage(_)
                        )
                    )
                ));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (1, 7, 7)
                );
                assert!(budget.failed_work().is_none());
                assert!(budget.failed_storage().is_some());
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_attachment_fixed_header_preserves_original_denials() {
        for storage in [false, true] {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if storage {
                assert!(budget.reserve_storage(1).is_err());
            } else {
                assert!(budget.charge_work(9).is_err());
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
                    matches!(reserve_private_bf16_attachment_header_v1(&mut budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                        if error == original)
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
    fn private_attachment_replay_rejects_equal_counters_on_another_account() {
        let mut original_work = Work::new(10);
        let mut other_work = Work::new(10);
        let mut original = Budget::new(&mut original_work, 7);
        let mut other = Budget::new(&mut other_work, 7);
        original.reserve_storage(7).unwrap();
        other.reserve_storage(7).unwrap();
        let ledger = original.work_ledger_identity_v1();
        assert!(ledger != other.work_ledger_identity_v1());
        check_private_bf16_attachment_ledger_v1(ledger, &mut original).unwrap();
        assert!(matches!(
            check_private_bf16_attachment_ledger_v1(ledger, &mut other),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting))
        ));
        assert_eq!(
            (original.work(), original.storage(), original.peak_storage()),
            (other.work(), other.storage(), other.peak_storage())
        );
        assert!(original.check_prior_denials_v1().is_ok());
        assert!(other.check_prior_denials_v1().is_ok());
    }

    #[test]
    fn private_attachment_panic_mapping_preserves_first_original_resource_kind() {
        // 0: no history; 1: Work; 2: Storage; 3: both, existing Work precedence.
        for kind in 0..4 {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            if kind == 1 || kind == 3 {
                assert!(budget.charge_work(9).is_err());
            }
            if kind == 2 || kind == 3 {
                assert!(budget.reserve_storage(1).is_err());
            }
            let original = budget.check_prior_denials_v1().err();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                let error = private_bf16_attachment_panic_error_v1(&budget);
                match &original {
                    Some(expected) => assert!(matches!(error,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual)
                            if &actual == expected)),
                    None => assert!(matches!(
                        error,
                        ProductionSemanticKirErrorV1::MirPlironTranslation(
                            ProductionMirPlironTranslationErrorV1::KernelShape
                        )
                    )),
                }
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
}

impl ProductionPrivateBf16AttachedRankedOwnerV1 {
    /// Replays the actual retained intact owner and report on its stored
    /// original Work identity, comparing the authenticated root/requested Return
    /// without exporting or retaining an external source snapshot. Move-only
    /// owner custody, not the internal SHA self-comparison, preserves source
    /// identity. Exact fresh-report equality and every existing check remain.
    #[doc(hidden)]
    pub fn verify_private_bf16_attached_retained_source_with_budget_v1(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.verify_private_bf16_attached_ranked_with_budget_v1(
            self.receipt
                .materialized
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            expected_root,
            expected_return,
            budget,
        )
    }
}

type PrivateBf16FormalAttemptV1 = Result<
    fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
    fe2o3_kernel_ir::FormalMemoryObligationError,
>;

fn private_bf16_formal_frame_storage_v1<F, I>(
    _: &F,
    _: &I,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    std::mem::size_of::<PrivateBf16FormalAttemptV1>()
        .checked_add(std::mem::size_of::<F>())
        .and_then(|n| n.checked_add(std::mem::size_of::<I>()))
        .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
}

fn with_private_bf16_formal_frame_v1<'work, F, I>(
    budget: &mut ArgumentBudgetV1<'work>,
    make: F,
    inspect: I,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    F: FnOnce() -> PrivateBf16FormalAttemptV1,
    I: FnOnce(
        &PrivateBf16FormalAttemptV1,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
{
    budget.check_prior_denials_v1()?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const _ as usize;
    let header = private_bf16_formal_frame_storage_v1(&make, &inspect)?;
    let protected = floor
        .checked_add(header)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    budget.charge_work(1)?;
    budget.reserve_storage(header)?;
    // This fixed frame is selected accounting only. Formal extraction's legacy
    // vector/graph/verification work and payload retain their existing domain.
    // The entry returns no owned analysis/report/error payload. The callback
    // is diagnostic only; observations cannot grant source or launch authority.
    let attempt = make();
    let inspected = inspect(&attempt, budget);
    drop(attempt);
    if budget as *const _ as usize != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() != protected
    {
        // Never repair callback-owned surplus or an invalid floor by subtraction.
        // Preserve original sticky refusal before inventing another diagnosis.
        budget.check_prior_denials_v1()?;
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(header)?;
    budget.check_prior_denials_v1()?;
    inspected
}

impl ProductionPrivateBf16AttachedRankedOwnerV1 {
    /// Borrows a fresh formal-memory analysis attempt from this INTACT nominal
    /// owner, on its original projection account, without granting admission.
    ///
    /// A fresh full nominal replay precedes extraction. The kernel is the actual
    /// singleton executable root; production witness extents and Bits64 remain
    /// descriptive analysis inputs, not authenticated runtime launch facts.
    /// The callback observes Complete, Incomplete, or the exact engine error.
    /// Returning Ok means only that diagnostic collection completed.
    ///
    /// The same original ledger prepays only this new fixed Result/callback
    /// frame. Formal extraction, public verification, graph reconstruction and
    /// their legacy allocations remain excluded, as in existing bounded-
    /// translation formal admission. Existing guarded/effect/CFG limits are
    /// unchanged. This is not a whole-analysis or allocator/RSS bound.
    ///
    /// On ordinary Result paths the actual attempt dies before exact frame
    /// refund. On unwind its locals die but accepted credit is conservatively
    /// retained; the caller's original owning-phase panic/drop guard is required.
    /// No source, report, receipt token or paid observation is returned.
    #[doc(hidden)]
    pub fn with_private_bf16_formal_diagnostic_v1<'work, I>(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'work>,
        inspect: I,
    ) -> Result<(), ProductionSemanticKirErrorV1>
    where
        I: FnOnce(
            &Result<
                fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                fe2o3_kernel_ir::FormalMemoryObligationError,
            >,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        self.verify_private_bf16_attached_retained_source_with_budget_v1(
            expected_root,
            expected_return,
            budget,
        )?;
        budget.check_prior_denials_v1()?;
        // Singleton selection and at most three witness-axis visits.
        budget.charge_work(8)?;
        let module = self.receipt.materialized.executable.module();
        let [kernel] = module.kernels.as_slice() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let witness = fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: kernel.domain.rank(),
            extents: crate::production_formal_memory_v1::witness_extents(&kernel.domain),
        };
        with_private_bf16_formal_frame_v1(
            budget,
            move || {
                fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                    module,
                    &kernel.id,
                    witness,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                )
            },
            inspect,
        )
    }
}

#[cfg(test)]
mod private_formal_diagnostic_resource_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work, FormalGuardedMemoryResourceErrorV1 as GuardError,
        FormalMemoryObligationError as FormalError,
    };

    fn engine_error() -> PrivateBf16FormalAttemptV1 {
        Err(FormalError::GuardedResource(GuardError::Accounting))
    }
    fn observe_engine_error(
        attempt: &PrivateBf16FormalAttemptV1,
        _: &mut Budget<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        assert!(matches!(
            attempt,
            Err(FormalError::GuardedResource(GuardError::Accounting))
        ));
        Ok(())
    }
    fn observer_refusal(
        _: &PrivateBf16FormalAttemptV1,
        _: &mut Budget<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    #[test]
    fn formal_diagnostic_fixed_frame_exact_and_one_short() {
        let make = engine_error;
        let inspect = observe_engine_error;
        let header = private_bf16_formal_frame_storage_v1(&make, &inspect).unwrap();
        for (work_limit, storage_limit, accepted) in [
            (1, 7 + header, true),
            (0, 7 + header, false),
            (1, 7 + header - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = with_private_bf16_formal_frame_v1(&mut budget, make, inspect);
            assert_eq!(result.is_ok(), accepted);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), 7);
            if accepted {
                assert_eq!((budget.work(), budget.peak_storage()), (1, 7 + header));
            } else if work_limit == 0 {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            Resource::Work(_)
                        )
                    )
                ));
                assert_eq!((budget.work(), budget.peak_storage()), (0, 7));
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            Resource::Storage(_)
                        )
                    )
                ));
                assert_eq!((budget.work(), budget.peak_storage()), (1, 7));
            }
        }
    }

    #[test]
    fn formal_diagnostic_callback_refusal_refunds_only_dead_frame() {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(7).unwrap();
        let header =
            private_bf16_formal_frame_storage_v1(&engine_error, &observer_refusal).unwrap();
        assert!(matches!(
            with_private_bf16_formal_frame_v1(&mut budget, engine_error, observer_refusal,),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (1, 7, 7 + header)
        );
        assert!(budget.check_prior_denials_v1().is_ok());
    }

    #[test]
    fn formal_diagnostic_prior_denial_precedes_new_frame_work() {
        for storage in [false, true] {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            let original = if storage {
                budget.reserve_storage(1).unwrap_err()
            } else {
                budget.charge_work(4).unwrap_err()
            };
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            for _ in 0..2 {
                assert!(matches!(with_private_bf16_formal_frame_v1(
                    &mut budget, engine_error, observe_engine_error,
                ), Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                    if error == original));
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
    fn formal_diagnostic_never_refunds_callback_surplus_or_accepts_changed_floor() {
        for release in [false, true] {
            let mut work = Work::new(10);
            let mut budget = Budget::new(&mut work, 10_000);
            budget.reserve_storage(7).unwrap();
            let inspect = |_: &PrivateBf16FormalAttemptV1, budget: &mut Budget<'_>| {
                if release {
                    budget.release_storage(1)?;
                } else {
                    budget.reserve_storage(1)?;
                }
                Ok(())
            };
            let header = private_bf16_formal_frame_storage_v1(&engine_error, &inspect).unwrap();
            assert!(matches!(
                with_private_bf16_formal_frame_v1(&mut budget, engine_error, inspect,),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        Resource::Accounting
                    )
                )
            ));
            assert_eq!(
                budget.storage(),
                if release {
                    7 + header - 1
                } else {
                    7 + header + 1
                }
            );
            assert!(budget.check_prior_denials_v1().is_ok());
        }
    }

    #[test]
    fn formal_diagnostic_panic_cannot_publish_completion_or_broadly_refund() {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(7).unwrap();
        let inspect = |_: &PrivateBf16FormalAttemptV1,
                       _: &mut Budget<'_>|
         -> Result<(), ProductionSemanticKirErrorV1> {
            panic!("diagnostic callback");
        };
        let header = private_bf16_formal_frame_storage_v1(&engine_error, &inspect).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_private_bf16_formal_frame_v1(&mut budget, engine_error, inspect)
        }));
        assert!(result.is_err());
        assert_eq!((budget.work(), budget.storage()), (1, 7 + header));
        // This isolated frame-control test owns the exact dead frame credit.
        // The genuine caller instead drops its poisoned owning phase.
        budget.release_storage(header).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn formal_diagnostic_engine_panic_retains_only_accepted_frame_credit() {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(7).unwrap();
        let make = || -> PrivateBf16FormalAttemptV1 {
            panic!("formal engine");
        };
        let header = private_bf16_formal_frame_storage_v1(&make, &observe_engine_error).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_private_bf16_formal_frame_v1(&mut budget, make, observe_engine_error)
        }));
        assert!(result.is_err());
        assert_eq!((budget.work(), budget.storage()), (1, 7 + header));
        assert!(budget.check_prior_denials_v1().is_ok());
        budget.release_storage(header).unwrap();
        assert_eq!(budget.storage(), 7);
    }
}

type PrivateBf16GuardedFormalResultV1 = Result<(), ProductionMemoryDischargeFailureV1>;

// Move-only local carrier: exact typed details are lent, then dropped before refund.
struct PrivateBf16GuardedFormalProofV1(PrivateBf16GuardedFormalResultV1);

fn private_bf16_ranked_formal_frame_storage_v1<I>(
    _: &I,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    std::mem::size_of::<Vec<FunctionOperationLocation>>()
        .checked_add(std::mem::size_of::<PrivateBf16GuardedFormalProofV1>())
        .and_then(|n| n.checked_add(std::mem::size_of::<GuardedAddressProofBudgetV1>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<I>()))
        .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
}

#[allow(
    clippy::too_many_arguments,
    reason = "Borrow one fresh actual report and its unchanged owner/witness on the original account"
)]
fn with_private_bf16_ranked_formal_frame_v1<'work, I>(
    module: &Module,
    kernel: &Kernel,
    witness: [u64; 3],
    max_operations: usize,
    selected_root: u32,
    attempt: &PrivateBf16FormalAttemptV1,
    budget: &mut ArgumentBudgetV1<'work>,
    inspect: I,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    I: FnOnce(
        &PrivateBf16FormalAttemptV1,
        &PrivateBf16GuardedFormalResultV1,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
{
    budget.check_prior_denials_v1()?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const _ as usize;
    budget.charge_work(4)?;
    let count = attempt
        .as_ref()
        .map_or(0, |analysis| analysis.incomplete_reasons().len());
    // An excessive reason roster refuses before allocating a location vector.
    let capacity = if count <= max_operations { count } else { 0 };
    let payload = capacity
        .checked_mul(std::mem::size_of::<FunctionOperationLocation>())
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let storage = private_bf16_ranked_formal_frame_storage_v1(&inspect)?
        .checked_add(payload)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let protected = floor
        .checked_add(storage)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    budget.reserve_storage(storage)?;
    let mut locations = Vec::new();
    let derived = (|| -> Result<PrivateBf16GuardedFormalResultV1, ProductionSemanticKirErrorV1> {
        let analysis = match attempt {
            Ok(analysis) => analysis,
            Err(_) => {
                return Ok(Err(ProductionMemoryDischargeFailureV1::stage(
                    "private nominal formal extraction failed; exact error retained in fresh attempt",
                )));
            }
        };
        let report = analysis.obligations();
        if !report.inter_invocation_conflicts().is_empty() {
            return Ok(Err(ProductionMemoryDischargeFailureV1::stage(
                "private nominal formal obligations retain inter-invocation conflicts",
            )));
        }
        let reasons = analysis.incomplete_reasons();
        if reasons.len() > max_operations {
            return Ok(Err(ProductionMemoryDischargeFailureV1::stage(
                "private nominal guarded reason roster exceeds the operation limit",
            )));
        }
        budget.charge_work(reasons.len())?;
        if reasons.iter().any(|reason| {
            !matches!(
                reason,
                FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }
            )
        }) {
            return Ok(Err(ProductionMemoryDischargeFailureV1::stage(
                "private nominal formal analysis has an unrelated incomplete reason",
            )));
        }
        if reasons.is_empty() {
            return Ok(if analysis.is_complete() {
                Ok(())
            } else {
                Err(ProductionMemoryDischargeFailureV1::stage(
                    "private nominal incomplete analysis has no exact discharge reasons",
                ))
            });
        }
        // Requested payload was paid before allocation; allocator slack is not
        // the logical storage domain. The original reason vector is untouched.
        locations
            .try_reserve_exact(capacity)
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        budget.charge_work(reasons.len())?;
        for reason in reasons {
            let FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } =
                reason
            else {
                unreachable!("closed reason roster checked before allocation")
            };
            locations.push(*location);
        }
        // Each pending reason must have exactly one actual fresh Read row.
        // Prepay both known-length loops before either traversal begins.
        let row_work = locations
            .len()
            .checked_mul(report.accesses().len())
            .and_then(|n| n.checked_mul(3))
            .and_then(|n| n.checked_add(locations.len()))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(row_work)?;
        for location in &locations {
            let matches = report
                .accesses()
                .iter()
                .filter(|row| {
                    row.location() == *location && row.kind() == FormalMemoryAccessKind::Read
                })
                .count();
            if matches != 1 {
                return Ok(Err(ProductionMemoryDischargeFailureV1::access(
                    *location,
                    "private nominal pending guard lacks one exact fresh Read row",
                )));
            }
        }
        // One original-account debit buys the entire unchanged local allowance.
        // It is not replenished per reason and unused work is never refunded.
        // Legacy definition/tree construction and its allocations remain outside
        // this selected-step allowance, exactly as for the existing guard engine.
        let allowance = max_operations
            .checked_mul(GUARDED_ADDRESS_PROOF_STEPS_PER_OPERATION_V1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(allowance)?;
        let mut proof_budget = match GuardedAddressProofBudgetV1::new(max_operations) {
            Ok(proof_budget) => proof_budget,
            Err(error) => return Ok(Err(error)),
        };
        let proof = guarded_accesses_have_structural_bounds_result(
            module,
            kernel,
            report,
            witness,
            &locations,
            max_operations,
            &mut proof_budget,
        );
        drop(proof_budget);
        Ok(proof)
    })();
    let (proved, inspected) = match derived {
        Ok(proof) => {
            let proof = PrivateBf16GuardedFormalProofV1(proof);
            let proved = proof.0.is_ok();
            let inspected = inspect(attempt, &proof.0, budget);
            // No paid diagnostic/proof detail survives the exact frame refund.
            drop(proof);
            (proved, inspected)
        }
        Err(error) => {
            // This uninvoked FnOnce is part of the paid frame too. Its captures
            // must die before refund, including when derived work was denied.
            drop(inspect);
            (false, Err(error))
        }
    };
    drop(locations);
    if budget as *const _ as usize != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() != protected
    {
        drop(inspected);
        budget.check_prior_denials_v1()?;
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(storage)?;
    budget.check_prior_denials_v1()?;
    // Original typed resource errors and the callback's own refusal survive.
    inspected?;
    if !proved {
        // Keep the public exhaustive error enum unchanged. The callback has
        // already borrowed the exact typed guard failure and unchanged engine
        // error/reason data; ignoring those cannot turn this refusal into Ok.
        return Err(unsupported(
            selected_root,
            None,
            None,
            "private nominal ranked formal guard discharge refused",
        ));
    }
    Ok(())
}

impl ProductionPrivateBf16AttachedRankedOwnerV1 {
    /// Runs a fresh owner-bound nominal replay and formal analysis, then proves
    /// every pending guarded-read reason against that same actual module/root.
    ///
    /// The callback borrows the unchanged raw attempt and exact typed discharge
    /// result. Successful discharge keeps the original Incomplete reasons as
    /// discharged records; it does not manufacture an engine Complete result.
    /// All bounds/alias obligations remain unauthenticated runtime obligations.
    /// The callback returns unit, exports no receipt, and cannot waive a refusal:
    /// this entry returns Ok only after every guard and postflight succeeds.
    ///
    /// New fixed/location scratch and the existing 32*max_operations selected
    /// guard allowance are prepaid on the original projection account. The
    /// inherited formal/graph/tree/parameter-definition/formatter domains remain
    /// excluded: this is not whole-analysis work, allocator, stack or RSS admission.
    /// Panic leaves conservative accepted credit for the owning phase to drop.
    /// Ordinary dispatch, legacy attachment and target/launch gates are unchanged.
    #[doc(hidden)]
    pub fn with_private_bf16_ranked_formal_v1<'work, I>(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'work>,
        inspect: I,
    ) -> Result<(), ProductionSemanticKirErrorV1>
    where
        I: FnOnce(
            &Result<
                fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                fe2o3_kernel_ir::FormalMemoryObligationError,
            >,
            &Result<(), ProductionMemoryDischargeFailureV1>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        // Only this existing owner method constructs the fresh attempt. No
        // external report, reason list, source clone or digest token is accepted.
        self.with_private_bf16_formal_diagnostic_v1(
            expected_root,
            expected_return,
            budget,
            |attempt, budget| {
                budget.check_prior_denials_v1()?;
                budget.charge_work(8)?;
                let module = self.receipt.materialized.executable.module();
                let [kernel] = module.kernels.as_slice() else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                with_private_bf16_ranked_formal_frame_v1(
                    module,
                    kernel,
                    crate::production_formal_memory_v1::witness_extents(&kernel.domain),
                    self.receipt.materialized.limits.max_operations,
                    expected_root.index(),
                    attempt,
                    budget,
                    inspect,
                )
            },
        )
    }
}

// Test-only access for resource controls that reuse the parent's inert graphs.
// Production helpers and their visibility remain unchanged.
#[cfg(test)]
pub(super) type PrivateBf16FormalAttemptForTestV1 = PrivateBf16FormalAttemptV1;

#[cfg(test)]
pub(super) type PrivateBf16GuardedFormalResultForTestV1 = PrivateBf16GuardedFormalResultV1;

#[cfg(test)]
pub(super) fn private_bf16_ranked_formal_frame_storage_for_test_v1<I>(
    inspect: &I,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    private_bf16_ranked_formal_frame_storage_v1(inspect)
}

#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "Test-only forwarding bridge to the unchanged private helper"
)]
pub(super) fn with_private_bf16_ranked_formal_frame_for_test_v1<'work, I>(
    module: &Module,
    kernel: &Kernel,
    witness: [u64; 3],
    max_operations: usize,
    selected_root: u32,
    attempt: &PrivateBf16FormalAttemptForTestV1,
    budget: &mut ArgumentBudgetV1<'work>,
    inspect: I,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    I: FnOnce(
        &PrivateBf16FormalAttemptForTestV1,
        &PrivateBf16GuardedFormalResultForTestV1,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
{
    with_private_bf16_ranked_formal_frame_v1(
        module,
        kernel,
        witness,
        max_operations,
        selected_root,
        attempt,
        budget,
        inspect,
    )
}

/// Private formal-memory continuation retaining the exact intact nominal owner.
/// The raw analysis and its original reasons are moved, never cloned or relabeled.
/// Bounds and alias requirements remain unauthenticated runtime obligations.
///
/// The caller must retain both original accounts. This fixed owner and selected
/// replay/guard work are metered; the legacy formal engine, its vector payloads,
/// exact report comparison and allocations retain their existing excluded domain.
#[doc(hidden)]
#[must_use = "dropping the private formal owner abandons its intact source and obligations"]
pub struct ProductionPrivateBf16FormalMemoryOwnerV1 {
    // Formal rows die before their actual source; the caller's phase dies last.
    analysis: fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
    attached: ProductionPrivateBf16AttachedRankedOwnerV1,
    retained_floor: usize,
}

type PrivateBf16FormalOwnerErrorV1 = crate::ProductionFormalMemoryErrorV1;

fn private_bf16_formal_owner_resource_v1(
    error: ArgumentResourceV1,
) -> PrivateBf16FormalOwnerErrorV1 {
    PrivateBf16FormalOwnerErrorV1::SemanticKir(error.into())
}

fn reserve_private_bf16_formal_owner_header_v1(
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    let header = ProductionPrivateBf16FormalMemoryOwnerV1::retained_storage_v1();
    // Incoming attachment credits remain retained separately. This new logical
    // owner header also covers its not-yet-moved analysis/result representation;
    // this is not a bound on incidental Rust stack temporaries or engine heap.
    if header < std::mem::size_of::<PrivateBf16FormalAttemptV1>() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.reserve_storage(header)?;
    Ok(header)
}

fn check_private_bf16_formal_owner_account_v1(
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    retained_floor: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_private_bf16_attachment_ledger_v1(ledger, budget)?;
    if budget.storage() < retained_floor {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

fn require_private_bf16_formal_analysis_match_v1(
    fresh: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
    retained: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if fresh != retained {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn require_private_bf16_formal_analysis_match_for_test_v1(
    fresh: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
    retained: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
) -> Result<(), ProductionSemanticKirErrorV1> {
    require_private_bf16_formal_analysis_match_v1(fresh, retained)
}

fn replay_private_bf16_formal_analysis_v1(
    attached: &ProductionPrivateBf16AttachedRankedOwnerV1,
    retained: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
    expected_root: SemanticFunctionIdV1,
    expected_return: [u8; 4],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    attached.with_private_bf16_ranked_formal_v1(
        expected_root,
        expected_return,
        budget,
        |fresh, proof, budget| {
            budget.check_prior_denials_v1()?;
            // The shared continuation returns the original guard refusal even
            // if this callback returns Ok; a failed proof is never admitted.
            if proof.is_err() {
                return Ok(());
            }
            let Ok(fresh) = fresh else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            // Exact full formal comparison retains the existing formal-engine
            // accounting domain, just as ordinary formal replay does.
            require_private_bf16_formal_analysis_match_v1(fresh, retained)
        },
    )
}

impl ProductionPrivateBf16AttachedRankedOwnerV1 {
    /// Consumes this exact attachment into a private formal owner on its stored
    /// original ledger. No caller-written report or source token is accepted.
    /// A fresh raw analysis is proved, then independently rederived and proved
    /// again before that SAME first analysis is moved into the returned owner.
    /// Ordinary dispatch, legacy Connected attachment and target gates stay shut.
    #[doc(hidden)]
    pub fn into_private_bf16_formal_memory_with_budget_v1(
        self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionPrivateBf16FormalMemoryOwnerV1, crate::ProductionFormalMemoryErrorV1>
    {
        check_private_bf16_attachment_ledger_v1(self.ledger, budget)
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
        let floor = budget.storage();
        let slot = budget as *const _ as usize;
        let ledger = self.ledger;
        let protected = floor
            .checked_add(ProductionPrivateBf16FormalMemoryOwnerV1::retained_storage_v1())
            .ok_or_else(|| private_bf16_formal_owner_resource_v1(ArgumentResourceV1::Arithmetic))?;
        let header = reserve_private_bf16_formal_owner_header_v1(budget)
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.verify_private_bf16_attached_retained_source_with_budget_v1(
                expected_root,
                expected_return,
                budget,
            )
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
            budget
                .check_prior_denials_v1()
                .map_err(private_bf16_formal_owner_resource_v1)?;
            budget
                .charge_work(8)
                .map_err(private_bf16_formal_owner_resource_v1)?;
            let module = self.receipt.materialized.executable.module();
            let [kernel] = module.kernels.as_slice() else {
                return Err(PrivateBf16FormalOwnerErrorV1::KernelCount {
                    actual: module.kernels.len(),
                });
            };
            let extents = crate::production_formal_memory_v1::witness_extents(&kernel.domain);
            let witness = fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            };
            // This is the actual first analysis, not a callback-report clone.
            let attempt: PrivateBf16FormalAttemptV1 = Ok(
                fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                    module,
                    &kernel.id,
                    witness,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                )
                .map_err(PrivateBf16FormalOwnerErrorV1::Analysis)?,
            );
            with_private_bf16_ranked_formal_frame_v1(
                module,
                kernel,
                extents,
                self.receipt.materialized.limits.max_operations,
                expected_root.index(),
                &attempt,
                budget,
                |_, _, _| Ok(()),
            )
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
            let analysis = attempt.map_err(PrivateBf16FormalOwnerErrorV1::Analysis)?;
            replay_private_bf16_formal_analysis_v1(
                &self,
                &analysis,
                expected_root,
                expected_return,
                budget,
            )
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
            budget
                .check_prior_denials_v1()
                .map_err(private_bf16_formal_owner_resource_v1)?;
            Ok::<_, PrivateBf16FormalOwnerErrorV1>(analysis)
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(PrivateBf16FormalOwnerErrorV1::SemanticKir(
                    private_bf16_attachment_panic_error_v1(budget),
                ))
            }
        };
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() != protected
        {
            drop(result);
            drop(self);
            // No broad refund can erase a failed callback/engine's live floor.
            budget
                .check_prior_denials_v1()
                .map_err(private_bf16_formal_owner_resource_v1)?;
            return Err(private_bf16_formal_owner_resource_v1(
                ArgumentResourceV1::Accounting,
            ));
        }
        if let Err(error) = budget.check_prior_denials_v1() {
            drop(result);
            drop(self);
            budget
                .release_storage(header)
                .map_err(private_bf16_formal_owner_resource_v1)?;
            return Err(private_bf16_formal_owner_resource_v1(error));
        }
        match result {
            Ok(analysis) => Ok(ProductionPrivateBf16FormalMemoryOwnerV1 {
                analysis,
                attached: self,
                retained_floor: protected,
            }),
            Err(error) => {
                drop(self);
                budget
                    .release_storage(header)
                    .map_err(private_bf16_formal_owner_resource_v1)?;
                Err(error)
            }
        }
    }
}

impl ProductionPrivateBf16FormalMemoryOwnerV1 {
    /// Newly retained logical fixed header, excluding existing formal payloads.
    pub const fn retained_storage_v1() -> usize {
        std::mem::size_of::<Self>()
    }

    /// Actual unchanged singleton root count; no report can manufacture it.
    pub fn root_count(&self) -> usize {
        self.attached.root_count()
    }

    /// Borrows the actual retained obligations, including runtime bounds/aliases.
    /// These rows are not authenticated launch or allocation evidence.
    pub fn obligations(&self) -> &FormalMemoryObligations {
        self.analysis.obligations()
    }

    /// Borrows the exact original reasons proved by the private guard engine.
    /// The underlying raw Incomplete analysis was not relabeled Complete.
    pub fn ranked_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        self.analysis.incomplete_reasons()
    }

    /// Reports the actual core-analysis classification, not composed discharge.
    pub fn raw_analysis_is_complete(&self) -> bool {
        self.analysis.is_complete()
    }

    /// Private retention never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Replays source/full translation, fresh formal extraction and all guarded
    /// proof joins, then compares the entire actual analysis on the same phase.
    #[doc(hidden)]
    pub fn verify_private_bf16_formal_memory_with_budget_v1(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), crate::ProductionFormalMemoryErrorV1> {
        check_private_bf16_formal_owner_account_v1(
            self.attached.ledger,
            self.retained_floor,
            budget,
        )
        .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
        replay_private_bf16_formal_analysis_v1(
            &self.attached,
            &self.analysis,
            expected_root,
            expected_return,
            budget,
        )
        .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)
    }
}

#[cfg(test)]
mod private_formal_owner_resource_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    #[test]
    fn private_formal_owner_header_exact_and_one_short() {
        let header = ProductionPrivateBf16FormalMemoryOwnerV1::retained_storage_v1();
        assert!(header >= std::mem::size_of::<PrivateBf16FormalAttemptV1>());
        for (work_limit, storage_limit, accepted) in [
            (1, 7 + header, true),
            (0, 7 + header, false),
            (1, 7 + header - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = reserve_private_bf16_formal_owner_header_v1(&mut budget);
            assert_eq!(result.is_ok(), accepted);
            assert!(budget.work_ledger_identity_v1() == ledger);
            if accepted {
                assert_eq!(result.unwrap(), header);
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (1, 7 + header, 7 + header)
                );
                budget.release_storage(header).unwrap();
            } else {
                assert_eq!(budget.storage(), 7);
                assert_eq!(budget.failed_work().is_some(), work_limit == 0);
                assert_eq!(budget.failed_storage().is_some(), work_limit != 0);
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_formal_owner_account_rejects_same_counters_foreign_ledger_and_short_floor() {
        let mut work = Work::new(10);
        let mut other_work = Work::new(10);
        let mut budget = Budget::new(&mut work, 8);
        let mut other = Budget::new(&mut other_work, 8);
        budget.reserve_storage(7).unwrap();
        other.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(check_private_bf16_formal_owner_account_v1(ledger, 7, &mut budget).is_ok());
        assert!(matches!(
            check_private_bf16_formal_owner_account_v1(ledger, 7, &mut other),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting))
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (other.work(), other.storage(), other.peak_storage())
        );
        assert!(matches!(
            check_private_bf16_formal_owner_account_v1(ledger, 8, &mut budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn private_formal_owner_header_and_replay_preserve_prior_denials() {
        for kind in 0..3 {
            let mut work = Work::new(5);
            let mut budget = Budget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            budget.charge_work(5).unwrap();
            let ledger = budget.work_ledger_identity_v1();
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
                    matches!(reserve_private_bf16_formal_owner_header_v1(&mut budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                    if error == original)
                );
                assert!(
                    matches!(check_private_bf16_formal_owner_account_v1(ledger, 7, &mut budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                    if error == original)
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
}

fn private_bf16_target_frame_storage_v1<I>() -> Result<usize, ProductionSemanticKirErrorV1> {
    std::mem::size_of::<Option<I>>()
        .checked_add(std::mem::size_of::<Result<(), PrivateBf16FormalOwnerErrorV1>>())
        .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
}

fn reserve_private_bf16_target_frame_v1<I>(
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    let storage = private_bf16_target_frame_storage_v1::<I>()?;
    budget.reserve_storage(storage)?;
    Ok(storage)
}

impl ProductionPrivateBf16FormalMemoryOwnerV1 {
    /// Lends the actual executable graph, selected source function and retained
    /// obligations only after a fresh full nominal/formal replay on this owner's
    /// original account. No source owner, report or authority token is returned.
    ///
    /// The callback returns unit. Any separately retained target output must
    /// already have its own caller-paid reservation and remain owned with this
    /// source and account. Callback storage must return to its incoming floor.
    /// Geometry, target cloning/verification and their existing tree/payload
    /// domains are not made globally metered by this fixed callback frame.
    #[doc(hidden)]
    pub fn with_private_bf16_target_source_v1<'work, I>(
        &self,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'work>,
        inspect: I,
    ) -> Result<(), crate::ProductionFormalMemoryErrorV1>
    where
        I: FnOnce(
            &fe2o3_kernel_ir::Module,
            &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
            &FormalMemoryObligations,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        check_private_bf16_formal_owner_account_v1(
            self.attached.ledger,
            self.retained_floor,
            budget,
        )
        .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
        let floor = budget.storage();
        let slot = budget as *const _ as usize;
        let ledger = budget.work_ledger_identity_v1();
        let storage = reserve_private_bf16_target_frame_v1::<I>(budget)
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)?;
        let protected = floor
            .checked_add(storage)
            .ok_or_else(|| private_bf16_formal_owner_resource_v1(ArgumentResourceV1::Arithmetic))?;
        let mut inspect = Some(inspect);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.verify_private_bf16_formal_memory_with_budget_v1(
                expected_root,
                expected_return,
                budget,
            )?;
            budget
                .check_prior_denials_v1()
                .map_err(private_bf16_formal_owner_resource_v1)?;
            budget
                .charge_work(4)
                .map_err(private_bf16_formal_owner_resource_v1)?;
            let semantic = self
                .attached
                .receipt
                .materialized
                .semantic_ssa()
                .source_semantic();
            let [root] = semantic.roots() else {
                return Err(PrivateBf16FormalOwnerErrorV1::SemanticKir(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                ));
            };
            if *root != expected_root {
                return Err(PrivateBf16FormalOwnerErrorV1::SemanticKir(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                ));
            }
            let function = semantic
                .functions()
                .get(expected_root.index() as usize)
                .ok_or(PrivateBf16FormalOwnerErrorV1::SemanticKir(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                ))?;
            let callback = inspect.take().ok_or_else(|| {
                private_bf16_formal_owner_resource_v1(ArgumentResourceV1::Accounting)
            })?;
            callback(
                self.attached.receipt.materialized.executable.module(),
                function,
                self.analysis.obligations(),
                budget,
            )
            .map_err(PrivateBf16FormalOwnerErrorV1::SemanticKir)
        }));
        // In particular, an early replay refusal must destroy the unused
        // captured callback before its paid Option<I> representation is refunded.
        drop(inspect);
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(PrivateBf16FormalOwnerErrorV1::SemanticKir(
                    private_bf16_attachment_panic_error_v1(budget),
                ))
            }
        };
        if budget as *const _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() != protected
        {
            drop(result);
            budget
                .check_prior_denials_v1()
                .map_err(private_bf16_formal_owner_resource_v1)?;
            // Do not refund unknown callback surplus or a violated original floor.
            return Err(private_bf16_formal_owner_resource_v1(
                ArgumentResourceV1::Accounting,
            ));
        }
        // Consume the paid Result representation before its frame refund.
        // A typed diagnostic payload is the same declared excluded error domain
        // as existing formal/target errors, not a retained authority record.
        match result {
            Ok(()) => {
                budget
                    .release_storage(storage)
                    .map_err(private_bf16_formal_owner_resource_v1)?;
                budget
                    .check_prior_denials_v1()
                    .map_err(private_bf16_formal_owner_resource_v1)?;
                Ok(())
            }
            Err(error) => {
                budget
                    .release_storage(storage)
                    .map_err(private_bf16_formal_owner_resource_v1)?;
                budget
                    .check_prior_denials_v1()
                    .map_err(private_bf16_formal_owner_resource_v1)?;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod private_target_frame_resource_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    #[test]
    fn private_target_frame_exact_and_one_short() {
        let frame = private_bf16_target_frame_storage_v1::<[u8; 32]>().unwrap();
        for (work_limit, storage_limit, accepted) in [
            (1, 7 + frame, true),
            (0, 7 + frame, false),
            (1, 7 + frame - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = reserve_private_bf16_target_frame_v1::<[u8; 32]>(&mut budget);
            assert_eq!(result.is_ok(), accepted);
            assert!(budget.work_ledger_identity_v1() == ledger);
            if accepted {
                assert_eq!(result.unwrap(), frame);
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (1, 7 + frame, 7 + frame)
                );
                budget.release_storage(frame).unwrap();
            } else {
                assert_eq!(budget.storage(), 7);
                assert_eq!(budget.failed_work().is_some(), work_limit == 0);
                assert_eq!(budget.failed_storage().is_some(), work_limit != 0);
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn private_target_frame_preserves_original_denials_at_exhausted_work() {
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
                assert!(matches!(
                    reserve_private_bf16_target_frame_v1::<[u8; 32]>(&mut budget),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                        if error == original
                ));
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
}

/// Same-name/domain join only. The enclosing checked optimizer owns B -> O
/// correspondence; this private helper never turns a name into source authority.
fn private_bf16_checked_output_kernel_v1<'o>(
    output: &'o Module,
    source: &Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'o Kernel, ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(8)?;
    let ([kernel], [original]) = (output.kernels.as_slice(), source.kernels.as_slice()) else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let names = kernel
        .id
        .as_str()
        .len()
        .checked_add(kernel.entry.as_str().len())
        .and_then(|n| n.checked_add(original.id.as_str().len()))
        .and_then(|n| n.checked_add(original.entry.as_str().len()))
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    budget.charge_work(names)?;
    if kernel.id != original.id
        || kernel.entry != original.entry
        || kernel.domain != original.domain
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(kernel)
}

fn with_private_bf16_actual_output_formal_v1<'work, I>(
    output: &Module,
    source: &Module,
    max_operations: usize,
    selected_root: u32,
    budget: &mut ArgumentBudgetV1<'work>,
    inspect: I,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    I: FnOnce(
        &PrivateBf16FormalAttemptV1,
        &PrivateBf16GuardedFormalResultV1,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
{
    let kernel = private_bf16_checked_output_kernel_v1(output, source, budget)?;
    let extents = crate::production_formal_memory_v1::witness_extents(&kernel.domain);
    with_private_bf16_formal_frame_v1(
        budget,
        || {
            fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                output,
                &kernel.id,
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: kernel.domain.rank(),
                    extents,
                },
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            )
        },
        |attempt, budget| {
            with_private_bf16_ranked_formal_frame_v1(
                output,
                kernel,
                extents,
                max_operations,
                selected_root,
                attempt,
                budget,
                inspect,
            )
        },
    )
}

impl ProductionPrivateBf16FormalMemoryOwnerV1 {
    /// Private borrowed actual-O rule, not a legacy Direct/Erased source anchor.
    /// Fresh nominal/source/formal replay selects this intact owner's original
    /// operation limit; a new raw analysis and guard proof use ONLY actual O.
    /// The caller must already own and replay the exact checked B -> O relation,
    /// target binding, O backing and both original accounts. This method proves
    /// no lineage for an arbitrary separately borrowed canonical output.
    ///
    /// Original Incomplete reasons remain unchanged borrowed discharged records.
    /// The callback returns unit; ignoring its typed proof cannot waive refusal.
    /// Bounds/alias obligations remain unauthenticated runtime requirements.
    /// Fixed frames, location payload and one unchanged guard allowance use the
    /// supplied original account. Legacy formal/tree/definition/payload work is
    /// still excluded; this is not whole-engine, allocator, stack or RSS metering.
    /// No ordinary admission, LLVM, artifact or launch authority is created.
    #[doc(hidden)]
    pub fn with_private_bf16_checked_output_guarded_formal_v1<'work, I>(
        &self,
        output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        expected_root: SemanticFunctionIdV1,
        expected_return: [u8; 4],
        budget: &mut ArgumentBudgetV1<'work>,
        inspect: I,
    ) -> Result<(), crate::ProductionFormalMemoryErrorV1>
    where
        I: FnOnce(
            &Result<
                fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                fe2o3_kernel_ir::FormalMemoryObligationError,
            >,
            &Result<(), ProductionMemoryDischargeFailureV1>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        self.with_private_bf16_target_source_v1(
            expected_root,
            expected_return,
            budget,
            |source, _function, _source_obligations, budget| {
                with_private_bf16_actual_output_formal_v1(
                    output.module(),
                    source,
                    self.attached.receipt.materialized.limits.max_operations,
                    expected_root.index(),
                    budget,
                    inspect,
                )
            },
        )
    }
}

#[cfg(test)]
mod private_checked_output_guard_tests {
    use super::*;
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, *};

    // Actual formal/structural checker control, not rustc or optimizer lineage.
    // O has a different block and one leading operation from the source fixture.
    fn graph(shifted: bool, mutation: u8) -> Module {
        let slice = Type::slice(
            Type::Scalar(ScalarType::U16),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U16),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let mut block = BasicBlock::new(BlockId(if shifted { 42 } else { 0 }));
        if shifted {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(11), Type::INDEX),
                OperationKind::Constant(Constant::Index(99)),
            ));
        }
        for (id, ty, kind) in [
            (
                3,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            (
                4,
                Type::INDEX,
                OperationKind::SliceLength {
                    slice: ValueId(if mutation == 2 { 1 } else { 0 }),
                },
            ),
            (
                5,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(if mutation == 3 { 1 } else { 0 })),
            ),
            (
                6,
                Type::Scalar(ScalarType::U16),
                OperationKind::Constant(Constant::U16(0)),
            ),
            (
                7,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: if mutation == 1 {
                        ComparePredicate::GreaterThan
                    } else {
                        ComparePredicate::LessThan
                    },
                    lhs: ValueId(2),
                    rhs: ValueId(4),
                },
            ),
            (
                8,
                Type::INDEX,
                OperationKind::Select {
                    condition: ValueId(7),
                    true_value: ValueId(if mutation == 4 { 4 } else { 2 }),
                    false_value: ValueId(5),
                },
            ),
            (
                9,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(8),
                },
            ),
            (
                10,
                Type::Scalar(ScalarType::U16),
                OperationKind::GuardedLoad {
                    pointer: ValueId(9),
                    predicate: ValueId(7),
                    fallback: ValueId(6),
                    access: MemoryAccess::new(AddressSpace::Global, 2),
                },
            ),
        ] {
            block
                .operations
                .push(Operation::effect_free(ValueDef::new(ValueId(id), ty), kind));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("private-actual-output-guard-control");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![slice.clone(), slice, Type::INDEX], vec![]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        module
    }

    fn observe(
        raw: &PrivateBf16FormalAttemptV1,
        proof: &PrivateBf16GuardedFormalResultV1,
        _: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let analysis = raw.as_ref().unwrap();
        assert!(!analysis.is_complete());
        assert_eq!(
            analysis.incomplete_reasons(),
            &[
                FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                    location: FunctionOperationLocation::new(BlockId(42), 8),
                }
            ]
        );
        assert!(proof.is_ok());
        Ok(())
    }

    fn trial(work_cap: usize, storage_cap: usize, mutation: u8) -> (bool, usize, usize, bool) {
        let source = graph(false, 0);
        let output = graph(true, mutation);
        let mut work = Work::new(work_cap);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_cap);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_private_bf16_actual_output_formal_v1(
            &output,
            &source,
            64,
            0,
            &mut budget,
            |raw, proof, budget| {
                if mutation == 0 {
                    observe(raw, proof, budget)
                } else {
                    assert!(raw.as_ref().is_ok_and(|r| !r.is_complete()));
                    assert!(proof.is_err());
                    // Ignoring a real guard refusal must not produce success.
                    Ok(())
                }
            },
        );
        assert_eq!(budget.storage(), 7);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result.is_ok(),
            budget.work(),
            budget.peak_storage(),
            budget.check_prior_denials_v1().is_err(),
        )
    }

    #[test]
    fn actual_output_guard_uses_fresh_shifted_locations_and_preserves_raw_reasons() {
        let (accepted, _, _, denied) = trial(1_000_000, 1_000_000, 0);
        assert!(accepted && !denied);
    }

    #[test]
    fn actual_output_guard_changed_predicate_slice_select_and_index_refuse() {
        for mutation in 1..=4 {
            let (accepted, _, _, denied) = trial(1_000_000, 1_000_000, mutation);
            assert!(!accepted && !denied);
        }
    }

    #[test]
    fn actual_output_guard_exact_and_one_short_work_storage_restore_floor() {
        let (accepted, used, peak, denied) = trial(1_000_000, 1_000_000, 0);
        assert!(accepted && !denied);
        assert!(trial(used, peak, 0).0);
        for (work, storage) in [(used - 1, peak), (used, peak - 1)] {
            let (accepted, _, _, denied) = trial(work, storage, 0);
            assert!(!accepted && denied);
        }
    }

    #[test]
    fn actual_output_guard_roster_name_entry_and_domain_substitution_refuse() {
        let source = graph(false, 0);
        for mutation in 0..5 {
            let mut output = graph(true, 0);
            match mutation {
                0 => output.kernels.clear(),
                1 => output.kernels.push(output.kernels[0].clone()),
                2 => output.kernels[0].id = KernelId::new("other"),
                3 => output.kernels[0].entry = FunctionId::new("other"),
                _ => {
                    output.kernels[0].domain = LaunchDomain::D1 {
                        x: LaunchExtent::Static(32),
                    }
                }
            }
            let mut work = Work::new(1_000_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            assert!(matches!(
                private_bf16_checked_output_kernel_v1(&output, &source, &mut budget),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(budget.storage(), 7);
            assert_eq!(budget.failed_work(), None);
        }
    }

    #[test]
    fn actual_output_guard_original_denials_precede_any_new_debit() {
        let source = graph(false, 0);
        let output = graph(true, 0);
        for kind in 0..3 {
            let mut work = Work::new(5);
            let mut budget = ArgumentBudgetV1::new(&mut work, 7);
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
                assert!(matches!(with_private_bf16_actual_output_formal_v1(
                    &output, &source, 64, 0, &mut budget, |_, _, _| panic!("denied before callback")),
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == prior));
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
    fn actual_output_guard_source_coordinates_and_duplicate_coverage_are_not_proofs() {
        for duplicate in [false, true] {
            let output = graph(true, 0);
            let source = if duplicate {
                graph(true, 0)
            } else {
                graph(false, 0)
            };
            let mut attempt = fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                &source,
                &source.kernels[0].id,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1],
                },
                FormalIndexWidth::Bits64,
            );
            if duplicate {
                let Ok(FormalMemoryObligationAnalysis::Incomplete { reasons, .. }) = &mut attempt
                else {
                    panic!("guarded fixture");
                };
                reasons.push(reasons[0].clone());
            }
            let mut work = Work::new(1_000_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(7).unwrap();
            assert!(
                with_private_bf16_ranked_formal_frame_v1(
                    &output,
                    &output.kernels[0],
                    [64, 1, 1],
                    64,
                    0,
                    &attempt,
                    &mut budget,
                    |_, proof, _| {
                        assert!(proof.is_err());
                        Ok(())
                    },
                )
                .is_err()
            );
            assert_eq!(budget.storage(), 7);
            assert!(budget.check_prior_denials_v1().is_ok());
        }
    }
}
