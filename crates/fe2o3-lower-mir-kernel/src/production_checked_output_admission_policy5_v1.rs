//! Additive fixed Policy5 final admission. Policy4's complete source/B/C/S
//! relation is reused unchanged; S/O is separately checked, never an origin map.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy5V1 as Checked;

include!("production_checked_output_erased_policy5_v1.rs");

/// Refusal while connecting original source custody to fixed Policy5 output.
#[derive(Debug)]
pub enum ProductionCheckedOutputAdmissionErrorPolicy5V1 {
    /// Source, resource, or final-output obligations could not be established.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// The unchanged Policy4 source/B/C/S prefix was not admitted.
    Prefix(ProductionCheckedOutputAdmissionErrorPolicy4V1),
    /// Independent replay rejected the checked S/O load-forwarding continuation.
    Optimization(fe2o3_kernel_opt::CanonicalPolicy5OptimizationErrorV1),
}
type E = ProductionCheckedOutputAdmissionErrorPolicy5V1;
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(e) => e.fmt(f),
            Self::Prefix(e) => e.fmt(f),
            Self::Optimization(e) => e.fmt(f),
        }
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Prefix(e) => Some(e),
            Self::Optimization(e) => Some(e),
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

/// Move-only original source/N, target B, unchanged C/S and checked actual O.
/// The first Load remains live; final private admission independently verifies
/// initialized valid reads. This is neither a default policy selection nor an
/// artifact/publication/launch authority. B is separately caller-reserved.
/// Inherited source/ranked/formal allocation domains are unchanged from Policy4.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedOutputOwnerPolicy4V1, ProductionCheckedOutputOwnerPolicy5V1};
/// fn relabel(x: ProductionCheckedOutputOwnerPolicy5V1) -> ProductionCheckedOutputOwnerPolicy4V1 { x }
/// ```
pub struct ProductionCheckedOutputOwnerPolicy5V1 {
    source: ProductionSemanticKirOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
    source_storage_floor: usize,
}
impl fmt::Debug for ProductionCheckedOutputOwnerPolicy5V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCheckedOutputOwnerPolicy5V1")
            .field("output", self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}
impl ProductionCheckedOutputOwnerPolicy5V1 {
    /// Consumes the exact source/ranked receipt, target-bound B, and fixed
    /// checked B/C/S/O history, replaying the prefix and actual-O obligations.
    /// The caller reserves the source/history floor and the separate B receipt;
    /// validation scratch uses this same ledger. Admission grants no artifact,
    /// protected-publication, or launch authority and does not select a default.
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
            .require_legacy_helper_policy_v1("Policy5 output admission")
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
    /// Borrows the retained original semantic source/N and ranked correspondence.
    pub const fn source_semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.source
    }
    /// Borrows the actual target-bound input B, not a reconstructed substitute.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// Borrows the complete checked Policy4 prefix and Policy5 continuation.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// Borrows actual final O, the endpoint of the independently checked history.
    pub fn output(&self) -> &Owner {
        self.checked.owner()
    }
    /// Borrows the freshly derived final-O formal memory obligations.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// Returns false; this owner cannot authorize publication or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Complete source and checked B/C/S/O history, excluding separately reserved B.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source_storage_floor
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }
    /// Replays original source/N, B/C/S/O, and exact final memory obligations.
    /// Requires the retained input floor in the caller's live ledger; B remains
    /// separately reserved. Scratch preserves that ledger and its incoming floor.
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
pub(super) fn check(
    source: &ProductionSemanticKirOwnerV1,
    bound: &Owner,
    checked: &Checked,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        // The old final S must be qualified before its preserved trap/control
        // facts may be used by the coordinate-preserving S/O continuation.
        drop(
            policy4::check(source, bound, checked.intermediate_policy4(), budget)
                .map_err(E::Prefix)?,
        );
        checked
            .replay_continuation(bound, budget)
            .map_err(E::Optimization)?;
        Ok(general::check_forwarded_output_v1(
            source,
            checked.intermediate_policy4().owner(),
            checked.owner(),
            budget,
        )?)
    })();
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err(resource(AssertOriginResourceV1::Accounting));
    }
    result
}
