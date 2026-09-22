//! Paid immutable access to a catalog derived from the actual retained source.
#![allow(
    clippy::result_large_err,
    reason = "Preserve typed child causes without allocation."
)]
#![allow(
    clippy::drop_non_drop,
    reason = "End witness borrows before releasing ledger storage."
)]

use super::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, ProductionSemanticKirErrorV1,
    ProductionSourceOutputCatalogErrorV1, SourceCatalogCorrespondenceV1,
    source_catalog_from_live_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, KernelIrContractCatalogBindingErrorV1,
    check_kernel_ir_contract_catalog_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use std::mem::size_of;

/// Source replay, derived-catalog or callback failure; nested causes stay typed.
#[derive(Debug)]
pub enum SourcePipelineCatalogCallbackErrorV1<E> {
    /// Work, storage, arithmetic or retained-budget custody validation failed.
    Resource(Resource),
    /// The retained semantic source or its executable correspondence failed replay.
    Source(ProductionSemanticKirErrorV1),
    /// Inventory construction rejected the actual retained executable graph.
    Inventory(CanonicalKirInventoryErrorV1),
    /// Deriving a contract catalog from the retained semantic source failed.
    Catalog(ProductionSourceOutputCatalogErrorV1),
    /// The derived catalog did not bind to the actual executable inventory.
    Binding(KernelIrContractCatalogBindingErrorV1),
    /// The callback returned its original typed error.
    Callback(E),
    /// The source owner lacks the connected pre-ranked executable required here.
    MissingConnectedSource,
    /// A genuine nonempty catalog needs transport that this callback does not support.
    UnsupportedNonemptyCatalog,
    /// Original and helper-erased source catalogs differ in their canonical bytes.
    CatalogMismatch,
    /// Source replay, catalog preparation or the callback panicked.
    Panicked,
}
impl<E> From<Resource> for SourcePipelineCatalogCallbackErrorV1<E> {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Debug> std::fmt::Display for SourcePipelineCatalogCallbackErrorV1<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source pipeline catalog callback: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for SourcePipelineCatalogCallbackErrorV1<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Callback(error) => Some(error),
            _ => None,
        }
    }
}

/// A temporary, genuinely source-derived catalog, not a source-to-output proof.
/// The complete catalog and binding receipts remain reserved until the callback
/// returns. No detached metadata or caller-provided empty catalog enters here.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedSourcePipelineCatalogV1;
/// fn duplicate(view: CheckedSourcePipelineCatalogV1<'_>) { let _ = view.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedSourcePipelineCatalogV1;
/// fn construct() { let _ = CheckedSourcePipelineCatalogV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedSourcePipelineCatalogV1;
/// fn empty() -> CheckedSourcePipelineCatalogV1<'static> { Default::default() }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{CheckedSourcePipelineCatalogV1, ProductionSemanticKirOwnerV1};
/// fn authority(view: CheckedSourcePipelineCatalogV1<'_>) -> ProductionSemanticKirOwnerV1 {
///     view.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{CanonicalOutputFormalSourceAnchorV1,
///     with_checked_source_pipeline_catalog_v1};
/// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1,
///     InertCanonicalKernelIrContractCatalogV1};
/// fn escape<'a>(source: CanonicalOutputFormalSourceAnchorV1<'a>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> &'a InertCanonicalKernelIrContractCatalogV1 {
///     with_checked_source_pipeline_catalog_v1(source, budget,
///         |view, budget| view.catalog(budget)).unwrap()
/// }
/// ```
pub struct CheckedSourcePipelineCatalogV1<'scope> {
    _source: Anchor<'scope>,
    original: &'scope Graph,
    catalog: &'scope Catalog,
    custody: &'scope CallbackCustody,
}
impl CheckedSourcePipelineCatalogV1<'_> {
    /// Borrows the freshly derived, independently N-bound catalog.
    pub fn catalog<'a>(&'a self, budget: &mut Budget<'_>) -> Result<&'a Catalog, Resource> {
        self.custody.validate(budget)?;
        budget.charge_work(1)?;
        Ok(self.catalog)
    }
    /// Borrows actual original N, never the erased or final optimized graph.
    pub fn original_executable<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<&'a Graph, Resource> {
        self.custody.validate(budget)?;
        budget.charge_work(1)?;
        Ok(self.original)
    }
    /// Retains source custody but grants no source-to-final history relation.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

