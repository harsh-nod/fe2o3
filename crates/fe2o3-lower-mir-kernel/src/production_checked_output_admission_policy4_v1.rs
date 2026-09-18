use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1 as Checked;

include!("production_checked_output_erased_policy4_v1.rs");

/// Failure to compose source/B/C admission with independently checked C/O.
#[derive(Debug)]
pub enum ProductionCheckedOutputAdmissionErrorPolicy4V1 {
    /// Source, intermediate, resource or fresh final-output admission failed.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// The fixed Policy4 composition did not independently replay.
    Optimization(fe2o3_kernel_opt::CanonicalPolicy4OptimizationErrorV1),
}
impl fmt::Display for ProductionCheckedOutputAdmissionErrorPolicy4V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(e) => e.fmt(f),
            Self::Optimization(e) => e.fmt(f),
        }
    }
}
impl Error for ProductionCheckedOutputAdmissionErrorPolicy4V1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Optimization(e) => Some(e),
        }
    }
}
type E = ProductionCheckedOutputAdmissionErrorPolicy4V1;
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

/// Consumed source/ranked custody, exact B, qualified C, and freshly checked O.
/// The intermediate Policy3 occurrence map describes B/C only. Its source
/// relation is composed with the independent coordinate-preserving C/O rule,
/// never relabeled as a direct B/O map. Fresh final obligations describe O.
///
/// Private allocation/read safety uses the separate exact private census; the
/// global formal report is not a proof about accesses it omits. Runtime bounds,
/// aliases, protected proof provenance and launch admission remain separate.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1;
/// fn clone_owner(owner: ProductionCheckedOutputOwnerPolicy4V1) { let _ = owner.clone(); }
/// ```
pub struct ProductionCheckedOutputOwnerPolicy4V1 {
    source: ProductionSemanticKirOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
    source_storage_floor: usize,
}
impl fmt::Debug for ProductionCheckedOutputOwnerPolicy4V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCheckedOutputOwnerPolicy4V1")
            .field("output", self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}
impl ProductionCheckedOutputOwnerPolicy4V1 {
    /// Consumes actual ranked source, B and sealed fixed-Policy4 custody.
    /// The caller reserves source/capture, B and the complete checked owner.
    /// Canonical scratch restores the entry floor; inherited source/ranked/formal
    /// engines retain their separate bounded accounting domain, as for Policy3.
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
        let minimum = source_storage_floor
            .checked_add(checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(resource(AssertOriginResourceV1::Accounting));
        }
        receipt
            .materialized
            .require_legacy_helper_policy_v1("Policy4 output admission")
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
        let source = ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
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

    /// Historical source and N, never the final executable accessor.
    pub const fn source_semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.source
    }
    /// Exact target-bound input B.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// The actual executed fixed composition, including its retained C.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// The actual final O, not intermediate C.
    pub fn output(&self) -> &Owner {
        self.checked.owner()
    }
    /// Fresh final global-memory obligations in exact root order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// This stage cannot publish, load or launch artifacts.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Required retained source/composition storage, excluding caller-reserved B.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source_storage_floor
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }
    /// Independently repeats source/B/C, C/O and fresh O admission.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), E> {
        output_admission_charge_v1(budget, 8)?;
        if budget.storage() < self.retained_input_storage_floor_v1()? {
            return Err(resource(AssertOriginResourceV1::Accounting));
        }
        if check(&self.source, &self.bound, &self.checked, budget)? != self.kernels {
            return Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
            )
            .into());
        }
        Ok(())
    }
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
        drop(general::check_general_output_v1(
            source,
            bound,
            checked.intermediate_policy3(),
            budget,
        )?);
        checked.replay(bound, budget).map_err(E::Optimization)?;
        Ok(general::check_forwarded_output_v1(
            source,
            checked.intermediate_policy3().owner(),
            checked.owner(),
            budget,
        )?)
    })();
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err(resource(AssertOriginResourceV1::Accounting));
    }
    result
}
