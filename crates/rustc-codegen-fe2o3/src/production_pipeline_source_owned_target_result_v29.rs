//! Backend-private checked source/adopted-owner continuation to inert target LLVM.
use super::{Budget, Handoff, Resource, Source, TargetProfile};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Debug)]
pub(crate) enum ClosedScalarTargetLlvmErrorV29 {
    Source(SourceError),
    Formal(fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18),
    Incomplete(Vec<fe2o3_kernel_ir::FormalMemoryIncompleteReason>),
    Unsupported(&'static str),
    Target(fe2o3_amdgcn_model::LoweringErrors),
}
impl From<SourceError> for ClosedScalarTargetLlvmErrorV29 {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}
impl From<Resource> for ClosedScalarTargetLlvmErrorV29 {
    fn from(error: Resource) -> Self {
        Self::Source(error.into())
    }
}
impl std::fmt::Display for ClosedScalarTargetLlvmErrorV29 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "closed scalar target LLVM IR: {self:?}")
    }
}
impl std::error::Error for ClosedScalarTargetLlvmErrorV29 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Formal(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Incomplete(_) | Self::Unsupported(_) => None,
        }
    }
}
type Error = ClosedScalarTargetLlvmErrorV29;

/// Inert LLVM IR, not final ISA, worker/default compilation or launch authority.
/// The result cannot outlive either real source or adopted V18 handoff.
/// Formal and target engines keep their separate bounded allocation/work
/// policy; only this wrapper, conservatively retained entry frames and actual
/// String capacity are retained credit.
#[must_use = "discard the target text before its borrowed handoff"]
pub(crate) struct ClosedScalarTargetLlvmV29<'handoff, 'view, 'source> {
    source: &'view Source<'source>,
    handoff: &'handoff Handoff<'view, 'source>,
    llvm_ir: String,
    target: TargetProfile,
    retained: usize,
    required: usize,
}
impl ClosedScalarTargetLlvmV29<'_, '_, '_> {
    fn custody(&self, budget: &Budget<'_>) -> Result<(), SourceError> {
        self.handoff
            .observe_retained_storage_v18(self.required, budget)
    }
    fn check(&self, budget: &Budget<'_>) -> Result<(), SourceError> {
        let custody = self.custody(budget);
        self.source
            .check_query_v18(budget)
            .and_then(|()| self.handoff.output(budget).map(|_| ()))
            .and(custody)
    }
    pub(crate) fn llvm_ir(&self, budget: &Budget<'_>) -> Result<&str, SourceError> {
        self.check(budget)?;
        Ok(&self.llvm_ir)
    }
    pub(crate) fn target(&self, budget: &Budget<'_>) -> Result<TargetProfile, SourceError> {
        self.check(budget)?;
        Ok(self.target)
    }
    pub(crate) fn retained_storage(&self, budget: &Budget<'_>) -> Result<usize, SourceError> {
        self.check(budget)?;
        Ok(self.retained)
    }
    pub(crate) fn discard(self, budget: &mut Budget<'_>) -> Result<(), SourceError> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            source,
            llvm_ir,
            retained,
            ..
        } = self;
        drop(llvm_ir);
        let settled = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| source.retain_query_resource_error_v18(error))
        });
        selected?;
        settled
    }
}

fn sum(parts: &[usize]) -> Result<usize, Resource> {
    parts.iter().try_fold(0usize, |sum, &n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })
}

fn headers() -> Result<(usize, usize), Resource> {
    type Capture<'a, 'view, 'source, 'work> = (
        &'view Source<'source>,
        &'a Handoff<'view, 'source>,
        TargetProfile,
        &'a mut Budget<'work>,
        &'a std::cell::Cell<usize>,
        usize,
        usize,
    );
    type Outcome = Result<(String, usize), Error>;
    let retained = sum(&[
        size_of::<ClosedScalarTargetLlvmV29<'_, '_, '_>>(),
        align_of::<ClosedScalarTargetLlvmV29<'_, '_, '_>>(),
    ])?;
    let scratch = sum(&[
        size_of::<Capture<'_, '_, '_, '_>>(),
        align_of::<Capture<'_, '_, '_, '_>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_>>>(),
        size_of::<Outcome>(),
        align_of::<Outcome>(),
        size_of::<std::thread::Result<Outcome>>(),
        size_of::<
            Result<
                fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18,
            >,
        >(),
        size_of::<Result<String, fe2o3_amdgcn_model::LoweringErrors>>(),
        size_of::<Result<(), Error>>(),
        size_of::<fe2o3_kernel_ir::FormalMemoryObligations>(),
        size_of::<[u64; 3]>(),
        size_of::<std::cell::Cell<usize>>(),
        size_of::<Result<(), SourceError>>(),
        align_of::<Result<(), SourceError>>(),
        size_of::<fe2o3_kernel_ir::CanonicalClosedScalarFormalScopeV18<'_>>(),
        size_of::<
            Result<
                fe2o3_kernel_ir::CanonicalClosedScalarFormalScopeV18<'_>,
                fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18,
            >,
        >(),
    ])?;
    Ok((retained, scratch))
}