type Error<E> = SourcePipelineCatalogCallbackErrorV1<E>;
type ResultV1<T, E> = Result<T, Error<E>>;
type PanicPayloads = [Option<Box<dyn std::any::Any + Send>>; 2];
struct CallbackCustody {
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    floor: usize,
}
impl CallbackCustody {
    fn validate(&self, budget: &Budget<'_>) -> Result<(), Resource> {
        if budget.work_ledger_identity_v1() != self.ledger
            || budget as *const Budget<'_> as usize != self.slot
            || budget.storage() < self.floor
        {
            return Err(Resource::Accounting);
        }
        Ok(())
    }
}
const GUARD: usize = 2 * size_of::<usize>()
    + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<PanicPayloads>();
type SourceParts<'a> = (
    &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    &'a super::SemanticKirCorrespondenceV1,
    &'a Graph,
    Option<&'a Graph>,
);

fn scoped<'w, T, E>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(
        &mut Budget<'w>,
        &mut PanicPayloads,
        CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
    ) -> ResultV1<T, E>,
) -> ResultV1<T, E> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let paid = GUARD
        .checked_add(size_of::<ResultV1<T, E>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        run(budget, &mut payloads, ledger, slot)
    })) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let same =
        budget.work_ledger_identity_v1() == ledger && slot == budget as *const Budget<'w> as usize;
    let minimum = floor.checked_add(paid).ok_or(Resource::Arithmetic)?;
    if !same || budget.storage() < minimum {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
    }
    // A rejected callback result is dropped before same-ledger scratch release.
    // Never refund or overwrite an independently supplied replacement ledger.
    if same && budget.storage() >= floor {
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(payload);
            }
        }
    }
    drop(payloads);
    result
}

fn empty<E>(catalog: &Catalog, budget: &mut Budget<'_>) -> ResultV1<(), E> {
    budget.charge_work(2)?;
    if !catalog.definitions().is_empty() || !catalog.bindings().is_empty() {
        return Err(Error::UnsupportedNonemptyCatalog);
    }
    Ok(())
}

