// Additive final source/N/E/B/C/O custody. The historical direct owner and its
// public signatures remain unchanged; E is never presented as original N.

/// Complete erased-source, exact E/B, checked B/C and independently replayed
/// C/O custody with fresh actual-O obligations. No final execution is selected
/// from an inert map or from the original source/N accessor.
///
/// The caller keeps the complete source and checked receipts reserved. B's
/// actual replay receipt remains separately caller-reserved, as for the direct
/// Policy4 owner. Existing wrapper/formal engine allocations retain their
/// inherited bounded domain; new canonical scratch restores the incoming floor.
/// This grants neither protected artifact publication nor runtime launch rights.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1, ProductionCheckedOutputOwnerPolicy4V1};
/// fn substitute(owner: ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1) -> ProductionCheckedOutputOwnerPolicy4V1 { owner }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1;
/// fn duplicate(owner: ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1) { let first = owner; let second = owner; drop((first, second)); }
/// ```
pub struct ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
}

impl fmt::Debug for ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1")
            .field(
                "original",
                &self.original_source().executable().canonical().identity(),
            )
            .field("erased", &self.erased().canonical().identity())
            .field("output", &self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}

impl ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 {
    /// Admits the complete source-to-output chain under the fixed Policy4 checks.
    pub fn try_admit_v1(
        source: ProductionUnitLocalErasedSourceOwnerV1,
        bound: Owner,
        checked: Checked,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, E> {
        output_admission_charge_v1(budget, 8)?;
        let minimum = source
            .retained_storage_floor_v1()
            .checked_add(checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(resource(AssertOriginResourceV1::Accounting));
        }
        let kernels = check_erased_policy4_v1(&source, &bound, &checked, budget)?;
        Ok(Self {
            source,
            bound,
            checked,
            kernels,
        })
    }

    /// Original source and neutral graph before checked silent-helper deletion.
    pub fn original_source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source.original_source()
    }
    /// Owning checked relation between original N and erased E.
    pub const fn erased_source(&self) -> &ProductionUnitLocalErasedSourceOwnerV1 {
        &self.source
    }
    /// Exact neutral E after checked silent-helper deletion.
    pub fn erased(&self) -> &Owner {
        self.source.erased()
    }
    /// Exact target-bound B derived from E.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// Sealed optimizer history retaining intermediate C and actual output O.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// Actual optimized O admitted by the final output checks.
    pub fn output(&self) -> &Owner {
        self.checked.owner()
    }
    /// Fresh memory obligations derived from actual O.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// These compiler checks alone do not grant artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Complete original/ranked/E/maps plus checked C/O reservation. This
    /// intentionally excludes separately caller-reserved B, like the direct
    /// Policy4 owner; no numeric receipt establishes B allocation custody.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source
            .retained_storage_floor_v1()
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }

    /// Freshly composes original source/N/E, exact E/B, B/C and C/O and actual O.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), E> {
        output_admission_charge_v1(budget, 8)?;
        if budget.storage() < self.retained_input_storage_floor_v1()? {
            return Err(resource(AssertOriginResourceV1::Accounting));
        }
        if check_erased_policy4_v1(&self.source, &self.bound, &self.checked, budget)?
            != self.kernels
        {
            return Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
            )
            .into());
        }
        Ok(())
    }
}

fn check_erased_policy4_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    bound: &Owner,
    checked: &Checked,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        drop(general::check_erased_general_output_v1(
            source,
            bound,
            checked.intermediate_policy3(),
            budget,
        )?);
        checked.replay(bound, budget).map_err(E::Optimization)?;
        Ok(general::check_erased_forwarded_output_v1(
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
