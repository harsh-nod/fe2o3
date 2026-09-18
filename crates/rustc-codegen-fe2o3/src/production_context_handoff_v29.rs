//! Project retained source anchors into the shared lowerer check before materialization.
//! Root agreement does not establish callback expansion, scope closure or authority.

use crate::collector::{CallBoundaryV29, ContextRootVisitErrorV29, RetainedContextEntriesV29};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
use fe2o3_lower_mir_kernel::{
    ProductionContextCallBoundaryV29, ProductionContextRootInputV29,
    ProductionSourceLaunchRosterV1, with_checked_context_root_v29,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

use super::ProductionPipelineError;

fn project_boundary(source: &CallBoundaryV29) -> ProductionContextCallBoundaryV29 {
    let (block, statement_count) = source.location();
    let (destination, destination_type) = source.destination();
    ProductionContextCallBoundaryV29 {
        block,
        statement_count,
        destination,
        destination_type,
        target: source.continuation(),
        unwind: source.unwind(),
    }
}

pub(crate) fn check_context_handoff_v29(
    entries: &RetainedContextEntriesV29,
    ssa: &ProductionSemanticSsaOwnerV1,
    launch: &ProductionSourceLaunchRosterV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), ProductionPipelineError> {
    entries
        .visit_root_anchors_v29(ssa.source_semantic(), budget, |entry, budget| {
            budget.charge_work(32)?;
            let (root, root_identity) = entry.root();
            let (helper, helper_identity) = entry.helper();
            let (issuer, issuer_identity) = entry.issuer();
            let (context_type, context_identity) = entry.context();
            let input = ProductionContextRootInputV29 {
                semantic_sha256: ssa.source_semantic_sha256(),
                root,
                root_identity,
                helper,
                helper_identity,
                issuer,
                issuer_identity,
                context_type,
                context_identity,
                issuance: project_boundary(entry.issuance()),
                helper_call: project_boundary(entry.helper_call()),
                helper_context_local: entry.helper_argument(),
                helper_arguments: entry.helper_operands(),
            };
            with_checked_context_root_v29(ssa, launch, input, budget, |_, _| Ok(()))
        })
        .map_err(|error| match error {
            ContextRootVisitErrorV29::Source(error) => ProductionPipelineError::SemanticImport(
                crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(Box::new(
                    error,
                )),
            ),
            ContextRootVisitErrorV29::Resource(error) => {
                ProductionPipelineError::ContextHandoff(error.into())
            }
            ContextRootVisitErrorV29::Consumer(error) => {
                ProductionPipelineError::ContextHandoff(error)
            }
        })
}