/// Replays actual Direct or N/E source custody and lends its derived catalog.
///
/// This first version explicitly refuses genuinely derived nonempty catalogs.
/// It does not implement final-graph catalog transport. The caller must still
/// independently bind the catalog to its actual final graph and replay history.
/// Existing source-replay allocation domains remain unchanged; new catalog,
/// inventory, binding and callback-view storage uses this same caller ledger.
/// Source backing must already be reserved. Callback-owned result backing is
/// caller-owned and must follow its own transfer-receipt protocol.
/// Every query and callback return must retain the same Budget slot, work ledger
/// and complete live catalog/inventory/binding/view reservation. Failed queries
/// neither charge an unrelated ledger nor return a borrowed owner.
pub fn with_checked_source_pipeline_catalog_v1<'w, T, E>(
    source: Anchor<'_>,
    budget: &mut Budget<'w>,
    use_catalog: impl for<'scope> FnOnce(
        CheckedSourcePipelineCatalogV1<'scope>,
        &mut Budget<'w>,
    ) -> Result<T, E>,
) -> ResultV1<T, E> {
    scoped(budget, |budget, payloads, ledger, slot| {
        budget.charge_work(5)?;
        let minimum = match source {
            Anchor::Direct(source) => source
                .pre_ranked_retained_analysis_storage_v1()
                .ok_or(Error::MissingConnectedSource)?,
            Anchor::Erased(source) => source.retained_storage_floor_v1(),
        };
        let guard = GUARD
            .checked_add(size_of::<ResultV1<T, E>>())
            .ok_or(Resource::Arithmetic)?;
        if budget
            .storage()
            .checked_sub(guard)
            .is_none_or(|s| s < minimum)
        {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(size_of::<SourceParts<'_>>())?;
        let (semantic, correspondence, original, erased) = match source {
            Anchor::Direct(source) => {
                let original = source
                    .pre_ranked_executable()
                    .ok_or(Error::MissingConnectedSource)?;
                source
                    .verify_equivalence_with_budget_v1(budget)
                    .map_err(Error::Source)?;
                (
                    source.semantic_ssa.source_semantic(),
                    &source.correspondence,
                    original,
                    None,
                )
            }
            Anchor::Erased(source) => {
                source.verify_equivalence(budget).map_err(Error::Source)?;
                let original = source.original_source();
                (
                    original.semantic_ssa.source_semantic(),
                    &original.correspondence,
                    original.executable(),
                    Some(source.erased()),
                )
            }
        };
        let (inventory, storage) =
            CanonicalKirInventoryV1::derive(original, budget).map_err(Error::Inventory)?;
        budget.reserve_storage(storage.retained_storage())?;
        let (catalog, storage) = source_catalog_from_live_v1(
            semantic,
            SourceCatalogCorrespondenceV1(correspondence),
            &inventory,
            budget,
        )
        .map_err(Error::Catalog)?;
        budget.reserve_storage(storage.retained_storage())?;
        let (binding, storage) = check_kernel_ir_contract_catalog_v1(&inventory, &catalog, budget)
            .map_err(Error::Binding)?;
        budget.reserve_storage(storage.retained_storage())?;
        empty(&catalog, budget)?;
        if let Some(erased) = erased {
            let floor = budget.storage();
            let (erased_inventory, storage) =
                CanonicalKirInventoryV1::derive(erased, budget).map_err(Error::Inventory)?;
            budget.reserve_storage(storage.retained_storage())?;
            let (erased_catalog, storage) = source_catalog_from_live_v1(
                semantic,
                SourceCatalogCorrespondenceV1(correspondence),
                &erased_inventory,
                budget,
            )
            .map_err(Error::Catalog)?;
            budget.reserve_storage(storage.retained_storage())?;
            let (erased_binding, storage) =
                check_kernel_ir_contract_catalog_v1(&erased_inventory, &erased_catalog, budget)
                    .map_err(Error::Binding)?;
            budget.reserve_storage(storage.retained_storage())?;
            empty(&erased_catalog, budget)?;
            budget.charge_work(
                catalog
                    .canonical_bytes()
                    .len()
                    .checked_add(erased_catalog.canonical_bytes().len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if catalog.canonical_bytes() != erased_catalog.canonical_bytes() {
                return Err(Error::CatalogMismatch);
            }
            drop(erased_binding);
            drop(erased_catalog);
            drop(erased_inventory);
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
        }
        budget.reserve_storage(
            size_of::<CheckedSourcePipelineCatalogV1<'_>>()
                .checked_add(size_of::<CallbackCustody>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let custody = CallbackCustody {
            ledger,
            slot,
            floor: budget.storage(),
        };
        custody.validate(budget)?;
        let view = CheckedSourcePipelineCatalogV1 {
            _source: source,
            original,
            catalog: &catalog,
            custody: &custody,
        };
        let mut result = use_catalog(view, budget).map_err(Error::Callback);
        if let Err(error) = custody.validate(budget) {
            let rejected = std::mem::replace(&mut result, Err(Error::Resource(error)));
            if let Err(payload) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(rejected)))
            {
                // Reuse the already paid outer slot. Its payload destructor
                // runs only after the outer guard has restored the ledger.
                payloads[1] = Some(payload);
            }
        }
        drop(binding);
        drop(catalog);
        drop(inventory);
        result
    })
}
