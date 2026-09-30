//! Nominal mixed inputs joined to the original protected compiler custody.
//! Shared capsule and sealed-verifier extensions still own publication admission.

use super::*;
use crate::production_pipeline::{AuthenticatedProductionBindings, ProductionCompilerCustody};
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1,
};
use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_verifier::MixedOptimizerRelocationCfgSubjectV28 as Subject;

/// These producer/consumer joins have no admitted mixed implementation yet.
/// They cannot be closed by caller flags, graph hashes or existing V12 receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MixedPublicationOpenGateV28 {
    OriginalMirToKirRefinement,
    ExecutedComposedRefinement,
    MixedSemanticCapsuleTransport,
    ProtectedCompilerExecutionJoin,
    // Existing inspected-artifact/descriptor admission; LLVM and LLD are trusted.
    FinalArtifactIdentityAndAdmission,
    SealedMixedWorkerFinalizerReplay,
    ConcreteRuntimePremiseDischarge,
}

#[derive(Debug)]
pub(crate) enum MixedPublicationErrorV28 {
    ExtractionOnly,
    LiveInvocation(ProtectedRustcInvocationErrorV1),
    Binding(&'static str),
}
impl std::fmt::Display for MixedPublicationErrorV28 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mixed publication custody: {self:?}")
    }
}
impl std::error::Error for MixedPublicationErrorV28 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LiveInvocation(error) => Some(error),
            Self::ExtractionOnly | Self::Binding(_) => None,
        }
    }
}
impl From<MixedPublicationErrorV28> for Error {
    fn from(error: MixedPublicationErrorV28) -> Self {
        Self::MixedPublication(error)
    }
}

/// The fixed pipeline lends every original and final owner to this move-only
/// candidate. No serialized graph, replacement descriptor or proof receipt can
/// construct it, and its higher-ranked consumer cannot retain it after cleanup.
#[must_use = "consume under original source custody or abandon the candidate"]
pub(crate) struct PreparedMixedPublicationV28<'a, 'v, 's> {
    inputs: FinalInputs<'a, 'a, 'v, 's>,
    required: usize,
}

/// Original protected rustc custody has been revalidated while all mixed owners
/// remain borrowed. This is neither a published capsule nor a verifier decision.
#[must_use = "retain the complete mixed candidate while joining shared publication"]
pub(crate) struct ProtectedMixedPublicationV28<'a, 'v, 's> {
    prepared: PreparedMixedPublicationV28<'a, 'v, 's>,
}

fn retained_invocation(
    custody: &ProductionCompilerCustody,
) -> Result<(&AdmittedProtectedRustcInvocationV1, BuildAttempt), Error> {
    match custody {
        ProductionCompilerCustody::ProtectedV3 {
            invocation,
            attempt,
        } => Ok((invocation, *attempt)),
        ProductionCompilerCustody::ExtractionOnly => {
            Err(MixedPublicationErrorV28::ExtractionOnly.into())
        }
    }
}

impl<'a, 'v, 's> PreparedMixedPublicationV28<'a, 'v, 's> {
    fn check(&self, budget: &Budget<'_>) -> Result<(), Error> {
        self.inputs
            .native
            .observe_retained_storage_v28(self.required, budget)?;
        self.inputs
            .composed
            .retained_storage(budget)
            .map_err(Error::MixedRelocationExpressions)?;
        self.inputs.worker.root_count(budget)?;
        Ok(())
    }

    pub(crate) fn replay(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.check(budget)?;
        self.inputs
            .composed
            .check_original_source(self.inputs.source.source_ssa(budget)?, budget)
            .map_err(Error::MixedRelocationExpressions)?;
        self.inputs
            .composed
            .replay(budget)
            .map_err(Error::MixedRelocationExpressions)
    }

    pub(crate) fn source(&self, budget: &Budget<'_>) -> Result<&Source<'s>, Error> {
        self.check(budget)?;
        Ok(self.inputs.source)
    }

    pub(crate) fn native(&self, budget: &Budget<'_>) -> Result<&Native<'v, 'v, 'v, 's>, Error> {
        self.check(budget)?;
        Ok(self.inputs.native)
    }

    pub(crate) fn refinement_subject(&self, budget: &Budget<'_>) -> Result<Subject, Error> {
        self.check(budget)?;
        self.inputs
            .composed
            .subject(budget)
            .map_err(Error::MixedRelocationExpressions)
    }

    pub(crate) fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8], Error> {
        self.check(budget)?;
        self.inputs
            .composed
            .generated_source(budget)
            .map_err(Error::MixedRelocationExpressions)
    }

    pub(crate) fn worker(
        &self,
        budget: &Budget<'_>,
    ) -> Result<&Worker<'a, 'a, 'v, 's, 'a, 'a>, Error> {
        self.check(budget)?;
        Ok(self.inputs.worker)
    }

    fn bindings(&self, budget: &Budget<'_>) -> Result<&AuthenticatedProductionBindings, Error> {
        self.check(budget)?;
        Ok(self.inputs.context.bindings)
    }

