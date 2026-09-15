use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_analysis::{
    CanonicalRankedViewErrorV1, CheckedCanonicalRankedViewV1,
    materialize_canonical_ranked_views_v1, needs_ranked_projection,
    prepare_canonical_ranked_view_v1,
};
use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrV13};
use pliron::{builtin::ops::ModuleOp, context::Context, op::Op};

use super::ProductionFinalGraphVerificationErrorV1 as Error;
use crate::{KirBridgeErrorV1, PlironSession};

pub(super) fn materialize(
    canonical: &VerifiedCanonicalKernelIrV13,
    epoch: u64,
    module: &Module,
    session: &mut PlironSession,
) -> Result<Vec<Option<CheckedCanonicalRankedViewV1>>, Error> {
    let plans = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .map(|function| {
            needs_ranked_projection(function)
                .then(|| prepare_canonical_ranked_view_v1(canonical, epoch, &function.id))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(projection)?;
    if plans.iter().all(Option::is_none) {
        return Ok(plans.into_iter().map(|_| None).collect());
    }
    let work = plans.iter().flatten().try_fold(3usize, |work, plan| {
        work.checked_add(plan.tree_work())
            .ok_or(Error::Bridge(KirBridgeErrorV1::SizeOverflow))
    })?;
    session
        .require_internal_tree_capacity(work)
        .map_err(bridge)?;
    let root = session
        .create_module("final_ranked_analysis")
        .map_err(bridge)?;
    let pointer = session
        .operations
        .get(&root.identity)
        .copied()
        .ok_or(Error::GraphSubjectMismatch)?;
    // Preflight completed before allocation. A partial build cannot be reused.
    let built = catch_unwind(AssertUnwindSafe(|| {
        let root = ModuleOp::from_operation(pointer);
        materialize_canonical_ranked_views_v1(&mut session.context, &root, plans)
    }));
    let views = match built {
        Ok(Ok(views)) => views,
        Ok(Err(error)) => {
            session.poisoned = true;
            return Err(projection(error));
        }
        Err(_) => {
            session.poisoned = true;
            return Err(Error::Bridge(KirBridgeErrorV1::UpstreamPanicked));
        }
    };
    session
        .finish_internal_root_construction(&root)
        .map_err(bridge)?;
    revalidate(&views, &session.context, canonical, epoch, module)?;
    Ok(views)
}

pub(super) fn revalidate(
    views: &[Option<CheckedCanonicalRankedViewV1>],
    context: &Context,
    canonical: &VerifiedCanonicalKernelIrV13,
    epoch: u64,
    module: &Module,
) -> Result<(), Error> {
    let functions = module
        .functions
        .iter()
        .filter(|function| function.body.is_some());
    if views.len() != functions.clone().count() {
        return Err(Error::GraphSubjectMismatch);
    }
    for (view, function) in views.iter().zip(functions) {
        if view.is_some() != needs_ranked_projection(function) {
            return Err(Error::GraphSubjectMismatch);
        }
        if let Some(view) = view {
            if view.function_id() != &function.id {
                return Err(Error::GraphSubjectMismatch);
            }
            view.revalidate(context, canonical, epoch)
                .map_err(projection)?;
        }
    }
    Ok(())
}

fn projection(error: CanonicalRankedViewErrorV1) -> Error {
    Error::AnalysisProjection(Box::new(error))
}

fn bridge(error: crate::OperationHandleError) -> Error {
    Error::Bridge(error.into())
}
