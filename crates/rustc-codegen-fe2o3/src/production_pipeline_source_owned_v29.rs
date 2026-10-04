//! Fixed live-rustc MIR29 import and nominal lexical V18 output continuations.
//! This is not the default compiler route or final ranked/formal/target authority.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as TargetProfile;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
pub(crate) use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionBoundScalarHandoffErrorV19, ProductionBoundScalarOutputHandoffV19 as BoundHandoff,
    ProductionClosedScalarHandoffErrorV18, ProductionClosedScalarOutputHandoffV18 as Handoff,
    ProductionExecutionSourceInputV29, ProductionKernelArgumentAbiInputV18,
    ProductionKernelArgumentAbiRootV18 as AbiRoot,
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionScalarCfgHandoffErrorV18,
    ProductionScalarCfgOutputHandoffV18 as CfgHandoff,
    ProductionScopeCallableCandidateV29 as Class, ProductionSourceOwnedViewErrorV18,
    ProductionSourceOwnedViewV18 as Source, ProductionUnqualifiedIntegerHandoffErrorV18,
    ProductionUnqualifiedIntegerOutputHandoffV18 as IntegerHandoff,
};
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[path = "production_pipeline_original_source_v18.rs"]
mod original_source_v18;

#[path = "production_pipeline_source_formal_context_v19.rs"]
mod formal_context_v19;

#[path = "production_pipeline_source_reference_obligations_v69.rs"]
mod reference_obligations_v69;

#[cfg(test)]
pub(crate) use reference_obligations_v69::tests::ReferenceObligationObservationV69;

/// Borrows the original compiler bindings through the lexical source visit.
/// A copied ABI roster or target string cannot construct this private context.
struct SourceBindingContextV29<'bindings> {
    bindings: &'bindings AuthenticatedProductionBindings,
}

pub(super) enum ImportProfile {
    Current,
    NominalV35,
    SourceOwnedV29,
}

