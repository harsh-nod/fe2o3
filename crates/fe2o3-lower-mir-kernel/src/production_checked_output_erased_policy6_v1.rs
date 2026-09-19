/// Move-only original N, distinct erased E, actual B, Policy5 history and I.
/// E is not relabeled as source N, nor is I relabeled as old Policy5 O.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1, ProductionCheckedOutputOwnerPolicy6V1};
/// fn relabel(owner: ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1) -> ProductionCheckedOutputOwnerPolicy6V1 { owner }
/// ```
pub struct ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
}
impl fmt::Debug for ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1")
            .field(
                "original",
                self.original_source().executable().canonical().identity(),
            )
            .field("erased", self.erased().canonical().identity())
            .field("output", self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}
impl ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    /// Consumes the genuine N/E source relation, B, and completed Policy6.
    /// Reserve the source, separate B, and complete Policy6 transfer first.
    /// Prefix source admission is replayed before independently checking I.
    pub fn try_admit_v1(
        source: ProductionUnitLocalErasedSourceOwnerV1,
        bound: Owner,
        checked: Checked,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, E> {
        output_admission_charge_v1(budget, 8)?;
        require_floor(source.retained_storage_floor_v1(), &checked, budget)?;
        let kernels = check_erased(&source, &bound, &checked, budget)?;
        Ok(Self {
            source,
            bound,
            checked,
            kernels,
        })
    }
    /// Original semantic source and pre-erasure N.
    pub fn original_source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source.original_source()
    }
    /// Owning original-N-to-E relation.
    pub const fn erased_source(&self) -> &ProductionUnitLocalErasedSourceOwnerV1 {
        &self.source
    }
    /// Distinct checked E, never original N.
    pub fn erased(&self) -> &Owner {
        self.source.erased()
    }
    /// Exact target-bound B derived from E.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// Full retained Policy5 prefix and actual O/I continuation.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// Actual final I.
    pub fn output(&self) -> &Owner {
        self.checked.owner()
    }
    /// Fresh I memory obligations, not cached O obligations.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
    /// This owner supplies no protected-publication or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Source/N/E/history logical floor, excluding separately reserved B.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source
            .retained_storage_floor_v1()
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }
    /// Replay genuine N/E, source/B/C/S/O, O/I and fresh final-I safety.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), E> {
        output_admission_charge_v1(budget, 8)?;
        require_floor(
            self.source.retained_storage_floor_v1(),
            &self.checked,
            budget,
        )?;
        if check_erased(&self.source, &self.bound, &self.checked, budget)? != self.kernels {
            return Err(formal_mismatch());
        }
        Ok(())
    }
}
fn check_erased(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    bound: &Owner,
    checked: &Checked,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        drop(
            policy5::check_erased(source, bound, checked.intermediate_policy5(), budget)
                .map_err(|error| E::Prefix(Box::new(error)))?,
        );
        checked
            .replay_continuation(bound, budget)
            .map_err(E::Optimization)?;
        Ok(general::check_erased_integer_continued_output_v1(
            source, checked, budget,
        )?)
    })();
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err(resource(AssertOriginResourceV1::Accounting));
    }
    result
}
