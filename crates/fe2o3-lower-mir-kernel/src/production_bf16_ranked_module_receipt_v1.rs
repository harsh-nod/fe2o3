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
