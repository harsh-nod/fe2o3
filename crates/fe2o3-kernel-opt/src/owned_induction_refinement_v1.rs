//! Owning, unnumbered checked induction-add refinement; no source authority.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirGuardedUpdateV1 as Update, CanonicalKirInductionErrorV1 as FactsError,
    CanonicalKirInductionFactsV1 as Facts, CanonicalKirInductionOutcomeV1 as Outcome,
    CanonicalKirInductionRefinementErrorV1 as PairError,
    CanonicalKirInductionRefinementOriginV1 as Row,
    CanonicalKirInductionRefinementStorageV1 as PairStorage,
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, CheckedCanonicalKirInductionRefinementV1 as Pair,
    check_canonical_kir_induction_refinement_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrReplayStorageV12 as OutputStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    CheckedBinaryOperator, Constant, Module, Operation, OperationKind, ScalarType, Type, ValueDef,
    VerifiedCanonicalKernelIrIdentityV12 as Identity, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "induction_refinement_build_v1.rs"]
mod build;

/// Typed selection, ownership, fresh admission or independent-replay refusal.
#[derive(Debug)]
pub enum OwnedInductionRefinementErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Facts(FactsError),
    Admission(AdmissionError),
    Pair(PairError),
    /// Actual checked O+S exceeded the caller's unchanged operation cap.
    OutputLimit {
        actual: usize,
        limit: usize,
    },
    /// Replay must retain all seven original limits, even if others admit input.
    LimitsMismatch,
    ForeignInput,
    Recipe(&'static str),
    Panicked,
}
type Error = OwnedInductionRefinementErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<InventoryError> for Error {
    fn from(v: InventoryError) -> Self {
        Self::Inventory(v)
    }
}
impl From<LoopError> for Error {
    fn from(v: LoopError) -> Self {
        Self::Loops(v)
    }
}
impl From<FactsError> for Error {
    fn from(v: FactsError) -> Self {
        Self::Facts(v)
    }
}
impl From<AdmissionError> for Error {
    fn from(v: AdmissionError) -> Self {
        Self::Admission(v)
    }
}
impl From<PairError> for Error {
    fn from(v: PairError) -> Self {
        Self::Pair(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning induction-add refinement: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual admitted output and complete original-order one-to-many lineage.
/// Input identity alone is not source custody. A source pipeline must retain its
/// actual original owner and independently connect source/report obligations.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedInductionRefinementContinuationV1;
/// fn copy(v: &OwnedInductionRefinementContinuationV1)
///     -> OwnedInductionRefinementContinuationV1 { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedInductionRefinementContinuationV1;
/// fn mutate(v: &mut OwnedInductionRefinementContinuationV1) { v.origins.clear(); }
/// ```
pub struct OwnedInductionRefinementContinuationV1 {
    output: Owner,
    output_storage: OutputStorage,
    input_identity: Identity,
    origins: Vec<Row>,
    limits: Limits,
    retained: usize,
}
impl OwnedInductionRefinementContinuationV1 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    pub fn origins(&self) -> &[Row] {
        &self.origins
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    /// Unreserved new output/header/actual origin-capacity addition.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Independently replays actual endpoints, complete origins and exact limits.
    /// Caller keeps both input and owning receipt live in the current ledger.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(7)?;
            if limits != self.limits {
                return Err(Error::LimitsMismatch);
            }
            meter.work(
                size_of::<Identity>()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained(self.output_storage, &self.origins)? {
                return Err(Resource::Accounting.into());
            }
            if input.canonical().identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|b| {
                Ok(check_canonical_kir_induction_refinement_v1(
                    input,
                    &self.output,
                    &self.origins,
                    limits,
                    b,
                )?)
            })
        })
    }
}

/// Replaces only proved nonwrapping unsigned CheckedAdd updates by adjacent
/// same-ID Add/false operations. Keeps all consumers, effects, CFG and metadata.
/// Fresh facts and actual-pair checking are mandatory; no source-name dispatch.
/// Extra layer work is O(F+B+R+O+wire), scratch O(O) beyond inherited analyses.
/// Checked O+S and u32 coordinates precede candidate copy or mutation. Candidate,
/// old/new operation/result backing, actual capacities and new Vec headers remain
/// live through use. Success returns one unreserved addition; failure/unwind
/// drops partial values before same-ledger floor cleanup and never refunds work.
pub fn prepare_owned_induction_refinement_v1(
    input: &Owner,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<OwnedInductionRefinementContinuationV1> {
    resources::scoped(budget, |meter| {
        meter.reserve(header()?)?;
        let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
        meter.reserve(is.retained_storage())?;
        let (loops, ls) = meter.derive(|b| Ok(Loops::derive(&inventory, limits, b)?))?;
        meter.reserve(ls.retained_storage())?;
        let (facts, fs) = meter.derive(|b| Ok(Facts::derive(&loops, limits, b)?))?;
        meter.reserve(fs.retained_storage())?;
        meter.derive(|b| Ok(facts.replay(&loops, limits, b)?))?;
        let origins = build::plan(&inventory, &facts, limits, meter)?;
        let (mut candidate, cs) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        meter.reserve(cs.retained_storage())?;
        let extra = build::materialize(&inventory, &origins, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let ps = {
            let (_pair, ps) = meter.derive(|b| {
                Ok(check_canonical_kir_induction_refinement_v1(
                    input, &output, &origins, limits, b,
                )?)
            })?;
            meter.reserve(ps.retained_storage())?;
            ps
        };
        meter.release(ps.retained_storage())?;
        let retained = retained(output_storage, &origins)?;
        drop(candidate);
        meter.release(
            cs.retained_storage()
                .checked_add(extra)
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(facts);
        meter.release(fs.retained_storage())?;
        drop(loops);
        meter.release(ls.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        meter.work(1)?;
        Ok(OwnedInductionRefinementContinuationV1 {
            output,
            output_storage,
            input_identity: *input.canonical().identity(),
            origins,
            limits,
            retained,
        })
    })
}

fn header() -> Result<usize> {
    size_of::<OwnedInductionRefinementContinuationV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(output: OutputStorage, rows: &Vec<Row>) -> Result<usize> {
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}

#[cfg(test)]
#[path = "owned_induction_refinement_v1_tests.rs"]
mod tests;