    fn revalidate_protected(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.replay(budget)?;
        let bindings = self.bindings(budget)?;
        let (invocation, _) = retained_invocation(&bindings.transaction.compiler_custody)?;
        let expected_target = bindings.rustc_target.profile().device_target();
        budget.charge_work(
            64usize
                .checked_add(expected_target.len())
                .and_then(|n| n.checked_add(invocation.descriptor().amd_target().len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != bindings.rustc_identity_inventory.sha256()
            || invocation.descriptor().amd_target() != expected_target
        {
            return Err(MixedPublicationErrorV28::Binding(
                "original inventory, preflight or protected target changed",
            )
            .into());
        }
        invocation
            .revalidate_for_publication_with_image_budget(budget)
            .map_err(MixedPublicationErrorV28::LiveInvocation)?;
        self.check(budget)
    }

    /// Joins genuine original protected custody; it neither serializes nor
    /// publishes. Complete process-observation metering remains with the shared
    /// protected invocation implementation used by the existing native route.
    pub(crate) fn into_protected(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<ProtectedMixedPublicationV28<'a, 'v, 's>, Error> {
        self.revalidate_protected(budget)?;
        Ok(ProtectedMixedPublicationV28 { prepared: self })
    }

    pub(crate) const fn open_gates(&self) -> &'static [MixedPublicationOpenGateV28] {
        use MixedPublicationOpenGateV28::*;
        &[
            OriginalMirToKirRefinement,
            ExecutedComposedRefinement,
            MixedSemanticCapsuleTransport,
            ProtectedCompilerExecutionJoin,
            FinalArtifactIdentityAndAdmission,
            SealedMixedWorkerFinalizerReplay,
            ConcreteRuntimePremiseDischarge,
        ]
    }

    pub(crate) const fn grants_publication_or_artifact_authority(&self) -> bool {
        false
    }
}

impl<'a, 'v, 's> ProtectedMixedPublicationV28<'a, 'v, 's> {
    /// Freshly checks original custody before a future shared capsule consumer.
    pub(crate) fn revalidate(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.prepared.revalidate_protected(budget)
    }

    pub(crate) fn inputs(
        &self,
        budget: &Budget<'_>,
    ) -> Result<&PreparedMixedPublicationV28<'a, 'v, 's>, Error> {
        self.prepared.check(budget)?;
        Ok(&self.prepared)
    }

    pub(crate) fn invocation(
        &self,
        budget: &Budget<'_>,
    ) -> Result<&fe2o3_rustc_invocation::RustcInvocationDescriptorV3, Error> {
        let bindings = self.prepared.bindings(budget)?;
        let (invocation, _) = retained_invocation(&bindings.transaction.compiler_custody)?;
        Ok(invocation.descriptor())
    }

    pub(crate) fn attempt(&self, budget: &Budget<'_>) -> Result<BuildAttempt, Error> {
        let bindings = self.prepared.bindings(budget)?;
        retained_invocation(&bindings.transaction.compiler_custody).map(|(_, attempt)| attempt)
    }
}

struct PublicationInput;
fn headers() -> Result<usize, Resource> {
    [
        size_of::<PreparedMixedPublicationV28<'_, '_, '_>>(),
        align_of::<PreparedMixedPublicationV28<'_, '_, '_>>(),
        size_of::<ProtectedMixedPublicationV28<'_, '_, '_>>(),
        align_of::<ProtectedMixedPublicationV28<'_, '_, '_>>(),
        size_of::<Result<ProtectedMixedPublicationV28<'_, '_, '_>, Error>>(),
        size_of::<Result<(&AdmittedProtectedRustcInvocationV1, BuildAttempt), Error>>(),
        size_of::<Result<(), Error>>(),
        size_of::<Result<(), ProtectedRustcInvocationErrorV1>>(),
        size_of::<[usize; 4]>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
impl<R, F> FinalConsumer<R, F> for PublicationInput
where
    F: for<'a, 'v, 's, 'w> FnOnce(
        PreparedMixedPublicationV28<'a, 'v, 's>,
        &mut Budget<'w>,
    ) -> Result<R, Error>,
{
    fn headers() -> Result<usize, Resource> {
        headers()?
            .checked_add(size_of::<Pending<F>>())
            .and_then(|n| n.checked_add(align_of::<Pending<F>>()))
            .ok_or(Resource::Arithmetic)
    }
    fn consume(
        inputs: FinalInputs<'_, '_, '_, '_>,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<R, Error> {
        let mut consume = Pending::new(consume);
        let prepared = PreparedMixedPublicationV28 {
            inputs,
            required: budget.storage(),
        };
        prepared.check(budget)?;
        consume.take()(prepared, budget)
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Lends the protected client's original account to the same fixed pipeline.
    /// Root-phase charges stay reserved until that enclosing transaction ends.
    /// Publication work must consume the borrowed candidate inside the callback.
    pub(crate) fn with_original_source_mixed_publication_on_account_v28<R, F>(
        self,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'a, 'v, 's, 'w> FnOnce(
            PreparedMixedPublicationV28<'a, 'v, 's>,
            &mut Budget<'w>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_on_account_v29::<MixedWorker<PublicationInput>, R, F>(
            ImportProfile::NominalV35,
            budget,
            consume,
        )
    }

    /// Retains one exact mixed chain for the shared protected publication join.
    /// The returned continuation contains observations only, after all actual
    /// source/output owners have been disposed inside this lexical transaction.
    pub(crate) fn with_original_source_mixed_publication_v28<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'a, 'v, 's, 'w> FnOnce(
            PreparedMixedPublicationV28<'a, 'v, 's>,
            &mut Budget<'w>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedWorker<PublicationInput>, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }

    #[cfg(test)]
    pub(crate) fn with_original_source_mixed_publication_test_limits_v28<R, F>(
        self,
        work: usize,
        storage: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'a, 'v, 's, 'w> FnOnce(
            PreparedMixedPublicationV28<'a, 'v, 's>,
            &mut Budget<'w>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedWorker<PublicationInput>, R, F>(
            ImportProfile::NominalV35,
            work,
            storage,
            consume,
        )
    }
}

#[cfg(test)]
#[path = "production_pipeline_source_mixed_publication_v28_tests.rs"]
mod tests;
