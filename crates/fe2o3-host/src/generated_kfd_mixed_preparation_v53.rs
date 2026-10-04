//! The real generated runtime packer, followed by complete nominal/V26 checks.
use super::*;
use fe2o3_kernel_descriptor::{DeviceDescriptorTableV3, mixed_conditional_v26::MixedContractV26};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

pub(crate) fn prepare<'allocation, K, Arguments>(
    table: &DeviceDescriptorTableV3<'_>,
    contract: &MixedContractV26<'_>,
    arguments: Arguments,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
    budget: &mut Budget<'_>,
) -> Result<
    (
        Gfx942RuntimeDispatchInputsV1,
        GeneratedKfdCompletion<'allocation>,
    ),
    crate::MixedWorkerV53PreparationError,
>
where
    K: CompilerGeneratedKernelExpectationV1,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
{
    use crate::MixedWorkerV53PreparationError as Error;
    let generated = Arguments::generated_argument_layout().map_err(Error::arguments)?;
    let kernel_id = KernelId::from_bytes(K::KERNEL_BINDING_ID_V1);
    let plan = crate::generated_argument_plan::mixed_generated_packing_candidate_v53(
        kernel_id, &generated,
    );
    let binding = arguments
        .bind_kfd_arguments(&plan)
        .map_err(Error::arguments)?;
    let (packed, retained_plan) = binding
        .pack_with_conditional_plan_v1(&plan, budget)
        .map_err(Error::arguments)?;
    budget
        .reserve_storage(retained_plan)
        .map_err(Error::arguments)?;
    // The complete ABI validator uses the sealed plan and its exact packing
    // observation. Neither the candidate shape nor kernel id can bypass it.
    let mixed = packed
        .bind_mixed_conditional_premises_v26(table, contract, geometry, budget)
        .map_err(Error::arguments)?;
    budget
        .reserve_storage(mixed.retained_storage())
        .map_err(Error::arguments)?;
    let result = mixed
        .into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds)
        .map_err(Error::Runtime)?;
    // The sealed plan has been dropped by the move into actual runtime inputs.
    // Only the premises remain retained on this incremental ledger.
    budget
        .release_storage(retained_plan)
        .map_err(Error::arguments)?;
    Ok(result)
}