#[derive(Debug)]
pub(crate) enum Error {
    Pipeline(Box<ProductionPipelineError>),
    Descriptor(crate::compiler_descriptor::CompilerDescriptorError),
    Source(ProductionSourceOwnedViewErrorV18),
    Handoff(ProductionClosedScalarHandoffErrorV18),
    IntegerHandoff(ProductionUnqualifiedIntegerHandoffErrorV18),
    ScalarCfgHandoff(ProductionScalarCfgHandoffErrorV18),
    BoundScalarHandoff(ProductionBoundScalarHandoffErrorV19),
    BoundPrivateWorklistHandoff(fe2o3_lower_mir_kernel::ProductionBoundPrivateHandoffErrorV21),
    ConditionalMixedHandoff(fe2o3_lower_mir_kernel::ProductionMixedSourceHandoffErrorV26),
    ConditionalMixedCfg(fe2o3_verifier::MixedOptimizerRefinementErrorV26),
    OriginalMir(fe2o3_verifier::MixedOptimizerRefinementErrorV26),
    MixedLicm(fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28),
    MixedLicmCompletion(fe2o3_lower_mir_kernel::ProductionMixedLicmCompletionErrorV28),
    MixedDescriptor(crate::compiler_descriptor::nominal_v3::NominalDescriptorErrorV3),
    MixedRelocationExpressions(fe2o3_verifier::MixedOptimizerRelocationErrorV28),
    MixedPublication(mixed_worker_v28::publication::MixedPublicationErrorV28),
    TargetLlvm(target_result::ClosedScalarTargetLlvmErrorV29),
    MixedWorkerInput(target_result::mixed_v26::worker_input_v26::MixedWorkerInputErrorV26),
    MixedPureCseWorkerInput(
        target_result::mixed_pure_cse_v26::worker_input_v26::MixedWorkerInputErrorV26,
    ),
    MixedLicmWorkerInput(target_result::mixed_licm_v28::worker_input_v26::MixedWorkerInputErrorV26),
    FormalReports(Box<formal_context_v19::ReportOptimizationErrorV19>),
    FormalPaths(Box<formal_context_v19::PathOptimizationErrorV20>),
    Resource(Resource),
    ReferenceObligations(reference_obligations_v69::ReferenceObligationErrorV69),
    Unsupported(&'static str),
}
impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error.as_ref()),
            Self::Descriptor(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::IntegerHandoff(error) => Some(error),
            Self::ScalarCfgHandoff(error) => Some(error),
            Self::BoundScalarHandoff(error) => Some(error),
            Self::BoundPrivateWorklistHandoff(error) => Some(error),
            Self::ConditionalMixedHandoff(error) => Some(error),
            Self::ConditionalMixedCfg(error) => Some(error),
            Self::OriginalMir(error) => Some(error),
            Self::MixedLicm(error) => Some(error),
            Self::MixedLicmCompletion(error) => Some(error),
            Self::MixedDescriptor(error) => Some(error),
            Self::MixedRelocationExpressions(error) => Some(error),
            Self::MixedPublication(error) => Some(error),
            Self::TargetLlvm(error) => Some(error),
            Self::MixedWorkerInput(error) => Some(error),
            Self::MixedPureCseWorkerInput(error) => Some(error),
            Self::MixedLicmWorkerInput(error) => Some(error),
            Self::FormalReports(error) => Some(error.as_ref()),
            Self::FormalPaths(error) => Some(error.as_ref()),
            Self::Resource(error) => Some(error),
            Self::ReferenceObligations(error) => Some(error),
            Self::Unsupported(_) => None,
        }
    }
}
impl From<ProductionPipelineError> for Error {
    fn from(error: ProductionPipelineError) -> Self {
        Self::Pipeline(Box::new(error))
    }
}
impl From<crate::compiler_descriptor::CompilerDescriptorError> for Error {
    fn from(error: crate::compiler_descriptor::CompilerDescriptorError) -> Self {
        Self::Descriptor(error)
    }
}
impl From<ProductionSourceOwnedViewErrorV18> for Error {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ProductionClosedScalarHandoffErrorV18> for Error {
    fn from(error: ProductionClosedScalarHandoffErrorV18) -> Self {
        Self::Handoff(error)
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28> for Error {
    fn from(error: fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28) -> Self {
        Self::MixedLicm(error)
    }
}
impl From<ProductionUnqualifiedIntegerHandoffErrorV18> for Error {
    fn from(error: ProductionUnqualifiedIntegerHandoffErrorV18) -> Self {
        Self::IntegerHandoff(error)
    }
}
impl From<ProductionScalarCfgHandoffErrorV18> for Error {
    fn from(error: ProductionScalarCfgHandoffErrorV18) -> Self {
        Self::ScalarCfgHandoff(error)
    }
}
impl From<ProductionBoundScalarHandoffErrorV19> for Error {
    fn from(error: ProductionBoundScalarHandoffErrorV19) -> Self {
        Self::BoundScalarHandoff(error)
    }
}
impl From<target_result::ClosedScalarTargetLlvmErrorV29> for Error {
    fn from(error: target_result::ClosedScalarTargetLlvmErrorV29) -> Self {
        Self::TargetLlvm(error)
    }
}

impl From<target_result::mixed_v26::worker_input_v26::MixedWorkerInputErrorV26> for Error {
    fn from(error: target_result::mixed_v26::worker_input_v26::MixedWorkerInputErrorV26) -> Self {
        Self::MixedWorkerInput(error)
    }
}

impl From<target_result::mixed_pure_cse_v26::worker_input_v26::MixedWorkerInputErrorV26> for Error {
    fn from(
        error: target_result::mixed_pure_cse_v26::worker_input_v26::MixedWorkerInputErrorV26,
    ) -> Self {
        Self::MixedPureCseWorkerInput(error)
    }
}

impl From<target_result::mixed_licm_v28::worker_input_v26::MixedWorkerInputErrorV26> for Error {
    fn from(
        error: target_result::mixed_licm_v28::worker_input_v26::MixedWorkerInputErrorV26,
    ) -> Self {
        Self::MixedLicmWorkerInput(error)
    }
}

// These are the existing source-owned qualification limits, not an increase to
// the separate legacy canonical phase. No caller selects a shipping policy.
const WORK_LIMIT: usize = 500_000_000;
const STORAGE_LIMIT: usize = 20_000_000;

/// Move-only compiler custody after a lexical source-owned observation.
///
/// The actual source and checked output have already been destroyed. Retaining
/// their original identities and compiler bindings does not retain executable
/// output or grant descriptor, lineage, worker, or publication authority.
/// These digests are identity evidence only; they cannot authenticate a later
/// reconstructed output. Final publication must consume the actual optimized
/// owner under live source custody or an independently admitted owned snapshot.
#[must_use = "dropping the continuation abandons retained compiler custody"]
pub(crate) struct SourceOwnedCompilationContinuationV29<R> {
    observation: R,
    original_source: [u8; 32],
    original_ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    bindings: AuthenticatedProductionBindings,
}

impl<R> SourceOwnedCompilationContinuationV29<R> {
    /// Observation-only callers explicitly give up the retained compiler custody.
    pub(crate) fn into_observation(self) -> R {
        let Self {
            observation,
            original_source: _,
            original_ssa: _,
            bindings,
        } = self;
        drop(bindings);
        observation
    }
}

pub(crate) fn paid_vec<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Error> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(count.checked_add(3).ok_or(Resource::Arithmetic)?)?;
    budget.reserve_storage(requested)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let capacity = result
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        capacity
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(result)
}

fn entry_headers<R, F>() -> Result<usize, Resource> {
    entry_headers_for_handoff::<R, F, Handoff<'static, 'static>>()
}

fn pay_source_visit_capture_v19<F>(visit: &F, budget: &mut Budget<'_>) -> Result<(), Resource> {
    budget.check_prior_denials_v1()?;
    let headers = std::mem::size_of_val(visit)
        .checked_mul(2)
        .and_then(|value| value.checked_add(std::mem::align_of_val(visit)))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(headers)?;
    budget.reserve_storage(headers)
}

fn entry_headers_for_handoff<R, F, H>() -> Result<usize, Resource> {
    type Invoke<'a, 'source, 'work, F, H> = (
        F,
        &'a Source<'source>,
        &'a H,
        &'a [AbiRoot<'a>],
        TargetProfile,
        &'a mut Budget<'work>,
    );
    [
        size_of::<F>(),
        align_of::<F>(),
        size_of::<AssertUnwindSafe<F>>(),
        size_of::<Invoke<'_, '_, '_, F, H>>(),
        align_of::<Invoke<'_, '_, '_, F, H>>(),
        size_of::<AssertUnwindSafe<Invoke<'_, '_, '_, F, H>>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        size_of::<AssertUnwindSafe<Result<R, Error>>>(),
        size_of::<SourceOwnedCompilationContinuationV29<R>>(),
        align_of::<SourceOwnedCompilationContinuationV29<R>>(),
        size_of::<Result<SourceOwnedCompilationContinuationV29<R>, Error>>(),
        align_of::<Result<SourceOwnedCompilationContinuationV29<R>, Error>>(),
        size_of::<PreparedSsaMaterializationV29>(),
        align_of::<PreparedSsaMaterializationV29>(),
        size_of::<Vec<Class>>(),
        size_of::<Vec<AbiRoot<'_>>>(),
        size_of::<ProductionKernelArgumentAbiInputV18<'_>>(),
        size_of::<ProductionExecutionSourceInputV29<'_>>(),
        size_of::<Work>(),
        size_of::<Budget<'_>>(),
        size_of::<SourceBindingContextV29<'_>>(),
        align_of::<SourceBindingContextV29<'_>>(),
        size_of::<&SourceBindingContextV29<'_>>(),
        size_of::<formal_context_v19::PendingConsumerV19<F>>(),
        align_of::<formal_context_v19::PendingConsumerV19<F>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

trait SourceHandoffPolicyV29<R, F> {
    fn entry_headers() -> Result<usize, Resource>;
    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<R, Error>;
}

macro_rules! source_handoff_policy_v29 {
    ($policy:ident, $handoff:ident, $prepare:ident) => {
        source_handoff_policy_v29!(@impl $policy, $handoff,
            |source, roots, context, budget, handoff| {
                let handoff = source.$prepare(ProductionKernelArgumentAbiInputV18 { roots }, budget)?;
            }, []
        );
    };
    (@impl $policy:ident, $handoff:ident,
        |$source:ident, $roots:ident, $context:ident, $budget:ident, $value:ident|
        { $($prepare:tt)* }, [$($header:expr),* $(,)?]) => {
        struct $policy;
        impl<R, F> SourceHandoffPolicyV29<R, F> for $policy
        where
            F: for<'view, 'source, 'abi, 'work> FnOnce(
                &'view Source<'source>,
                &$handoff<'view, 'source>,
                &[AbiRoot<'abi>],
                TargetProfile,
                &mut Budget<'work>,
            ) -> Result<R, Error>,
        {
            fn entry_headers() -> Result<usize, Resource> {
                [
                    entry_headers_for_handoff::<R, F, $handoff<'static, 'static>>()?,
                    size_of::<formal_context_v19::PendingConsumerV19<F>>(),
                    align_of::<formal_context_v19::PendingConsumerV19<F>>(),
                    $($header),*
                ].into_iter().try_fold(0usize, |sum, bytes| {
                    sum.checked_add(bytes).ok_or(Resource::Arithmetic)
                })
            }

            fn consume<'view, 'source, 'abi, 'work>(
                $source: &'view Source<'source>,
                $roots: &[AbiRoot<'abi>],
                $context: &SourceBindingContextV29<'_>,
                $budget: &mut Budget<'work>,
                consume: F,
            ) -> Result<R, Error> {
                let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
                $($prepare)*
                $value.check_original_source($source.source_ssa($budget)?, $budget)?;
                let borrowed = &$value;
                let callback_budget = &mut *$budget;
                let target = $context.bindings.rustc_target.profile();
                let consume = pending.take();
                // Concrete nominal handoffs carry the source/view outlives
                // relationship into the callback without a quantified GAT.
                let result = catch_unwind(AssertUnwindSafe(move || {
                    consume($source, borrowed, $roots, target, callback_budget)
                }));
                let settled = $value.discard($budget);
                match result {
                    Ok(Ok(value)) => {
                        match settled {
                            Ok(()) => Ok(value),
                            Err(error) => {
                                formal_context_v19::discard(value);
                                Err(error.into())
                            }
                        }
                    }
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                }
            }
        }
    };
}

source_handoff_policy_v29!(ClosedScalar, Handoff, checked_closed_scalar_output_v18);
source_handoff_policy_v29!(ScalarCfg, CfgHandoff, checked_scalar_cfg_output_v18);
source_handoff_policy_v29!(
    UnqualifiedInteger,
    IntegerHandoff,
    unqualified_integer_output_v18
);

source_handoff_policy_v29!(@impl BoundScalar, BoundHandoff,
    |source, roots, context, budget, handoff| {
        let (launches, width) = context.launches(source, budget)?;
        let handoff = source.checked_bound_scalar_output_v19(
            ProductionKernelArgumentAbiInputV18 { roots }, &launches, width, budget,
        )?;
    }, [
        formal_context_v19::launch_context_headers_v19()?,
        size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
        size_of::<ProductionBoundScalarHandoffErrorV19>(),
    ]
);

#[path = "production_pipeline_source_bound_worklist_v21.rs"]
mod bound_worklist_v21;

#[path = "production_pipeline_source_mixed_worklist_v26.rs"]
mod mixed_worklist_v26;

#[path = "production_pipeline_source_mixed_pure_cse_v26.rs"]
mod mixed_pure_cse_v26;

#[path = "production_pipeline_source_mixed_cfg_v27.rs"]
mod mixed_cfg_v27;

#[path = "production_pipeline_source_mixed_fixedpoint_licm_v29.rs"]
pub(crate) mod mixed_fixedpoint_licm_v29;
#[path = "production_pipeline_source_mixed_licm_v28.rs"]
mod mixed_licm_v28;

#[path = "production_pipeline_source_mixed_relocation_v28.rs"]
mod mixed_relocation_v28;

#[path = "production_pipeline_source_mixed_worker_v28.rs"]
pub(crate) mod mixed_worker_v28;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// The fixed source profile is selected before any semantic admission.
    /// The continuation borrows the genuine source and move-only adopted output
    /// on one ledger; neither can escape. No count/report is executable authority.
    pub(crate) fn with_source_owned_scalar_handoff_v29<R, F>(self, consume: F) -> Result<R, Error>
    where
        F: for<'view, 'source, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_custody_v29(consume)
            .map(SourceOwnedCompilationContinuationV29::into_observation)
    }

    /// Preserve the actual compiler bindings across the callback without
    /// widening the closed scalar admission or authorizing its observed result.
    pub(crate) fn with_source_owned_scalar_custody_v29<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_custody_limits_v29(
            WORK_LIMIT,
            STORAGE_LIMIT,
            move |source, handoff, _, _, budget| consume(source, handoff, budget),
        )
    }

    fn with_source_owned_scalar_limits_v29<R, F>(
        self,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_custody_limits_v29(work_limit, storage_limit, consume)
            .map(SourceOwnedCompilationContinuationV29::into_observation)
    }

    fn with_source_owned_scalar_custody_limits_v29<R, F>(
        self,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<ClosedScalar, R, F>(
            ImportProfile::SourceOwnedV29,
            work_limit,
            storage_limit,
            consume,
        )
    }

    /// Consumes the actual integer output lexically while original source and
    /// compiler bindings remain alive. It cannot authorize later reconstruction.
    pub(crate) fn with_source_owned_integer_handoff_v29<R, F>(self, consume: F) -> Result<R, Error>
    where
        F: for<'view, 'source, 'work> FnOnce(
            &'view Source<'source>,
            &IntegerHandoff<'view, 'source>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<UnqualifiedInteger, R, _>(
            ImportProfile::SourceOwnedV29,
            WORK_LIMIT,
            STORAGE_LIMIT,
            move |source, handoff, _, _, budget| consume(source, handoff, budget),
        )
        .map(SourceOwnedCompilationContinuationV29::into_observation)
    }

