//! Fresh actual-output formal extraction with a closed structural guarded-read rule.

use super::*;
use crate::ProductionFormalMemoryErrorV1 as Error;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalMemoryObligationAnalysis, VerifiedCanonicalKernelIrModuleV12,
    derive_kernel_memory_obligations_for_launch,
};

/// Only this function creates the fresh report and its complete exception list.
/// Neither an N report nor a caller-selected subset of reasons can be supplied.
/// This establishes the existing structural guarded-load rule on actual O, not
/// source correspondence, arbitrary launch bounds, alias discharge or authority.
/// The formal engine and guarded checker retain their separate bounded work and
/// allocation domains; this is not canonical-ledger or whole-compiler accounting.
pub(crate) fn derive_checked_output_guarded_obligations_v1(
    output: &VerifiedCanonicalKernelIrModuleV12,
    max_operations: usize,
) -> Result<Box<[FormalMemoryObligations]>, Error> {
    let module = output.module();
    if module.kernels.is_empty() {
        return Err(Error::KernelCount { actual: 0 });
    }
    let mut kernels = Vec::with_capacity(module.kernels.len());
    for kernel in &module.kernels {
        let extents = crate::production_formal_memory_v1::witness_extents(&kernel.domain);
        let analysis = derive_kernel_memory_obligations_for_launch(
            module,
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            },
            FormalIndexWidth::Bits64,
        )
        .map_err(Error::Analysis)?;
        let obligations =
            match analysis {
                FormalMemoryObligationAnalysis::Complete(obligations) => obligations,
                FormalMemoryObligationAnalysis::Incomplete { partial, reasons } => {
                    if reasons.is_empty() || reasons.iter().any(|reason| {
                        !matches!(
                            reason,
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }
                        )
                    }) {
                        return Err(Error::Incomplete {
                            reasons: reasons.into_boxed_slice(),
                        });
                    }
                    let locations = reasons
                        .iter()
                        .filter_map(|reason| match reason {
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                                location,
                            } => Some(*location),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let result =
                        GuardedAddressProofBudgetV1::new(max_operations).and_then(|mut budget| {
                            guarded_accesses_have_structural_bounds_result(
                                module,
                                kernel,
                                &partial,
                                extents,
                                &locations,
                                max_operations,
                                &mut budget,
                            )
                        });
                    if let Err(detail) = result {
                        return Err(Error::GuardedAccessDischarge {
                            reasons: reasons.into_boxed_slice(),
                            detail,
                        });
                    }
                    partial
                }
            };
        if !obligations.inter_invocation_conflicts().is_empty() {
            return Err(Error::InterInvocationConflicts {
                conflicts: obligations
                    .inter_invocation_conflicts()
                    .to_vec()
                    .into_boxed_slice(),
            });
        }
        kernels.push(obligations);
    }
    Ok(kernels.into_boxed_slice())
}
