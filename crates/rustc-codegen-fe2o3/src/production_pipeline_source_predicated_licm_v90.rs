//! The exact predicated Policy11 prefix and checked LICM continuation.
use super::*;
use fe2o3_kernel_ir::ExplicitLaunchExtent;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalPredicatedFixedpointOutputHandoffV89 as Prefix,
    ProductionMixedLicmRelocationErrorV28 as MotionError, ProductionMixedSourceHandoffErrorV26,
    ProductionPredicatedFixedpointLicmRelocationV90 as Relocation,
};

fn prepare_prefix<'view, 'source>(
    source: &'view Source<'source>,
    abi: ProductionKernelArgumentAbiInputV18<'_>,
    launches: &'view [ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<Prefix<'view, 'source>, ProductionMixedSourceHandoffErrorV26> {
    source.conditional_predicated_fixedpoint_output_v89(abi, launches, width, budget)
}

fn prepare_relocation<'prefix, 'view, 'source>(
    prefix: &'prefix Prefix<'view, 'source>,
    budget: &mut Budget<'_>,
) -> Result<Relocation<'prefix, 'view, 'source>, MotionError> {
    prefix.prepare_predicated_fixedpoint_licm_v90(budget)
}

include!("production_pipeline_source_mixed_licm_policy_v29.rs");
