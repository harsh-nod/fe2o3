//! Copied test instrumentation, never a budget/source pointer or an owner.
use super::*;
use std::cell::Cell;
#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq, Eq)]
pub(crate) struct PhaseObservation {
    pub(crate) entry_storage: usize,
    pub(crate) source_storage: usize,
    pub(crate) protected_storage: usize,
    pub(crate) materializer_storage: Option<usize>,
    pub(crate) final_storage: usize,
    pub(crate) same_ledger: bool,
    pub(crate) work: Option<usize>,
    pub(crate) failed_work: bool,
    pub(crate) failed_storage: bool,
    pub(crate) result_ok: bool,
}
thread_local! {
    // One preallocated row per actual callback thread, not an observation Vec.
    static PHASE:Cell<Option<PhaseObservation>>=const {Cell::new(None)};
}
pub(super) fn record(row: PhaseObservation) {
    PHASE.with(|slot| {
        assert!(
            slot.replace(Some(row)).is_none(),
            "duplicate phase observation"
        )
    });
}
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_bf16_mfma_source_for_test_v1<R>(
        self,
        inspect: impl for<'a, 'work> FnOnce(
            &SourceOwnedBf16MfmaRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    ) -> (
        Result<R, Box<ProductionPipelineError>>,
        Option<PhaseObservation>,
    ) {
        PHASE.with(|slot| assert!(slot.take().is_none(), "stale phase observation"));
        let result =
            self.materialize_with_bf16_mfma_inspection_v1(inspect)
                .map(|(ordinary, value)| {
                    drop(ordinary);
                    value
                });
        let observed = PHASE.with(Cell::take);
        (result, observed)
    }
}