    /// A concrete live-rustc pre-finalization consumer. The real optimized owner
    /// must exist before this refusal. No native session, receipt, descriptor,
    /// proof, or publication authority is manufactured to bypass later gates.
    pub(crate) fn source_owned_integer_finalizer_refusal_v29(self) -> Error {
        let result = self.with_source_owned_integer_handoff_v29::<std::convert::Infallible, _>(
            |source, handoff, budget| {
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                handoff.output(budget)?;
                Err(Error::Unsupported(
                    "source-owned integer final admission required",
                ))
            },
        );
        match result {
            Ok(never) => match never {},
            Err(error) => error,
        }
    }

    fn with_source_owned_custody_policy_v29<P: SourceHandoffPolicyV29<R, F>, R, F>(
        self,
        import_profile: ImportProfile,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error> {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        self.with_source_owned_custody_policy_on_account_v29::<P, R, F>(
            import_profile,
            &mut budget,
            consume,
        )
    }

    // Both local observations and protected clients use this same source visit.
    // Root-phase charges remain with the caller's enclosing transaction.
    fn with_source_owned_custody_policy_on_account_v29<P: SourceHandoffPolicyV29<R, F>, R, F>(
        self,
        import_profile: ImportProfile,
        mut budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error> {
        let mut consume = formal_context_v19::PendingConsumerV19::new(consume);
        budget.check_prior_denials_v1()?;
        let ssa = self
            .import_semantic_mir_with_profile_v29(import_profile)?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        if ssa
            .stage
            .bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != ssa.stage.bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch.into());
        }
        // Binding and independent CPU replay are not a semantic proof. Until
        // the source-owned consumer exists, every nonempty obligation set refuses.
        reference_obligations_v69::require_discharged(
            &ssa.stage.semantic_ssa,
            &ssa.stage.bindings,
            budget,
        )?;
        let prepared = ssa.prepare_materialization_inputs_v29(|roots| {
            roots.iter().map(|root| {
                let launch = root.source_launch().ok_or(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup))?;
                Ok(crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                    root.logical_name(), root.kernel_binding_bytes(), launch))
            }).collect()
        })?;
        let headers = P::entry_headers()?;
        budget.charge_work(headers)?;
        budget.reserve_storage(headers)?;
        // Root-phase storage is not refunded across a callback. All actual
        // owned projection payloads remain paid until this ledger is dropped.
        // The source and handoff use their existing linked cleanup internally.
        let PreparedSsaMaterializationV29 {
            semantic_ssa,
            ranked_roots,
            launch,
            bindings,
        } = prepared;
        context_handoff_v29::check_context_handoff_v29(
            &bindings.context_entries,
            &semantic_ssa,
            &launch,
            &mut budget,
            |_, _| Ok(()),
        )?;
        let contexts = bindings
            .context_entries
            .materialization_source_v29(semantic_ssa.source_semantic(), &mut budget)
            .map_err(|error| match error {
                crate::collector::ContextRootVisitErrorV29::Source(error) => {
                    Error::from(ProductionPipelineError::SemanticImport(
                        crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(
                            Box::new(error),
                        ),
                    ))
                }
                crate::collector::ContextRootVisitErrorV29::Resource(error) => {
                    Error::Resource(error)
                }
                crate::collector::ContextRootVisitErrorV29::Consumer(never) => match never {},
            })?;
        let original_sha = *semantic_ssa.source_semantic_sha256();
        let original_ssa = semantic_ssa.identity();
        let binding_context = SourceBindingContextV29 {
            bindings: &bindings,
        };
        let abi = crate::compiler_descriptor::source_owned_v29::SourceAbi::capture(
            &bindings.typed_descriptor_roots,
            &mut budget,
        )?;
        let roots = abi.roots(&mut budget)?;
        let source = prepare_original_source_v29(
            semantic_ssa,
            launch,
            contexts.as_ref(),
            &roots,
            &mut budget,
        )?;
        #[cfg(test)]
        tests::observe_prepared_source_v29(&roots, contexts.is_some());
        let binding_context = &binding_context;
        let visit = move |source: &Source<'_>, budget: &mut Budget<'_>| {
            if source.source_ssa(budget)?.identity() != original_ssa
                || source.source_semantic(budget)?.semantic_sha256().as_bytes() != &original_sha
            {
                return Err(Error::Unsupported("source-owned original identity changed"));
            }
            #[cfg(test)]
            tests::observe_materialized_source_v29();
            P::consume(source, &roots, binding_context, budget, consume.take())
        };
        // Measure the real source-visit capture, including the borrowed compiler
        // context and owned ABI-root vector. It stays paid for this root phase.
        pay_source_visit_capture_v19(&visit, &mut budget)?;
        let result = source.with_source_consumer_v18(&mut budget, visit);
        drop((contexts, abi, ranked_roots));
        let observation = result?;
        Ok(SourceOwnedCompilationContinuationV29 {
            observation,
            original_source: original_sha,
            original_ssa,
            bindings,
        })
    }
}

