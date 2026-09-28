//! Fixed live-rustc MIR29 import and lexical V18 closed scalar continuation.
//! This is not the default compiler route or final ranked/formal/target authority.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as TargetProfile;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
pub(crate) use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionClosedScalarHandoffErrorV18, ProductionClosedScalarOutputHandoffV18 as Handoff,
    ProductionExecutionSourceInputV29, ProductionKernelArgumentAbiInputV18,
    ProductionKernelArgumentAbiRootV18 as AbiRoot,
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionScopeCallableCandidateV29 as Class,
    ProductionSourceOwnedViewErrorV18, ProductionSourceOwnedViewV18 as Source,
};
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

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
    TargetLlvm(target_result::ClosedScalarTargetLlvmErrorV29),
    Resource(Resource),
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
            Self::TargetLlvm(error) => Some(error),
            Self::Resource(error) => Some(error),
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
impl From<target_result::ClosedScalarTargetLlvmErrorV29> for Error {
    fn from(error: target_result::ClosedScalarTargetLlvmErrorV29) -> Self {
        Self::TargetLlvm(error)
    }
}

// These are the existing source-owned qualification limits, not an increase to
// the separate legacy canonical phase. No caller selects a shipping policy.
const WORK_LIMIT: usize = 500_000_000;
const STORAGE_LIMIT: usize = 20_000_000;

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
    type Invoke<'a, 'source, 'work, F> = (
        F,
        &'a Source<'source>,
        &'a Handoff<'a, 'source>,
        &'a [AbiRoot<'a>],
        TargetProfile,
        &'a mut Budget<'work>,
    );
    [
        size_of::<F>(),
        align_of::<F>(),
        size_of::<AssertUnwindSafe<F>>(),
        size_of::<Invoke<'_, '_, '_, F>>(),
        align_of::<Invoke<'_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<Invoke<'_, '_, '_, F>>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        size_of::<AssertUnwindSafe<Result<R, Error>>>(),
        size_of::<PreparedSsaMaterializationV29>(),
        align_of::<PreparedSsaMaterializationV29>(),
        size_of::<Vec<Class>>(),
        size_of::<Vec<AbiRoot<'_>>>(),
        size_of::<ProductionKernelArgumentAbiInputV18<'_>>(),
        size_of::<ProductionExecutionSourceInputV29<'_>>(),
        size_of::<Work>(),
        size_of::<Budget<'_>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

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
        self.with_source_owned_scalar_limits_v29(
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
        let ssa = self
            .import_semantic_mir_with_profile_v29(ImportProfile::SourceOwnedV29)?
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
        // Functional-reference obligations need their genuine later consumer.
        if !ssa
            .stage
            .bindings
            .reference_effect_bindings
            .as_slice()
            .is_empty()
        {
            return Err(Error::Unsupported(
                "source-owned scalar reference obligations",
            ));
        }
        let prepared = ssa.prepare_materialization_inputs_v29(|roots| {
            roots.iter().map(|root| {
                let launch = root.source_launch().ok_or(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup))?;
                Ok(crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                    root.logical_name(), root.kernel_binding_bytes(), launch))
            }).collect()
        })?;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let headers = entry_headers::<R, F>()?;
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
        if contexts.is_some() {
            return Err(Error::Unsupported("closed scalar context provider"));
        }
        drop(contexts);
        let original_sha = *semantic_ssa.source_semantic_sha256();
        let original_ssa = semantic_ssa.identity();
        let target = bindings.rustc_target.profile();
        let abi = crate::compiler_descriptor::source_owned_v29::ScalarAbi::capture(
            &bindings.typed_descriptor_roots,
            &mut budget,
        )?;
        let roots = abi.roots(&mut budget)?;
        let mut classes = paid_vec(
            semantic_ssa.source_semantic().callables().len(),
            &mut budget,
        )?;
        classes.resize(
            semantic_ssa.source_semantic().callables().len(),
            Class::Ordinary,
        );
        let source = Pending::prepare_source_with_kernel_abi_budget_v18(
            semantic_ssa,
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: &original_sha,
                roots: &[],
                classes: &classes,
                events: &[],
            },
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        let result = source.with_source_consumer_v18(&mut budget, move |source, budget| {
            if source.source_ssa(budget)?.identity() != original_ssa
                || source.source_semantic(budget)?.semantic_sha256().as_bytes() != &original_sha
            {
                return Err(Error::Unsupported("source-owned original identity changed"));
            }
            let handoff = source.checked_closed_scalar_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            handoff.check_original_source(source.source_ssa(budget)?, budget)?;
            let borrowed = &handoff;
            let callback_budget = &mut *budget;
            // The actual F is owned by this catch, including its destructor.
            let original_roots = &roots;
            let result = catch_unwind(AssertUnwindSafe(move || {
                consume(source, borrowed, original_roots, target, callback_budget)
            }));
            let settled = handoff.discard(budget).map_err(Error::from);
            match result {
                Ok(Ok(value)) => {
                    settled?;
                    Ok(value)
                }
                Ok(Err(error)) => Err(error),
                Err(payload) => resume_unwind(payload),
            }
        });
        drop((classes, abi, ranked_roots));
        drop(bindings);
        result
    }
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

#[path = "production_pipeline_source_owned_target_llvm_v29.rs"]
mod target_llvm;

#[path = "production_pipeline_source_owned_target_result_v29.rs"]
pub(crate) mod target_result;
