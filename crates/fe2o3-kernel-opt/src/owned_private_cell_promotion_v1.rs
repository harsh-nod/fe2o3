//! Closed owning private-cell transaction. No fixed policy or source/native gate.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirPrivateCellAccessKindV1 as AccessKind,
    CanonicalKirPrivateCellCensusErrorV1 as CensusError,
    CanonicalKirPrivateCellCensusLimitsV1 as Limits, CanonicalKirPrivateCellCensusV1 as Census,
    CanonicalKirPrivateCellOriginKindV1 as OriginKind, CanonicalKirPrivateCellOriginV1 as Origin,
    CanonicalKirPrivateCellPromotionErrorV1 as PairError,
    CanonicalKirPrivateCellPromotionStorageV1 as PairStorage,
    CheckedCanonicalKirPrivateCellPromotionV1 as Pair,
    check_canonical_kir_private_cell_promotion_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrReplayStorageV12 as OutputStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, Module, Operation, OperationKind as Kind,
    ValueId, VerifiedCanonicalKernelIrIdentityV12 as Identity,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "private_cell_promotion_build_v1.rs"]
mod build;

/// Failure of the closed transaction; no partial candidate or owner escapes.
#[derive(Debug)]
pub enum OwnedPrivateCellPromotionErrorV1 {
    /// Cumulative work/storage or accounting denial.
    Resource(Resource),
    /// Actual input inventory denial.
    Inventory(InventoryError),
    /// Fresh full-use census denial.
    Census(CensusError),
    /// Candidate-copy or fresh-output admission denial.
    Admission(AdmissionError),
    /// Mandatory independent actual-pair check failed.
    Pair(PairError),
    /// Private prepared recipe invariant failed.
    Recipe(&'static str),
    /// Typed input identity differs; matching identities still require replay.
    ForeignInput,
    /// Panic caught after discarding the partial transaction.
    Panicked,
}
type Error = OwnedPrivateCellPromotionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Meter<'_, 'w>) -> Result<T>,
) -> Result<T> {
    resources::scoped(budget, run)
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<InventoryError> for Error {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}
impl From<CensusError> for Error {
    fn from(value: CensusError) -> Self {
        Self::Census(value)
    }
}
impl From<AdmissionError> for Error {
    fn from(value: AdmissionError) -> Self {
        Self::Admission(value)
    }
}
impl From<PairError> for Error {
    fn from(value: PairError) -> Self {
        Self::Pair(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning private-cell promotion: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual final output and complete inert lineage, without an input borrow.
/// Only the closed factory constructs this value; there is no raw attachment,
/// mutable output, cloning, signed proof or fixed execution-record constructor.
/// The caller must retain its genuine source/prefix separately for later source
/// admission. This local canonical relation does not prove allocation/resource
/// refinement, Rust lifetime/Drop/layout or native behavior.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedPrivateCellPromotionContinuationV1;
/// fn duplicate(value: &OwnedPrivateCellPromotionContinuationV1)
///     -> OwnedPrivateCellPromotionContinuationV1 { value.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedPrivateCellPromotionContinuationV1;
/// fn detach(value: OwnedPrivateCellPromotionContinuationV1) { let _ = value.output; }
/// ```
/// ```
/// use fe2o3_kernel_opt::{prepare_owned_private_cell_promotion_v1,
///     OwnedPrivateCellPromotionContinuationV1, OwnedPrivateCellPromotionErrorV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn no_self_reference(input: Owner, budget: &mut Budget<'_>)
///     -> Result<OwnedPrivateCellPromotionContinuationV1, OwnedPrivateCellPromotionErrorV1> {
///     let output = prepare_owned_private_cell_promotion_v1(&input, budget)?;
///     drop(input);
///     Ok(output)
/// }
/// ```
pub struct OwnedPrivateCellPromotionContinuationV1 {
    output: Owner,
    output_storage: OutputStorage,
    input_identity: Identity,
    selected: Vec<Coordinate>,
    origins: Vec<Origin>,
    retained: usize,
}
impl OwnedPrivateCellPromotionContinuationV1 {
    /// Actual freshly admitted final graph.
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    /// Exact original canonical digest and length, not an execution proof.
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    /// All original census-eligible allocations selected by the factory.
    pub fn selected_allocations(&self) -> &[Coordinate] {
        &self.selected
    }
    /// One original-to-final row for every actual output operation.
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }
    /// Full unreserved receipt transferred from the completed transaction.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// No source, native, publication, launch or fixed-policy authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Requires the owning receipt already reserved. The exact receipt and full
    /// typed identity are checked before mandatory independent actual-pair replay.
    /// The returned borrowed witness receipt is unreserved. Matching identities
    /// never replace replay; independently admitted equal bytes are permitted.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |meter| {
            meter.work(
                size_of::<Identity>()
                    .checked_add(4)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained(self.output_storage, &self.selected, &self.origins)? {
                return Err(Resource::Accounting.into());
            }
            if input.canonical().identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|budget| {
                Ok(check_canonical_kir_private_cell_promotion_v1(
                    input,
                    &self.output,
                    &self.selected,
                    &self.origins,
                    Limits::default(),
                    budget,
                )?)
            })
        })
    }
}