/// Own the genuine SSA and launch through capture, then pass the real prepared
/// source to its lexical consumer on this same ledger. Full original ABI and
/// retained execution rows are proposals replayed by the lowerer, not permits.
fn prepare_original_source_v29(
    semantic_ssa: fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    launch: fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
    contexts: Option<&crate::collector::RetainedExecutionSourceV29<'_>>,
    roots: &[AbiRoot<'_>],
    budget: &mut Budget<'_>,
) -> Result<fe2o3_lower_mir_kernel::ProductionPreparedSourceV18, Error> {
    if let Some(contexts) = contexts {
        pay_context_preparation_headers_v29(budget)?;
        // Capture owns all rows before this projection releases its temporary
        // backing. Original retained compiler bindings remain live outside.
        return context_handoff_v29::with_projected_execution_source_v29(
            contexts,
            budget,
            move |input, budget| {
                Ok(Pending::prepare_source_with_kernel_abi_budget_v18(
                    semantic_ssa,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots },
                    fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                    budget,
                ))
            },
        )
        .map_err(ProductionPipelineError::ContextHandoff)?
        .map_err(Error::from);
    }
    let original_sha = *semantic_ssa.source_semantic_sha256();
    let mut classes = paid_vec(semantic_ssa.source_semantic().callables().len(), budget)?;
    classes.resize(
        semantic_ssa.source_semantic().callables().len(),
        Class::Ordinary,
    );
    Pending::prepare_source_with_kernel_abi_budget_v18(
        semantic_ssa,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: &original_sha,
            roots: &[],
            classes: &classes,
            events: &[],
        },
        ProductionKernelArgumentAbiInputV18 { roots },
        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .map_err(Error::from)
}

