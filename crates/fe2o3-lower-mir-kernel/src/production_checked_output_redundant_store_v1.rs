//! Borrowed source/final safety for a separately checked private Store deletion.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as StoreOwner;
use fe2o3_kernel_opt::CheckedRedundantStoreOutputV1 as StoreOutput;
#[path = "production_checked_output_redundant_store_view_v1.rs"]
mod view;
use view::StoreDeletionView;

/// Failure to connect a checked private Store deletion to its retained source.
#[derive(Debug)]
pub enum ProductionRedundantStoreAdmissionErrorV1 {
    /// Work, storage, allocation or accounting contract failure.
    Resource(AssertOriginResourceV1),
    /// The complete source-owned Policy6 prefix failed independent replay.
    Prefix(Box<ProductionCheckedOutputAdmissionErrorPolicy6V1>),
    /// The actual input/output Store-deletion relation failed independent replay.
    Deletion(fe2o3_kernel_opt::CheckedRedundantStoreErrorV1),
    /// Source correspondence or fresh output safety obligations were not met.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// A panic discarded the candidate without issuing an admission witness.
    Panicked,
}
type StoreError = ProductionRedundantStoreAdmissionErrorV1;
type StoreResult<T> = Result<T, StoreError>;
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned redundant private Store: {self:?}")
    }
}
impl Error for StoreError {}
impl From<AssertOriginResourceV1> for StoreError {
    fn from(error: AssertOriginResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<ProductionCheckedOutputAdmissionErrorPolicy3V1> for StoreError {
    fn from(error: ProductionCheckedOutputAdmissionErrorPolicy3V1) -> Self {
        Self::Admission(error)
    }
}

#[derive(Clone, Copy)]
enum StorePrefix<'a> {
    Direct(&'a ProductionCheckedOutputOwnerPolicy6V1),
    Erased(&'a ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1),
}
impl<'a> StorePrefix<'a> {
    fn output(self) -> &'a StoreOwner {
        match self {
            Self::Direct(p) => p.output(),
            Self::Erased(p) => p.output(),
        }
    }
    fn source(self) -> GeneralSourceContextV1<'a> {
        match self {
            Self::Direct(p) => GeneralSourceContextV1::Direct(p.source_semantic_kir()),
            Self::Erased(p) => GeneralSourceContextV1::Erased(p.erased_source()),
        }
    }
    fn bound(self) -> &'a StoreOwner {
        match self {
            Self::Direct(p) => p.bound(),
            Self::Erased(p) => p.bound(),
        }
    }
    fn checked(self) -> &'a fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 {
        match self {
            Self::Direct(p) => p.checked_output(),
            Self::Erased(p) => p.checked_output(),
        }
    }
    fn floor(self) -> StoreResult<usize> {
        match self {
            Self::Direct(p) => p.retained_input_storage_floor_v1(),
            Self::Erased(p) => p.retained_input_storage_floor_v1(),
        }
        .map_err(|e| StoreError::Prefix(Box::new(e)))
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> StoreResult<()> {
        match self {
            Self::Direct(p) => p.verify_equivalence(budget),
            Self::Erased(p) => p.verify_equivalence(budget),
        }
        .map_err(|e| StoreError::Prefix(Box::new(e)))
    }
}

/// Borrowed checked source-to-J relation. Neither owner is cloned or relabeled.
/// Fresh J formal obligations are checked on each replay, not retained as I
/// reports or exposed as publication evidence. Source/ranked/formal engine
/// allocation exclusions are inherited from the existing Policy6 admission.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionRedundantStoreAdmissionV1;
/// fn detach<'a>(value: ProductionRedundantStoreAdmissionV1<'a>)
///     -> ProductionRedundantStoreAdmissionV1<'static> { value }
/// ```
pub struct ProductionRedundantStoreAdmissionV1<'a> {
    prefix: StorePrefix<'a>,
    deletion: &'a StoreOutput<'a>,
}

/// Additional logical storage for the borrowed witness, excluding both owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRedundantStoreAdmissionStorageV1(usize);
impl ProductionRedundantStoreAdmissionStorageV1 {
    /// Bytes transferred unreserved on successful admission.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl ProductionCheckedOutputOwnerPolicy6V1 {
    /// Both owners and the separately reserved B stay caller-owned. Reserve
    /// the returned witness receipt before later controlled allocations.
    pub fn check_redundant_store_output_v1<'a>(
        &'a self,
        deletion: &'a StoreOutput<'a>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> StoreResult<(
        ProductionRedundantStoreAdmissionV1<'a>,
        ProductionRedundantStoreAdmissionStorageV1,
    )> {
        admit_store_output(StorePrefix::Direct(self), deletion, budget)
    }
}
impl ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    /// Replays original N/E custody; E is never substituted for original N.
    pub fn check_redundant_store_output_v1<'a>(
        &'a self,
        deletion: &'a StoreOutput<'a>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> StoreResult<(
        ProductionRedundantStoreAdmissionV1<'a>,
        ProductionRedundantStoreAdmissionStorageV1,
    )> {
        admit_store_output(StorePrefix::Erased(self), deletion, budget)
    }
}
impl ProductionRedundantStoreAdmissionV1<'_> {
    /// Actual retained Policy6 input graph I.
    pub fn input(&self) -> &StoreOwner {
        self.prefix.output()
    }
    /// Actual checked graph J after the separately certified Store deletion.
    pub fn output(&self) -> &StoreOwner {
        self.deletion.output()
    }
    /// This borrowed relation never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Replays the complete prefix, deletion, source lifetimes and fresh J safety.
    /// Both owners, the witness and the separately reserved B must remain prepaid.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> StoreResult<()> {
        let inherited = required_store_floor(self.prefix, self.deletion)?;
        let required = inherited
            .checked_add(std::mem::size_of::<Self>())
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        store_scope(required, budget, |budget| {
            check_store_output(
                self.prefix,
                StoreDeletionView::Borrowed(self.deletion),
                budget,
            )
            .map(drop)
        })
    }
}