/// Selects all eligible allocations in one original census, performs one private
/// stable mutation sweep, freshly admits the edited candidate, and independently
/// checks the full actual pair before returning. It is not a fixpoint or DCE.
/// Unsupported allocations remain unchanged; all value/trap producers remain.
///
/// The immutable input and its reservation stay caller-owned. Candidate copy,
/// analysis, row capacities, fresh admission and replay coexist on one live
/// ledger. Success restores entry storage and transfers the complete returned
/// owning receipt unreserved; reserve it before further metered work. Failure
/// discards all owned candidates before permitted cleanup. Work/peak/denial
/// history persist. Logical inherited receipts and actual new capacities are
/// not allocator/RSS bounds. No caller callback, selector or raw output enters.
pub fn prepare_owned_private_cell_promotion_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<OwnedPrivateCellPromotionContinuationV1> {
    scoped(budget, |meter| {
        meter.work(1)?;
        meter.reserve(header()?)?;
        let (inventory, is) = meter.derive(|budget| Ok(Inventory::derive(input, budget)?))?;
        meter.reserve(is.retained_storage())?;
        let (census, cs) =
            meter.derive(|budget| Ok(Census::derive(&inventory, Limits::default(), budget)?))?;
        meter.reserve(cs.retained_storage())?;
        let recipe = build::Recipe::prepare(&inventory, &census, meter)?;
        let (mut candidate, candidate_size) =
            meter.derive(|budget| Ok(input.copy_module_for_transformation_v12(budget)?))?;
        meter.reserve(candidate_size.retained_storage())?;
        let (selected, origins) = recipe.apply(&mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|budget| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, budget,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let relation_size = {
            let (_relation, receipt) = meter.derive(|budget| {
                Ok(check_canonical_kir_private_cell_promotion_v1(
                    input,
                    &output,
                    &selected,
                    &origins,
                    Limits::default(),
                    budget,
                )?)
            })?;
            meter.reserve(receipt.retained_storage())?;
            receipt
        };
        meter.release(relation_size.retained_storage())?;
        let retained = retained(output_storage, &selected, &origins)?;
        // Candidate capacity never shrinks or receives early deletion credits.
        drop(candidate);
        meter.release(candidate_size.retained_storage())?;
        drop(census);
        meter.release(cs.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        Ok(OwnedPrivateCellPromotionContinuationV1 {
            output,
            output_storage,
            input_identity: *input.canonical().identity(),
            selected,
            origins,
            retained,
        })
    })
}

fn header() -> Result<usize> {
    size_of::<OwnedPrivateCellPromotionContinuationV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn capacity<T>(rows: &Vec<T>) -> Result<usize> {
    rows.capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(
    output: OutputStorage,
    selected: &Vec<Coordinate>,
    origins: &Vec<Origin>,
) -> Result<usize> {
    let selected_bytes = capacity(selected)?;
    let origin_bytes = capacity(origins)?;
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(selected_bytes))
        .and_then(|n| n.checked_add(origin_bytes))
        .ok_or_else(|| Resource::Arithmetic.into())
}

#[cfg(test)]
#[path = "owned_private_cell_promotion_v1_tests.rs"]
mod tests;
