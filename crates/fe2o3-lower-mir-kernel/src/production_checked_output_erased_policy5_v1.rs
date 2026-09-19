/// Distinct original source/N and checked erased E, target B, unchanged C/S,
/// and load-forwarded actual O. E is never presented as original source/N.
/// Source/ranked/E/maps and complete checked history are caller-reserved;
/// B retains its separate real admission receipt, exactly as for Policy4.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1, ProductionCheckedOutputOwnerPolicy5V1};
/// fn relabel(x: ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1) -> ProductionCheckedOutputOwnerPolicy5V1 { x }
/// ```
pub struct ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: Owner,
    checked: Checked,
    kernels: Box<[FormalMemoryObligations]>,
}
impl fmt::Debug for ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1")
            .field(
                "original",
                self.original_source().executable().canonical().identity(),
            )
            .field("erased", self.erased().canonical().identity())
            .field("output", self.output().canonical().identity())
            .finish_non_exhaustive()
    }
}
impl ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1 {
    /// Consumes the exact original source/N plus checked erasure E, target B,
    /// and fixed checked B/C/S/O history. Replays the Policy4 prefix and actual-O
    /// obligations without substituting E for N. The caller reserves the source
    /// and checked-history floors plus separate B storage; scratch uses this
    /// ledger. Success grants neither protected-publication nor launch authority.
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
    /// Borrows the retained original semantic source and pre-erasure N.
    pub fn original_source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source.original_source()
    }
    /// Borrows the owning, independently replayable original-N-to-E relation.
    pub const fn erased_source(&self) -> &ProductionUnitLocalErasedSourceOwnerV1 {
        &self.source
    }
    /// Borrows checked E, the neutral target-binding input, never original N.
    pub fn erased(&self) -> &Owner {
        self.source.erased()
    }
    /// Borrows actual target-bound B derived from E.
    pub const fn bound(&self) -> &Owner {
        &self.bound
    }
    /// Borrows the complete checked Policy4 prefix and Policy5 continuation.
    pub const fn checked_output(&self) -> &Checked {
        &self.checked
    }
    /// Borrows actual final O after the independently checked S/O continuation.
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
    /// Returns the retained source/N/E/maps and checked-history floor, excluding
    /// separately reserved B. This logical receipt is not a heap/RSS measurement.
    pub fn retained_input_storage_floor_v1(&self) -> Result<usize, E> {
        self.source
            .retained_storage_floor_v1()
            .checked_add(self.checked.retained_storage())
            .ok_or_else(|| resource(AssertOriginResourceV1::Arithmetic))
    }
    /// Replays the exact source/N/E relation, B/C/S/O, and final memory obligations.
    /// Requires the retained input floor in the caller's live ledger; B remains
    /// separately reserved. Scratch preserves that ledger and its incoming floor.
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
pub(super) fn check_erased(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    bound: &Owner,
    checked: &Checked,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = (|| {
        drop(
            policy4::check_erased_policy4_v1(source, bound, checked.intermediate_policy4(), budget)
                .map_err(E::Prefix)?,
        );
        checked
            .replay_continuation(bound, budget)
            .map_err(E::Optimization)?;
        Ok(general::check_erased_forwarded_output_v1(
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
