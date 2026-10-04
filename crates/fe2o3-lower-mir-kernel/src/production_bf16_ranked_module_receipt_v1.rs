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
