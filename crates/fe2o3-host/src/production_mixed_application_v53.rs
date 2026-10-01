//! Actual inherited mixed application handoff. Preparation is not load authority.
use super::*;
use crate::{
    CompilerGeneratedKernelExpectationRosterV1, InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    MixedWorkerV53PreparationError, PreparedMixedWorkerV53Invocation,
    WorkerV3CompilerCurrentRecordAuditErrorV1, WorkerV3CompilerCurrentRecordAuditV1,
    consume_inherited_mixed_worker_v53_application_handoff,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionMixedWorkerV53ApplicationError {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Preparation(MixedWorkerV53PreparationError),
    CurrentRecord(WorkerV3CompilerCurrentRecordAuditErrorV1),
}
impl fmt::Display for ProductionMixedWorkerV53ApplicationError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(out, "mixed application handoff failed: {error}"),
            Self::Preparation(error) => {
                write!(out, "mixed application preparation failed: {error}")
            }
            Self::CurrentRecord(error) => {
                write!(
                    out,
                    "mixed application current-record audit failed: {error}"
                )
            }
        }
    }
}
impl Error for ProductionMixedWorkerV53ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::CurrentRecord(error) => Some(error),
        }
    }
}

/// Consumes the real inherited V53 publication, all root contracts, and generated
/// arguments into a current, physically prepared mixed invocation. The existing
/// one-shot service authenticates its exact compiler current record. This does not
/// authenticate the separate protected native proof or authorize KFD loading.
/// The returned storage addition is already reserved and must remain charged
/// until the owner is dropped; do not reserve it a second time.
///
/// # Safety
/// Invoke during cooperative process startup before threads, signal handlers,
/// descendants, or foreign code can observe or mutate inherited descriptors or
/// the handoff environment. Transfer each inherited descriptor exactly once.
pub unsafe fn prepare_inherited_mixed_worker_v53_application<'allocation, R, K, Arguments>(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    arguments: Arguments,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedMixedWorkerV53Invocation<'allocation, R, K>,
        WorkerV3CompilerCurrentRecordAuditV1,
        usize,
    ),
    ProductionMixedWorkerV53ApplicationError,
>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
{
    // SAFETY: the caller transfers the same startup custody as the handoff API.
    let owner = unsafe { consume_inherited_mixed_worker_v53_application_handoff::<R>(budget) }
        .map_err(ProductionMixedWorkerV53ApplicationError::Handoff)?;
    let audit = owner
        .with_verification_request(|request| auditor.audit_mixed_v53(&request))
        .map_err(|error| {
            ProductionMixedWorkerV53ApplicationError::Preparation(
                MixedWorkerV53PreparationError::Admission(error),
            )
        })?
        .map_err(ProductionMixedWorkerV53ApplicationError::CurrentRecord)?;
    let (prepared, retained) = owner
        .prepare_gfx942::<K, Arguments>(
            arguments,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
            budget,
        )
        .map_err(ProductionMixedWorkerV53ApplicationError::Preparation)?;
    Ok((prepared, audit, retained))
}
