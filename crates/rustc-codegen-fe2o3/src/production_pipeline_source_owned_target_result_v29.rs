//! Backend-private checked source/adopted-owner continuation to inert target LLVM.
use super::{BoundHandoff, Budget, Handoff, Resource, Source, TargetProfile};
use fe2o3_lower_mir_kernel::ProductionScalarCfgOutputHandoffV18 as CfgHandoff;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Debug)]
pub(crate) enum ClosedScalarTargetLlvmErrorV29 {
    Source(SourceError),
    Formal(fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18),
    ScalarCfgFormal(fe2o3_kernel_ir::CanonicalScalarCfgFormalErrorV18),
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
            Self::ScalarCfgFormal(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Incomplete(_) | Self::Unsupported(_) => None,
        }
    }
}
type Error = ClosedScalarTargetLlvmErrorV29;

include!("production_pipeline_source_owned_target_policy_v29.rs");

#[path = "production_pipeline_source_owned_target_mixed_v26.rs"]
pub(crate) mod mixed_v26;

#[path = "production_pipeline_source_owned_target_mixed_pure_cse_v26.rs"]
pub(crate) mod mixed_pure_cse_v26;

pub(crate) type ClosedScalarTargetLlvmV29<'handoff, 'view, 'source> =
    TargetLlvmV29<'handoff, 'view, 'source, Handoff<'view, 'source>>;
pub(crate) type ScalarCfgTargetLlvmV29<'handoff, 'view, 'source> =
    TargetLlvmV29<'handoff, 'view, 'source, CfgHandoff<'view, 'source>>;
pub(crate) type BoundScalarTargetLlvmV19<'handoff, 'view, 'source> =
    TargetLlvmV29<'handoff, 'view, 'source, BoundHandoff<'view, 'source>>;

/// Inert LLVM IR, not final ISA, worker/default compilation or launch authority.
/// The result cannot outlive either real source or adopted V18 handoff.
/// Policy checks and target engines keep their separate bounded allocation/work
/// policy; only this wrapper, conservatively retained entry frames and actual
/// String capacity are retained credit.
#[must_use = "discard the target text before its borrowed handoff"]
pub(crate) struct TargetLlvmV29<'handoff, 'view, 'source, H: TargetOutputHandoffV29> {
    source: &'view Source<'source>,
    handoff: &'handoff H,
    llvm_ir: String,
    target: TargetProfile,
    retained: usize,
    required: usize,
}
impl<H: TargetOutputHandoffV29> TargetLlvmV29<'_, '_, '_, H> {
    fn custody(&self, budget: &Budget<'_>) -> Result<(), SourceError> {
        self.handoff.observe_retained_storage(self.required, budget)
    }
    fn check(&self, budget: &Budget<'_>) -> Result<(), SourceError> {
        let custody = self.custody(budget);
        self.source
            .check_query_v18(budget)
            .and_then(|()| self.handoff.owner(budget).map(|_| ()))
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
    headers_for::<Handoff<'_, '_>>()
}

fn headers_for<H: TargetOutputHandoffV29>() -> Result<(usize, usize), Resource> {
    type Capture<'a, 'view, 'source, 'work, H> = (
        &'view Source<'source>,
        &'a H,
        TargetProfile,
        &'a mut Budget<'work>,
        &'a std::cell::Cell<usize>,
        usize,
        usize,
    );
    type Outcome = Result<(String, usize), Error>;
    let retained = sum(&[
        size_of::<TargetLlvmV29<'_, '_, '_, H>>(),
        align_of::<TargetLlvmV29<'_, '_, '_, H>>(),
    ])?;
    let scratch = sum(&[
        size_of::<Capture<'_, '_, '_, '_, H>>(),
        align_of::<Capture<'_, '_, '_, '_, H>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_, H>>>(),
        size_of::<Outcome>(),
        align_of::<Outcome>(),
        size_of::<std::thread::Result<Outcome>>(),
        H::formal_headers()?,
        size_of::<Result<String, fe2o3_amdgcn_model::LoweringErrors>>(),
        size_of::<Result<(), Error>>(),
        size_of::<fe2o3_kernel_ir::FormalMemoryObligations>(),
        size_of::<[u64; 3]>(),
        size_of::<std::cell::Cell<usize>>(),
        size_of::<Result<(), SourceError>>(),
        align_of::<Result<(), SourceError>>(),
    ])?;
    Ok((retained, scratch))
}

fn formal<S: TargetFormalScopeV29>(
    mut scope: S,
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryObligationAnalysis, LaunchExtent,
    };
    if owner.module().kernels.is_empty() {
        return Err(Error::Unsupported("empty formal root roster"));
    }
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
        let report = scope.derive(
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            },
            FormalIndexWidth::Bits64,
        )?;
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
            return Err(Error::Unsupported(S::RESIDUAL));
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
    check_and_lower_target_llvm(source, handoff, target, budget)
}

pub(crate) fn check_and_lower_scalar_cfg_target_llvm_v18<'handoff, 'view, 'source>(
    source: &'view Source<'source>,
    handoff: &'handoff CfgHandoff<'view, 'source>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<ScalarCfgTargetLlvmV29<'handoff, 'view, 'source>, Error> {
    check_and_lower_target_llvm(source, handoff, target, budget)
}

pub(crate) fn check_and_lower_bound_scalar_target_llvm_v19<'handoff, 'view, 'source>(
    source: &'view Source<'source>,
    handoff: &'handoff BoundHandoff<'view, 'source>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<BoundScalarTargetLlvmV19<'handoff, 'view, 'source>, Error> {
    check_and_lower_target_llvm(source, handoff, target, budget)
}

fn check_and_lower_target_llvm<'handoff, 'view, 'source, H: TargetOutputHandoffV29>(
    source: &'view Source<'source>,
    handoff: &'handoff H,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<TargetLlvmV29<'handoff, 'view, 'source, H>, Error> {
    // This actual source/SSA join precedes header charging on a foreign budget.
    handoff.check_original(source.source_ssa(budget)?, budget)?;
    let floor = budget.storage();
    let accepted = std::cell::Cell::new(0usize);
    let (header, scratch) =
        headers_for::<H>().map_err(|error| source.retain_query_resource_error_v18(error))?;
    let caught = {
        let budget = &mut *budget;
        let accepted = &accepted;
        catch_unwind(AssertUnwindSafe(move || {
            let initial = sum(&[header, scratch])?;
            budget.reserve_storage(initial)?;
            accepted.set(initial);
            let original = source.canonical(budget)?;
            let output = handoff.owner(budget)?;
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
            handoff.formal(original, budget)?;
            handoff.formal(output, budget)?;
            let text = match target {
                TargetProfile::Gfx942 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
                TargetProfile::Gfx950 => fe2o3_amdgcn_model::lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
            }.map_err(Error::Target)?;
            let total = sum(&[initial, text.capacity()])?;
            budget.reserve_storage(text.capacity())?;
            accepted.set(total);
            source.check_query_v18(budget)?;
            handoff.owner(budget)?;
            Ok((text, header))
        }))
    };
    // Each addition was accepted by this unchanged budget since the entry
    // floor; no callback or target-engine allocation touches the ledger.
    let required = floor
        .checked_add(accepted.get())
        .expect("accepted storage is representable");
    let custody = handoff.observe_retained_storage(required, budget);
    match caught {
        Ok(Ok((text, _))) if custody.is_ok() => {
            // The result/catch/entry envelopes stay conservatively paid until
            // explicit text disposal; no refund while their frames are live.
            let retained = accepted.get();
            Ok(TargetLlvmV29 {
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
