//! Additive final-I admission after the unchanged, source-qualified Policy5 O.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 as Checked;

include!("production_checked_output_erased_policy6_v1.rs");

/// Refusal while joining original source custody to fixed Policy6 actual I.
#[derive(Debug)]
pub enum ProductionCheckedOutputAdmissionErrorPolicy6V1 {
    /// Source, resources, or freshly derived final-I safety failed.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// The complete unchanged source/B/C/S/O prefix was not qualified.
    Prefix(ProductionCheckedOutputAdmissionErrorPolicy5V1),
    /// The independently checked O/I continuation or sealed execution failed.
    Optimization(fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1),
}
type E = ProductionCheckedOutputAdmissionErrorPolicy6V1;
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(f),
            Self::Prefix(error) => error.fmt(f),
            Self::Optimization(error) => error.fmt(f),
        }
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Prefix(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}
impl From<ProductionCheckedOutputAdmissionErrorPolicy3V1> for E {
    fn from(error: ProductionCheckedOutputAdmissionErrorPolicy3V1) -> Self {
        Self::Admission(error)
    }
}
fn resource(error: AssertOriginResourceV1) -> E {
    E::Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
        error,
    ))
}

/// Move-only original source/N, actual bound B, unchanged Policy5 history and I.
/// Final reports describe I; no O-derived artifact or report is substituted.
/// This owner does not select a default or grant artifact/publication authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1;
/// fn duplicate(owner: ProductionCheckedOutputOwnerPolicy6V1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedOutputOwnerPolicy5V1, ProductionCheckedOutputOwnerPolicy6V1};
/// fn relabel(owner: ProductionCheckedOutputOwnerPolicy6V1) -> ProductionCheckedOutputOwnerPolicy5V1 { owner }
/// ```
pub struct ProductionCheckedOutputOwnerPolicy6V1 {
    source: ProductionSemanticKirOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
    source_storage_floor: usize,
}
impl fmt::Debug for ProductionCheckedOutputOwnerPolicy6V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCheckedOutputOwnerPolicy6V1")
            .field("output", self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}
impl ProductionCheckedOutputOwnerPolicy6V1 {
    /// Consumes the genuine source/ranked receipt, B, and completed Policy6.
    /// The caller must already reserve source, B, and the entire transferred
    /// Policy6 receipt, not just its consumed Policy5 prefix. Scratch preserves
    /// that floor. No compilation algorithm or native artifact is rerun here.
    pub fn try_admit_v1(
        receipt: ProductionMaterializedRankedModuleReceiptV1,
        bound: Owner,
        checked: Checked,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, E> {
        use ProductionCheckedOutputAdmissionErrorPolicy3V1 as P3;
        output_admission_charge_v1(budget, 8)?;
        let source_storage_floor = receipt
            .materialized
            .unit_local_source_storage_floor_v1()
            .map_err(P3::Source)?;
        require_floor(source_storage_floor, &checked, budget)?;
        receipt
            .materialized
            .require_legacy_helper_policy_v1("Policy6 output admission")
            .map_err(P3::Source)?;
        validate_source_ranked_roster_v1(
            &receipt.materialized.semantic_ssa,
            &receipt.materialized.source_launch,
            &receipt.roots,
        )
        .map_err(P3::Source)?;
        output_admission_charge_v1(budget, 2)?;
        if receipt.roots.is_empty()
            || receipt.roots.len() != receipt.materialized.executable().module().kernels.len()
        {
            return Err(
                output_admission_unsupported_v1("ranked", "complete nonempty root roster").into(),
            );
        }
        let source =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
                receipt, budget,
            )
            .map_err(P3::Source)?;
        let kernels = check(&source, &bound, &checked, budget)?;
        Ok(Self {
            source,
            bound,
            checked,
            kernels,
            source_storage_floor,
        })
    }
    /// Original semantic source/N and genuine ranked correspondence.
    pub const fn source_semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.source
    }
    /// Exact retained target-bound B, before every neutral optimization.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// Complete unchanged Policy5 prefix and separate checked O/I history.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// Actual final I, never Policy5's historical O.
    pub fn output(&self) -> &Owner {
        self.checked.owner()
    }
    /// Fresh final-I formal memory obligations.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// This local-rule owner grants neither publication nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Retained source/history floor, excluding the separately reserved B.
    /// Inherited source/ranked/formal allocation domains match Policy5.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source_storage_floor
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }
    /// Replay source through O, the actual O/I relation, and fresh I obligations.
    /// Caller reservations and the original work ledger are preserved.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), E> {
        output_admission_charge_v1(budget, 8)?;
        require_floor(self.source_storage_floor, &self.checked, budget)?;
        if check(&self.source, &self.bound, &self.checked, budget)? != self.kernels {
            return Err(formal_mismatch());
        }
        Ok(())
    }
}

fn require_floor(
    source: usize,
    checked: &Checked,
    budget: &AssertOriginBudgetV1<'_>,
) -> Result<(), E> {
    let floor = source
        .checked_add(checked.retained_storage())
        .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))?;
    if budget.storage() < floor {
        return Err(resource(AssertOriginResourceV1::Accounting));
    }
    Ok(())
}
fn formal_mismatch() -> E {
    E::Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1::Formal(
        crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
    ))
}
fn check(
    source: &ProductionSemanticKirOwnerV1,
    bound: &Owner,
    checked: &Checked,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        drop(
            policy5::check(source, bound, checked.intermediate_policy5(), budget)
                .map_err(E::Prefix)?,
        );
        checked
            .replay_continuation(bound, budget)
            .map_err(E::Optimization)?;
        Ok(general::check_integer_continued_output_v1(
            source, checked, budget,
        )?)
    })();
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err(resource(AssertOriginResourceV1::Accounting));
    }
    result
}