fn context_preparation_headers_v29() -> Result<usize, Resource> {
    type Capture<'a> = (
        fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
        &'a [AbiRoot<'a>],
    );
    type Prepared = Result<
        fe2o3_lower_mir_kernel::ProductionPreparedSourceV18,
        ProductionSourceOwnedViewErrorV18,
    >;
    type Invoke<'a, 'work> = (
        Capture<'a>,
        &'a crate::collector::RetainedExecutionSourceV29<'a>,
        &'a mut Budget<'work>,
        &'a mut usize,
    );
    type Projected = Result<Prepared, fe2o3_lower_mir_kernel::ProductionContextRootErrorV29>;
    [
        size_of::<Capture<'_>>(),
        align_of::<Capture<'_>>(),
        size_of::<Invoke<'_, '_>>(),
        align_of::<Invoke<'_, '_>>(),
        size_of::<AssertUnwindSafe<Invoke<'_, '_>>>(),
        size_of::<Prepared>(),
        align_of::<Prepared>(),
        size_of::<Projected>(),
        align_of::<Projected>(),
        size_of::<std::thread::Result<Projected>>(),
        size_of::<Option<crate::collector::RetainedExecutionSourceV29<'_>>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

fn pay_context_preparation_headers_v29(budget: &mut Budget<'_>) -> Result<(), Error> {
    let headers = context_preparation_headers_v29()?;
    budget.charge_work(headers)?;
    budget.reserve_storage(headers)?;
    Ok(())
}

#[cfg(test)]
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn source_owned_ssa_for_test_v29(
        self,
        source_owned: bool,
    ) -> Result<fe2o3_pliron::ProductionSemanticSsaOwnerV1, Error> {
        let profile = if source_owned {
            ImportProfile::SourceOwnedV29
        } else {
            ImportProfile::Current
        };
        let source = self
            .import_semantic_mir_with_profile_v29(profile)?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        Ok(source.stage.semantic_ssa)
    }

    pub(crate) fn with_source_owned_scalar_test_v29<R, F>(self, consume: F) -> Result<R, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &[AbiRoot<'abi>],
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_limits_v29(
            WORK_LIMIT,
            STORAGE_LIMIT,
            move |source, handoff, roots, _, budget| consume(source, handoff, roots, budget),
        )
    }

    pub(crate) fn with_source_owned_scalar_test_limits_v29<R, F>(
        self,
        storage: usize,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Handoff<'view, 'source>,
            &[AbiRoot<'abi>],
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_limits_v29(
            WORK_LIMIT,
            storage,
            move |source, handoff, roots, _, budget| consume(source, handoff, roots, budget),
        )
    }
}

#[cfg(test)]
#[path = "production_pipeline_source_owned_v29_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use tests::{
    PreparationObservationV29, start_preparation_observation_v29, take_preparation_observation_v29,
};

#[path = "production_pipeline_source_owned_target_llvm_v29.rs"]
mod target_llvm;

#[path = "production_pipeline_source_owned_target_result_v29.rs"]
pub(crate) mod target_result;
