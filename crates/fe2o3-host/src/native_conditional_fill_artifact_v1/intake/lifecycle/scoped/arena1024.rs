//! One actual host proof/currentness loan roots every original arena carrier.

use super::*;
use crate::generated_runtime_carrier::GeneratedRegistryCarrierV1;
use fe2o3_runtime::{
    RuntimeGfx942ArenaPreparationErrorV1, RuntimeGfx942GeneratedArena1024V1,
    RuntimeGfx942GeneratedIndependentArena1024V1, RuntimeGfx942RegistryCompletionCarrierV1,
};

impl<'scope, 'work> NativeConditionalFillInvocationScopeV1<'scope, 'work> {
    /// Prepares a distinct 1024-member independent-disjoint-WO arena from this
    /// actual retained proof/source owner. Every member remains the checked
    /// closed-fill family; the aggregate requires exact disjoint output ranges
    /// and no input dependencies before native effects. Neither ordering nor
    /// varied logical counts claim measured overlap or out-of-order completion.
    /// Original authority probes and budgets are unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_independent_arena1024<'a, K, A>(
        &'a self,
        metadata: &fe2o3_resource_accounting::ResourceCreditAccountV1,
        device: &CheckedGfx942XnackMinusDevice,
        arguments: &mut dyn FnMut(usize) -> Result<(A, AqlDispatchGeometryV1)>,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        RuntimeGfx942GeneratedIndependentArena1024V1<
            impl RuntimeGfx942RegistryCompletionCarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let arena = RuntimeGfx942GeneratedIndependentArena1024V1::try_new(
            metadata,
            device.observation().unique_id(),
            |index| {
                let (arguments, geometry) = arguments(index)?;
                GeneratedRegistryCarrierV1::new(self.prepare_carrier_with_order::<K, A>(
                    arguments,
                    device,
                    geometry,
                    timeout_milliseconds,
                    limits,
                    result_budget,
                    FillOrderV1::IndependentDisjointWriteOnly,
                )?)
                .map_err(failure)
            },
        )
        .map_err(|error| match error {
            RuntimeGfx942ArenaPreparationErrorV1::Member { error, .. } => error,
            RuntimeGfx942ArenaPreparationErrorV1::Capacity => {
                failure("independent arena original metadata capacity")
            }
            RuntimeGfx942ArenaPreparationErrorV1::Source(error) => failure(error),
        })?;
        self.revalidate()?;
        Ok(arena)
    }

    /// Prepares exactly 1024 original native closed-fill invocations, in order,
    /// using heap metadata charged before the first `arguments` callback.
    /// Each callback supplies one original generated argument value and geometry;
    /// its separately charged result observer remains with the caller.
    ///
    /// All source residuals, authorities and original account loans remain until
    /// actual common native destruction. Positive logical counts may differ;
    /// full workgroups still use the admitted closed64 profile. The arena is
    /// ordered and single-use, not independent-order/hardware-depth qualification.
    /// Neither this factory nor its runtime scope replenishes proof/probe budgets.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_arena1024<'a, K, A>(
        &'a self,
        metadata: &fe2o3_resource_accounting::ResourceCreditAccountV1,
        device: &CheckedGfx942XnackMinusDevice,
        arguments: &mut dyn FnMut(usize) -> Result<(A, AqlDispatchGeometryV1)>,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        RuntimeGfx942GeneratedArena1024V1<
            impl RuntimeGfx942RegistryCompletionCarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let arena = RuntimeGfx942GeneratedArena1024V1::try_new(
            metadata,
            device.observation().unique_id(),
            |index| {
                let (arguments, geometry) = arguments(index)?;
                GeneratedRegistryCarrierV1::new(self.prepare_carrier::<K, A>(
                    arguments,
                    device,
                    geometry,
                    timeout_milliseconds,
                    limits,
                    result_budget,
                )?)
                .map_err(failure)
            },
        )
        .map_err(|error| match error {
            RuntimeGfx942ArenaPreparationErrorV1::Member { error, .. } => error,
            RuntimeGfx942ArenaPreparationErrorV1::Capacity => {
                failure("arena original metadata capacity")
            }
            RuntimeGfx942ArenaPreparationErrorV1::Source(error) => failure(error),
        })?;
        self.revalidate()?;
        Ok(arena)
    }
}
