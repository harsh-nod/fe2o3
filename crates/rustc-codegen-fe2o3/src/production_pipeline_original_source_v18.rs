//! Original live-rustc source admission, without converting its schema or types.
use super::*;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Preserve rustc's original nominal types and the authentic producer-selected
    /// grammar through the shared source-owned preparation and integer optimizer.
    ///
    /// This uses the existing nominal-preserving importer, not a fixed MIR35
    /// reencoding: ordinary source retains Current grammar and authenticated
    /// specialized producers retain their exact sibling admission checks. Mixed
    /// grammars which cannot represent nominal words still refuse. The actual
    /// source and output remain lexical; this grants no final publication,
    /// target, functional-reference, or native execution authority.
    pub(crate) fn with_original_source_integer_custody_v18<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &IntegerHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<UnqualifiedInteger, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }

    /// Exercises genuine original-source preparation and checked optimization,
    /// then refuses the unavailable general memory/native/publication admission.
    /// This fixed nominal-preserving continuation is not a default-route fallback.
    pub(crate) fn original_source_integer_finalizer_refusal_v18(self) -> Result<(), Error> {
        self.with_original_source_integer_custody_v18::<(), _>(|source, handoff, _, _, budget| {
            handoff.check_original_source(source.source_ssa(budget)?, budget)?;
            let _ = handoff.output(budget)?;
            Err(Error::Unsupported(
                "original source integer final admission required",
            ))
        })
        .map(SourceOwnedCompilationContinuationV29::into_observation)
    }

    #[cfg(test)]
    pub(crate) fn original_source_ssa_for_test_v18(
        self,
    ) -> Result<fe2o3_pliron::ProductionSemanticSsaOwnerV1, Error> {
        let source = self
            .import_semantic_mir_with_profile_v29(ImportProfile::NominalV35)?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        Ok(source.stage.semantic_ssa)
    }

    #[cfg(test)]
    pub(crate) fn with_original_source_integer_test_limits_v18<R, F>(
        self,
        storage_limit: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &IntegerHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<UnqualifiedInteger, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            storage_limit,
            consume,
        )
    }
}
