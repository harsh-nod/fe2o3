//! Private nonempty-read query entry. It never runs the historical empty
//! observer, and does not replace ordinary nominal compilation's refusal.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryStorageV1, CanonicalKirInventoryV1,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn inventory_error(error: CanonicalKirInventoryErrorV1) -> Error {
    match error {
        CanonicalKirInventoryErrorV1::Resource(error) => Error::Resource(error),
        CanonicalKirInventoryErrorV1::InconsistentOwner => {
            Error::Unavailable("single-read canonical inventory owner/roster mismatch")
        }
    }
}
fn query_error(error: QueryError) -> Error {
    match error {
        QueryError::Resource(error) => Error::Resource(error),
        QueryError::Unavailable(detail) => Error::Unavailable(detail),
        QueryError::CallbackPanicked => Error::CallbackPanicked,
    }
}
pub(super) fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let entry = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let frame = std::mem::size_of::<(
        &ProductionPreRankedKirOwnerV1,
        &CheckedBf16CallInstanceV1<'static>,
        &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
        &mut Budget<'static>,
        usize,
        usize,
        usize,
        usize,
        CanonicalKirInventoryV1<'static>,
        CanonicalKirInventoryStorageV1,
        CanonicalKirInventoryErrorV1,
        Result<
            (
                CanonicalKirInventoryV1<'static>,
                CanonicalKirInventoryStorageV1,
            ),
            CanonicalKirInventoryErrorV1,
        >,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        Result<(), Error>,
        Result<(), QueryError>,
        std::result::Result<Result<(), Error>, Box<dyn std::any::Any + Send>>,
    )>();
    budget.reserve_storage(frame)?;
    let frame_floor = budget.storage();
    let result = (|| {
        let (inventory, storage) =
            CanonicalKirInventoryV1::derive(owner.executable(), budget).map_err(inventory_error)?;
        budget.reserve_storage(storage.retained_storage())?;
        let inventory_floor = budget.storage();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            crate::production_ranked_projection_v1::observe_initial_nonempty_reads_for_test_v1(
                owner, source, actual, &inventory, budget,
            )
            .map_err(query_error)
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(Error::CallbackPanicked)
            }
        };
        if budget.work_ledger_identity_v1() != ledger || budget.storage() < inventory_floor {
            drop(inventory);
            return Err(Error::Resource(Resource::Accounting));
        }
        drop(inventory);
        budget.release_storage(storage.retained_storage())?;
        if budget.storage() != frame_floor {
            return Err(Error::Resource(Resource::Accounting));
        }
        result
    })();
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < frame_floor {
        return Err(Error::Resource(Resource::Accounting));
    }
    budget.release_storage(frame)?;
    if budget.storage() != entry {
        return Err(Error::Resource(Resource::Accounting));
    }
    result
}