fn required_store_floor(prefix: StorePrefix<'_>, deletion: &StoreOutput<'_>) -> StoreResult<usize> {
    prefix
        .floor()?
        .checked_add(deletion.retained_storage())
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn admit_store_output<'a>(
    prefix: StorePrefix<'a>,
    deletion: &'a StoreOutput<'a>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> StoreResult<(
    ProductionRedundantStoreAdmissionV1<'a>,
    ProductionRedundantStoreAdmissionStorageV1,
)> {
    let required = required_store_floor(prefix, deletion)?;
    store_scope(required, budget, |budget| {
        drop(check_store_output(
            prefix,
            StoreDeletionView::Borrowed(deletion),
            budget,
        )?);
        let storage = std::mem::size_of::<ProductionRedundantStoreAdmissionV1<'_>>();
        budget.reserve_storage(storage)?;
        Ok((
            ProductionRedundantStoreAdmissionV1 { prefix, deletion },
            ProductionRedundantStoreAdmissionStorageV1(storage),
        ))
    })
}

fn store_scope<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>) -> StoreResult<T>,
) -> StoreResult<T> {
    if budget.storage() < required {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut deferred_panic = None;
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(StoreError::Panicked)
        }
    };
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(result)))
        {
            deferred_panic = Some(payload);
        }
        drop(deferred_panic);
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(result)))
        {
            deferred_panic = Some(payload);
        }
        drop(deferred_panic);
        return Err(error.into());
    }
    // A custom panic payload may itself unwind on destruction, but cannot skip
    // restoration of this scope's reservations. Its owned scratch is gone first.
    drop(deferred_panic);
    result
}

fn check_store_output(
    prefix: StorePrefix<'_>,
    deletion: StoreDeletionView<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> StoreResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(3)?;
    if !std::ptr::eq(prefix.output(), deletion.input()) {
        return Err(refused("redundant Store", "exact retained Policy6 input owner").into());
    }
    prefix.replay(budget)?;
    let (_relation, storage) = deletion.replay(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    // Prepay returned rows outside the UnitLocal erasure callback so its
    // scoped scratch cleanup cannot release storage backing escaped reports.
    let rows = deletion
        .output()
        .module()
        .kernels
        .len()
        .checked_mul(std::mem::size_of::<FormalMemoryObligations>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(rows)?;
    let source = prefix.source();
    let (coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            source.neutral()?,
            prefix.bound(),
            budget,
        )
        .map_err(E::Coordinates)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (bound, storage) =
        CanonicalKirInventoryV1::derive(prefix.bound(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    match source {
        GeneralSourceContextV1::Direct(source) => {
            let sites = private_memory::source_statement_sites_v1(source, &bound, budget)?;
            check_store_sites(prefix, deletion, &bound, &sites, budget)
        }
        GeneralSourceContextV1::Erased(source) => source
            .with_checked_erasure_v1(budget, |erasure, budget| {
                Ok(store_scope(budget.storage(), budget, |budget| {
                    if !std::ptr::eq(erasure.output(), coordinates.input()) {
                        return Err(
                            E::SourceOutput(ProductionSourceOutputErrorV1::InputCustody).into()
                        );
                    }
                    let map = ErasedSourceCoordinateMapV1 {
                        deletion: erasure,
                        floor: budget.storage(),
                    };
                    let sites = private_memory::erased_source_statement_sites_v1(
                        source, &map, &bound, budget,
                    )?;
                    check_store_sites(prefix, deletion, &bound, &sites, budget)
                }))
            })
            .map_err(E::Source)?,
    }
}

#[path = "production_checked_output_redundant_store_sites_v1.rs"]
mod sites;
use sites::check_store_sites;
#[path = "production_checked_output_owned_redundant_store_v1.rs"]
mod owned;
pub use owned::{
    ProductionOwnedRedundantStoreContinuationV1, ProductionOwnedRedundantStoreStorageV1,
    ProductionOwnedUnitLocalRedundantStoreContinuationV1,
};

#[path = "production_checked_output_commutative_v1.rs"]
mod commutative;
pub use commutative::{
    ProductionCommutativeContinuationErrorV1, ProductionCommutativeContinuationStorageV1,
    ProductionOwnedCommutativeContinuationV1, ProductionOwnedPrivateCellPromotionContinuationV1,
    ProductionOwnedUnitLocalCommutativeContinuationV1,
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    ProductionPrivateCellPromotionContinuationErrorV1,
    ProductionPrivateCellPromotionContinuationStorageV1,
};
pub use commutative::{
    ProductionLicmErrorV1, ProductionLicmStorageV1, ProductionOwnedLicmContinuationV1,
    ProductionOwnedUnitLocalLicmContinuationV1,
};
pub use commutative::{
    ProductionLoopInductionQueryErrorV1, ProductionLoopInductionQueryStorageV1,
    ProductionLoopInductionQueryV1,
};
pub use commutative::{
    ProductionLoopPreheaderIncomingOriginV1, ProductionLoopPreheaderOriginV1,
    ProductionLoopPreheaderParameterOriginV1, ProductionLoopPreheadersErrorV1,
    ProductionLoopPreheadersStorageV1, ProductionOwnedLoopPreheadersContinuationV1,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
};

#[cfg(test)]
#[path = "production_checked_output_redundant_store_scope_v1_tests.rs"]
mod scope_tests;
