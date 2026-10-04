//! Test-only SAME-compilation continuation. This never reconstructs an owner
//! from the copied snapshot, and never starts or replaces a resource meter.
use super::*;

pub(crate) struct Continued<R> {
    pub(crate) result: Result<TargetLoweredProductionCompilation, Box<ProductionPipelineError>>,
    pub(crate) snapshot: Option<R>,
    pub(crate) phase: Option<PhaseObservation>,
    pub(crate) stage: &'static str,
    pub(crate) original_v12_identity: Option<[u8; 32]>,
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn continue_bf16_mfma_source_for_test_v1<R>(
        self,
        inspect: impl for<'a, 'work> FnOnce(
            &SourceOwnedBf16MfmaRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    ) -> Continued<R> {
        PHASE.with(|slot| assert!(slot.take().is_none(), "stale phase observation"));
        let materialized = self.materialize_with_bf16_mfma_inspection_v1(inspect);
        let phase = PHASE.with(Cell::take);
        let (ordinary, snapshot) = match materialized {
            Ok(value) => value,
            Err(error) => {
                return Continued {
                    result: Err(error),
                    snapshot: None,
                    phase,
                    stage: "pre-ranked-source",
                    original_v12_identity: None,
                };
            }
        };
        let original = *ordinary.materialized.executable().canonical().identity();
        let mut stage = "ranked";
        // Original phase accounting has ended exactly where it already ended.
        // Later mandatory stages keep their own existing accounting contracts.
        let result = (|| -> Result<_, ProductionPipelineError> {
            let ranked = ordinary.verify_general_kernel_checks()?;
            stage = "target-neutral-attachment";
            let neutral = ranked.attach_target_neutral_checks()?;
            stage = "formal-memory";
            let formal = neutral.admit_formal_memory()?;
            stage = "target";
            let target = formal.lower_production_target()?;
            stage = "retained-original-owner";
            let retained = target
                .admitted
                .semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| {
                    unavailable("test normal continuation lost original pre-ranked owner")
                })?;
            if retained.canonical().identity() != &original {
                return Err(unavailable(
                    "test normal continuation changed original pre-ranked identity",
                ));
            }
            stage = "target-lowered";
            Ok(target)
        })();
        Continued {
            result: result.map_err(Box::new),
            snapshot: Some(snapshot),
            phase,
            stage,
            original_v12_identity: Some(*original.digest()),
        }
    }
}