fn formal(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryObligationAnalysis, LaunchExtent,
    };
    if owner.module().kernels.is_empty() {
        return Err(Error::Unsupported("empty formal root roster"));
    }
    let mut scope =
        fe2o3_kernel_ir::CanonicalClosedScalarFormalScopeV18::new(owner).map_err(Error::Formal)?;
    for kernel in &owner.module().kernels {
        budget.charge_work(sum(&[
            kernel.id.as_str().len(),
            kernel.entry.as_str().len(),
            9,
        ])?)?;
        let mut extents = [1_u64; 3];
        for (axis, extent) in kernel.domain.extents().enumerate() {
            let LaunchExtent::Static(extent) = extent else {
                return Err(Error::Unsupported("dynamic formal launch geometry"));
            };
            extents[axis] = u64::from(extent);
        }
        let report = scope
            .derive(
                &kernel.id,
                ExplicitLaunchExtent::Exact {
                    rank: kernel.domain.rank(),
                    extents,
                },
                FormalIndexWidth::Bits64,
            )
            .map_err(Error::Formal)?;
        let facts = match report {
            FormalMemoryObligationAnalysis::Complete(facts) => facts,
            FormalMemoryObligationAnalysis::Incomplete { reasons, .. } => {
                return Err(Error::Incomplete(reasons));
            }
        };
        if facts.kernel() != &kernel.id
            || facts.entry() != &kernel.entry
            || !facts.allocations().is_empty()
            || !facts.accesses().is_empty()
            || !facts.bounds_requirements().is_empty()
            || !facts.runtime_alias_requirements().is_empty()
            || !facts.inter_invocation_conflicts().is_empty()
        {
            return Err(Error::Unsupported(
                "closed scalar formal obligations remain",
            ));
        }
        drop(facts);
    }
    Ok(())
}

/// The caller binds target selection to its genuine retained rustc target.
/// This function creates no target or runtime publication authority.
pub(crate) fn check_and_lower_target_llvm_v18<'handoff, 'view, 'source>(
    source: &'view Source<'source>,
    handoff: &'handoff Handoff<'view, 'source>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<ClosedScalarTargetLlvmV29<'handoff, 'view, 'source>, Error> {
    // This actual source/SSA join precedes header charging on a foreign budget.
    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
    let floor = budget.storage();
    let accepted = std::cell::Cell::new(0usize);
    let (header, scratch) =
        headers().map_err(|error| source.retain_query_resource_error_v18(error))?;
    let caught = {
        let budget = &mut *budget;
        let accepted = &accepted;
        catch_unwind(AssertUnwindSafe(move || {
            let initial = sum(&[header, scratch])?;
            budget.reserve_storage(initial)?;
            accepted.set(initial);
            let original = source.canonical(budget)?;
            let output = handoff.output(budget)?.owner();
            let before = &original.module().kernels;
            let after = &output.module().kernels;
            budget.charge_work(sum(&[before.len(), after.len(), 1])?)?;
            if before.len() != after.len() {
                return Err(Error::Unsupported("changed target root roster"));
            }
            for (input, output) in before.iter().zip(after) {
                budget.charge_work(sum(&[
                    input.id.as_str().len(),
                    output.id.as_str().len(),
                    input.entry.as_str().len(),
                    output.entry.as_str().len(),
                    4,
                ])?)?;
                if input.id != output.id
                    || input.entry != output.entry
                    || input.domain != output.domain
                    || input.workgroup_size != output.workgroup_size
                {
                    return Err(Error::Unsupported("changed target root or geometry"));
                }
            }
            formal(original, budget)?;
            formal(output, budget)?;
            let text = match target {
                TargetProfile::Gfx942 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
                TargetProfile::Gfx950 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
            }.map_err(Error::Target)?;
            let total = sum(&[initial, text.capacity()])?;
            budget.reserve_storage(text.capacity())?;
            accepted.set(total);
            source.check_query_v18(budget)?;
            handoff.output(budget)?;
            Ok((text, header))
        }))
    };
    // Each addition was accepted by this unchanged budget since the entry
    // floor; no callback or target-engine allocation touches the ledger.
    let required = floor
        .checked_add(accepted.get())
        .expect("accepted storage is representable");
    let custody = handoff.observe_retained_storage_v18(required, budget);
    match caught {
        Ok(Ok((text, _))) if custody.is_ok() => {
            // The result/catch/entry envelopes stay conservatively paid until
            // explicit text disposal; no refund while their frames are live.
            let retained = accepted.get();
            Ok(ClosedScalarTargetLlvmV29 {
                source,
                handoff,
                llvm_ir: text,
                target,
                retained,
                required: budget.storage(),
            })
        }
        Ok(Ok((text, _))) => {
            drop(text);
            Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into())
        }
        Ok(Err(error)) => {
            if custody.is_ok() {
                let _ = budget
                    .release_storage(accepted.get())
                    .map_err(|error| source.retain_query_resource_error_v18(error));
            }
            if let Error::Source(SourceError::Resource(resource)) = &error {
                let selected = source.retain_query_resource_error_v18(*resource);
                return Err(selected.into());
            }
            Err(error)
        }
        Err(payload) => {
            if custody.is_ok() {
                let _ = budget
                    .release_storage(accepted.get())
                    .map_err(|error| source.retain_query_resource_error_v18(error));
            }
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    include!("production_pipeline_source_owned_target_result_v29_tests.rs");
}
